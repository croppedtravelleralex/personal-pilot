pub mod state;

use std::sync::Arc;

use crate::{
    db::init::DbPool,
    queue::memory::MemoryTaskQueue,
    runner::TaskRunner,
};

use self::state::AppState;

pub fn build_app_state(db: DbPool, runner: Arc<dyn TaskRunner>, api_key: Option<String>) -> AppState {
    AppState {
        db,
        queue: MemoryTaskQueue::new(),
        api_key,
        runner,
    }
}
