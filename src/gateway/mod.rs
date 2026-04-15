mod control;

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result as AnyhowResult;
use axum::{
    extract::State,
    http::{header::{AUTHORIZATION, HOST}, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::{get, get_service, post},
    Json, Router,
};
use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::services::ServeDir;

use crate::db::init::{init_db, DbPool};

#[derive(Clone)]
pub struct GatewayState {
    pub admin_token: Option<String>,
    pub tokens: Arc<HashMap<String, DownstreamToken>>,
    rate_limits: Arc<Mutex<HashMap<String, TokenWindow>>>,
    usage_log: Arc<Mutex<Vec<UsageEvent>>>,
    pub config: GatewayConfig,
    pub http_client: Client,
    pub db: DbPool,
}

#[derive(Clone, Debug)]
pub struct GatewayConfig {
    pub requests_per_minute: u32,
    pub concurrency_per_token: u32,
    pub upstream_base_url: Option<String>,
    pub upstream_bearer_token: Option<String>,
    pub runtime_mode: String,
    pub ui_dir: PathBuf,
    pub database_url: String,
    pub control_base_url: String,
    pub control_api_key: Option<String>,
    pub draft_share_ttl_seconds: u64,
    pub public_preview_hosts: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownstreamToken {
    pub key: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct UsageEvent {
    pub token_label: String,
    pub path: String,
    pub model: Option<String>,
    pub upstream_target: Option<String>,
    pub status_code: u16,
}

#[derive(Clone, Debug, Serialize)]
pub struct GatewayStatsSnapshot {
    pub total_events: usize,
    pub by_token: BTreeMap<String, usize>,
    pub by_status: BTreeMap<String, usize>,
    pub by_model: BTreeMap<String, usize>,
    pub recent: Vec<UsageEvent>,
}

#[derive(Clone, Debug)]
struct TokenWindow {
    window_started: Instant,
    count: u32,
}

#[derive(Debug, Serialize)]
struct ModelsResponse {
    object: &'static str,
    data: Vec<ModelCard>,
}

#[derive(Debug, Serialize)]
struct ModelCard {
    id: String,
    object: &'static str,
    owned_by: &'static str,
}

pub fn build_gateway_router(state: GatewayState) -> Router {
    let ui_dir = state.config.ui_dir.clone();
    let dashboard_assets =
        get_service(ServeDir::new(ui_dir.clone()).append_index_html_on_directories(true));
    let dashboard_fallback = get(serve_dashboard_index);
    Router::new()
        .route("/", get(index_page))
        .route("/health", get(gateway_health))
        .route("/admin/usage", get(admin_usage))
        .route("/admin/stats", get(admin_stats))
        .route("/admin/dashboard", get(admin_dashboard))
        .route("/dashboard-session", get(dashboard_session_info))
        .route("/dashboard-preview/", get(dashboard_preview_page))
        .route(
            "/dashboard-preview-session",
            get(dashboard_preview_session_info),
        )
        .merge(control::build_admin_control_router())
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .nest_service("/dashboard", dashboard_assets)
        .fallback(dashboard_fallback)
        .with_state(state)
}

pub async fn gateway_state_from_env() -> AnyhowResult<GatewayState> {
    let admin_token = std::env::var("GATEWAY_ADMIN_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let requests_per_minute = std::env::var("GATEWAY_RATE_LIMIT_PER_MINUTE")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(30);
    let concurrency_per_token = std::env::var("GATEWAY_CONCURRENCY_PER_TOKEN")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(3);
    let upstream_base_url = std::env::var("UPSTREAM_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let upstream_bearer_token = std::env::var("UPSTREAM_BEARER_TOKEN")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let runtime_mode = normalize_runtime_mode(
        std::env::var("GATEWAY_RUNTIME_MODE").ok(),
        upstream_base_url.as_deref(),
        std::env::var("GATEWAY_CONTROL_BASE_URL").ok().as_deref(),
    );
    let ui_dir = std::env::var("GATEWAY_UI_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("gateway-ui"));
    let database_url = std::env::var("GATEWAY_DATABASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "sqlite://data/auto_open_browser.db".to_string());
    let control_base_url = std::env::var("GATEWAY_CONTROL_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
    let control_api_key = std::env::var("GATEWAY_CONTROL_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let draft_share_ttl_seconds = std::env::var("GATEWAY_DRAFT_SHARE_TTL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(24 * 60 * 60);
    let public_preview_hosts = std::env::var("GATEWAY_PUBLIC_PREVIEW_HOSTS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let tokens_json = std::env::var("GATEWAY_DOWNSTREAM_TOKENS_JSON")
        .or_else(|_| {
            std::env::var("GATEWAY_DOWNSTREAM_TOKENS_B64").map(|v| {
                String::from_utf8(
                    base64::engine::general_purpose::STANDARD
                        .decode(v)
                        .unwrap_or_default(),
                )
                .unwrap_or_else(|_| "[]".to_string())
            })
        })
        .unwrap_or_else(|_| "[]".to_string());
    let parsed: Vec<DownstreamToken> = serde_json::from_str(&tokens_json).unwrap_or_default();
    let mut map = HashMap::new();
    for token in parsed {
        map.insert(token.key.clone(), token);
    }
    let db = init_db(&database_url).await?;
    Ok(GatewayState {
        admin_token,
        tokens: Arc::new(map),
        rate_limits: Arc::new(Mutex::new(HashMap::new())),
        usage_log: Arc::new(Mutex::new(Vec::new())),
        config: GatewayConfig {
            requests_per_minute,
            concurrency_per_token,
            upstream_base_url,
            upstream_bearer_token,
            runtime_mode,
            ui_dir,
            database_url,
            control_base_url,
            control_api_key,
            draft_share_ttl_seconds,
            public_preview_hosts,
        },
        http_client: Client::new(),
        db,
    })
}

async fn index_page() -> impl IntoResponse {
    axum::response::Redirect::temporary("/dashboard/")
}

async fn serve_dashboard_index(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let path = uri.path();
    if path.starts_with("/v1/") || path.starts_with("/admin/") || path == "/health" {
        return error_response(StatusCode::NOT_FOUND, "not_found", "route not found");
    }

    let index_path = state.config.ui_dir.join("index.html");
    match tokio::fs::read_to_string(&index_path).await {
        Ok(mut html) => {
            let bootstrap = if is_public_preview_request(&state, &headers) {
                Some(json!({
                    "readonly": true,
                    "publicPreview": true,
                    "bootstrapPath": "/public/dashboard/bootstrap"
                }))
            } else {
                state.admin_token
                    .as_deref()
                    .map(|token| json!({ "adminToken": token }))
            };
            html = rewrite_dashboard_base(&html, "/dashboard/", bootstrap);
            axum::response::Html(html).into_response()
        }
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ui_unavailable",
            "dashboard ui not available",
        ),
    }
}

async fn dashboard_preview_page(State(state): State<GatewayState>) -> Response {
    let index_path = state.config.ui_dir.join("index.html");
    match tokio::fs::read_to_string(&index_path).await {
        Ok(html) => axum::response::Html(rewrite_dashboard_base(
            &html,
            "/dashboard/",
            Some(json!({
                "readonly": true,
                "publicPreview": true,
                "bootstrapPath": "/public/dashboard/bootstrap"
            })),
        ))
        .into_response(),
        Err(_) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ui_unavailable",
            "dashboard ui not available",
        ),
    }
}

fn rewrite_dashboard_base(html: &str, base_path: &str, bootstrap: Option<Value>) -> String {
    let mut rewritten = html
        .replace("href=\"/", &format!("href=\"{base_path}"))
        .replace("src=\"/", &format!("src=\"{base_path}"));
    if let Some(payload) = bootstrap {
        let injected = format!(
            "<script>window.__DASHBOARD_BOOTSTRAP__={};</script>",
            payload
        );
        rewritten = rewritten.replace("</head>", &format!("{injected}\n  </head>"));
    }
    rewritten
}

async fn gateway_health(State(state): State<GatewayState>) -> impl IntoResponse {
    Json(json!({
        "status":"ok",
        "service":"agent-gateway-v0",
        "upstream_configured": state.config.upstream_base_url.is_some() && state.config.upstream_bearer_token.is_some(),
        "requests_per_minute": state.config.requests_per_minute,
        "concurrency_per_token": state.config.concurrency_per_token,
    }))
}

async fn admin_usage(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }
    let events = state
        .usage_log
        .lock()
        .expect("usage log mutex poisoned")
        .clone();
    Json(json!({"events": events})).into_response()
}

async fn admin_dashboard(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }
    axum::response::Redirect::temporary("/dashboard/").into_response()
}

async fn admin_stats(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }
    Json(build_gateway_stats_snapshot(&state)).into_response()
}

async fn dashboard_session_info(State(state): State<GatewayState>, headers: HeaderMap) -> impl IntoResponse {
    let public_preview = is_public_preview_request(&state, &headers);
    Json(json!({
        "admin_token": if public_preview { Value::Null } else { serde_json::to_value(state.admin_token.clone()).unwrap_or(Value::Null) },
        "runtime_mode": state.config.runtime_mode,
        "auto_connected": if public_preview { false } else { state.admin_token.is_some() },
        "readonly": public_preview,
        "public_preview": public_preview,
        "bootstrap_path": if public_preview { Some("/public/dashboard/bootstrap") } else { None::<&str> },
    }))
}

async fn dashboard_preview_session_info(State(state): State<GatewayState>) -> impl IntoResponse {
    Json(json!({
        "admin_token": Value::Null,
        "runtime_mode": state.config.runtime_mode,
        "auto_connected": false,
        "readonly": true,
        "public_preview": true,
        "bootstrap_path": "./bootstrap",
    }))
}

fn is_public_preview_request(state: &GatewayState, headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let normalized = host
        .split(':')
        .next()
        .unwrap_or(host)
        .trim()
        .to_ascii_lowercase();
    state
        .config
        .public_preview_hosts
        .iter()
        .any(|candidate| candidate == &normalized)
}

fn normalize_runtime_mode(
    raw: Option<String>,
    upstream_base_url: Option<&str>,
    control_base_url: Option<&str>,
) -> String {
    let normalized = raw
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase);
    if let Some(value) = normalized {
        return value;
    }
    let upstream_is_local = upstream_base_url.map(is_local_url).unwrap_or(true);
    let control_is_local = control_base_url.map(is_local_url).unwrap_or(true);
    if upstream_is_local && control_is_local {
        "dev".to_string()
    } else {
        "prod_live".to_string()
    }
}

fn is_local_url(url: &str) -> bool {
    url.contains("127.0.0.1") || url.contains("localhost")
}

pub(super) fn build_gateway_stats_snapshot(state: &GatewayState) -> GatewayStatsSnapshot {
    let events = state
        .usage_log
        .lock()
        .expect("usage log mutex poisoned")
        .clone();
    let mut by_token = BTreeMap::new();
    let mut by_status = BTreeMap::new();
    let mut by_model = BTreeMap::new();
    for event in &events {
        *by_token.entry(event.token_label.clone()).or_insert(0usize) += 1;
        *by_status
            .entry(event.status_code.to_string())
            .or_insert(0usize) += 1;
        *by_model
            .entry(event.model.clone().unwrap_or_else(|| "unknown".to_string()))
            .or_insert(0usize) += 1;
    }
    GatewayStatsSnapshot {
        total_events: events.len(),
        by_token,
        by_status,
        by_model,
        recent: events.iter().rev().take(20).cloned().collect::<Vec<_>>(),
    }
}

async fn list_models(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    let token = match authorize(&state, &headers) {
        Ok(token) => token,
        Err(resp) => return resp,
    };
    if let Err(resp) = check_rate_limit(&state, &token) {
        return resp;
    }
    log_usage(
        &state,
        &token,
        "/v1/models",
        None,
        Some("gateway".to_string()),
        StatusCode::OK,
    );
    Json(ModelsResponse {
        object: "list",
        data: vec![
            ModelCard {
                id: "agent-proxy-v0".to_string(),
                object: "model",
                owned_by: "alexstudio",
            },
            ModelCard {
                id: "agent-proxy-upstream".to_string(),
                object: "model",
                owned_by: "alexstudio",
            },
        ],
    })
    .into_response()
}

async fn chat_completions(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let token = match authorize(&state, &headers) {
        Ok(token) => token,
        Err(resp) => return resp,
    };
    if let Err(resp) = check_rate_limit(&state, &token) {
        return resp;
    }

    let sanitized_payload = sanitize_chat_payload(payload);
    let model = sanitized_payload
        .get("model")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());

    if let Some(upstream_base_url) = state.config.upstream_base_url.clone() {
        let Some(upstream_token) = state.config.upstream_bearer_token.clone() else {
            let resp = error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_unavailable",
                "upstream auth not configured",
            );
            log_usage(
                &state,
                &token,
                "/v1/chat/completions",
                model,
                Some("upstream".to_string()),
                StatusCode::BAD_GATEWAY,
            );
            return resp;
        };
        let url = format!(
            "{}/v1/chat/completions",
            upstream_base_url.trim_end_matches('/')
        );
        let response = state
            .http_client
            .post(url)
            .bearer_auth(upstream_token)
            .header("x-gateway-source", token.label.clone())
            .json(&sanitized_payload)
            .send()
            .await;
        match response {
            Ok(resp) => {
                let status = resp.status();
                let body = match resp.json::<Value>().await {
                    Ok(body) => body,
                    Err(_) => {
                        return error_with_usage(
                            &state,
                            &token,
                            model,
                            StatusCode::BAD_GATEWAY,
                            "upstream_invalid_json",
                            "upstream returned invalid json",
                        )
                    }
                };
                log_usage(
                    &state,
                    &token,
                    "/v1/chat/completions",
                    model,
                    Some("upstream".to_string()),
                    status,
                );
                (status, Json(body)).into_response()
            }
            Err(err) if err.is_timeout() => error_with_usage(
                &state,
                &token,
                model,
                StatusCode::GATEWAY_TIMEOUT,
                "upstream_timeout",
                "upstream request timed out",
            ),
            Err(_) => error_with_usage(
                &state,
                &token,
                model,
                StatusCode::BAD_GATEWAY,
                "upstream_unavailable",
                "failed to reach upstream",
            ),
        }
    } else {
        log_usage(
            &state,
            &token,
            "/v1/chat/completions",
            model.clone(),
            Some("skeleton".to_string()),
            StatusCode::OK,
        );
        Json(json!({
            "id": "chatcmpl-agent-gateway-v0",
            "object": "chat.completion",
            "created": 1775450000,
            "model": model.unwrap_or_else(|| "agent-proxy-v0".to_string()),
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "gateway skeleton ok; upstream not wired yet"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 0,
                "completion_tokens": 0,
                "total_tokens": 0
            }
        }))
        .into_response()
    }
}

