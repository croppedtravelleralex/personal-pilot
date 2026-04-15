use std::{
    sync::{Arc, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{net::TcpListener, sync::Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tower::ServiceExt;
use persona_pilot::{
    api::routes::build_router,
    app::build_app_state,
    db::init::init_db,
    domain::{
        run::{
            RUN_STATUS_CANCELLED, RUN_STATUS_FAILED, RUN_STATUS_SUCCEEDED, RUN_STATUS_TIMED_OUT,
        },
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
    runner::{lightpanda::LightpandaRunner, spawn_runner_workers},
};

const DEFAULT_LIGHTPANDA_TEST_PROXY_ID: &str = "proxy-lightpanda-default";

fn unique_db_url() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("sqlite:///tmp/persona_pilot_lightpanda_test_{nanos}.db")
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
        let status = json.get("status").and_then(Value::as_str).unwrap_or("");
        if !matches!(status, "queued" | "running") {
            return json;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("task did not reach terminal status in time");
}

async fn build_lightpanda_test_app(
    database_url: &str,
) -> anyhow::Result<(persona_pilot::app::state::AppState, axum::Router)> {
    let db = init_db(database_url).await?;
    seed_active_proxy(&db, DEFAULT_LIGHTPANDA_TEST_PROXY_ID, "lp-test", "us-east").await;
    let runner: Arc<dyn persona_pilot::runner::TaskRunner> =
        Arc::new(LightpandaRunner::default());
    let state = build_app_state(db, runner.clone(), None, 1);
    spawn_runner_workers(state.clone(), runner, 1).await;
    let app = build_router(state.clone());
    Ok((state, app))
}

async fn seed_active_proxy(db: &sqlx::SqlitePool, proxy_id: &str, provider: &str, region: &str) {
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

async fn create_browser_task(
    app: &axum::Router,
    endpoint: &str,
    url: &str,
    timeout_seconds: i64,
) -> String {
    let payload = json!({
        "url": url,
        "timeout_seconds": timeout_seconds,
        "network_policy_json": {
            "mode": "required_proxy",
            "proxy_id": DEFAULT_LIGHTPANDA_TEST_PROXY_ID
        }
    });
    let (status, json) = json_response(
        app,
        Request::builder()
            .method("POST")
            .uri(endpoint)
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    json.get("id")
        .and_then(Value::as_str)
        .expect("task id")
        .to_string()
}

async fn create_browser_task_with_payload(
    app: &axum::Router,
    endpoint: &str,
    payload: Value,
) -> String {
    let (status, json) = json_response(
        app,
        Request::builder()
            .method("POST")
            .uri(endpoint)
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    json.get("id")
        .and_then(Value::as_str)
        .expect("task id")
        .to_string()
}

async fn load_result_row(
    state: &persona_pilot::app::state::AppState,
    task_id: &str,
) -> (
    String,
    String,
    Value,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let (task_status, run_status, result_json_text, error_message, runner_id, heartbeat_at): (
        String,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = sqlx::query_as(
        r#"SELECT t.status, r.status, t.result_json, t.error_message, t.runner_id, t.heartbeat_at
           FROM tasks t JOIN runs r ON r.task_id = t.id
           WHERE t.id = ? ORDER BY r.attempt DESC LIMIT 1"#,
    )
    .bind(task_id)
    .fetch_one(&state.db)
    .await
    .expect("load task row");

    (
        task_status,
        run_status,
        serde_json::from_str(result_json_text.as_deref().expect("result json"))
            .expect("parse result"),
        error_message,
        runner_id,
        heartbeat_at,
    )
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
    let normalized_body = if cfg!(unix) {
        body.replacen("#!/usr/bin/env bash", "#!/bin/bash", 1)
    } else {
        body.to_string()
    };
    std::fs::write(&path, normalized_body).expect("write stub script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
    path
}

#[derive(Clone)]
enum FakeCdpScenario {
    Success {
        title: &'static str,
        final_url: &'static str,
        html: &'static str,
        text: &'static str,
        cookies: Vec<Value>,
        local_storage: Value,
        session_storage: Value,
    },
    NavigateError {
        error_text: &'static str,
    },
}

struct FakeCdpServer {
    endpoint: String,
    handle: tokio::task::JoinHandle<()>,
    requests: Arc<Mutex<Vec<Value>>>,
}

impl FakeCdpServer {
    async fn start(scenario: FakeCdpScenario) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind fake cdp");
        let addr = listener.local_addr().expect("local addr");
        let endpoint = format!("ws://127.0.0.1:{}/", addr.port());
        let requests = Arc::new(Mutex::new(Vec::<Value>::new()));
        let requests_for_task = requests.clone();
        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut ws = accept_async(stream).await.expect("upgrade ws");
            while let Some(message) = ws.next().await {
                let Ok(message) = message else { break };
                let Message::Text(text) = message else {
                    continue;
                };
                let payload: Value = serde_json::from_str(&text).expect("request json");
                requests_for_task.lock().await.push(payload.clone());
                let id = payload.get("id").and_then(Value::as_u64).expect("id");
                let method = payload
                    .get("method")
                    .and_then(Value::as_str)
                    .expect("method");
                let session_id = payload
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);

                let result = match method {
                    "Target.createTarget" => json!({ "targetId": "target-1" }),
                    "Target.attachToTarget" => json!({ "sessionId": "session-1" }),
                    "Page.enable" | "Runtime.enable" | "Network.enable" | "Network.setCookies" => {
                        json!({})
                    }
                    "Network.getCookies" => match &scenario {
                        FakeCdpScenario::Success { cookies, .. } => json!({ "cookies": cookies }),
                        FakeCdpScenario::NavigateError { .. } => json!({ "cookies": [] }),
                    },
                    "Page.navigate" => match &scenario {
                        FakeCdpScenario::Success { .. } => {
                            json!({ "frameId": "frame-1", "loaderId": "loader-1" })
                        }
                        FakeCdpScenario::NavigateError { error_text } => json!({
                            "frameId": "frame-1",
                            "loaderId": "loader-1",
                            "errorText": error_text
                        }),
                    },
                    "Runtime.evaluate" => {
                        let expression = payload
                            .get("params")
                            .and_then(|value| value.get("expression"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        match &scenario {
                            FakeCdpScenario::Success {
                                title,
                                final_url,
                                html,
                                text,
                                ..
                            } if expression.contains("readyState") => json!({
                                "result": {
                                    "type": "object",
                                    "value": {
                                        "readyState": "complete",
                                        "title": title,
                                        "href": final_url,
                                        "hasHtml": true,
                                        "hasBody": true
                                    }
                                }
                            }),
                            FakeCdpScenario::Success {
                                title,
                                final_url,
                                html,
                                ..
                            } if expression.contains("outerHTML") => json!({
                                "result": {
                                    "type": "object",
                                    "value": {
                                        "title": title,
                                        "href": final_url,
                                        "html": html
                                    }
                                }
                            }),
                            FakeCdpScenario::Success {
                                title,
                                final_url,
                                text,
                                ..
                            } if expression.contains("innerText") => json!({
                                "result": {
                                    "type": "object",
                                    "value": {
                                        "title": title,
                                        "href": final_url,
                                        "text": text
                                    }
                                }
                            }),
                            FakeCdpScenario::Success {
                                local_storage,
                                session_storage,
                                ..
                            } if expression.contains("__PP_STORAGE_SNAPSHOT__") => json!({
                                "result": {
                                    "type": "object",
                                    "value": {
                                        "localStorage": local_storage,
                                        "sessionStorage": session_storage
                                    }
                                }
                            }),
                            FakeCdpScenario::Success { .. }
                                if expression.contains("__PP_STORAGE_RESTORE__") =>
                            {
                                json!({
                                    "result": {
                                        "type": "object",
                                        "value": {
                                            "localStorageRestoreCount": 1,
                                            "sessionStorageRestoreCount": 1
                                        }
                                    }
                                })
                            }
                            _ => json!({ "result": { "type": "object", "value": {} } }),
                        }
                    }
                    _ => json!({}),
                };

                let mut response = json!({ "id": id, "result": result });
                if let Some(session_id) = session_id {
                    response["sessionId"] = Value::String(session_id);
                }
                ws.send(Message::Text(response.to_string().into()))
                    .await
                    .expect("send response");
            }
        });

        Self {
            endpoint,
            handle,
            requests,
        }
    }

    async fn recorded_requests(&self) -> Vec<Value> {
        self.requests.lock().await.clone()
    }

    async fn stop(self) {
        self.handle.abort();
        let _ = self.handle.await;
    }
}

fn sample_page() -> FakeCdpScenario {
    FakeCdpScenario::Success {
        title: "Example Domain",
        final_url: "https://example.com/final",
        html: "<html><body><h1>Example Domain</h1></body></html>",
        text: "Example Domain",
        cookies: Vec::new(),
        local_storage: json!({}),
        session_storage: json!({}),
    }
}

#[tokio::test]
async fn lightpanda_runner_open_page_success_returns_title_and_final_url() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let server = FakeCdpServer::start(sample_page()).await;
    std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &server.endpoint);

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_browser_task(&app, "/browser/open", "https://example.com/open", 5).await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_SUCCEEDED)
    );

    let (task_status, run_status, result_json, error_message, runner_id, heartbeat_at) =
        load_result_row(&state, &task_id).await;
    assert_eq!(task_status, TASK_STATUS_SUCCEEDED);
    assert_eq!(run_status, RUN_STATUS_SUCCEEDED);
    assert!(error_message.is_none());
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());
    assert_eq!(
        result_json.get("status").and_then(Value::as_str),
        Some("succeeded")
    );
    assert_eq!(
        result_json.get("title").and_then(Value::as_str),
        Some("Example Domain")
    );
    assert_eq!(
        result_json.get("final_url").and_then(Value::as_str),
        Some("https://example.com/final")
    );
    assert_eq!(result_json.get("content_kind"), Some(&Value::Null));

    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");
    server.stop().await;
}

#[tokio::test]
async fn lightpanda_runner_get_html_and_text_actions_fill_content_fields() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");

    let html_server = FakeCdpServer::start(sample_page()).await;
    std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &html_server.endpoint);
    let html_task_id =
        create_browser_task(&app, "/browser/html", "https://example.com/html", 5).await;
    let html_task = wait_for_terminal_status(&app, &html_task_id).await;
    assert_eq!(
        html_task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_SUCCEEDED)
    );
    let (_, _, html_result, _, _, _) = load_result_row(&state, &html_task_id).await;
    assert_eq!(
        html_result.get("content_kind").and_then(Value::as_str),
        Some("text/html")
    );
    assert_eq!(
        html_result
            .get("content_source_action")
            .and_then(Value::as_str),
        Some("get_html")
    );
    assert_eq!(
        html_result.get("content_ready").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        html_result.get("html_preview").and_then(Value::as_str),
        Some("<html><body><h1>Example Domain</h1></body></html>")
    );
    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");
    html_server.stop().await;

    let text_server = FakeCdpServer::start(sample_page()).await;
    std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &text_server.endpoint);
    let text_task_id =
        create_browser_task(&app, "/browser/text", "https://example.com/text", 5).await;
    let text_task = wait_for_terminal_status(&app, &text_task_id).await;
    assert_eq!(
        text_task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_SUCCEEDED)
    );
    let (_, _, text_result, _, _, _) = load_result_row(&state, &text_task_id).await;
    assert_eq!(
        text_result.get("content_kind").and_then(Value::as_str),
        Some("text/plain")
    );
    assert_eq!(
        text_result
            .get("content_source_action")
            .and_then(Value::as_str),
        Some("extract_text")
    );
    assert_eq!(
        text_result.get("content_ready").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        text_result.get("text_preview").and_then(Value::as_str),
        Some("Example Domain")
    );

    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");
    text_server.stop().await;
}

