use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{body::Body, http::{Request, StatusCode}};
use AutoOpenBrowser::{
    build_test_app,
    api::routes::build_router,
    app::build_app_state,
    db::init::init_db,
    runner::fake::FakeRunner,
    domain::{
        run::{RUN_STATUS_FAILED, RUN_STATUS_RUNNING},
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING, TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
    runner::engine::reclaim_stale_running_tasks,
};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;

fn unique_db_url() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("sqlite:///tmp/auto_open_browser_test_{nanos}.db")
}

async fn json_response(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("request should succeed");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let json = serde_json::from_slice::<Value>(&body).expect("json body");
    (status, json)
}

async fn text_response(app: &axum::Router, request: Request<Body>) -> (StatusCode, String) {
    let response = app.clone().oneshot(request).await.expect("request should succeed");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let text = String::from_utf8(body.to_vec()).expect("utf8 body");
    (status, text)
}

async fn create_task(app: &axum::Router, kind: &str) -> String {
    let payload = serde_json::json!({
        "kind": kind,
        "url": "https://example.com",
        "timeout_seconds": 5
    });
    let (status, json) = json_response(
        app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    json.get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string()
}

async fn wait_for_terminal_status(app: &axum::Router, task_id: &str) -> Value {
    for _ in 0..20 {
        let (_, json) = json_response(
            app,
            Request::builder()
                .uri(format!("/tasks/{task_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        let status = json.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if !matches!(status, TASK_STATUS_QUEUED | TASK_STATUS_RUNNING) {
            return json;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("task did not reach terminal status in time");
}

#[tokio::test]
async fn task_with_fingerprint_profile_is_injected_into_runner_result() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let profile_payload = serde_json::json!({
        "id": "fp-desktop-chrome",
        "name": "Desktop Chrome",
        "profile_json": {
            "browser": {"name": "chrome", "version": "123"},
            "os": {"name": "macos", "version": "14.4"},
            "headers": {"accept_language": "en-US,en;q=0.9"}
        }
    });

    let (profile_status, _profile_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/fingerprint-profiles")
            .header("content-type", "application/json")
            .body(Body::from(profile_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(profile_status, StatusCode::CREATED);

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-desktop-chrome"
    });
    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();

    let _task = wait_for_terminal_status(&app, &task_id).await;

    let (result_json_text, fp_id, fp_version): (Option<String>, Option<String>, Option<i64>) = sqlx::query_as(
        r#"SELECT result_json, fingerprint_profile_id, fingerprint_profile_version FROM tasks WHERE id = ?"#,
    )
    .bind(&task_id)
    .fetch_one(&_state.db)
    .await
    .expect("load task result");

    assert_eq!(fp_id.as_deref(), Some("fp-desktop-chrome"));
    assert_eq!(fp_version, Some(1));

    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse result json");
    let fingerprint = result_json.get("fingerprint_profile").expect("fingerprint profile in runner result");
    assert_eq!(fingerprint.get("id").and_then(|v| v.as_str()), Some("fp-desktop-chrome"));
    assert_eq!(fingerprint.get("version").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(
        fingerprint
            .get("profile")
            .and_then(|v| v.get("browser"))
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str()),
        Some("chrome")
    );
}

#[tokio::test]
async fn task_with_missing_fingerprint_profile_runs_without_injected_profile() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = "task-missing-fingerprint-profile".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (
            id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
            priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at,
            fingerprint_profile_id, fingerprint_profile_version, result_json, error_message
        ) VALUES (?, 'open_page', ?, '{"url":"https://example.com","timeout_seconds":5}', NULL, NULL,
                  0, '1', '1', NULL, NULL, NULL, NULL, 'fp-missing', 7, NULL, NULL)"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_QUEUED)
    .execute(&state.db)
    .await
    .expect("insert queued task with missing profile");

    let _task = wait_for_terminal_status(&app, &task_id).await;

    let (result_json_text, fp_id, fp_version): (Option<String>, Option<String>, Option<i64>) = sqlx::query_as(
        r#"SELECT result_json, fingerprint_profile_id, fingerprint_profile_version FROM tasks WHERE id = ?"#,
    )
    .bind(&task_id)
    .fetch_one(&state.db)
    .await
    .expect("load task result");

    assert_eq!(fp_id.as_deref(), Some("fp-missing"));
    assert_eq!(fp_version, Some(7));

    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse result json");
    assert!(result_json.get("fingerprint_profile").unwrap_or(&Value::Null).is_null());
}

#[tokio::test]
async fn inactive_fingerprint_profile_is_rejected_at_task_creation() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO fingerprint_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
           VALUES ('fp-inactive', 'Inactive', 3, 'inactive', NULL, '{"browser":{"name":"chrome"}}', '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert inactive fingerprint profile");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-inactive"
    });
    let (status, body) = text_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("fingerprint profile not found or inactive"), "unexpected body: {body:?}");
}

#[tokio::test]
async fn task_with_stale_fingerprint_profile_version_runs_without_injected_profile() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO fingerprint_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
           VALUES ('fp-stale', 'Stale', 2, 'active', NULL, '{"browser":{"name":"chrome","version":"124"}}', '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert active fingerprint profile");

    let task_id = "task-stale-fingerprint-version".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (
            id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
            priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at,
            fingerprint_profile_id, fingerprint_profile_version, result_json, error_message
        ) VALUES (?, 'open_page', ?, '{"url":"https://example.com","timeout_seconds":5}', NULL, NULL,
                  0, '1', '1', NULL, NULL, NULL, NULL, 'fp-stale', 1, NULL, NULL)"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_QUEUED)
    .execute(&state.db)
    .await
    .expect("insert queued task with stale profile version");

    let _task = wait_for_terminal_status(&app, &task_id).await;

    let result_json_text: Option<String> = sqlx::query_scalar(
        r#"SELECT result_json FROM tasks WHERE id = ?"#,
    )
    .bind(&task_id)
    .fetch_one(&state.db)
    .await
    .expect("load task result");

    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse result json");
    assert!(result_json.get("fingerprint_profile").unwrap_or(&Value::Null).is_null());
}

#[tokio::test]
async fn fingerprint_resolution_logs_are_recorded_for_resolved_and_missing_profiles() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let profile_payload = serde_json::json!({
        "id": "fp-logging",
        "name": "Logging Profile",
        "profile_json": {
            "timezone": "Asia/Shanghai",
            "locale": "zh-CN"
        }
    });
    let (profile_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/fingerprint-profiles")
            .header("content-type", "application/json")
            .body(Body::from(profile_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(profile_status, StatusCode::CREATED);

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-logging"
    });
    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let _task = wait_for_terminal_status(&app, &task_id).await;

    let resolved_logs: Vec<String> = sqlx::query_scalar(
        r#"SELECT message FROM logs WHERE task_id = ? ORDER BY created_at ASC, id ASC"#,
    )
    .bind(&task_id)
    .fetch_all(&state.db)
    .await
    .expect("load resolved logs");
    assert!(
        resolved_logs.iter().any(|msg| msg.contains("fingerprint profile resolved for runner execution")),
        "resolved logs: {resolved_logs:?}"
    );

    let missing_task_id = "task-missing-fingerprint-log".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (
            id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
            priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at,
            fingerprint_profile_id, fingerprint_profile_version, result_json, error_message
        ) VALUES (?, 'open_page', ?, '{"url":"https://example.com","timeout_seconds":5}', NULL, NULL,
                  0, '1', '1', NULL, NULL, NULL, NULL, 'fp-missing-log', 9, NULL, NULL)"#,
    )
    .bind(&missing_task_id)
    .bind(TASK_STATUS_QUEUED)
    .execute(&state.db)
    .await
    .expect("insert missing profile task");

    let _task = wait_for_terminal_status(&app, &missing_task_id).await;

    let missing_logs: Vec<String> = sqlx::query_scalar(
        r#"SELECT message FROM logs WHERE task_id = ? ORDER BY created_at ASC, id ASC"#,
    )
    .bind(&missing_task_id)
    .fetch_all(&state.db)
    .await
    .expect("load missing logs");
    assert!(
        missing_logs.iter().any(|msg| msg.contains("fingerprint profile requested but not resolved at execution time")),
        "missing logs: {missing_logs:?}"
    );
}

#[tokio::test]
async fn status_and_task_detail_expose_fingerprint_resolution_status() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let profile_payload = serde_json::json!({
        "id": "fp-status",
        "name": "Status Profile",
        "profile_json": {
            "timezone": "Asia/Shanghai",
            "locale": "zh-CN"
        }
    });
    let (profile_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/fingerprint-profiles")
            .header("content-type", "application/json")
            .body(Body::from(profile_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(profile_status, StatusCode::CREATED);

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-status"
    });
    let (create_status, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);
    assert_eq!(create_json.get("fingerprint_resolution_status").and_then(|v| v.as_str()), Some("pending"));
    let task_id = create_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();

    let _task = wait_for_terminal_status(&app, &task_id).await;

    let (task_status, task_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(task_status, StatusCode::OK);
    assert_eq!(task_json.get("fingerprint_resolution_status").and_then(|v| v.as_str()), Some("resolved"));

    let (_, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let latest = status_json.get("latest_tasks").and_then(|v| v.as_array()).expect("latest tasks");
    assert!(latest.iter().any(|task| {
        task.get("id").and_then(|v| v.as_str()) == Some(task_id.as_str())
            && task.get("fingerprint_resolution_status").and_then(|v| v.as_str()) == Some("resolved")
    }));

    let downgraded_task_id = "task-status-downgraded".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (
            id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, fingerprint_profile_id, fingerprint_profile_version, result_json, error_message
        ) VALUES (?, 'open_page', 'succeeded', '{"url":"https://example.com"}', NULL, NULL, 0, '1', '1', '1', '2', NULL, NULL, 'fp-missing-status', 7, '{"fingerprint_profile":null}', NULL)"#,
    )
    .bind(&downgraded_task_id)
    .execute(&state.db)
    .await
    .expect("insert downgraded task");

    let (_, downgraded_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{downgraded_task_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(downgraded_json.get("fingerprint_resolution_status").and_then(|v| v.as_str()), Some("downgraded"));
}

#[tokio::test]
async fn status_exposes_fingerprint_metrics_summary() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, fingerprint_profile_id, fingerprint_profile_version, result_json, error_message)
           VALUES
           ('task-fp-pending', 'open_page', 'queued', '{}', NULL, NULL, 0, '4', '4', NULL, NULL, NULL, NULL, 'fp-a', 1, NULL, NULL),
           ('task-fp-resolved', 'open_page', 'succeeded', '{}', NULL, NULL, 0, '3', '3', '3', '3', NULL, NULL, 'fp-b', 2, '{"fingerprint_profile":{"id":"fp-b","version":2}}', NULL),
           ('task-fp-downgraded', 'open_page', 'succeeded', '{}', NULL, NULL, 0, '2', '2', '2', '2', NULL, NULL, 'fp-c', 3, '{"fingerprint_profile":null}', NULL),
           ('task-fp-none', 'open_page', 'succeeded', '{}', NULL, NULL, 0, '1', '1', '1', '1', NULL, NULL, NULL, NULL, '{"ok":true}', NULL)"#,
    )
    .execute(&state.db)
    .await
    .expect("insert status metric tasks");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let metrics = json.get("fingerprint_metrics").expect("fingerprint metrics");
    assert_eq!(metrics.get("pending").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("resolved").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("downgraded").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("none").and_then(|v| v.as_i64()), Some(1));
}

