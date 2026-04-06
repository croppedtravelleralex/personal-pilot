use std::{collections::HashMap, sync::{Arc, Mutex}, time::{Duration, Instant}};

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct GatewayState {
    pub admin_token: Option<String>,
    pub tokens: Arc<HashMap<String, DownstreamToken>>,
    pub rate_limits: Arc<Mutex<HashMap<String, TokenWindow>>>,
    pub config: GatewayConfig,
}

#[derive(Clone, Debug)]
pub struct GatewayConfig {
    pub requests_per_minute: u32,
    pub concurrency_per_token: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownstreamToken {
    pub key: String,
    pub label: String,
    pub enabled: bool,
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
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .with_state(state)
}

pub fn gateway_state_from_env() -> GatewayState {
    let admin_token = std::env::var("GATEWAY_ADMIN_TOKEN").ok().filter(|v| !v.trim().is_empty());
    let requests_per_minute = std::env::var("GATEWAY_RATE_LIMIT_PER_MINUTE").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(30);
    let concurrency_per_token = std::env::var("GATEWAY_CONCURRENCY_PER_TOKEN").ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(3);
    let tokens_json = std::env::var("GATEWAY_DOWNSTREAM_TOKENS_JSON").unwrap_or_else(|_| "[]".to_string());
    let parsed: Vec<DownstreamToken> = serde_json::from_str(&tokens_json).unwrap_or_default();
    let mut map = HashMap::new();
    for token in parsed {
        map.insert(token.key.clone(), token);
    }
    GatewayState {
        admin_token,
        tokens: Arc::new(map),
        rate_limits: Arc::new(Mutex::new(HashMap::new())),
        config: GatewayConfig { requests_per_minute, concurrency_per_token },
    }
}

async fn gateway_health() -> impl IntoResponse {
    Json(json!({"status":"ok","service":"agent-gateway-v0"}))
}

async fn list_models(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    let token = match authorize(&state, &headers) {
        Ok(token) => token,
        Err(resp) => return resp,
    };
    if let Err(resp) = check_rate_limit(&state, &token) {
        return resp;
    }
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

    let model = payload.get("model").and_then(|v| v.as_str()).unwrap_or("agent-proxy-v0");
    let message_count = payload.get("messages").and_then(|v| v.as_array()).map(|v| v.len()).unwrap_or(0);

    Json(json!({
        "id": "chatcmpl-agent-gateway-v0",
        "object": "chat.completion",
        "created": 1775450000,
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": format!("gateway skeleton ok; accepted {} messages; upstream not wired yet", message_count)
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

fn authorize(state: &GatewayState, headers: &HeaderMap) -> Result<DownstreamToken, Response> {
    let provided = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .or_else(|| headers.get("x-api-key").and_then(|value| value.to_str().ok()));

    let Some(provided) = provided else {
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
            admin_token: None,
            tokens: Arc::new(tokens),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
            config: GatewayConfig { requests_per_minute: 2, concurrency_per_token: 3 },
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
}
