use std::{sync::{Arc, OnceLock}, time::{Duration, SystemTime, UNIX_EPOCH}};

use axum::{body::Body, http::{Request, StatusCode}};
use AutoOpenBrowser::{
    api::routes::build_router,
    app::build_app_state,
    db::init::init_db,
    domain::{
        run::{RUN_STATUS_CANCELLED, RUN_STATUS_FAILED, RUN_STATUS_TIMED_OUT},
        task::{TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT},
    },
    runner::{lightpanda::LightpandaRunner, spawn_runner_workers},
};
use serde_json::Value;
use tokio::sync::Mutex;
use tower::ServiceExt;

fn unique_db_url() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("sqlite:///tmp/auto_open_browser_lightpanda_test_{nanos}.db")
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

async fn wait_for_terminal_status(app: &axum::Router, task_id: &str) -> Value {
    for _ in 0..40 {
        let (_, json) = json_response(
            app,
            Request::builder()
                .uri(format!("/tasks/{task_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        let status = json.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if !matches!(status, "queued" | "running") {
            return json;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("task did not reach terminal status in time");
}

async fn build_lightpanda_test_app(database_url: &str) -> anyhow::Result<(AutoOpenBrowser::app::state::AppState, axum::Router)> {
    let db = init_db(database_url).await?;
    let runner: Arc<dyn AutoOpenBrowser::runner::TaskRunner> = Arc::new(LightpandaRunner::default());
    let state = build_app_state(db, runner.clone(), None, 1);
    spawn_runner_workers(state.clone(), runner, 1).await;
    let app = build_router(state.clone());
    Ok((state, app))
}

async fn create_task(app: &axum::Router, kind: &str, url: &str, timeout_seconds: i64) -> String {
    let payload = serde_json::json!({
        "kind": kind,
        "url": url,
        "timeout_seconds": timeout_seconds
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

fn lightpanda_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn make_stub_script(name: &str, body: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{name}_{nanos}.sh"));
    std::fs::write(&path, body).expect("write stub script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
    path
}

#[tokio::test]
async fn lightpanda_runner_success_persists_result_and_env_injection() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_success",
        "#!/usr/bin/env bash\nset -euo pipefail\nurl=\"${@: -1}\"\nproxy=\"http://${LIGHTPANDA_PROXY_USERNAME:-}:${LIGHTPANDA_PROXY_PASSWORD:-}@${LIGHTPANDA_PROXY_HOST:-}:${LIGHTPANDA_PROXY_PORT:-}\"\nprintf '{\"url\":\"%s\",\"proxy\":\"%s\",\"timezone\":\"%s\",\"user_agent\":\"%s\"}' \"$url\" \"$proxy\" \"${LIGHTPANDA_FP_TIMEZONE:-}\" \"${LIGHTPANDA_FP_USER_AGENT:-}\"\n",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");

    sqlx::query(r#"INSERT INTO fingerprint_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
           VALUES ('fp-lightpanda', 'Lightpanda FP', 1, 'active', NULL, '{"timezone":"Asia/Singapore","user_agent":"UA-Test/1.0"}', '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert fp");
    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, created_at, updated_at)
           VALUES ('proxy-lightpanda', 'http', '127.0.0.1', 8080, 'u', 'p', 'sg', 'SG', 'manual', 'active', 0.9, 0, 0, '1', '1')"#)
        .execute(&state.db)
        .await
        .expect("insert proxy");

    let payload = serde_json::json!({
        "kind": "get_html",
        "url": "https://example.com/success",
        "timeout_seconds": 5,
        "fingerprint_profile_id": "fp-lightpanda",
        "network_policy_json": {"resolved_proxy": {
            "id": "proxy-lightpanda", "scheme": "http", "host": "127.0.0.1", "port": 8080,
            "username": "u", "password": "p", "region": "sg", "country": "SG", "provider": "manual", "score": 0.9
        }}
    });
    let (status, json) = json_response(
        &app,
        Request::builder().method("POST").uri("/tasks").header("content-type", "application/json").body(Body::from(payload.to_string())).expect("request"),
    ).await;
    assert_eq!(status, StatusCode::CREATED);
    let task_id = json.get("id").and_then(|v| v.as_str()).expect("task id").to_string();

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));

    let (task_status, run_status, result_json_text, error_message, runner_id, heartbeat_at): (String, String, Option<String>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT t.status, r.status, t.result_json, t.error_message, t.runner_id, t.heartbeat_at
                          FROM tasks t JOIN runs r ON r.task_id = t.id WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load success rows");
    assert_eq!(task_status, TASK_STATUS_SUCCEEDED);
    assert_eq!(run_status, TASK_STATUS_SUCCEEDED);
    assert!(error_message.is_none());
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());

    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse result");
    assert_eq!(result_json.get("status").and_then(|v| v.as_str()), Some("succeeded"));
    assert_eq!(result_json.get("proxy").and_then(|v| v.get("host")).and_then(|v| v.as_str()), Some("127.0.0.1"));
    assert!(result_json.get("stdout_preview").and_then(|v| v.as_str()).unwrap_or_default().contains("Asia/Singapore"));
    assert!(result_json.get("stdout_preview").and_then(|v| v.as_str()).unwrap_or_default().contains("http://u:p@127.0.0.1:8080"));
    assert!(result_json.get("stdout_preview").and_then(|v| v.as_str()).unwrap_or_default().contains("UA-Test/1.0"));

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}

#[tokio::test]
async fn lightpanda_runner_timeout_marks_timed_out_and_cleans_state() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_timeout",
        "#!/usr/bin/env bash\nset -euo pipefail\nsleep 5\n",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_task(&app, "get_html", "https://example.com/timeout", 1).await;

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_TIMED_OUT));

    let (task_status, run_status, result_json_text, error_message, runner_id, heartbeat_at): (String, String, Option<String>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT t.status, r.status, t.result_json, t.error_message, t.runner_id, t.heartbeat_at
                          FROM tasks t JOIN runs r ON r.task_id = t.id WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load timeout rows");
    assert_eq!(task_status, TASK_STATUS_TIMED_OUT);
    assert_eq!(run_status, RUN_STATUS_TIMED_OUT);
    assert!(error_message.unwrap_or_default().contains("timed out"));
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());
    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse timeout result");
    assert_eq!(result_json.get("failure_scope").and_then(|v| v.as_str()), Some("runner_timeout"));
    assert_eq!(result_json.get("execution_stage").and_then(|v| v.as_str()), Some("navigate"));

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}

#[tokio::test]
async fn lightpanda_runner_browser_navigation_failure_maps_to_browser_execution() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_browser_navigation_failure",
        "#!/usr/bin/env bash
set -euo pipefail
printf 'navigation failed while opening page\n' >&2
exit 1
",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_task(&app, "open_page", "https://example.com/navigation-fail", 5).await;

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_FAILED));

    let (task_status, run_status, result_json_text, error_message, runner_id, heartbeat_at): (String, String, Option<String>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT t.status, r.status, t.result_json, t.error_message, t.runner_id, t.heartbeat_at
                          FROM tasks t JOIN runs r ON r.task_id = t.id WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load browser execution rows");
    assert_eq!(task_status, TASK_STATUS_FAILED);
    assert_eq!(run_status, RUN_STATUS_FAILED);
    let error_message = error_message.unwrap_or_default();
    assert!(!error_message.is_empty());
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());

    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse browser execution result");
    assert_eq!(result_json.get("status").and_then(|v| v.as_str()), Some(RUN_STATUS_FAILED));
    assert_eq!(result_json.get("error_kind").and_then(|v| v.as_str()), Some("runner_non_zero_exit"));
    assert_eq!(result_json.get("failure_scope").and_then(|v| v.as_str()), Some("browser_execution"));
    assert_eq!(
        result_json.get("browser_failure_signal").and_then(|v| v.as_str()),
        Some("browser_navigation_failure_signal")
    );
    assert_eq!(result_json.get("execution_stage").and_then(|v| v.as_str()), Some("navigate"));
    assert!(
        result_json
            .get("stderr_preview")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("navigation failed")
    );

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}

