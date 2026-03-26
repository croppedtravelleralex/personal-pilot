use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Created,
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    pub url: Option<String>,
    pub script: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub kind: String,
    pub status: TaskStatus,
    pub priority: i32,
    pub input: TaskInput,
    pub created_at: String,
}
