use std::{
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use persona_pilot::{
    api::{handlers::run_proxy_replenish_mvp_tick, routes::build_router},
    app::build_app_state,
    build_test_app,
    db::init::{init_db, DbPool},
    domain::{
        run::{RUN_STATUS_FAILED, RUN_STATUS_RUNNING},
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING,
            TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
    network_identity::proxy_harvest::set_proxy_runtime_mode_override,
    runner::engine::{reclaim_stale_running_tasks, update_proxy_health_after_execution},
    runner::{fake::FakeRunner, types::RunnerProxySelection, RunnerOutcomeStatus},
};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tower::ServiceExt;
use uuid::Uuid;

struct ScopedEnvVar {
    key: String,
    previous: Option<String>,
    used_proxy_mode_override: bool,
    _proxy_mode_guard: Option<MutexGuard<'static, ()>>,
}

fn proxy_mode_override_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

impl ScopedEnvVar {
    fn set(key: &str, value: &str) -> Self {
        if key == "PERSONA_PILOT_PROXY_MODE" {
            let proxy_mode_guard = Some(
                proxy_mode_override_lock()
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
            );
            let previous = set_proxy_runtime_mode_override(Some(value));
            return Self {
                key: key.to_string(),
                previous,
                used_proxy_mode_override: true,
                _proxy_mode_guard: proxy_mode_guard,
            };
        }
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        Self {
            key: key.to_string(),
            previous,
            used_proxy_mode_override: false,
            _proxy_mode_guard: None,
        }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        if self.used_proxy_mode_override {
            set_proxy_runtime_mode_override(self.previous.as_deref());
            return;
        }
        if let Some(previous) = &self.previous {
            std::env::set_var(&self.key, previous);
        } else {
            std::env::remove_var(&self.key);
        }
    }
}

fn unique_db_url() -> String {
    format!("sqlite:///tmp/persona_pilot_test_{}.db", Uuid::new_v4())
}

async fn json_response(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("request should succeed");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let json = serde_json::from_slice::<Value>(&body).unwrap_or_else(|err| {
        panic!(
            "json body: {err}; status={status}; raw={}",
            String::from_utf8_lossy(&body)
        )
    });
    (status, json)
}

async fn text_response(app: &axum::Router, request: Request<Body>) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("request should succeed");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let text = String::from_utf8(body.to_vec()).expect("utf8 body");
    (status, text)
}

async fn seed_active_proxy(db: &DbPool, proxy_id: &str, provider: &str, region: &str) {
    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               success_count, failure_count, last_verify_status, last_verify_geo_match_ok,
               last_smoke_upstream_ok, last_exit_country, last_exit_region, last_verify_at,
               created_at, updated_at
           ) VALUES (
               ?, 'http', '127.0.0.1', 8080, NULL, NULL, ?, 'US', ?, 'active', 0.95,
               0, 0, 'ok', 1, 1, 'US', ?, '9999999999', '1', '1'
           )"#,
    )
    .bind(proxy_id)
    .bind(region)
    .bind(provider)
    .bind(region)
    .execute(db)
    .await
    .expect("insert active proxy seed");
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

async fn create_browser_task_with_proxy(app: &axum::Router, kind: &str, proxy_id: &str) -> String {
    let payload = serde_json::json!({
        "kind": kind,
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": proxy_id
        }
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
async fn fingerprint_profile_response_includes_canonical_summary_and_runtime_projection() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let profile_payload = serde_json::json!({
        "id": "fp-canonical-summary",
        "name": "Canonical Summary",
        "profile_json": {
            "family_id": "win11_business_laptop",
            "family_variant": "mainstream_ultrabook",
            "control": {
                "browser": {
                    "user_agent": "Mozilla/5.0",
                    "ua_platform": "Win32"
                },
                "os": {
                    "timezone": "Asia/Shanghai"
                },
                "locale": {
                    "locale": "zh-CN",
                    "accept_language": "zh-CN,zh;q=0.9"
                },
                "display": {
                    "screen_width": 1920,
                    "screen_height": 1080,
                    "viewport_width": 1536,
                    "viewport_height": 864,
                    "device_pixel_ratio": 1
                },
                "hardware": {
                    "hardware_concurrency": 8,
                    "device_memory_gb": 16,
                    "touch_support": false,
                    "max_touch_points": 0
                },
                "network": {
                    "sticky_session_ttl": 1800,
                    "rotation_policy": "sticky"
                }
            }
        }
    });

    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/fingerprint-profiles")
            .header("content-type", "application/json")
            .body(Body::from(profile_payload.to_string()))
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        json.get("family_id").and_then(|value| value.as_str()),
        Some("win11_business_laptop")
    );
    assert_eq!(
        json.get("schema_kind").and_then(|value| value.as_str()),
        Some("canonical_grouped")
    );
    assert!(
        json.get("declared_control_count")
            .and_then(|value| value.as_u64())
            .unwrap_or(0)
            >= 10
    );
    assert!(json
        .get("supported_runtime_fields")
        .and_then(|value| value.as_array())
        .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("timezone"))));
    assert!(json
        .get("consumption_explain")
        .and_then(|value| value.get("resolved_fields"))
        .and_then(|value| value.as_array())
        .is_some_and(|items| items
            .iter()
            .any(|item| item.as_str() == Some("device_memory_gb"))));
    assert_eq!(
        json.get("consistency_assessment")
            .and_then(|value| value.get("schema_kind"))
            .and_then(|value| value.as_str()),
        Some("canonical_grouped")
    );
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
    seed_active_proxy(&_state.db, "proxy-fp-open", "fp-seed", "us-east").await;

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-desktop-chrome",
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": "proxy-fp-open"
        }
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
    let task_id = json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();

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

    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result json");
    let fingerprint = result_json
        .get("fingerprint_profile")
        .expect("fingerprint profile in runner result");
    assert_eq!(
        fingerprint.get("id").and_then(|v| v.as_str()),
        Some("fp-desktop-chrome")
    );
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

    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result json");
    assert!(result_json
        .get("fingerprint_profile")
        .unwrap_or(&Value::Null)
        .is_null());
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
    assert!(
        body.contains("fingerprint profile not found or inactive"),
        "unexpected body: {body:?}"
    );
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

    let result_json_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load task result");

    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result json");
    assert!(result_json
        .get("fingerprint_profile")
        .unwrap_or(&Value::Null)
        .is_null());
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
    let task_id = json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let _task = wait_for_terminal_status(&app, &task_id).await;

    let resolved_logs: Vec<String> = sqlx::query_scalar(
        r#"SELECT message FROM logs WHERE task_id = ? ORDER BY created_at ASC, id ASC"#,
    )
    .bind(&task_id)
    .fetch_all(&state.db)
    .await
    .expect("load resolved logs");
    assert!(
        resolved_logs
            .iter()
            .any(|msg| msg.contains("fingerprint profile resolved for runner execution")),
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
        missing_logs
            .iter()
            .any(|msg| msg
                .contains("fingerprint profile requested but not resolved at execution time")),
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
    seed_active_proxy(&state.db, "proxy-fp-status", "fp-seed", "us-east").await;

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-status",
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": "proxy-fp-status"
        }
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
    assert_eq!(
        create_json
            .get("fingerprint_resolution_status")
            .and_then(|v| v.as_str()),
        Some("pending")
    );
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();

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
    assert_eq!(
        task_json
            .get("fingerprint_resolution_status")
            .and_then(|v| v.as_str()),
        Some("resolved")
    );

    let (_, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let latest = status_json
        .get("latest_tasks")
        .and_then(|v| v.as_array())
        .expect("latest tasks");
    assert!(latest.iter().any(|task| {
        task.get("id").and_then(|v| v.as_str()) == Some(task_id.as_str())
            && task
                .get("fingerprint_resolution_status")
                .and_then(|v| v.as_str())
                == Some("resolved")
    }));
    let latest_browser = status_json
        .get("latest_browser_tasks")
        .and_then(|v| v.as_array())
        .expect("latest browser tasks");
    assert!(latest_browser
        .iter()
        .any(|task| task.get("id").and_then(|v| v.as_str()) == Some(task_id.as_str())));

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
    assert_eq!(
        downgraded_json
            .get("fingerprint_resolution_status")
            .and_then(|v| v.as_str()),
        Some("downgraded")
    );
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
    let metrics = json
        .get("fingerprint_metrics")
        .expect("fingerprint metrics");
    assert_eq!(metrics.get("pending").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("resolved").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("downgraded").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(metrics.get("none").and_then(|v| v.as_i64()), Some(1));
    let worker = json.get("worker").expect("worker");
    assert!(
        worker
            .get("fingerprint_medium_max_concurrency")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= 2
    );
    assert!(
        worker
            .get("fingerprint_heavy_max_concurrency")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= 1
    );
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
    assert_eq!(
        retry_json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_QUEUED)
    );

    let cancel_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/tasks/{task_id}/cancel"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("cancel response");
    let cancel_status = cancel_response.status();
    let cancel_body = axum::body::to_bytes(cancel_response.into_body(), usize::MAX)
        .await
        .expect("body");
    let cancel_json: Value = serde_json::from_slice(&cancel_body).expect("json body");

    assert!(matches!(
        cancel_status,
        StatusCode::OK | StatusCode::CONFLICT
    ));
    let final_status: String = sqlx::query_scalar(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_one(&state.db)
        .await
        .expect("final task status");
    assert!(matches!(
        final_status.as_str(),
        TASK_STATUS_CANCELLED | TASK_STATUS_QUEUED | TASK_STATUS_RUNNING | TASK_STATUS_SUCCEEDED
    ));
    assert!(cancel_json.get("status").is_some() || cancel_json.get("message").is_some());
}

#[tokio::test]
async fn status_exposes_worker_backoff_parameterization() {
    std::env::set_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MIN_MS", "333");
    std::env::set_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MAX_MS", "4444");

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

    std::env::remove_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MIN_MS");
    std::env::remove_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MAX_MS");

    assert_eq!(status, StatusCode::OK);
    let worker = json.get("worker").expect("worker");
    assert_eq!(
        worker.get("idle_backoff_min_ms").and_then(|v| v.as_u64()),
        Some(333)
    );
    assert_eq!(
        worker.get("idle_backoff_max_ms").and_then(|v| v.as_u64()),
        Some(4444)
    );
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
    assert!(
        retry_body.contains("does not allow retry"),
        "unexpected body: {retry_body:?}"
    );
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

    let reclaimed = reclaim_stale_running_tasks(&state, 1)
        .await
        .expect("reclaim");
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
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    seed_active_proxy(&state.db, "proxy-fake-success", "default", "us-east").await;
    let task_id = create_browser_task_with_proxy(&app, "open_page", "proxy-fake-success").await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );

    let (_, runs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/runs?limit=5&offset=0"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert!(runs_json.as_array().map(|a| !a.is_empty()).unwrap_or(false));
    assert_eq!(
        runs_json[0].get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );

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
    assert!(status_json
        .get("latest_tasks")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false));
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

    let reclaimed = reclaim_stale_running_tasks(&state, 1)
        .await
        .expect("reclaim");
    assert_eq!(reclaimed, 1);

    let (status, runner_id): (String, Option<String>) =
        sqlx::query_as(r#"SELECT status, runner_id FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load task after reclaim");
    assert_eq!(status, TASK_STATUS_QUEUED);
    assert_eq!(runner_id, None);

    let (run_status, error_message): (String, Option<String>) =
        sqlx::query_as(r#"SELECT status, error_message FROM runs WHERE id = ?"#)
            .bind(&run_id)
            .fetch_one(&state.db)
            .await
            .expect("load run after reclaim");
    assert_eq!(run_status, RUN_STATUS_FAILED);
    assert_eq!(
        error_message.as_deref(),
        Some("reclaimed after stale running timeout")
    );
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

    let reclaimed = reclaim_stale_running_tasks(&state, 1)
        .await
        .expect("reclaim");
    assert_eq!(reclaimed, 0);

    let (status, runner_id): (String, Option<String>) =
        sqlx::query_as(r#"SELECT status, runner_id FROM tasks WHERE id = ?"#)
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
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    seed_active_proxy(&state.db, "proxy-queue-run", "default", "us-east").await;
    let task_id = create_browser_task_with_proxy(&app, "open_page", "proxy-queue-run").await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );
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
    assert_eq!(
        cancel_json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_CANCELLED)
    );
}

#[tokio::test]
async fn reclaimed_task_can_run_again_to_terminal_state() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    seed_active_proxy(&state.db, "proxy-reclaim-rerun", "default", "us-east").await;

    let task_id = "task-reclaim-rerun".to_string();
    let run_id = "run-reclaim-rerun".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{"url":"https://example.com","timeout_seconds":5,"network_policy_json":{"mode":"required_proxy","proxy_id":"proxy-reclaim-rerun"}}', '{"mode":"required_proxy","proxy_id":"proxy-reclaim-rerun"}', NULL, 0, '1', '1', '1', NULL, 'fake-0', NULL, NULL, NULL)"#,
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

    let reclaimed = reclaim_stale_running_tasks(&state, 1)
        .await
        .expect("reclaim");
    assert_eq!(reclaimed, 1);

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );
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
    let retry_response = app
        .clone()
        .oneshot(retry_request)
        .await
        .expect("retry request should succeed");
    let retry_status = retry_response.status();
    let retry_body = axum::body::to_bytes(retry_response.into_body(), usize::MAX)
        .await
        .expect("retry body");
    let retry_text = String::from_utf8(retry_body.to_vec()).expect("retry utf8");
    if retry_status == StatusCode::OK {
        let retry_json: Value = serde_json::from_str(&retry_text).expect("retry json body");
        assert_eq!(
            retry_json.get("status").and_then(|v| v.as_str()),
            Some(TASK_STATUS_QUEUED)
        );
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
    seed_active_proxy(&state.db, "proxy-reclaim-retry", "default", "us-east").await;

    let task_id = "task-reclaim-retry".to_string();
    let run_id = "run-reclaim-retry".to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', ?, '{"url":"https://example.com","timeout_seconds":5,"network_policy_json":{"mode":"required_proxy","proxy_id":"proxy-reclaim-retry"}}', '{"mode":"required_proxy","proxy_id":"proxy-reclaim-retry"}', NULL, 0, '1', '1', '1', NULL, 'fake-0', NULL, NULL, NULL)"#,
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

    let reclaimed = reclaim_stale_running_tasks(&state, 1)
        .await
        .expect("reclaim");
    assert_eq!(reclaimed, 1);

    let retry_request = Request::builder()
        .method("POST")
        .uri(format!("/tasks/{task_id}/retry"))
        .body(Body::empty())
        .expect("request");
    let retry_response = app
        .clone()
        .oneshot(retry_request)
        .await
        .expect("retry request should succeed");
    let retry_status = retry_response.status();
    let retry_body = axum::body::to_bytes(retry_response.into_body(), usize::MAX)
        .await
        .expect("retry body");
    let retry_text = String::from_utf8(retry_body.to_vec()).expect("retry utf8");
    if retry_status == StatusCode::OK {
        let retry_json: Value = serde_json::from_str(&retry_text).expect("retry json body");
        assert_eq!(
            retry_json.get("status").and_then(|v| v.as_str()),
            Some(TASK_STATUS_QUEUED)
        );
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
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );
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

    let reclaimed = reclaim_stale_running_tasks(&state, 1)
        .await
        .expect("reclaim");
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
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_TIMED_OUT)
    );

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
    assert_eq!(
        retry_json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_QUEUED)
    );
}

#[tokio::test]
async fn retry_flow_requeues_failed_fake_task() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = create_task(&app, "fail").await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_FAILED)
    );

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
    assert_eq!(
        retry_json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_QUEUED)
    );
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
    let (create_status, create_body) = text_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(
        create_status,
        StatusCode::CREATED,
        "unexpected create body: {create_body}"
    );
    let create_json: Value = serde_json::from_str(&create_body).expect("create proxy json");
    assert_eq!(
        create_json.get("id").and_then(|v| v.as_str()),
        Some("proxy-us-1")
    );

    let (_, list_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert!(list_json.as_array().map(|a| !a.is_empty()).unwrap_or(false));

    let (_, get_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-us-1")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        get_json.get("region").and_then(|v| v.as_str()),
        Some("us-east")
    );
    assert_eq!(
        get_json.get("provider").and_then(|v| v.as_str()),
        Some("manual")
    );
}

#[tokio::test]
async fn browser_open_creates_open_page_task() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "url": "https://example.com",
        "timeout_seconds": 7,
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": "proxy-browser-open-1"
        }
    });
    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/browser/open")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("open_page"));
    assert_eq!(
        json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_QUEUED)
    );
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id");

    let stored: (String, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT kind, input_json, network_policy_json FROM tasks WHERE id = ?"#)
            .bind(task_id)
            .fetch_one(&state.db)
            .await
            .expect("load browser-open task");
    assert_eq!(stored.0, "open_page");
    let input_json: Value =
        serde_json::from_str(stored.1.as_deref().expect("input_json")).expect("parse input json");
    assert_eq!(
        input_json.get("url").and_then(|v| v.as_str()),
        Some("https://example.com")
    );
    assert_eq!(
        input_json.get("timeout_seconds").and_then(|v| v.as_i64()),
        Some(7)
    );
    let network_policy: Value =
        serde_json::from_str(stored.2.as_deref().expect("network_policy_json"))
            .expect("parse network policy");
    assert_eq!(
        network_policy.get("mode").and_then(|v| v.as_str()),
        Some("required_proxy")
    );
    assert_eq!(
        network_policy.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-browser-open-1")
    );
}

#[tokio::test]
async fn browser_html_creates_get_html_task() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "url": "https://example.com/page",
        "timeout_seconds": 9,
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": "proxy-browser-html-1"
        }
    });
    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/browser/html")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("get_html"));
    assert_eq!(
        json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_QUEUED)
    );
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id");

    let stored: (String, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT kind, input_json, network_policy_json FROM tasks WHERE id = ?"#)
            .bind(task_id)
            .fetch_one(&state.db)
            .await
            .expect("load browser-html task");
    assert_eq!(stored.0, "get_html");
    let input_json: Value =
        serde_json::from_str(stored.1.as_deref().expect("input_json")).expect("parse input json");
    assert_eq!(
        input_json.get("url").and_then(|v| v.as_str()),
        Some("https://example.com/page")
    );
    assert_eq!(
        input_json.get("timeout_seconds").and_then(|v| v.as_i64()),
        Some(9)
    );
    let network_policy: Value =
        serde_json::from_str(stored.2.as_deref().expect("network_policy_json"))
            .expect("parse network policy");
    assert_eq!(
        network_policy.get("mode").and_then(|v| v.as_str()),
        Some("required_proxy")
    );
    assert_eq!(
        network_policy.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-browser-html-1")
    );
}

#[tokio::test]
async fn browser_title_creates_get_title_task() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "url": "https://example.com/title",
        "timeout_seconds": 6,
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": "proxy-browser-title-1"
        }
    });
    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/browser/title")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("get_title"));
    assert_eq!(
        json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_QUEUED)
    );
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id");

    let stored: (String, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT kind, input_json, network_policy_json FROM tasks WHERE id = ?"#)
            .bind(task_id)
            .fetch_one(&state.db)
            .await
            .expect("load browser-title task");
    assert_eq!(stored.0, "get_title");
    let input_json: Value =
        serde_json::from_str(stored.1.as_deref().expect("input_json")).expect("parse input json");
    assert_eq!(
        input_json.get("url").and_then(|v| v.as_str()),
        Some("https://example.com/title")
    );
    assert_eq!(
        input_json.get("timeout_seconds").and_then(|v| v.as_i64()),
        Some(6)
    );
    let network_policy: Value =
        serde_json::from_str(stored.2.as_deref().expect("network_policy_json"))
            .expect("parse network policy");
    assert_eq!(
        network_policy.get("mode").and_then(|v| v.as_str()),
        Some("required_proxy")
    );
    assert_eq!(
        network_policy.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-browser-title-1")
    );
}

#[tokio::test]
async fn browser_final_url_creates_get_final_url_task() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "url": "https://example.com/redirect",
        "timeout_seconds": 8,
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": "proxy-browser-final-url-1"
        }
    });
    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/browser/final-url")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("get_final_url")
    );
    assert_eq!(
        json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_QUEUED)
    );
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id");

    let stored: (String, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT kind, input_json, network_policy_json FROM tasks WHERE id = ?"#)
            .bind(task_id)
            .fetch_one(&state.db)
            .await
            .expect("load browser-final-url task");
    assert_eq!(stored.0, "get_final_url");
    let input_json: Value =
        serde_json::from_str(stored.1.as_deref().expect("input_json")).expect("parse input json");
    assert_eq!(
        input_json.get("url").and_then(|v| v.as_str()),
        Some("https://example.com/redirect")
    );
    assert_eq!(
        input_json.get("timeout_seconds").and_then(|v| v.as_i64()),
        Some(8)
    );
    let network_policy: Value =
        serde_json::from_str(stored.2.as_deref().expect("network_policy_json"))
            .expect("parse network policy");
    assert_eq!(
        network_policy.get("mode").and_then(|v| v.as_str()),
        Some("required_proxy")
    );
    assert_eq!(
        network_policy.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-browser-final-url-1")
    );
}