#[tokio::test]
async fn lightpanda_runner_get_title_and_final_url_actions_fill_minimal_fields() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");

    let title_server = FakeCdpServer::start(sample_page()).await;
    std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &title_server.endpoint);
    let title_task_id =
        create_browser_task(&app, "/browser/title", "https://example.com/title", 5).await;
    let title_task = wait_for_terminal_status(&app, &title_task_id).await;
    assert_eq!(
        title_task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_SUCCEEDED)
    );
    let (_, _, title_result, _, _, _) = load_result_row(&state, &title_task_id).await;
    assert_eq!(
        title_result.get("title").and_then(Value::as_str),
        Some("Example Domain")
    );
    assert_eq!(title_result.get("content_kind"), Some(&Value::Null));
    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");
    title_server.stop().await;

    let final_url_server = FakeCdpServer::start(sample_page()).await;
    std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &final_url_server.endpoint);
    let final_url_task_id = create_browser_task(
        &app,
        "/browser/final-url",
        "https://example.com/final-url",
        5,
    )
    .await;
    let final_url_task = wait_for_terminal_status(&app, &final_url_task_id).await;
    assert_eq!(
        final_url_task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_SUCCEEDED)
    );
    let (_, _, final_url_result, _, _, _) = load_result_row(&state, &final_url_task_id).await;
    assert_eq!(
        final_url_result.get("final_url").and_then(Value::as_str),
        Some("https://example.com/final")
    );
    assert_eq!(final_url_result.get("content_kind"), Some(&Value::Null));

    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");
    final_url_server.stop().await;
}

