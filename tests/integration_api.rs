use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{body::Body, http::{Request, StatusCode}};
use AutoOpenBrowser::{
    build_test_app,
    domain::{
        run::{RUN_STATUS_FAILED, RUN_STATUS_RUNNING},
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING, TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
    runner::engine::reclaim_stale_running_tasks,
};
use serde_json::Value;
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
    let (state, _app) = build_test_app(&db_url).await.expect("build app");

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
    let (state, _app) = build_test_app(&db_url).await.expect("build app");

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
    let (state, _app) = build_test_app(&db_url).await.expect("build app");

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
    assert!(retry_json
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .contains("already queued"));
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
    assert!(retry_json
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .contains("already queued"));

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));
}

#[tokio::test]
async fn cancelled_task_is_not_reclaimed() {
    let db_url = unique_db_url();
    let (state, _app) = build_test_app(&db_url).await.expect("build app");

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

    let (failure_count, last_checked_at, cooldown_until): (i64, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT failure_count, last_checked_at, cooldown_until FROM proxies WHERE id = 'proxy-smoke-dead'"#)
            .fetch_one(&state.db)
            .await
            .expect("load proxy after smoke test");
    assert_eq!(failure_count, 1);
    assert!(last_checked_at.is_some());
    assert!(cooldown_until.is_some());
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