#[tokio::test]
async fn browser_text_creates_extract_text_task() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "url": "https://example.com/article",
        "timeout_seconds": 10,
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": "proxy-browser-text-1"
        }
    });
    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/browser/text")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        json.get("kind").and_then(|v| v.as_str()),
        Some("extract_text")
    );
    assert_eq!(
        json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_QUEUED)
    );
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id");

    let stored: (String, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT kind, input_json, network_policy_json FROM tasks WHERE id = ?"#)
            .bind(task_id)
            .fetch_one(&state.db)
            .await
            .expect("load browser-text task");
    assert_eq!(stored.0, "extract_text");
    let input_json: Value =
        serde_json::from_str(stored.1.as_deref().expect("input_json")).expect("parse input json");
    assert_eq!(
        input_json.get("url").and_then(|v| v.as_str()),
        Some("https://example.com/article")
    );
    assert_eq!(
        input_json.get("timeout_seconds").and_then(|v| v.as_i64()),
        Some(10)
    );
    let network_policy: Value =
        serde_json::from_str(stored.2.as_deref().expect("network_policy_json"))
            .expect("parse network policy");
    assert_eq!(
        network_policy.get("mode").and_then(|v| v.as_str()),
        Some("required_proxy")
    );
    assert_eq!(
        network_policy.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-browser-text-1")
    );
}

#[tokio::test]
async fn browser_outward_contracts_roundtrip_across_task_and_run_views() {
    let cases = [
        (
            "get_html",
            serde_json::json!({
                "kind": "get_html",
                "url": "https://example.com/html",
                "timeout_seconds": 7,
                "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-browser-contract-html"}
            }),
            Some("content_preview"),
            Some("text/html"),
            Some("get_html"),
            Some(true),
            Some("Fake title for https://example.com/html"),
            Some("https://example.com/html#final"),
        ),
        (
            "get_title",
            serde_json::json!({
                "kind": "get_title",
                "url": "https://example.com/title-contract",
                "timeout_seconds": 7,
                "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-browser-contract-title"}
            }),
            None,
            None,
            None,
            None,
            Some("Fake title for https://example.com/title-contract"),
            Some("https://example.com/title-contract#final"),
        ),
        (
            "get_final_url",
            serde_json::json!({
                "kind": "get_final_url",
                "url": "https://example.com/final-contract",
                "timeout_seconds": 7,
                "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-browser-contract-final"}
            }),
            None,
            None,
            None,
            None,
            Some("Fake title for https://example.com/final-contract"),
            Some("https://example.com/final-contract#final"),
        ),
        (
            "extract_text",
            serde_json::json!({
                "kind": "extract_text",
                "url": "https://example.com/text-contract",
                "timeout_seconds": 7,
                "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-browser-contract-text"}
            }),
            Some("content_preview"),
            Some("text/plain"),
            Some("extract_text"),
            Some(true),
            Some("Fake title for https://example.com/text-contract"),
            Some("https://example.com/text-contract#final"),
        ),
    ];

    for (
        kind,
        payload,
        preview_key,
        expected_content_kind,
        expected_source_action,
        expected_content_ready,
        expected_title,
        expected_final_url,
    ) in cases
    {
        let db_url = unique_db_url();
        let (state, app) = build_test_app(&db_url).await.expect("build app");
        let proxy_id = payload
            .get("network_policy_json")
            .and_then(|value| value.get("proxy_id"))
            .and_then(|value| value.as_str())
            .expect("proxy id in payload");
        seed_active_proxy(&state.db, proxy_id, "browser-contract", "us-east").await;

        let (_, create_json) = json_response(
            &app,
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request"),
        )
        .await;
        let task_id = create_json
            .get("id")
            .and_then(|v| v.as_str())
            .expect("task id")
            .to_string();
        let task_json = wait_for_terminal_status(&app, &task_id).await;

        let (status, runs_json) = json_response(
            &app,
            Request::builder()
                .uri(format!("/tasks/{task_id}/runs"))
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "kind={kind}");
        let runs = runs_json.as_array().expect("runs array");
        assert!(!runs.is_empty(), "kind={kind}");
        let run = &runs[0];

        assert_eq!(run.get("title"), task_json.get("title"), "kind={kind}");
        assert_eq!(
            run.get("final_url"),
            task_json.get("final_url"),
            "kind={kind}"
        );
        assert_eq!(
            run.get("content_preview"),
            task_json.get("content_preview"),
            "kind={kind}"
        );
        assert_eq!(
            run.get("content_length"),
            task_json.get("content_length"),
            "kind={kind}"
        );
        assert_eq!(
            run.get("content_truncated"),
            task_json.get("content_truncated"),
            "kind={kind}"
        );
        assert_eq!(
            run.get("content_kind"),
            task_json.get("content_kind"),
            "kind={kind}"
        );
        assert_eq!(
            run.get("content_source_action"),
            task_json.get("content_source_action"),
            "kind={kind}"
        );
        assert_eq!(
            run.get("content_ready"),
            task_json.get("content_ready"),
            "kind={kind}"
        );

        assert_eq!(
            task_json.get("content_kind").and_then(|v| v.as_str()),
            expected_content_kind,
            "kind={kind}"
        );
        assert_eq!(
            task_json
                .get("content_source_action")
                .and_then(|v| v.as_str()),
            expected_source_action,
            "kind={kind}"
        );
        assert_eq!(
            task_json.get("content_ready").and_then(|v| v.as_bool()),
            expected_content_ready,
            "kind={kind}"
        );
        assert_eq!(
            task_json.get("title").and_then(|v| v.as_str()),
            expected_title,
            "kind={kind}"
        );
        assert_eq!(
            task_json.get("final_url").and_then(|v| v.as_str()),
            expected_final_url,
            "kind={kind}"
        );

        match preview_key {
            Some("content_preview") => assert!(
                task_json
                    .get("content_preview")
                    .and_then(|v| v.as_str())
                    .map(|v| !v.is_empty())
                    .unwrap_or(false),
                "kind={kind}"
            ),
            _ => assert!(
                task_json.get("content_preview").is_none()
                    || task_json
                        .get("content_preview")
                        .is_some_and(|v| v.is_null()),
                "kind={kind}"
            ),
        }
    }
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
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id");

    let stored: Option<String> =
        sqlx::query_scalar(r#"SELECT network_policy_json FROM tasks WHERE id = ?"#)
            .bind(task_id)
            .fetch_one(&state.db)
            .await
            .expect("load task network policy");
    let parsed: Value = serde_json::from_str(stored.as_deref().expect("network policy"))
        .expect("parse network policy");
    assert_eq!(
        parsed.get("mode").and_then(|v| v.as_str()),
        Some("required_proxy")
    );
    assert_eq!(
        parsed.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-us-1")
    );
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
    let success_task_id = success_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
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
    let timeout_task_id = timeout_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
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
async fn proxy_health_is_not_penalized_after_cancelled_execution() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-health-cancel', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'manual', 'active', 0.95, 2, 1, NULL, NULL, NULL, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-health-cancel"}
    });
    let (_, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();

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
    assert_eq!(
        cancel_json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_CANCELLED)
    );

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_CANCELLED)
    );

    let (success_count, failure_count, cooldown_until): (i64, i64, Option<String>) =
        sqlx::query_as(r#"SELECT success_count, failure_count, cooldown_until FROM proxies WHERE id = 'proxy-health-cancel'"#)
            .fetch_one(&state.db)
            .await
            .expect("load proxy after cancel");
    assert_eq!(success_count, 2);
    assert_eq!(failure_count, 1);
    assert!(cooldown_until.is_none());
}

#[tokio::test]
async fn proxy_health_is_updated_after_failed_execution_with_stage_evidence() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-health-fail', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'manual', 'active', 0.95, 1, 0, NULL, NULL, NULL, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let payload = serde_json::json!({
        "kind": "fail",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-health-fail"}
    });
    let (_, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_FAILED)
    );

    let (success_count, failure_count, cooldown_until, score): (i64, i64, Option<String>, f64) =
        sqlx::query_as(r#"SELECT success_count, failure_count, cooldown_until, score FROM proxies WHERE id = 'proxy-health-fail'"#)
            .fetch_one(&state.db)
            .await
            .expect("load proxy after fail");
    assert_eq!(success_count, 1);
    assert_eq!(failure_count, 1);
    assert!(cooldown_until.is_some());
    assert!(score < 0.95);
}

#[tokio::test]
async fn proxy_health_skips_timeout_penalty_without_stage_evidence() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let state = build_app_state(db.clone(), std::sync::Arc::new(FakeRunner), None, 1);

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, created_at, updated_at) VALUES ('proxy-health-no-evidence', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'manual', 'active', 0.91, 4, 2, NULL, NULL, NULL, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let proxy = RunnerProxySelection {
        id: "proxy-health-no-evidence".to_string(),
        scheme: "http".to_string(),
        host: "127.0.0.1".to_string(),
        port: 8080,
        username: None,
        password: None,
        region: Some("us-east".to_string()),
        country: Some("US".to_string()),
        provider: Some("manual".to_string()),
        score: 0.91,
        resolution_status: "selected".to_string(),
        source_label: None,
        source_tier: None,
        verification_path: None,
        last_verify_source: None,
        last_exit_country: None,
        last_exit_region: None,
    };

    update_proxy_health_after_execution(
        &state,
        Some(&proxy),
        RunnerOutcomeStatus::TimedOut,
        Some(&serde_json::json!({
            "runner": "fake",
            "status": "timed_out",
            "error_kind": "timeout"
        })),
    )
    .await
    .expect("update proxy health");

    let (success_count, failure_count, cooldown_until, score): (i64, i64, Option<String>, f64) =
        sqlx::query_as(r#"SELECT success_count, failure_count, cooldown_until, score FROM proxies WHERE id = 'proxy-health-no-evidence'"#)
            .fetch_one(&state.db)
            .await
            .expect("load proxy after no-evidence timeout");
    assert_eq!(success_count, 4);
    assert_eq!(failure_count, 2);
    assert!(cooldown_until.is_none());
    assert_eq!(score, 0.91);
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
    let (_, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let _ = wait_for_terminal_status(&app, &task_id).await;

    let result_json_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load result");
    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result");
    assert_eq!(
        result_json
            .get("proxy")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str()),
        Some("proxy-allowed")
    );
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
    let (_, json1) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload1.to_string()))
            .expect("request"),
    )
    .await;
    let task_id1 = json1
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let _ = wait_for_terminal_status(&app, &task_id1).await;

    let result1_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id1)
            .fetch_one(&state.db)
            .await
            .expect("load result1");
    let result1: Value = serde_json::from_str(result1_text.as_deref().expect("result1 json"))
        .expect("parse result1");
    let first_proxy_id = result1
        .get("proxy")
        .and_then(|v| v.get("id"))
        .and_then(|v| v.as_str())
        .expect("first proxy")
        .to_string();

    let payload2 = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/2",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-a", "sticky_session": sticky}
    });
    let (_, json2) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload2.to_string()))
            .expect("request"),
    )
    .await;
    let task_id2 = json2
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let _ = wait_for_terminal_status(&app, &task_id2).await;

    let result2_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id2)
            .fetch_one(&state.db)
            .await
            .expect("load result2");
    let result2: Value = serde_json::from_str(result2_text.as_deref().expect("result2 json"))
        .expect("parse result2");
    assert_eq!(
        result2
            .get("proxy")
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str()),
        Some(first_proxy_id.as_str())
    );
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
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(proxy_status, StatusCode::CREATED);

    let task_payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-observe-1"}
    });
    let (create_status, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(task_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();

    let task_detail = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task_detail.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-observe-1")
    );
    assert_eq!(
        task_detail.get("proxy_provider").and_then(|v| v.as_str()),
        Some("pool-observe")
    );
    assert_eq!(
        task_detail.get("proxy_region").and_then(|v| v.as_str()),
        Some("us-east")
    );
    assert!(matches!(
        task_detail
            .get("proxy_resolution_status")
            .and_then(|v| v.as_str()),
        Some("resolved") | Some("resolved_sticky")
    ));

    let (_, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        status_json
            .get("proxy_metrics")
            .and_then(|v| v.get("resolved"))
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    let latest = status_json
        .get("latest_tasks")
        .and_then(|v| v.as_array())
        .expect("latest tasks");
    assert!(latest
        .iter()
        .any(|task| task.get("proxy_id").and_then(|v| v.as_str()) == Some("proxy-observe-1")));
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
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (smoke_status, smoke_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-smoke-dead/smoke")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(smoke_status, StatusCode::OK);
    assert_eq!(
        smoke_json.get("reachable").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        smoke_json.get("protocol_ok").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        smoke_json.get("upstream_ok").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        smoke_json.get("anonymity_level").and_then(|v| v.as_str()),
        None
    );

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
    let (_, json1) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id1 = json1
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let _ = wait_for_terminal_status(&app, &task_id1).await;

    let (bound_proxy_id, bound_provider): (String, Option<String>) = sqlx::query_as(
        r#"SELECT proxy_id, provider FROM proxy_session_bindings WHERE session_key = ?"#,
    )
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
    let (_, json2) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload2.to_string()))
            .expect("request"),
    )
    .await;
    let task_id2 = json2
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task2 = wait_for_terminal_status(&app, &task_id2).await;
    assert_eq!(
        task2.get("proxy_id").and_then(|v| v.as_str()),
        Some(bound_proxy_id.as_str())
    );
    assert_eq!(
        task2
            .get("proxy_resolution_status")
            .and_then(|v| v.as_str()),
        Some("resolved_sticky")
    );
}

#[tokio::test]
async fn browser_task_region_shortage_falls_back_to_other_active_region() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    seed_active_proxy(&state.db, "proxy-region-fallback", "pool-a", "eu-west").await;

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/fallback",
        "timeout_seconds": 5,
        "network_policy_json": {
            "mode": "required_proxy",
            "provider": "pool-a",
            "region": "us-east"
        }
    });
    let (_, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );
    assert_eq!(
        task.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-region-fallback")
    );
    assert_eq!(
        task.get("proxy_region").and_then(|v| v.as_str()),
        Some("eu-west")
    );
    assert_eq!(
        task.get("selection_explain")
            .and_then(|v| v.get("fallback_reason"))
            .and_then(|v| v.as_str()),
        Some("region_shortage_fallback_to_any_active"),
    );
    assert!(task
        .get("selection_reason_summary")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .contains("fell back to another active region"));
}

#[tokio::test]
async fn browser_task_site_history_prefers_proxy_with_better_site_record() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    seed_active_proxy(&state.db, "proxy-site-good", "pool-a", "us-east").await;
    seed_active_proxy(&state.db, "proxy-site-bad", "pool-a", "us-east").await;

    sqlx::query(
        r#"UPDATE proxies
           SET score = CASE id
               WHEN 'proxy-site-good' THEN 0.80
               WHEN 'proxy-site-bad' THEN 0.99
               ELSE score
           END
           WHERE id IN ('proxy-site-good', 'proxy-site-bad')"#,
    )
    .execute(&state.db)
    .await
    .expect("adjust proxy scores");

    sqlx::query(
        r#"INSERT INTO proxy_site_stats (
               proxy_id, site_key, success_count, failure_count,
               last_success_at, last_failure_at, last_failure_scope, last_browser_failure_signal, updated_at
           ) VALUES
           ('proxy-site-good', 'example.com', 4, 0, '10', NULL, NULL, NULL, '10'),
           ('proxy-site-bad', 'example.com', 0, 4, NULL, '11', 'browser_execution', 'browser_navigation_failure_signal', '11')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert proxy site stats");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/site-aware",
        "timeout_seconds": 5,
        "network_policy_json": {
            "mode": "required_proxy",
            "provider": "pool-a",
            "region": "us-east"
        }
    });
    let (_, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );
    assert_eq!(
        task.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-site-good")
    );
    assert!(task
        .get("selection_reason_summary")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .contains("site_success"));
    let result_json_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load result json");
    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result json");
    let preview = result_json
        .get("payload")
        .and_then(|v| v.get("network_policy_json"))
        .and_then(|v| v.get("candidate_rank_preview"))
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        preview
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str()),
        Some("proxy-site-good")
    );
    assert!(preview
        .first()
        .and_then(|v| v.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .contains("site_success"));
    assert!(preview
        .get(1)
        .and_then(|v| v.get("summary"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .contains("site_failure"));
}

#[tokio::test]
async fn browser_task_strict_region_shortage_fails_without_fallback() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    seed_active_proxy(&state.db, "proxy-region-strict", "pool-a", "eu-west").await;

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/strict-region",
        "timeout_seconds": 5,
        "network_policy_json": {
            "mode": "required_proxy",
            "provider": "pool-a",
            "region": "us-east",
            "strict_region": true
        }
    });
    let (_, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_FAILED)
    );
    assert_eq!(
        task.get("failure_scope").and_then(|v| v.as_str()),
        Some("network_policy")
    );
    assert_eq!(
        task.get("proxy_resolution_status").and_then(|v| v.as_str()),
        Some("unresolved")
    );
    assert_eq!(
        task.get("selection_explain")
            .and_then(|v| v.get("no_match_reason_code"))
            .and_then(|v| v.as_str()),
        Some("no_match_after_strict_region_filter"),
    );
    assert_eq!(
        task.get("selection_explain")
            .and_then(|v| v.get("fallback_reason"))
            .and_then(|v| v.as_str()),
        Some("auto_primary_path"),
    );
}

#[tokio::test]
async fn browser_task_auto_identity_session_reuses_bound_proxy() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let fingerprint_profile = serde_json::json!({
        "id": "fp-auto-session",
        "name": "Auto Session",
        "profile_json": {
            "browser": {"name": "chrome", "version": "123"},
            "os": {"name": "windows", "version": "11"}
        }
    });
    let (profile_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/fingerprint-profiles")
            .header("content-type", "application/json")
            .body(Body::from(fingerprint_profile.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(profile_status, StatusCode::CREATED);

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               success_count, failure_count, last_verify_status, last_verify_geo_match_ok,
               last_smoke_upstream_ok, last_exit_country, last_exit_region, last_verify_at,
               created_at, updated_at
           ) VALUES (
               'proxy-auto-a', 'http', '127.0.0.1', 8180, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.99,
               0, 0, 'ok', 1, 1, 'US', 'us-east', '9999999999', '1', '1'
           ),
           (
               'proxy-auto-b', 'http', '127.0.0.2', 8181, NULL, NULL, 'us-east', 'US', 'pool-a', 'active', 0.80,
               0, 0, 'ok', 1, 1, 'US', 'us-east', '9999999999', '1', '1'
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert active proxies");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/session/one",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-auto-session",
        "network_policy_json": {
            "mode": "required_proxy",
            "provider": "pool-a",
            "region": "us-east"
        }
    });
    let (_, create_one) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id_one = create_one
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task one id")
        .to_string();
    let task_one = wait_for_terminal_status(&app, &task_id_one).await;
    assert_eq!(
        task_one.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );
    assert_eq!(
        task_one
            .get("execution_identity")
            .and_then(|v| v.get("identity_session_status"))
            .and_then(|v| v.as_str()),
        Some("auto_created"),
    );

    let (session_key, bound_proxy_id, site_key, bound_fp_id): (
        String,
        String,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        r#"SELECT session_key, proxy_id, site_key, fingerprint_profile_id
               FROM proxy_session_bindings
               WHERE fingerprint_profile_id = 'fp-auto-session'"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load auto session binding");
    assert!(session_key.contains("fp-auto-session"));
    assert_eq!(site_key.as_deref(), Some("example.com"));
    assert_eq!(bound_fp_id.as_deref(), Some("fp-auto-session"));
    assert_eq!(bound_proxy_id, "proxy-auto-a");

    let payload_two = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/session/two",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-auto-session",
        "network_policy_json": {
            "mode": "required_proxy",
            "provider": "pool-a",
            "region": "us-east"
        }
    });
    let (_, create_two) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload_two.to_string()))
            .expect("request"),
    )
    .await;
    let task_id_two = create_two
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task two id")
        .to_string();
    let task_two = wait_for_terminal_status(&app, &task_id_two).await;
    assert_eq!(
        task_two.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );
    assert_eq!(
        task_two.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-auto-a")
    );
    assert_eq!(
        task_two
            .get("proxy_resolution_status")
            .and_then(|v| v.as_str()),
        Some("resolved_sticky")
    );
    assert_eq!(
        task_two
            .get("execution_identity")
            .and_then(|v| v.get("identity_session_status"))
            .and_then(|v| v.as_str()),
        Some("auto_reused"),
    );
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
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(
                    b"HTTP/1.1 200 Connection Established

ip=9.9.9.9
",
                ),
            )
            .await;
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
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (smoke_status, smoke_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-smoke-http/smoke")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(smoke_status, StatusCode::OK);
    assert_eq!(
        smoke_json.get("reachable").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        smoke_json.get("protocol_ok").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        smoke_json.get("upstream_ok").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        smoke_json.get("exit_ip").and_then(|v| v.as_str()),
        Some("9.9.9.9")
    );
    assert_eq!(
        smoke_json.get("anonymity_level").and_then(|v| v.as_str()),
        Some("elite")
    );

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
    assert_eq!(last_exit_ip.as_deref(), Some("9.9.9.9"));
    assert_eq!(last_anonymity_level.as_deref(), Some("elite"));
}

#[tokio::test]
async fn proxy_smoke_test_classifies_transparent_proxy_response() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(
                    b"HTTP/1.1 200 Connection Established
X-Forwarded-For: 198.51.100.7

ip=8.8.4.4
",
                ),
            )
            .await;
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
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (smoke_status, smoke_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-smoke-transparent/smoke")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(smoke_status, StatusCode::OK);
    assert_eq!(
        smoke_json.get("anonymity_level").and_then(|v| v.as_str()),
        Some("transparent")
    );
    assert_eq!(
        smoke_json.get("exit_ip").and_then(|v| v.as_str()),
        Some("8.8.4.4")
    );
}

