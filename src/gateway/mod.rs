use std::{collections::HashMap, sync::{Arc, Mutex}, time::{Duration, Instant}};

use axum::{
    extract::State,
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct GatewayState {
    pub admin_token: Option<String>,
    pub tokens: Arc<HashMap<String, DownstreamToken>>,
    rate_limits: Arc<Mutex<HashMap<String, TokenWindow>>>,
    usage_log: Arc<Mutex<Vec<UsageEvent>>>,
    pub config: GatewayConfig,
    pub http_client: Client,
}

#[derive(Clone, Debug)]
pub struct GatewayConfig {
    pub requests_per_minute: u32,
    pub concurrency_per_token: u32,
    pub upstream_base_url: Option<String>,
    pub upstream_bearer_token: Option<String>,
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
    Router::new()
        .route("/health", get(gateway_health))
        .route("/admin/usage", get(admin_usage))
        .route("/admin/stats", get(admin_stats))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .with_state(state)
}

pub fn gateway_state_from_env() -> GatewayState {
    let admin_token = std::env::var("GATEWAY_ADMIN_TOKEN").ok().filter(|v| !v.trim().is_empty());
    let requests_per_minute = std::env::var("GATEWAY_RATE_LIMIT_PER_MINUTE").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(30);
    let concurrency_per_token = std::env::var("GATEWAY_CONCURRENCY_PER_TOKEN").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(3);
    let upstream_base_url = std::env::var("UPSTREAM_BASE_URL").ok().filter(|v| !v.trim().is_empty());
    let upstream_bearer_token = std::env::var("UPSTREAM_BEARER_TOKEN").ok().filter(|v| !v.trim().is_empty());
    let tokens_json = std::env::var("GATEWAY_DOWNSTREAM_TOKENS_JSON")
        .or_else(|_| std::env::var("GATEWAY_DOWNSTREAM_TOKENS_B64").map(|v| String::from_utf8(base64::decode(v).unwrap_or_default()).unwrap_or_else(|_| "[]".to_string())))
        .unwrap_or_else(|_| "[]".to_string());
    let parsed: Vec<DownstreamToken> = serde_json::from_str(&tokens_json).unwrap_or_default();
    let mut map = HashMap::new();
    for token in parsed {
        map.insert(token.key.clone(), token);
    }
    GatewayState {
        admin_token,
        tokens: Arc::new(map),
        rate_limits: Arc::new(Mutex::new(HashMap::new())),
        usage_log: Arc::new(Mutex::new(Vec::new())),
        config: GatewayConfig { requests_per_minute, concurrency_per_token, upstream_base_url, upstream_bearer_token },
        http_client: Client::new(),
    }
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
    let events = state.usage_log.lock().expect("usage log mutex poisoned").clone();
    Json(json!({"events": events})).into_response()
}

async fn admin_stats(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }
    let events = state.usage_log.lock().expect("usage log mutex poisoned").clone();
    let mut by_token = std::collections::BTreeMap::new();
    let mut by_status = std::collections::BTreeMap::new();
    let mut by_model = std::collections::BTreeMap::new();
    for event in &events {
        *by_token.entry(event.token_label.clone()).or_insert(0usize) += 1;
        *by_status.entry(event.status_code.to_string()).or_insert(0usize) += 1;
        *by_model.entry(event.model.clone().unwrap_or_else(|| "unknown".to_string())).or_insert(0usize) += 1;
    }
    Json(json!({
        "total_events": events.len(),
        "by_token": by_token,
        "by_status": by_status,
        "by_model": by_model,
        "recent": events.iter().rev().take(20).cloned().collect::<Vec<_>>()
    })).into_response()
}

async fn list_models(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    let token = match authorize(&state, &headers) {
        Ok(token) => token,
        Err(resp) => return resp,
    };
    if let Err(resp) = check_rate_limit(&state, &token) {
        return resp;
    }
    log_usage(&state, &token, "/v1/models", None, Some("gateway".to_string()), StatusCode::OK);
    Json(ModelsResponse {
        object: "list",
        data: vec![
            ModelCard { id: "agent-proxy-v0".to_string(), object: "model", owned_by: "alexstudio" },
            ModelCard { id: "agent-proxy-upstream".to_string(), object: "model", owned_by: "alexstudio" },
        ],
    }).into_response()
}

async fn chat_completions(State(state): State<GatewayState>, headers: HeaderMap, Json(payload): Json<Value>) -> Response {
    let token = match authorize(&state, &headers) {
        Ok(token) => token,
        Err(resp) => return resp,
    };
    if let Err(resp) = check_rate_limit(&state, &token) {
        return resp;
    }

    let sanitized_payload = sanitize_chat_payload(payload);
    let model = sanitized_payload.get("model").and_then(|v| v.as_str()).map(|v| v.to_string());

    if let Some(upstream_base_url) = state.config.upstream_base_url.clone() {
        let Some(upstream_token) = state.config.upstream_bearer_token.clone() else {
            let resp = error_response(StatusCode::BAD_GATEWAY, "upstream_unavailable", "upstream auth not configured");
            log_usage(&state, &token, "/v1/chat/completions", model, Some("upstream".to_string()), StatusCode::BAD_GATEWAY);
            return resp;
        };
        let url = format!("{}/v1/chat/completions", upstream_base_url.trim_end_matches('/'));
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
                    Err(_) => return error_with_usage(&state, &token, model, StatusCode::BAD_GATEWAY, "upstream_invalid_json", "upstream returned invalid json"),
                };
                log_usage(&state, &token, "/v1/chat/completions", model, Some("upstream".to_string()), status);
                (status, Json(body)).into_response()
            }
            Err(err) if err.is_timeout() => error_with_usage(&state, &token, model, StatusCode::GATEWAY_TIMEOUT, "upstream_timeout", "upstream request timed out"),
            Err(_) => error_with_usage(&state, &token, model, StatusCode::BAD_GATEWAY, "upstream_unavailable", "failed to reach upstream"),
        }
    } else {
        log_usage(&state, &token, "/v1/chat/completions", model.clone(), Some("skeleton".to_string()), StatusCode::OK);
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
        })).into_response()
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

