use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::{body::Body, http::{Request, StatusCode}};
use AutoOpenBrowser::{
    build_test_app,
    domain::task::{
        TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING, TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
    },
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
