use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerFingerprintProfile {
    pub id: String,
    pub version: i64,
    pub profile_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerBehaviorProfile {
    pub id: String,
    pub version: i64,
    pub profile_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunnerExecutionIntent {
    pub identity_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub proxy_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerBehaviorPlan {
    pub plan_version: i64,
    pub seed: String,
    pub page_archetype: Option<String>,
    pub budget_json: Option<Value>,
    pub steps_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerFormFieldPlan {
    pub key: String,
    pub role: String,
    pub selector: Option<String>,
    pub selector_source: String,
    pub required: bool,
    pub sensitive: bool,
    pub value_source: String,
    pub secret_ref: Option<String>,
    pub bundle_key: Option<String>,
    pub resolved_value: Option<Value>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerFormSubmitPlan {
    pub selector: String,
    pub selector_source: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerFormSuccessPlan {
    pub ready_selector: String,
    pub ready_selector_source: String,
    pub url_patterns: Vec<String>,
    pub title_contains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerFormErrorSignals {
    pub login_error: Vec<String>,
    pub field_error: Vec<String>,
    pub account_locked: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerFormActionPlan {
    pub plan_version: i64,
    pub mode: String,
    pub execution_mode: String,
    pub site_policy_id: Option<String>,
    pub site_policy_version: Option<i64>,
    pub form_selector: Option<String>,
    pub form_selector_source: String,
    pub secret_bundle_ref: Option<String>,
    pub fields: Vec<RunnerFormFieldPlan>,
    pub submit: Option<RunnerFormSubmitPlan>,
    pub success: Option<RunnerFormSuccessPlan>,
    pub error_signals: Option<RunnerFormErrorSignals>,
    pub retry_limit: i64,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub source_label: Option<String>,
    pub source_tier: Option<String>,
    pub verification_path: Option<String>,
    pub last_verify_source: Option<String>,
    pub last_exit_country: Option<String>,
    pub last_exit_region: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RunnerTask {
    pub task_id: String,
    pub attempt: i64,
    pub kind: String,
    pub payload: Value,
    pub timeout_seconds: Option<i64>,
    pub execution_intent: Option<RunnerExecutionIntent>,
    pub fingerprint_profile: Option<RunnerFingerprintProfile>,
    pub behavior_profile: Option<RunnerBehaviorProfile>,
    pub behavior_plan: Option<RunnerBehaviorPlan>,
    pub form_action_plan: Option<RunnerFormActionPlan>,
    pub proxy: Option<RunnerProxySelection>,
    pub session_cookies: Option<Vec<Value>>,
    pub session_local_storage: Option<Value>,
    pub session_session_storage: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct RunnerSummaryArtifact {
    pub category: SummaryArtifactCategory,
    pub key: String,
    pub source: String,
    pub severity: SummaryArtifactSeverity,
    pub title: String,
    pub summary: String,
}

#[derive(Debug, Clone, Copy)]
pub enum SummaryArtifactSeverity {
    Info,
    Warning,
    Error,
}

impl SummaryArtifactSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            SummaryArtifactSeverity::Info => "info",
            SummaryArtifactSeverity::Warning => "warning",
            SummaryArtifactSeverity::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SummaryArtifactCategory {
    Execution,
    Result,
    Debug,
    Transient,
    Summary,
}

pub struct RunnerExecutionResult {
    pub status: RunnerOutcomeStatus,
    pub result_json: Option<Value>,
    pub error_message: Option<String>,
    pub summary_artifacts: Vec<RunnerSummaryArtifact>,
    pub session_cookies: Option<Vec<Value>>,
    pub session_local_storage: Option<Value>,
    pub session_session_storage: Option<Value>,
}

impl RunnerExecutionResult {
    pub fn success(result_json: Option<Value>) -> Self {
        Self {
            status: RunnerOutcomeStatus::Succeeded,
            result_json,
            error_message: None,
            summary_artifacts: vec![RunnerSummaryArtifact {
                category: SummaryArtifactCategory::Execution,
                key: "runner.execution".to_string(),
                source: "runner.core".to_string(),
                severity: SummaryArtifactSeverity::Info,
                title: "runner execution summary".to_string(),
                summary: "runner finished successfully".to_string(),
            }],
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        }
    }

    pub fn failure(message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            status: RunnerOutcomeStatus::Failed,
            result_json: None,
            error_message: Some(msg.clone()),
            summary_artifacts: vec![RunnerSummaryArtifact {
                category: SummaryArtifactCategory::Execution,
                key: "runner.execution".to_string(),
                source: "runner.core".to_string(),
                severity: SummaryArtifactSeverity::Error,
                title: "runner failure summary".to_string(),
                summary: msg,
            }],
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        }
    }
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
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, Copy)]
pub struct RunnerCapabilities {
    pub supports_timeout: bool,
    pub supports_cancel_running: bool,
    pub supports_artifacts: bool,
}
