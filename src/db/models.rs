use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub input_json: String,
    pub network_policy_json: Option<String>,
    pub behavior_policy_json: Option<String>,
    pub execution_intent_json: Option<String>,
    pub fingerprint_profile_json: Option<String>,
    pub priority: i32,
    pub created_at: String,
    pub queued_at: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub runner_id: Option<String>,
    pub heartbeat_at: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub fingerprint_profile_version: Option<i64>,
    pub identity_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub behavior_profile_version: Option<i64>,
    pub network_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
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
    pub result_json: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintProfileRecord {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub tags_json: Option<String>,
    pub profile_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorProfileRecord {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub tags_json: Option<String>,
    pub profile_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityProfileRecord {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_profile_id: Option<String>,
    pub identity_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkProfileRecord {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub network_policy_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionProfileRecord {
    pub id: String,
    pub name: String,
    pub version: i64,
    pub status: String,
    pub continuity_mode: String,
    pub retention_policy_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteBehaviorPolicyRecord {
    pub id: String,
    pub version: i64,
    pub site_key: String,
    pub page_archetype: Option<String>,
    pub action_kind: Option<String>,
    pub behavior_profile_id: String,
    pub priority: i64,
    pub required: bool,
    pub override_json: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DashboardOnboardingDraftRecord {
    pub id: String,
    pub share_token: String,
    pub share_expires_at: String,
    pub status: String,
    pub login_url: String,
    pub site_key: String,
    pub success_hint: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub identity_profile_id: Option<String>,
    pub session_profile_id: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub proxy_id: Option<String>,
    pub credential_mode: String,
    pub credential_ref: Option<String>,
    pub inferred_contract_json: Option<String>,
    pub final_contract_json: Option<String>,
    pub site_policy_id: Option<String>,
    pub site_policy_version: Option<i64>,
    pub shadow_task_id: Option<String>,
    pub active_success_task_id: Option<String>,
    pub active_failure_task_id: Option<String>,
    pub continuity_task_id: Option<String>,
    pub evidence_summary_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