#[tokio::test]
async fn lightpanda_runner_auto_session_restores_and_persists_cookies() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");

    let (profile_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/fingerprint-profiles")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "id": "fp-cookie-session",
                    "name": "Cookie Session",
                    "profile_json": {
                        "browser": {"name": "chrome", "version": "123"},
                        "os": {"name": "linux", "version": "ubuntu"}
                    }
                })
                .to_string(),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(profile_status, StatusCode::CREATED);

    let first_server = FakeCdpServer::start(FakeCdpScenario::Success {
        title: "Cookie Page",
        final_url: "https://example.com/session-one",
        html: "<html><body>cookie one</body></html>",
        text: "cookie one",
        cookies: vec![json!({
            "name": "sid",
            "value": "alpha",
            "domain": "example.com",
            "path": "/"
        })],
        local_storage: json!({"theme": "dark"}),
        session_storage: json!({"step": "1"}),
    })
    .await;
    std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &first_server.endpoint);
    let first_task_id = create_browser_task_with_payload(
        &app,
        "/browser/open",
        json!({
            "url": "https://example.com/session-one",
            "timeout_seconds": 5,
            "fingerprint_profile_id": "fp-cookie-session",
            "network_policy_json": {
                "mode": "required_proxy",
                "provider": "lp-test",
                "region": "us-east"
            }
        }),
    )
    .await;
    let first_task = wait_for_terminal_status(&app, &first_task_id).await;
    assert_eq!(
        first_task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_SUCCEEDED)
    );
    assert_eq!(
        first_task
            .get("execution_identity")
            .and_then(|v| v.get("identity_session_status"))
            .and_then(Value::as_str),
        Some("auto_created"),
    );
    assert_eq!(
        first_task
            .get("execution_identity")
            .and_then(|v| v.get("local_storage_persist_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        first_task
            .get("execution_identity")
            .and_then(|v| v.get("session_storage_persist_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    first_server.stop().await;

    let (
        stored_cookies_after_first,
        stored_local_storage_after_first,
        stored_session_storage_after_first,
    ): (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT cookies_json, local_storage_json, session_storage_json
           FROM proxy_session_bindings
           WHERE fingerprint_profile_id = 'fp-cookie-session'"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load session state after first task");
    assert!(stored_cookies_after_first
        .unwrap_or_default()
        .contains("\"alpha\""));
    assert!(stored_local_storage_after_first
        .unwrap_or_default()
        .contains("\"dark\""));
    assert!(stored_session_storage_after_first
        .unwrap_or_default()
        .contains("\"1\""));

    let second_server = FakeCdpServer::start(FakeCdpScenario::Success {
        title: "Cookie Page",
        final_url: "https://example.com/session-two",
        html: "<html><body>cookie two</body></html>",
        text: "cookie two",
        cookies: vec![json!({
            "name": "sid",
            "value": "beta",
            "domain": "example.com",
            "path": "/"
        })],
        local_storage: json!({"theme": "light"}),
        session_storage: json!({"step": "2"}),
    })
    .await;
    std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &second_server.endpoint);
    let second_task_id = create_browser_task_with_payload(
        &app,
        "/browser/open",
        json!({
            "url": "https://example.com/session-two",
            "timeout_seconds": 5,
            "fingerprint_profile_id": "fp-cookie-session",
            "network_policy_json": {
                "mode": "required_proxy",
                "provider": "lp-test",
                "region": "us-east"
            }
        }),
    )
    .await;
    let second_task = wait_for_terminal_status(&app, &second_task_id).await;
    assert_eq!(
        second_task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_SUCCEEDED)
    );
    assert_eq!(
        second_task
            .get("execution_identity")
            .and_then(|v| v.get("identity_session_status"))
            .and_then(Value::as_str),
        Some("auto_reused"),
    );
    assert_eq!(
        second_task
            .get("execution_identity")
            .and_then(|v| v.get("cookie_restore_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        second_task
            .get("execution_identity")
            .and_then(|v| v.get("cookie_persist_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        second_task
            .get("execution_identity")
            .and_then(|v| v.get("local_storage_restore_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        second_task
            .get("execution_identity")
            .and_then(|v| v.get("local_storage_persist_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        second_task
            .get("execution_identity")
            .and_then(|v| v.get("session_storage_restore_count"))
            .and_then(Value::as_i64),
        Some(1),
    );
    assert_eq!(
        second_task
            .get("execution_identity")
            .and_then(|v| v.get("session_storage_persist_count"))
            .and_then(Value::as_i64),
        Some(1),
    );

    let requests = second_server.recorded_requests().await;
    let set_cookies = requests
        .iter()
        .find(|request| request.get("method").and_then(Value::as_str) == Some("Network.setCookies"))
        .expect("Network.setCookies request");
    assert_eq!(
        set_cookies
            .get("params")
            .and_then(|v| v.get("cookies"))
            .and_then(|v| v.as_array())
            .and_then(|items| items.first())
            .and_then(|cookie| cookie.get("value"))
            .and_then(Value::as_str),
        Some("alpha"),
    );
    let restore_storage = requests
        .iter()
        .find(|request| {
            request.get("method").and_then(Value::as_str) == Some("Runtime.evaluate")
                && request
                    .get("params")
                    .and_then(|value| value.get("expression"))
                    .and_then(Value::as_str)
                    .map(|expression| {
                        expression.contains("__PP_STORAGE_RESTORE__")
                            && expression.contains("\"theme\":\"dark\"")
                            && expression.contains("\"step\":\"1\"")
                    })
                    .unwrap_or(false)
        })
        .expect("storage restore Runtime.evaluate request");
    assert_eq!(
        restore_storage.get("sessionId").and_then(Value::as_str),
        Some("session-1"),
    );
    second_server.stop().await;

    let (
        stored_cookies_after_second,
        stored_local_storage_after_second,
        stored_session_storage_after_second,
    ): (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        r#"SELECT cookies_json, local_storage_json, session_storage_json
           FROM proxy_session_bindings
           WHERE fingerprint_profile_id = 'fp-cookie-session'"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load session state after second task");
    assert!(stored_cookies_after_second
        .unwrap_or_default()
        .contains("\"beta\""));
    assert!(stored_local_storage_after_second
        .unwrap_or_default()
        .contains("\"light\""));
    assert!(stored_session_storage_after_second
        .unwrap_or_default()
        .contains("\"2\""));

    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");
}

#[tokio::test]
async fn lightpanda_runner_reports_canonical_fingerprint_consumption_fields() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let db_url = unique_db_url();
    let (_state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");

    let (profile_status, _) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/fingerprint-profiles")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "id": "fp-canonical-consumption",
                    "name": "Canonical Consumption",
                    "profile_json": {
                        "timezone": "Asia/Shanghai",
                        "locale": "zh-CN",
                        "headers": {
                            "accept_language": "zh-CN,zh;q=0.9"
                        },
                        "device_memory": 8,
                        "hardware_concurrency": 4
                    }
                })
                .to_string(),
            ))
            .expect("request"),
    )
    .await;
    assert_eq!(profile_status, StatusCode::CREATED);

    let server = FakeCdpServer::start(FakeCdpScenario::Success {
        title: "Canonical Page",
        final_url: "https://example.com/canonical",
        html: "<html><body>canonical</body></html>",
        text: "canonical",
        cookies: Vec::new(),
        local_storage: json!({}),
        session_storage: json!({}),
    })
    .await;
    std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &server.endpoint);

    let task_id = create_browser_task_with_payload(
        &app,
        "/browser/open",
        json!({
            "url": "https://example.com/canonical",
            "timeout_seconds": 5,
            "fingerprint_profile_id": "fp-canonical-consumption",
            "network_policy_json": {
                "mode": "required_proxy",
                "proxy_id": DEFAULT_LIGHTPANDA_TEST_PROXY_ID
            }
        }),
    )
    .await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_SUCCEEDED)
    );

    let runtime = task
        .get("fingerprint_runtime_explain")
        .expect("fingerprint runtime explain");
    assert_eq!(
        runtime
            .get("consumption_source_of_truth")
            .and_then(Value::as_str),
        Some("runner_runtime"),
    );
    assert_eq!(
        runtime.get("consumption_version").and_then(Value::as_str),
        Some("fingerprint_consumption_schema_v1"),
    );

    let explain = runtime
        .get("consumption_explain")
        .expect("consumption explain");
    assert_eq!(
        explain.get("consumption_status").and_then(Value::as_str),
        Some("fully_consumed"),
    );
    assert_eq!(
        explain.get("consumption_version").and_then(Value::as_str),
        Some("fingerprint_consumption_schema_v1"),
    );

    let applied_fields = explain
        .get("applied_fields")
        .and_then(Value::as_array)
        .expect("applied_fields");
    assert!(applied_fields
        .iter()
        .any(|value| value.as_str() == Some("accept_language")));
    assert!(applied_fields
        .iter()
        .any(|value| value.as_str() == Some("device_memory_gb")));
    assert!(applied_fields
        .iter()
        .any(|value| value.as_str() == Some("hardware_concurrency")));
    assert!(!applied_fields
        .iter()
        .any(|value| value.as_str() == Some("device_memory")));

    server.stop().await;
    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");
}

#[tokio::test]
async fn lightpanda_runner_browser_failures_map_to_browser_execution() {
    let _env_guard = lightpanda_env_lock().lock().await;
    for (needle, signal) in [
        (
            "navigation failed while opening page",
            "browser_navigation_failure_signal",
        ),
        (
            "dns name not resolved while opening page",
            "browser_dns_failure_signal",
        ),
        (
            "tls certificate failure while opening page",
            "browser_tls_failure_signal",
        ),
    ] {
        let server =
            FakeCdpServer::start(FakeCdpScenario::NavigateError { error_text: needle }).await;
        std::env::set_var("LIGHTPANDA_TEST_WS_ENDPOINT", &server.endpoint);
        let db_url = unique_db_url();
        let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
        let task_id =
            create_browser_task(&app, "/browser/open", "https://example.com/fail", 5).await;
        let task = wait_for_terminal_status(&app, &task_id).await;
        assert_eq!(
            task.get("status").and_then(Value::as_str),
            Some(TASK_STATUS_FAILED)
        );
        let (_, run_status, result_json, error_message, runner_id, heartbeat_at) =
            load_result_row(&state, &task_id).await;
        assert_eq!(run_status, RUN_STATUS_FAILED);
        assert!(
            error_message
                .unwrap_or_default()
                .contains("navigation failed")
                || needle.contains("dns")
                || needle.contains("tls")
        );
        assert!(runner_id.is_none());
        assert!(heartbeat_at.is_none());
        assert_eq!(
            result_json.get("failure_scope").and_then(Value::as_str),
            Some("browser_execution")
        );
        assert_eq!(
            result_json
                .get("browser_failure_signal")
                .and_then(Value::as_str),
            Some(signal)
        );
        assert_eq!(
            result_json.get("execution_stage").and_then(Value::as_str),
            Some("navigate")
        );
        std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");
        server.stop().await;
    }
}

#[tokio::test]
async fn lightpanda_runner_timeout_marks_timed_out_and_cleans_state() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_timeout",
        "#!/usr/bin/env bash\nset -euo pipefail\nsleep 5\n",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);
    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id =
        create_browser_task(&app, "/browser/html", "https://example.com/timeout", 1).await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_TIMED_OUT)
    );

    let (task_status, run_status, result_json, error_message, runner_id, heartbeat_at) =
        load_result_row(&state, &task_id).await;
    assert_eq!(task_status, TASK_STATUS_TIMED_OUT);
    assert_eq!(run_status, RUN_STATUS_TIMED_OUT);
    assert!(error_message.unwrap_or_default().contains("timed out"));
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());
    assert_eq!(
        result_json.get("failure_scope").and_then(Value::as_str),
        Some("runner_timeout")
    );

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}

