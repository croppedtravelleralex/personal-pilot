use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    db::init::DbPool, network_identity::proxy_selection::ProxySelectionTuning,
    queue::memory::MemoryTaskQueue, runner::TaskRunner,
};

pub type InlineSecretVault = Arc<Mutex<HashMap<String, serde_json::Value>>>;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub queue: MemoryTaskQueue,
    pub api_key: Option<String>,
    pub runner: Arc<dyn TaskRunner>,
    pub worker_count: usize,
    pub proxy_runtime_mode: String,
    pub proxy_selection_tuning: ProxySelectionTuning,
    pub inline_secret_vault: InlineSecretVault,
}