fn sanitize_chat_payload(mut payload: Value) -> Value {
    if let Some(obj) = payload.as_object_mut() {
        obj.remove("api_key");
        obj.remove("authorization");
        obj.remove("upstream_token");
        obj.remove("upstream_auth");
    }
    payload
}

pub(crate) fn authorize_admin(state: &GatewayState, headers: &HeaderMap) -> Result<(), Response> {
    let Some(expected) = state.admin_token.as_deref() else {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "admin_disabled",
            "admin endpoint disabled",
        ));
    };
    let provided = extract_bearer_or_api_key(headers);
    if provided != Some(expected) {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "auth_failed",
            "invalid admin token",
        ));
    }
    Ok(())
}

fn authorize(state: &GatewayState, headers: &HeaderMap) -> Result<DownstreamToken, Response> {
    let Some(provided) = extract_bearer_or_api_key(headers) else {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "auth_failed",
            "missing bearer token",
        ));
    };

    let Some(token) = state.tokens.get(provided).cloned() else {
        return Err(error_response(
            StatusCode::UNAUTHORIZED,
            "auth_failed",
            "invalid bearer token",
        ));
    };

    if !token.enabled {
        return Err(error_response(
            StatusCode::FORBIDDEN,
            "auth_failed",
            "token disabled",
        ));
    }

    Ok(token)
}

