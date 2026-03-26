use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub input_json: String,
    pub network_policy_json: Option<String>,
    pub fingerprint_profile_json: Option<String>,
    pub priority: i32,
    pub created_at: String,
    pub queued_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub task_id: String,
    pub status: String,
    pub attempt: i32,
    pub runner_kind: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub kind: String,
    pub storage_path: String,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    pub id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub level: String,
    pub message: String,
    pub created_at: String,
}