#[tokio::test]
async fn verify_proxy_reports_geo_match_and_country_region() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(
                    b"HTTP/1.1 200 Connection Established

ip=8.8.8.8
country=US
region=Virginia
",
                ),
            )
            .await;
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
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (verify_status, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-us/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(verify_status, StatusCode::OK);
    assert_eq!(
        verify_json.get("reachable").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        verify_json.get("protocol_ok").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        verify_json.get("upstream_ok").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        verify_json.get("exit_ip").and_then(|v| v.as_str()),
        Some("8.8.8.8")
    );
    assert_eq!(
        verify_json.get("exit_country").and_then(|v| v.as_str()),
        Some("US")
    );
    assert_eq!(
        verify_json.get("exit_region").and_then(|v| v.as_str()),
        Some("Virginia")
    );
    assert_eq!(
        verify_json.get("geo_match_ok").and_then(|v| v.as_bool()),
        Some(true)
    );

    let (_, proxy_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-verify-us")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        proxy_json
            .get("last_verify_status")
            .and_then(|v| v.as_str()),
        Some("ok")
    );
    assert_eq!(
        proxy_json
            .get("last_verify_geo_match_ok")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        proxy_json.get("last_exit_ip").and_then(|v| v.as_str()),
        Some("8.8.8.8")
    );
    assert_eq!(
        proxy_json.get("last_exit_country").and_then(|v| v.as_str()),
        Some("US")
    );
    assert_eq!(
        proxy_json.get("last_exit_region").and_then(|v| v.as_str()),
        Some("Virginia")
    );
    assert_eq!(
        proxy_json
            .get("last_anonymity_level")
            .and_then(|v| v.as_str()),
        Some("elite")
    );
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
            Request::builder()
                .method("POST")
                .uri("/proxies")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request"),
        )
        .await;
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
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = task_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-verified-best")
    );
}

#[tokio::test]
async fn verify_proxy_task_kind_executes_and_persists_result() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(
                    b"HTTP/1.1 200 Connection Established

ip=1.1.1.1
country=US
region=Oregon
",
                ),
            )
            .await;
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
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let task_payload = serde_json::json!({
        "kind": "verify_proxy",
        "proxy_id": "proxy-task-verify",
        "timeout_seconds": 5
    });
    let (_, task_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(task_payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = task_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_SUCCEEDED)
    );

    let (_, proxy_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-task-verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        proxy_json
            .get("last_verify_status")
            .and_then(|v| v.as_str()),
        Some("ok")
    );
    assert_eq!(
        proxy_json.get("last_exit_region").and_then(|v| v.as_str()),
        Some("Oregon")
    );
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
            Request::builder()
                .method("POST")
                .uri("/proxies")
                .header("content-type", "application/json")
                .body(Body::from(proxy_payload.to_string()))
                .expect("request"),
        )
        .await;
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
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert!(batch_json
        .get("batch_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .starts_with("verify-batch-"));
    assert!(batch_json
        .get("created_at")
        .and_then(|v| v.as_str())
        .is_some());
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(2));
    assert_eq!(
        batch_json
            .get("stale_after_seconds")
            .and_then(|v| v.as_i64()),
        Some(7200)
    );
    assert_eq!(
        batch_json
            .get("task_timeout_seconds")
            .and_then(|v| v.as_i64()),
        Some(9)
    );
    assert_eq!(
        batch_json
            .get("provider_summary")
            .and_then(|v| v.as_array())
            .map(|v| v.len()),
        Some(1)
    );

    let batch_id = batch_json
        .get("batch_id")
        .and_then(|v| v.as_str())
        .expect("batch id");
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
async fn verify_batch_executes_verify_tasks_and_persists_proxy_results() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "demo_public");
    let listener_one = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener one");
    let addr_one = listener_one.local_addr().expect("local addr one");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener_one.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\nip=1.1.1.1\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let listener_two = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener two");
    let addr_two = listener_two.local_addr().expect("local addr two");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener_two.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\nip=1.1.1.2\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES (
               'source-batch-run', 'manual', 'controlled', 1, 1, 'external_probe_v2',
               'high', 'internal', 1, '{}', 300, 1.0,
               0, NULL, NULL, NULL, 'completed', NULL, 100.0, '1', '1'
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert verify source registry");

    for (id, host, port) in [
        (
            "proxy-batch-run-1",
            addr_one.ip().to_string(),
            addr_one.port(),
        ),
        (
            "proxy-batch-run-2",
            addr_two.ip().to_string(),
            addr_two.port(),
        ),
    ] {
        let proxy_payload = serde_json::json!({
            "id": id,
            "scheme": "http",
            "host": host,
            "port": port,
            "region": "us-east",
            "country": "US",
            "provider": "pool-batch-run",
            "score": 0.9
        });
        let (status, _) = json_response(
            &app,
            Request::builder()
                .method("POST")
                .uri("/proxies")
                .header("content-type", "application/json")
                .body(Body::from(proxy_payload.to_string()))
                .expect("request"),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }

    sqlx::query(
        r#"UPDATE proxies
           SET source_label = 'source-batch-run', last_seen_at = '9999999999', updated_at = '9999999999'
           WHERE provider = 'pool-batch-run'"#,
    )
    .execute(&state.db)
    .await
    .expect("attach source metadata to batch proxies");

    let batch_payload = serde_json::json!({
        "provider": "pool-batch-run",
        "region": "us-east",
        "limit": 10,
        "only_stale": true,
        "task_timeout_seconds": 5,
        "recently_used_within_seconds": 0,
        "failed_only": false
    });
    let (batch_status, batch_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(2));
    let batch_id = batch_json
        .get("batch_id")
        .and_then(|v| v.as_str())
        .expect("batch id");

    let mut task_ids: Vec<String> = Vec::new();
    for _ in 0..12 {
        task_ids = sqlx::query_scalar(r#"SELECT id FROM tasks WHERE kind = 'verify_proxy' AND json_extract(input_json, '$.verify_batch_id') = ? ORDER BY id ASC"#)
            .bind(batch_id)
            .fetch_all(&state.db)
            .await
            .expect("load verify task ids");
        if task_ids.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(task_ids.len(), 2);

    for task_id in &task_ids {
        let task = wait_for_terminal_status(&app, task_id).await;
        assert_eq!(
            task.get("status").and_then(|v| v.as_str()),
            Some(TASK_STATUS_SUCCEEDED)
        );
    }

    for proxy_id in ["proxy-batch-run-1", "proxy-batch-run-2"] {
        let (_, proxy_json) = json_response(
            &app,
            Request::builder()
                .uri(format!("/proxies/{proxy_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        assert_eq!(
            proxy_json
                .get("last_verify_status")
                .and_then(|v| v.as_str()),
            Some("ok")
        );
        assert_eq!(
            proxy_json.get("last_exit_region").and_then(|v| v.as_str()),
            Some("Virginia")
        );
    }
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
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    let metrics = status_json.get("verify_metrics").expect("verify metrics");
    assert_eq!(metrics.get("verified_ok").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(
        metrics.get("verified_failed").and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        metrics.get("geo_match_ok").and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        metrics
            .get("stale_or_missing_verify")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
}

#[tokio::test]
async fn status_exposes_mode_and_effective_proxy_pool_ratio_fields() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-prod', 'text_url', 'private', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1'),
           ('source-demo', 'text_url', 'public', 1, 0, 'demo_smoke', 'unknown', 'free', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 70.0, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source registry");

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, last_seen_at, last_verify_status, last_verify_at, created_at, updated_at
           ) VALUES
           ('proxy-prod-active', 'http', '127.0.0.1', 9101, NULL, NULL, 'us-east', 'US', 'pool-prod', 'active', 0.9, 'source-prod', '9999999999', 'ok', '9999999999', '1', '9999999999'),
           ('proxy-prod-candidate', 'http', '127.0.0.1', 9102, NULL, NULL, 'us-east', 'US', 'pool-prod', 'candidate', 0.6, 'source-prod', '9999999999', NULL, NULL, '1', '9999999999'),
           ('proxy-prod-rejected', 'http', '127.0.0.1', 9103, NULL, NULL, 'us-east', 'US', 'pool-prod', 'candidate_rejected', 0.4, 'source-prod', '9999999999', 'failed', '9999999999', '1', '9999999999'),
           ('proxy-demo-active', 'http', '127.0.0.1', 9104, NULL, NULL, 'us-east', 'US', 'pool-demo', 'active', 0.95, 'source-demo', '9999999999', 'ok', '9999999999', '1', '9999999999')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert proxies");

    let (status_code, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status_code, StatusCode::OK);
    assert_eq!(
        status_json.get("mode").and_then(|v| v.as_str()),
        Some("prod_live")
    );

    let pool = status_json
        .get("proxy_pool_status")
        .expect("proxy_pool_status");
    assert_eq!(pool.get("mode").and_then(|v| v.as_str()), Some("prod_live"));
    assert_eq!(pool.get("total").and_then(|v| v.as_i64()), Some(3));
    assert_eq!(pool.get("active").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(pool.get("candidate").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(
        pool.get("candidate_rejected").and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        pool.get("eligible_pool_total").and_then(|v| v.as_i64()),
        Some(3)
    );
    assert_eq!(
        pool.get("fresh_candidate_total").and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        pool.get("recent_rejected_total").and_then(|v| v.as_i64()),
        Some(1)
    );

    let reported = pool
        .get("reported_active_ratio_percent")
        .and_then(|v| v.as_f64())
        .expect("reported ratio");
    let effective = pool
        .get("effective_active_ratio_percent")
        .and_then(|v| v.as_f64())
        .expect("effective ratio");
    let compatibility = pool
        .get("active_ratio_percent")
        .and_then(|v| v.as_f64())
        .expect("compat ratio");
    assert!((reported - 33.3333).abs() < 0.1, "reported={reported}");
    assert!((effective - 33.3333).abs() < 0.1, "effective={effective}");
    assert!((compatibility - reported).abs() < f64::EPSILON);
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
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(0));
}

#[tokio::test]
async fn verify_batch_filters_demo_only_sources_out_of_prod_live_mode() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-prod-verify', 'text_url', 'private', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1'),
           ('source-demo-verify', 'text_url', 'public', 1, 0, 'demo_smoke', 'unknown', 'free', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 60.0, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source registry");

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, last_verify_status, last_verify_at, created_at, updated_at
           ) VALUES
           ('proxy-prod-verify', 'http', '127.0.0.1', 9201, NULL, NULL, 'us-east', 'US', 'pool-mode', 'active', 0.91, 'source-prod-verify', NULL, NULL, '1', '1'),
           ('proxy-demo-verify', 'http', '127.0.0.1', 9202, NULL, NULL, 'us-east', 'US', 'pool-mode', 'active', 0.99, 'source-demo-verify', NULL, NULL, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert active proxies");

    let batch_payload = serde_json::json!({
        "provider": "pool-mode",
        "region": "us-east",
        "limit": 10,
        "only_stale": true,
        "stale_after_seconds": 7200
    });
    let (batch_status, batch_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(1));

    let queued_proxy_id: String = sqlx::query_scalar(
        r#"SELECT json_extract(input_json, '$.proxy_id')
           FROM tasks
           WHERE kind = 'verify_proxy'
           ORDER BY id DESC
           LIMIT 1"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load queued proxy id");
    assert_eq!(queued_proxy_id, "proxy-prod-verify");

    let filters_json: String = sqlx::query_scalar(
        r#"SELECT filters_json
           FROM verify_batches
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load filters json");
    let filters_value: Value = serde_json::from_str(&filters_json).expect("parse filters json");
    assert_eq!(
        filters_value.get("mode").and_then(|v| v.as_str()),
        Some("prod_live")
    );
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
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
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
            Request::builder()
                .method("POST")
                .uri("/proxies")
                .header("content-type", "application/json")
                .body(Body::from(proxy_payload.to_string()))
                .expect("request"),
        )
        .await;
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
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(2));
    let summary = batch_json
        .get("provider_summary")
        .and_then(|v| v.as_array())
        .expect("provider summary");
    assert_eq!(summary.len(), 2);
    assert!(summary
        .iter()
        .any(
            |item| item.get("provider").and_then(|v| v.as_str()) == Some("pool-a")
                && item.get("accepted").and_then(|v| v.as_i64()) == Some(1)
                && item.get("skipped_due_to_cap").and_then(|v| v.as_i64()) == Some(1)
        ));
    assert!(summary
        .iter()
        .any(
            |item| item.get("provider").and_then(|v| v.as_str()) == Some("pool-b")
                && item.get("accepted").and_then(|v| v.as_i64()) == Some(1)
                && item.get("skipped_due_to_cap").and_then(|v| v.as_i64()) == Some(1)
        ));

    let scheduled: Vec<(String,)> = sqlx::query_as(r#"SELECT json_extract(input_json, '$.proxy_id') FROM tasks WHERE kind = 'verify_proxy' ORDER BY id ASC"#)
        .fetch_all(&state.db)
        .await
        .expect("load queued proxy ids");
    assert_eq!(scheduled.len(), 2);
    assert_ne!(scheduled[0].0, scheduled[1].0);
}

#[tokio::test]
async fn verify_batch_prioritizes_underrepresented_source_when_top1_is_concentrated() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-dominant', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1'),
           ('source-minority', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source registry");

    for idx in 0..8 {
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   source_label, last_verify_status, last_verify_at, created_at, updated_at
               ) VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'shared', 'US', 'pool-a', 'active', 0.9, 'source-dominant', NULL, NULL, ?, ?)"#,
        )
        .bind(format!("dominant-seed-{idx}"))
        .bind(8100 + idx)
        .bind(format!("{}", 10 + idx))
        .bind(format!("{}", 10 + idx))
        .execute(&state.db)
        .await
        .expect("insert dominant seed");
    }

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, last_verify_status, last_verify_at, created_at, updated_at
           ) VALUES
           ('minority-seed-0', 'http', '127.0.0.1', 8201, NULL, NULL, 'shared', 'US', 'pool-b', 'active', 0.9, 'source-minority', NULL, NULL, '20', '20'),
           ('verify-dominant-target', 'http', '127.0.0.1', 8202, NULL, NULL, 'shared', 'US', 'pool-a', 'active', 0.9, 'source-dominant', NULL, NULL, '1', '1'),
           ('verify-minority-target', 'http', '127.0.0.1', 8203, NULL, NULL, 'shared', 'US', 'pool-b', 'active', 0.9, 'source-minority', NULL, NULL, '2', '2')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert verify targets");

    let batch_payload = serde_json::json!({
        "region": "shared",
        "limit": 1,
        "only_stale": true
    });
    let (batch_status, batch_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(1));

    let scheduled_proxy_id: String = sqlx::query_scalar(
        r#"SELECT json_extract(input_json, '$.proxy_id') FROM tasks WHERE kind = 'verify_proxy' ORDER BY id DESC LIMIT 1"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load scheduled proxy id");
    assert_eq!(scheduled_proxy_id, "verify-minority-target");
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
            Request::builder()
                .method("POST")
                .uri("/proxies")
                .header("content-type", "application/json")
                .body(Body::from(proxy_payload.to_string()))
                .expect("request"),
        )
        .await;
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
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let batch_id = batch_json
        .get("batch_id")
        .and_then(|v| v.as_str())
        .expect("batch id")
        .to_string();

    let (_, list_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/verify-batch?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let items = list_json.as_array().expect("verify batch list");
    assert!(!items.is_empty());
    assert!(items
        .iter()
        .any(|item| item.get("id").and_then(|v| v.as_str()) == Some(batch_id.as_str())));

    let (_, detail_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/proxies/verify-batch/{batch_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        detail_json.get("id").and_then(|v| v.as_str()),
        Some(batch_id.as_str())
    );
    assert_eq!(
        detail_json.get("accepted_count").and_then(|v| v.as_i64()),
        Some(2)
    );
    let queued = detail_json
        .get("queued_count")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
    let running = detail_json
        .get("running_count")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
    let succeeded = detail_json
        .get("succeeded_count")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
    let failed = detail_json
        .get("failed_count")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
    assert_eq!(queued + running + succeeded + failed, 2);
    assert!(matches!(
        detail_json.get("status").and_then(|v| v.as_str()),
        Some("running") | Some("completed") | Some("scheduled")
    ));
    assert!(detail_json.get("provider_summary_json").is_some());
    assert!(detail_json.get("filters_json").is_some());
}

#[tokio::test]
async fn replenish_tick_global_prioritizes_underrepresented_source_candidates() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, _app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-dominant', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1'),
           ('source-minority', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source registry");

    for idx in 0..6 {
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   source_label, created_at, updated_at
               ) VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'shared', 'US', 'pool-a', 'active', 0.9, 'source-dominant', ?, ?)"#,
        )
        .bind(format!("dominant-active-{idx}"))
        .bind(8300 + idx)
        .bind(format!("{}", 100 + idx))
        .bind(format!("{}", 100 + idx))
        .execute(&state.db)
        .await
        .expect("insert dominant active");
    }

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, created_at, updated_at
           ) VALUES
           ('minority-active-0', 'http', '127.0.0.1', 8401, NULL, NULL, 'shared', 'US', 'pool-b', 'active', 0.9, 'source-minority', '200', '200'),
           ('candidate-dominant', 'http', '127.0.0.1', 8402, NULL, NULL, 'shared', 'US', 'pool-a', 'candidate', 0.7, 'source-dominant', '1', '1'),
           ('candidate-minority', 'http', '127.0.0.1', 8403, NULL, NULL, 'shared', 'US', 'pool-b', 'candidate', 0.7, 'source-minority', '2', '2')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert replenish candidates");

    let batches = run_proxy_replenish_mvp_tick(&state)
        .await
        .expect("run replenish tick");
    assert_eq!(batches.len(), 1);
    assert!(batches[0].accepted >= 1);
    assert_eq!(
        batches[0].proxy_ids.first().map(String::as_str),
        Some("candidate-minority")
    );
}