pub(crate) fn extract_bearer_or_api_key(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .or_else(|| {
            headers
                .get("x-api-key")
                .and_then(|value| value.to_str().ok())
        })
}

fn check_rate_limit(state: &GatewayState, token: &DownstreamToken) -> Result<(), Response> {
    let mut guard = state.rate_limits.lock().expect("rate limit mutex poisoned");
    let now = Instant::now();
    let window = guard.entry(token.label.clone()).or_insert(TokenWindow {
        window_started: now,
        count: 0,
    });
    if now.duration_since(window.window_started) >= Duration::from_secs(60) {
        window.window_started = now;
        window.count = 0;
    }
    if window.count >= state.config.requests_per_minute {
        return Err(error_response(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "request rate exceeded",
        ));
    }
    window.count += 1;
    Ok(())
}

fn log_usage(
    state: &GatewayState,
    token: &DownstreamToken,
    path: &str,
    model: Option<String>,
    upstream_target: Option<String>,
    status_code: StatusCode,
) {
    let mut guard = state.usage_log.lock().expect("usage log mutex poisoned");
    guard.push(UsageEvent {
        token_label: token.label.clone(),
        path: path.to_string(),
        model,
        upstream_target,
        status_code: status_code.as_u16(),
    });
    if guard.len() > 500 {
        let drain = guard.len() - 500;
        guard.drain(0..drain);
    }
}