#[tokio::test]
async fn cancel_after_retry_race_returns_stable_conflict_or_cancelled() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = "task-cancel-after-retry-race".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{}', NULL, NULL, 0, '1', '1', '1', '2', NULL, NULL, NULL, 'failed before retry')"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_FAILED)
    .execute(&state.db)
    .await
    .expect("insert failed task");

    let (retry_status, retry_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/tasks/{task_id}/retry"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(retry_status, StatusCode::OK);
    assert_eq!(retry_json.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_QUEUED));

    let cancel_response = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri(format!("/tasks/{task_id}/cancel"))
            .body(Body::empty())
            .expect("request"),
    ).await.expect("cancel response");
    let cancel_status = cancel_response.status();
    let cancel_body = axum::body::to_bytes(cancel_response.into_body(), usize::MAX).await.expect("body");
    let cancel_json: Value = serde_json::from_slice(&cancel_body).expect("json body");

    assert!(matches!(cancel_status, StatusCode::OK | StatusCode::CONFLICT));
    let final_status: String = sqlx::query_scalar(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_one(&state.db)
        .await
        .expect("final task status");
    assert!(matches!(final_status.as_str(), TASK_STATUS_CANCELLED | TASK_STATUS_QUEUED | TASK_STATUS_RUNNING | TASK_STATUS_SUCCEEDED));
    assert!(cancel_json.get("status").is_some() || cancel_json.get("message").is_some());
}

#[tokio::test]
async fn status_exposes_worker_backoff_parameterization() {
    std::env::set_var("AUTO_OPEN_BROWSER_RUNNER_IDLE_BACKOFF_MIN_MS", "333");
    std::env::set_var("AUTO_OPEN_BROWSER_RUNNER_IDLE_BACKOFF_MAX_MS", "4444");

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=5&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    std::env::remove_var("AUTO_OPEN_BROWSER_RUNNER_IDLE_BACKOFF_MIN_MS");
    std::env::remove_var("AUTO_OPEN_BROWSER_RUNNER_IDLE_BACKOFF_MAX_MS");

    assert_eq!(status, StatusCode::OK);
    let worker = json.get("worker").expect("worker");
    assert_eq!(worker.get("idle_backoff_min_ms").and_then(|v| v.as_u64()), Some(333));
    assert_eq!(worker.get("idle_backoff_max_ms").and_then(|v| v.as_u64()), Some(4444));
}

#[tokio::test]
async fn retry_on_running_task_returns_conflict() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = "task-retry-running-conflict".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{}', NULL, NULL, 0, '1', '1', '1', NULL, 'fake-0', '1', NULL, NULL)"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert running task");

    let (retry_status, retry_body) = text_response(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/tasks/{task_id}/retry"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(retry_status, StatusCode::CONFLICT);
    assert!(retry_body.contains("does not allow retry"), "unexpected body: {retry_body:?}");
}

#[tokio::test]
async fn running_task_without_runner_id_is_not_reclaimed() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db, runner, None, 1);
    let _app = build_router(state.clone());

    let task_id = "task-running-without-runner-id".to_string();
    let run_id = "run-running-without-runner-id".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{}', NULL, NULL, 0, '1', '1', '1', NULL, NULL, NULL, NULL, NULL)"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert running task without runner id");

    sqlx::query(
        r#"INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)
           VALUES (?, ?, ?, 1, 'fake', '1', NULL, NULL)"#,
    )
    .bind(&run_id)
    .bind(&task_id)
    .bind(RUN_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert run");

    let reclaimed = reclaim_stale_running_tasks(&state, 1).await.expect("reclaim");
    assert_eq!(reclaimed, 0);

    let status: String = sqlx::query_scalar(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_one(&state.db)
        .await
        .expect("load task");
    assert_eq!(status, TASK_STATUS_RUNNING);
}

#[tokio::test]
async fn fake_runner_success_flow_is_visible_across_endpoints() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = create_task(&app, "open_page").await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));

    let (_, runs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/runs?limit=5&offset=0"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert!(runs_json.as_array().map(|a| !a.is_empty()).unwrap_or(false));
    assert_eq!(runs_json[0].get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));

    let (_, logs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/logs?limit=10&offset=0"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert!(logs_json.as_array().map(|a| !a.is_empty()).unwrap_or(false));

    let (_, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=5&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert!(status_json.get("latest_tasks").and_then(|v| v.as_array()).map(|a| !a.is_empty()).unwrap_or(false));
}

#[tokio::test]
async fn stale_running_task_can_be_reclaimed_back_to_queue() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db, runner, None, 1);
    let _app = build_router(state.clone());

    let task_id = "task-stale-running".to_string();
    let run_id = "run-stale-running".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{}', NULL, NULL, 0, '1', '1', '1', NULL, 'fake-0', NULL, NULL, NULL)"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert stale task");

    sqlx::query(
        r#"INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)
           VALUES (?, ?, ?, 1, 'fake', '1', NULL, NULL)"#,
    )
    .bind(&run_id)
    .bind(&task_id)
    .bind(RUN_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert stale run");

    let reclaimed = reclaim_stale_running_tasks(&state, 1).await.expect("reclaim");
    assert_eq!(reclaimed, 1);

    let (status, runner_id): (String, Option<String>) = sqlx::query_as(
        r#"SELECT status, runner_id FROM tasks WHERE id = ?"#,
    )
    .bind(&task_id)
    .fetch_one(&state.db)
    .await
    .expect("load task after reclaim");
    assert_eq!(status, TASK_STATUS_QUEUED);
    assert_eq!(runner_id, None);

    let (run_status, error_message): (String, Option<String>) = sqlx::query_as(
        r#"SELECT status, error_message FROM runs WHERE id = ?"#,
    )
    .bind(&run_id)
    .fetch_one(&state.db)
    .await
    .expect("load run after reclaim");
    assert_eq!(run_status, RUN_STATUS_FAILED);
    assert_eq!(error_message.as_deref(), Some("reclaimed after stale running timeout"));
}

#[tokio::test]
async fn running_task_with_fresh_heartbeat_is_not_reclaimed() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db, runner, None, 1);
    let _app = build_router(state.clone());

    let task_id = "task-fresh-heartbeat".to_string();
    let run_id = "run-fresh-heartbeat".to_string();
    let heartbeat_now = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 5)
        .to_string();

    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{}', NULL, NULL, 0, '1', '1', '1', NULL, 'fake-0', ?, NULL, NULL)"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_RUNNING)
    .bind(&heartbeat_now)
    .execute(&state.db)
    .await
    .expect("insert running task");

    sqlx::query(
        r#"INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)
           VALUES (?, ?, ?, 1, 'fake', '1', NULL, NULL)"#,
    )
    .bind(&run_id)
    .bind(&task_id)
    .bind(RUN_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert running run");

    let reclaimed = reclaim_stale_running_tasks(&state, 1).await.expect("reclaim");
    assert_eq!(reclaimed, 0);

    let (status, runner_id): (String, Option<String>) = sqlx::query_as(
        r#"SELECT status, runner_id FROM tasks WHERE id = ?"#,
    )
    .bind(&task_id)
    .fetch_one(&state.db)
    .await
    .expect("load task after reclaim attempt");
    assert_eq!(status, TASK_STATUS_RUNNING);
    assert_eq!(runner_id.as_deref(), Some("fake-0"));
}

#[tokio::test]
async fn queued_task_runs_even_if_memory_queue_entry_is_removed() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = create_task(&app, "open_page").await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));
}

#[tokio::test]
async fn queued_cancel_succeeds_even_if_memory_queue_entry_is_missing() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = create_task(&app, "open_page").await;
    let (cancel_status, cancel_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/tasks/{task_id}/cancel"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(cancel_status, StatusCode::OK);
    assert_eq!(cancel_json.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_CANCELLED));
}


#[tokio::test]
async fn reclaimed_task_can_run_again_to_terminal_state() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = "task-reclaim-rerun".to_string();
    let run_id = "run-reclaim-rerun".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{"url":"https://example.com","timeout_seconds":5}', NULL, NULL, 0, '1', '1', '1', NULL, 'fake-0', NULL, NULL, NULL)"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert stale task");

    sqlx::query(
        r#"INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)
           VALUES (?, ?, ?, 1, 'fake', '1', NULL, NULL)"#,
    )
    .bind(&run_id)
    .bind(&task_id)
    .bind(RUN_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert stale run");

    let reclaimed = reclaim_stale_running_tasks(&state, 1).await.expect("reclaim");
    assert_eq!(reclaimed, 1);

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));
}

#[tokio::test]
async fn retry_on_already_queued_task_returns_idempotent_success() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = create_task(&app, "open_page").await;

    let retry_request = Request::builder()
        .method("POST")
        .uri(format!("/tasks/{task_id}/retry"))
        .body(Body::empty())
        .expect("request");
    let retry_response = app.clone().oneshot(retry_request).await.expect("retry request should succeed");
    let retry_status = retry_response.status();
    let retry_body = axum::body::to_bytes(retry_response.into_body(), usize::MAX).await.expect("retry body");
    let retry_text = String::from_utf8(retry_body.to_vec()).expect("retry utf8");
    if retry_status == StatusCode::OK {
        let retry_json: Value = serde_json::from_str(&retry_text).expect("retry json body");
        assert_eq!(retry_json.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_QUEUED));
        assert!(retry_json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("already queued"));
    } else {
        assert_eq!(retry_status, StatusCode::CONFLICT);
        assert!(retry_text.contains("task status does not allow retry now"));
    }
}


