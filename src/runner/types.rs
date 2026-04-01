use serde_json::Value;

#[derive(Debug, Clone)]
pub struct RunnerFingerprintProfile {
    pub id: String,
    pub version: i64,
    pub profile_json: Value,
}

#[derive(Debug, Clone)]
pub struct RunnerProxySelection {
    pub id: String,
    pub scheme: String,
    pub host: String,
    pub port: i64,
    pub username: Option<String>,
    pub password: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub provider: Option<String>,
    pub score: f64,
    pub resolution_status: String,
}

#[derive(Debug, Clone)]
pub struct RunnerTask {
    pub task_id: String,
    pub attempt: i64,
    pub kind: String,
    pub payload: Value,
    pub timeout_seconds: Option<i64>,
    pub fingerprint_profile: Option<RunnerFingerprintProfile>,
    pub proxy: Option<RunnerProxySelection>,
}

#[derive(Debug, Clone)]
pub struct RunnerSummaryArtifact {
    pub category: SummaryArtifactCategory,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Copy)]
pub enum SummaryArtifactCategory {
    Result,
    Debug,
    Transient,
    Summary,
}

pub struct RunnerExecutionResult {
    pub status: RunnerOutcomeStatus,
    pub result_json: Option<Value>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunnerCancelResult {
    pub accepted: bool,
    pub message: String,
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