#[tokio::test]
async fn lightpanda_runner_browser_dns_failure_maps_to_browser_execution() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_browser_dns_failure",
        "#!/usr/bin/env bash
set -euo pipefail
printf 'dns name not resolved while opening page\n' >&2
exit 1
",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_task(&app, "open_page", "https://example.com/dns-fail", 5).await;

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_FAILED));

    let (task_status, run_status, result_json_text, error_message, runner_id, heartbeat_at): (String, String, Option<String>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT t.status, r.status, t.result_json, t.error_message, t.runner_id, t.heartbeat_at
                          FROM tasks t JOIN runs r ON r.task_id = t.id WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load browser dns rows");
    assert_eq!(task_status, TASK_STATUS_FAILED);
    assert_eq!(run_status, RUN_STATUS_FAILED);
    let error_message = error_message.unwrap_or_default();
    assert!(!error_message.is_empty());
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());

    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse browser dns result");
    assert_eq!(result_json.get("status").and_then(|v| v.as_str()), Some(RUN_STATUS_FAILED));
    assert_eq!(result_json.get("error_kind").and_then(|v| v.as_str()), Some("runner_non_zero_exit"));
    assert_eq!(result_json.get("failure_scope").and_then(|v| v.as_str()), Some("browser_execution"));
    assert_eq!(
        result_json.get("browser_failure_signal").and_then(|v| v.as_str()),
        Some("browser_dns_failure_signal")
    );
    assert_eq!(result_json.get("execution_stage").and_then(|v| v.as_str()), Some("navigate"));
    assert!(
        result_json
            .get("stderr_preview")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("dns name not resolved")
    );

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}

