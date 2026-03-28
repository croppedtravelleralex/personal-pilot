pub mod app;
pub mod api;
pub mod domain;
pub mod db;
pub mod queue;
pub mod runner;
pub mod network_identity;

use std::sync::Arc;

use axum::Router;

use crate::{
    api::routes::build_router,
    app::{build_app_state, state::AppState},
    db::init::{init_db, DbPool},
    runner::{fake::FakeRunner, spawn_runner_loop, TaskRunner},
};

pub async fn build_test_app(database_url: &str) -> anyhow::Result<(AppState, Router)> {
    let db = init_db(database_url).await?;
    let runner: Arc<dyn TaskRunner> = Arc::new(FakeRunner);
    let state = build_app_state(db, runner.clone(), None);
    spawn_runner_loop(state.clone(), runner).await;
    let app = build_router(state.clone());
    Ok((state, app))
}

pub async fn build_test_app_with_db(database_url: &str, db: DbPool) -> anyhow::Result<(AppState, Router)> {
    let _ = database_url;
    let runner: Arc<dyn TaskRunner> = Arc::new(FakeRunner);
    let state = build_app_state(db, runner.clone(), None);
    spawn_runner_loop(state.clone(), runner).await;
    let app = build_router(state.clone());
    Ok((state, app))
}
