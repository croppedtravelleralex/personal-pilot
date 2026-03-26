use axum::{routing::{get, post}, Router};

use crate::app::state::AppState;

use super::handlers::{
    cancel_task, create_task, get_task, get_task_logs, get_task_runs, health, retry_task, status,
};

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/status", get(status))
        .route("/tasks", post(create_task))
        .route("/tasks/:id", get(get_task))
        .route("/tasks/:id/runs", get(get_task_runs))
        .route("/tasks/:id/logs", get(get_task_logs))
        .route("/tasks/:id/retry", post(retry_task))
        .route("/tasks/:id/cancel", post(cancel_task))
        .with_state(state)
}