#[tokio::test]
async fn reclaimed_task_retry_endpoint_is_idempotent_and_task_still_completes() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = "task-reclaim-retry".to_string();
    let run_id = "run-reclaim-retry".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{"url":"https://example.com","timeout_seconds":5}', NULL, NULL, 0, '1', '1', '1', NULL, 'fake-0', NULL, NULL, NULL)"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert stale task");

    sqlx::query(
        r#"INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)
           VALUES (?, ?, ?, 1, 'fake', '1', NULL, NULL)"#,
    )
    .bind(&run_id)
    .bind(&task_id)
    .bind(RUN_STATUS_RUNNING)
    .execute(&state.db)
    .await
    .expect("insert stale run");

    let reclaimed = reclaim_stale_running_tasks(&state, 1).await.expect("reclaim");
    assert_eq!(reclaimed, 1);

    let retry_request = Request::builder()
        .method("POST")
        .uri(format!("/tasks/{task_id}/retry"))
        .body(Body::empty())
        .expect("request");
    let retry_response = app.clone().oneshot(retry_request).await.expect("retry request should succeed");
    let retry_status = retry_response.status();
    let retry_body = axum::body::to_bytes(retry_response.into_body(), usize::MAX).await.expect("retry body");
    let retry_text = String::from_utf8(retry_body.to_vec()).expect("retry utf8");
    if retry_status == StatusCode::OK {
        let retry_json: Value = serde_json::from_str(&retry_text).expect("retry json body");
        assert_eq!(retry_json.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_QUEUED));
        assert!(retry_json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("already queued"));
    } else {
        assert_eq!(retry_status, StatusCode::CONFLICT);
        assert!(retry_text.contains("task status does not allow retry now"));
    }

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));
}

#[tokio::test]
async fn cancelled_task_is_not_reclaimed() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db, runner, None, 1);
    let _app = build_router(state.clone());

    let task_id = "task-cancelled-stays-cancelled".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{}', NULL, NULL, 0, '1', '1', '1', '2', 'fake-0', '2', NULL, 'task cancelled while running')"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_CANCELLED)
    .execute(&state.db)
    .await
    .expect("insert cancelled task");

    let reclaimed = reclaim_stale_running_tasks(&state, 1).await.expect("reclaim");
    assert_eq!(reclaimed, 0);

    let status: String = sqlx::query_scalar(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_one(&state.db)
        .await
        .expect("load cancelled task");
    assert_eq!(status, TASK_STATUS_CANCELLED);
}

#[tokio::test]
async fn retry_flow_requeues_timed_out_fake_task() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = create_task(&app, "timeout").await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_TIMED_OUT));

    let (retry_status, retry_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/tasks/{task_id}/retry"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(retry_status, StatusCode::OK);
    assert_eq!(retry_json.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_QUEUED));
}

#[tokio::test]
async fn retry_flow_requeues_failed_fake_task() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = create_task(&app, "fail").await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_FAILED));

    let (retry_status, retry_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri(format!("/tasks/{task_id}/retry"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(retry_status, StatusCode::OK);
    assert_eq!(retry_json.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_QUEUED));
}


#[tokio::test]
async fn proxy_v1_create_list_and_get_work() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "id": "proxy-us-1",
        "scheme": "http",
        "host": "127.0.0.1",
        "port": 8080,
        "region": "us-east",
        "country": "US",
        "provider": "manual",
        "score": 0.95
    });
    let (create_status, create_body) = text_response(&app, Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request")).await;
    assert_eq!(create_status, StatusCode::CREATED, "unexpected create body: {create_body}");
    let create_json: Value = serde_json::from_str(&create_body).expect("create proxy json");
    assert_eq!(create_json.get("id").and_then(|v| v.as_str()), Some("proxy-us-1"));

    let (_, list_json) = json_response(&app, Request::builder().uri("/proxies?limit=10&offset=0").body(Body::empty()).expect("request")).await;
    assert!(list_json.as_array().map(|a| !a.is_empty()).unwrap_or(false));

    let (_, get_json) = json_response(&app, Request::builder().uri("/proxies/proxy-us-1").body(Body::empty()).expect("request")).await;
    assert_eq!(get_json.get("region").and_then(|v| v.as_str()), Some("us-east"));
    assert_eq!(get_json.get("provider").and_then(|v| v.as_str()), Some("manual"));
}

#[tokio::test]
async fn create_task_persists_network_policy_json() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {
            "mode": "required_proxy",
            "region": "us-east",
            "proxy_id": "proxy-us-1",
            "min_score": 0.8
        }
    });
    let (status, json) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request")).await;
    assert_eq!(status, StatusCode::CREATED);
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id");

    let stored: Option<String> = sqlx::query_scalar(r#"SELECT network_policy_json FROM tasks WHERE id = ?"#).bind(task_id).fetch_one(&state.db).await.expect("load task network policy");
    let parsed: Value = serde_json::from_str(stored.as_deref().expect("network policy")).expect("parse network policy");
    assert_eq!(parsed.get("mode").and_then(|v| v.as_str()), Some("required_proxy"));
    assert_eq!(parsed.get("proxy_id").and_then(|v| v.as_str()), Some("proxy-us-1"));
}

#[tokio::test]
async fn proxy_health_is_updated_after_success_and_timeout() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-health-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'manual', 'active', 0.95, 0, 0, NULL, NULL, NULL, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let success_payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-health-1"}
    });
    let (_, success_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(success_payload.to_string()))
            .expect("request"),
    )
    .await;
    let success_task_id = success_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let _ = wait_for_terminal_status(&app, &success_task_id).await;

    let (success_count, failure_count, last_used_at, cooldown_until): (i64, i64, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT success_count, failure_count, last_used_at, cooldown_until FROM proxies WHERE id = 'proxy-health-1'"#)
            .fetch_one(&state.db)
            .await
            .expect("load proxy after success");
    assert_eq!(success_count, 1);
    assert_eq!(failure_count, 0);
    assert!(last_used_at.is_some());
    assert!(cooldown_until.is_none());

    let timeout_payload = serde_json::json!({
        "kind": "timeout",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-health-1"}
    });
    let (_, timeout_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(timeout_payload.to_string()))
            .expect("request"),
    )
    .await;
    let timeout_task_id = timeout_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let _ = wait_for_terminal_status(&app, &timeout_task_id).await;

    let (success_count2, failure_count2, cooldown_until2): (i64, i64, Option<String>) =
        sqlx::query_as(r#"SELECT success_count, failure_count, cooldown_until FROM proxies WHERE id = 'proxy-health-1'"#)
            .fetch_one(&state.db)
            .await
            .expect("load proxy after timeout");
    assert_eq!(success_count2, 1);
    assert_eq!(failure_count2, 1);
    assert!(cooldown_until2.is_some());
}


#[tokio::test]
async fn proxy_selection_filters_provider_and_cooldown() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-cooldown', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99, 0, 0, NULL, NULL, '9999999999', '1', '1')"#).execute(&state.db).await.expect("insert cooldown proxy");
    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-allowed', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.90, 0, 0, NULL, NULL, NULL, '1', '1')"#).execute(&state.db).await.expect("insert allowed proxy");
    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-other-provider', 'http', '127.0.0.3', 8082, NULL, NULL, 'us-east', 'US', 'pool-b', 'active', 0.95, 0, 0, NULL, NULL, NULL, '1', '1')"#).execute(&state.db).await.expect("insert other proxy");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "region": "us-east", "provider": "pool-a", "min_score": 0.8}
    });
    let (_, json) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request")).await;
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let _ = wait_for_terminal_status(&app, &task_id).await;

    let result_json_text: Option<String> = sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#).bind(&task_id).fetch_one(&state.db).await.expect("load result");
    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse result");
    assert_eq!(result_json.get("proxy").and_then(|v| v.get("id")).and_then(|v| v.as_str()), Some("proxy-allowed"));
}

#[tokio::test]
async fn proxy_selection_reuses_sticky_session_when_available() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-sticky-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.91, 0, 0, NULL, NULL, NULL, '1', '1')"#).execute(&state.db).await.expect("insert sticky proxy");
    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-sticky-2', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99, 0, 0, NULL, NULL, NULL, '1', '1')"#).execute(&state.db).await.expect("insert fallback proxy");

    let sticky = "session-alpha";
    let payload1 = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/1",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-a", "sticky_session": sticky}
    });
    let (_, json1) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload1.to_string())).expect("request")).await;
    let task_id1 = json1.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let _ = wait_for_terminal_status(&app, &task_id1).await;

    let result1_text: Option<String> = sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#).bind(&task_id1).fetch_one(&state.db).await.expect("load result1");
    let result1: Value = serde_json::from_str(result1_text.as_deref().expect("result1 json")).expect("parse result1");
    let first_proxy_id = result1.get("proxy").and_then(|v| v.get("id")).and_then(|v| v.as_str()).expect("first proxy").to_string();

    let payload2 = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/2",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-a", "sticky_session": sticky}
    });
    let (_, json2) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload2.to_string())).expect("request")).await;
    let task_id2 = json2.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let _ = wait_for_terminal_status(&app, &task_id2).await;

    let result2_text: Option<String> = sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#).bind(&task_id2).fetch_one(&state.db).await.expect("load result2");
    let result2: Value = serde_json::from_str(result2_text.as_deref().expect("result2 json")).expect("parse result2");
    assert_eq!(result2.get("proxy").and_then(|v| v.get("id")).and_then(|v| v.as_str()), Some(first_proxy_id.as_str()));
}