#[tokio::test]
async fn lightpanda_runner_browser_tls_failure_maps_to_browser_execution() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_browser_tls_failure",
        "#!/usr/bin/env bash
set -euo pipefail
printf 'tls certificate failure while opening page\n' >&2
exit 1
",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_task(&app, "open_page", "https://example.com/tls-fail", 5).await;

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_FAILED));

    let (task_status, run_status, result_json_text, error_message, runner_id, heartbeat_at): (String, String, Option<String>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT t.status, r.status, t.result_json, t.error_message, t.runner_id, t.heartbeat_at
                          FROM tasks t JOIN runs r ON r.task_id = t.id WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load browser tls rows");
    assert_eq!(task_status, TASK_STATUS_FAILED);
    assert_eq!(run_status, RUN_STATUS_FAILED);
    let error_message = error_message.unwrap_or_default();
    assert!(!error_message.is_empty());
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());

    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse browser tls result");
    assert_eq!(result_json.get("status").and_then(|v| v.as_str()), Some(RUN_STATUS_FAILED));
    assert_eq!(result_json.get("error_kind").and_then(|v| v.as_str()), Some("runner_non_zero_exit"));
    assert_eq!(result_json.get("failure_scope").and_then(|v| v.as_str()), Some("browser_execution"));
    assert_eq!(
        result_json.get("browser_failure_signal").and_then(|v| v.as_str()),
        Some("browser_tls_failure_signal")
    );
    assert_eq!(result_json.get("execution_stage").and_then(|v| v.as_str()), Some("navigate"));
    assert!(
        result_json
            .get("stderr_preview")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("tls certificate failure")
    );

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}

#[tokio::test]
async fn lightpanda_runner_success_maps_to_no_browser_failure_signal() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_browser_success_control",
        r#"#!/usr/bin/env bash
set -euo pipefail
printf '{"ok":true,"url":"https://example.com/success-control"}'
"#,
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_task(&app, "open_page", "https://example.com/success-control", 5).await;

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_SUCCEEDED));

    let (task_status, run_status, result_json_text, error_message, runner_id, heartbeat_at): (String, String, Option<String>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT t.status, r.status, t.result_json, t.error_message, t.runner_id, t.heartbeat_at
                          FROM tasks t JOIN runs r ON r.task_id = t.id WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load success control rows");
    assert_eq!(task_status, TASK_STATUS_SUCCEEDED);
    assert_eq!(run_status, TASK_STATUS_SUCCEEDED);
    assert!(error_message.is_none());
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());

    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse success control result");
    assert_eq!(result_json.get("status").and_then(|v| v.as_str()), Some("succeeded"));
    assert_eq!(result_json.get("failure_scope"), Some(&Value::Null));
    assert_eq!(result_json.get("browser_failure_signal"), Some(&Value::Null));
    assert_eq!(result_json.get("execution_stage").and_then(|v| v.as_str()), Some("action"));

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}

