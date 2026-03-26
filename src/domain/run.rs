use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub task_id: String,
    pub status: RunStatus,
    pub attempt: i32,
    pub runner_kind: String,
}