#[tokio::test]
async fn verify_batch_prioritizes_underrepresented_provider_when_source_balanced() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-shared', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert shared source registry");

    for idx in 0..6 {
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   source_label, last_verify_status, last_verify_at, created_at, updated_at
               ) VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'shared', 'US', 'pool-a', 'active', 0.9, 'source-shared', NULL, NULL, ?, ?)"#,
        )
        .bind(format!("provider-dominant-seed-{idx}"))
        .bind(8450 + idx)
        .bind(format!("{}", 10 + idx))
        .bind(format!("{}", 10 + idx))
        .execute(&state.db)
        .await
        .expect("insert dominant provider seed");
    }

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, last_verify_status, last_verify_at, created_at, updated_at
           ) VALUES
           ('provider-minority-seed-0', 'http', '127.0.0.1', 8521, NULL, NULL, 'shared', 'US', 'pool-b', 'active', 0.9, 'source-shared', NULL, NULL, '20', '20'),
           ('verify-provider-dominant', 'http', '127.0.0.1', 8522, NULL, NULL, 'shared', 'US', 'pool-a', 'active', 0.9, 'source-shared', NULL, NULL, '1', '1'),
           ('verify-provider-minority', 'http', '127.0.0.1', 8523, NULL, NULL, 'shared', 'US', 'pool-b', 'active', 0.9, 'source-shared', NULL, NULL, '2', '2')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert provider verify targets");

    let batch_payload = serde_json::json!({
        "region": "shared",
        "limit": 1,
        "only_stale": true
    });
    let (batch_status, batch_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(batch_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(batch_status, StatusCode::ACCEPTED);
    assert_eq!(batch_json.get("accepted").and_then(|v| v.as_i64()), Some(1));

    let scheduled_proxy_id: String = sqlx::query_scalar(
        r#"SELECT json_extract(input_json, '$.proxy_id') FROM tasks WHERE kind = 'verify_proxy' ORDER BY id DESC LIMIT 1"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load scheduled provider-prioritized proxy id");
    assert_eq!(scheduled_proxy_id, "verify-provider-minority");
}

#[tokio::test]
async fn replenish_tick_global_prioritizes_underrepresented_provider_candidates() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, _app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-shared', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert shared source registry");

    for idx in 0..6 {
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   source_label, created_at, updated_at
               ) VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'shared', 'US', 'pool-a', 'active', 0.9, 'source-shared', ?, ?)"#,
        )
        .bind(format!("provider-dominant-active-{idx}"))
        .bind(8530 + idx)
        .bind(format!("{}", 100 + idx))
        .bind(format!("{}", 100 + idx))
        .execute(&state.db)
        .await
        .expect("insert provider dominant active");
    }

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, created_at, updated_at
           ) VALUES
           ('provider-minority-active-0', 'http', '127.0.0.1', 8601, NULL, NULL, 'shared', 'US', 'pool-b', 'active', 0.9, 'source-shared', '200', '200'),
           ('candidate-provider-dominant', 'http', '127.0.0.1', 8602, NULL, NULL, 'shared', 'US', 'pool-a', 'candidate', 0.7, 'source-shared', '1', '1'),
           ('candidate-provider-minority', 'http', '127.0.0.1', 8603, NULL, NULL, 'shared', 'US', 'pool-b', 'candidate', 0.7, 'source-shared', '2', '2')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert provider replenish candidates");

    let batches = run_proxy_replenish_mvp_tick(&state)
        .await
        .expect("run replenish tick");
    assert_eq!(batches.len(), 1);
    assert!(batches[0].accepted >= 1);
    assert_eq!(
        batches[0].proxy_ids.first().map(String::as_str),
        Some("candidate-provider-minority")
    );
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
    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-fresh-verified")
    );
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
    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-ok-lower-score")
    );
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
    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-geo-match")
    );
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
    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-fresh-verified-2")
    );
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
    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-good-history")
    );
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
    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-good-provider")
    );
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
    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-older-failure")
    );
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
    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-stable-region")
    );
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

    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-verified-balanced-direct")
    );
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

    assert_eq!(
        selected.as_ref().map(|row| row.0.as_str()),
        Some("proxy-high-trust-direct")
    );
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
    let (_, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task_json.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-explain-best")
    );
    let selection_reason = task_json
        .get("selection_reason_summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(selection_reason.contains("trust score"));
    assert!(
        selection_reason.contains("wins on")
            || selection_reason.contains("penalized by")
            || selection_reason.contains("better on")
            || selection_reason.contains("worse on")
    );
    assert!(task_json
        .get("trust_score_total")
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(task_json.get("winner_vs_runner_up_diff").is_some());
    assert!(task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .map(|items| items
            .iter()
            .any(|item| item.get("title").and_then(|v| v.as_str())
                == Some("proxy selection decision")))
        .unwrap_or(false));
    let selection_artifact = task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("title").and_then(|v| v.as_str()) == Some("proxy selection decision")
            })
        })
        .expect("selection summary artifact");
    assert!(selection_artifact
        .get("summary")
        .and_then(|v| v.as_str())
        .map(|s| s.contains("this proxy stayed ahead by") && s.contains("biggest score drivers"))
        .unwrap_or(false));
    assert_eq!(
        selection_artifact.get("key").and_then(|v| v.as_str()),
        Some("proxy.selection.decision")
    );
    assert_eq!(
        selection_artifact.get("source").and_then(|v| v.as_str()),
        Some("selection.proxy")
    );
    assert_eq!(
        selection_artifact.get("severity").and_then(|v| v.as_str()),
        Some("info")
    );
    let identity_artifact = task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("title").and_then(|v| v.as_str()) == Some("identity and network summary")
            })
        })
        .expect("identity summary artifact");
    let identity_summary = identity_artifact
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        identity_summary.contains("proxy pool-x@us-east")
            || identity_summary.contains("proxy proxy-explain-best")
    );
    assert!(identity_summary.contains("proxy resolution resolved"));
    assert!(identity_summary.contains("selection summary"));
    assert!(!identity_summary.contains("pool is healthy for this request"));
    let growth_artifact = task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("title").and_then(|v| v.as_str()) == Some("proxy growth assessment")
            })
        })
        .expect("growth summary artifact");
    let growth_summary = growth_artifact
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        growth_summary.contains("pool is healthy for this request")
            || growth_summary.contains("pool needs replenishment for this request")
    );
    assert!(growth_summary.contains("target region "));
    assert!(growth_summary.contains("region fit"));
    assert!(!growth_summary.contains("biggest score drivers"));
    assert!(task_json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("winner_total_score"))
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(task_json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("runner_up_total_score"))
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(task_json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("score_gap"))
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(task_json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert!(task_json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
        .map(|v| v.len() <= 5)
        .unwrap_or(false));
    assert!(task_json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("label"))
        .and_then(|v| v.as_str())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert!(task_json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("direction"))
        .and_then(|v| v.as_str())
        .map(|v| matches!(v, "winner" | "runner_up" | "neutral"))
        .unwrap_or(false));
    if let Some(diff) = task_json.get("winner_vs_runner_up_diff") {
        let winner_total = diff
            .get("winner_total_score")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let runner_total = diff
            .get("runner_up_total_score")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let score_gap = diff
            .get("score_gap")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        assert_eq!(winner_total - runner_total, score_gap);
    }
    if let Some(factors) = task_json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
    {
        let deltas: Vec<i64> = factors
            .iter()
            .filter_map(|v| v.get("delta").and_then(|v| v.as_i64()).map(|d| d.abs()))
            .collect();
        assert!(deltas.windows(2).all(|w| w[0] >= w[1]));
        let labels: Vec<&str> = factors
            .iter()
            .filter_map(|v| v.get("label").and_then(|v| v.as_str()))
            .collect();
        assert!(labels.iter().all(|label| matches!(
            *label,
            "verify_ok"
                | "geo_match"
                | "geo_risk"
                | "upstream_ok"
                | "raw_score"
                | "missing_verify"
                | "stale_verify"
                | "verify_failed_heavy"
                | "verify_failed_light"
                | "verify_failed_base"
                | "history_risk"
                | "provider_risk"
                | "provider_region_risk"
                | "verify_confidence"
                | "verify_score_delta"
                | "verify_source"
                | "anonymity"
                | "probe_latency"
                | "verify_risk"
                | "soft_min_score"
                | "site_success"
                | "site_failure"
        )));
        assert!(!labels.iter().any(|label| matches!(
            *label,
            "geo_mismatch" | "region_mismatch" | "exit_ip_not_public" | "probe_error_category"
        )));
    }

    let result_json_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load result json");
    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result json");
    let policy = result_json
        .get("payload")
        .and_then(|v| v.get("network_policy_json"))
        .expect("policy");
    assert!(policy
        .get("trust_score_components")
        .and_then(|v| v.get("verify_ok_bonus"))
        .and_then(|v| v.as_i64())
        .is_some());
    let preview = policy
        .get("candidate_rank_preview")
        .and_then(|v| v.as_array())
        .expect("candidate preview");
    assert!(!preview.is_empty());
    assert_eq!(
        preview[0].get("id").and_then(|v| v.as_str()),
        Some("proxy-explain-best")
    );
    let preview_diff = preview[0]
        .get("winner_vs_runner_up_diff")
        .expect("preview diff");
    let task_diff = task_json
        .get("winner_vs_runner_up_diff")
        .expect("task diff");
    assert_eq!(preview_diff.get("score_gap"), task_diff.get("score_gap"));
    assert_eq!(
        preview_diff.get("winner_total_score"),
        task_diff.get("winner_total_score")
    );
    let summary = preview[0]
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(!summary.is_empty());
    assert!(
        summary.contains("wins on")
            || summary.contains("penalized by")
            || summary.contains("better on")
            || summary.contains("worse on")
    );
    assert!(
        summary.contains("verify_ok")
            || summary.contains("geo_match")
            || summary.contains("upstream_ok")
            || summary.contains("raw_score")
            || summary.contains("provider_risk")
            || summary.contains("provider_region_risk")
            || summary.contains("history_risk")
            || summary.contains("stale_verify")
            || summary.contains("missing_verify")
            || summary.contains("geo_risk")
            || summary.contains("verify_risk")
    );
    assert!(!summary.contains("verify_ok_bonus"));
    assert!(!summary.contains("provider_region_cluster_penalty"));
    assert!(!summary.contains("geo_mismatch"));
    assert!(!summary.contains("region_mismatch"));
    assert!(!summary.contains("exit_ip_not_public"));
    assert!(!summary.contains("probe_error_category"));
}

#[tokio::test]
async fn task_and_run_views_expose_browser_failure_signal_fields() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = "task-browser-failure-fields";
    let run_id = "run-browser-failure-fields";
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'open_page', 'failed', '{"url":"https://example.com/browser-failure"}', NULL, NULL, 0, '1', '1', '2', '3', 'runner-browser-failure', '2', ?, 'navigation failed')"#,
    )
    .bind(task_id)
    .bind(serde_json::json!({
        "status": "failed",
        "error_kind": "runner_non_zero_exit",
        "failure_scope": "browser_execution",
        "browser_failure_signal": "browser_navigation_failure_signal",
        "execution_stage": "navigate",
        "summary_artifacts": [{
            "key": "open_page.execution",
            "source": "runner.lightpanda",
            "category": "execution",
            "severity": "error",
            "title": "open_page failed",
            "summary": "failure_scope=browser_execution browser_failure_signal=browser_navigation_failure_signal"
        }]
    }).to_string())
    .execute(&state.db)
    .await
    .expect("insert task");

    sqlx::query(
        r#"INSERT INTO runs (id, task_id, status, attempt, runner_kind, result_json, error_message, started_at, finished_at)
           VALUES (?, ?, 'failed', 1, 'lightpanda', ?, 'navigation failed', '2', '3')"#,
    )
    .bind(run_id)
    .bind(task_id)
    .bind(serde_json::json!({
        "status": "failed",
        "error_kind": "runner_non_zero_exit",
        "failure_scope": "browser_execution",
        "browser_failure_signal": "browser_navigation_failure_signal",
        "execution_stage": "navigate",
        "summary_artifacts": [{
            "key": "open_page.execution",
            "source": "runner.lightpanda",
            "category": "execution",
            "severity": "error",
            "title": "open_page failed",
            "summary": "failure_scope=browser_execution browser_failure_signal=browser_navigation_failure_signal"
        }]
    }).to_string())
    .execute(&state.db)
    .await
    .expect("insert run");

    let (_, task_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        task_json.get("failure_scope").and_then(|v| v.as_str()),
        Some("browser_execution")
    );
    assert_eq!(
        task_json
            .get("browser_failure_signal")
            .and_then(|v| v.as_str()),
        Some("browser_navigation_failure_signal")
    );
    assert_eq!(
        task_json
            .get("summary_artifacts")
            .and_then(|v| v.as_array())
            .and_then(|items| items
                .iter()
                .find(|item| item.get("title").and_then(|v| v.as_str())
                    == Some("browser failure summary")))
            .and_then(|item| item.get("summary"))
            .and_then(|v| v.as_str())
            .map(|s| s.contains("execution_stage=navigate")),
        Some(true)
    );

    let (_, runs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/runs"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let runs = runs_json.as_array().expect("runs array");
    assert_eq!(
        runs[0].get("failure_scope").and_then(|v| v.as_str()),
        Some("browser_execution")
    );
    assert_eq!(
        runs[0]
            .get("browser_failure_signal")
            .and_then(|v| v.as_str()),
        Some("browser_navigation_failure_signal")
    );
    assert_eq!(
        runs[0]
            .get("summary_artifacts")
            .and_then(|v| v.as_array())
            .and_then(|items| items
                .iter()
                .find(|item| item.get("title").and_then(|v| v.as_str())
                    == Some("browser failure summary")))
            .and_then(|item| item.get("summary"))
            .and_then(|v| v.as_str())
            .map(|s| s.contains("execution_stage=navigate")),
        Some(true)
    );
}

#[tokio::test]
async fn status_latest_execution_summaries_include_browser_failure_artifact() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES ('task-status-browser-failure', 'open_page', 'failed', '{"url":"https://example.com/status-browser-failure"}', NULL, NULL, 0, '1', '1', '2', '3', 'runner-browser-failure', '2', ?, 'navigation failed')"#,
    )
    .bind(serde_json::json!({
        "status": "failed",
        "error_kind": "runner_non_zero_exit",
        "failure_scope": "browser_execution",
        "browser_failure_signal": "browser_navigation_failure_signal",
        "execution_stage": "navigate",
        "summary_artifacts": [{
            "key": "open_page.execution",
            "source": "runner.lightpanda",
            "category": "execution",
            "severity": "error",
            "title": "open_page failed",
            "summary": "failure_scope=browser_execution browser_failure_signal=browser_navigation_failure_signal execution_stage=navigate"
        }]
    }).to_string())
    .execute(&state.db)
    .await
    .expect("insert task");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let latest = json
        .get("latest_execution_summaries")
        .and_then(|v| v.as_array())
        .expect("latest_execution_summaries");
    let browser_failure = latest
        .iter()
        .find(|item| item.get("title").and_then(|v| v.as_str()) == Some("browser failure summary"))
        .expect("browser failure summary artifact");
    let failure_summary = browser_failure
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(failure_summary.contains("failure_scope=browser_execution"));
    assert!(failure_summary.contains("browser_failure_signal=browser_navigation_failure_signal"));
    assert!(failure_summary.contains("execution_stage=navigate"));
    assert_eq!(
        browser_failure.get("severity").and_then(|v| v.as_str()),
        Some("error")
    );
}

#[tokio::test]
async fn status_latest_execution_summaries_include_browser_failure_and_selection_artifacts_when_available(
) {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-status-best', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-s', 'active', 0.74, 7, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-status-second', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-s', 'active', 0.68, 5, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 0, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/status-summary",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-s", "region": "us-east"}
    });
    let (_, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;
    if task_json.get("winner_vs_runner_up_diff").is_none() {
        return;
    }

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let latest = json
        .get("latest_execution_summaries")
        .and_then(|v| v.as_array())
        .expect("latest_execution_summaries");
    let latest_tasks = json
        .get("latest_tasks")
        .and_then(|v| v.as_array())
        .expect("latest_tasks");
    let latest_browser_tasks = json
        .get("latest_browser_tasks")
        .and_then(|v| v.as_array())
        .expect("latest_browser_tasks");
    assert!(!latest_tasks.is_empty());
    assert!(!latest_browser_tasks.is_empty());
    if let Some(selection) = latest
        .iter()
        .find(|item| item.get("title").and_then(|v| v.as_str()) == Some("proxy selection decision"))
    {
        let summary = selection
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(summary.contains("this proxy stayed ahead by"));
        assert!(summary.contains("biggest score drivers"));
        assert_eq!(
            selection.get("key").and_then(|v| v.as_str()),
            Some("proxy.selection.decision")
        );
        assert_eq!(
            selection.get("source").and_then(|v| v.as_str()),
            Some("selection.proxy")
        );
        assert_eq!(
            selection.get("severity").and_then(|v| v.as_str()),
            Some("info")
        );
        assert_eq!(
            selection.get("task_id").and_then(|v| v.as_str()),
            Some(task_id.as_str())
        );
        assert_eq!(
            selection.get("task_kind").and_then(|v| v.as_str()),
            Some("open_page")
        );
        assert_eq!(
            selection.get("task_status").and_then(|v| v.as_str()),
            Some("succeeded")
        );
    }
}

#[tokio::test]
async fn status_latest_execution_summaries_deduplicate_repeated_high_level_artifacts_across_tasks()
{
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-dedupe-best', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-d', 'active', 0.74, 7, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-dedupe-second', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-d', 'active', 0.68, 5, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 0, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");

    for suffix in ["a", "b"] {
        let payload = serde_json::json!({
            "kind": "open_page",
            "url": format!("https://example.com/status-dedupe-{suffix}"),
            "timeout_seconds": 5,
            "network_policy_json": {"mode": "required_proxy", "provider": "pool-d", "region": "us-east"}
        });
        let (_, create_json) = json_response(
            &app,
            Request::builder()
                .method("POST")
                .uri("/tasks")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request"),
        )
        .await;
        let task_id = create_json
            .get("id")
            .and_then(|v| v.as_str())
            .expect("task id")
            .to_string();
        let _ = wait_for_terminal_status(&app, &task_id).await;
    }

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let latest = json
        .get("latest_execution_summaries")
        .and_then(|v| v.as_array())
        .expect("latest_execution_summaries");
    assert_eq!(
        latest
            .iter()
            .filter(
                |item| item.get("key").and_then(|v| v.as_str()) == Some("proxy.selection.decision")
            )
            .count(),
        1
    );
    assert_eq!(
        latest
            .iter()
            .filter(
                |item| item.get("key").and_then(|v| v.as_str()) == Some("identity.network.summary")
            )
            .count(),
        1
    );
    assert_eq!(
        latest
            .iter()
            .filter(|item| item.get("title").and_then(|v| v.as_str())
                == Some("proxy growth assessment"))
            .count(),
        1
    );
    assert!(latest.len() <= 5);
}

#[tokio::test]
async fn status_latest_execution_summaries_prioritize_error_over_info() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-priority-best', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-p', 'active', 0.74, 7, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-priority-second', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-p', 'active', 0.68, 5, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 0, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");

    let ok_payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/summary-priority-ok",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-p", "region": "us-east"}
    });
    let (_, ok_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(ok_payload.to_string()))
            .expect("request"),
    )
    .await;
    let ok_task_id = ok_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("ok task id")
        .to_string();
    let _ = wait_for_terminal_status(&app, &ok_task_id).await;

    let fail_payload = serde_json::json!({
        "kind": "verify_proxy",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-p", "region": "us-east"}
    });
    let (_, fail_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(fail_payload.to_string()))
            .expect("request"),
    )
    .await;
    let fail_task_id = fail_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("fail task id")
        .to_string();
    let _ = wait_for_terminal_status(&app, &fail_task_id).await;

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let latest = json
        .get("latest_execution_summaries")
        .and_then(|v| v.as_array())
        .expect("latest_execution_summaries");
    assert!(!latest.is_empty());
    assert_eq!(
        latest[0].get("severity").and_then(|v| v.as_str()),
        Some("error")
    );
    assert_eq!(
        latest[0].get("task_id").and_then(|v| v.as_str()),
        Some(fail_task_id.as_str())
    );
    assert_eq!(
        latest[0].get("key").and_then(|v| v.as_str()),
        Some("verify_proxy.execution")
    );
    let failure_summary = latest[0]
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(failure_summary.contains("kind=verify_proxy"));
}

#[tokio::test]
async fn status_tracks_browser_ready_tasks_separately_from_latest_tasks() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES
           ('task-status-browser-visible', 'get_title', 'succeeded', '{}', NULL, NULL, 0, '3', '3', '3', '3', NULL, NULL, '{"title":"Visible title","final_url":"https://example.com/visible"}', NULL),
           ('task-status-generic-newer', 'verify_proxy', 'failed', '{}', NULL, NULL, 0, '4', '4', '4', '4', NULL, NULL, '{"error_kind":"timeout"}', 'boom')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert mixed tasks");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let latest_tasks = json
        .get("latest_tasks")
        .and_then(|v| v.as_array())
        .expect("latest tasks");
    let latest_browser_tasks = json
        .get("latest_browser_tasks")
        .and_then(|v| v.as_array())
        .expect("latest browser tasks");
    assert!(latest_tasks
        .iter()
        .any(|task| task.get("id").and_then(|v| v.as_str()) == Some("task-status-generic-newer")));
    assert!(
        latest_browser_tasks
            .iter()
            .any(|task| task.get("id").and_then(|v| v.as_str())
                == Some("task-status-browser-visible"))
    );
    assert!(!latest_browser_tasks
        .iter()
        .any(|task| task.get("id").and_then(|v| v.as_str()) == Some("task-status-generic-newer")));
}

#[tokio::test]
async fn status_browser_ready_tasks_prioritize_content_ready_and_readability() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES
           ('task-browser-order-title', 'get_title', 'succeeded', '{}', NULL, NULL, 0, '3', '3', '3', '3', NULL, NULL, '{"title":"Readable title","final_url":"https://example.com/title"}', NULL),
           ('task-browser-order-ready', 'extract_text', 'succeeded', '{}', NULL, NULL, 0, '4', '4', '4', '4', NULL, NULL, '{"final_url":"https://example.com/text","content_preview":"hello world","content_kind":"text/plain","content_length":11,"content_ready":true}', NULL),
           ('task-browser-order-weak', 'get_final_url', 'succeeded', '{}', NULL, NULL, 0, '5', '5', '5', '5', NULL, NULL, '{"final_url":"https://example.com/weak"}', NULL)"#,
    )
    .execute(&state.db)
    .await
    .expect("insert ordered tasks");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let latest_browser_tasks = json
        .get("latest_browser_tasks")
        .and_then(|v| v.as_array())
        .expect("latest browser tasks");
    assert_eq!(
        latest_browser_tasks
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str()),
        Some("task-browser-order-ready")
    );
    assert_eq!(
        latest_browser_tasks
            .get(1)
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str()),
        Some("task-browser-order-title")
    );
}

#[tokio::test]
async fn status_browser_ready_tasks_prefers_recent_readable_title_when_content_ready_is_absent() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES
           ('task-browser-readable-older', 'get_title', 'succeeded', '{}', NULL, NULL, 0, '3', '3', '3', '3', NULL, NULL, '{"title":"Readable older title","final_url":"https://example.com/older"}', NULL),
           ('task-browser-final-newer', 'get_final_url', 'succeeded', '{}', NULL, NULL, 0, '4', '4', '4', '4', NULL, NULL, '{"final_url":"https://example.com/newer"}', NULL)"#,
    )
    .execute(&state.db)
    .await
    .expect("insert readable ordering tasks");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    let latest_browser_tasks = json
        .get("latest_browser_tasks")
        .and_then(|v| v.as_array())
        .expect("latest browser tasks");
    assert_eq!(
        latest_browser_tasks
            .first()
            .and_then(|v| v.get("id"))
            .and_then(|v| v.as_str()),
        Some("task-browser-readable-older")
    );
}

