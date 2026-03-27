use serde_json::Value;

#[derive(Debug, Clone)]
pub struct RunnerTask {
    pub task_id: String,
    pub attempt: i64,
    pub kind: String,
    pub payload: Value,
    pub timeout_seconds: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct RunnerExecutionResult {
    pub status: RunnerOutcomeStatus,
    pub result_json: Option<Value>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum RunnerOutcomeStatus {
    Succeeded,
    Failed,
    TimedOut,
}

#[derive(Debug, Clone, Copy)]
pub struct RunnerCapabilities {
    pub supports_timeout: bool,
    pub supports_cancel_running: bool,
    pub supports_artifacts: bool,
}