#[tokio::test]
async fn status_and_task_detail_expose_proxy_metrics_and_identity() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let proxy_payload = serde_json::json!({
        "id": "proxy-observe-1",
        "scheme": "http",
        "host": "127.0.0.1",
        "port": 8080,
        "region": "us-east",
        "country": "US",
        "provider": "pool-observe",
        "score": 0.95
    });
    let (proxy_status, _) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(proxy_status, StatusCode::CREATED);

    let task_payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-observe-1"}
    });
    let (create_status, create_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(task_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(create_status, StatusCode::CREATED);
    let task_id = create_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();

    let task_detail = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task_detail.get("proxy_id").and_then(|v| v.as_str()), Some("proxy-observe-1"));
    assert_eq!(task_detail.get("proxy_provider").and_then(|v| v.as_str()), Some("pool-observe"));
    assert_eq!(task_detail.get("proxy_region").and_then(|v| v.as_str()), Some("us-east"));
    assert!(matches!(task_detail.get("proxy_resolution_status").and_then(|v| v.as_str()), Some("resolved") | Some("resolved_sticky")));

    let (_, status_json) = json_response(
        &app,
        Request::builder().uri("/status?limit=10&offset=0").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(status_json.get("proxy_metrics").and_then(|v| v.get("resolved")).and_then(|v| v.as_i64()), Some(1));
    let latest = status_json.get("latest_tasks").and_then(|v| v.as_array()).expect("latest tasks");
    assert!(latest.iter().any(|task| task.get("proxy_id").and_then(|v| v.as_str()) == Some("proxy-observe-1")));
}


#[tokio::test]
async fn proxy_smoke_test_marks_unreachable_proxy_failed() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let proxy_payload = serde_json::json!({
        "id": "proxy-smoke-dead",
        "scheme": "http",
        "host": "127.0.0.1",
        "port": 65534,
        "region": "local",
        "country": "ZZ",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (smoke_status, smoke_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/proxy-smoke-dead/smoke").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(smoke_status, StatusCode::OK);
    assert_eq!(smoke_json.get("reachable").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(smoke_json.get("protocol_ok").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(smoke_json.get("upstream_ok").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(smoke_json.get("anonymity_level").and_then(|v| v.as_str()), None);

    let (failure_count, last_checked_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok): (i64, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>) =
        sqlx::query_as(r#"SELECT failure_count, last_checked_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok FROM proxies WHERE id = 'proxy-smoke-dead'"#)
            .fetch_one(&state.db)
            .await
            .expect("load proxy after smoke test");
    assert_eq!(failure_count, 1);
    assert!(last_checked_at.is_some());
    assert!(cooldown_until.is_some());
    assert_eq!(last_smoke_status.as_deref(), Some("failed"));
    assert_eq!(last_smoke_protocol_ok, Some(0));
    assert_eq!(last_smoke_upstream_ok, Some(0));
}


#[tokio::test]
async fn sticky_session_binding_table_is_written_and_reused() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-bind-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.91, 0, 0, NULL, NULL, NULL, '1', '1')"#).execute(&state.db).await.expect("insert bind proxy 1");
    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-bind-2', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99, 0, 0, NULL, NULL, NULL, '1', '1')"#).execute(&state.db).await.expect("insert bind proxy 2");

    let sticky = "session-bind-alpha";
    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/1",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-a", "sticky_session": sticky}
    });
    let (_, json1) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request")).await;
    let task_id1 = json1.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let _ = wait_for_terminal_status(&app, &task_id1).await;

    let (bound_proxy_id, bound_provider): (String, Option<String>) = sqlx::query_as(r#"SELECT proxy_id, provider FROM proxy_session_bindings WHERE session_key = ?"#)
        .bind(sticky)
        .fetch_one(&state.db)
        .await
        .expect("load sticky binding");
    assert!(["proxy-bind-1", "proxy-bind-2"].contains(&bound_proxy_id.as_str()));
    assert_eq!(bound_provider.as_deref(), Some("pool-a"));

    let payload2 = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/2",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-a", "sticky_session": sticky}
    });
    let (_, json2) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload2.to_string())).expect("request")).await;
    let task_id2 = json2.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let task2 = wait_for_terminal_status(&app, &task_id2).await;
    assert_eq!(task2.get("proxy_id").and_then(|v| v.as_str()), Some(bound_proxy_id.as_str()));
    assert_eq!(task2.get("proxy_resolution_status").and_then(|v| v.as_str()), Some("resolved_sticky"));
}


#[tokio::test]
async fn status_counts_are_aggregated_correctly() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db, runner, None, 1);
    let app = build_router(state.clone());

    let fixtures = [
        ("task-q", TASK_STATUS_QUEUED),
        ("task-r", TASK_STATUS_RUNNING),
        ("task-s", TASK_STATUS_SUCCEEDED),
        ("task-f", TASK_STATUS_FAILED),
        ("task-t", TASK_STATUS_TIMED_OUT),
        ("task-c", TASK_STATUS_CANCELLED),
    ];
    for (id, status) in fixtures {
        sqlx::query(
            r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
               VALUES (?, 'open_page', ?, '{}', NULL, NULL, 0, '1', '1', NULL, NULL, NULL, NULL, NULL, NULL)"#,
        )
        .bind(id)
        .bind(status)
        .execute(&state.db)
        .await
        .expect("insert fixture task");
    }

    let (_, status_json) = json_response(
        &app,
        Request::builder().uri("/status?limit=10&offset=0").body(Body::empty()).expect("request"),
    ).await;

    let counts = status_json.get("counts").expect("counts");
    assert_eq!(counts.get("total").and_then(|v| v.as_i64()), Some(6));
    assert_eq!(counts.get("queued").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(counts.get("running").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(counts.get("succeeded").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(counts.get("failed").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(counts.get("timed_out").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(counts.get("cancelled").and_then(|v| v.as_i64()), Some(1));
}


#[tokio::test]
async fn proxy_smoke_test_accepts_http_like_proxy_response() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf)).await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established

ip=203.0.113.8
"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-smoke-http",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "local",
        "country": "ZZ",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (smoke_status, smoke_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/proxy-smoke-http/smoke").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(smoke_status, StatusCode::OK);
    assert_eq!(smoke_json.get("reachable").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(smoke_json.get("protocol_ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(smoke_json.get("upstream_ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(smoke_json.get("exit_ip").and_then(|v| v.as_str()), Some("203.0.113.8"));
    assert_eq!(smoke_json.get("anonymity_level").and_then(|v| v.as_str()), Some("elite"));

    let (failure_count, last_checked_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level): (i64, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT failure_count, last_checked_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level FROM proxies WHERE id = 'proxy-smoke-http'"#)
            .fetch_one(&state.db)
            .await
            .expect("load proxy after smoke test success");
    assert_eq!(failure_count, 0);
    assert!(last_checked_at.is_some());
    assert!(cooldown_until.is_none());
    assert_eq!(last_smoke_status.as_deref(), Some("ok"));
    assert_eq!(last_smoke_protocol_ok, Some(1));
    assert_eq!(last_smoke_upstream_ok, Some(1));
    assert_eq!(last_exit_ip.as_deref(), Some("203.0.113.8"));
    assert_eq!(last_anonymity_level.as_deref(), Some("elite"));
}


#[tokio::test]
async fn proxy_smoke_test_classifies_transparent_proxy_response() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf)).await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established
X-Forwarded-For: 198.51.100.7

ip=198.51.100.20
"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-smoke-transparent",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "local",
        "country": "ZZ",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (smoke_status, smoke_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/proxy-smoke-transparent/smoke").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(smoke_status, StatusCode::OK);
    assert_eq!(smoke_json.get("anonymity_level").and_then(|v| v.as_str()), Some("transparent"));
    assert_eq!(smoke_json.get("exit_ip").and_then(|v| v.as_str()), Some("198.51.100.20"));
}


#[tokio::test]
async fn verify_proxy_reports_geo_match_and_country_region() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf)).await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established

ip=198.51.100.10
country=US
region=Virginia
"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-verify-us",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "us-east",
        "country": "US",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (verify_status, verify_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/proxy-verify-us/verify").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(verify_status, StatusCode::OK);
    assert_eq!(verify_json.get("reachable").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(verify_json.get("protocol_ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(verify_json.get("upstream_ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(verify_json.get("exit_ip").and_then(|v| v.as_str()), Some("198.51.100.10"));
    assert_eq!(verify_json.get("exit_country").and_then(|v| v.as_str()), Some("US"));
    assert_eq!(verify_json.get("exit_region").and_then(|v| v.as_str()), Some("Virginia"));
    assert_eq!(verify_json.get("geo_match_ok").and_then(|v| v.as_bool()), Some(true));

    let (_, proxy_json) = json_response(
        &app,
        Request::builder().uri("/proxies/proxy-verify-us").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(proxy_json.get("last_verify_status").and_then(|v| v.as_str()), Some("ok"));
    assert_eq!(proxy_json.get("last_verify_geo_match_ok").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(proxy_json.get("last_exit_ip").and_then(|v| v.as_str()), Some("198.51.100.10"));
    assert_eq!(proxy_json.get("last_exit_country").and_then(|v| v.as_str()), Some("US"));
    assert_eq!(proxy_json.get("last_exit_region").and_then(|v| v.as_str()), Some("Virginia"));
    assert_eq!(proxy_json.get("last_anonymity_level").and_then(|v| v.as_str()), Some("elite"));
}


#[tokio::test]
async fn proxy_selection_prefers_verified_proxy_health_signals() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let verified = serde_json::json!({
        "id": "proxy-verified-best",
        "scheme": "http",
        "host": "127.0.0.1",
        "port": 9001,
        "provider": "pool-a",
        "region": "us-east",
        "country": "US",
        "score": 0.8
    });
    let plain = serde_json::json!({
        "id": "proxy-plain-worse",
        "scheme": "http",
        "host": "127.0.0.1",
        "port": 9002,
        "provider": "pool-a",
        "region": "us-east",
        "country": "US",
        "score": 0.95
    });
    for payload in [verified, plain] {
        let (status, _) = json_response(
            &app,
            Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request"),
        ).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let db = init_db(&db_url).await.expect("init db again");
    sqlx::query(r#"UPDATE proxies SET last_verify_status = 'ok', last_verify_geo_match_ok = 1, last_smoke_upstream_ok = 1, last_exit_country = 'US', last_exit_region = 'Virginia' WHERE id = 'proxy-verified-best'"#)
        .execute(&db)
        .await
        .expect("mark verified proxy");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-a", "region": "us-east"}
    });
    let (_, task_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request"),
    ).await;
    let task_id = task_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("proxy_id").and_then(|v| v.as_str()), Some("proxy-verified-best"));
}


#[tokio::test]
async fn verify_proxy_task_kind_executes_and_persists_result() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf)).await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established

ip=203.0.113.9
country=US
region=Oregon
"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-task-verify",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "us-west",
        "country": "US",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(create_status, StatusCode::CREATED);

    let task_payload = serde_json::json!({
        "kind": "verify_proxy",
        "proxy_id": "proxy-task-verify",
        "timeout_seconds": 5
    });
    let (_, task_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(task_payload.to_string())).expect("request"),
    ).await;
    let task_id = task_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));

    let (_, proxy_json) = json_response(
        &app,
        Request::builder().uri("/proxies/proxy-task-verify").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(proxy_json.get("last_verify_status").and_then(|v| v.as_str()), Some("ok"));
    assert_eq!(proxy_json.get("last_exit_region").and_then(|v| v.as_str()), Some("Oregon"));
}


