use axum::{middleware, routing::{get, post}, Router};

use crate::app::state::AppState;

use super::{auth::auth_middleware, handlers::{
    cancel_task, create_fingerprint_profile, create_proxy, create_task, get_fingerprint_profile,
    get_proxy, get_task, get_task_logs, get_task_runs, health, list_fingerprint_profiles,
    list_proxies, retry_task, smoke_test_proxy, verify_proxy, status,
}};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/tasks", post(create_task))
        .route("/fingerprint-profiles", post(create_fingerprint_profile).get(list_fingerprint_profiles))
        .route("/proxies", post(create_proxy).get(list_proxies))
        .route("/fingerprint-profiles/:id", get(get_fingerprint_profile))
        .route("/proxies/:id", get(get_proxy))
        .route("/proxies/:id/smoke", post(smoke_test_proxy))
        .route("/proxies/:id/verify", post(verify_proxy))
        .route("/tasks/:id", get(get_task))
        .route("/tasks/:id/runs", get(get_task_runs))
        .route("/tasks/:id/logs", get(get_task_logs))
        .route("/tasks/:id/retry", post(retry_task))
        .route("/tasks/:id/cancel", post(cancel_task))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth_middleware))
}