#[tokio::test]
async fn verify_migration_columns_are_added_for_old_proxy_table() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db first");
    drop(db);

    let path = db_url.strip_prefix("sqlite://").expect("sqlite path");
    let old_db = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("connect old db");
    sqlx::query("DROP TABLE proxies")
        .execute(&old_db)
        .await
        .expect("drop proxies");
    sqlx::query(
        r#"CREATE TABLE proxies (
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
    )"#,
    )
    .execute(&old_db)
    .await
    .expect("create old proxies");
    drop(old_db);

    let db2 = init_db(&format!("sqlite://{path}"))
        .await
        .expect("re-init db");
    let cols: Vec<(i64, String, String, i64, Option<String>, i64)> =
        sqlx::query_as("PRAGMA table_info(proxies)")
            .fetch_all(&db2)
            .await
            .expect("pragma table info");
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
    let (_, ok_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(ok_payload.to_string()))
            .expect("request"),
    )
    .await;
    let ok_id = ok_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let _ = wait_for_terminal_status(&app, &ok_id).await;
    let score_after_success: f64 =
        sqlx::query_scalar("SELECT score FROM proxies WHERE id = 'proxy-feedback-1'")
            .fetch_one(&state.db)
            .await
            .expect("score after success");
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
        Request::builder()
            .uri("/proxies/proxy-explain-single/explain")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let diff = json.get("winner_vs_runner_up_diff").expect("winner diff");
    assert_eq!(
        diff.get("runner_up_total_score").and_then(|v| v.as_i64()),
        diff.get("winner_total_score").and_then(|v| v.as_i64())
    );
    assert_eq!(diff.get("score_gap").and_then(|v| v.as_i64()), Some(0));
    if let Some(factors) = diff.get("factors").and_then(|v| v.as_array()) {
        let labels: Vec<&str> = factors
            .iter()
            .filter_map(|v| v.get("label").and_then(|v| v.as_str()))
            .collect();
        assert!(labels.iter().all(|label| matches!(
            *label,
            "verify_ok"
                | "geo_match"
                | "geo_risk"
                | "upstream_ok"
                | "raw_score"
                | "missing_verify"
                | "stale_verify"
                | "verify_failed_heavy"
                | "verify_failed_light"
                | "verify_failed_base"
                | "history_risk"
                | "provider_risk"
                | "provider_region_risk"
                | "verify_confidence"
                | "verify_score_delta"
                | "verify_source"
                | "anonymity"
                | "probe_latency"
                | "verify_risk"
                | "soft_min_score"
        )));
        assert!(!labels.iter().any(|label| matches!(
            *label,
            "geo_mismatch" | "region_mismatch" | "exit_ip_not_public" | "probe_error_category"
        )));
        let directions: Vec<&str> = factors
            .iter()
            .filter_map(|v| v.get("direction").and_then(|v| v.as_str()))
            .collect();
        assert!(directions.iter().all(|d| *d == "neutral"));
    }
}

#[tokio::test]
async fn proxy_explain_endpoint_exposes_provider_risk_version_visibility_fields() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, created_at, updated_at)
                  VALUES
                  ('proxy-explain-version', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-version-fields', 'active', 0.9, 5, 0, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    persona_pilot::db::init::refresh_provider_risk_snapshots(&state.db)
        .await
        .expect("refresh snapshots");
    persona_pilot::db::init::refresh_cached_trust_scores(&state.db)
        .await
        .expect("refresh caches");
    sqlx::query("UPDATE provider_risk_snapshots SET version = version + 1 WHERE provider = 'pool-version-fields'")
        .execute(&state.db)
        .await
        .expect("bump version");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("GET")
            .uri("/proxies/proxy-explain-version/explain")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.get("provider_risk_version_status")
            .and_then(|v| v.as_str()),
        Some("aligned")
    );
    assert!(json
        .get("provider_risk_version_current")
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(json
        .get("provider_risk_version_seen")
        .and_then(|v| v.as_i64())
        .is_some());
}

#[tokio::test]
async fn proxy_explain_endpoint_with_higher_candidate_count_still_returns_preview() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let mut values = Vec::new();
    for i in 0..12 {
        let id = format!("proxy-explain-bulk-{}", i);
        let score = 0.90 - (i as f64 * 0.02);
        let success = 10 - (i.min(5) as i64);
        let verify_geo = if i % 3 == 0 { 0 } else { 1 };
        values.push(format!(
            "('{}', 'http', '127.0.0.1', {}, NULL, NULL, 'us-east', 'US', 'pool-bulk', 'active', {:.2}, {}, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', {}, 'US', 'Virginia', '9999999999', '1', '1')",
            id,
            8100 + i,
            score,
            success,
            verify_geo
        ));
    }
    let sql = format!(
        r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES {}"#,
        values.join(",\n")
    );
    sqlx::query(&sql)
        .execute(&state.db)
        .await
        .expect("insert proxies");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-explain-bulk-0/explain")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let preview = json
        .get("candidate_rank_preview")
        .and_then(|v| v.as_array())
        .expect("candidate_rank_preview");
    assert!(!preview.is_empty());
    assert!(preview.len() <= 5);
    let preview_ids: Vec<_> = preview
        .iter()
        .filter_map(|item| item.get("id").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(
        preview_ids,
        vec![
            "proxy-explain-bulk-1",
            "proxy-explain-bulk-2",
            "proxy-explain-bulk-4"
        ]
    );
}

#[tokio::test]
async fn auto_selection_prefers_stronger_verify_source_signal() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, last_verify_source, created_at, updated_at)
                  VALUES
                  ('proxy-source-local', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-source', 'active', 0.70, 8, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', 'local_verify', '1', '1'),
                  ('proxy-source-runner', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-source', 'active', 0.70, 8, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', 'runner_verify', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/source-rank",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-source", "region": "us-east"}
    });
    let (_, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;

    assert_eq!(
        task_json.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-source-local")
    );
    let diff = task_json
        .get("winner_vs_runner_up_diff")
        .expect("winner diff");
    let score_gap = diff
        .get("score_gap")
        .and_then(|v| v.as_i64())
        .unwrap_or_default();
    assert_eq!(score_gap, 1);
    let factors = diff
        .get("factors")
        .and_then(|v| v.as_array())
        .expect("factors");
    assert!(factors
        .iter()
        .any(
            |f| f.get("label").and_then(|v| v.as_str()) == Some("verify_source")
                && f.get("delta").and_then(|v| v.as_i64()) == Some(1)
        ));

    let result_json_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load result json");
    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result json");
    let preview = result_json
        .get("payload")
        .and_then(|v| v.get("network_policy_json"))
        .and_then(|v| v.get("candidate_rank_preview"))
        .and_then(|v| v.as_array())
        .expect("candidate preview");
    assert_eq!(
        preview[0].get("id").and_then(|v| v.as_str()),
        Some("proxy-source-local")
    );
    let summary = preview[0]
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        summary.contains("verify_source")
            || summary.contains("wins on verify_source")
            || summary.contains("better on verify_source")
    );
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
        Request::builder()
            .uri("/proxies/proxy-explain-endpoint/explain")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-explain-endpoint")
    );
    assert!(json
        .get("trust_score_components")
        .and_then(|v| v.get("verify_ok_bonus"))
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(json
        .get("candidate_rank_preview")
        .and_then(|v| v.as_array())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert!(json.get("winner_vs_runner_up_diff").is_some());
    assert!(json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("winner_total_score"))
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("runner_up_total_score"))
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("score_gap"))
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert!(json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
        .map(|v| v.len() <= 5)
        .unwrap_or(false));
    assert!(json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("label"))
        .and_then(|v| v.as_str())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert!(json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.get("direction"))
        .and_then(|v| v.as_str())
        .map(|v| matches!(v, "winner" | "runner_up" | "neutral"))
        .unwrap_or(false));
    if let Some(diff) = json.get("winner_vs_runner_up_diff") {
        let winner_total = diff
            .get("winner_total_score")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let runner_total = diff
            .get("runner_up_total_score")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        let score_gap = diff
            .get("score_gap")
            .and_then(|v| v.as_i64())
            .unwrap_or_default();
        assert_eq!(winner_total - runner_total, score_gap);
    }
    if let Some(factors) = json
        .get("winner_vs_runner_up_diff")
        .and_then(|v| v.get("factors"))
        .and_then(|v| v.as_array())
    {
        let deltas: Vec<i64> = factors
            .iter()
            .filter_map(|v| v.get("delta").and_then(|v| v.as_i64()).map(|d| d.abs()))
            .collect();
        assert!(deltas.windows(2).all(|w| w[0] >= w[1]));
        let labels: Vec<&str> = factors
            .iter()
            .filter_map(|v| v.get("label").and_then(|v| v.as_str()))
            .collect();
        assert!(labels.iter().all(|label| matches!(
            *label,
            "verify_ok"
                | "geo_match"
                | "geo_risk"
                | "upstream_ok"
                | "raw_score"
                | "missing_verify"
                | "stale_verify"
                | "verify_failed_heavy"
                | "verify_failed_light"
                | "verify_failed_base"
                | "history_risk"
                | "provider_risk"
                | "provider_region_risk"
                | "verify_confidence"
                | "verify_score_delta"
                | "verify_source"
                | "anonymity"
                | "probe_latency"
                | "verify_risk"
                | "soft_min_score"
        )));
        assert!(!labels.iter().any(|label| matches!(
            *label,
            "geo_mismatch" | "region_mismatch" | "exit_ip_not_public" | "probe_error_category"
        )));
    }
}

#[tokio::test]
async fn verify_probe_updates_proxy_score_via_score_delta() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 1024];
            let _ = socket.read(&mut buf).await;
            let response = b"HTTP/1.1 200 Connection Established
ip=9.9.9.9
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

    let before: f64 =
        sqlx::query_scalar("SELECT score FROM proxies WHERE id = 'proxy-verify-score'")
            .fetch_one(&state.db)
            .await
            .expect("before score");
    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-score/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("ok"));
    let after: f64 =
        sqlx::query_scalar("SELECT score FROM proxies WHERE id = 'proxy-verify-score'")
            .fetch_one(&state.db)
            .await
            .expect("after score");
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

    persona_pilot::db::init::refresh_provider_risk_snapshots(&db)
        .await
        .expect("refresh snapshots");
    let risk_hit: i64 = sqlx::query_scalar(
        "SELECT risk_hit FROM provider_risk_snapshots WHERE provider = 'pool-risk'",
    )
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

    persona_pilot::db::init::refresh_provider_risk_snapshots(&db)
        .await
        .expect("refresh snapshots");
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

    persona_pilot::db::init::refresh_provider_risk_snapshots(&db)
        .await
        .expect("refresh all");
    sqlx::query("UPDATE proxies SET failure_count = 20 WHERE provider = 'pool-b'")
        .execute(&db)
        .await
        .expect("update pool-b");
    persona_pilot::db::init::refresh_provider_risk_snapshot_for_provider(&db, Some("pool-b"))
        .await
        .expect("refresh pool-b only");

    let pool_a: i64 = sqlx::query_scalar(
        "SELECT risk_hit FROM provider_risk_snapshots WHERE provider = 'pool-a'",
    )
    .fetch_one(&db)
    .await
    .expect("pool-a");
    let pool_b: i64 = sqlx::query_scalar(
        "SELECT risk_hit FROM provider_risk_snapshots WHERE provider = 'pool-b'",
    )
    .fetch_one(&db)
    .await
    .expect("pool-b");
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

    persona_pilot::db::init::refresh_provider_risk_snapshots(&db)
        .await
        .expect("refresh risk snapshots");
    persona_pilot::db::init::refresh_cached_trust_scores(&db)
        .await
        .expect("refresh trust score cache");
    let cached: i64 =
        sqlx::query_scalar("SELECT cached_trust_score FROM proxies WHERE id = 'proxy-cache-1'")
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
    persona_pilot::db::init::refresh_provider_risk_snapshots(&state.db)
        .await
        .expect("refresh provider risk snapshots");
    persona_pilot::db::init::refresh_cached_trust_scores(&state.db)
        .await
        .expect("refresh cached trust scores");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/cached-order",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-cached", "region": "us-east"}
    });
    let (_, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task_json.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-cache-order-high")
    );
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

    persona_pilot::db::init::refresh_provider_risk_snapshots(&db)
        .await
        .expect("refresh risk snapshots");
    persona_pilot::db::init::refresh_cached_trust_scores_for_provider(&db, Some("pool-scope"))
        .await
        .expect("refresh cached trust by provider");
    let cached_one: i64 = sqlx::query_scalar(
        "SELECT COALESCE(cached_trust_score, 0) FROM proxies WHERE id = 'proxy-scope-1'",
    )
    .fetch_one(&db)
    .await
    .expect("cache 1");
    let cached_two: i64 = sqlx::query_scalar(
        "SELECT COALESCE(cached_trust_score, 0) FROM proxies WHERE id = 'proxy-scope-2'",
    )
    .fetch_one(&db)
    .await
    .expect("cache 2");
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
    persona_pilot::db::init::refresh_provider_risk_snapshots(&state.db)
        .await
        .expect("refresh risk snapshots");
    persona_pilot::db::init::refresh_cached_trust_scores(&state.db)
        .await
        .expect("refresh trust cache");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-cache-check/trust-cache-check")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-cache-check")
    );
    assert!(json
        .get("cached_trust_score")
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(json
        .get("recomputed_trust_score")
        .and_then(|v| v.as_i64())
        .is_some());
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
    persona_pilot::db::init::refresh_provider_risk_snapshots(&state.db)
        .await
        .expect("refresh risk snapshots");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-cache-repair/trust-cache-repair")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-cache-repair")
    );
    assert_eq!(json.get("repaired").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(json.get("in_sync").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(json.get("delta").and_then(|v| v.as_i64()), Some(0));
    assert!(
        json.get("cached_trust_score")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)
            > 0
    );
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
    persona_pilot::db::init::refresh_provider_risk_snapshots(&state.db)
        .await
        .expect("refresh risk snapshots");

    let (scan_status, scan_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/trust-cache-scan")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(scan_status, StatusCode::OK);
    assert_eq!(scan_json.get("total").and_then(|v| v.as_u64()), Some(2));
    assert!(
        scan_json
            .get("drifted")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= 1
    );

    let (repair_status, repair_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/trust-cache-repair-batch")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(repair_status, StatusCode::OK);
    assert_eq!(repair_json.get("scanned").and_then(|v| v.as_u64()), Some(2));
    assert!(
        repair_json
            .get("repaired")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= 1
    );
    assert_eq!(
        repair_json
            .get("remaining_drifted")
            .and_then(|v| v.as_u64()),
        Some(0)
    );
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
    persona_pilot::db::init::refresh_provider_risk_snapshots(&state.db)
        .await
        .expect("refresh risk snapshots");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/trust-cache-maintenance")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("scanned_before").and_then(|v| v.as_u64()), Some(2));
    assert!(
        json.get("drifted_before")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            >= 1
    );
    assert!(json.get("repaired").and_then(|v| v.as_u64()).unwrap_or(0) >= 1);
    assert_eq!(
        json.get("remaining_drifted").and_then(|v| v.as_u64()),
        Some(0)
    );
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
    persona_pilot::db::init::refresh_provider_risk_snapshots(&state.db)
        .await
        .expect("refresh risk snapshots");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/trust-cache-scan?only_drifted=true&limit=1&provider=poolflt")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json.get("total").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(json.get("drifted").and_then(|v| v.as_u64()), Some(1));
}

#[tokio::test]
async fn task_runs_expose_run_level_trace_metadata_and_standardized_artifacts() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-run-trace-best', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-run-trace', 'active', 0.74, 7, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-run-trace-second', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-run-trace', 'active', 0.68, 5, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 0, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");

    let profile_payload = serde_json::json!({
        "id": "fp-run-trace",
        "name": "Run Trace Profile",
        "profile_json": {
            "timezone": "Asia/Shanghai",
            "locale": "zh-CN",
            "unsupported_blob": {"k": "v"}
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
        "kind": "get_title",
        "url": "https://example.com/run-trace",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-run-trace",
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-run-trace", "region": "us-east"}
    });
    let (_, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;

    let (status, runs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/runs"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let runs = runs_json.as_array().expect("runs array");
    assert!(!runs.is_empty());
    let run = &runs[0];
    let run_id = run.get("id").and_then(|v| v.as_str()).expect("run id");
    let attempt = run
        .get("attempt")
        .and_then(|v| v.as_i64())
        .expect("attempt");
    let artifacts = run
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("artifacts");
    assert!(!artifacts.is_empty());
    assert!(artifacts
        .iter()
        .all(|item| item.get("run_id").and_then(|v| v.as_str()) == Some(run_id)));
    assert!(artifacts
        .iter()
        .all(|item| item.get("attempt").and_then(|v| v.as_i64()) == Some(attempt)));
    assert!(artifacts.iter().all(|item| item
        .get("timestamp")
        .and_then(|v| v.as_str())
        .map(|v| !v.is_empty())
        .unwrap_or(false)));
    assert!(artifacts.iter().all(|item| item
        .get("source")
        .and_then(|v| v.as_str())
        .map(|v| v.starts_with("runner.") || v.starts_with("selection."))
        .unwrap_or(false)));
    assert!(artifacts.iter().all(|item| matches!(
        item.get("severity").and_then(|v| v.as_str()),
        Some("info") | Some("warning") | Some("error")
    )));
    assert!(artifacts.iter().all(|item| matches!(
        item.get("category").and_then(|v| v.as_str()),
        Some("execution") | Some("summary") | Some("result") | Some("debug") | Some("transient")
    )));
    assert_eq!(
        run.get("proxy_id").and_then(|v| v.as_str()),
        task_json.get("proxy_id").and_then(|v| v.as_str())
    );
    assert_eq!(
        run.get("proxy_provider").and_then(|v| v.as_str()),
        task_json.get("proxy_provider").and_then(|v| v.as_str())
    );
    assert_eq!(
        run.get("proxy_region").and_then(|v| v.as_str()),
        task_json.get("proxy_region").and_then(|v| v.as_str())
    );
    assert_eq!(
        run.get("proxy_resolution_status").and_then(|v| v.as_str()),
        task_json
            .get("proxy_resolution_status")
            .and_then(|v| v.as_str())
    );
    assert_eq!(
        run.get("trust_score_total").and_then(|v| v.as_i64()),
        task_json.get("trust_score_total").and_then(|v| v.as_i64())
    );
    assert_eq!(
        run.get("selection_reason_summary").and_then(|v| v.as_str()),
        task_json
            .get("selection_reason_summary")
            .and_then(|v| v.as_str())
    );
    assert_eq!(
        run.get("selection_explain"),
        task_json.get("selection_explain")
    );
    assert_eq!(
        run.get("fingerprint_runtime_explain"),
        task_json.get("fingerprint_runtime_explain")
    );
    assert_eq!(
        run.get("execution_identity"),
        task_json.get("execution_identity")
    );
    assert_eq!(
        run.get("identity_network_explain"),
        task_json.get("identity_network_explain")
    );
    assert_eq!(
        run.get("winner_vs_runner_up_diff"),
        task_json.get("winner_vs_runner_up_diff")
    );
    assert_eq!(
        run.get("fingerprint_runtime_explain")
            .and_then(|v| v.get("consumption_explain"))
            .and_then(|v| v.get("consumption_status"))
            .and_then(|v| v.as_str()),
        Some("partially_consumed")
    );
    assert_eq!(
        task_json
            .get("fingerprint_runtime_explain")
            .and_then(|v| v.get("consumption_explain"))
            .and_then(|v| v.get("ignored_count"))
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        task_json.get("title").and_then(|v| v.as_str()),
        Some("Fake title for https://example.com/run-trace")
    );
    assert_eq!(
        task_json.get("final_url").and_then(|v| v.as_str()),
        Some("https://example.com/run-trace#final")
    );
    assert_eq!(task_json.get("content_kind").and_then(|v| v.as_str()), None);
    assert_eq!(
        run.get("title").and_then(|v| v.as_str()),
        task_json.get("title").and_then(|v| v.as_str())
    );
    assert_eq!(
        run.get("final_url").and_then(|v| v.as_str()),
        task_json.get("final_url").and_then(|v| v.as_str())
    );
    assert_eq!(run.get("content_preview"), task_json.get("content_preview"));
    assert_eq!(run.get("content_length"), task_json.get("content_length"));
    assert_eq!(
        run.get("content_truncated"),
        task_json.get("content_truncated")
    );
    assert_eq!(run.get("content_kind"), task_json.get("content_kind"));
    assert_eq!(
        run.get("content_source_action"),
        task_json.get("content_source_action")
    );
    assert_eq!(run.get("content_ready"), task_json.get("content_ready"));

    let task_artifacts = task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("task artifacts");
    let selection_artifact = task_artifacts
        .iter()
        .find(|item| item.get("key").and_then(|v| v.as_str()) == Some("proxy.selection.decision"))
        .expect("selection artifact");
    assert_eq!(
        selection_artifact.get("source").and_then(|v| v.as_str()),
        Some("selection.proxy")
    );
    let identity_artifact = task_artifacts
        .iter()
        .find(|item| {
            item.get("title").and_then(|v| v.as_str()) == Some("identity and network summary")
        })
        .expect("identity summary artifact");
    assert!(identity_artifact
        .get("summary")
        .and_then(|v| v.as_str())
        .map(|s| s.contains("fingerprint consumption partially_consumed"))
        .unwrap_or(false));
}

#[tokio::test]
async fn proxy_explain_endpoint_exposes_trace_metadata_fields() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, cached_trust_score, trust_score_cached_at, created_at, updated_at)
                  VALUES ('proxy-explain-trace', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-trace', 'active', 0.77, 5, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', 57, '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-explain-trace/explain")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        json.get("explain_source").and_then(|v| v.as_str()),
        Some("proxy_trust_cache+candidate_preview")
    );
    assert!(json
        .get("explain_generated_at")
        .and_then(|v| v.as_str())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert!(json
        .get("trust_score_cached_at")
        .and_then(|v| v.as_str())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
}

#[tokio::test]
async fn proxy_explain_candidate_preview_roundtrips_as_typed_shape() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES
                  ('proxy-typed-preview-best', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-typed', 'active', 0.77, 5, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1'),
                  ('proxy-typed-preview-second', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-east', 'US', 'pool-typed', 'active', 0.66, 4, 2, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 0, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxies");
    persona_pilot::db::init::refresh_provider_risk_snapshots(&state.db)
        .await
        .expect("refresh provider risk snapshots");
    persona_pilot::db::init::refresh_cached_trust_scores(&state.db)
        .await
        .expect("refresh cached trust scores");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-typed-preview-best/explain")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let preview = json
        .get("candidate_rank_preview")
        .and_then(|v| v.as_array())
        .expect("candidate_rank_preview");
    assert!(!preview.is_empty());
    let first = &preview[0];
    assert!(first
        .get("id")
        .and_then(|v| v.as_str())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert!(first.get("score").and_then(|v| v.as_f64()).is_some());
    assert!(first
        .get("trust_score_total")
        .and_then(|v| v.as_i64())
        .is_some());
    assert!(first
        .get("summary")
        .and_then(|v| v.as_str())
        .map(|v| !v.is_empty())
        .unwrap_or(false));
    assert!(first.get("winner_vs_runner_up_diff").is_some());
}

#[tokio::test]
async fn proxy_explain_trust_score_components_roundtrip_as_typed_shape() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at)
                  VALUES ('proxy-components-typed', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-components', 'active', 0.77, 5, 1, NULL, NULL, NULL, NULL, NULL, 1, NULL, NULL, NULL, 'ok', 1, 'US', 'Virginia', '9999999999', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let (status, json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-components-typed/explain")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let comp = json.get("trust_score_components").expect("components");
    for key in [
        "verify_ok_bonus",
        "verify_geo_match_bonus",
        "smoke_upstream_ok_bonus",
        "raw_score_component",
        "verify_confidence_bonus",
        "verify_score_delta_bonus",
        "verify_source_bonus",
        "missing_verify_penalty",
        "stale_verify_penalty",
        "verify_failed_heavy_penalty",
        "verify_failed_light_penalty",
        "verify_failed_base_penalty",
        "individual_history_penalty",
        "provider_risk_penalty",
        "provider_region_cluster_penalty",
    ] {
        assert!(
            comp.get(key).and_then(|v| v.as_i64()).is_some(),
            "missing key {key}"
        );
    }
    assert_eq!(
        comp.get("verify_source_bonus").and_then(|v| v.as_i64()),
        Some(0)
    );
}

#[tokio::test]
async fn verify_proxy_uses_region_match_and_complete_identity_in_confidence() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\nip=8.8.8.8\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-verify-region-complete",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "Virginia",
        "country": "US",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (verify_status, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-region-complete/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(verify_status, StatusCode::OK);
    assert_eq!(
        verify_json.get("geo_match_ok").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        verify_json.get("region_match_ok").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        verify_json
            .get("identity_fields_complete")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        verify_json
            .get("verification_confidence")
            .and_then(|v| v.as_f64()),
        Some(0.98)
    );
    assert_eq!(
        verify_json
            .get("verification_score_delta")
            .and_then(|v| v.as_i64()),
        Some(18)
    );
}

#[tokio::test]
async fn verify_proxy_rejects_invalid_exit_ip_shape_from_identity_probe() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\nip=not-an-ip\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-verify-invalid-ip",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "Virginia",
        "country": "US",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (verify_status, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-invalid-ip/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(verify_status, StatusCode::OK);
    assert_eq!(verify_json.get("exit_ip").and_then(|v| v.as_str()), None);
    assert_eq!(
        verify_json
            .get("identity_fields_complete")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        verify_json.get("upstream_ok").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        verify_json.get("status").and_then(|v| v.as_str()),
        Some("failed")
    );
}

#[tokio::test]
async fn verify_proxy_penalizes_non_public_exit_ip_and_transparent_headers() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\nX-Forwarded-For: 10.0.0.7\r\nVia: 1.1 example\r\n\r\nip=10.0.0.7\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-verify-private-transparent",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "Virginia",
        "country": "US",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (verify_status, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-private-transparent/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(verify_status, StatusCode::OK);
    assert_eq!(
        verify_json.get("upstream_ok").and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        verify_json.get("anonymity_level").and_then(|v| v.as_str()),
        Some("transparent")
    );
    assert_eq!(
        verify_json
            .get("probe_error_category")
            .and_then(|v| v.as_str()),
        Some("exit_ip_not_public")
    );
    assert_eq!(
        verify_json.get("status").and_then(|v| v.as_str()),
        Some("failed")
    );
}

#[tokio::test]
async fn verify_proxy_returns_human_readable_risk_summary() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\nX-Forwarded-For: 10.0.0.7\r\n\r\nip=10.0.0.7\ncountry=CA\nregion=Ontario\n"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-verify-risk-summary",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "Virginia",
        "country": "US",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (verify_status, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-risk-summary/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(verify_status, StatusCode::OK);
    assert_eq!(
        verify_json.get("risk_level").and_then(|v| v.as_str()),
        Some("high")
    );
    let reasons = verify_json
        .get("risk_reasons")
        .and_then(|v| v.as_array())
        .expect("risk reasons");
    let as_strings: Vec<&str> = reasons.iter().filter_map(|v| v.as_str()).collect();
    assert!(as_strings.contains(&"exit_ip_not_public"));
    assert!(as_strings.contains(&"transparent_proxy"));
    assert!(as_strings.contains(&"geo_mismatch"));
    assert!(as_strings.contains(&"region_mismatch"));
}

#[tokio::test]
async fn verify_proxy_classifies_failure_stage_and_detail() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\nVia: 1.1 example\r\n\r\nip=8.8.8.8\ncountry=CA\nregion=Ontario\n"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    let proxy_payload = serde_json::json!({
        "id": "proxy-verify-failure-stage",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "Virginia",
        "country": "US",
        "provider": "smoke",
        "score": 0.5
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (verify_status, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-failure-stage/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(verify_status, StatusCode::OK);
    assert_eq!(
        verify_json.get("failure_stage").and_then(|v| v.as_str()),
        Some("risk")
    );
    assert_eq!(
        verify_json
            .get("failure_stage_detail")
            .and_then(|v| v.as_str()),
        Some("anonymous_proxy")
    );
    assert_eq!(
        verify_json.get("risk_level").and_then(|v| v.as_str()),
        Some("medium")
    );
}

#[tokio::test]
async fn verify_proxy_returns_verification_class_labels() {
    let listener_ok = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ok listener");
    let ok_addr = listener_ok.local_addr().expect("ok local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener_ok.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\nip=8.8.8.8\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let listener_bad = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind bad listener");
    let bad_addr = listener_bad.local_addr().expect("bad local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener_bad.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\nX-Forwarded-For: 10.0.0.7\r\n\r\nip=10.0.0.7\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    for payload in [
        serde_json::json!({
            "id": "proxy-verify-class-ok",
            "scheme": "http",
            "host": ok_addr.ip().to_string(),
            "port": ok_addr.port(),
            "region": "Virginia",
            "country": "US",
            "provider": "smoke",
            "score": 0.5
        }),
        serde_json::json!({
            "id": "proxy-verify-class-bad",
            "scheme": "http",
            "host": bad_addr.ip().to_string(),
            "port": bad_addr.port(),
            "region": "Virginia",
            "country": "US",
            "provider": "smoke",
            "score": 0.5
        }),
    ] {
        let (create_status, _) = json_response(
            &app,
            Request::builder()
                .method("POST")
                .uri("/proxies")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request"),
        )
        .await;
        assert_eq!(create_status, StatusCode::CREATED);
    }

    let (_, ok_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-class-ok/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        ok_json.get("verification_class").and_then(|v| v.as_str()),
        Some("trusted")
    );

    let (_, bad_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-verify-class-bad/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        bad_json.get("verification_class").and_then(|v| v.as_str()),
        Some("rejected")
    );
}

#[tokio::test]
async fn verify_proxy_returns_recommended_action_labels() {
    let listener_ok = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ok listener");
    let ok_addr = listener_ok.local_addr().expect("ok local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener_ok.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\nip=8.8.8.8\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let listener_risky = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind risky listener");
    let risky_addr = listener_risky.local_addr().expect("risky local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener_risky.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\nVia: 1.1 example\r\n\r\nip=8.8.8.8\ncountry=CA\nregion=Ontario\n"),
            ).await;
        }
    });

    let listener_bad = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind bad listener");
    let bad_addr = listener_bad.local_addr().expect("bad local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener_bad.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), socket.read(&mut buf))
                .await;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                socket.write_all(b"HTTP/1.1 200 Connection Established\r\nX-Forwarded-For: 10.0.0.7\r\n\r\nip=10.0.0.7\ncountry=US\nregion=Virginia\n"),
            ).await;
        }
    });

    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");
    for payload in [
        serde_json::json!({"id":"proxy-action-ok","scheme":"http","host":ok_addr.ip().to_string(),"port":ok_addr.port(),"region":"Virginia","country":"US","provider":"smoke","score":0.5}),
        serde_json::json!({"id":"proxy-action-risky","scheme":"http","host":risky_addr.ip().to_string(),"port":risky_addr.port(),"region":"Virginia","country":"US","provider":"smoke","score":0.5}),
        serde_json::json!({"id":"proxy-action-bad","scheme":"http","host":bad_addr.ip().to_string(),"port":bad_addr.port(),"region":"Virginia","country":"US","provider":"smoke","score":0.5}),
    ] {
        let (create_status, _) = json_response(
            &app,
            Request::builder()
                .method("POST")
                .uri("/proxies")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .expect("request"),
        )
        .await;
        assert_eq!(create_status, StatusCode::CREATED);
    }

    let (_, ok_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-action-ok/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        ok_json.get("recommended_action").and_then(|v| v.as_str()),
        Some("use")
    );

    let (_, risky_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-action-risky/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        risky_json
            .get("recommended_action")
            .and_then(|v| v.as_str()),
        Some("retry_later")
    );

    let (_, bad_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-action-bad/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        bad_json.get("recommended_action").and_then(|v| v.as_str()),
        Some("quarantine")
    );
}