#[tokio::test]
async fn verify_batch_enqueues_verify_proxy_tasks() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    for (id, provider, region, score) in [
        ("proxy-batch-1", "pool-a", "us-east", 0.9),
        ("proxy-batch-2", "pool-a", "us-east", 0.8),
        ("proxy-batch-3", "pool-b", "eu-west", 0.95),
    ] {
        let proxy_payload = serde_json::json!({
            "id": id,
            "scheme": "http",
            "host": "127.0.0.1",
            "port": 8000,
            "region": region,
            "country": "US",
            "provider": provider,
            "score": score
        });
        let (status, _) = json_response(
            &app,
            Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
        ).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let batch_payload = serde_json::json!({
        "provider": "pool-a",
        "region": "us-east",
        "limit": 10,
        "only_stale": true,
        "min_score": 0.5,
        "stale_after_seconds": 7200,
        "task_timeout_seconds": 9,
        "recently_used_within_seconds": 0,
        "failed_only": false
    });
    let (batch_status, batch_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/verify-batch").header("content-type", "application/json").body(Body::from(batch_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert!(batch_json.get("batch_id").and_then(|v| v.as_str()).unwrap_or_default().starts_with("verify-batch-"));
    assert!(batch_json.get("created_at").and_then(|v| v.as_str()).is_some());
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(batch_json.get("stale_after_seconds").and_then(|v| v.as_i64()), Some(7200));
    assert_eq!(batch_json.get("task_timeout_seconds").and_then(|v| v.as_i64()), Some(9));
    assert_eq!(batch_json.get("provider_summary").and_then(|v| v.as_array()).map(|v| v.len()), Some(1));

    let batch_id = batch_json.get("batch_id").and_then(|v| v.as_str()).expect("batch id");
    let mut queued_verify_tasks = 0_i64;
    for _ in 0..8 {
        queued_verify_tasks = sqlx::query_scalar(r#"SELECT COUNT(*) FROM tasks WHERE kind = 'verify_proxy' AND status = 'queued' AND json_extract(input_json, '$.verify_batch_id') = ?"#)
            .bind(batch_id)
            .fetch_one(&state.db)
            .await
            .expect("count verify tasks for batch");
        if queued_verify_tasks == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(queued_verify_tasks, 2);
    let queued_timeout: i64 = sqlx::query_scalar(r#"SELECT json_extract(input_json, '$.timeout_seconds') FROM tasks WHERE kind = 'verify_proxy' AND json_extract(input_json, '$.verify_batch_id') = ? ORDER BY id LIMIT 1"#)
        .bind(batch_id)
        .fetch_one(&state.db)
        .await
        .expect("load timeout seconds");
    assert_eq!(queued_timeout, 9);
    let queued_batch_id: String = sqlx::query_scalar(r#"SELECT json_extract(input_json, '$.verify_batch_id') FROM tasks WHERE kind = 'verify_proxy' AND json_extract(input_json, '$.verify_batch_id') = ? ORDER BY id LIMIT 1"#)
        .bind(batch_id)
        .fetch_one(&state.db)
        .await
        .expect("load verify batch id");
    assert_eq!(queued_batch_id, batch_id);
}


#[tokio::test]
async fn status_exposes_verify_metrics_summary() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    let db = state.db.clone();

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-v-ok', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.9, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '10', '1', '1'),
                  ('proxy-v-failed', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.8, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'failed', 0, 'US', 'Virginia', '11', '1', '1'),
                  ('proxy-v-missing', 'http', '127.0.0.3', 8082, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.7, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let (_, status_json) = json_response(
        &app,
        Request::builder().uri("/status?limit=10&offset=0").body(Body::empty()).expect("request"),
    ).await;

    let metrics = status_json.get("verify_metrics").expect("verify metrics");
    assert_eq!(metrics.get("verified_ok").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("verified_failed").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("geo_match_ok").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("stale_or_missing_verify").and_then(|v| v.as_i64()), Some(1));
}


#[tokio::test]
async fn verify_batch_skips_recently_verified_proxy_when_only_stale() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    let db = state.db.clone();

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES ('proxy-recent-verify', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.9, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert recent proxy");

    let batch_payload = serde_json::json!({
        "provider": "pool-a",
        "region": "us-east",
        "limit": 10,
        "only_stale": true,
        "stale_after_seconds": 7200
    });
    let (batch_status, batch_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/verify-batch").header("content-type", "application/json").body(Body::from(batch_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(0));
}


#[tokio::test]
async fn verify_batch_can_focus_recent_failed_proxies() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db.clone(), runner, None, 1);
    let app = build_router(state.clone());

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-recent-failed', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.9, 0, 0, NULL, '9999999999', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'failed', 0, 'US', 'Virginia', '10', '1', '1'),
                  ('proxy-old-failed', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.8, 0, 0, NULL, '10', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'failed', 0, 'US', 'Virginia', '10', '1', '1'),
                  ('proxy-recent-ok', 'http', '127.0.0.3', 8082, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.95, 0, 0, NULL, '9999999999', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let batch_payload = serde_json::json!({
        "provider": "pool-a",
        "region": "us-east",
        "limit": 10,
        "only_stale": false,
        "recently_used_within_seconds": 3600,
        "failed_only": true
    });
    let (batch_status, batch_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/verify-batch").header("content-type", "application/json").body(Body::from(batch_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(1));

    let proxy_id: String = sqlx::query_scalar(r#"SELECT json_extract(input_json, '$.proxy_id') FROM tasks WHERE kind = 'verify_proxy' ORDER BY id DESC LIMIT 1"#)
        .fetch_one(&db)
        .await
        .expect("load queued proxy id");
    assert_eq!(proxy_id, "proxy-recent-failed");
}


#[tokio::test]
async fn verify_batch_respects_max_per_provider_cap() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    for (id, provider) in [
        ("proxy-cap-a1", "pool-a"),
        ("proxy-cap-a2", "pool-a"),
        ("proxy-cap-b1", "pool-b"),
        ("proxy-cap-b2", "pool-b"),
    ] {
        let proxy_payload = serde_json::json!({
            "id": id,
            "scheme": "http",
            "host": "127.0.0.1",
            "port": 8000,
            "region": "shared",
            "country": "US",
            "provider": provider,
            "score": 0.9
        });
        let (status, _) = json_response(
            &app,
            Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
        ).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let batch_payload = serde_json::json!({
        "region": "shared",
        "limit": 10,
        "only_stale": true,
        "max_per_provider": 1
    });
    let (batch_status, batch_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/verify-batch").header("content-type", "application/json").body(Body::from(batch_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(2));
    let summary = batch_json.get("provider_summary").and_then(|v| v.as_array()).expect("provider summary");
    assert_eq!(summary.len(), 2);
    assert!(summary.iter().any(|item| item.get("provider").and_then(|v| v.as_str()) == Some("pool-a") && item.get("accepted").and_then(|v| v.as_i64()) == Some(1) && item.get("skipped_due_to_cap").and_then(|v| v.as_i64()) == Some(1)));
    assert!(summary.iter().any(|item| item.get("provider").and_then(|v| v.as_str()) == Some("pool-b") && item.get("accepted").and_then(|v| v.as_i64()) == Some(1) && item.get("skipped_due_to_cap").and_then(|v| v.as_i64()) == Some(1)));

    let scheduled: Vec<(String,)> = sqlx::query_as(r#"SELECT json_extract(input_json, '$.proxy_id') FROM tasks WHERE kind = 'verify_proxy' ORDER BY id ASC"#)
        .fetch_all(&state.db)
        .await
        .expect("load queued proxy ids");
    assert_eq!(scheduled.len(), 2);
    assert_ne!(scheduled[0].0, scheduled[1].0);
}


#[tokio::test]
async fn verify_batch_is_persisted_and_queryable() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    for id in ["proxy-batch-persist-1", "proxy-batch-persist-2"] {
        let proxy_payload = serde_json::json!({
            "id": id,
            "scheme": "http",
            "host": "127.0.0.1",
            "port": 8000,
            "region": "persist",
            "country": "US",
            "provider": "pool-persist",
            "score": 0.9
        });
        let (status, _) = json_response(
            &app,
            Request::builder().method("POST").uri("/proxies").header("content-type", "application/json").body(Body::from(proxy_payload.to_string())).expect("request"),
        ).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let batch_payload = serde_json::json!({
        "provider": "pool-persist",
        "region": "persist",
        "limit": 10,
        "only_stale": true,
        "max_per_provider": 2
    });
    let (status, batch_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/verify-batch").header("content-type", "application/json").body(Body::from(batch_payload.to_string())).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let batch_id = batch_json.get("batch_id").and_then(|v| v.as_str()).expect("batch id").to_string();

    let (_, list_json) = json_response(
        &app,
        Request::builder().uri("/proxies/verify-batch?limit=10&offset=0").body(Body::empty()).expect("request"),
    ).await;
    let items = list_json.as_array().expect("verify batch list");
    assert!(!items.is_empty());
    assert!(items.iter().any(|item| item.get("id").and_then(|v| v.as_str()) == Some(batch_id.as_str())));

    let (_, detail_json) = json_response(
        &app,
        Request::builder().uri(format!("/proxies/verify-batch/{batch_id}")).body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(detail_json.get("id").and_then(|v| v.as_str()), Some(batch_id.as_str()));
    assert_eq!(detail_json.get("accepted_count").and_then(|v| v.as_i64()), Some(2));
    let queued = detail_json.get("queued_count").and_then(|v| v.as_i64()).unwrap_or_default();
    let running = detail_json.get("running_count").and_then(|v| v.as_i64()).unwrap_or_default();
    let succeeded = detail_json.get("succeeded_count").and_then(|v| v.as_i64()).unwrap_or_default();
    let failed = detail_json.get("failed_count").and_then(|v| v.as_i64()).unwrap_or_default();
    assert_eq!(queued + running + succeeded + failed, 2);
    assert!(matches!(detail_json.get("status").and_then(|v| v.as_str()), Some("running") | Some("completed") | Some("scheduled")));
    assert!(detail_json.get("provider_summary_json").is_some());
    assert!(detail_json.get("filters_json").is_some());
}


#[tokio::test]
async fn proxy_selection_prefers_fresh_verified_proxy_over_stale_high_score_proxy() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let _state = build_app_state(db.clone(), runner, None, 1);

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-fresh-verified', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.85, 0, 0, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-stale-high-score', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99, 0, 0, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '1', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE
               WHEN last_verify_status = 'failed' THEN 3
               WHEN last_verify_at IS NULL THEN 2
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
               ELSE 0
             END ASC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Some("pool-a".to_string()))
    .bind(Some("pool-a".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");
    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-fresh-verified"));
}

#[tokio::test]
async fn proxy_selection_penalizes_recent_verify_failures_even_with_higher_score() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let _state = build_app_state(db.clone(), runner, None, 1);

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-ok-lower-score', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.70, 0, 0, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-failed-higher-score', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99, 0, 0, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'failed', 0, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE
               WHEN last_verify_status = 'failed' THEN 3
               WHEN last_verify_at IS NULL THEN 2
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
               ELSE 0
             END ASC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Some("pool-a".to_string()))
    .bind(Some("pool-a".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");
    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-ok-lower-score"));
}


#[tokio::test]
async fn proxy_selection_prefers_geo_match_verified_proxy_over_smoke_only_proxy() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-geo-match', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.80, 0, 0, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-smoke-only', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.95, 0, 0, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, NULL, 0, NULL, NULL, NULL, '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE
               WHEN last_verify_status = 'failed' THEN 3
               WHEN last_verify_at IS NULL THEN 2
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
               ELSE 0
             END ASC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Some("pool-a".to_string()))
    .bind(Some("pool-a".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");
    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-geo-match"));
}

#[tokio::test]
async fn proxy_selection_prefers_fresh_verified_proxy_over_missing_verify_proxy() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-fresh-verified-2', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.70, 0, 0, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-missing-verify', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99, 0, 0, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE
               WHEN last_verify_status = 'failed' THEN 3
               WHEN last_verify_at IS NULL THEN 2
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
               ELSE 0
             END ASC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Some("pool-a".to_string()))
    .bind(Some("pool-a".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");
    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-fresh-verified-2"));
}


#[tokio::test]
async fn proxy_selection_penalizes_bad_long_term_history_even_with_fresh_verify() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-good-history', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.70, 8, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-bad-history', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.95, 1, 6, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE
               WHEN last_verify_status = 'failed' THEN 3
               WHEN last_verify_at IS NULL THEN 2
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN failure_count >= success_count + 3 THEN 2
               WHEN failure_count > success_count THEN 1
               ELSE 0
             END ASC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Some("pool-a".to_string()))
    .bind(Some("pool-a".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");
    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-good-history"));
}


#[tokio::test]
async fn proxy_selection_penalizes_bad_provider_history_even_with_better_score() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-good-provider', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-good', 'active', 0.70, 8, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-bad-provider-a', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-bad', 'active', 0.99, 1, 6, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-bad-provider-b', 'http', '127.0.0.3', 8082, NULL, NULL, 'us-east', 'US', 'pool-bad', 'active', 0.60, 0, 4, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE
               WHEN last_verify_status = 'failed' THEN 3
               WHEN last_verify_at IS NULL THEN 2
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN provider IS NOT NULL AND provider IN (
                   SELECT provider FROM proxies WHERE provider IS NOT NULL GROUP BY provider HAVING SUM(failure_count) >= SUM(success_count) + 5
               ) THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN failure_count >= success_count + 3 THEN 2
               WHEN failure_count > success_count THEN 1
               ELSE 0
             END ASC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");
    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-good-provider"));
}


#[tokio::test]
async fn proxy_selection_penalizes_more_recent_failure_more_than_older_failure() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-older-failure', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.70, 5, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'failed', 1, 'US', 'Virginia', '9999990000', '1', '1'),
                  ('proxy-more-recent-failure', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.95, 5, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'failed', 1, 'US', 'Virginia', '9999999000', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE
               WHEN last_verify_status = 'failed' THEN 3
               WHEN last_verify_at IS NULL THEN 2
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 1800 THEN 2
               WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 7200 THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN provider IS NOT NULL AND provider IN (
                   SELECT provider FROM proxies WHERE provider IS NOT NULL GROUP BY provider HAVING SUM(failure_count) >= SUM(success_count) + 5
               ) THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN failure_count >= success_count + 3 THEN 2
               WHEN failure_count > success_count THEN 1
               ELSE 0
             END ASC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Some("pool-a".to_string()))
    .bind(Some("pool-a".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .bind("9999999999")
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");
    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-older-failure"));
}


#[tokio::test]
async fn proxy_selection_penalizes_recent_provider_region_failure_cluster() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-stable-region', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-good', 'active', 0.70, 8, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-cluster-fail-a', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-bad', 'active', 0.99, 5, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'failed', 1, 'US', 'Virginia', '9999999000', '1', '1'),
                  ('proxy-cluster-fail-b', 'http', '127.0.0.3', 8082, NULL, NULL, 'us-east', 'US', 'pool-bad', 'active', 0.98, 5, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'failed', 1, 'US', 'Virginia', '9999999100', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
             CASE
               WHEN last_verify_status = 'failed' THEN 3
               WHEN last_verify_at IS NULL THEN 2
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN provider IS NOT NULL AND region IS NOT NULL AND (provider, region) IN (
                   SELECT provider, region FROM proxies
                   WHERE provider IS NOT NULL AND region IS NOT NULL AND last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600
                   GROUP BY provider, region
                   HAVING COUNT(*) >= 2
               ) THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 1800 THEN 2
               WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 7200 THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN provider IS NOT NULL AND provider IN (
                   SELECT provider FROM proxies WHERE provider IS NOT NULL GROUP BY provider HAVING SUM(failure_count) >= SUM(success_count) + 5
               ) THEN 1
               ELSE 0
             END ASC,
             CASE
               WHEN failure_count >= success_count + 3 THEN 2
               WHEN failure_count > success_count THEN 1
               ELSE 0
             END ASC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Option::<String>::None)
    .bind(Option::<String>::None)
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .bind("9999999999")
    .bind("9999999999")
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");
    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-stable-region"));
}


#[tokio::test]
async fn proxy_trust_score_penalizes_missing_verify_even_against_much_higher_raw_score() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-verified-balanced-direct', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.55, 6, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-missing-verify-max-score-direct', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99, 6, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             ((CASE WHEN last_verify_status = 'ok' THEN 30 ELSE 0 END) +
              (CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 20 ELSE 0 END) +
              (CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 10 ELSE 0 END) -
              (CASE WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 1800 THEN 30
                    WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 7200 THEN 15
                    WHEN last_verify_status = 'failed' THEN 10
                    ELSE 0 END) -
              (CASE WHEN last_verify_at IS NULL THEN 12
                    WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 8
                    ELSE 0 END) -
              (CASE WHEN failure_count >= success_count + 3 THEN 18
                    WHEN failure_count > success_count THEN 8
                    ELSE 0 END) -
              (CASE WHEN provider IS NOT NULL AND provider IN (
                         SELECT provider FROM proxies WHERE provider IS NOT NULL GROUP BY provider HAVING SUM(failure_count) >= SUM(success_count) + 5
                    ) THEN 10 ELSE 0 END) -
              (CASE WHEN provider IS NOT NULL AND region IS NOT NULL AND (provider, region) IN (
                         SELECT provider, region FROM proxies
                         WHERE provider IS NOT NULL AND region IS NOT NULL AND last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600
                         GROUP BY provider, region HAVING COUNT(*) >= 2
                    ) THEN 12 ELSE 0 END) +
              CAST(score * 10 AS INTEGER)) DESC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Some("pool-a".to_string()))
    .bind(Some("pool-a".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .bind("9999999999")
    .bind("9999999999")
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");

    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-verified-balanced-direct"));
}


#[tokio::test]
async fn proxy_trust_score_prefers_healthier_proxy_in_direct_ordering() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-high-trust-direct', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.70, 8, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-higher-raw-score-direct', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99, 0, 20, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'failed', 0, 'US', 'Virginia', '9999999000', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    let selected: Option<(String,)> = sqlx::query_as(
        r#"SELECT id FROM proxies
           WHERE status = 'active'
             AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
           ORDER BY
             ((CASE WHEN last_verify_status = 'ok' THEN 30 ELSE 0 END) +
              (CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 20 ELSE 0 END) +
              (CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 10 ELSE 0 END) -
              (CASE WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 1800 THEN 30
                    WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 7200 THEN 15
                    WHEN last_verify_status = 'failed' THEN 10
                    ELSE 0 END) -
              (CASE WHEN last_verify_at IS NULL THEN 12
                    WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 8
                    ELSE 0 END) -
              (CASE WHEN failure_count >= success_count + 3 THEN 18
                    WHEN failure_count > success_count THEN 8
                    ELSE 0 END) -
              (CASE WHEN provider IS NOT NULL AND provider IN (
                         SELECT provider FROM proxies WHERE provider IS NOT NULL GROUP BY provider HAVING SUM(failure_count) >= SUM(success_count) + 5
                    ) THEN 10 ELSE 0 END) -
              (CASE WHEN provider IS NOT NULL AND region IS NOT NULL AND (provider, region) IN (
                         SELECT provider, region FROM proxies
                         WHERE provider IS NOT NULL AND region IS NOT NULL AND last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600
                         GROUP BY provider, region HAVING COUNT(*) >= 2
                    ) THEN 12 ELSE 0 END)) DESC,
             score DESC,
             COALESCE(last_used_at, '0') ASC,
             created_at ASC
           LIMIT 1"#,
    )
    .bind("9999999999")
    .bind(Some("pool-a".to_string()))
    .bind(Some("pool-a".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(Some("us-east".to_string()))
    .bind(0.0_f64)
    .bind("9999999999")
    .bind("9999999999")
    .bind("9999999999")
    .bind("9999999999")
    .fetch_optional(&db)
    .await
    .expect("select proxy");

    assert_eq!(selected.as_ref().map(|row| row.0.as_str()), Some("proxy-high-trust-direct"));
}


#[tokio::test]
async fn auto_selection_result_exposes_trust_score_components_and_candidate_preview() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-explain-best', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-x', 'active', 0.70, 8, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-explain-second', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-x', 'active', 0.65, 4, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 0, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/explain",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-x", "region": "us-east"}
    });
    let (_, create_json) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request")).await;
    let task_id = create_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task_json.get("proxy_id").and_then(|v| v.as_str()), Some("proxy-explain-best"));
    assert!(task_json.get("selection_reason_summary").and_then(|v| v.as_str()).unwrap_or("").contains("trust score"));
    assert!(task_json.get("trust_score_total").and_then(|v| v.as_i64()).is_some());
    assert!(task_json.get("winner_vs_runner_up_diff").is_some());
    assert!(task_json.get("winner_vs_runner_up_diff").and_then(|v| v.get("winner_total_score")).and_then(|v| v.as_i64()).is_some());
    assert!(task_json.get("winner_vs_runner_up_diff").and_then(|v| v.get("runner_up_total_score")).and_then(|v| v.as_i64()).is_some());
    assert!(task_json.get("winner_vs_runner_up_diff").and_then(|v| v.get("score_gap")).and_then(|v| v.as_i64()).is_some());
    assert!(task_json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()).map(|v| !v.is_empty()).unwrap_or(false));
    assert!(task_json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()).map(|v| v.len() <= 5).unwrap_or(false));
    assert!(task_json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()).and_then(|arr| arr.first()).and_then(|v| v.get("label")).and_then(|v| v.as_str()).map(|v| !v.is_empty()).unwrap_or(false));
    assert!(task_json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()).and_then(|arr| arr.first()).and_then(|v| v.get("direction")).and_then(|v| v.as_str()).map(|v| matches!(v, "winner" | "runner_up" | "neutral")).unwrap_or(false));
    if let Some(diff) = task_json.get("winner_vs_runner_up_diff") {
        let winner_total = diff.get("winner_total_score").and_then(|v| v.as_i64()).unwrap_or_default();
        let runner_total = diff.get("runner_up_total_score").and_then(|v| v.as_i64()).unwrap_or_default();
        let score_gap = diff.get("score_gap").and_then(|v| v.as_i64()).unwrap_or_default();
        assert_eq!(winner_total - runner_total, score_gap);
    }
    if let Some(factors) = task_json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()) {
        let deltas: Vec<i64> = factors.iter().filter_map(|v| v.get("delta").and_then(|v| v.as_i64()).map(|d| d.abs())).collect();
        assert!(deltas.windows(2).all(|w| w[0] >= w[1]));
        let labels: Vec<&str> = factors.iter().filter_map(|v| v.get("label").and_then(|v| v.as_str())).collect();
        assert!(labels.iter().all(|label| matches!(*label, "verify_ok" | "geo_match" | "upstream_ok" | "raw_score" | "missing_verify" | "stale_verify" | "verify_failed_heavy" | "verify_failed_light" | "verify_failed_base" | "history_risk" | "provider_risk" | "provider_region_risk")));
    }

    let result_json_text: Option<String> = sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_one(&state.db)
        .await
        .expect("load result json");
    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse result json");
    let policy = result_json.get("payload").and_then(|v| v.get("network_policy_json")).expect("policy");
    assert!(policy.get("trust_score_components").and_then(|v| v.get("verify_ok_bonus")).and_then(|v| v.as_i64()).is_some());
    let preview = policy.get("candidate_rank_preview").and_then(|v| v.as_array()).expect("candidate preview");
    assert!(!preview.is_empty());
    assert_eq!(preview[0].get("id").and_then(|v| v.as_str()), Some("proxy-explain-best"));
    let summary = preview[0].get("summary").and_then(|v| v.as_str()).unwrap_or("");
    assert!(!summary.is_empty());
    assert!(summary.contains("wins on") || summary.contains("penalized by") || summary.contains("better on") || summary.contains("worse on"));
    assert!(!summary.contains("verify_ok_bonus"));
    assert!(!summary.contains("provider_region_cluster_penalty"));
}

