use std::net::SocketAddr;

use anyhow::Result;
use axum::serve;
use tokio::net::TcpListener;

use AutoOpenBrowser::{
    api::routes::build_router,
    app::state::AppState,
    db::init::init_db,
    queue::memory::MemoryTaskQueue,
    runner::fake::spawn_fake_runner_loop,
};

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = "sqlite://data/auto_open_browser.db";
    let db = init_db(database_url).await?;
    let queue = MemoryTaskQueue::new();
    let state = AppState { db, queue };

    spawn_fake_runner_loop(state.clone()).await;

    let app = build_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = TcpListener::bind(addr).await?;

    println!("AutoOpenBrowser listening on http://{}", addr);
    println!("Database initialized at {}", database_url);
    serve(listener, app).await?;

    Ok(())
}