#[tokio::test]
async fn create_task_resolves_proxy_with_soft_min_score_penalty() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let proxy_payload = serde_json::json!({
        "id":"proxy-soft-1",
        "scheme":"http",
        "host":"127.0.0.1",
        "port":8080,
        "region":"us-east",
        "country":"US",
        "provider":"pool-soft",
        "score":0.65
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-soft", "region": "us-east", "min_score": 0.6, "soft_min_score": 0.8}
    });
    let (status, task_create) = json_response(
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
    let task_id = task_create
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("proxy_id").and_then(|v| v.as_str()),
        Some("proxy-soft-1")
    );
    let explain = task.get("selection_explain").expect("selection explain");
    assert_eq!(
        explain.get("soft_min_score").and_then(|v| v.as_f64()),
        Some(0.8)
    );
    assert_eq!(
        explain
            .get("soft_min_score_penalty_applied")
            .and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(explain
        .get("fingerprint_budget_medium_limit")
        .and_then(|v| v.as_u64())
        .is_some());
    assert!(explain
        .get("fingerprint_budget_heavy_limit")
        .and_then(|v| v.as_u64())
        .is_some());
    let runtime = task
        .get("fingerprint_runtime_explain")
        .expect("fingerprint runtime explain");
    assert!(runtime.get("fingerprint_budget_tag").is_some() || runtime.is_null());
    assert!(task
        .get("trust_score_total")
        .and_then(|v| v.as_i64())
        .is_some());
}

#[tokio::test]
async fn create_task_hard_min_score_still_rejects_below_threshold() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let proxy_payload = serde_json::json!({
        "id":"proxy-hard-1",
        "scheme":"http",
        "host":"127.0.0.1",
        "port":8081,
        "region":"us-east",
        "country":"US",
        "provider":"pool-hard",
        "score":0.65
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(proxy_payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com",
        "timeout_seconds": 5,
        "network_policy_json": {"mode": "required_proxy", "provider": "pool-hard", "region": "us-east", "min_score": 0.7, "soft_min_score": 0.9}
    });
    let (status, task_create) = json_response(
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
    let task_id = task_create
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert!(
        task.get("proxy_id").is_none() || task.get("proxy_id").and_then(|v| v.as_str()).is_none()
    );
    let result_json_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&_state.db)
            .await
            .expect("load result");
    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result json");
    let policy = result_json
        .get("payload")
        .and_then(|v| v.get("network_policy_json"))
        .expect("policy");
    let explain = policy.get("selection_explain").expect("selection explain");
    assert_eq!(
        explain.get("no_match_reason_code").and_then(|v| v.as_str()),
        Some("no_match_after_min_score_filter")
    );
}

#[tokio::test]
async fn status_detail_and_runs_share_execution_identity_contract() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO fingerprint_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
           VALUES ('fp-contract-missing', 'Contract Missing', 1, 'active', NULL, '{"locale":"en-US"}', '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert active fingerprint profile");

    let payload = serde_json::json!({
        "kind": "get_html",
        "url": "https://example.com/contract-identity",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-contract-missing",
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-contract-missing"}
    });
    let (create_status, create_body) = text_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(
        create_status,
        StatusCode::CREATED,
        "create task body: {create_body}"
    );
    let create_json: Value = serde_json::from_str(&create_body).expect("create task json body");
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;

    let task_execution_identity = task_json
        .get("execution_identity")
        .cloned()
        .expect("task execution identity");
    assert_eq!(
        task_execution_identity
            .get("fingerprint_profile_id")
            .and_then(|v| v.as_str()),
        Some("fp-contract-missing")
    );
    assert_eq!(
        task_execution_identity
            .get("fingerprint_resolution_status")
            .and_then(|v| v.as_str()),
        Some("resolved")
    );
    assert!(task_execution_identity
        .get("proxy_id")
        .map(|v| v.is_null())
        .unwrap_or(true));

    let (_, runs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/runs"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let runs = runs_json.as_array().expect("runs array");
    assert!(!runs.is_empty());
    let run = &runs[0];
    let run_execution_identity = run
        .get("execution_identity")
        .expect("run execution identity");
    assert_eq!(
        run_execution_identity.get("proxy_resolution_status"),
        task_execution_identity.get("proxy_resolution_status")
    );
    assert_eq!(
        run_execution_identity.get("selection_reason_summary"),
        task_execution_identity.get("selection_reason_summary")
    );
    assert_eq!(
        run_execution_identity.get("selection_explain"),
        task_execution_identity.get("selection_explain")
    );
    assert_eq!(
        run_execution_identity.get("fingerprint_runtime_explain"),
        task_execution_identity.get("fingerprint_runtime_explain")
    );
    assert_eq!(run.get("failure_scope"), task_json.get("failure_scope"));
    assert_eq!(
        run.get("browser_failure_signal"),
        task_json.get("browser_failure_signal")
    );
    let run_summary_artifacts = run
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("run summary artifacts");
    let task_summary_artifacts = task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("task summary artifacts");
    assert_eq!(run_summary_artifacts.len(), task_summary_artifacts.len());
    for (run_artifact, task_artifact) in run_summary_artifacts
        .iter()
        .zip(task_summary_artifacts.iter())
    {
        assert_eq!(run_artifact.get("key"), task_artifact.get("key"));
        assert_eq!(run_artifact.get("summary"), task_artifact.get("summary"));
        assert_eq!(run_artifact.get("title"), task_artifact.get("title"));
    }
    let run_identity_network = run
        .get("identity_network_explain")
        .expect("run identity network explain");
    let task_identity_network = task_json
        .get("identity_network_explain")
        .expect("task identity network explain");
    assert_eq!(
        run_identity_network.get("selection_reason_summary"),
        task_identity_network.get("selection_reason_summary")
    );
    assert_eq!(
        run_identity_network.get("proxy_resolution_status"),
        task_identity_network.get("proxy_resolution_status")
    );
    assert_eq!(
        run_identity_network.get("fingerprint_runtime_explain"),
        task_identity_network.get("fingerprint_runtime_explain")
    );
    assert_eq!(
        run.get("selection_explain"),
        task_json.get("selection_explain")
    );
    assert_eq!(
        run.get("fingerprint_runtime_explain"),
        task_json.get("fingerprint_runtime_explain")
    );

    let (_, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let latest_tasks = status_json
        .get("latest_tasks")
        .and_then(|v| v.as_array())
        .expect("latest tasks");
    let latest_task = latest_tasks
        .iter()
        .find(|item| item.get("id").and_then(|v| v.as_str()) == Some(task_id.as_str()))
        .expect("latest task item");
    let latest_task_execution_identity = latest_task
        .get("execution_identity")
        .expect("latest task execution identity");
    assert_eq!(
        latest_task_execution_identity.get("proxy_resolution_status"),
        task_execution_identity.get("proxy_resolution_status")
    );
    assert_eq!(
        latest_task_execution_identity.get("selection_reason_summary"),
        task_execution_identity.get("selection_reason_summary")
    );
    assert_eq!(
        latest_task_execution_identity.get("fingerprint_runtime_explain"),
        task_execution_identity.get("fingerprint_runtime_explain")
    );
    assert_eq!(
        latest_task.get("failure_scope"),
        task_json.get("failure_scope")
    );
    assert_eq!(
        latest_task.get("browser_failure_signal"),
        task_json.get("browser_failure_signal")
    );
    let latest_task_summary_artifacts = latest_task
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("latest task summary artifacts");
    let task_summary_artifacts = task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("task summary artifacts");
    assert_eq!(
        latest_task_summary_artifacts.len(),
        task_summary_artifacts.len()
    );
    for (latest_artifact, task_artifact) in latest_task_summary_artifacts
        .iter()
        .zip(task_summary_artifacts.iter())
    {
        assert_eq!(latest_artifact.get("key"), task_artifact.get("key"));
        assert_eq!(latest_artifact.get("summary"), task_artifact.get("summary"));
        assert_eq!(latest_artifact.get("title"), task_artifact.get("title"));
    }

    let latest_browser_tasks = status_json
        .get("latest_browser_tasks")
        .and_then(|v| v.as_array())
        .expect("latest browser tasks");
    if let Some(latest_browser_task) = latest_browser_tasks
        .iter()
        .find(|item| item.get("id").and_then(|v| v.as_str()) == Some(task_id.as_str()))
    {
        let latest_browser_execution_identity = latest_browser_task
            .get("execution_identity")
            .expect("latest browser execution identity");
        assert_eq!(
            latest_browser_execution_identity.get("proxy_resolution_status"),
            task_execution_identity.get("proxy_resolution_status")
        );
        assert_eq!(
            latest_browser_execution_identity.get("selection_reason_summary"),
            task_execution_identity.get("selection_reason_summary")
        );
        assert_eq!(
            latest_browser_execution_identity.get("fingerprint_runtime_explain"),
            task_execution_identity.get("fingerprint_runtime_explain")
        );
        let latest_browser_summary_artifacts = latest_browser_task
            .get("summary_artifacts")
            .and_then(|v| v.as_array())
            .expect("latest browser summary artifacts");
        assert_eq!(
            latest_browser_summary_artifacts.len(),
            task_summary_artifacts.len()
        );
        for (latest_artifact, task_artifact) in latest_browser_summary_artifacts
            .iter()
            .zip(task_summary_artifacts.iter())
        {
            assert_eq!(latest_artifact.get("key"), task_artifact.get("key"));
            assert_eq!(latest_artifact.get("summary"), task_artifact.get("summary"));
            assert_eq!(latest_artifact.get("title"), task_artifact.get("title"));
        }
    }
}

#[tokio::test]
async fn browser_task_direct_mode_is_rejected_for_browser_kinds() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "kind": "open_page",
        "url": "https://example.com/direct-forbidden",
        "timeout_seconds": 5,
        "network_policy_json": {
            "mode": "direct"
        }
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
    assert_eq!(
        body,
        "direct mode is forbidden for browser tasks; browser access must use proxy pool"
    );
}

#[tokio::test]
async fn browser_task_without_active_proxy_fails_with_consistent_no_proxy_contract() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "url": "https://example.com/no-proxy",
        "timeout_seconds": 5,
        "network_policy_json": {
            "provider": "pool-missing",
            "region": "us-east"
        }
    });
    let (create_status, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/browser/open")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();

    let task_json = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task_json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_FAILED)
    );
    assert_eq!(
        task_json.get("failure_scope").and_then(|v| v.as_str()),
        Some("network_policy")
    );
    assert_eq!(task_json.get("browser_failure_signal"), Some(&Value::Null));
    assert_eq!(
        task_json
            .get("selection_reason_summary")
            .and_then(|v| v.as_str()),
        Some("no eligible active proxy matched the current policy filters")
    );
    assert_eq!(
        task_json
            .get("proxy_resolution_status")
            .and_then(|v| v.as_str()),
        Some("unresolved")
    );
    assert_eq!(
        task_json
            .get("summary_artifacts")
            .and_then(|v| v.as_array())
            .map(|items| items
                .iter()
                .any(|item| item.get("title").and_then(|v| v.as_str())
                    == Some("proxy selection failed"))),
        Some(true)
    );

    let (_, runs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/runs"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let run = runs_json
        .as_array()
        .and_then(|items| items.first())
        .expect("latest run");
    assert_eq!(
        run.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_FAILED)
    );
    assert_eq!(
        run.get("failure_scope").and_then(|v| v.as_str()),
        Some("network_policy")
    );
    assert_eq!(run.get("browser_failure_signal"), Some(&Value::Null));
    assert_eq!(
        run.get("selection_reason_summary"),
        task_json.get("selection_reason_summary")
    );
    assert_eq!(
        run.get("selection_explain"),
        task_json.get("selection_explain")
    );
    assert_eq!(
        run.get("execution_identity"),
        task_json.get("execution_identity")
    );

    let (_, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=20&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let latest_task = status_json
        .get("latest_tasks")
        .and_then(|v| v.as_array())
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("id").and_then(|v| v.as_str()) == Some(task_id.as_str()))
        })
        .expect("status latest task");
    assert_eq!(
        latest_task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_FAILED)
    );
    assert_eq!(
        latest_task.get("failure_scope"),
        task_json.get("failure_scope")
    );
    assert_eq!(
        latest_task.get("selection_reason_summary"),
        task_json.get("selection_reason_summary")
    );
    assert_eq!(
        latest_task.get("selection_explain"),
        task_json.get("selection_explain")
    );
    assert_eq!(
        latest_task.get("execution_identity"),
        task_json.get("execution_identity")
    );
}