#[tokio::test]
async fn verify_migration_columns_are_added_for_old_proxy_table() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db first");
    drop(db);

    let path = db_url.strip_prefix("sqlite://").expect("sqlite path");
    let old_db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect(&db_url).await.expect("connect old db");
    sqlx::query("DROP TABLE proxies").execute(&old_db).await.expect("drop proxies");
    sqlx::query(r#"CREATE TABLE proxies (
        id TEXT PRIMARY KEY,
        scheme TEXT NOT NULL,
        host TEXT NOT NULL,
        port INTEGER NOT NULL,
        username TEXT,
        password TEXT,
        region TEXT,
        country TEXT,
        provider TEXT,
        status TEXT NOT NULL DEFAULT 'active',
        score REAL NOT NULL DEFAULT 1.0,
        success_count INTEGER NOT NULL DEFAULT 0,
        failure_count INTEGER NOT NULL DEFAULT 0,
        last_checked_at TEXT,
        last_used_at TEXT,
        cooldown_until TEXT,
        last_smoke_status TEXT,
        last_smoke_protocol_ok INTEGER,
        last_smoke_upstream_ok INTEGER,
        last_exit_ip TEXT,
        last_anonymity_level TEXT,
        last_smoke_at TEXT,
        last_verify_status TEXT,
        last_verify_geo_match_ok INTEGER,
        last_exit_country TEXT,
        last_exit_region TEXT,
        last_verify_at TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )"#).execute(&old_db).await.expect("create old proxies");
    drop(old_db);

    let db2 = init_db(&format!("sqlite://{path}")).await.expect("re-init db");
    let cols: Vec<(i64, String, String, i64, Option<String>, i64)> = sqlx::query_as("PRAGMA table_info(proxies)").fetch_all(&db2).await.expect("pragma table info");
    let names: Vec<String> = cols.into_iter().map(|row| row.1).collect();
    assert!(names.contains(&"last_probe_latency_ms".to_string()));
    assert!(names.contains(&"last_probe_error".to_string()));
    assert!(names.contains(&"last_probe_error_category".to_string()));
    assert!(names.contains(&"last_verify_confidence".to_string()));
    assert!(names.contains(&"last_verify_score_delta".to_string()));
    assert!(names.contains(&"last_verify_source".to_string()));
}