fn error_with_usage(
    state: &GatewayState,
    token: &DownstreamToken,
    model: Option<String>,
    status: StatusCode,
    code: &str,
    message: &str,
) -> Response {
    log_usage(
        state,
        token,
        "/v1/chat/completions",
        model,
        Some("upstream".to_string()),
        status,
    );
    error_response(status, code, message)
}

pub(crate) fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"error": {"code": code, "message": message}})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use serde_json::Value;
    use tower::ServiceExt;

    async fn test_state() -> GatewayState {
        let mut tokens = HashMap::new();
        tokens.insert(
            "test-token".to_string(),
            DownstreamToken {
                key: "test-token".to_string(),
                label: "local-test".to_string(),
                enabled: true,
            },
        );
        let db_url = format!(
            "sqlite://{}/gateway-test-{}.db",
            std::env::temp_dir().display(),
            uuid::Uuid::new_v4()
        );
        let db = init_db(&db_url).await.expect("init gateway test db");
        GatewayState {
            admin_token: Some("admin-token".to_string()),
            tokens: Arc::new(tokens),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
            usage_log: Arc::new(Mutex::new(Vec::new())),
            config: GatewayConfig {
                requests_per_minute: 2,
                concurrency_per_token: 3,
                upstream_base_url: None,
                upstream_bearer_token: None,
                runtime_mode: "dev".to_string(),
                ui_dir: PathBuf::from("gateway-ui"),
                database_url: db_url,
                control_base_url: "http://127.0.0.1:3000".to_string(),
                control_api_key: None,
                draft_share_ttl_seconds: 3600,
                public_preview_hosts: vec!["agent.alexstudio.top".to_string()],
            },
            http_client: Client::new(),
            db,
        }
    }

    #[tokio::test]
    async fn models_requires_bearer_token() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn models_returns_proxy_catalog_when_authorized() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn models_rate_limit_applies_per_token() {
        let app = build_gateway_router(test_state().await);
        for _ in 0..2 {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/v1/models")
                        .header(AUTHORIZATION, "Bearer test-token")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn chat_completion_logs_usage_event() {
        let state = test_state().await;
        let app = build_gateway_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"model":"agent-proxy-v0","messages":[{"role":"user","content":"hi"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let log = state.usage_log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].path, "/v1/chat/completions");
        assert_eq!(log[0].token_label, "local-test");
    }

    #[tokio::test]
    async fn admin_usage_requires_admin_token() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/usage")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn dashboard_root_serves_ui() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/dashboard/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Behavior Realism Dashboard"));
        assert!(!html.contains("GATEWAY_CONTROL_API_KEY"));
    }

    #[tokio::test]
    async fn dashboard_preview_serves_readonly_shell() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/dashboard-preview/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let html = String::from_utf8_lossy(&body);
        assert!(html.contains("Behavior Realism Dashboard"));
        assert!(html.contains("./app.js"));
        assert!(!html.contains("admin-token"));
    }

    #[tokio::test]
    async fn dashboard_session_info_exposes_auto_connect_context() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/dashboard-session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.get("admin_token").and_then(Value::as_str),
            Some("admin-token")
        );
        assert_eq!(
            json.get("auto_connected").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn public_host_dashboard_session_returns_readonly_context() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/dashboard-session")
                    .header(HOST, "agent.alexstudio.top")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("admin_token").unwrap_or(&Value::Null).is_null());
        assert_eq!(json.get("readonly").and_then(Value::as_bool), Some(true));
        assert_eq!(
            json.get("public_preview").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            json.get("bootstrap_path").and_then(Value::as_str),
            Some("/public/dashboard/bootstrap")
        );
    }

    #[tokio::test]
    async fn dashboard_preview_session_is_readonly() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/dashboard-preview-session")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("admin_token").unwrap_or(&Value::Null).is_null());
        assert_eq!(json.get("readonly").and_then(Value::as_bool), Some(true));
        assert_eq!(
            json.get("public_preview").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn admin_control_requires_admin_token() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/control/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn admin_control_create_draft_derives_site_key_and_share_link() {
        let app = build_gateway_router(test_state().await);
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/control/onboarding-drafts")
                    .header(AUTHORIZATION, "Bearer admin-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "login_url":"https://Example.com/login",
                            "credential_mode":"inline_once"
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.get("site_key").and_then(Value::as_str),
            Some("example.com")
        );
        assert_eq!(
            json.get("credential_mode").and_then(Value::as_str),
            Some("inline_once")
        );
        assert_eq!(json.get("credential_ref"), Some(&Value::Null));
        assert!(json
            .get("share_url")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("/dashboard/?draft="));
    }

    #[tokio::test]
    async fn admin_control_bootstrap_exposes_dashboard_shell_fields() {
        let state = test_state().await;
        let app = build_gateway_router(state.clone());
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"model":"agent-proxy-v0","messages":[{"role":"user","content":"ping"}]}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/control/bootstrap")
                    .header(AUTHORIZATION, "Bearer admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.get("runtime_mode").and_then(Value::as_str),
            Some("dev")
        );
        assert_eq!(
            json.get("gateway_stats_snapshot")
                .and_then(|value| value.get("total_events"))
                .and_then(Value::as_u64),
            Some(1)
        );
        assert!(json
            .get("overview_tasks")
            .and_then(Value::as_array)
            .is_some());
        assert!(json
            .get("continuity_events")
            .and_then(Value::as_array)
            .is_some());
        assert!(json
            .get("site_validation_rollups")
            .and_then(Value::as_array)
            .is_some());
        assert!(json
            .get("ui_model")
            .and_then(|value| value.get("shell"))
            .is_some());
        assert!(json
            .get("ui_model")
            .and_then(|value| value.get("overview"))
            .and_then(|value| value.get("primary_cards"))
            .and_then(Value::as_array)
            .is_some());
        assert!(json
            .get("ui_model")
            .and_then(|value| value.get("onboarding"))
            .and_then(|value| value.get("site_rows"))
            .and_then(Value::as_array)
            .is_some());
        assert!(json
            .get("ui_model")
            .and_then(|value| value.get("display_meta"))
            .is_some());
    }

    #[tokio::test]
    async fn public_dashboard_bootstrap_is_readonly_and_safe() {
        let state = test_state().await;
        let app = build_gateway_router(state.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/public/dashboard/bootstrap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.get("readonly").and_then(Value::as_bool), Some(true));
        assert_eq!(
            json.get("public_preview").and_then(Value::as_bool),
            Some(true)
        );
        assert!(json.get("drafts").is_none());
        assert!(json.get("behavior_profiles").is_none());
        assert!(json.get("identity_profiles").is_none());
        assert!(json.get("site_policies").is_none());
        assert!(json
            .get("ui_model")
            .and_then(|value| value.get("shell"))
            .and_then(|value| value.get("readonly"))
            .and_then(Value::as_bool)
            .unwrap_or(false));
    }

    #[tokio::test]
    async fn admin_control_publish_rejects_missing_ready_selector() {
        let app = build_gateway_router(test_state().await);
        let create = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/control/onboarding-drafts")
                    .header(AUTHORIZATION, "Bearer admin-token")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{
                            "login_url":"https://example.com/login",
                            "behavior_profile_id":"system-default-browser-v1",
                            "credential_mode":"alias",
                            "credential_ref":"identity://demo",
                            "final_contract_json":{
                                "mode":"auth",
                                "field_roles":{
                                    "password":{"selector":"input[type='password']"},
                                    "submit":{"selector":"button[type='submit']"}
                                },
                                "success":{}
                            }
                        }"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let draft_id = json.get("id").and_then(Value::as_str).unwrap();
        let publish = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/admin/control/onboarding-drafts/{draft_id}/publish"
                    ))
                    .header(AUTHORIZATION, "Bearer admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(publish.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn admin_control_share_token_restore_and_expiry_work() {
        let state = test_state().await;
        let app = build_gateway_router(state.clone());
        let create = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/admin/control/onboarding-drafts")
                    .header(AUTHORIZATION, "Bearer admin-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"login_url":"https://example.com/login"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(create.into_body(), usize::MAX).await.unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let share_token = json.get("share_token").and_then(Value::as_str).unwrap();

        let restore = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/admin/control/onboarding-drafts?share_token={share_token}"
                    ))
                    .header(AUTHORIZATION, "Bearer admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(restore.status(), StatusCode::OK);

        sqlx::query(
            "UPDATE dashboard_onboarding_drafts SET share_expires_at = '1' WHERE share_token = ?",
        )
        .bind(share_token)
        .execute(&state.db)
        .await
        .unwrap();

        let expired = app
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/admin/control/onboarding-drafts?share_token={share_token}"
                    ))
                    .header(AUTHORIZATION, "Bearer admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(expired.status(), StatusCode::GONE);
    }

    #[test]
    fn sanitize_chat_payload_removes_dangerous_fields() {
        let payload = json!({
            "model": "agent-proxy-v0",
            "api_key": "downstream",
            "authorization": "Bearer leaked",
            "upstream_token": "secret",
            "messages": []
        });
        let sanitized = sanitize_chat_payload(payload);
        let obj = sanitized.as_object().unwrap();
        assert!(!obj.contains_key("api_key"));
        assert!(!obj.contains_key("authorization"));
        assert!(!obj.contains_key("upstream_token"));
        assert!(obj.contains_key("model"));
    }
}
