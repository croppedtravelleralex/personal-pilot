use std::{collections::VecDeque, sync::{Arc, Mutex}};

#[derive(Debug, Clone)]
pub struct MemoryTaskQueue {
    inner: Arc<Mutex<VecDeque<String>>>,
}

impl MemoryTaskQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn push(&self, task_id: String) {
        let mut guard = self.inner.lock().expect("memory queue poisoned");
        guard.push_back(task_id);
    }

    pub fn pop(&self) -> Option<String> {
        let mut guard = self.inner.lock().expect("memory queue poisoned");
        guard.pop_front()
    }

    pub fn remove(&self, task_id: &str) -> bool {
        let mut guard = self.inner.lock().expect("memory queue poisoned");
        if let Some(index) = guard.iter().position(|id| id == task_id) {
            guard.remove(index);
            true
        } else {
            false
        }
    }

    pub fn len(&self) -> usize {
        let guard = self.inner.lock().expect("memory queue poisoned");
        guard.len()
    }
}