#[tokio::test]
async fn execution_feedback_updates_proxy_score() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at)
                  VALUES ('proxy-feedback-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-f', 'active', 0.50, 0, 0, NULL, NULL, NULL, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let ok_payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/ok",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-feedback-1"}
    });
    let (_, ok_json) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(ok_payload.to_string())).expect("request")).await;
    let ok_id = ok_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let _ = wait_for_terminal_status(&app, &ok_id).await;
    let score_after_success: f64 = sqlx::query_scalar("SELECT score FROM proxies WHERE id = 'proxy-feedback-1'").fetch_one(&state.db).await.expect("score after success");
    assert!(score_after_success > 0.50);
}



#[tokio::test]
async fn proxy_explain_endpoint_single_candidate_has_zero_gap_and_empty_runner_up_score() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES ('proxy-explain-single', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-single', 'active', 0.77, 5, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let (status, json) = json_response(
        &app,
        Request::builder().uri("/proxies/proxy-explain-single/explain").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::OK);
    let diff = json.get("winner_vs_runner_up_diff").expect("winner diff");
    assert_eq!(diff.get("runner_up_total_score").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(diff.get("score_gap").and_then(|v| v.as_i64()), diff.get("winner_total_score").and_then(|v| v.as_i64()));
    assert!(diff.get("factors").and_then(|v| v.as_array()).map(|v| v.len() <= 5).unwrap_or(false));
}

#[tokio::test]
async fn proxy_explain_endpoint_returns_components_and_preview() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES ('proxy-explain-endpoint', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-e', 'active', 0.77, 5, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let (status, json) = json_response(
        &app,
        Request::builder().uri("/proxies/proxy-explain-endpoint/explain").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("proxy_id").and_then(|v| v.as_str()), Some("proxy-explain-endpoint"));
    assert!(json.get("trust_score_components").and_then(|v| v.get("verify_ok_bonus")).and_then(|v| v.as_i64()).is_some());
    assert!(json.get("candidate_rank_preview").and_then(|v| v.as_array()).map(|v| !v.is_empty()).unwrap_or(false));
    assert!(json.get("winner_vs_runner_up_diff").is_some());
    assert!(json.get("winner_vs_runner_up_diff").and_then(|v| v.get("winner_total_score")).and_then(|v| v.as_i64()).is_some());
    assert!(json.get("winner_vs_runner_up_diff").and_then(|v| v.get("runner_up_total_score")).and_then(|v| v.as_i64()).is_some());
    assert!(json.get("winner_vs_runner_up_diff").and_then(|v| v.get("score_gap")).and_then(|v| v.as_i64()).is_some());
    assert!(json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()).map(|v| !v.is_empty()).unwrap_or(false));
    assert!(json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()).map(|v| v.len() <= 5).unwrap_or(false));
    assert!(json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()).and_then(|arr| arr.first()).and_then(|v| v.get("label")).and_then(|v| v.as_str()).map(|v| !v.is_empty()).unwrap_or(false));
    assert!(json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()).and_then(|arr| arr.first()).and_then(|v| v.get("direction")).and_then(|v| v.as_str()).map(|v| matches!(v, "winner" | "runner_up" | "neutral")).unwrap_or(false));
    if let Some(diff) = json.get("winner_vs_runner_up_diff") {
        let winner_total = diff.get("winner_total_score").and_then(|v| v.as_i64()).unwrap_or_default();
        let runner_total = diff.get("runner_up_total_score").and_then(|v| v.as_i64()).unwrap_or_default();
        let score_gap = diff.get("score_gap").and_then(|v| v.as_i64()).unwrap_or_default();
        assert_eq!(winner_total - runner_total, score_gap);
    }
    if let Some(factors) = json.get("winner_vs_runner_up_diff").and_then(|v| v.get("factors")).and_then(|v| v.as_array()) {
        let deltas: Vec<i64> = factors.iter().filter_map(|v| v.get("delta").and_then(|v| v.as_i64()).map(|d| d.abs())).collect();
        assert!(deltas.windows(2).all(|w| w[0] >= w[1]));
        let labels: Vec<&str> = factors.iter().filter_map(|v| v.get("label").and_then(|v| v.as_str())).collect();
        assert!(labels.iter().all(|label| matches!(*label, "verify_ok" | "geo_match" | "upstream_ok" | "raw_score" | "missing_verify" | "stale_verify" | "verify_failed_heavy" | "verify_failed_light" | "verify_failed_base" | "history_risk" | "provider_risk" | "provider_region_risk")));
    }
}

#[tokio::test]
async fn verify_probe_updates_proxy_score_via_score_delta() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 1024];
            let _ = socket.read(&mut buf).await;
            let response = b"HTTP/1.1 200 Connection Established
ip=203.0.113.8
country=US
region=Virginia