#[tokio::test]
async fn candidate_proxy_ingest_dedupes_same_endpoint() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let candidate_one = serde_json::json!({
        "id": "proxy-candidate-a",
        "scheme": "http",
        "host": "127.0.0.10",
        "port": 8080,
        "region": "us-east",
        "country": "US",
        "provider": "harvest-a",
        "status": "candidate",
        "score": 0.6
    });
    let (status_one, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(candidate_one.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status_one, StatusCode::CREATED);

    let candidate_two = serde_json::json!({
        "id": "proxy-candidate-b",
        "scheme": "http",
        "host": "127.0.0.10",
        "port": 8080,
        "region": "us-east",
        "country": "US",
        "provider": "harvest-a",
        "status": "candidate",
        "score": 0.7
    });
    let (status_two, body_two) = text_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(candidate_two.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status_two, StatusCode::OK, "dedupe response: {body_two}");
    let proxy_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM proxies WHERE scheme = 'http' AND host = '127.0.0.10' AND port = 8080 AND provider = 'harvest-a' AND region = 'us-east'"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("count deduped proxies");
    assert_eq!(proxy_count, 1);
    let stored: (String, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT id, status, last_seen_at FROM proxies WHERE host = '127.0.0.10' LIMIT 1"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load deduped proxy");
    assert_eq!(stored.0, "proxy-candidate-a");
    assert_eq!(stored.1.as_deref(), Some("candidate"));
    assert!(stored.2.is_some());
}

#[tokio::test]
async fn candidate_proxy_verify_success_promotes_to_active() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(Duration::from_secs(3), socket.read(&mut buf)).await;
            let _ = tokio::time::timeout(
                Duration::from_secs(3),
                socket.write_all(
                    b"HTTP/1.1 200 Connection Established\r\n\r\nip=1.1.1.9\ncountry=US\nregion=Virginia\n",
                ),
            )
            .await;
        }
    });

    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    let candidate = serde_json::json!({
        "id": "proxy-promote-ok",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "Virginia",
        "country": "US",
        "provider": "candidate-promote",
        "status": "candidate"
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(candidate.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (_, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-promote-ok/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        verify_json.get("status").and_then(|v| v.as_str()),
        Some("ok")
    );

    let (_, proxy_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-promote-ok")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        proxy_json.get("status").and_then(|v| v.as_str()),
        Some("active")
    );
    let stored: (Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT promoted_at, cooldown_until FROM proxies WHERE id = 'proxy-promote-ok'"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load promoted candidate");
    assert!(stored.0.is_some());
    assert!(stored.1.is_none());
}

#[tokio::test]
async fn candidate_proxy_verify_connect_tunnel_without_identity_fields_still_promotes_to_active() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0_u8; 256];
            let _ = tokio::time::timeout(Duration::from_secs(3), socket.read(&mut buf)).await;
            let _ = tokio::time::timeout(
                Duration::from_secs(3),
                socket.write_all(
                    b"HTTP/1.1 200 Connection Established\r\nProxy-Agent: test-proxy\r\n\r\n",
                ),
            )
            .await;
        }
    });

    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    let candidate = serde_json::json!({
        "id": "proxy-promote-connect-ok",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "global",
        "country": "unknown",
        "provider": "candidate-promote-connect",
        "status": "candidate"
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(candidate.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (_, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-promote-connect-ok/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        verify_json.get("status").and_then(|v| v.as_str()),
        Some("ok")
    );
    assert_eq!(
        verify_json
            .get("identity_fields_complete")
            .and_then(|v| v.as_bool()),
        Some(false)
    );
    assert_eq!(
        verify_json.get("upstream_ok").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        verify_json.get("message").and_then(|v| v.as_str()),
        Some("proxy verify established CONNECT tunnel without identity echo fields anonymity=Some(\"elite\")")
    );

    let (_, proxy_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-promote-connect-ok")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        proxy_json.get("status").and_then(|v| v.as_str()),
        Some("active")
    );
    let stored: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT promoted_at, cooldown_until, last_probe_error_category FROM proxies WHERE id = 'proxy-promote-connect-ok'"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load promoted candidate");
    assert!(stored.0.is_some());
    assert!(stored.1.is_none());
    assert!(stored.2.is_none());
}

#[tokio::test]
async fn candidate_proxy_verify_failure_marks_candidate_rejected_and_sets_cooldown() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("local addr");
    drop(listener);

    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    let candidate = serde_json::json!({
        "id": "proxy-promote-fail",
        "scheme": "http",
        "host": addr.ip().to_string(),
        "port": addr.port(),
        "region": "us-east",
        "country": "US",
        "provider": "candidate-fail",
        "status": "candidate"
    });
    let (create_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies")
            .header("content-type", "application/json")
            .body(Body::from(candidate.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);

    let (_, verify_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/proxy-promote-fail/verify")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        verify_json.get("status").and_then(|v| v.as_str()),
        Some("failed")
    );

    let (_, proxy_json) = json_response(
        &app,
        Request::builder()
            .uri("/proxies/proxy-promote-fail")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(
        proxy_json.get("status").and_then(|v| v.as_str()),
        Some("candidate_rejected")
    );
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let stored: (Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT cooldown_until, promoted_at FROM proxies WHERE id = 'proxy-promote-fail'"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load rejected candidate");
    let cooldown_until = stored
        .0
        .and_then(|value| value.parse::<i64>().ok())
        .expect("cooldown_until");
    assert!(cooldown_until >= now + 1700);
    assert!(stored.1.is_none());
}

#[tokio::test]
async fn replenish_tick_prioritizes_hot_region_candidates() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db.clone(), runner, None, 1);

    for idx in 0..20 {
        sqlx::query(
            r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, created_at, updated_at)
               VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'us-west', 'US', 'steady', 'active', 0.9, 0, 0, '1', '1')"#,
        )
        .bind(format!("proxy-active-{idx}"))
        .bind(9000_i64 + idx)
        .execute(&db)
        .await
        .expect("insert active baseline proxy");
    }

    for (proxy_id, region) in [
        ("proxy-candidate-us-1", "us-east"),
        ("proxy-candidate-us-2", "us-east"),
        ("proxy-candidate-eu-1", "eu-west"),
    ] {
        sqlx::query(
            r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, source_label, last_seen_at, created_at, updated_at)
               VALUES (?, 'http', '127.0.0.1', 9100, NULL, NULL, ?, 'US', 'harvest', 'candidate', 0.6, 'seed', '10', '10', '10')"#,
        )
        .bind(proxy_id)
        .bind(region)
        .execute(&db)
        .await
        .expect("insert candidate proxy");
    }

    for idx in 0..2 {
        let input_json = serde_json::json!({
            "url": "https://example.com/replenish",
            "timeout_seconds": 5,
            "network_policy_json": {
                "mode": "required_proxy",
                "region": "us-east"
            }
        })
        .to_string();
        sqlx::query(
            r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
               VALUES (?, 'open_page', 'queued', ?, ?, NULL, 0, '1', '1', NULL, NULL, NULL, NULL, NULL, NULL)"#,
        )
        .bind(format!("task-replenish-{idx}"))
        .bind(&input_json)
        .bind(serde_json::json!({"mode": "required_proxy", "region": "us-east"}).to_string())
        .execute(&db)
        .await
        .expect("insert hot-region task");
    }

    let batches = run_proxy_replenish_mvp_tick(&state)
        .await
        .expect("run replenish tick");
    assert!(!batches.is_empty());
    assert!(batches
        .iter()
        .any(|batch| batch.target_region.as_deref() == Some("us-east")));
    let region_batch = batches
        .iter()
        .find(|batch| batch.target_region.as_deref() == Some("us-east"))
        .expect("region replenish batch");
    assert_eq!(region_batch.accepted, 2);
    assert!(region_batch
        .proxy_ids
        .iter()
        .all(|id| id.starts_with("proxy-candidate-us-")));

    let scheduled: Vec<(String,)> = sqlx::query_as(
        r#"SELECT json_extract(input_json, '$.proxy_id') FROM tasks WHERE kind = 'verify_proxy' AND json_extract(input_json, '$.verify_batch_id') = ? ORDER BY id ASC"#,
    )
    .bind(&region_batch.batch_id)
    .fetch_all(&db)
    .await
    .expect("load scheduled replenish tasks");
    assert_eq!(scheduled.len(), 2);
    assert!(scheduled
        .iter()
        .all(|(proxy_id,)| proxy_id.starts_with("proxy-candidate-us-")));
}

#[tokio::test]
async fn replenish_tick_prioritizes_complete_metadata_before_blank_provider() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db.clone(), runner, None, 1);

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-good', 'text_url', 1, '{}', 300, 0.6, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1'),
           ('source-bad', 'text_url', 1, '{}', 300, 0.6, 0, NULL, NULL, NULL, 'completed', NULL, 20.0, '1', '1')"#,
    )
    .execute(&db)
    .await
    .expect("insert source registry rows");

    for idx in 0..20 {
        sqlx::query(
            r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, created_at, updated_at)
               VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'us-west', 'US', 'steady', 'active', 0.9, 0, 0, '1', '1')"#,
        )
        .bind(format!("proxy-steady-{idx}"))
        .bind(9400_i64 + idx)
        .execute(&db)
        .await
        .expect("insert active baseline proxy");
    }

    for (proxy_id, source_label, provider) in [
        ("proxy-complete-good", "source-good", Some("provider-good")),
        ("proxy-complete-bad", "source-bad", Some("provider-bad")),
        ("proxy-blank-provider", "source-good", None),
    ] {
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   source_label, last_seen_at, created_at, updated_at
               ) VALUES (?, 'http', '127.0.0.1', 9500, NULL, NULL, 'us-east', 'US', ?, 'candidate', 0.6, ?, '10', '10', '10')"#,
        )
        .bind(proxy_id)
        .bind(provider)
        .bind(source_label)
        .execute(&db)
        .await
        .expect("insert candidate");
    }

    for idx in 0..2 {
        let input_json = serde_json::json!({
            "url": "https://example.com/replenish-priority",
            "timeout_seconds": 5,
            "network_policy_json": {
                "mode": "required_proxy",
                "region": "us-east"
            }
        })
        .to_string();
        sqlx::query(
            r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
               VALUES (?, 'open_page', 'queued', ?, ?, NULL, 0, '1', '1', NULL, NULL, NULL, NULL, NULL, NULL)"#,
        )
        .bind(format!("task-priority-{idx}"))
        .bind(&input_json)
        .bind(serde_json::json!({"mode": "required_proxy", "region": "us-east"}).to_string())
        .execute(&db)
        .await
        .expect("insert hot-region task");
    }

    let batches = run_proxy_replenish_mvp_tick(&state)
        .await
        .expect("run replenish tick");
    let region_batch = batches
        .iter()
        .find(|batch| batch.target_region.as_deref() == Some("us-east"))
        .expect("region replenish batch");
    assert_eq!(region_batch.accepted, 3);
    let scheduled: Vec<(String,)> = sqlx::query_as(
        r#"SELECT json_extract(input_json, '$.proxy_id')
           FROM tasks
           WHERE kind = 'verify_proxy'
             AND json_extract(input_json, '$.verify_batch_id') = ?
           ORDER BY rowid ASC"#,
    )
    .bind(&region_batch.batch_id)
    .fetch_all(&db)
    .await
    .expect("load scheduled verify tasks");
    let scheduled_ids = scheduled
        .into_iter()
        .map(|(proxy_id,)| proxy_id)
        .collect::<Vec<_>>();
    assert_eq!(scheduled_ids.len(), 3);
    assert_eq!(scheduled_ids[0], "proxy-complete-good");
    assert_eq!(scheduled_ids[1], "proxy-complete-bad");
    assert_eq!(scheduled_ids[2], "proxy-blank-provider");
}

#[tokio::test]
async fn replenish_tick_suppresses_duplicate_recent_region_batch() {
    let db_url = unique_db_url();
    let db = init_db(&db_url).await.expect("init db");
    let runner = std::sync::Arc::new(FakeRunner);
    let state = build_app_state(db.clone(), runner, None, 1);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    for idx in 0..20 {
        sqlx::query(
            r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, created_at, updated_at)
               VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'us-west', 'US', 'steady', 'active', 0.9, '1', '1')"#,
        )
        .bind(format!("proxy-keepalive-{idx}"))
        .bind(9200_i64 + idx)
        .execute(&db)
        .await
        .expect("insert active proxy");
    }

    sqlx::query(
        r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, source_label, last_seen_at, created_at, updated_at)
           VALUES ('proxy-candidate-dup', 'http', '127.0.0.1', 9300, NULL, NULL, 'us-east', 'US', 'harvest', 'candidate', 0.6, 'seed', '10', '10', '10')"#,
    )
    .execute(&db)
    .await
    .expect("insert candidate");

    let input_json = serde_json::json!({
        "url": "https://example.com/replenish",
        "timeout_seconds": 5,
        "network_policy_json": {
            "mode": "required_proxy",
            "region": "us-east"
        }
    })
    .to_string();
    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES ('task-dup-region', 'open_page', 'queued', ?, ?, NULL, 0, '1', '1', NULL, NULL, NULL, NULL, NULL, NULL)"#,
    )
    .bind(&input_json)
    .bind(serde_json::json!({"mode": "required_proxy", "region": "us-east"}).to_string())
    .execute(&db)
    .await
    .expect("insert hot-region task");

    sqlx::query(
        r#"INSERT INTO verify_batches (id, status, requested_count, accepted_count, skipped_count, stale_after_seconds, task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at)
           VALUES ('verify-batch-recent-dup', 'scheduled', 1, 1, 0, 0, 5, '[]', ?, ?, ?)"#,
    )
    .bind(serde_json::json!({
        "reason": "replenish_mvp",
        "candidate_mode": true,
        "target_region": "us-east"
    })
    .to_string())
    .bind(&now)
    .bind(&now)
    .execute(&db)
    .await
    .expect("insert recent replenish batch");

    let batches = run_proxy_replenish_mvp_tick(&state)
        .await
        .expect("run replenish tick");
    assert!(batches.is_empty());
    let scheduled_count: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*) FROM tasks WHERE kind = 'verify_proxy'"#)
            .fetch_one(&db)
            .await
            .expect("count verify tasks");
    assert_eq!(scheduled_count, 0);
}

#[tokio::test]
async fn status_exposes_proxy_harvest_source_summaries() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES (
               'source-status', 'text_url', 1, '{"default_fields":{"provider":"seed","region":"global","country":"unknown"}}', 300, 0.6,
               0, NULL, '10', '11', 'completed', NULL, 77.0, '1', '1'
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source");
    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, last_seen_at, promoted_at, last_verify_status, created_at, updated_at
           ) VALUES
           ('proxy-source-active', 'http', '127.0.0.1', 9600, NULL, NULL, 'global', 'unknown', 'seed', 'active', 0.7, 'source-status', '11', '11', 'ok', '1', '1'),
           ('proxy-source-candidate', 'http', '127.0.0.1', 9601, NULL, NULL, 'global', 'unknown', 'seed', 'candidate', 0.6, 'source-status', '11', NULL, NULL, '1', '1'),
           ('proxy-source-rejected', 'http', '127.0.0.1', 9602, NULL, NULL, NULL, NULL, NULL, 'candidate_rejected', 0.4, 'source-status', '11', NULL, 'failed', '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source proxies");
    sqlx::query(
        r#"INSERT INTO proxy_harvest_runs (
               id, source_label, source_kind, fetched_count, accepted_count, deduped_count, rejected_count,
               status, summary_json, started_at, finished_at
           ) VALUES (
               'proxy-harvest-source-status', 'source-status', 'text_url', 10, 2, 7, 1, 'completed',
               '{"null_metadata_count":1,"active_count_snapshot":1,"candidate_count_snapshot":1,"candidate_rejected_count_snapshot":1,"source_promotion_rate_snapshot":0.5}',
               '10', '11'
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source run");

    let (_, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=20&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let source_summary = status_json
        .get("proxy_harvest_metrics")
        .and_then(|value| value.get("source_summaries"))
        .and_then(|value| value.as_array())
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("source_label").and_then(|v| v.as_str()) == Some("source-status")
            })
        })
        .expect("source summary");
    assert_eq!(
        source_summary
            .get("candidate_count")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        source_summary.get("active_count").and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        source_summary
            .get("candidate_rejected_count")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        source_summary
            .get("null_provider_count")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        source_summary
            .get("null_region_count")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        source_summary.get("health_score").and_then(|v| v.as_f64()),
        Some(77.0)
    );
    assert_eq!(
        source_summary
            .get("promotion_rate")
            .and_then(|v| v.as_f64()),
        Some(0.5)
    );

    let recent_run = status_json
        .get("proxy_harvest_metrics")
        .and_then(|value| value.get("recent_harvest_runs"))
        .and_then(|value| value.as_array())
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("id").and_then(|v| v.as_str()) == Some("proxy-harvest-source-status")
            })
        })
        .expect("recent run");
    assert_eq!(
        recent_run
            .get("null_metadata_count")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        recent_run
            .get("active_count_snapshot")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        recent_run
            .get("candidate_count_snapshot")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        recent_run
            .get("candidate_rejected_count_snapshot")
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    assert_eq!(
        recent_run
            .get("source_promotion_rate_snapshot")
            .and_then(|v| v.as_f64()),
        Some(0.5)
    );
}

#[tokio::test]
async fn create_task_persists_typed_proxy_columns() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES (
               'source-typed-task', 'text_url', 'private_paid', 0, 1, 'external_probe_v2',
               'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1'
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source");
    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, last_verify_status, last_verify_at, created_at, updated_at
           ) VALUES (
               'proxy-explicit-typed', 'http', '127.0.0.1', 9991, NULL, NULL, 'eu-west', 'DE', 'typed-pool', 'active', 0.95,
               'source-typed-task', NULL, NULL, '1', '1'
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert active proxy");

    let payload = serde_json::json!({
        "provider": "typed-pool",
        "region": "eu-west",
        "limit": 1,
        "only_stale": true,
        "stale_after_seconds": 7200
    });
    let (create_status, _create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/proxies/verify-batch")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::ACCEPTED);
    let task_id: String = sqlx::query_scalar(
        r#"SELECT id FROM tasks WHERE kind = 'verify_proxy' ORDER BY created_at DESC, id DESC LIMIT 1"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("queued verify task id");

    let typed_row: (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT proxy_id, requested_region, proxy_mode FROM tasks WHERE id = ?"#)
            .bind(task_id)
            .fetch_one(&state.db)
            .await
            .expect("load typed task columns");
    assert_eq!(typed_row.0.as_deref(), Some("proxy-explicit-typed"));
    assert_eq!(typed_row.1.as_deref(), Some("eu-west"));
    assert_eq!(typed_row.2.as_deref(), Some("prod_live"));
}

#[tokio::test]
async fn status_exposes_recent_hot_regions_and_inventory_concentration() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string();

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-a', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 95.0, '1', '1'),
           ('source-b', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 93.0, '1', '1'),
           ('source-c', 'text_url', 'controlled_lab', 0, 1, 'external_probe_v2', 'medium', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 90.0, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source registry");

    for idx in 0..6 {
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   source_label, last_verify_status, last_verify_at, created_at, updated_at
               ) VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'ap-southeast', 'SG', 'pool-a', 'active', 0.95, 'source-a', 'ok', '9999999999', '1', '1')"#,
        )
        .bind(format!("proxy-source-a-{idx}"))
        .bind(9700_i64 + idx)
        .execute(&state.db)
        .await
        .expect("insert source-a proxy");
    }
    for idx in 0..5 {
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   source_label, last_verify_status, last_verify_at, created_at, updated_at
               ) VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'eu-west', 'DE', 'pool-b', 'active', 0.92, 'source-b', 'ok', '9999999999', '1', '1')"#,
        )
        .bind(format!("proxy-source-b-{idx}"))
        .bind(9800_i64 + idx)
        .execute(&state.db)
        .await
        .expect("insert source-b proxy");
    }
    for idx in 0..2 {
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   source_label, last_verify_status, last_verify_at, created_at, updated_at
               ) VALUES (?, 'http', '127.0.0.1', ?, NULL, NULL, 'cn', 'CN', 'pool-c', 'active', 0.88, 'source-c', 'ok', '9999999999', '1', '1')"#,
        )
        .bind(format!("proxy-source-c-{idx}"))
        .bind(9900_i64 + idx)
        .execute(&state.db)
        .await
        .expect("insert source-c proxy");
    }

    for (task_id, region) in [
        ("task-hot-1", "ap-southeast"),
        ("task-hot-2", "ap-southeast"),
        ("task-hot-3", "ap-southeast"),
        ("task-hot-4", "eu-west"),
        ("task-hot-5", "eu-west"),
        ("task-hot-6", "cn"),
    ] {
        let input_json = serde_json::json!({
            "url": format!("https://example.com/{task_id}"),
            "timeout_seconds": 5,
            "proxy_mode": "prod_live",
            "requested_region": region,
            "network_policy_json": {
                "mode": "required_proxy",
                "region": region
            }
        })
        .to_string();
        sqlx::query(
            r#"INSERT INTO tasks (
                   id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
                   proxy_id, requested_region, proxy_mode,
                   priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message
               ) VALUES (?, 'open_page', 'succeeded', ?, ?, NULL, NULL, ?, ?, 0, ?, ?, ?, ?, NULL, NULL, '{"status":"succeeded"}', NULL)"#,
        )
        .bind(task_id)
        .bind(&input_json)
        .bind(serde_json::json!({"mode": "required_proxy", "region": region}).to_string())
        .bind(region)
        .bind("prod_live")
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(&state.db)
        .await
        .expect("insert recent hot task");
    }

    let (status_code, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status_code, StatusCode::OK);
    let pool = status_json
        .get("proxy_pool_status")
        .expect("proxy pool status");
    assert_eq!(
        pool.get("hot_regions")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(0)
    );
    assert_eq!(
        pool.get("recent_hot_regions")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("ap-southeast")
    );
    assert_eq!(
        pool.get("recent_hot_region_counts")
            .and_then(|value| value.get("ap-southeast"))
            .and_then(Value::as_i64),
        Some(3)
    );
    assert_eq!(
        pool.get("hot_region_window_seconds")
            .and_then(Value::as_i64),
        Some(600)
    );
    let top1 = pool
        .get("source_concentration_top1_percent")
        .and_then(Value::as_f64)
        .expect("top1 concentration");
    let top3 = pool
        .get("source_concentration_top3_percent")
        .and_then(Value::as_f64)
        .expect("top3 concentration");
    assert!((top1 - 46.15).abs() < 0.2, "top1={top1}");
    assert!((top3 - 100.0).abs() < 0.1, "top3={top3}");
    assert_eq!(
        pool.get("active_sources_with_min_inventory")
            .and_then(Value::as_i64),
        Some(2)
    );
    assert_eq!(
        pool.get("active_regions_with_min_inventory")
            .and_then(Value::as_i64),
        Some(2)
    );
}

