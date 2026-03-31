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
pub struct VerifyMetricsResponse {
    pub verified_ok: i64,
    pub verified_failed: i64,
    pub geo_match_ok: i64,
    pub stale_or_missing_verify: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub service: String,
    pub queue_len: usize,
    pub counts: TaskStatusCounts,
    pub worker: WorkerStatusResponse,
    pub fingerprint_metrics: FingerprintMetricsResponse,
    pub proxy_metrics: ProxyMetricsResponse,
    pub verify_metrics: VerifyMetricsResponse,
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
    pub proxy_id: Option<String>,
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
#[serde(rename_all = "snake_case")]
pub enum WinnerVsRunnerUpDirection {
    Winner,
    RunnerUp,
    Neutral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinnerVsRunnerUpFactor {
    pub factor: String,
    pub label: String,
    pub winner_value: i64,
    pub runner_up_value: i64,
    pub delta: i64,
    pub direction: WinnerVsRunnerUpDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WinnerVsRunnerUpDiff {
    pub winner_total_score: i64,
    pub runner_up_total_score: i64,
    pub score_gap: i64,
    pub factors: Vec<WinnerVsRunnerUpFactor>,
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
    pub trust_score_total: Option<i64>,
    pub selection_reason_summary: Option<String>,
    pub winner_vs_runner_up_diff: Option<WinnerVsRunnerUpDiff>,
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
    pub last_smoke_status: Option<String>,
    pub last_smoke_protocol_ok: Option<bool>,
    pub last_smoke_upstream_ok: Option<bool>,
    pub last_exit_ip: Option<String>,
    pub last_anonymity_level: Option<String>,
    pub last_smoke_at: Option<String>,
    pub last_verify_status: Option<String>,
    pub last_verify_geo_match_ok: Option<bool>,
    pub last_exit_country: Option<String>,
    pub last_exit_region: Option<String>,
    pub last_verify_at: Option<String>,
    pub last_probe_latency_ms: Option<i64>,
    pub last_probe_error: Option<String>,
    pub last_probe_error_category: Option<String>,
    pub last_verify_confidence: Option<f64>,
    pub last_verify_score_delta: Option<i64>,
    pub last_verify_source: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySmokeResponse {
    pub id: String,
    pub reachable: bool,
    pub protocol_ok: bool,
    pub upstream_ok: bool,
    pub exit_ip: Option<String>,
    pub anonymity_level: Option<String>,
    pub latency_ms: Option<u128>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyVerifyResponse {
    pub id: String,
    pub reachable: bool,
    pub protocol_ok: bool,
    pub upstream_ok: bool,
    pub exit_ip: Option<String>,
    pub exit_country: Option<String>,
    pub exit_region: Option<String>,
    pub geo_match_ok: Option<bool>,
    pub anonymity_level: Option<String>,
    pub latency_ms: Option<u128>,
    pub probe_error: Option<String>,
    pub probe_error_category: Option<String>,
    pub verification_confidence: Option<f64>,
    pub verification_score_delta: Option<i64>,
    pub verify_source: Option<String>,
    pub status: String,
    pub message: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyVerifyBatchRequest {
    pub provider: Option<String>,
    pub region: Option<String>,
    pub limit: Option<i64>,
    pub only_stale: Option<bool>,
    pub min_score: Option<f64>,
    pub stale_after_seconds: Option<i64>,
    pub task_timeout_seconds: Option<i64>,
    pub recently_used_within_seconds: Option<i64>,
    pub failed_only: Option<bool>,
    pub max_per_provider: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyVerifyBatchProviderSummary {
    pub provider: String,
    pub accepted: i64,
    pub skipped_due_to_cap: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyVerifyBatchResponse {
    pub batch_id: String,
    pub created_at: String,
    pub requested: i64,
    pub accepted: i64,
    pub skipped: i64,
    pub stale_after_seconds: i64,
    pub task_timeout_seconds: i64,
    pub provider_summary: Vec<ProxyVerifyBatchProviderSummary>,
    pub status: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyBatchResponse {
    pub id: String,
    pub status: String,
    pub requested_count: i64,
    pub accepted_count: i64,
    pub skipped_count: i64,
    pub queued_count: i64,
    pub running_count: i64,
    pub succeeded_count: i64,
    pub failed_count: i64,
    pub stale_after_seconds: i64,
    pub task_timeout_seconds: i64,
    pub provider_summary_json: Option<serde_json::Value>,
    pub filters_json: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyBatchListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySelectionExplainResponse {
    pub proxy_id: String,
    pub trust_score_total: Option<i64>,
    pub selection_reason_summary: String,
    pub trust_score_components: serde_json::Value,
    pub candidate_rank_preview: Vec<serde_json::Value>,
    pub winner_vs_runner_up_diff: Option<WinnerVsRunnerUpDiff>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTrustCacheCheckResponse {
    pub proxy_id: String,
    pub cached_trust_score: Option<i64>,
    pub recomputed_trust_score: Option<i64>,
    pub delta: Option<i64>,
    pub in_sync: bool,
    pub cached_at: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTrustCacheRepairResponse {
    pub proxy_id: String,
    pub cached_trust_score: Option<i64>,
    pub recomputed_trust_score: Option<i64>,
    pub delta: Option<i64>,
    pub in_sync: bool,
    pub repaired: bool,
    pub cached_at: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTrustCacheScanItem {
    pub proxy_id: String,
    pub provider: Option<String>,
    pub cached_trust_score: Option<i64>,
    pub recomputed_trust_score: Option<i64>,
    pub delta: Option<i64>,
    pub in_sync: bool,
    pub cached_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTrustCacheScanResponse {
    pub total: usize,
    pub drifted: usize,
    pub items: Vec<ProxyTrustCacheScanItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTrustCacheRepairBatchResponse {
    pub scanned: usize,
    pub repaired: usize,
    pub remaining_drifted: usize,
    pub items: Vec<ProxyTrustCacheScanItem>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTrustCacheMaintenanceResponse {
    pub scanned_before: usize,
    pub drifted_before: usize,
    pub repaired: usize,
    pub remaining_drifted: usize,
    pub ok: bool,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyTrustCacheScanQuery {
    pub limit: Option<usize>,
    pub only_drifted: Option<bool>,
    pub provider: Option<String>,
}
