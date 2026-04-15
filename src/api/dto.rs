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
    pub fingerprint_medium_max_concurrency: usize,
    pub fingerprint_heavy_max_concurrency: usize,
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
pub struct BrowserSummaryResponse {
    pub title: Option<String>,
    pub final_url: Option<String>,
    pub content_kind: Option<String>,
    pub content_preview: Option<String>,
    pub content_length: Option<i64>,
    pub content_ready: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyPoolStatusSummary {
    pub total: i64,
    pub active: i64,
    pub candidate: i64,
    pub candidate_rejected: i64,
    pub active_ratio_percent: i64,
    pub hot_regions: Vec<String>,
    pub region_shortages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyReplenishMetricsSummary {
    pub recent_batches: i64,
    pub promotion_rate: f64,
    pub reject_rate: f64,
    pub fallback_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySessionMetricsSummary {
    pub active_sessions: i64,
    pub reused_sessions: i64,
    pub created_sessions: i64,
    pub cookie_restore_count: i64,
    pub cookie_persist_count: i64,
    pub local_storage_restore_count: i64,
    pub local_storage_persist_count: i64,
    pub session_storage_restore_count: i64,
    pub session_storage_persist_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySiteMetricsSummary {
    pub tracked_sites: i64,
    pub site_records: i64,
    pub top_failing_sites: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorMetricsResponse {
    pub runs_with_behavior: i64,
    pub shadow_runs: i64,
    pub active_runs: i64,
    pub aborted_by_budget: i64,
    pub avg_added_latency_ms: i64,
    pub top_behavior_profiles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthMetricsResponse {
    pub auth_runs: i64,
    pub auth_success_runs: i64,
    pub auth_failed_runs: i64,
    pub auth_blocked_missing_contract: i64,
    pub auth_transient_retries: i64,
    pub auth_inline_secret_unavailable: i64,
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
    pub proxy_pool_status: ProxyPoolStatusSummary,
    pub proxy_replenish_metrics: ProxyReplenishMetricsSummary,
    pub proxy_harvest_metrics: crate::network_identity::proxy_harvest::ProxyHarvestMetrics,
    pub proxy_site_metrics: ProxySiteMetricsSummary,
    pub identity_session_metrics: IdentitySessionMetricsSummary,
    pub behavior_metrics: BehaviorMetricsResponse,
    pub auth_metrics: AuthMetricsResponse,
    pub latest_execution_summaries: Vec<SummaryArtifactResponse>,
    pub latest_tasks: Vec<TaskResponse>,
    pub latest_browser_tasks: Vec<TaskResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionIntentRequest {
    pub identity_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub proxy_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorBudgetRequest {
    pub max_added_latency_ms: Option<i64>,
    pub timeout_reserve_ms: Option<i64>,
    pub max_step_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorPolicyRequest {
    pub mode: Option<String>,
    pub page_archetype: Option<String>,
    pub allow_site_overrides: Option<bool>,
    pub budget: Option<BehaviorBudgetRequest>,
    pub plan_seed: Option<String>,
    pub allowed_primitives: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormFieldInputRequest {
    pub key: String,
    pub role: String,
    pub selector: Option<String>,
    pub required: Option<bool>,
    pub sensitive: Option<bool>,
    pub value: Option<serde_json::Value>,
    pub secret_ref: Option<String>,
    pub bundle_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSubmitInputRequest {
    pub selector: Option<String>,
    pub trigger: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSuccessInputRequest {
    pub ready_selector: Option<String>,
    pub url_patterns: Option<Vec<String>>,
    pub title_contains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormInputRequest {
    pub mode: String,
    pub form_selector: Option<String>,
    pub secret_bundle_ref: Option<String>,
    pub fields: Vec<FormFieldInputRequest>,
    pub submit: Option<FormSubmitInputRequest>,
    pub success: Option<FormSuccessInputRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub kind: String,
    pub url: Option<String>,
    pub script: Option<String>,
    pub timeout_seconds: Option<i64>,
    pub priority: Option<i32>,
    pub persona_id: Option<String>,
    pub identity_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub proxy_id: Option<String>,
    pub execution_intent: Option<ExecutionIntentRequest>,
    pub behavior_policy_json: Option<serde_json::Value>,
    pub network_policy_json: Option<serde_json::Value>,
    pub requested_operation_kind: Option<String>,
    pub manual_gate_policy: Option<String>,
    pub form_input: Option<FormInputRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuityHeartbeatTickItemResponse {
    pub persona_id: String,
    pub store_id: String,
    pub platform_id: String,
    pub status: String,
    pub reason: String,
    pub task_id: Option<String>,
    pub target_url: Option<String>,
    pub heartbeat_interval_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinuityHeartbeatTickResponse {
    pub ticked_at: String,
    pub evaluated_count: i64,
    pub scheduled_count: i64,
    pub skipped_count: i64,
    pub items: Vec<ContinuityHeartbeatTickItemResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserOpenRequest {
    pub url: String,
    pub timeout_seconds: Option<i64>,
    pub priority: Option<i32>,
    pub identity_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub proxy_id: Option<String>,
    pub execution_intent: Option<ExecutionIntentRequest>,
    pub behavior_policy_json: Option<serde_json::Value>,
    pub network_policy_json: Option<serde_json::Value>,
    pub form_input: Option<FormInputRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserGetHtmlRequest {
    pub url: String,
    pub timeout_seconds: Option<i64>,
    pub priority: Option<i32>,
    pub identity_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub proxy_id: Option<String>,
    pub execution_intent: Option<ExecutionIntentRequest>,
    pub behavior_policy_json: Option<serde_json::Value>,
    pub network_policy_json: Option<serde_json::Value>,
    pub form_input: Option<FormInputRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserGetTitleRequest {
    pub url: String,
    pub timeout_seconds: Option<i64>,
    pub priority: Option<i32>,
    pub identity_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub proxy_id: Option<String>,
    pub execution_intent: Option<ExecutionIntentRequest>,
    pub behavior_policy_json: Option<serde_json::Value>,
    pub network_policy_json: Option<serde_json::Value>,
    pub form_input: Option<FormInputRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserGetFinalUrlRequest {
    pub url: String,
    pub timeout_seconds: Option<i64>,
    pub priority: Option<i32>,
    pub identity_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub proxy_id: Option<String>,
    pub execution_intent: Option<ExecutionIntentRequest>,
    pub behavior_policy_json: Option<serde_json::Value>,
    pub network_policy_json: Option<serde_json::Value>,
    pub form_input: Option<FormInputRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserExtractTextRequest {
    pub url: String,
    pub timeout_seconds: Option<i64>,
    pub priority: Option<i32>,
    pub identity_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub proxy_id: Option<String>,
    pub execution_intent: Option<ExecutionIntentRequest>,
    pub behavior_policy_json: Option<serde_json::Value>,
    pub network_policy_json: Option<serde_json::Value>,
    pub form_input: Option<FormInputRequest>,
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
pub struct TrustScoreComponents {
    pub verify_ok_bonus: i64,
    pub verify_geo_match_bonus: i64,
    pub site_success_bonus: i64,
    pub geo_mismatch_penalty: i64,
    pub region_mismatch_penalty: i64,
    pub geo_risk_penalty: i64,
    pub smoke_upstream_ok_bonus: i64,
    pub raw_score_component: i64,
    pub missing_verify_penalty: i64,
    pub stale_verify_penalty: i64,
    pub verify_failed_heavy_penalty: i64,
    pub verify_failed_light_penalty: i64,
    pub verify_failed_base_penalty: i64,
    pub individual_history_penalty: i64,
    pub provider_risk_penalty: i64,
    pub provider_region_cluster_penalty: i64,
    pub verify_confidence_bonus: i64,
    pub verify_score_delta_bonus: i64,
    pub verify_source_bonus: i64,
    pub anonymity_bonus: i64,
    pub latency_penalty: i64,
    pub exit_ip_not_public_penalty: i64,
    pub probe_error_penalty: i64,
    pub verify_risk_penalty: i64,
    pub site_failure_penalty: i64,
    pub soft_min_score_penalty: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRankPreviewItem {
    pub id: String,
    pub provider: Option<String>,
    pub region: Option<String>,
    pub score: f64,
    pub trust_score_total: i64,
    pub summary: String,
    pub winner_vs_runner_up_diff: Option<WinnerVsRunnerUpDiff>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyPoolHealthAssessmentExplain {
    pub available_ratio_percent: i64,
    pub healthy_ratio_band: String,
    pub below_min_ratio: bool,
    pub above_max_ratio: bool,
    pub below_min_total: bool,
    pub below_min_region: bool,
    pub require_replenish: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionMatchExplain {
    pub target_region: Option<String>,
    pub proxy_region: Option<String>,
    pub match_mode: String,
    pub matches: bool,
    pub score: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyGrowthExplain {
    pub target_region: Option<String>,
    pub selected_proxy_region: Option<String>,
    pub inventory_snapshot:
        Option<crate::network_identity::proxy_growth::ProxyPoolInventorySnapshot>,
    pub health_assessment: Option<ProxyPoolHealthAssessmentExplain>,
    pub region_match: Option<RegionMatchExplain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxySelectionExplain {
    pub selection_mode: Option<String>,
    pub explicit_override: Option<bool>,
    pub sticky_reused: Option<bool>,
    pub sticky_binding_age_seconds: Option<i64>,
    pub sticky_reuse_reason: Option<String>,
    pub would_rank_position_if_auto: Option<i64>,
    pub eligibility_gate: Option<String>,
    pub soft_min_score: Option<f64>,
    pub soft_min_score_penalty_applied: Option<bool>,
    pub fallback_reason: Option<String>,
    pub no_match_reason_code: Option<String>,
    pub proxy_growth: Option<ProxyGrowthExplain>,
    pub fingerprint_budget_tag: Option<String>,
    pub fingerprint_budget_medium_limit: Option<usize>,
    pub fingerprint_budget_heavy_limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumptionExplain {
    pub declared_fields: Vec<String>,
    pub resolved_fields: Vec<String>,
    pub applied_fields: Vec<String>,
    pub ignored_fields: Vec<String>,
    pub declared_count: usize,
    pub resolved_count: usize,
    pub applied_count: usize,
    pub ignored_count: usize,
    pub consumption_status: String,
    pub partial_support_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintRuntimeExplain {
    pub fingerprint_budget_tag: Option<String>,
    pub fingerprint_consistency:
        Option<crate::network_identity::fingerprint_consistency::FingerprintConsistencyAssessment>,
    pub consumption_explain: Option<ConsumptionExplain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryArtifactResponse {
    pub category: String,
    pub key: String,
    pub source: String,
    pub severity: String,
    pub title: String,
    pub summary: String,
    pub task_id: Option<String>,
    pub task_kind: Option<String>,
    pub task_status: Option<String>,
    pub run_id: Option<String>,
    pub attempt: Option<i32>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionIdentity {
    pub fingerprint_profile_id: Option<String>,
    pub fingerprint_profile_version: Option<i64>,
    pub fingerprint_resolution_status: Option<String>,
    pub fingerprint_runtime_explain: Option<FingerprintRuntimeExplain>,
    pub behavior_profile_id: Option<String>,
    pub behavior_profile_version: Option<i64>,
    pub behavior_resolution_status: Option<String>,
    pub behavior_execution_mode: Option<String>,
    pub page_archetype: Option<String>,
    pub behavior_seed: Option<String>,
    pub behavior_runtime_explain: Option<crate::behavior::BehaviorRuntimeExplain>,
    pub behavior_trace_summary: Option<crate::behavior::BehaviorTraceSummary>,
    pub proxy_id: Option<String>,
    pub proxy_provider: Option<String>,
    pub proxy_region: Option<String>,
    pub proxy_resolution_status: Option<String>,
    pub selection_reason_summary: Option<String>,
    pub selection_explain: Option<ProxySelectionExplain>,
    pub trust_score_total: Option<i64>,
    pub identity_session_status: Option<String>,
    pub cookie_restore_count: Option<i64>,
    pub cookie_persist_count: Option<i64>,
    pub local_storage_restore_count: Option<i64>,
    pub local_storage_persist_count: Option<i64>,
    pub session_storage_restore_count: Option<i64>,
    pub session_storage_persist_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityNetworkExplain {
    pub execution_identity: ExecutionIdentity,
    pub selection_explain: Option<ProxySelectionExplain>,
    pub fingerprint_runtime_explain: Option<FingerprintRuntimeExplain>,
    pub proxy_id: Option<String>,
    pub proxy_provider: Option<String>,
    pub proxy_region: Option<String>,
    pub proxy_resolution_status: Option<String>,
    pub selection_reason_summary: Option<String>,
    pub trust_score_total: Option<i64>,
    pub identity_session_status: Option<String>,
    pub cookie_restore_count: Option<i64>,
    pub cookie_persist_count: Option<i64>,
    pub local_storage_restore_count: Option<i64>,
    pub local_storage_persist_count: Option<i64>,
    pub session_storage_restore_count: Option<i64>,
    pub session_storage_persist_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub priority: i32,
    pub persona_id: Option<String>,
    pub platform_id: Option<String>,
    pub manual_gate_request_id: Option<String>,
    pub manual_gate_status: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub summary_artifacts: Vec<SummaryArtifactResponse>,
    pub fingerprint_profile_id: Option<String>,
    pub fingerprint_profile_version: Option<i64>,
    pub fingerprint_resolution_status: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub behavior_profile_version: Option<i64>,
    pub behavior_resolution_status: Option<String>,
    pub behavior_execution_mode: Option<String>,
    pub page_archetype: Option<String>,
    pub behavior_seed: Option<String>,
    pub behavior_runtime_explain: Option<crate::behavior::BehaviorRuntimeExplain>,
    pub behavior_trace_summary: Option<crate::behavior::BehaviorTraceSummary>,
    pub form_action_status: Option<String>,
    pub form_action_mode: Option<String>,
    pub form_action_retry_count: Option<i64>,
    pub form_action_summary_json: Option<serde_json::Value>,
    pub proxy_id: Option<String>,
    pub proxy_provider: Option<String>,
    pub proxy_region: Option<String>,
    pub proxy_resolution_status: Option<String>,
    pub trust_score_total: Option<i64>,
    pub selection_reason_summary: Option<String>,
    pub selection_explain: Option<ProxySelectionExplain>,
    pub fingerprint_runtime_explain: Option<FingerprintRuntimeExplain>,
    pub execution_identity: Option<ExecutionIdentity>,
    pub identity_network_explain: Option<IdentityNetworkExplain>,
    pub winner_vs_runner_up_diff: Option<WinnerVsRunnerUpDiff>,
    pub failure_scope: Option<String>,
    pub browser_failure_signal: Option<String>,
    pub title: Option<String>,
    pub final_url: Option<String>,
    pub content_preview: Option<String>,
    pub content_length: Option<i64>,
    pub content_truncated: Option<bool>,
    pub content_kind: Option<String>,
    pub content_source_action: Option<String>,
    pub content_ready: Option<bool>,
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
    pub summary_artifacts: Vec<SummaryArtifactResponse>,
    pub behavior_profile_id: Option<String>,
    pub behavior_profile_version: Option<i64>,
    pub behavior_resolution_status: Option<String>,
    pub behavior_execution_mode: Option<String>,
    pub page_archetype: Option<String>,
    pub behavior_seed: Option<String>,
    pub behavior_runtime_explain: Option<crate::behavior::BehaviorRuntimeExplain>,
    pub behavior_trace_summary: Option<crate::behavior::BehaviorTraceSummary>,
    pub form_action_status: Option<String>,
    pub form_action_mode: Option<String>,
    pub form_action_retry_count: Option<i64>,
    pub form_action_summary_json: Option<serde_json::Value>,
    pub proxy_id: Option<String>,
    pub proxy_provider: Option<String>,
    pub proxy_region: Option<String>,
    pub proxy_resolution_status: Option<String>,
    pub trust_score_total: Option<i64>,
    pub selection_reason_summary: Option<String>,
    pub selection_explain: Option<ProxySelectionExplain>,
    pub fingerprint_runtime_explain: Option<FingerprintRuntimeExplain>,
    pub execution_identity: Option<ExecutionIdentity>,
    pub identity_network_explain: Option<IdentityNetworkExplain>,
    pub winner_vs_runner_up_diff: Option<WinnerVsRunnerUpDiff>,
    pub failure_scope: Option<String>,
    pub browser_failure_signal: Option<String>,
    pub title: Option<String>,
    pub final_url: Option<String>,
    pub content_preview: Option<String>,
    pub content_length: Option<i64>,
    pub content_truncated: Option<bool>,
    pub content_kind: Option<String>,
    pub content_source_action: Option<String>,
    pub content_ready: Option<bool>,
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
pub struct CreateBehaviorProfileRequest {
    pub id: String,
    pub name: String,
    pub status: Option<String>,
    pub tags_json: Option<String>,
    pub profile_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateBehaviorProfileRequest {
    pub name: Option<String>,
    pub status: Option<String>,
    pub tags_json: Option<String>,
    pub profile_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorProfileResponse {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub tags_json: Option<String>,
    pub profile_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateIdentityProfileRequest {
    pub id: String,
    pub name: String,
    pub status: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub identity_json: serde_json::Value,
    pub secret_aliases_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateIdentityProfileRequest {
    pub name: Option<String>,
    pub status: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub identity_json: Option<serde_json::Value>,
    pub secret_aliases_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityProfileResponse {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub identity_json: serde_json::Value,
    pub secret_aliases_json: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNetworkProfileRequest {
    pub id: String,
    pub name: String,
    pub status: Option<String>,
    pub network_policy_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNetworkProfileRequest {
    pub name: Option<String>,
    pub status: Option<String>,
    pub network_policy_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProfileResponse {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub network_policy_json: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSessionProfileRequest {
    pub id: String,
    pub name: String,
    pub status: Option<String>,
    pub continuity_mode: String,
    pub retention_policy_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSessionProfileRequest {
    pub name: Option<String>,
    pub status: Option<String>,
    pub continuity_mode: Option<String>,
    pub retention_policy_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionProfileResponse {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub continuity_mode: String,
    pub retention_policy_json: Option<serde_json::Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSiteBehaviorPolicyRequest {
    pub id: String,
    pub site_key: String,
    pub page_archetype: Option<String>,
    pub action_kind: Option<String>,
    pub behavior_profile_id: String,
    pub priority: Option<i64>,
    pub required: Option<bool>,
    pub override_json: Option<serde_json::Value>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSiteBehaviorPolicyRequest {
    pub site_key: Option<String>,
    pub page_archetype: Option<String>,
    pub action_kind: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub priority: Option<i64>,
    pub required: Option<bool>,
    pub override_json: Option<serde_json::Value>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteBehaviorPolicyResponse {
    pub id: String,
    pub version: i64,
    pub site_key: String,
    pub page_archetype: Option<String>,
    pub action_kind: Option<String>,
    pub behavior_profile_id: String,
    pub priority: i64,
    pub required: bool,
    pub override_json: Option<serde_json::Value>,
    pub status: String,
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
    pub region_match_ok: Option<bool>,
    pub identity_fields_complete: Option<bool>,
    pub risk_level: Option<String>,
    pub risk_reasons: Vec<String>,
    pub failure_stage: Option<String>,
    pub failure_stage_detail: Option<String>,
    pub anonymity_level: Option<String>,
    pub latency_ms: Option<u128>,
    pub probe_error: Option<String>,
    pub probe_error_category: Option<String>,
    pub verification_confidence: Option<f64>,
    pub verification_class: Option<String>,
    pub recommended_action: Option<String>,
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
    pub soft_min_score: Option<f64>,
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
    pub trust_score_cached_at: Option<String>,
    pub explain_generated_at: String,
    pub explain_source: String,
    pub provider_risk_version_current: Option<i64>,
    pub provider_risk_version_seen: Option<i64>,
    pub provider_risk_version_status: String,
    pub selection_reason_summary: String,
    pub trust_score_components: TrustScoreComponents,
    pub candidate_rank_preview: Vec<CandidateRankPreviewItem>,
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