";
            let _ = socket.write_all(response).await;
        }
    });

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at)
                  VALUES (?, 'http', ?, ?, NULL, NULL, 'us-east', 'US', 'pool-v', 'active', 0.50, 0, 0, NULL, NULL, NULL, '1', '1')"#)
        .bind("proxy-verify-score")
        .bind(addr.ip().to_string())
        .bind(i64::from(addr.port()))
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let before: f64 = sqlx::query_scalar("SELECT score FROM proxies WHERE id = 'proxy-verify-score'").fetch_one(&state.db).await.expect("before score");
    let (status, json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/proxy-verify-score/verify").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("ok"));
    let after: f64 = sqlx::query_scalar("SELECT score FROM proxies WHERE id = 'proxy-verify-score'").fetch_one(&state.db).await.expect("after score");
    assert!(after > before);
}


#[tokio::test]
async fn provider_risk_snapshots_are_materialized_on_init() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, created_at, updated_at)
                  VALUES
                  ('proxy-risk-a', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-risk', 'active', 0.5, 1, 10, '1', '1'),
                  ('proxy-risk-b', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-risk', 'active', 0.5, 1, 10, '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&db).await.expect("refresh snapshots");
    let risk_hit: i64 = sqlx::query_scalar("SELECT risk_hit FROM provider_risk_snapshots WHERE provider = 'pool-risk'")
        .fetch_one(&db)
        .await
        .expect("provider risk snapshot");
    assert_eq!(risk_hit, 1);
}

#[tokio::test]
async fn provider_region_risk_snapshots_are_materialized_on_refresh() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_verify_status, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-pr-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-pr', 'active', 0.5, 0, 5, 'failed', '9999999999', '1', '1'),
                  ('proxy-pr-2', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-pr', 'active', 0.5, 0, 5, 'failed', '9999999999', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&db).await.expect("refresh snapshots");
    let risk_hit: i64 = sqlx::query_scalar("SELECT risk_hit FROM provider_region_risk_snapshots WHERE provider = 'pool-pr' AND region = 'us-east'")
        .fetch_one(&db)
        .await
        .expect("provider region risk snapshot");
    assert_eq!(risk_hit, 1);
}


#[tokio::test]
async fn targeted_provider_snapshot_refresh_updates_only_requested_provider() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, created_at, updated_at)
                  VALUES
                  ('proxy-target-a1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.5, 1, 10, '1', '1'),
                  ('proxy-target-b1', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-west', 'US', 'pool-b', 'active', 0.5, 10, 1, '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&db).await.expect("refresh all");
    sqlx::query("UPDATE proxies SET failure_count = 20 WHERE provider = 'pool-b'").execute(&db).await.expect("update pool-b");
    AutoOpenBrowser::db::init::refresh_provider_risk_snapshot_for_provider(&db, Some("pool-b")).await.expect("refresh pool-b only");

    let pool_a: i64 = sqlx::query_scalar("SELECT risk_hit FROM provider_risk_snapshots WHERE provider = 'pool-a'").fetch_one(&db).await.expect("pool-a");
    let pool_b: i64 = sqlx::query_scalar("SELECT risk_hit FROM provider_risk_snapshots WHERE provider = 'pool-b'").fetch_one(&db).await.expect("pool-b");
    assert_eq!(pool_a, 1);
    assert_eq!(pool_b, 1);
}


#[tokio::test]
async fn trust_score_cache_is_materialized_and_reused() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                  VALUES ('proxy-cache-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-cache', 'active', 0.8, 5, 0, 'ok', 1, 1, '9999999999', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxy");

    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&db).await.expect("refresh risk snapshots");
    AutoOpenBrowser::db::init::refresh_cached_trust_scores(&db).await.expect("refresh trust score cache");
    let cached: i64 = sqlx::query_scalar("SELECT cached_trust_score FROM proxies WHERE id = 'proxy-cache-1'")
        .fetch_one(&db)
        .await
        .expect("cached trust score");
    assert!(cached > 0);
}


#[tokio::test]
async fn auto_selection_can_order_by_cached_trust_score() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-cache-order-low', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-cached', 'active', 0.95, 0, 0, NULL, 0, 0, NULL, '1', '1'),
                  ('proxy-cache-order-high', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-cached', 'active', 0.40, 5, 0, 'ok', 1, 1, '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");
    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&state.db).await.expect("refresh provider risk snapshots");
    AutoOpenBrowser::db::init::refresh_cached_trust_scores(&state.db).await.expect("refresh cached trust scores");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/cached-order",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-cached", "region": "us-east"}
    });
    let (_, create_json) = json_response(&app, Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request")).await;
    let task_id = create_json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task_json.get("proxy_id").and_then(|v| v.as_str()), Some("proxy-cache-order-high"));
}


#[tokio::test]
async fn scoped_cached_trust_score_refresh_updates_provider_group() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-scope-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-scope', 'active', 0.4, 0, 0, NULL, 0, 0, NULL, '1', '1'),
                  ('proxy-scope-2', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-west', 'US', 'pool-scope', 'active', 0.4, 5, 0, 'ok', 1, 1, '9999999999', '1', '1')"#)
        .execute(&db)
        .await
        .expect("insert proxies");

    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&db).await.expect("refresh risk snapshots");
    AutoOpenBrowser::db::init::refresh_cached_trust_scores_for_provider(&db, Some("pool-scope")).await.expect("refresh cached trust by provider");
    let cached_one: i64 = sqlx::query_scalar("SELECT COALESCE(cached_trust_score, 0) FROM proxies WHERE id = 'proxy-scope-1'").fetch_one(&db).await.expect("cache 1");
    let cached_two: i64 = sqlx::query_scalar("SELECT COALESCE(cached_trust_score, 0) FROM proxies WHERE id = 'proxy-scope-2'").fetch_one(&db).await.expect("cache 2");
    assert!(cached_two > cached_one);
}


#[tokio::test]
async fn trust_cache_check_endpoint_reports_sync_status() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                  VALUES ('proxy-cache-check', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-check', 'active', 0.8, 5, 0, 'ok', 1, 1, '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");
    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&state.db).await.expect("refresh risk snapshots");
    AutoOpenBrowser::db::init::refresh_cached_trust_scores(&state.db).await.expect("refresh trust cache");

    let (status, json) = json_response(
        &app,
        Request::builder().uri("/proxies/proxy-cache-check/trust-cache-check").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("proxy_id").and_then(|v| v.as_str()), Some("proxy-cache-check"));
    assert!(json.get("cached_trust_score").and_then(|v| v.as_i64()).is_some());
    assert!(json.get("recomputed_trust_score").and_then(|v| v.as_i64()).is_some());
    assert_eq!(json.get("delta").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(json.get("in_sync").and_then(|v| v.as_bool()), Some(true));
}


#[tokio::test]
async fn trust_cache_repair_endpoint_repairs_drifted_cache() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, cached_trust_score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                  VALUES ('proxy-cache-repair', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-repair', 'active', 0.8, 0, 5, 0, 'ok', 1, 1, '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");
    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&state.db).await.expect("refresh risk snapshots");

    let (status, json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/proxy-cache-repair/trust-cache-repair").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("proxy_id").and_then(|v| v.as_str()), Some("proxy-cache-repair"));
    assert_eq!(json.get("repaired").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(json.get("in_sync").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(json.get("delta").and_then(|v| v.as_i64()), Some(0));
    assert!(json.get("cached_trust_score").and_then(|v| v.as_i64()).unwrap_or(0) > 0);
}


#[tokio::test]
async fn trust_cache_scan_and_batch_repair_endpoints_work() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, cached_trust_score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-batch-cache-a', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-batch-cache', 'active', 0.8, 0, 5, 0, 'ok', 1, 1, '9999999999', '1', '1'),
                  ('proxy-batch-cache-b', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-west', 'US', 'pool-batch-cache', 'active', 0.2, 0, 0, 0, NULL, 0, 0, NULL, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");
    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&state.db).await.expect("refresh risk snapshots");

    let (scan_status, scan_json) = json_response(
        &app,
        Request::builder().uri("/proxies/trust-cache-scan").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(scan_status, StatusCode::OK);
    assert_eq!(scan_json.get("total").and_then(|v| v.as_u64()), Some(2));
    assert!(scan_json.get("drifted").and_then(|v| v.as_u64()).unwrap_or(0) >= 1);

    let (repair_status, repair_json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/trust-cache-repair-batch").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(repair_status, StatusCode::OK);
    assert_eq!(repair_json.get("scanned").and_then(|v| v.as_u64()), Some(2));
    assert!(repair_json.get("repaired").and_then(|v| v.as_u64()).unwrap_or(0) >= 1);
    assert_eq!(repair_json.get("remaining_drifted").and_then(|v| v.as_u64()), Some(0));
}


#[tokio::test]
async fn trust_cache_maintenance_endpoint_repairs_all_drift() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, cached_trust_score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-maint-a', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-maint', 'active', 0.8, 0, 5, 0, 'ok', 1, 1, '9999999999', '1', '1'),
                  ('proxy-maint-b', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-west', 'US', 'pool-maint', 'active', 0.1, 0, 0, 0, NULL, 0, 0, NULL, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");
    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&state.db).await.expect("refresh risk snapshots");

    let (status, json) = json_response(
        &app,
        Request::builder().method("POST").uri("/proxies/trust-cache-maintenance").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("scanned_before").and_then(|v| v.as_u64()), Some(2));
    assert!(json.get("drifted_before").and_then(|v| v.as_u64()).unwrap_or(0) >= 1);
    assert!(json.get("repaired").and_then(|v| v.as_u64()).unwrap_or(0) >= 1);
    assert_eq!(json.get("remaining_drifted").and_then(|v| v.as_u64()), Some(0));
    assert_eq!(json.get("ok").and_then(|v| v.as_bool()), Some(true));
}


#[tokio::test]
async fn trust_cache_scan_supports_limit_and_only_drifted_filters() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, cached_trust_score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                  VALUES
                  ('poolflt-a', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'poolflt', 'active', 0.8, 0, 5, 0, 'ok', 1, 1, '9999999999', '1', '1'),
                  ('poolflt-b', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-west', 'US', 'poolflt', 'active', 0.2, 0, 0, 0, NULL, 0, 0, NULL, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");
    AutoOpenBrowser::db::init::refresh_provider_risk_snapshots(&state.db).await.expect("refresh risk snapshots");

    let (status, json) = json_response(
        &app,
        Request::builder().uri("/proxies/trust-cache-scan?only_drifted=true&limit=1&provider=poolflt").body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("total").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(json.get("drifted").and_then(|v| v.as_u64()), Some(1));
}