fn authorize_admin(state: &GatewayState, headers: &HeaderMap) -> Result<(), Response> {
    let Some(expected) = state.admin_token.as_deref() else {
        return Err(error_response(StatusCode::FORBIDDEN, "admin_disabled", "admin endpoint disabled"));
    };
    let provided = extract_bearer_or_api_key(headers);
    if provided != Some(expected) {
        return Err(error_response(StatusCode::UNAUTHORIZED, "auth_failed", "invalid admin token"));
    }
    Ok(())
}

fn authorize(state: &GatewayState, headers: &HeaderMap) -> Result<DownstreamToken, Response> {
    let Some(provided) = extract_bearer_or_api_key(headers) else {
        return Err(error_response(StatusCode::UNAUTHORIZED, "auth_failed", "missing bearer token"));
    };

    let Some(token) = state.tokens.get(provided).cloned() else {
        return Err(error_response(StatusCode::UNAUTHORIZED, "auth_failed", "invalid bearer token"));
    };

    if !token.enabled {
        return Err(error_response(StatusCode::FORBIDDEN, "auth_failed", "token disabled"));
    }

    Ok(token)
}

fn extract_bearer_or_api_key(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .or_else(|| headers.get("x-api-key").and_then(|value| value.to_str().ok()))
}

fn check_rate_limit(state: &GatewayState, token: &DownstreamToken) -> Result<(), Response> {
    let mut guard = state.rate_limits.lock().expect("rate limit mutex poisoned");
    let now = Instant::now();
    let window = guard.entry(token.label.clone()).or_insert(TokenWindow { window_started: now, count: 0 });
    if now.duration_since(window.window_started) >= Duration::from_secs(60) {
        window.window_started = now;
        window.count = 0;
    }
    if window.count >= state.config.requests_per_minute {
        return Err(error_response(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "request rate exceeded"));
    }
    window.count += 1;
    Ok(())
}

fn log_usage(state: &GatewayState, token: &DownstreamToken, path: &str, model: Option<String>, upstream_target: Option<String>, status_code: StatusCode) {
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

fn error_with_usage(state: &GatewayState, token: &DownstreamToken, model: Option<String>, status: StatusCode, code: &str, message: &str) -> Response {
    log_usage(state, token, "/v1/chat/completions", model, Some("upstream".to_string()), status);
    error_response(status, code, message)
}

fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({"error": {"code": code, "message": message}}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    fn test_state() -> GatewayState {
        let mut tokens = HashMap::new();
        tokens.insert("test-token".to_string(), DownstreamToken {
            key: "test-token".to_string(),
            label: "local-test".to_string(),
            enabled: true,
        });
        GatewayState {
            admin_token: Some("admin-token".to_string()),
            tokens: Arc::new(tokens),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
            usage_log: Arc::new(Mutex::new(Vec::new())),
            config: GatewayConfig { requests_per_minute: 2, concurrency_per_token: 3, upstream_base_url: None, upstream_bearer_token: None },
            http_client: Client::new(),
        }
    }

    #[tokio::test]
    async fn models_requires_bearer_token() {
        let app = build_gateway_router(test_state());
        let response = app.oneshot(Request::builder().uri("/v1/models").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn models_returns_proxy_catalog_when_authorized() {
        let app = build_gateway_router(test_state());
        let response = app.oneshot(
            Request::builder()
                .uri("/v1/models")
                .header(AUTHORIZATION, "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn models_rate_limit_applies_per_token() {
        let app = build_gateway_router(test_state());
        for _ in 0..2 {
            let response = app.clone().oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .header(AUTHORIZATION, "Bearer test-token")
                    .body(Body::empty())
                    .unwrap(),
            ).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let response = app.oneshot(
            Request::builder()
                .uri("/v1/models")
                .header(AUTHORIZATION, "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn chat_completion_logs_usage_event() {
        let state = test_state();
        let app = build_gateway_router(state.clone());
        let response = app.oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header(AUTHORIZATION, "Bearer test-token")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"model":"agent-proxy-v0","messages":[{"role":"user","content":"hi"}]}"#))
                .unwrap(),
        ).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let log = state.usage_log.lock().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].path, "/v1/chat/completions");
        assert_eq!(log[0].token_label, "local-test");
    }

    #[tokio::test]
    async fn admin_usage_requires_admin_token() {
        let app = build_gateway_router(test_state());
        let response = app.oneshot(Request::builder().uri("/admin/usage").body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
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