#[tokio::test]
async fn lightpanda_runner_non_zero_exit_marks_failed_before_endpoint_ready() {
    let _env_guard = lightpanda_env_lock().lock().await;
    let script = make_stub_script(
        "lightpanda_non_zero",
        "#!/usr/bin/env bash\nset -euo pipefail\necho runner crashed >&2\nexit 42\n",
    );
    std::env::set_var("LIGHTPANDA_BIN", &script);
    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id = create_browser_task(&app, "/browser/title", "https://example.com/fail", 5).await;
    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_FAILED)
    );

    let (task_status, run_status, result_json, error_message, _, _) =
        load_result_row(&state, &task_id).await;
    assert_eq!(task_status, TASK_STATUS_FAILED);
    assert_eq!(run_status, RUN_STATUS_FAILED);
    assert!(error_message
        .unwrap_or_default()
        .contains("websocket endpoint"));
    assert_eq!(
        result_json.get("error_kind").and_then(Value::as_str),
        Some("runner_non_zero_exit")
    );
    assert_eq!(
        result_json.get("failure_scope").and_then(Value::as_str),
        Some("runner_process_exit")
    );
    assert_eq!(
        result_json.get("execution_stage").and_then(Value::as_str),
        Some("launch")
    );
    assert_eq!(
        result_json.get("exit_code").and_then(Value::as_i64),
        Some(42)
    );

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
    std::env::remove_var("LIGHTPANDA_TEST_WS_ENDPOINT");

    let db_url = unique_db_url();
    let (state, app) = build_lightpanda_test_app(&db_url).await.expect("build app");
    let task_id =
        create_browser_task(&app, "/browser/open", "https://example.com/cancel", 10).await;

    for _ in 0..20 {
        let (_, task_json) = json_response(
            &app,
            Request::builder()
                .uri(format!("/tasks/{task_id}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await;
        if task_json.get("status").and_then(Value::as_str) == Some("running") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

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
        cancel_json.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_CANCELLED)
    );

    let task = wait_for_terminal_status(&app, &task_id).await;
    assert_eq!(
        task.get("status").and_then(Value::as_str),
        Some(TASK_STATUS_CANCELLED)
    );
    let (task_status, run_status, result_json, task_error, runner_id, heartbeat_at) =
        load_result_row(&state, &task_id).await;
    assert_eq!(task_status, TASK_STATUS_CANCELLED);
    assert_eq!(run_status, RUN_STATUS_CANCELLED);
    assert_eq!(task_error.as_deref(), Some("task cancelled while running"));
    assert!(runner_id.is_none());
    assert!(heartbeat_at.is_none());
    assert_eq!(
        result_json.get("status").and_then(Value::as_str),
        Some("cancelled")
    );
    assert_eq!(
        result_json.get("failure_scope").and_then(Value::as_str),
        Some("runner_cancelled")
    );
    assert_eq!(
        result_json.get("error_kind").and_then(Value::as_str),
        Some("runner_cancelled")
    );

    std::env::remove_var("LIGHTPANDA_BIN");
    let _ = std::fs::remove_file(script);
}
