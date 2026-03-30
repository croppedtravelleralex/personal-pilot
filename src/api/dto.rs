use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusCounts {
    pub total: i64,
    pub queued: i64,
    pub running: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub timed_out: i64,
    pub cancelled: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub queue_len: usize,
    pub counts: TaskStatusCounts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerStatusResponse {
    pub worker_count: usize,
    pub queue_mode: String,
    pub reclaim_after_seconds: Option<u64>,
    pub heartbeat_interval_seconds: u64,
    pub claim_retry_limit: u32,
    pub idle_backoff_min_ms: u64,
    pub idle_backoff_max_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintMetricsResponse {
    pub pending: i64,
    pub resolved: i64,
    pub downgraded: i64,
    pub none: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyMetricsResponse {
    pub direct: i64,
    pub resolved: i64,
    pub resolved_sticky: i64,
    pub unresolved: i64,
    pub none: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub service: String,
    pub queue_len: usize,
    pub counts: TaskStatusCounts,
    pub worker: WorkerStatusResponse,
    pub fingerprint_metrics: FingerprintMetricsResponse,
    pub proxy_metrics: ProxyMetricsResponse,
    pub latest_tasks: Vec<TaskResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub kind: String,
    pub url: Option<String>,
    pub script: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub priority: Option<i32>,
    pub fingerprint_profile_id: Option<String>,
    pub network_policy_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryTaskResponse {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelTaskResponse {
    pub id: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub priority: i32,
    pub fingerprint_profile_id: Option<String>,
    pub fingerprint_profile_version: Option<i64>,
    pub fingerprint_resolution_status: Option<String>,
    pub proxy_id: Option<String>,
    pub proxy_provider: Option<String>,
    pub proxy_region: Option<String>,
    pub proxy_resolution_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResponse {
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
pub struct LogResponse {
    pub id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub level: String,
    pub message: String,
    pub created_at: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFingerprintProfileRequest {
    pub id: String,
    pub name: String,
    pub tags_json: Option<String>,
    pub profile_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintProfileResponse {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub tags_json: Option<String>,
    pub profile_json: serde_json::Value,
    pub validation_ok: bool,
    pub validation_issues: Vec<crate::network_identity::validator::FingerprintValidationIssue>,
    pub created_at: String,
    pub updated_at: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProxyRequest {
    pub id: String,
    pub scheme: String,
    pub host: String,
    pub port: i64,
    pub username: Option<String>,
    pub password: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub provider: Option<String>,
    pub status: Option<String>,
    pub score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyResponse {
    pub id: String,
    pub scheme: String,
    pub host: String,
    pub port: i64,
    pub username: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub provider: Option<String>,
    pub status: String,
    pub score: f64,
    pub success_count: i64,
    pub failure_count: i64,
    pub last_checked_at: Option<String>,
    pub last_used_at: Option<String>,
    pub cooldown_until: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySmokeResponse {
    pub id: String,
    pub reachable: bool,
    pub latency_ms: Option<u128>,
    pub status: String,
    pub message: String,
}