#[tokio::test]
async fn status_exposes_effective_geo_quality_fields_per_source() {
    let _mode_guard = ScopedEnvVar::set("PERSONA_PILOT_PROXY_MODE", "prod_live");
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO proxy_harvest_sources (
               source_label, source_kind, source_tier, for_demo, for_prod, validation_mode,
               expected_geo_quality, cost_class, enabled, config_json, interval_seconds, base_proxy_score,
               consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
               last_run_status, last_error, health_score, created_at, updated_at
           ) VALUES
           ('source-verified', 'text_url', 'private_paid', 0, 1, 'external_probe_v2', 'high', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 96.0, '1', '1'),
           ('source-host-inferred', 'text_url', 'controlled_lab', 0, 1, 'host_geo', 'unknown', 'paid', 1, '{}', 300, 1.0, 0, NULL, NULL, NULL, 'completed', NULL, 90.0, '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert source rows");

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               source_label, last_exit_country, last_exit_region, last_verify_source, last_verify_status, last_verify_at, created_at, updated_at
           ) VALUES
           ('proxy-verified-1', 'http', '127.0.0.1', 9951, NULL, NULL, 'us-east', 'US', 'pool-v', 'active', 0.95, 'source-verified', 'US', 'Virginia', 'local_verify', 'ok', '9999999999', '1', '1'),
           ('proxy-verified-2', 'http', '127.0.0.1', 9952, NULL, NULL, 'us-east', 'US', 'pool-v', 'active', 0.94, 'source-verified', 'US', 'Virginia', 'runner_verify', 'ok', '9999999999', '1', '1'),
           ('proxy-host-1', 'http', '127.0.0.1', 9953, NULL, NULL, 'eu-west', 'DE', 'pool-h', 'active', 0.88, 'source-host-inferred', 'DE', 'Berlin', 'imported_verify+geoip_host_enrich', 'ok', '9999999999', '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert proxy rows");

    let (_, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    let summaries = status_json
        .get("proxy_harvest_metrics")
        .and_then(|value| value.get("source_summaries"))
        .and_then(Value::as_array)
        .expect("source summaries");
    let verified = summaries
        .iter()
        .find(|item| item.get("source_label").and_then(Value::as_str) == Some("source-verified"))
        .expect("verified summary");
    assert_eq!(
        verified.get("declared_geo_quality").and_then(Value::as_str),
        Some("high")
    );
    assert_eq!(
        verified
            .get("effective_geo_quality")
            .and_then(Value::as_str),
        Some("externally_verified")
    );
    assert_eq!(
        verified.get("geo_coverage_percent").and_then(Value::as_f64),
        Some(100.0)
    );
    assert_eq!(
        verified.get("active_region_count").and_then(Value::as_i64),
        Some(1)
    );
    assert_eq!(
        verified.get("active_country_count").and_then(Value::as_i64),
        Some(1)
    );

    let host_inferred = summaries
        .iter()
        .find(|item| {
            item.get("source_label").and_then(Value::as_str) == Some("source-host-inferred")
        })
        .expect("host inferred summary");
    assert_eq!(
        host_inferred
            .get("effective_geo_quality")
            .and_then(Value::as_str),
        Some("host_geo_inferred")
    );
    assert_eq!(
        host_inferred
            .get("active_share_percent")
            .and_then(Value::as_f64),
        Some(33.33)
    );
}

#[tokio::test]
async fn status_exposes_proxy_site_metrics_and_storage_session_metrics() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    seed_active_proxy(&state.db, "proxy-site-a", "pool-a", "us-east").await;

    sqlx::query(
        r#"INSERT INTO proxy_session_bindings (
               session_key, proxy_id, provider, region, fingerprint_profile_id, site_key,
               requested_region, requested_provider, cookies_json, cookie_updated_at,
               local_storage_json, session_storage_json, storage_updated_at,
               last_success_at, last_failure_at, last_used_at, expires_at, created_at, updated_at
           ) VALUES (
               'session-site-a', 'proxy-site-a', 'pool-a', 'us-east', 'fp-site', 'example.com',
               'us-east', 'pool-a', '[]', '3', '{"theme":"dark"}', '{"step":"1"}', '3',
               '3', NULL, '3', '9999999999', '3', '3'
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert proxy session binding");

    sqlx::query(
        r#"INSERT INTO proxy_site_stats (
               proxy_id, site_key, success_count, failure_count,
               last_success_at, last_failure_at, last_failure_scope,
               last_browser_failure_signal, updated_at
           ) VALUES
           ('proxy-site-a', 'example.com', 2, 3, '10', '11', 'browser_execution', 'browser_navigation_failure_signal', '11'),
           ('proxy-site-a', 'static.example.com', 1, 0, '12', NULL, NULL, NULL, '12')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert proxy site stats");

    let task_one_result = serde_json::json!({
        "status": "succeeded",
        "identity_session_status": "auto_created",
        "cookie_restore_count": 0,
        "cookie_persist_count": 1,
        "local_storage_restore_count": 0,
        "local_storage_persist_count": 2,
        "session_storage_restore_count": 0,
        "session_storage_persist_count": 1
    })
    .to_string();
    let task_two_result = serde_json::json!({
        "status": "succeeded",
        "identity_session_status": "auto_reused",
        "cookie_restore_count": 1,
        "cookie_persist_count": 1,
        "local_storage_restore_count": 2,
        "local_storage_persist_count": 1,
        "session_storage_restore_count": 1,
        "session_storage_persist_count": 1
    })
    .to_string();

    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES
           ('task-site-metrics-1', 'open_page', 'succeeded', '{"url":"https://example.com/a","timeout_seconds":5}', '{"mode":"required_proxy","provider":"pool-a","region":"us-east"}', NULL, 0, '1', '1', '2', '3', NULL, NULL, ?, NULL),
           ('task-site-metrics-2', 'open_page', 'succeeded', '{"url":"https://example.com/b","timeout_seconds":5}', '{"mode":"required_proxy","provider":"pool-a","region":"us-east"}', NULL, 0, '4', '4', '5', '6', NULL, NULL, ?, NULL)"#,
    )
    .bind(&task_one_result)
    .bind(&task_two_result)
    .execute(&state.db)
    .await
    .expect("insert session metric tasks");

    let (status_code, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status_code, StatusCode::OK);
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("active_sessions"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("reused_sessions"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("created_sessions"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("cookie_restore_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("cookie_persist_count"))
            .and_then(Value::as_i64),
        Some(2),
    );
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("local_storage_restore_count"))
            .and_then(Value::as_i64),
        Some(2),
    );
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("local_storage_persist_count"))
            .and_then(Value::as_i64),
        Some(3),
    );
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("session_storage_restore_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        status_json
            .get("identity_session_metrics")
            .and_then(|value| value.get("session_storage_persist_count"))
            .and_then(Value::as_i64),
        Some(2),
    );
    assert_eq!(
        status_json
            .get("proxy_site_metrics")
            .and_then(|value| value.get("tracked_sites"))
            .and_then(Value::as_i64),
        Some(2),
    );
    assert_eq!(
        status_json
            .get("proxy_site_metrics")
            .and_then(|value| value.get("site_records"))
            .and_then(Value::as_i64),
        Some(2),
    );
    assert_eq!(
        status_json
            .get("proxy_site_metrics")
            .and_then(|value| value.get("top_failing_sites"))
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("example.com:3"),
    );
}

#[tokio::test]
async fn cancelled_contract_is_visible_across_status_detail_and_runs() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    let task_id = "task-cancelled-contract".to_string();
    let run_id = "run-cancelled-contract".to_string();
    let cancelled_result = serde_json::json!({
        "runner": "lightpanda",
        "ok": false,
        "status": "cancelled",
        "error_kind": "runner_cancelled",
        "failure_scope": "runner_cancelled",
        "execution_stage": "action",
        "task_id": task_id,
        "message": "task cancelled while running"
    })
    .to_string();

    sqlx::query(
        r#"INSERT INTO tasks (id, kind, status, input_json, network_policy_json, fingerprint_profile_json, priority, created_at, queued_at, started_at, finished_at, runner_id, heartbeat_at, result_json, error_message)
           VALUES (?, 'get_html', ?, '{"url":"https://example.com/cancelled-contract","timeout_seconds":60}', NULL, NULL, 0, '1', '1', '2', '3', NULL, NULL, ?, 'task cancelled while running')"#,
    )
    .bind(&task_id)
    .bind(TASK_STATUS_CANCELLED)
    .bind(&cancelled_result)
    .execute(&state.db)
    .await
    .expect("insert cancelled task");

    sqlx::query(
        r#"INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, result_json, error_message)
           VALUES (?, ?, ?, 1, 'lightpanda', '2', '3', ?, 'task cancelled while running')"#,
    )
    .bind(&run_id)
    .bind(&task_id)
    .bind(TASK_STATUS_CANCELLED)
    .bind(&cancelled_result)
    .execute(&state.db)
    .await
    .expect("insert cancelled run");

    let (task_status, task_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(task_status, StatusCode::OK);
    assert_eq!(
        task_json.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_CANCELLED)
    );
    assert_eq!(
        task_json.get("failure_scope").and_then(|v| v.as_str()),
        Some("runner_cancelled")
    );
    assert_eq!(
        task_json
            .get("summary_artifacts")
            .and_then(|v| v.as_array())
            .map(|v| !v.is_empty()),
        Some(true)
    );
    assert_eq!(
        task_json
            .get("summary_artifacts")
            .and_then(|v| v.as_array())
            .and_then(|items| items
                .iter()
                .find(|item| item.get("title").and_then(|v| v.as_str())
                    == Some("browser failure summary")))
            .and_then(|item| item.get("summary"))
            .and_then(|v| v.as_str())
            .map(|s| s.contains("execution_stage=action")),
        Some(true)
    );
    let task_execution_identity = task_json
        .get("execution_identity")
        .cloned()
        .expect("task execution identity");
    assert!(task_execution_identity.is_object());

    let (runs_status, runs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/runs"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(runs_status, StatusCode::OK);
    let runs = runs_json.as_array().expect("runs array");
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(
        run.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_CANCELLED)
    );
    assert_eq!(
        run.get("failure_scope").and_then(|v| v.as_str()),
        Some("runner_cancelled")
    );
    assert_eq!(
        run.get("execution_identity"),
        Some(&task_execution_identity)
    );
    assert_eq!(
        run.get("summary_artifacts")
            .and_then(|v| v.as_array())
            .and_then(|items| items
                .iter()
                .find(|item| item.get("title").and_then(|v| v.as_str())
                    == Some("browser failure summary")))
            .and_then(|item| item.get("summary"))
            .and_then(|v| v.as_str())
            .map(|s| s.contains("execution_stage=action")),
        Some(true)
    );
    let run_summary_artifacts = run
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("run summary artifacts");
    let task_summary_artifacts = task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("task summary artifacts");
    assert!(!run_summary_artifacts.is_empty());
    assert!(!task_summary_artifacts.is_empty());
    assert_eq!(
        run_summary_artifacts[0].get("key"),
        task_summary_artifacts[0].get("key")
    );
    assert_eq!(
        run_summary_artifacts[0].get("summary"),
        task_summary_artifacts[0].get("summary")
    );

    let (status_code, status_json) = json_response(
        &app,
        Request::builder()
            .uri("/status?limit=10&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status_code, StatusCode::OK);
    assert_eq!(
        status_json
            .get("counts")
            .and_then(|v| v.get("cancelled"))
            .and_then(|v| v.as_i64()),
        Some(1)
    );
    let latest_tasks = status_json
        .get("latest_tasks")
        .and_then(|v| v.as_array())
        .expect("latest tasks");
    let latest_task = latest_tasks
        .iter()
        .find(|item| item.get("id").and_then(|v| v.as_str()) == Some(task_id.as_str()))
        .expect("cancelled latest task");
    assert_eq!(
        latest_task.get("status").and_then(|v| v.as_str()),
        Some(TASK_STATUS_CANCELLED)
    );
    assert_eq!(
        latest_task.get("failure_scope").and_then(|v| v.as_str()),
        Some("runner_cancelled")
    );
    assert_eq!(
        latest_task.get("execution_identity"),
        Some(&task_execution_identity)
    );
    assert_eq!(
        latest_task
            .get("summary_artifacts")
            .and_then(|v| v.as_array())
            .and_then(|items| items
                .iter()
                .find(|item| item.get("title").and_then(|v| v.as_str())
                    == Some("browser failure summary")))
            .and_then(|item| item.get("summary"))
            .and_then(|v| v.as_str())
            .map(|s| s.contains("execution_stage=action")),
        Some(true)
    );

    let latest_execution_summaries = status_json
        .get("latest_execution_summaries")
        .and_then(|v| v.as_array())
        .expect("latest execution summaries");
    assert!(latest_execution_summaries.iter().any(|item| {
        item.get("task_id").and_then(|v| v.as_str()) == Some(task_id.as_str())
            && item
                .get("summary")
                .and_then(|v| v.as_str())
                .map(|s| {
                    (s.contains("failure_scope=runner_cancelled") || s.contains("runner_cancelled"))
                        && s.contains("execution_stage=action")
                })
                .unwrap_or(false)
    }));
}

#[tokio::test]
async fn launch_result_contract_is_preserved_in_task_and_run_detail() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");

    sqlx::query(
        r#"INSERT INTO fingerprint_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
           VALUES ('fp-launch-result', 'Launch Result', 1, 'active', NULL, '{"browser":{"name":"chrome","version":"124"}}', '1', '1')"#,
    )
    .execute(&state.db)
    .await
    .expect("insert active fingerprint profile");
    seed_active_proxy(
        &state.db,
        "proxy-launch-result",
        "launch-contract",
        "us-east",
    )
    .await;

    let payload = serde_json::json!({
        "kind": "get_title",
        "url": "https://example.com/launch-result",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-launch-result",
        "network_policy_json": {"mode": "required_proxy", "proxy_id": "proxy-launch-result"}
    });
    let (create_status, create_body) = text_response(
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
    let create_json: Value = serde_json::from_str(&create_body).expect("create task json body");
    let task_id = create_json
        .get("id")
        .and_then(|v| v.as_str())
        .expect("task id")
        .to_string();
    let task_json = wait_for_terminal_status(&app, &task_id).await;

    let result_json_text: Option<String> =
        sqlx::query_scalar(r#"SELECT result_json FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load task result json");
    let result_json: Value =
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result json");
    assert_eq!(result_json.get("title"), task_json.get("title"));
    assert_eq!(result_json.get("final_url"), task_json.get("final_url"));
    assert_eq!(
        result_json.get("content_kind"),
        task_json.get("content_kind")
    );
    assert_eq!(
        result_json.get("content_source_action"),
        task_json.get("content_source_action")
    );
    assert_eq!(
        result_json.get("content_ready"),
        task_json.get("content_ready")
    );

    let (status, runs_json) = json_response(
        &app,
        Request::builder()
            .uri(format!("/tasks/{task_id}/runs"))
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let runs = runs_json.as_array().expect("runs array");
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run.get("status"), task_json.get("status"));
    assert_eq!(run.get("title"), task_json.get("title"));
    assert_eq!(run.get("final_url"), task_json.get("final_url"));
    assert_eq!(run.get("content_kind"), task_json.get("content_kind"));
    assert_eq!(
        run.get("content_source_action"),
        task_json.get("content_source_action")
    );
    assert_eq!(run.get("content_ready"), task_json.get("content_ready"));
    let run_summary_artifacts = run
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("run summary artifacts");
    let task_summary_artifacts = task_json
        .get("summary_artifacts")
        .and_then(|v| v.as_array())
        .expect("task summary artifacts");
    assert_eq!(run_summary_artifacts.len(), task_summary_artifacts.len());
    for (run_artifact, task_artifact) in run_summary_artifacts
        .iter()
        .zip(task_summary_artifacts.iter())
    {
        assert_eq!(run_artifact.get("key"), task_artifact.get("key"));
        assert_eq!(run_artifact.get("title"), task_artifact.get("title"));
        assert_eq!(run_artifact.get("summary"), task_artifact.get("summary"));
    }
}

#[tokio::test]
async fn platform_template_crud_roundtrips_identity_markers_json() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "id": "tpl-xhs-identity-markers",
        "platform_id": "xiaohongshu-crud",
        "name": "XHS Identity Marker Template",
        "warm_paths_json": ["/dashboard"],
        "revisit_paths_json": ["/dashboard", "/notes"],
        "stateful_paths_json": ["/dashboard", "/notes"],
        "write_operation_paths_json": ["/publish", "/notes"],
        "high_risk_paths_json": ["/security", "/finance"],
        "allowed_regions_json": ["CN-31"],
        "preferred_locale": "zh-CN",
        "preferred_timezone": "Asia/Shanghai",
        "continuity_checks_json": ["login_state", "identity", "region", "dashboard", "notes"],
        "identity_markers_json": ["store-identity-marker", "创作中心"],
        "login_loss_signals_json": ["login"],
        "recovery_steps_json": ["reload", "revisit"],
        "readiness_level": "sample_ready"
    });

    let (create_status, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/platform-templates")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);
    assert_eq!(
        create_json
            .get("identity_markers_json")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(2)
    );
    assert_eq!(
        create_json
            .get("identity_markers_json")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("store-identity-marker")
    );

    let (get_status, get_json) = json_response(
        &app,
        Request::builder()
            .uri("/platform-templates/tpl-xhs-identity-markers")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(
        get_json
            .get("identity_markers_json")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(Value::as_str),
        Some("创作中心")
    );

    let (list_status, list_json) = json_response(
        &app,
        Request::builder()
            .uri("/platform-templates?limit=50&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(list_status, StatusCode::OK);
    let list_item = list_json
        .as_array()
        .expect("platform template list")
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some("tpl-xhs-identity-markers"))
        .expect("created platform template in list");
    assert_eq!(
        list_item
            .get("identity_markers_json")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("store-identity-marker")
    );
}

#[tokio::test]
async fn store_platform_override_crud_roundtrips_identity_markers_json() {
    let db_url = unique_db_url();
    let (_state, app) = build_test_app(&db_url).await.expect("build app");

    let payload = serde_json::json!({
        "id": "override-xhs-identity-markers",
        "store_id": "store-001",
        "platform_id": "xiaohongshu",
        "admin_origin": "https://seller.xiaohongshu.com",
        "entry_origin": "https://seller.xiaohongshu.com",
        "entry_paths_json": ["/dashboard", "/notes"],
        "revisit_paths_json": ["/dashboard", "/notes"],
        "identity_markers_json": ["store-identity-marker", "店铺后台"],
        "login_loss_signals_json": ["login"]
    });

    let (create_status, create_json) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/store-platform-overrides")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(create_status, StatusCode::CREATED);
    assert_eq!(
        create_json.get("admin_origin").and_then(Value::as_str),
        Some("https://seller.xiaohongshu.com")
    );
    assert_eq!(
        create_json
            .get("identity_markers_json")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("store-identity-marker")
    );

    let (get_status, get_json) = json_response(
        &app,
        Request::builder()
            .uri("/store-platform-overrides/override-xhs-identity-markers")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(get_status, StatusCode::OK);
    assert_eq!(
        get_json
            .get("entry_paths_json")
            .and_then(Value::as_array)
            .map(|items| items.len()),
        Some(2)
    );
    assert_eq!(
        get_json
            .get("identity_markers_json")
            .and_then(Value::as_array)
            .and_then(|items| items.get(1))
            .and_then(Value::as_str),
        Some("店铺后台")
    );

    let (list_status, list_json) = json_response(
        &app,
        Request::builder()
            .uri("/store-platform-overrides?limit=50&offset=0")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(list_status, StatusCode::OK);
    let list_item = list_json
        .as_array()
        .expect("override list")
        .iter()
        .find(|item| {
            item.get("id").and_then(Value::as_str) == Some("override-xhs-identity-markers")
        })
        .expect("created override in list");
    assert_eq!(
        list_item
            .get("identity_markers_json")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(Value::as_str),
        Some("store-identity-marker")
    );
}
