pub mod state;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    db::init::DbPool, network_identity::proxy_selection::proxy_selection_tuning_from_env,
    queue::memory::MemoryTaskQueue, runner::TaskRunner,
};

use self::state::AppState;

pub fn build_app_state(
    db: DbPool,
    runner: Arc<dyn TaskRunner>,
    api_key: Option<String>,
    worker_count: usize,
) -> AppState {
    AppState {
        db,
        queue: MemoryTaskQueue::new(),
        api_key,
        runner,
        worker_count,
        proxy_selection_tuning: proxy_selection_tuning_from_env(),
        inline_secret_vault: Arc::new(Mutex::new(HashMap::new())),
    }
}
