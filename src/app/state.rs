use crate::{db::init::DbPool, queue::memory::MemoryTaskQueue};

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub queue: MemoryTaskQueue,
}
