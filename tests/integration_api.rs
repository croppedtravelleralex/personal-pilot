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
    let heartbeat_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

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
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

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