#[tokio::test]
async fn lightpanda_runner_non_zero_exit_marks_failed() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_fail",
        "#!/usr/bin/env bash\nset -euo pipefail\necho runner crashed >&2\nexit 42\n",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_task(&app, "get_title", "https://example.com/fail", 5).await;

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_FAILED));

    let (task_status, run_status, result_json_text, error_message): (String, String, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT t.status, r.status, t.result_json, t.error_message
                          FROM tasks t JOIN runs r ON r.task_id = t.id WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load failure rows");
    assert_eq!(task_status, TASK_STATUS_FAILED);
    assert_eq!(run_status, RUN_STATUS_FAILED);
    assert!(error_message.unwrap_or_default().contains("non-zero status"));
    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse result");
    assert_eq!(result_json.get("error_kind").and_then(|v| v.as_str()), Some("runner_non_zero_exit"));
    assert_eq!(result_json.get("failure_scope").and_then(|v| v.as_str()), Some("runner_process_exit"));
    assert_eq!(result_json.get("execution_stage").and_then(|v| v.as_str()), Some("action"));
    assert_eq!(result_json.get("exit_code").and_then(|v| v.as_i64()), Some(42));
    assert!(result_json.get("stderr_preview").and_then(|v| v.as_str()).unwrap_or_default().contains("runner crashed"));

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}

#[tokio::test]
async fn lightpanda_running_cancel_marks_cancelled() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_cancel",
        "#!/usr/bin/env bash\nset -euo pipefail\ntrap 'exit 143' TERM\nwhile true; do sleep 1; done\n",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_task(&app, "fetch", "https://example.com/cancel", 10).await;

    for _ in 0..20 {
        let (_, task_json) = json_response(
            &app,
            Request::builder().uri(format!("/tasks/{task_id}")).body(Body::empty()).expect("request"),
        ).await;
        if task_json.get("status").and_then(|v| v.as_str()) == Some("running") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    let (cancel_status, cancel_json) = json_response(
        &app,
        Request::builder().method("POST").uri(format!("/tasks/{task_id}/cancel")).body(Body::empty()).expect("request"),
    ).await;
    assert_eq!(cancel_status, StatusCode::OK);
    assert_eq!(cancel_json.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_CANCELLED));

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(task.get("status").and_then(|v| v.as_str()), Some(TASK_STATUS_CANCELLED));

    let (task_status, run_status, result_json_text, task_error, runner_id, heartbeat_at): (String, String, Option<String>, Option<String>, Option<String>, Option<String>) =
        sqlx::query_as(r#"SELECT t.status, r.status, t.result_json, t.error_message, t.runner_id, t.heartbeat_at
                          FROM tasks t JOIN runs r ON r.task_id = t.id WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await
            .expect("load cancel rows");
    assert_eq!(task_status, TASK_STATUS_CANCELLED);
    assert_eq!(run_status, RUN_STATUS_CANCELLED);
    assert_eq!(task_error.as_deref(), Some("task cancelled while running"));
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());
    let result_json: Value = serde_json::from_str(result_json_text.as_deref().expect("result json")).expect("parse cancel result");
    assert_eq!(result_json.get("status").and_then(|v| v.as_str()), Some("cancelled"));
    assert_eq!(result_json.get("failure_scope").and_then(|v| v.as_str()), Some("runner_cancelled"));
    assert_eq!(result_json.get("error_kind").and_then(|v| v.as_str()), Some("runner_cancelled"));
    assert_eq!(result_json.get("execution_stage").and_then(|v| v.as_str()), Some("action"));

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}
