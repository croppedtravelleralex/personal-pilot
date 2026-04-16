use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    db::init::DbPool,
    network_identity::{
        fingerprint_consistency::assess_fingerprint_profile_consistency,
        fingerprint_consumption::build_lightpanda_runtime_projection,
        first_family::{
            detect_fingerprint_schema_kind, first_family_declared_control_fields,
            first_family_section_summaries, inferred_family_id, inferred_family_variant,
            runtime_supported_control_fields,
        },
        validator::validate_fingerprint_profile,
    },
    runner::{
        runner_claim_retry_limit_from_env, runner_concurrency_from_env,
        runner_heartbeat_interval_seconds_from_env, runner_idle_backoff_max_ms_from_env,
        runner_idle_backoff_min_ms_from_env, runner_reclaim_seconds_from_env, RunnerKind,
        TaskRunner,
    },
};

const DEFAULT_DATABASE_URL: &str = "sqlite://data/persona_pilot.db";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTaskCounts {
    pub total: i64,
    pub queued: i64,
    pub running: i64,
    pub succeeded: i64,
    pub failed: i64,
    pub timed_out: i64,
    pub cancelled: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWorkerSnapshot {
    pub runner_kind: String,
    pub worker_count: usize,
    pub reclaim_after_seconds: Option<u64>,
    pub heartbeat_interval_seconds: u64,
    pub claim_retry_limit: u32,
    pub idle_backoff_min_ms: u64,
    pub idle_backoff_max_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTaskItem {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub priority: i32,
    pub persona_id: Option<String>,
    pub platform_id: Option<String>,
    pub manual_gate_request_id: Option<String>,
    pub is_browser_task: bool,
    pub title: Option<String>,
    pub final_url: Option<String>,
    pub content_preview: Option<String>,
    pub content_kind: Option<String>,
    pub content_ready: Option<bool>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStatusSnapshot {
    pub service: String,
    pub runtime_mode: String,
    pub queue_len: i64,
    pub counts: DesktopTaskCounts,
    pub worker: DesktopWorkerSnapshot,
    pub latest_tasks: Vec<DesktopTaskItem>,
    pub latest_browser_tasks: Vec<DesktopTaskItem>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTaskQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub status_filter: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTaskPage {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub items: Vec<DesktopTaskItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLogItem {
    pub id: String,
    pub task_id: String,
    pub run_id: Option<String>,
    pub level: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLogQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub task_id_filter: Option<String>,
    pub level_filter: Option<String>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLogPage {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub items: Vec<DesktopLogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsSnapshot {
    pub project_root: String,
    pub database_url: String,
    pub database_path: String,
    pub data_dir: String,
    pub reports_dir: String,
    pub logs_dir: String,
    pub packaged_data_dir: String,
    pub packaged_reports_dir: String,
    pub packaged_logs_dir: String,
    pub runner_kind: String,
    pub worker_count: usize,
    pub reclaim_after_seconds: Option<u64>,
    pub heartbeat_interval_seconds: u64,
    pub claim_retry_limit: u32,
    pub idle_backoff_min_ms: u64,
    pub idle_backoff_max_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRuntimeSettingsDraft {
    pub runner_kind: String,
    pub worker_count: usize,
    pub reclaim_after_seconds: Option<u64>,
    pub heartbeat_interval_seconds: u64,
    pub claim_retry_limit: u32,
    pub idle_backoff_min_ms: u64,
    pub idle_backoff_max_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsMutationResult {
    pub action: String,
    pub snapshot: DesktopSettingsSnapshot,
    pub updated_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLocalApiSnapshot {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub health_url: String,
    pub config_path: String,
    pub bind_mode: String,
    pub start_mode: String,
    pub auth_mode: String,
    pub request_logging_enabled: bool,
    pub require_local_token: bool,
    pub read_only_safe_mode: bool,
    pub max_concurrent_sessions: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLocalApiSettingsDraft {
    pub host: String,
    pub port: u16,
    pub start_mode: String,
    pub auth_mode: String,
    pub request_logging_enabled: bool,
    pub require_local_token: bool,
    pub read_only_safe_mode: bool,
    pub max_concurrent_sessions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLocalApiMutationResult {
    pub action: String,
    pub snapshot: DesktopLocalApiSnapshot,
    pub updated_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBrowserEnvironmentPolicySnapshot {
    pub browser_family: String,
    pub launch_strategy: String,
    pub profile_storage_mode: String,
    pub environment_root: String,
    pub profile_workspace_dir: String,
    pub downloads_dir: String,
    pub extensions_dir: String,
    pub bookmarks_catalog_path: String,
    pub profile_archive_dir: String,
    pub default_viewport_preset: String,
    pub keep_user_data_between_runs: bool,
    pub allow_extensions: bool,
    pub allow_bookmarks_seed: bool,
    pub allow_profile_archive_import: bool,
    pub headless_allowed: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBrowserEnvironmentPolicyDraft {
    pub browser_family: String,
    pub launch_strategy: String,
    pub profile_storage_mode: String,
    pub default_viewport_preset: String,
    pub keep_user_data_between_runs: bool,
    pub allow_extensions: bool,
    pub allow_bookmarks_seed: bool,
    pub allow_profile_archive_import: bool,
    pub headless_allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBrowserEnvironmentPolicyMutationResult {
    pub action: String,
    pub snapshot: DesktopBrowserEnvironmentPolicySnapshot,
    pub updated_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLocalAssetEntry {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: String,
    pub status: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLocalAssetWorkspaceSnapshot {
    pub workspace_root: String,
    pub control_root: String,
    pub browser_environment_root: String,
    pub import_queue_dir: String,
    pub export_queue_dir: String,
    pub local_api_config_path: String,
    pub runtime_policy_path: String,
    pub browser_environment_policy_path: String,
    pub entries: Vec<DesktopLocalAssetEntry>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopImportExportFieldDefinition {
    pub key: String,
    pub label: String,
    pub required: bool,
    pub description: String,
    pub example: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopImportExportSkeleton {
    pub mode: String,
    pub import_manifest_path: String,
    pub export_manifest_path: String,
    pub import_queue_dir: String,
    pub export_queue_dir: String,
    pub supported_import_kinds: Vec<String>,
    pub supported_export_kinds: Vec<String>,
    pub import_fields: Vec<DesktopImportExportFieldDefinition>,
    pub export_fields: Vec<DesktopImportExportFieldDefinition>,
    pub notes: Vec<String>,
    pub updated_at: String,
}

fn now_ts_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn sanitize_page(page: Option<i64>) -> i64 {
    page.unwrap_or(1).max(1)
}

fn sanitize_page_size(page_size: Option<i64>, default_value: i64, max_value: i64) -> i64 {
    page_size.unwrap_or(default_value).clamp(1, max_value)
}

fn normalized_optional_filter(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "all")
}

fn build_like_term(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"))
}

fn normalized_optional_text(raw: Option<String>) -> Option<String> {
    raw.map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_ts_i64(raw: Option<&str>) -> Option<i64> {
    raw.and_then(|value| value.trim().parse::<i64>().ok())
}

fn sticky_ttl_seconds_from_expires_at(expires_at: Option<&str>, now_ts: i64) -> Option<i64> {
    let expires_at_ts = parse_ts_i64(expires_at)?;
    let ttl = expires_at_ts - now_ts;
    (ttl > 0).then_some(ttl)
}

fn build_proxy_residency_status(
    session_key: Option<&str>,
    expires_at: Option<&str>,
    proxy_status: Option<&str>,
    requested_provider: Option<&str>,
    proxy_provider: Option<&str>,
    requested_region: Option<&str>,
    proxy_region: Option<&str>,
    now_ts: i64,
) -> String {
    let normalized_proxy_status = proxy_status.unwrap_or("unknown").to_ascii_lowercase();
    if normalized_proxy_status != "active" {
        return "proxy_inactive".to_string();
    }

    if session_key.is_none() {
        if requested_provider.is_some() || requested_region.is_some() {
            return "provider_rotation_pending".to_string();
        }
        return "stateless_rotation".to_string();
    }

    if let Some(expires_ts) = parse_ts_i64(expires_at) {
        if expires_ts < now_ts {
            return "sticky_expired".to_string();
        }
    }

    let provider_mismatch = match (requested_provider, proxy_provider) {
        (Some(requested), Some(current)) => requested.trim() != current.trim(),
        _ => false,
    };
    if provider_mismatch {
        return "provider_override_pending".to_string();
    }

    let region_mismatch = match (requested_region, proxy_region) {
        (Some(requested), Some(current)) => requested.trim() != current.trim(),
        _ => false,
    };
    if region_mismatch {
        return "region_override_pending".to_string();
    }

    if expires_at.is_some() {
        return "sticky_active".to_string();
    }

    "sticky_unbounded".to_string()
}

fn build_proxy_rotation_mode(
    requested_mode: Option<&str>,
    session_key: Option<&str>,
    residency_status: &str,
    requested_provider: Option<&str>,
    requested_region: Option<&str>,
) -> String {
    if let Some(mode) = requested_mode {
        return mode.to_string();
    }

    if session_key.is_some() {
        if residency_status == "sticky_expired" {
            return "sticky_rebind".to_string();
        }
        return "sticky_refresh".to_string();
    }

    if requested_provider.is_some() || requested_region.is_some() {
        return "provider_aware_rotate".to_string();
    }

    "pool_rotate".to_string()
}

#[derive(Debug, Clone)]
struct DesktopProxyProviderRefreshSpec {
    source_label: String,
    provider_key: String,
    method: Method,
    url: String,
    timeout_seconds: u64,
    headers: BTreeMap<String, String>,
    query: Vec<(String, String)>,
    body: Option<Value>,
    require_http_2xx: bool,
    success_path: Option<String>,
    success_equals: Option<Value>,
    success_in: Option<Vec<Value>>,
    success_contains: Option<String>,
    request_id_path: Option<String>,
}

#[derive(Debug, Clone)]
struct DesktopProxyProviderRefreshSuccess {
    source_label: String,
    provider_key: String,
    status_code: u16,
    provider_request_id: Option<String>,
    response_excerpt: Option<String>,
}

#[derive(Debug, Clone)]
struct DesktopProxyProviderRefreshFailure {
    error_kind: String,
    message: String,
    rollback_signal: String,
    source_label: Option<String>,
    status_code: Option<u16>,
    response_excerpt: Option<String>,
}

fn normalize_lookup_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn json_scalar_to_text(value: &Value) -> Option<String> {
    match value {
        Value::String(raw) => Some(raw.clone()),
        Value::Number(raw) => Some(raw.to_string()),
        Value::Bool(raw) => Some(if *raw { "true" } else { "false" }.to_string()),
        Value::Null => None,
        _ => Some(value.to_string()),
    }
}

fn apply_template_tokens(raw: &str, variables: &BTreeMap<String, String>) -> String {
    let mut rendered = raw.to_string();
    for (key, value) in variables {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
        rendered = rendered.replace(&format!("{{{{ {key} }}}}"), value);
    }
    rendered
}

fn apply_template_tokens_to_json(value: &Value, variables: &BTreeMap<String, String>) -> Value {
    match value {
        Value::String(raw) => Value::String(apply_template_tokens(raw, variables)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| apply_template_tokens_to_json(item, variables))
                .collect(),
        ),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, item)| (key.clone(), apply_template_tokens_to_json(item, variables)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn extract_provider_refresh_object<'a>(
    config_json: &'a Value,
) -> Option<&'a serde_json::Map<String, Value>> {
    config_json
        .get("provider_refresh")
        .or_else(|| config_json.get("change_ip"))
        .or_else(|| config_json.get("ip_refresh"))
        .or_else(|| config_json.get("refresh"))
        .and_then(Value::as_object)
}

fn value_matches_provider_key(raw: Option<&Value>, provider_key: &str) -> bool {
    let Some(raw) = raw else {
        return false;
    };
    let Some(candidate) = raw.as_str() else {
        return false;
    };
    normalize_lookup_key(candidate) == provider_key
}

fn array_matches_provider_key(raw: Option<&Value>, provider_key: &str) -> bool {
    raw.and_then(Value::as_array).is_some_and(|items| {
        items.iter().any(|item| {
            item.as_str()
                .is_some_and(|value| normalize_lookup_key(value) == provider_key)
        })
    })
}

fn source_matches_provider(provider_key: &str, source_label: &str, config_json: &Value) -> bool {
    if normalize_lookup_key(source_label) == provider_key {
        return true;
    }
    let Some(config_obj) = config_json.as_object() else {
        return false;
    };
    if value_matches_provider_key(config_obj.get("provider"), provider_key)
        || array_matches_provider_key(config_obj.get("providers"), provider_key)
        || array_matches_provider_key(config_obj.get("provider_aliases"), provider_key)
    {
        return true;
    }
    let Some(refresh_obj) = extract_provider_refresh_object(config_json) else {
        return false;
    };
    value_matches_provider_key(refresh_obj.get("provider"), provider_key)
        || array_matches_provider_key(refresh_obj.get("providers"), provider_key)
        || array_matches_provider_key(refresh_obj.get("provider_aliases"), provider_key)
}

fn truncate_response_excerpt(raw: &str, max_chars: usize) -> Option<String> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        return None;
    }
    if normalized.chars().count() <= max_chars {
        return Some(normalized.to_string());
    }
    let mut shortened = normalized.chars().take(max_chars).collect::<String>();
    shortened.push_str("...");
    Some(shortened)
}

fn json_path_lookup<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for segment in path
        .split('.')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
    {
        if let Ok(index) = segment.parse::<usize>() {
            current = current.get(index)?;
        } else {
            current = current.get(segment)?;
        }
    }
    Some(current)
}

fn is_truthy_json_value(value: &Value) -> bool {
    match value {
        Value::Bool(raw) => *raw,
        Value::Null => false,
        Value::Number(raw) => {
            if let Some(i64_value) = raw.as_i64() {
                return i64_value != 0;
            }
            if let Some(u64_value) = raw.as_u64() {
                return u64_value != 0;
            }
            raw.as_f64().unwrap_or(0.0) != 0.0
        }
        Value::String(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            !(normalized.is_empty() || normalized == "0" || normalized == "false")
        }
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
    }
}

fn parse_refresh_timeout_seconds(refresh_obj: &serde_json::Map<String, Value>) -> u64 {
    let timeout = refresh_obj
        .get("timeout_seconds")
        .or_else(|| refresh_obj.get("timeoutSeconds"))
        .and_then(Value::as_u64)
        .unwrap_or(15);
    timeout.clamp(1, 120)
}

fn parse_refresh_method(
    refresh_obj: &serde_json::Map<String, Value>,
) -> std::result::Result<Method, DesktopProxyProviderRefreshFailure> {
    let raw = refresh_obj
        .get("method")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("POST");
    Method::from_bytes(raw.as_bytes()).map_err(|_| DesktopProxyProviderRefreshFailure {
        error_kind: "provider_refresh_config_invalid".to_string(),
        message: format!("provider refresh method is invalid: {raw}"),
        rollback_signal: "binding_not_applied".to_string(),
        source_label: None,
        status_code: None,
        response_excerpt: None,
    })
}

async fn resolve_provider_refresh_spec(
    db: &DbPool,
    provider_key: &str,
    proxy_source_label: Option<&str>,
    variables: &BTreeMap<String, String>,
) -> std::result::Result<DesktopProxyProviderRefreshSpec, DesktopProxyProviderRefreshFailure> {
    let rows = sqlx::query(
        r#"SELECT source_label, config_json
           FROM proxy_harvest_sources
           WHERE enabled = 1
           ORDER BY source_label ASC"#,
    )
    .fetch_all(db)
    .await
    .map_err(|err| DesktopProxyProviderRefreshFailure {
        error_kind: "provider_refresh_config_query_failed".to_string(),
        message: format!("failed to load provider refresh config: {err}"),
        rollback_signal: "binding_not_applied".to_string(),
        source_label: None,
        status_code: None,
        response_excerpt: None,
    })?;

    let preferred_source = proxy_source_label.map(normalize_lookup_key);
    let mut preferred_match: Option<(String, Value)> = None;
    let mut fallback_match: Option<(String, Value)> = None;

    for row in rows {
        let source_label: String = row.get("source_label");
        let raw_config: String = row.get("config_json");
        let parsed_config = match serde_json::from_str::<Value>(&raw_config) {
            Ok(config) => config,
            Err(_) => continue,
        };
        if !source_matches_provider(provider_key, &source_label, &parsed_config) {
            continue;
        }
        let normalized_source = normalize_lookup_key(&source_label);
        if preferred_source.as_ref() == Some(&normalized_source) {
            preferred_match = Some((source_label, parsed_config));
            break;
        }
        if fallback_match.is_none() {
            fallback_match = Some((source_label, parsed_config));
        }
    }

    let (source_label, config_json) = preferred_match.or(fallback_match).ok_or_else(|| {
        DesktopProxyProviderRefreshFailure {
            error_kind: "provider_refresh_config_missing".to_string(),
            message: format!(
                "provider refresh config is missing for provider '{provider_key}' in proxy_harvest_sources.config_json"
            ),
            rollback_signal: "binding_not_applied".to_string(),
            source_label: None,
            status_code: None,
            response_excerpt: None,
        }
    })?;

    let refresh_obj = extract_provider_refresh_object(&config_json).ok_or_else(|| {
        DesktopProxyProviderRefreshFailure {
            error_kind: "provider_refresh_endpoint_missing".to_string(),
            message: format!(
                "source '{}' does not declare provider_refresh/change_ip config in config_json",
                source_label
            ),
            rollback_signal: "binding_not_applied".to_string(),
            source_label: Some(source_label.clone()),
            status_code: None,
            response_excerpt: None,
        }
    })?;

    if matches!(
        refresh_obj.get("enabled").and_then(Value::as_bool),
        Some(false)
    ) {
        return Err(DesktopProxyProviderRefreshFailure {
            error_kind: "provider_refresh_disabled".to_string(),
            message: format!("provider refresh is disabled for source '{}'", source_label),
            rollback_signal: "binding_not_applied".to_string(),
            source_label: Some(source_label),
            status_code: None,
            response_excerpt: None,
        });
    }

    let method = parse_refresh_method(refresh_obj)?;
    let url = refresh_obj
        .get("url")
        .and_then(Value::as_str)
        .map(|raw| apply_template_tokens(raw, variables))
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
        .ok_or_else(|| DesktopProxyProviderRefreshFailure {
            error_kind: "provider_refresh_endpoint_missing".to_string(),
            message: format!(
                "provider refresh url is missing for source '{}'",
                source_label
            ),
            rollback_signal: "binding_not_applied".to_string(),
            source_label: Some(source_label.clone()),
            status_code: None,
            response_excerpt: None,
        })?;

    let timeout_seconds = parse_refresh_timeout_seconds(refresh_obj);

    let mut headers = BTreeMap::new();
    if let Some(header_map) = refresh_obj.get("headers").and_then(Value::as_object) {
        for (key, value) in header_map {
            if key.trim().is_empty() {
                continue;
            }
            let Some(raw_value) = json_scalar_to_text(value) else {
                continue;
            };
            headers.insert(
                key.trim().to_string(),
                apply_template_tokens(&raw_value, variables),
            );
        }
    }

    if let Some(auth_env_key) = refresh_obj
        .get("auth_env")
        .or_else(|| refresh_obj.get("authEnv"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let auth_token =
            env::var(auth_env_key).map_err(|_| DesktopProxyProviderRefreshFailure {
                error_kind: "provider_refresh_auth_missing".to_string(),
                message: format!(
                    "provider refresh requires env '{}' but it is not set",
                    auth_env_key
                ),
                rollback_signal: "binding_not_applied".to_string(),
                source_label: Some(source_label.clone()),
                status_code: None,
                response_excerpt: None,
            })?;
        let auth_header = refresh_obj
            .get("auth_header")
            .or_else(|| refresh_obj.get("authHeader"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Authorization");
        let auth_prefix = refresh_obj
            .get("auth_prefix")
            .or_else(|| refresh_obj.get("authPrefix"))
            .and_then(Value::as_str)
            .unwrap_or("Bearer ");
        headers.insert(
            auth_header.to_string(),
            format!("{auth_prefix}{auth_token}"),
        );
    }

    let mut query = Vec::new();
    if let Some(query_obj) = refresh_obj.get("query").and_then(Value::as_object) {
        for (key, value) in query_obj {
            if key.trim().is_empty() {
                continue;
            }
            let Some(raw_value) = json_scalar_to_text(value) else {
                continue;
            };
            query.push((
                key.trim().to_string(),
                apply_template_tokens(&raw_value, variables),
            ));
        }
    }

    let body = refresh_obj
        .get("body")
        .map(|body| apply_template_tokens_to_json(body, variables));

    let success_path = refresh_obj
        .get("success_path")
        .or_else(|| refresh_obj.get("successPath"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let success_equals = refresh_obj
        .get("success_equals")
        .or_else(|| refresh_obj.get("successEquals"))
        .map(|value| apply_template_tokens_to_json(value, variables));
    let success_in = refresh_obj
        .get("success_in")
        .or_else(|| refresh_obj.get("successIn"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| apply_template_tokens_to_json(item, variables))
                .collect::<Vec<_>>()
        });
    let success_contains = refresh_obj
        .get("success_contains")
        .or_else(|| refresh_obj.get("successContains"))
        .and_then(Value::as_str)
        .map(|raw| apply_template_tokens(raw, variables))
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty());
    let request_id_path = refresh_obj
        .get("request_id_path")
        .or_else(|| refresh_obj.get("requestIdPath"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    Ok(DesktopProxyProviderRefreshSpec {
        source_label,
        provider_key: provider_key.to_string(),
        method,
        url,
        timeout_seconds,
        headers,
        query,
        body,
        require_http_2xx: refresh_obj
            .get("require_http_2xx")
            .or_else(|| refresh_obj.get("requireHttp2xx"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        success_path,
        success_equals,
        success_in,
        success_contains,
        request_id_path,
    })
}

async fn execute_provider_refresh(
    spec: &DesktopProxyProviderRefreshSpec,
) -> std::result::Result<DesktopProxyProviderRefreshSuccess, DesktopProxyProviderRefreshFailure> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(spec.timeout_seconds))
        .build()
        .map_err(|err| DesktopProxyProviderRefreshFailure {
            error_kind: "provider_refresh_client_init_failed".to_string(),
            message: format!("failed to initialize provider refresh client: {err}"),
            rollback_signal: "binding_not_applied".to_string(),
            source_label: Some(spec.source_label.clone()),
            status_code: None,
            response_excerpt: None,
        })?;

    let mut request_builder = client.request(spec.method.clone(), &spec.url);
    if !spec.query.is_empty() {
        request_builder = request_builder.query(&spec.query);
    }
    for (key, value) in &spec.headers {
        request_builder = request_builder.header(key, value);
    }
    if let Some(body) = spec.body.as_ref() {
        request_builder = request_builder.json(body);
    }

    let response =
        request_builder
            .send()
            .await
            .map_err(|err| DesktopProxyProviderRefreshFailure {
                error_kind: if err.is_timeout() {
                    "provider_refresh_timeout".to_string()
                } else if err.is_connect() {
                    "provider_refresh_connect_failed".to_string()
                } else {
                    "provider_refresh_request_failed".to_string()
                },
                message: format!("provider refresh request failed: {err}"),
                rollback_signal: "binding_not_applied".to_string(),
                source_label: Some(spec.source_label.clone()),
                status_code: None,
                response_excerpt: None,
            })?;

    let status_code = response.status().as_u16();
    let status_success = response.status().is_success();
    let response_body =
        response
            .text()
            .await
            .map_err(|err| DesktopProxyProviderRefreshFailure {
                error_kind: "provider_refresh_response_read_failed".to_string(),
                message: format!("provider refresh response read failed: {err}"),
                rollback_signal: "binding_not_applied".to_string(),
                source_label: Some(spec.source_label.clone()),
                status_code: Some(status_code),
                response_excerpt: None,
            })?;
    let response_excerpt = truncate_response_excerpt(&response_body, 360);

    if spec.require_http_2xx && !status_success {
        return Err(DesktopProxyProviderRefreshFailure {
            error_kind: "provider_refresh_http_status".to_string(),
            message: format!("provider refresh responded with non-2xx status {status_code}"),
            rollback_signal: "binding_not_applied".to_string(),
            source_label: Some(spec.source_label.clone()),
            status_code: Some(status_code),
            response_excerpt,
        });
    }

    if let Some(expected_fragment) = spec.success_contains.as_deref() {
        if !response_body.contains(expected_fragment) {
            return Err(DesktopProxyProviderRefreshFailure {
                error_kind: "provider_refresh_success_check_failed".to_string(),
                message: format!(
                    "provider refresh response does not contain expected fragment '{}'",
                    expected_fragment
                ),
                rollback_signal: "binding_not_applied".to_string(),
                source_label: Some(spec.source_label.clone()),
                status_code: Some(status_code),
                response_excerpt,
            });
        }
    }

    let parsed_json = serde_json::from_str::<Value>(&response_body).ok();
    if let Some(success_path) = spec.success_path.as_deref() {
        let Some(json) = parsed_json.as_ref() else {
            return Err(DesktopProxyProviderRefreshFailure {
                error_kind: "provider_refresh_response_not_json".to_string(),
                message: "provider refresh success_path is configured but response is not JSON"
                    .to_string(),
                rollback_signal: "binding_not_applied".to_string(),
                source_label: Some(spec.source_label.clone()),
                status_code: Some(status_code),
                response_excerpt,
            });
        };
        let Some(actual_value) = json_path_lookup(json, success_path) else {
            return Err(DesktopProxyProviderRefreshFailure {
                error_kind: "provider_refresh_success_path_missing".to_string(),
                message: format!(
                    "provider refresh response missing success_path '{}'",
                    success_path
                ),
                rollback_signal: "binding_not_applied".to_string(),
                source_label: Some(spec.source_label.clone()),
                status_code: Some(status_code),
                response_excerpt,
            });
        };
        if let Some(expected_value) = spec.success_equals.as_ref() {
            if actual_value != expected_value {
                return Err(DesktopProxyProviderRefreshFailure {
                    error_kind: "provider_refresh_success_check_failed".to_string(),
                    message: format!(
                        "provider refresh success_path '{}' mismatch: expected {}, got {}",
                        success_path, expected_value, actual_value
                    ),
                    rollback_signal: "binding_not_applied".to_string(),
                    source_label: Some(spec.source_label.clone()),
                    status_code: Some(status_code),
                    response_excerpt,
                });
            }
        } else if let Some(allowed_values) = spec.success_in.as_ref() {
            if !allowed_values.iter().any(|value| value == actual_value) {
                return Err(DesktopProxyProviderRefreshFailure {
                    error_kind: "provider_refresh_success_check_failed".to_string(),
                    message: format!(
                        "provider refresh success_path '{}' did not match any allowed value",
                        success_path
                    ),
                    rollback_signal: "binding_not_applied".to_string(),
                    source_label: Some(spec.source_label.clone()),
                    status_code: Some(status_code),
                    response_excerpt,
                });
            }
        } else if !is_truthy_json_value(actual_value) {
            return Err(DesktopProxyProviderRefreshFailure {
                error_kind: "provider_refresh_success_check_failed".to_string(),
                message: format!(
                    "provider refresh success_path '{}' resolved to non-truthy value {}",
                    success_path, actual_value
                ),
                rollback_signal: "binding_not_applied".to_string(),
                source_label: Some(spec.source_label.clone()),
                status_code: Some(status_code),
                response_excerpt,
            });
        }
    }

    let provider_request_id =
        if let (Some(json), Some(path)) = (parsed_json.as_ref(), spec.request_id_path.as_deref()) {
            json_path_lookup(json, path).and_then(json_scalar_to_text)
        } else {
            None
        };

    Ok(DesktopProxyProviderRefreshSuccess {
        source_label: spec.source_label.clone(),
        provider_key: spec.provider_key.clone(),
        status_code,
        provider_request_id,
        response_excerpt,
    })
}

async fn insert_change_proxy_ip_task(
    db: &DbPool,
    task_id: &str,
    status: &str,
    input_json: &Value,
    result_json: &Value,
    error_message: Option<&str>,
    created_at: &str,
    queued_at: Option<&str>,
) -> Result<()> {
    let proxy_id = input_json
        .get("proxy_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let requested_region = input_json
        .get("requested_region")
        .and_then(Value::as_str)
        .map(str::to_string);
    let proxy_mode = input_json
        .get("rotation_mode")
        .or_else(|| input_json.get("mode"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let finished_at = matches!(status, "succeeded" | "failed" | "timed_out" | "cancelled")
        .then_some(created_at);
    sqlx::query(
        r#"INSERT INTO tasks (
               id, kind, status, input_json, proxy_id, requested_region, proxy_mode,
               priority, created_at, queued_at, started_at, finished_at, result_json, error_message
           ) VALUES (?, 'change_proxy_ip', ?, ?, ?, ?, ?, 5, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(task_id)
    .bind(status)
    .bind(input_json.to_string())
    .bind(proxy_id)
    .bind(requested_region)
    .bind(proxy_mode)
    .bind(created_at)
    .bind(queued_at)
    .bind(created_at)
    .bind(finished_at)
    .bind(result_json.to_string())
    .bind(error_message)
    .execute(db)
    .await?;
    Ok(())
}

fn is_browser_task(kind: &str) -> bool {
    matches!(
        kind,
        "open_page" | "get_html" | "get_title" | "get_final_url" | "extract_text"
    )
}

fn parse_result_value(raw: Option<&str>) -> Option<Value> {
    raw.and_then(|value| serde_json::from_str::<Value>(value).ok())
}

fn find_json_value<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn pick_string(root: &Value, candidates: &[&[&str]]) -> Option<String> {
    candidates.iter().find_map(|candidate| {
        find_json_value(root, candidate).and_then(|value| value.as_str().map(ToOwned::to_owned))
    })
}

fn pick_bool(root: &Value, candidates: &[&[&str]]) -> Option<bool> {
    candidates
        .iter()
        .find_map(|candidate| find_json_value(root, candidate).and_then(Value::as_bool))
}

fn trim_preview(value: Option<String>, max_chars: usize) -> Option<String> {
    value.map(|preview| {
        let trimmed = preview.trim();
        if trimmed.chars().count() <= max_chars {
            trimmed.to_string()
        } else {
            let shortened = trimmed.chars().take(max_chars).collect::<String>();
            format!("{shortened}...")
        }
    })
}

fn map_task_row(row: &sqlx::sqlite::SqliteRow) -> DesktopTaskItem {
    let result_json: Option<String> = row.get("result_json");
    let parsed = parse_result_value(result_json.as_deref());
    let title = parsed
        .as_ref()
        .and_then(|value| pick_string(value, &[&["title"], &["browser", "title"]]));
    let final_url = parsed.as_ref().and_then(|value| {
        pick_string(
            value,
            &[&["final_url"], &["finalUrl"], &["browser", "final_url"]],
        )
    });
    let content_preview = trim_preview(
        parsed.as_ref().and_then(|value| {
            pick_string(
                value,
                &[
                    &["content_preview"],
                    &["contentPreview"],
                    &["content", "preview"],
                ],
            )
        }),
        180,
    );
    let content_kind = parsed.as_ref().and_then(|value| {
        pick_string(
            value,
            &[&["content_kind"], &["contentKind"], &["content", "kind"]],
        )
    });
    let content_ready = parsed.as_ref().and_then(|value| {
        pick_bool(
            value,
            &[&["content_ready"], &["contentReady"], &["content", "ready"]],
        )
    });
    let kind: String = row.get("kind");

    DesktopTaskItem {
        id: row.get("id"),
        kind: kind.clone(),
        status: row.get("status"),
        priority: row.get("priority"),
        persona_id: row.get("persona_id"),
        platform_id: row.get("platform_id"),
        manual_gate_request_id: row.get("manual_gate_request_id"),
        is_browser_task: is_browser_task(&kind),
        title,
        final_url,
        content_preview,
        content_kind,
        content_ready,
        error_message: row.get("error_message"),
        created_at: row.get("created_at"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
    }
}

async fn load_task_counts(db: &DbPool) -> Result<DesktopTaskCounts> {
    let row = sqlx::query(
        r#"SELECT
               COUNT(*) AS total,
               SUM(CASE WHEN status = 'queued' THEN 1 ELSE 0 END) AS queued,
               SUM(CASE WHEN status = 'running' THEN 1 ELSE 0 END) AS running,
               SUM(CASE WHEN status = 'succeeded' THEN 1 ELSE 0 END) AS succeeded,
               SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed,
               SUM(CASE WHEN status = 'timed_out' THEN 1 ELSE 0 END) AS timed_out,
               SUM(CASE WHEN status = 'cancelled' THEN 1 ELSE 0 END) AS cancelled
           FROM tasks"#,
    )
    .fetch_one(db)
    .await?;

    Ok(DesktopTaskCounts {
        total: row.get::<i64, _>("total"),
        queued: row.get::<i64, _>("queued"),
        running: row.get::<i64, _>("running"),
        succeeded: row.get::<i64, _>("succeeded"),
        failed: row.get::<i64, _>("failed"),
        timed_out: row.get::<i64, _>("timed_out"),
        cancelled: row.get::<i64, _>("cancelled"),
    })
}

async fn load_latest_tasks(
    db: &DbPool,
    limit: i64,
    browser_only: bool,
) -> Result<Vec<DesktopTaskItem>> {
    let sql = if browser_only {
        r#"SELECT
               id, kind, status, priority, persona_id, platform_id, manual_gate_request_id,
               result_json, error_message, created_at, started_at, finished_at
           FROM tasks
           WHERE kind IN ('open_page', 'get_html', 'get_title', 'get_final_url', 'extract_text')
           ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
           LIMIT ?"#
    } else {
        r#"SELECT
               id, kind, status, priority, persona_id, platform_id, manual_gate_request_id,
               result_json, error_message, created_at, started_at, finished_at
           FROM tasks
           ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
           LIMIT ?"#
    };

    let rows = sqlx::query(sql).bind(limit).fetch_all(db).await?;
    Ok(rows.iter().map(map_task_row).collect())
}

pub async fn load_desktop_status(
    db: &DbPool,
    database_url: Option<&str>,
) -> Result<DesktopStatusSnapshot> {
    let counts = load_task_counts(db).await?;
    let latest_tasks = load_latest_tasks(db, 6, false).await?;
    let latest_browser_tasks = load_latest_tasks(db, 6, true).await?;
    let database_url = database_url.unwrap_or(DEFAULT_DATABASE_URL);
    let runtime_settings = effective_runtime_settings(database_url);

    Ok(DesktopStatusSnapshot {
        service: "PersonaPilot Desktop".to_string(),
        runtime_mode: "desktop_local".to_string(),
        queue_len: counts.queued,
        counts,
        worker: DesktopWorkerSnapshot {
            runner_kind: runtime_settings.runner_kind,
            worker_count: runtime_settings.worker_count,
            reclaim_after_seconds: runtime_settings.reclaim_after_seconds,
            heartbeat_interval_seconds: runtime_settings.heartbeat_interval_seconds,
            claim_retry_limit: runtime_settings.claim_retry_limit,
            idle_backoff_min_ms: runtime_settings.idle_backoff_min_ms,
            idle_backoff_max_ms: runtime_settings.idle_backoff_max_ms,
        },
        latest_tasks,
        latest_browser_tasks,
        updated_at: now_ts_string(),
    })
}

pub async fn load_desktop_tasks(db: &DbPool, query: DesktopTaskQuery) -> Result<DesktopTaskPage> {
    let page = sanitize_page(query.page);
    let page_size = sanitize_page_size(query.page_size, 50, 200);
    let offset = (page - 1) * page_size;
    let status_filter = normalized_optional_filter(query.status_filter);
    let like_term = build_like_term(query.search);

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM tasks
           WHERE (? IS NULL OR status = ?)
             AND (
               ? IS NULL
               OR id LIKE ?
               OR kind LIKE ?
               OR COALESCE(persona_id, '') LIKE ?
               OR COALESCE(platform_id, '') LIKE ?
             )"#,
    )
    .bind(status_filter.as_deref())
    .bind(status_filter.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .fetch_one(db)
    .await?;

    let rows = sqlx::query(
        r#"SELECT
               id, kind, status, priority, persona_id, platform_id, manual_gate_request_id,
               result_json, error_message, created_at, started_at, finished_at
           FROM tasks
           WHERE (? IS NULL OR status = ?)
             AND (
               ? IS NULL
               OR id LIKE ?
               OR kind LIKE ?
               OR COALESCE(persona_id, '') LIKE ?
               OR COALESCE(platform_id, '') LIKE ?
             )
           ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(status_filter.as_deref())
    .bind(status_filter.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .bind(page_size)
    .bind(offset)
    .fetch_all(db)
    .await?;

    Ok(DesktopTaskPage {
        page,
        page_size,
        total,
        items: rows.iter().map(map_task_row).collect(),
    })
}

pub async fn load_desktop_logs(db: &DbPool, query: DesktopLogQuery) -> Result<DesktopLogPage> {
    let page = sanitize_page(query.page);
    let page_size = sanitize_page_size(query.page_size, 100, 200);
    let offset = (page - 1) * page_size;
    let task_id_filter = normalized_optional_filter(query.task_id_filter);
    let level_filter = normalized_optional_filter(query.level_filter);
    let like_term = build_like_term(query.search);

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM logs
           WHERE (? IS NULL OR task_id = ?)
             AND (? IS NULL OR LOWER(level) = LOWER(?))
             AND (? IS NULL OR message LIKE ?)"#,
    )
    .bind(task_id_filter.as_deref())
    .bind(task_id_filter.as_deref())
    .bind(level_filter.as_deref())
    .bind(level_filter.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .fetch_one(db)
    .await?;

    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        r#"SELECT id, task_id, run_id, level, message, created_at
           FROM logs
           WHERE (? IS NULL OR task_id = ?)
             AND (? IS NULL OR LOWER(level) = LOWER(?))
             AND (? IS NULL OR message LIKE ?)
           ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(task_id_filter.as_deref())
    .bind(task_id_filter.as_deref())
    .bind(level_filter.as_deref())
    .bind(level_filter.as_deref())
    .bind(like_term.as_deref())
    .bind(like_term.as_deref())
    .bind(page_size)
    .bind(offset)
    .fetch_all(db)
    .await?;

    Ok(DesktopLogPage {
        page,
        page_size,
        total,
        items: rows
            .into_iter()
            .map(
                |(id, task_id, run_id, level, message, created_at)| DesktopLogItem {
                    id,
                    task_id,
                    run_id,
                    level,
                    message,
                    created_at,
                },
            )
            .collect(),
    })
}

fn sqlite_path_from_url(database_url: &str) -> PathBuf {
    database_url
        .strip_prefix("sqlite://")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(database_url))
}

fn join_persona_pilot_dir(base: Option<String>, leaf: &str) -> String {
    base.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("C:\\Users\\Public\\AppData\\Local"))
        .join("PersonaPilot")
        .join(leaf)
        .to_string_lossy()
        .to_string()
}

pub fn default_database_url() -> String {
    env::var("PERSONA_PILOT_DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

pub fn default_desktop_runtime_settings() -> DesktopRuntimeSettingsDraft {
    DesktopRuntimeSettingsDraft {
        runner_kind: "fake".to_string(),
        worker_count: 1,
        reclaim_after_seconds: None,
        heartbeat_interval_seconds: 5,
        claim_retry_limit: 8,
        idle_backoff_min_ms: 250,
        idle_backoff_max_ms: 3000,
    }
}

fn env_runtime_settings() -> DesktopRuntimeSettingsDraft {
    DesktopRuntimeSettingsDraft {
        runner_kind: format!("{:?}", RunnerKind::from_env()).to_ascii_lowercase(),
        worker_count: runner_concurrency_from_env(),
        reclaim_after_seconds: runner_reclaim_seconds_from_env(),
        heartbeat_interval_seconds: runner_heartbeat_interval_seconds_from_env(),
        claim_retry_limit: runner_claim_retry_limit_from_env(),
        idle_backoff_min_ms: runner_idle_backoff_min_ms_from_env(),
        idle_backoff_max_ms: runner_idle_backoff_max_ms_from_env(),
    }
}

fn runtime_settings_path(database_url: &str) -> PathBuf {
    sqlite_path_from_url(database_url)
        .parent()
        .unwrap_or_else(|| Path::new("data"))
        .join("desktop_runtime_settings.json")
}

fn load_persisted_runtime_settings(database_url: &str) -> Option<DesktopRuntimeSettingsDraft> {
    let path = runtime_settings_path(database_url);
    let raw = fs::read_to_string(path).ok()?;
    let parsed = serde_json::from_str::<DesktopRuntimeSettingsDraft>(&raw).ok()?;
    validate_runtime_settings_draft(parsed).ok()
}

fn effective_runtime_settings(database_url: &str) -> DesktopRuntimeSettingsDraft {
    load_persisted_runtime_settings(database_url).unwrap_or_else(env_runtime_settings)
}

fn validate_runtime_settings_draft(
    draft: DesktopRuntimeSettingsDraft,
) -> Result<DesktopRuntimeSettingsDraft> {
    let runner_kind = draft.runner_kind.trim().to_ascii_lowercase();
    if !matches!(runner_kind.as_str(), "fake" | "lightpanda") {
        return Err(anyhow::anyhow!(
            "unsupported runner kind: {}",
            draft.runner_kind
        ));
    }
    if draft.worker_count == 0 {
        return Err(anyhow::anyhow!("worker_count must be greater than 0"));
    }
    if draft.heartbeat_interval_seconds == 0 {
        return Err(anyhow::anyhow!(
            "heartbeat_interval_seconds must be greater than 0"
        ));
    }
    if draft.claim_retry_limit == 0 {
        return Err(anyhow::anyhow!("claim_retry_limit must be greater than 0"));
    }
    if draft.idle_backoff_max_ms < draft.idle_backoff_min_ms {
        return Err(anyhow::anyhow!(
            "idle_backoff_max_ms must be greater than or equal to idle_backoff_min_ms"
        ));
    }

    Ok(DesktopRuntimeSettingsDraft {
        runner_kind,
        worker_count: draft.worker_count,
        reclaim_after_seconds: draft.reclaim_after_seconds.filter(|value| *value > 0),
        heartbeat_interval_seconds: draft.heartbeat_interval_seconds,
        claim_retry_limit: draft.claim_retry_limit,
        idle_backoff_min_ms: draft.idle_backoff_min_ms,
        idle_backoff_max_ms: draft.idle_backoff_max_ms,
    })
}

fn persist_runtime_settings(
    database_url: &str,
    draft: &DesktopRuntimeSettingsDraft,
) -> Result<PathBuf> {
    let path = runtime_settings_path(database_url);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(draft)?;
    fs::write(&path, payload)?;
    Ok(path)
}

pub fn apply_desktop_runtime_settings(
    database_url: &str,
    draft: DesktopRuntimeSettingsDraft,
) -> Result<DesktopSettingsMutationResult> {
    let draft = validate_runtime_settings_draft(draft)?;
    let path = persist_runtime_settings(database_url, &draft)?;
    let updated_at = now_ts_string();

    Ok(DesktopSettingsMutationResult {
        action: "applied".to_string(),
        snapshot: read_desktop_settings(Some(database_url)),
        updated_at,
        message: format!("Runtime settings were applied to {}.", path.display()),
    })
}

pub fn restore_desktop_runtime_settings_defaults(
    database_url: &str,
) -> Result<DesktopSettingsMutationResult> {
    let defaults = default_desktop_runtime_settings();
    let path = persist_runtime_settings(database_url, &defaults)?;
    let updated_at = now_ts_string();

    Ok(DesktopSettingsMutationResult {
        action: "restored".to_string(),
        snapshot: read_desktop_settings(Some(database_url)),
        updated_at,
        message: format!(
            "Runtime settings were restored to desktop defaults in {}.",
            path.display()
        ),
    })
}

pub fn read_desktop_settings(database_url: Option<&str>) -> DesktopSettingsSnapshot {
    let database_url = database_url
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_database_url);
    let runtime_settings = effective_runtime_settings(&database_url);
    let project_root = env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .to_string_lossy()
        .to_string();
    let database_path = sqlite_path_from_url(&database_url);
    let data_dir = database_path
        .parent()
        .unwrap_or_else(|| Path::new("data"))
        .to_path_buf();
    let local_app_data = env::var("LOCALAPPDATA").ok();

    DesktopSettingsSnapshot {
        project_root,
        database_url: database_url.clone(),
        database_path: database_path.to_string_lossy().to_string(),
        data_dir: data_dir.to_string_lossy().to_string(),
        reports_dir: data_dir.join("reports").to_string_lossy().to_string(),
        logs_dir: data_dir.join("logs").to_string_lossy().to_string(),
        packaged_data_dir: join_persona_pilot_dir(local_app_data.clone(), "data"),
        packaged_reports_dir: join_persona_pilot_dir(local_app_data.clone(), "reports"),
        packaged_logs_dir: join_persona_pilot_dir(local_app_data, "logs"),
        runner_kind: runtime_settings.runner_kind,
        worker_count: runtime_settings.worker_count,
        reclaim_after_seconds: runtime_settings.reclaim_after_seconds,
        heartbeat_interval_seconds: runtime_settings.heartbeat_interval_seconds,
        claim_retry_limit: runtime_settings.claim_retry_limit,
        idle_backoff_min_ms: runtime_settings.idle_backoff_min_ms,
        idle_backoff_max_ms: runtime_settings.idle_backoff_max_ms,
    }
}

fn data_root_from_database_url(database_url: &str) -> PathBuf {
    sqlite_path_from_url(database_url)
        .parent()
        .unwrap_or_else(|| Path::new("data"))
        .to_path_buf()
}

fn control_root_from_database_url(database_url: &str) -> PathBuf {
    data_root_from_database_url(database_url).join("control")
}

fn asset_workspace_root_from_database_url(database_url: &str) -> PathBuf {
    data_root_from_database_url(database_url).join("assets")
}

fn browser_environment_root_from_database_url(database_url: &str) -> PathBuf {
    data_root_from_database_url(database_url).join("browser-environments")
}

fn path_timestamp_or_now(path: &Path) -> String {
    fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|timestamp| timestamp.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(now_ts_string)
}

fn local_api_settings_path(database_url: &str) -> PathBuf {
    control_root_from_database_url(database_url).join("desktop_local_api_settings.json")
}

pub fn default_desktop_local_api_settings() -> DesktopLocalApiSettingsDraft {
    DesktopLocalApiSettingsDraft {
        host: "127.0.0.1".to_string(),
        port: 3000,
        start_mode: "manual".to_string(),
        auth_mode: "desktop_session".to_string(),
        request_logging_enabled: true,
        require_local_token: false,
        read_only_safe_mode: false,
        max_concurrent_sessions: 4,
    }
}

fn validate_local_api_settings_draft(
    draft: DesktopLocalApiSettingsDraft,
) -> Result<DesktopLocalApiSettingsDraft> {
    let host = draft.host.trim().to_ascii_lowercase();
    if !matches!(host.as_str(), "127.0.0.1" | "localhost") {
        return Err(anyhow::anyhow!(
            "host must stay on loopback. Supported values: 127.0.0.1 or localhost"
        ));
    }

    if draft.port == 0 {
        return Err(anyhow::anyhow!("port must be greater than 0"));
    }

    if draft.max_concurrent_sessions == 0 || draft.max_concurrent_sessions > 32 {
        return Err(anyhow::anyhow!(
            "max_concurrent_sessions must be between 1 and 32"
        ));
    }

    let start_mode = draft.start_mode.trim().to_ascii_lowercase();
    if !matches!(start_mode.as_str(), "manual" | "auto_on_shell_open") {
        return Err(anyhow::anyhow!(
            "unsupported start_mode: {}",
            draft.start_mode
        ));
    }

    let auth_mode = draft.auth_mode.trim().to_ascii_lowercase();
    if !matches!(auth_mode.as_str(), "desktop_session" | "loopback_token") {
        return Err(anyhow::anyhow!(
            "unsupported auth_mode: {}",
            draft.auth_mode
        ));
    }

    Ok(DesktopLocalApiSettingsDraft {
        host,
        port: draft.port,
        start_mode,
        auth_mode,
        request_logging_enabled: draft.request_logging_enabled,
        require_local_token: draft.require_local_token,
        read_only_safe_mode: draft.read_only_safe_mode,
        max_concurrent_sessions: draft.max_concurrent_sessions,
    })
}

fn load_persisted_local_api_settings(database_url: &str) -> Option<DesktopLocalApiSettingsDraft> {
    let raw = fs::read_to_string(local_api_settings_path(database_url)).ok()?;
    let parsed = serde_json::from_str::<DesktopLocalApiSettingsDraft>(&raw).ok()?;
    validate_local_api_settings_draft(parsed).ok()
}

fn effective_local_api_settings(database_url: &str) -> DesktopLocalApiSettingsDraft {
    load_persisted_local_api_settings(database_url)
        .unwrap_or_else(default_desktop_local_api_settings)
}

fn persist_local_api_settings(
    database_url: &str,
    draft: &DesktopLocalApiSettingsDraft,
) -> Result<PathBuf> {
    let path = local_api_settings_path(database_url);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(draft)?)?;
    Ok(path)
}

pub fn read_desktop_local_api_snapshot(database_url: Option<&str>) -> DesktopLocalApiSnapshot {
    let database_url = database_url
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_database_url);
    let settings = effective_local_api_settings(&database_url);
    let config_path = local_api_settings_path(&database_url);
    let base_url = format!("http://{}:{}", settings.host, settings.port);

    DesktopLocalApiSnapshot {
        host: settings.host.clone(),
        port: settings.port,
        base_url: base_url.clone(),
        health_url: format!("{base_url}/health"),
        config_path: config_path.to_string_lossy().to_string(),
        bind_mode: "loopback_only".to_string(),
        start_mode: settings.start_mode,
        auth_mode: settings.auth_mode,
        request_logging_enabled: settings.request_logging_enabled,
        require_local_token: settings.require_local_token,
        read_only_safe_mode: settings.read_only_safe_mode,
        max_concurrent_sessions: settings.max_concurrent_sessions,
        updated_at: path_timestamp_or_now(&config_path),
    }
}

pub fn apply_desktop_local_api_settings(
    database_url: &str,
    draft: DesktopLocalApiSettingsDraft,
) -> Result<DesktopLocalApiMutationResult> {
    let draft = validate_local_api_settings_draft(draft)?;
    let path = persist_local_api_settings(database_url, &draft)?;
    let updated_at = path_timestamp_or_now(&path);

    Ok(DesktopLocalApiMutationResult {
        action: "applied".to_string(),
        snapshot: read_desktop_local_api_snapshot(Some(database_url)),
        updated_at,
        message: format!("Local API settings were written to {}.", path.display()),
    })
}

pub fn restore_desktop_local_api_defaults(
    database_url: &str,
) -> Result<DesktopLocalApiMutationResult> {
    let defaults = default_desktop_local_api_settings();
    let path = persist_local_api_settings(database_url, &defaults)?;
    let updated_at = path_timestamp_or_now(&path);

    Ok(DesktopLocalApiMutationResult {
        action: "restored".to_string(),
        snapshot: read_desktop_local_api_snapshot(Some(database_url)),
        updated_at,
        message: format!(
            "Local API settings were restored to desktop defaults in {}.",
            path.display()
        ),
    })
}

fn browser_environment_policy_path(database_url: &str) -> PathBuf {
    control_root_from_database_url(database_url).join("desktop_browser_environment_policy.json")
}

pub fn default_desktop_browser_environment_policy() -> DesktopBrowserEnvironmentPolicyDraft {
    DesktopBrowserEnvironmentPolicyDraft {
        browser_family: "chrome".to_string(),
        launch_strategy: "reuse_or_bootstrap".to_string(),
        profile_storage_mode: "per_profile".to_string(),
        default_viewport_preset: "desktop_1600".to_string(),
        keep_user_data_between_runs: true,
        allow_extensions: true,
        allow_bookmarks_seed: true,
        allow_profile_archive_import: true,
        headless_allowed: false,
    }
}

fn validate_browser_environment_policy_draft(
    draft: DesktopBrowserEnvironmentPolicyDraft,
) -> Result<DesktopBrowserEnvironmentPolicyDraft> {
    let browser_family = draft.browser_family.trim().to_ascii_lowercase();
    if !matches!(browser_family.as_str(), "chrome" | "edge" | "lightpanda") {
        return Err(anyhow::anyhow!(
            "unsupported browser_family: {}",
            draft.browser_family
        ));
    }

    let launch_strategy = draft.launch_strategy.trim().to_ascii_lowercase();
    if !matches!(
        launch_strategy.as_str(),
        "reuse_or_bootstrap" | "clean_bootstrap" | "attach_existing"
    ) {
        return Err(anyhow::anyhow!(
            "unsupported launch_strategy: {}",
            draft.launch_strategy
        ));
    }

    let profile_storage_mode = draft.profile_storage_mode.trim().to_ascii_lowercase();
    if !matches!(
        profile_storage_mode.as_str(),
        "per_profile" | "shared_workspace"
    ) {
        return Err(anyhow::anyhow!(
            "unsupported profile_storage_mode: {}",
            draft.profile_storage_mode
        ));
    }

    let default_viewport_preset = draft.default_viewport_preset.trim().to_ascii_lowercase();
    if !matches!(
        default_viewport_preset.as_str(),
        "desktop_1600" | "desktop_1920" | "laptop_1440"
    ) {
        return Err(anyhow::anyhow!(
            "unsupported default_viewport_preset: {}",
            draft.default_viewport_preset
        ));
    }

    Ok(DesktopBrowserEnvironmentPolicyDraft {
        browser_family,
        launch_strategy,
        profile_storage_mode,
        default_viewport_preset,
        keep_user_data_between_runs: draft.keep_user_data_between_runs,
        allow_extensions: draft.allow_extensions,
        allow_bookmarks_seed: draft.allow_bookmarks_seed,
        allow_profile_archive_import: draft.allow_profile_archive_import,
        headless_allowed: draft.headless_allowed,
    })
}

fn load_persisted_browser_environment_policy(
    database_url: &str,
) -> Option<DesktopBrowserEnvironmentPolicyDraft> {
    let raw = fs::read_to_string(browser_environment_policy_path(database_url)).ok()?;
    let parsed = serde_json::from_str::<DesktopBrowserEnvironmentPolicyDraft>(&raw).ok()?;
    validate_browser_environment_policy_draft(parsed).ok()
}

fn effective_browser_environment_policy(
    database_url: &str,
) -> DesktopBrowserEnvironmentPolicyDraft {
    load_persisted_browser_environment_policy(database_url)
        .unwrap_or_else(default_desktop_browser_environment_policy)
}

fn persist_browser_environment_policy(
    database_url: &str,
    draft: &DesktopBrowserEnvironmentPolicyDraft,
) -> Result<PathBuf> {
    let path = browser_environment_policy_path(database_url);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(draft)?)?;
    Ok(path)
}

pub fn read_desktop_browser_environment_policy(
    database_url: Option<&str>,
) -> DesktopBrowserEnvironmentPolicySnapshot {
    let database_url = database_url
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_database_url);
    let policy = effective_browser_environment_policy(&database_url);
    let environment_root = browser_environment_root_from_database_url(&database_url);
    let workspace_root = asset_workspace_root_from_database_url(&database_url);
    let policy_path = browser_environment_policy_path(&database_url);

    DesktopBrowserEnvironmentPolicySnapshot {
        browser_family: policy.browser_family,
        launch_strategy: policy.launch_strategy,
        profile_storage_mode: policy.profile_storage_mode,
        environment_root: environment_root.to_string_lossy().to_string(),
        profile_workspace_dir: environment_root
            .join("profiles")
            .to_string_lossy()
            .to_string(),
        downloads_dir: environment_root
            .join("downloads")
            .to_string_lossy()
            .to_string(),
        extensions_dir: environment_root
            .join("extensions")
            .to_string_lossy()
            .to_string(),
        bookmarks_catalog_path: workspace_root
            .join("bookmarks")
            .join("catalog.json")
            .to_string_lossy()
            .to_string(),
        profile_archive_dir: workspace_root
            .join("profile-archives")
            .to_string_lossy()
            .to_string(),
        default_viewport_preset: policy.default_viewport_preset,
        keep_user_data_between_runs: policy.keep_user_data_between_runs,
        allow_extensions: policy.allow_extensions,
        allow_bookmarks_seed: policy.allow_bookmarks_seed,
        allow_profile_archive_import: policy.allow_profile_archive_import,
        headless_allowed: policy.headless_allowed,
        updated_at: path_timestamp_or_now(&policy_path),
    }
}

pub fn apply_desktop_browser_environment_policy(
    database_url: &str,
    draft: DesktopBrowserEnvironmentPolicyDraft,
) -> Result<DesktopBrowserEnvironmentPolicyMutationResult> {
    let draft = validate_browser_environment_policy_draft(draft)?;
    let path = persist_browser_environment_policy(database_url, &draft)?;
    let updated_at = path_timestamp_or_now(&path);

    Ok(DesktopBrowserEnvironmentPolicyMutationResult {
        action: "applied".to_string(),
        snapshot: read_desktop_browser_environment_policy(Some(database_url)),
        updated_at,
        message: format!(
            "Browser environment policy was written to {}.",
            path.display()
        ),
    })
}

pub fn restore_desktop_browser_environment_policy_defaults(
    database_url: &str,
) -> Result<DesktopBrowserEnvironmentPolicyMutationResult> {
    let defaults = default_desktop_browser_environment_policy();
    let path = persist_browser_environment_policy(database_url, &defaults)?;
    let updated_at = path_timestamp_or_now(&path);

    Ok(DesktopBrowserEnvironmentPolicyMutationResult {
        action: "restored".to_string(),
        snapshot: read_desktop_browser_environment_policy(Some(database_url)),
        updated_at,
        message: format!(
            "Browser environment policy was restored to defaults in {}.",
            path.display()
        ),
    })
}

fn local_asset_entry_status(path: &Path) -> String {
    if path.exists() {
        "ready".to_string()
    } else {
        "provision_on_demand".to_string()
    }
}

fn build_local_asset_entry(
    id: &str,
    label: &str,
    kind: &str,
    path: PathBuf,
    description: &str,
) -> DesktopLocalAssetEntry {
    DesktopLocalAssetEntry {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        status: local_asset_entry_status(&path),
        path: path.to_string_lossy().to_string(),
        description: description.to_string(),
    }
}

pub fn read_desktop_local_asset_workspace(
    database_url: Option<&str>,
) -> DesktopLocalAssetWorkspaceSnapshot {
    let database_url = database_url
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_database_url);
    let workspace_root = asset_workspace_root_from_database_url(&database_url);
    let control_root = control_root_from_database_url(&database_url);
    let browser_environment_root = browser_environment_root_from_database_url(&database_url);
    let import_queue_dir = workspace_root.join("imports");
    let export_queue_dir = workspace_root.join("exports");
    let local_api_config_path = local_api_settings_path(&database_url);
    let runtime_policy_path = runtime_settings_path(&database_url);
    let browser_policy_path = browser_environment_policy_path(&database_url);
    let profile_workspace_dir = browser_environment_root.join("profiles");
    let downloads_dir = browser_environment_root.join("downloads");
    let extensions_dir = browser_environment_root.join("extensions");
    let bookmark_catalog_path = workspace_root.join("bookmarks").join("catalog.json");
    let profile_archive_dir = workspace_root.join("profile-archives");

    let entries = vec![
        build_local_asset_entry(
            "runtimePolicy",
            "Runtime policy",
            "config_file",
            runtime_policy_path.clone(),
            "Runner concurrency, heartbeat, reclaim and idle backoff live here.",
        ),
        build_local_asset_entry(
            "localApiConfig",
            "Local API config",
            "config_file",
            local_api_config_path.clone(),
            "Loopback host, auth mode and start policy stay in this local control file.",
        ),
        build_local_asset_entry(
            "browserEnvironmentPolicy",
            "Browser environment policy",
            "config_file",
            browser_policy_path.clone(),
            "Local browser family, profile storage and asset toggles are persisted here.",
        ),
        build_local_asset_entry(
            "browserProfilesDir",
            "Browser profiles",
            "directory",
            profile_workspace_dir,
            "Per-profile user data workspaces are staged here for local runs.",
        ),
        build_local_asset_entry(
            "browserDownloadsDir",
            "Downloads staging",
            "directory",
            downloads_dir,
            "Local browser downloads and ad-hoc captures land here.",
        ),
        build_local_asset_entry(
            "browserExtensionsDir",
            "Extensions shelf",
            "directory",
            extensions_dir,
            "Local unpacked extensions and helper bundles can be mounted from here.",
        ),
        build_local_asset_entry(
            "bookmarkCatalog",
            "Bookmarks catalog",
            "catalog_file",
            bookmark_catalog_path,
            "Seed bookmarks and reusable link collections can be curated locally here.",
        ),
        build_local_asset_entry(
            "profileArchiveDir",
            "Profile archive vault",
            "directory",
            profile_archive_dir,
            "Import/export profile archives stay on disk without any cloud dependency.",
        ),
        build_local_asset_entry(
            "importQueueDir",
            "Import queue",
            "directory",
            import_queue_dir.clone(),
            "Drop local manifests or bundles here for future staged import flows.",
        ),
        build_local_asset_entry(
            "exportQueueDir",
            "Export queue",
            "directory",
            export_queue_dir.clone(),
            "Prepared local export bundles can be materialized here.",
        ),
    ];

    DesktopLocalAssetWorkspaceSnapshot {
        workspace_root: workspace_root.to_string_lossy().to_string(),
        control_root: control_root.to_string_lossy().to_string(),
        browser_environment_root: browser_environment_root.to_string_lossy().to_string(),
        import_queue_dir: import_queue_dir.to_string_lossy().to_string(),
        export_queue_dir: export_queue_dir.to_string_lossy().to_string(),
        local_api_config_path: local_api_config_path.to_string_lossy().to_string(),
        runtime_policy_path: runtime_policy_path.to_string_lossy().to_string(),
        browser_environment_policy_path: browser_policy_path.to_string_lossy().to_string(),
        entries,
        updated_at: now_ts_string(),
    }
}

pub fn read_desktop_import_export_skeleton(
    database_url: Option<&str>,
) -> DesktopImportExportSkeleton {
    let database_url = database_url
        .map(ToOwned::to_owned)
        .unwrap_or_else(default_database_url);
    let workspace_root = asset_workspace_root_from_database_url(&database_url);
    let import_queue_dir = workspace_root.join("imports");
    let export_queue_dir = workspace_root.join("exports");

    DesktopImportExportSkeleton {
        mode: "local_manifest_queue".to_string(),
        import_manifest_path: import_queue_dir
            .join("import-manifest.template.json")
            .to_string_lossy()
            .to_string(),
        export_manifest_path: export_queue_dir
            .join("export-bundle.template.json")
            .to_string_lossy()
            .to_string(),
        import_queue_dir: import_queue_dir.to_string_lossy().to_string(),
        export_queue_dir: export_queue_dir.to_string_lossy().to_string(),
        supported_import_kinds: vec![
            "profile_archive".to_string(),
            "extensions_bundle".to_string(),
            "bookmark_catalog".to_string(),
            "runtime_policy".to_string(),
        ],
        supported_export_kinds: vec![
            "profile_archive".to_string(),
            "browser_environment".to_string(),
            "bookmark_catalog".to_string(),
            "runtime_policy".to_string(),
        ],
        import_fields: vec![
            DesktopImportExportFieldDefinition {
                key: "assetKind".to_string(),
                label: "Asset kind".to_string(),
                required: true,
                description:
                    "Declare whether the bundle contains profile archives, extensions, bookmarks or runtime policy."
                        .to_string(),
                example: Some("profile_archive".to_string()),
            },
            DesktopImportExportFieldDefinition {
                key: "sourcePath".to_string(),
                label: "Source path".to_string(),
                required: true,
                description: "Absolute local path to the file or folder to import.".to_string(),
                example: Some("D:\\SelfMadeTool\\persona-pilot\\data\\assets\\profile-archives\\demo-profile.zip".to_string()),
            },
            DesktopImportExportFieldDefinition {
                key: "targetSlot".to_string(),
                label: "Target slot".to_string(),
                required: true,
                description: "Describe which local asset slot should receive the imported bundle.".to_string(),
                example: Some("browserProfilesDir".to_string()),
            },
            DesktopImportExportFieldDefinition {
                key: "notes".to_string(),
                label: "Operator notes".to_string(),
                required: false,
                description: "Optional local-only annotation kept with the manifest.".to_string(),
                example: Some("Initial profile bootstrap".to_string()),
            },
        ],
        export_fields: vec![
            DesktopImportExportFieldDefinition {
                key: "assetKind".to_string(),
                label: "Asset kind".to_string(),
                required: true,
                description: "Declare which local asset set should be bundled.".to_string(),
                example: Some("browser_environment".to_string()),
            },
            DesktopImportExportFieldDefinition {
                key: "sourceEntryId".to_string(),
                label: "Source entry".to_string(),
                required: true,
                description: "Reference one of the local asset workspace entry ids.".to_string(),
                example: Some("browserProfilesDir".to_string()),
            },
            DesktopImportExportFieldDefinition {
                key: "bundleName".to_string(),
                label: "Bundle name".to_string(),
                required: true,
                description: "Human-readable local export bundle label.".to_string(),
                example: Some("spring-launch-profiles".to_string()),
            },
            DesktopImportExportFieldDefinition {
                key: "includeMetadata".to_string(),
                label: "Include metadata".to_string(),
                required: false,
                description:
                    "Whether to emit policy snapshots and manifest metadata alongside the bundle."
                        .to_string(),
                example: Some("true".to_string()),
            },
        ],
        notes: vec![
            "Only local file bundles and queue directories are modeled here. No cloud sync or team workspace is involved.".to_string(),
            "The import/export layer is intentionally manifest-first so future compiler or recorder outputs can plug into the same queue.".to_string(),
            "Opening an asset entry will prepare parent directories on demand, but it will not fabricate remote or cloud contracts.".to_string(),
        ],
        updated_at: now_ts_string(),
    }
}

pub fn resolve_desktop_local_asset_entry_path(
    database_url: &str,
    entry_id: &str,
) -> Result<(PathBuf, bool)> {
    let workspace_root = asset_workspace_root_from_database_url(database_url);
    let browser_environment_root = browser_environment_root_from_database_url(database_url);

    match entry_id {
        "runtimePolicy" => Ok((runtime_settings_path(database_url), true)),
        "localApiConfig" => Ok((local_api_settings_path(database_url), true)),
        "browserEnvironmentPolicy" => Ok((browser_environment_policy_path(database_url), true)),
        "browserProfilesDir" => Ok((browser_environment_root.join("profiles"), false)),
        "browserDownloadsDir" => Ok((browser_environment_root.join("downloads"), false)),
        "browserExtensionsDir" => Ok((browser_environment_root.join("extensions"), false)),
        "bookmarkCatalog" => Ok((workspace_root.join("bookmarks").join("catalog.json"), true)),
        "profileArchiveDir" => Ok((workspace_root.join("profile-archives"), false)),
        "importQueueDir" => Ok((workspace_root.join("imports"), false)),
        "exportQueueDir" => Ok((workspace_root.join("exports"), false)),
        _ => Err(anyhow::anyhow!("unsupported local asset entry: {entry_id}")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopEntityReference {
    pub id: String,
    pub name: Option<String>,
    pub status: Option<String>,
    pub version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileRuntimeSummary {
    pub status: String,
    pub current_task_id: Option<String>,
    pub last_task_id: Option<String>,
    pub last_task_status: Option<String>,
    pub last_task_at: Option<String>,
    pub last_opened_at: Option<String>,
    pub last_synced_at: Option<String>,
    pub active_session_count: i64,
    pub pending_action_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileProxySummary {
    pub proxy_id: Option<String>,
    pub provider: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub resolution_status: Option<String>,
    pub usage_mode: Option<String>,
    pub session_key: Option<String>,
    pub requested_provider: Option<String>,
    pub requested_region: Option<String>,
    pub residency_status: Option<String>,
    pub rotation_mode: Option<String>,
    pub sticky_ttl_seconds: Option<i64>,
    pub expires_at: Option<String>,
    pub last_verified_at: Option<String>,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileHealthSummary {
    pub status: String,
    pub continuity_score: f64,
    pub active_session_count: i64,
    pub login_risk_count: i64,
    pub last_event_type: Option<String>,
    pub last_task_at: Option<String>,
    pub snapshot_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileRow {
    pub id: String,
    pub label: String,
    pub store_id: String,
    pub platform_id: String,
    pub device_family: String,
    pub status: String,
    pub country_anchor: String,
    pub region_anchor: Option<String>,
    pub locale: String,
    pub timezone: String,
    pub group_labels: Vec<String>,
    pub tags: Vec<String>,
    pub fingerprint_profile_id: String,
    pub behavior_profile_id: Option<String>,
    pub network_policy_id: String,
    pub continuity_policy_id: String,
    pub credential_ref: Option<String>,
    pub runtime: DesktopProfileRuntimeSummary,
    pub proxy: Option<DesktopProfileProxySummary>,
    pub health: Option<DesktopProfileHealthSummary>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileDetail {
    pub profile: DesktopProfileRow,
    pub fingerprint_profile: DesktopEntityReference,
    pub fingerprint_summary: Option<DesktopProfileFingerprintSummary>,
    pub behavior_profile: Option<DesktopEntityReference>,
    pub network_policy: DesktopEntityReference,
    pub continuity_policy: DesktopEntityReference,
    pub platform_template: Option<DesktopEntityReference>,
    pub store_platform_override: Option<DesktopEntityReference>,
    pub recent_tasks: Vec<DesktopTaskItem>,
    pub recent_logs: Vec<DesktopLogItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopFingerprintSectionSummary {
    pub name: String,
    pub declared_count: usize,
    pub declared_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopFingerprintRuntimeSupportSummary {
    pub supported_fields: Vec<String>,
    pub unsupported_fields: Vec<String>,
    pub supported_count: usize,
    pub unsupported_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopFingerprintConsumptionSummary {
    pub status: String,
    pub version: String,
    pub declared_count: usize,
    pub resolved_count: usize,
    pub applied_count: usize,
    pub ignored_count: usize,
    pub partial_support_warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopFingerprintConsistencySummary {
    pub status: String,
    pub coherence_score: i64,
    pub hard_failure_count: usize,
    pub soft_warning_count: usize,
    pub risk_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileFingerprintSummary {
    pub profile_id: String,
    pub profile_version: i64,
    pub family_id: Option<String>,
    pub family_variant: Option<String>,
    pub schema_kind: String,
    pub declared_control_fields: Vec<String>,
    pub declared_control_count: usize,
    pub declared_sections: Vec<DesktopFingerprintSectionSummary>,
    pub runtime_support: DesktopFingerprintRuntimeSupportSummary,
    pub consistency: DesktopFingerprintConsistencySummary,
    pub consumption: DesktopFingerprintConsumptionSummary,
    pub validation_ok: bool,
    pub validation_issues: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfilePageQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub group_filters: Option<Vec<String>>,
    pub tag_filters: Option<Vec<String>>,
    pub status_filters: Option<Vec<String>>,
    pub platform_filters: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfilePage {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub items: Vec<DesktopProfileRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyHealthSummary {
    pub proxy_id: String,
    pub overall_score: Option<f64>,
    pub grade: Option<String>,
    pub trust_score: Option<i64>,
    pub smoke_status: Option<String>,
    pub verify_status: Option<String>,
    pub geo_match_ok: Option<bool>,
    pub latency_ms: Option<i64>,
    pub checked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyHealth {
    pub proxy_id: String,
    pub overall_score: Option<f64>,
    pub grade: Option<String>,
    pub trust_score: Option<i64>,
    pub smoke_status: Option<String>,
    pub verify_status: Option<String>,
    pub geo_match_ok: Option<bool>,
    pub latency_ms: Option<i64>,
    pub checked_at: Option<String>,
    pub reachable: Option<bool>,
    pub protocol_ok: Option<bool>,
    pub upstream_ok: Option<bool>,
    pub exit_ip: Option<String>,
    pub exit_country: Option<String>,
    pub exit_region: Option<String>,
    pub anonymity_level: Option<String>,
    pub verify_confidence: Option<f64>,
    pub verify_score_delta: Option<i64>,
    pub verify_source: Option<String>,
    pub probe_error: Option<String>,
    pub probe_error_category: Option<String>,
    pub summary: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyUsageSummary {
    pub linked_profile_count: i64,
    pub active_session_count: i64,
    pub last_used_at: Option<String>,
    pub session_key: Option<String>,
    pub requested_region: Option<String>,
    pub requested_provider: Option<String>,
    pub residency_status: Option<String>,
    pub rotation_mode: Option<String>,
    pub sticky_ttl_seconds: Option<i64>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyUsageItem {
    pub session_key: String,
    pub profile_id: Option<String>,
    pub profile_label: Option<String>,
    pub store_id: Option<String>,
    pub platform_id: Option<String>,
    pub site_key: Option<String>,
    pub status: String,
    pub requested_region: Option<String>,
    pub requested_provider: Option<String>,
    pub last_used_at: String,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyRow {
    pub id: String,
    pub endpoint_label: String,
    pub scheme: String,
    pub host: String,
    pub port: i64,
    pub has_credentials: bool,
    pub provider: Option<String>,
    pub source_label: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub status: String,
    pub score: f64,
    pub success_count: i64,
    pub failure_count: i64,
    pub last_checked_at: Option<String>,
    pub last_used_at: Option<String>,
    pub cooldown_until: Option<String>,
    pub last_seen_at: Option<String>,
    pub promoted_at: Option<String>,
    pub health: Option<DesktopProxyHealthSummary>,
    pub usage: DesktopProxyUsageSummary,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyPageQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub status_filters: Option<Vec<String>>,
    pub region_filters: Option<Vec<String>>,
    pub provider_filters: Option<Vec<String>>,
    pub source_filters: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyPage {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub items: Vec<DesktopProxyRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTemplateVariableDefinition {
    pub key: String,
    pub label: Option<String>,
    pub source: String,
    pub required: bool,
    pub sensitive: bool,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTemplateCoverageSummary {
    pub warm_path_count: i64,
    pub revisit_path_count: i64,
    pub stateful_path_count: i64,
    pub write_operation_path_count: i64,
    pub high_risk_path_count: i64,
    pub variable_count: i64,
    pub step_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTemplateMetadata {
    pub id: String,
    pub name: String,
    pub platform_id: String,
    pub store_id: Option<String>,
    pub source: String,
    pub status: String,
    pub readiness_level: String,
    pub preferred_locale: Option<String>,
    pub preferred_timezone: Option<String>,
    pub allowed_regions: Vec<String>,
    pub coverage: DesktopTemplateCoverageSummary,
    pub variable_definitions: Vec<DesktopTemplateVariableDefinition>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTemplateMetadataPageQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub platform_filters: Option<Vec<String>>,
    pub readiness_filters: Option<Vec<String>>,
    pub status_filters: Option<Vec<String>>,
    pub source_filters: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTemplateMetadataPage {
    pub page: i64,
    pub page_size: i64,
    pub total: i64,
    pub items: Vec<DesktopTemplateMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRecorderSnapshotQuery {
    pub session_id: Option<String>,
    pub profile_id: Option<String>,
    pub platform_id: Option<String>,
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRecorderTabSnapshot {
    pub tab_id: String,
    pub title: Option<String>,
    pub url: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRecorderStep {
    pub id: String,
    pub index: i64,
    pub action_type: String,
    pub label: String,
    pub tab_id: Option<String>,
    pub url: Option<String>,
    pub selector: Option<String>,
    pub selector_source: Option<String>,
    pub input_key: Option<String>,
    pub value_preview: Option<String>,
    pub value_source: Option<String>,
    pub wait_ms: Option<i64>,
    pub sensitive: bool,
    pub captured_at: String,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRecorderSnapshot {
    pub session_id: String,
    pub status: String,
    pub profile_id: Option<String>,
    pub platform_id: Option<String>,
    pub template_id: Option<String>,
    pub current_tab_id: Option<String>,
    pub current_url: Option<String>,
    pub is_dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub step_count: i64,
    pub sensitive_step_count: i64,
    pub variable_count: i64,
    pub started_at: Option<String>,
    pub stopped_at: Option<String>,
    pub updated_at: String,
    pub tabs: Vec<DesktopRecorderTabSnapshot>,
    pub steps: Vec<DesktopRecorderStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSyncWindowBounds {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSyncWindowState {
    pub window_id: String,
    pub native_handle: Option<String>,
    pub title: Option<String>,
    pub status: String,
    pub order_index: i64,
    pub is_main_window: bool,
    pub is_focused: bool,
    pub is_minimized: bool,
    pub is_visible: bool,
    pub profile_id: Option<String>,
    pub profile_label: Option<String>,
    pub store_id: Option<String>,
    pub platform_id: Option<String>,
    pub last_seen_at: Option<String>,
    pub last_action_at: Option<String>,
    pub bounds: Option<DesktopSyncWindowBounds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSyncLayoutState {
    pub mode: String,
    pub main_window_id: Option<String>,
    pub columns: Option<i64>,
    pub rows: Option<i64>,
    pub gap_px: i64,
    pub overlap_offset_x: Option<i64>,
    pub overlap_offset_y: Option<i64>,
    pub uniform_width: Option<i64>,
    pub uniform_height: Option<i64>,
    pub sync_scroll: bool,
    pub sync_navigation: bool,
    pub sync_input: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSynchronizerSnapshot {
    pub windows: Vec<DesktopSyncWindowState>,
    pub layout: DesktopSyncLayoutState,
    pub focused_window_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCreateProfileInput {
    pub id: String,
    pub store_id: String,
    pub platform_id: String,
    pub device_family: Option<String>,
    pub country_anchor: String,
    pub region_anchor: Option<String>,
    pub locale: String,
    pub timezone: String,
    pub fingerprint_profile_id: String,
    pub behavior_profile_id: Option<String>,
    pub network_policy_id: String,
    pub continuity_policy_id: String,
    pub credential_ref: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopUpdateProfileInput {
    pub id: String,
    pub store_id: Option<String>,
    pub platform_id: Option<String>,
    pub device_family: Option<String>,
    pub country_anchor: Option<String>,
    pub region_anchor: Option<String>,
    pub locale: Option<String>,
    pub timezone: Option<String>,
    pub fingerprint_profile_id: Option<String>,
    pub behavior_profile_id: Option<String>,
    pub network_policy_id: Option<String>,
    pub continuity_policy_id: Option<String>,
    pub credential_ref: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileMutationResult {
    pub action: String,
    pub profile: DesktopProfileDetail,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileBatchActionRequest {
    pub profile_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProfileBatchActionResult {
    pub action: String,
    pub profile_ids: Vec<String>,
    pub updated_at: String,
    pub message: String,
    pub task_ids: Vec<String>,
    pub proxy_ids: Option<Vec<String>>,
    pub verify_batch_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesktopProfileBatchActionKind {
    Start,
    Stop,
    Open,
    CheckProxy,
    Sync,
}

impl DesktopProfileBatchActionKind {
    fn action_name(self) -> &'static str {
        match self {
            DesktopProfileBatchActionKind::Start => "startProfiles",
            DesktopProfileBatchActionKind::Stop => "stopProfiles",
            DesktopProfileBatchActionKind::Open => "openProfiles",
            DesktopProfileBatchActionKind::CheckProxy => "checkProfileProxies",
            DesktopProfileBatchActionKind::Sync => "syncProfiles",
        }
    }

    fn task_kind(self) -> &'static str {
        match self {
            DesktopProfileBatchActionKind::Start => "desktop_profile_start",
            DesktopProfileBatchActionKind::Stop => "desktop_profile_stop",
            DesktopProfileBatchActionKind::Open => "desktop_profile_open",
            DesktopProfileBatchActionKind::CheckProxy => "desktop_profile_check_proxy",
            DesktopProfileBatchActionKind::Sync => "desktop_profile_sync",
        }
    }

    fn task_title(self) -> &'static str {
        match self {
            DesktopProfileBatchActionKind::Start => "Start profile",
            DesktopProfileBatchActionKind::Stop => "Stop profile",
            DesktopProfileBatchActionKind::Open => "Open profile",
            DesktopProfileBatchActionKind::CheckProxy => "Check profile proxy",
            DesktopProfileBatchActionKind::Sync => "Sync profile",
        }
    }

    fn event_type(self) -> &'static str {
        match self {
            DesktopProfileBatchActionKind::Start => "desktop_profile_start",
            DesktopProfileBatchActionKind::Stop => "desktop_profile_stop",
            DesktopProfileBatchActionKind::Open => "desktop_profile_open",
            DesktopProfileBatchActionKind::CheckProxy => "desktop_profile_proxy_check",
            DesktopProfileBatchActionKind::Sync => "desktop_profile_sync",
        }
    }

    fn log_message(self, profile: &ProfileBaseRecord) -> String {
        match self {
            DesktopProfileBatchActionKind::Start => format!(
                "Desktop workbench requested start for profile {} ({} / {}).",
                profile.id, profile.store_id, profile.platform_id
            ),
            DesktopProfileBatchActionKind::Stop => format!(
                "Desktop workbench requested stop for profile {} ({} / {}).",
                profile.id, profile.store_id, profile.platform_id
            ),
            DesktopProfileBatchActionKind::Open => format!(
                "Desktop workbench opened profile {} ({} / {}).",
                profile.id, profile.store_id, profile.platform_id
            ),
            DesktopProfileBatchActionKind::CheckProxy => format!(
                "Desktop workbench requested proxy verification for profile {} ({} / {}).",
                profile.id, profile.store_id, profile.platform_id
            ),
            DesktopProfileBatchActionKind::Sync => format!(
                "Desktop workbench requested sync for profile {} ({} / {}).",
                profile.id, profile.store_id, profile.platform_id
            ),
        }
    }
}

fn normalize_profile_batch_ids(request: &DesktopProfileBatchActionRequest) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut profile_ids = Vec::new();

    for profile_id in &request.profile_ids {
        let normalized = profile_id.trim();
        if !normalized.is_empty() && seen.insert(normalized.to_string()) {
            profile_ids.push(normalized.to_string());
        }
    }

    profile_ids
}

async fn resolve_profile_batch_targets(
    db: &DbPool,
    request: &DesktopProfileBatchActionRequest,
) -> Result<Vec<ProfileBaseRecord>> {
    let requested_ids = normalize_profile_batch_ids(request);
    if requested_ids.is_empty() {
        return Err(anyhow::anyhow!(
            "profile_ids must contain at least one profile id"
        ));
    }

    let profiles_by_id = load_profile_base_records(db)
        .await?
        .into_iter()
        .map(|profile| (profile.id.clone(), profile))
        .collect::<BTreeMap<_, _>>();

    let missing = requested_ids
        .iter()
        .filter(|profile_id| !profiles_by_id.contains_key(*profile_id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(anyhow::anyhow!(
            "profiles not found: {}",
            missing.join(", ")
        ));
    }

    Ok(requested_ids
        .into_iter()
        .filter_map(|profile_id| profiles_by_id.get(&profile_id).cloned())
        .collect())
}

async fn touch_profile_updated_at(db: &DbPool, persona_id: &str, updated_at: &str) -> Result<()> {
    sqlx::query("UPDATE persona_profiles SET updated_at = ? WHERE id = ?")
        .bind(updated_at)
        .bind(persona_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn insert_desktop_profile_task(
    db: &DbPool,
    profile: &ProfileBaseRecord,
    action_kind: DesktopProfileBatchActionKind,
    created_at: &str,
    proxy_ids: &[String],
    verify_batch_id: Option<&str>,
) -> Result<String> {
    let task_id = format!("desktop-task-{}", Uuid::new_v4());
    let result_json = serde_json::json!({
        "title": format!("{} {}", action_kind.task_title(), profile.id),
        "action": action_kind.action_name(),
        "profileId": profile.id,
        "storeId": profile.store_id,
        "platformId": profile.platform_id,
        "proxyIds": proxy_ids,
        "verifyBatchId": verify_batch_id,
        "source": "desktop_workbench",
        "status": "accepted",
    });

    sqlx::query(
        r#"INSERT INTO tasks (
               id, kind, status, input_json, priority, created_at, started_at, finished_at,
               result_json, persona_id, platform_id, fingerprint_profile_id, behavior_profile_id
           ) VALUES (?, ?, 'succeeded', ?, 0, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&task_id)
    .bind(action_kind.task_kind())
    .bind(result_json.to_string())
    .bind(created_at)
    .bind(created_at)
    .bind(created_at)
    .bind(result_json.to_string())
    .bind(&profile.id)
    .bind(&profile.platform_id)
    .bind(&profile.fingerprint_profile_id)
    .bind(&profile.behavior_profile_id)
    .execute(db)
    .await?;

    Ok(task_id)
}

async fn insert_desktop_profile_log(
    db: &DbPool,
    task_id: &str,
    message: &str,
    created_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO logs (id, task_id, run_id, level, message, created_at)
           VALUES (?, ?, NULL, 'INFO', ?, ?)"#,
    )
    .bind(format!("desktop-log-{}", Uuid::new_v4()))
    .bind(task_id)
    .bind(message)
    .bind(created_at)
    .execute(db)
    .await?;
    Ok(())
}

async fn insert_desktop_profile_event(
    db: &DbPool,
    profile: &ProfileBaseRecord,
    task_id: &str,
    action_kind: DesktopProfileBatchActionKind,
    created_at: &str,
    proxy_ids: &[String],
    verify_batch_id: Option<&str>,
) -> Result<()> {
    let event_json = serde_json::json!({
        "action": action_kind.action_name(),
        "profileId": profile.id,
        "storeId": profile.store_id,
        "platformId": profile.platform_id,
        "proxyIds": proxy_ids,
        "verifyBatchId": verify_batch_id,
        "source": "desktop_workbench",
    });

    sqlx::query(
        r#"INSERT INTO continuity_events (
               id, persona_id, store_id, platform_id, task_id, run_id, event_type, severity, event_json, created_at
           ) VALUES (?, ?, ?, ?, ?, NULL, ?, 'info', ?, ?)"#,
    )
    .bind(format!("desktop-event-{}", Uuid::new_v4()))
    .bind(&profile.id)
    .bind(&profile.store_id)
    .bind(&profile.platform_id)
    .bind(task_id)
    .bind(action_kind.event_type())
    .bind(event_json.to_string())
    .bind(created_at)
    .execute(db)
    .await?;
    Ok(())
}

async fn load_bound_proxy_ids_for_fingerprint(
    db: &DbPool,
    fingerprint_profile_id: &str,
) -> Result<Vec<String>> {
    let rows = sqlx::query_scalar::<_, String>(
        r#"SELECT DISTINCT proxy_id
           FROM proxy_session_bindings
           WHERE fingerprint_profile_id = ?
             AND proxy_id IS NOT NULL
           ORDER BY CAST(last_used_at AS INTEGER) DESC, proxy_id DESC"#,
    )
    .bind(fingerprint_profile_id)
    .fetch_all(db)
    .await?;

    Ok(rows)
}

async fn run_desktop_profile_batch_action(
    db: &DbPool,
    action_kind: DesktopProfileBatchActionKind,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult> {
    let targets = resolve_profile_batch_targets(db, &request).await?;
    let updated_at = now_ts_string();
    let mut task_ids = Vec::new();
    let mut proxy_ids: Option<Vec<String>> = None;
    let mut verify_batch_id: Option<String> = None;
    let mut proxy_ids_by_profile = BTreeMap::<String, Vec<String>>::new();

    if action_kind == DesktopProfileBatchActionKind::CheckProxy {
        let mut all_proxy_ids = Vec::new();
        let mut seen_proxy_ids = BTreeSet::new();

        for profile in &targets {
            let profile_proxy_ids =
                load_bound_proxy_ids_for_fingerprint(db, &profile.fingerprint_profile_id).await?;
            for proxy_id in &profile_proxy_ids {
                if seen_proxy_ids.insert(proxy_id.clone()) {
                    all_proxy_ids.push(proxy_id.clone());
                }
            }
            proxy_ids_by_profile.insert(profile.id.clone(), profile_proxy_ids);
        }

        if !all_proxy_ids.is_empty() {
            let response = run_desktop_proxy_batch_check(
                db,
                DesktopProxyBatchCheckRequest {
                    proxy_ids: Some(all_proxy_ids.clone()),
                    limit: Some(all_proxy_ids.len() as i64),
                    ..DesktopProxyBatchCheckRequest::default()
                },
            )
            .await?;
            verify_batch_id = Some(response.batch_id);
            proxy_ids = Some(all_proxy_ids);
        }
    }

    for profile in &targets {
        touch_profile_updated_at(db, &profile.id, &updated_at).await?;
        let profile_proxy_ids = proxy_ids_by_profile
            .get(&profile.id)
            .cloned()
            .unwrap_or_default();
        let task_id = insert_desktop_profile_task(
            db,
            profile,
            action_kind,
            &updated_at,
            &profile_proxy_ids,
            verify_batch_id.as_deref(),
        )
        .await?;
        insert_desktop_profile_log(db, &task_id, &action_kind.log_message(profile), &updated_at)
            .await?;
        insert_desktop_profile_event(
            db,
            profile,
            &task_id,
            action_kind,
            &updated_at,
            &profile_proxy_ids,
            verify_batch_id.as_deref(),
        )
        .await?;
        task_ids.push(task_id);
    }

    let message = match action_kind {
        DesktopProfileBatchActionKind::Start => format!(
            "Recorded desktop start action for {} profiles.",
            targets.len()
        ),
        DesktopProfileBatchActionKind::Stop => format!(
            "Recorded desktop stop action for {} profiles.",
            targets.len()
        ),
        DesktopProfileBatchActionKind::Open => format!(
            "Recorded desktop open action for {} profiles.",
            targets.len()
        ),
        DesktopProfileBatchActionKind::Sync => format!(
            "Recorded desktop sync action for {} profiles.",
            targets.len()
        ),
        DesktopProfileBatchActionKind::CheckProxy => {
            let proxy_count = proxy_ids.as_ref().map(|items| items.len()).unwrap_or(0);
            if let Some(batch_id) = verify_batch_id.as_deref() {
                format!(
                    "Requested proxy verification for {} profiles across {} linked proxies (batch {}).",
                    targets.len(),
                    proxy_count,
                    batch_id
                )
            } else {
                format!(
                    "Recorded proxy verification request for {} profiles, but no linked proxies were found.",
                    targets.len()
                )
            }
        }
    };

    Ok(DesktopProfileBatchActionResult {
        action: action_kind.action_name().to_string(),
        profile_ids: targets.iter().map(|profile| profile.id.clone()).collect(),
        updated_at,
        message,
        task_ids,
        proxy_ids,
        verify_batch_id,
    })
}

pub async fn start_desktop_profiles(
    db: &DbPool,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult> {
    run_desktop_profile_batch_action(db, DesktopProfileBatchActionKind::Start, request).await
}

pub async fn stop_desktop_profiles(
    db: &DbPool,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult> {
    run_desktop_profile_batch_action(db, DesktopProfileBatchActionKind::Stop, request).await
}

pub async fn open_desktop_profiles(
    db: &DbPool,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult> {
    run_desktop_profile_batch_action(db, DesktopProfileBatchActionKind::Open, request).await
}

pub async fn check_desktop_profile_proxies(
    db: &DbPool,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult> {
    run_desktop_profile_batch_action(db, DesktopProfileBatchActionKind::CheckProxy, request).await
}

pub async fn sync_desktop_profiles(
    db: &DbPool,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult> {
    run_desktop_profile_batch_action(db, DesktopProfileBatchActionKind::Sync, request).await
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyBatchCheckRequest {
    pub proxy_ids: Option<Vec<String>>,
    pub provider: Option<String>,
    pub region: Option<String>,
    pub limit: Option<i64>,
    pub only_stale: Option<bool>,
    pub stale_after_seconds: Option<i64>,
    pub task_timeout_seconds: Option<i64>,
    pub min_score: Option<f64>,
    pub recently_used_within_seconds: Option<i64>,
    pub failed_only: Option<bool>,
    pub max_per_provider: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyBatchCheckProviderSummary {
    pub provider: String,
    pub accepted: i64,
    pub skipped_due_to_cap: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyBatchCheckResponse {
    pub batch_id: String,
    pub status: String,
    pub requested_count: i64,
    pub accepted_count: i64,
    pub skipped_count: i64,
    pub stale_after_seconds: i64,
    pub task_timeout_seconds: i64,
    pub provider_summary: Vec<DesktopProxyBatchCheckProviderSummary>,
    pub filters: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyChangeIpRequest {
    pub proxy_id: String,
    pub mode: Option<String>,
    pub session_key: Option<String>,
    pub requested_provider: Option<String>,
    pub requested_region: Option<String>,
    pub sticky_ttl_seconds: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyChangeIpProviderRefresh {
    pub source_label: String,
    pub provider_key: String,
    pub status_code: u16,
    pub provider_request_id: Option<String>,
    pub response_excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopProxyChangeIpResult {
    pub proxy_id: String,
    pub status: String,
    pub mode: String,
    pub session_key: Option<String>,
    pub requested_provider: Option<String>,
    pub requested_region: Option<String>,
    pub sticky_ttl_seconds: Option<i64>,
    pub note: String,
    pub residency_status: String,
    pub rotation_mode: String,
    pub tracking_task_id: String,
    pub expires_at: Option<String>,
    pub updated_at: String,
    pub message: String,
    pub execution_status: String,
    pub rollback_signal: String,
    pub error_kind: Option<String>,
    pub provider_refresh: DesktopProxyChangeIpProviderRefresh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTemplateUpsertInput {
    pub id: String,
    pub name: String,
    pub platform_id: String,
    pub store_id: Option<String>,
    pub status: Option<String>,
    pub readiness_level: Option<String>,
    pub allowed_regions: Option<Vec<String>>,
    pub preferred_locale: Option<String>,
    pub preferred_timezone: Option<String>,
    pub warm_paths: Option<Vec<String>>,
    pub revisit_paths: Option<Vec<String>>,
    pub stateful_paths: Option<Vec<String>>,
    pub write_operation_paths: Option<Vec<String>>,
    pub high_risk_paths: Option<Vec<String>>,
    pub continuity_checks: Option<Value>,
    pub identity_markers: Option<Value>,
    pub login_loss_signals: Option<Value>,
    pub recovery_steps: Option<Value>,
    pub behavior_defaults: Option<Value>,
    pub event_chain_templates: Option<Value>,
    pub page_semantics: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTemplateDeleteInput {
    pub id: String,
    pub store_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTemplateMutationResult {
    pub action: String,
    pub template: DesktopTemplateMetadata,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCompileTemplateRunRequest {
    pub template_id: String,
    pub store_id: Option<String>,
    pub profile_ids: Vec<String>,
    pub variable_bindings: Value,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCompileTemplateRunResult {
    pub template_id: String,
    pub store_id: Option<String>,
    pub accepted_profile_count: i64,
    pub accepted_profile_ids: Vec<String>,
    pub variable_keys: Vec<String>,
    pub manifest_path: String,
    pub dry_run: bool,
    pub status: String,
    pub compiled_at: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLaunchTemplateRunRequest {
    pub template_id: String,
    pub store_id: Option<String>,
    pub profile_ids: Vec<String>,
    pub variable_bindings: Value,
    pub dry_run: Option<bool>,
    pub mode: Option<String>,
    pub launch_note: Option<String>,
    pub source_run_id: Option<String>,
    pub recorder_session_id: Option<String>,
    pub target_scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLaunchTemplateRunResult {
    pub run_id: String,
    pub task_id: Option<String>,
    pub status: String,
    pub message: String,
    pub manual_gate_request_id: Option<String>,
    pub launched_at: String,
    pub accepted_profile_count: i64,
    pub accepted_profile_ids: Vec<String>,
    pub task_ids: Vec<String>,
    pub task_count: i64,
    pub manifest_path: String,
    pub launch_summary: DesktopLaunchTemplateRunSummary,
    pub raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopReadRunDetailQuery {
    pub run_id: Option<String>,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRunArtifact {
    pub id: String,
    pub label: String,
    pub path: Option<String>,
    pub status: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopLaunchTemplateRunSummary {
    pub template_id: String,
    pub launch_kind: String,
    pub launch_mode: String,
    pub primary_task_id: Option<String>,
    pub task_count: i64,
    pub accepted_profile_count: i64,
    pub accepted_profile_ids: Vec<String>,
    pub source_run_id: Option<String>,
    pub recorder_session_id: Option<String>,
    pub target_scope: Option<String>,
    pub launch_note: Option<String>,
    pub compiled_at: String,
    pub launched_at: String,
    pub manifest_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRunTimelineEntry {
    pub id: String,
    pub label: String,
    pub status: String,
    pub detail: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRunDetail {
    pub run_id: String,
    pub task_id: Option<String>,
    pub status: String,
    pub headline: String,
    pub message: Option<String>,
    pub failure_reason: Option<String>,
    pub manual_gate_request_id: Option<String>,
    pub manual_gate_status: Option<String>,
    pub updated_at_label: Option<String>,
    pub created_at_label: Option<String>,
    pub task_status: String,
    pub run_attempt: Option<i64>,
    pub runner_kind: Option<String>,
    pub artifact_count: i64,
    pub log_count: i64,
    pub timeline_count: i64,
    pub artifacts: Vec<DesktopRunArtifact>,
    pub timeline: Vec<DesktopRunTimelineEntry>,
    pub summary: DesktopRunDetailSummary,
    pub raw: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRunDetailSummary {
    pub task_status: String,
    pub run_status: String,
    pub run_attempt: Option<i64>,
    pub runner_kind: Option<String>,
    pub artifact_count: i64,
    pub log_count: i64,
    pub timeline_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTaskWriteResult {
    pub task_id: String,
    pub status: String,
    pub message: String,
    pub updated_at: String,
    pub run_id: Option<String>,
    pub manual_gate_request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopManualGateActionRequest {
    pub manual_gate_request_id: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStartBehaviorRecordingRequest {
    pub session_id: Option<String>,
    pub profile_id: Option<String>,
    pub platform_id: Option<String>,
    pub template_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopStopBehaviorRecordingRequest {
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAppendBehaviorRecordingStepRequest {
    pub session_id: Option<String>,
    pub profile_id: Option<String>,
    pub platform_id: Option<String>,
    pub template_id: Option<String>,
    pub step_id: String,
    pub index: i64,
    pub action_type: String,
    pub label: String,
    pub tab_id: Option<String>,
    pub url: Option<String>,
    pub selector: Option<String>,
    pub selector_source: Option<String>,
    pub input_key: Option<String>,
    pub value_preview: Option<String>,
    pub value_source: Option<String>,
    pub wait_ms: Option<i64>,
    pub sensitive: bool,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSyncLayoutUpdate {
    pub mode: Option<String>,
    pub columns: Option<i64>,
    pub rows: Option<i64>,
    pub gap_px: Option<i64>,
    pub overlap_offset_x: Option<i64>,
    pub overlap_offset_y: Option<i64>,
    pub uniform_width: Option<i64>,
    pub uniform_height: Option<i64>,
    pub sync_scroll: Option<bool>,
    pub sync_navigation: Option<bool>,
    pub sync_input: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSynchronizerBroadcastRequest {
    pub channel: String,
    pub source_window_id: Option<String>,
    pub target_window_ids: Option<Vec<String>>,
    pub intent_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSynchronizerActionResult {
    pub action: String,
    pub snapshot: DesktopSynchronizerSnapshot,
    pub updated_at: String,
    pub message: String,
}

fn parse_json_text(raw: Option<&str>) -> Option<Value> {
    raw.and_then(|value| serde_json::from_str::<Value>(value).ok())
}

fn parse_string_array_text(raw: Option<&str>) -> Vec<String> {
    match parse_json_text(raw) {
        Some(Value::Array(items)) => items
            .into_iter()
            .filter_map(|item| item.as_str().map(|value| value.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_bool_flag(raw: Option<i64>) -> Option<bool> {
    raw.map(|value| value != 0)
}

fn json_string(value: Option<&Value>) -> Option<String> {
    value.map(Value::to_string)
}

fn string_array_json(values: Option<&Vec<String>>) -> String {
    let empty: Vec<String> = Vec::new();
    serde_json::to_string(values.unwrap_or(&empty)).unwrap_or_else(|_| "[]".to_string())
}

fn collect_tags(tag_groups: &[Vec<String>]) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for group in tag_groups {
        for value in group {
            let normalized = value.trim();
            if !normalized.is_empty() {
                unique.insert(normalized.to_string());
            }
        }
    }
    unique.into_iter().collect()
}

fn normalize_filter_values(raw: &Option<Vec<String>>) -> Option<Vec<String>> {
    raw.as_ref()
        .map(|values| {
            values
                .iter()
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty() && value != "all")
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
}

fn matches_filter(value: &str, filters: &Option<Vec<String>>) -> bool {
    let Some(filters) = filters.as_ref() else {
        return true;
    };

    let candidate = value.trim().to_lowercase();
    filters.iter().any(|filter| filter == &candidate)
}

fn matches_any_filter(values: &[String], filters: &Option<Vec<String>>) -> bool {
    let Some(filters) = filters.as_ref() else {
        return true;
    };

    values.iter().any(|value| {
        let candidate = value.trim().to_lowercase();
        filters.iter().any(|filter| filter == &candidate)
    })
}

fn matches_search(haystacks: &[String], search: &Option<String>) -> bool {
    let Some(search) = search
        .as_ref()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
    else {
        return true;
    };

    haystacks
        .iter()
        .any(|value| value.to_lowercase().contains(&search))
}

fn paginate_items<T: Clone>(items: &[T], page: i64, page_size: i64) -> Vec<T> {
    let start = ((page - 1) * page_size).max(0) as usize;
    if start >= items.len() {
        return Vec::new();
    }
    let end = (start + page_size as usize).min(items.len());
    items[start..end].to_vec()
}

async fn load_entity_reference(
    db: &DbPool,
    table: &str,
    id: &str,
) -> Result<DesktopEntityReference> {
    let sql = match table {
        "fingerprint_profiles" => {
            "SELECT id, name, status, version FROM fingerprint_profiles WHERE id = ?"
        }
        "behavior_profiles" => {
            "SELECT id, name, status, version FROM behavior_profiles WHERE id = ?"
        }
        "network_policies" => {
            "SELECT id, name, status, NULL as version FROM network_policies WHERE id = ?"
        }
        "continuity_policies" => {
            "SELECT id, name, status, NULL as version FROM continuity_policies WHERE id = ?"
        }
        "platform_templates" => {
            "SELECT id, name, status, NULL as version FROM platform_templates WHERE id = ?"
        }
        "store_platform_overrides" => {
            "SELECT id, platform_id as name, status, NULL as version FROM store_platform_overrides WHERE id = ?"
        }
        _ => return Err(anyhow::anyhow!("unsupported desktop entity table: {table}")),
    };

    let row = sqlx::query(sql).bind(id).fetch_optional(db).await?;
    let Some(row) = row else {
        return Err(anyhow::anyhow!("{table} not found: {id}"));
    };

    Ok(DesktopEntityReference {
        id: row.get("id"),
        name: row.get("name"),
        status: row.get("status"),
        version: row.get("version"),
    })
}

async fn ensure_reference_exists(db: &DbPool, table: &str, id: &str) -> Result<()> {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE id = ?");
    let count: i64 = sqlx::query_scalar(&sql).bind(id).fetch_one(db).await?;
    if count <= 0 {
        return Err(anyhow::anyhow!("{table} not found: {id}"));
    }
    Ok(())
}

fn template_step_count(event_chain_templates: Option<&Value>) -> i64 {
    match event_chain_templates {
        Some(Value::Array(items)) => items.len() as i64,
        Some(Value::Object(map)) => map
            .get("steps")
            .and_then(Value::as_array)
            .map(|items| items.len() as i64)
            .unwrap_or(0),
        _ => 0,
    }
}

fn template_variable_definitions(
    event_chain_templates: Option<&Value>,
) -> Vec<DesktopTemplateVariableDefinition> {
    let Some(Value::Object(map)) = event_chain_templates else {
        return Vec::new();
    };
    let Some(Value::Array(items)) = map.get("variables") else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|item| {
            let Value::Object(variable) = item else {
                return None;
            };
            let key = variable.get("key")?.as_str()?.trim().to_string();
            if key.is_empty() {
                return None;
            }
            Some(DesktopTemplateVariableDefinition {
                key,
                label: variable
                    .get("label")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                source: variable
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or("literal")
                    .to_string(),
                required: variable
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                sensitive: variable
                    .get("sensitive")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                default_value: variable.get("defaultValue").cloned(),
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
struct ProfileBaseRecord {
    id: String,
    store_id: String,
    platform_id: String,
    device_family: String,
    country_anchor: String,
    region_anchor: Option<String>,
    locale: String,
    timezone: String,
    fingerprint_profile_id: String,
    behavior_profile_id: Option<String>,
    network_policy_id: String,
    continuity_policy_id: String,
    credential_ref: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
    fingerprint_tags: Vec<String>,
    behavior_tags: Vec<String>,
}

#[derive(Debug, Clone)]
struct ProxyBaseRecord {
    id: String,
    scheme: String,
    host: String,
    port: i64,
    has_credentials: bool,
    provider: Option<String>,
    source_label: Option<String>,
    region: Option<String>,
    country: Option<String>,
    status: String,
    score: f64,
    success_count: i64,
    failure_count: i64,
    last_checked_at: Option<String>,
    last_used_at: Option<String>,
    cooldown_until: Option<String>,
    last_seen_at: Option<String>,
    promoted_at: Option<String>,
    last_smoke_status: Option<String>,
    last_smoke_protocol_ok: Option<bool>,
    last_smoke_upstream_ok: Option<bool>,
    last_exit_ip: Option<String>,
    last_anonymity_level: Option<String>,
    last_exit_country: Option<String>,
    last_exit_region: Option<String>,
    last_verify_status: Option<String>,
    last_verify_geo_match_ok: Option<bool>,
    last_verify_at: Option<String>,
    last_probe_latency_ms: Option<i64>,
    last_probe_error: Option<String>,
    last_probe_error_category: Option<String>,
    last_verify_confidence: Option<f64>,
    last_verify_score_delta: Option<i64>,
    last_verify_source: Option<String>,
    cached_trust_score: Option<i64>,
    proxy_health_score: Option<f64>,
    proxy_health_grade: Option<String>,
    proxy_health_checked_at: Option<String>,
    proxy_health_summary_json: Option<String>,
    created_at: String,
    updated_at: String,
}

async fn load_profile_base_records(db: &DbPool) -> Result<Vec<ProfileBaseRecord>> {
    let rows = sqlx::query(
        r#"SELECT
               p.id,
               p.store_id,
               p.platform_id,
               p.device_family,
               p.country_anchor,
               p.region_anchor,
               p.locale,
               p.timezone,
               p.fingerprint_profile_id,
               p.behavior_profile_id,
               p.network_policy_id,
               p.continuity_policy_id,
               p.credential_ref,
               p.status,
               p.created_at,
               p.updated_at,
               fp.tags_json AS fingerprint_tags_json,
               bp.tags_json AS behavior_tags_json
           FROM persona_profiles p
           LEFT JOIN fingerprint_profiles fp ON fp.id = p.fingerprint_profile_id
           LEFT JOIN behavior_profiles bp ON bp.id = p.behavior_profile_id
           ORDER BY CAST(p.updated_at AS INTEGER) DESC, p.id DESC"#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ProfileBaseRecord {
            id: row.get("id"),
            store_id: row.get("store_id"),
            platform_id: row.get("platform_id"),
            device_family: row.get("device_family"),
            country_anchor: row.get("country_anchor"),
            region_anchor: row.get("region_anchor"),
            locale: row.get("locale"),
            timezone: row.get("timezone"),
            fingerprint_profile_id: row.get("fingerprint_profile_id"),
            behavior_profile_id: row.get("behavior_profile_id"),
            network_policy_id: row.get("network_policy_id"),
            continuity_policy_id: row.get("continuity_policy_id"),
            credential_ref: row.get("credential_ref"),
            status: row.get("status"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            fingerprint_tags: parse_string_array_text(
                row.get::<Option<String>, _>("fingerprint_tags_json")
                    .as_deref(),
            ),
            behavior_tags: parse_string_array_text(
                row.get::<Option<String>, _>("behavior_tags_json")
                    .as_deref(),
            ),
        })
        .collect())
}

fn derive_profile_runtime_status(
    profile_status: &str,
    current_task_status: Option<&str>,
    last_task_status: Option<&str>,
    pending_action_count: i64,
) -> String {
    if profile_status.eq_ignore_ascii_case("disabled") {
        return "stopped".to_string();
    }
    if matches!(current_task_status, Some("queued")) {
        return "starting".to_string();
    }
    if matches!(current_task_status, Some("running")) {
        return "running".to_string();
    }
    if pending_action_count > 0 {
        return "syncing".to_string();
    }
    if matches!(last_task_status, Some("failed" | "timed_out" | "cancelled")) {
        return "error".to_string();
    }
    "idle".to_string()
}

async fn load_profile_runtime_summary(
    db: &DbPool,
    profile_status: &str,
    persona_id: &str,
    fingerprint_profile_id: &str,
) -> Result<DesktopProfileRuntimeSummary> {
    let row = sqlx::query(
        r#"SELECT
               (SELECT id
                  FROM tasks
                 WHERE persona_id = ?
                   AND status IN ('queued', 'running')
                 ORDER BY
                   CASE status WHEN 'running' THEN 0 ELSE 1 END ASC,
                   CAST(created_at AS INTEGER) DESC,
                   id DESC
                 LIMIT 1) AS current_task_id,
               (SELECT status
                  FROM tasks
                 WHERE persona_id = ?
                   AND status IN ('queued', 'running')
                 ORDER BY
                   CASE status WHEN 'running' THEN 0 ELSE 1 END ASC,
                   CAST(created_at AS INTEGER) DESC,
                   id DESC
                 LIMIT 1) AS current_task_status,
               (SELECT id
                  FROM tasks
                 WHERE persona_id = ?
                 ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
                 LIMIT 1) AS last_task_id,
               (SELECT status
                  FROM tasks
                 WHERE persona_id = ?
                 ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
                 LIMIT 1) AS last_task_status,
               (SELECT MAX(created_at) FROM tasks WHERE persona_id = ?) AS last_task_at,
               (SELECT MAX(created_at)
                  FROM continuity_events
                 WHERE persona_id = ?
                   AND event_type LIKE '%open%') AS last_opened_at,
               (SELECT MAX(created_at)
                  FROM continuity_events
                 WHERE persona_id = ?
                   AND event_type LIKE '%sync%') AS last_synced_at,
               (SELECT COUNT(*)
                  FROM proxy_session_bindings
                 WHERE fingerprint_profile_id = ?
                   AND (
                     expires_at IS NULL
                     OR CAST(expires_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER)
                   )) AS active_session_count,
               (SELECT COUNT(*)
                  FROM manual_gate_requests
                 WHERE persona_id = ?
                   AND status = 'pending') AS pending_action_count"#,
    )
    .bind(persona_id)
    .bind(persona_id)
    .bind(persona_id)
    .bind(persona_id)
    .bind(persona_id)
    .bind(persona_id)
    .bind(persona_id)
    .bind(fingerprint_profile_id)
    .bind(persona_id)
    .fetch_one(db)
    .await?;

    let current_task_status: Option<String> = row.get("current_task_status");
    let last_task_status: Option<String> = row.get("last_task_status");
    let pending_action_count: i64 = row.get("pending_action_count");

    Ok(DesktopProfileRuntimeSummary {
        status: derive_profile_runtime_status(
            profile_status,
            current_task_status.as_deref(),
            last_task_status.as_deref(),
            pending_action_count,
        ),
        current_task_id: row.get("current_task_id"),
        last_task_id: row.get("last_task_id"),
        last_task_status,
        last_task_at: row.get("last_task_at"),
        last_opened_at: row.get("last_opened_at"),
        last_synced_at: row.get("last_synced_at"),
        active_session_count: row.get("active_session_count"),
        pending_action_count,
    })
}

async fn load_profile_proxy_summary(
    db: &DbPool,
    fingerprint_profile_id: &str,
) -> Result<Option<DesktopProfileProxySummary>> {
    let now_ts = now_ts_string().parse::<i64>().unwrap_or(0);
    let row = sqlx::query(
        r#"SELECT
               psb.proxy_id,
                p.provider,
                COALESCE(p.region, psb.region) AS region,
                COALESCE(p.country, psb.requested_region) AS country,
                p.last_verify_status AS resolution_status,
                p.status AS proxy_status,
                psb.session_key,
                psb.requested_provider,
                psb.requested_region,
                psb.expires_at,
                p.last_verify_at AS last_verified_at,
                psb.last_used_at
            FROM proxy_session_bindings psb
            LEFT JOIN proxies p ON p.id = psb.proxy_id
            WHERE psb.fingerprint_profile_id = ?
            ORDER BY
              CASE
                WHEN psb.expires_at IS NULL THEN 1
                WHEN CAST(psb.expires_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER) THEN 0
                ELSE 2
              END ASC,
              CAST(psb.last_used_at AS INTEGER) DESC,
              psb.session_key DESC
            LIMIT 1"#,
    )
    .bind(fingerprint_profile_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| {
        let session_key: Option<String> = row.get("session_key");
        let requested_provider: Option<String> = row.get("requested_provider");
        let requested_region: Option<String> = row.get("requested_region");
        let expires_at: Option<String> = row.get("expires_at");
        let provider: Option<String> = row.get("provider");
        let region: Option<String> = row.get("region");
        let proxy_status: Option<String> = row.get("proxy_status");
        let residency_status = build_proxy_residency_status(
            session_key.as_deref(),
            expires_at.as_deref(),
            proxy_status.as_deref(),
            requested_provider.as_deref(),
            provider.as_deref(),
            requested_region.as_deref(),
            region.as_deref(),
            now_ts,
        );
        let rotation_mode = build_proxy_rotation_mode(
            None,
            session_key.as_deref(),
            &residency_status,
            requested_provider.as_deref(),
            requested_region.as_deref(),
        );

        DesktopProfileProxySummary {
            proxy_id: row.get("proxy_id"),
            provider,
            region,
            country: row.get("country"),
            resolution_status: row.get("resolution_status"),
            usage_mode: Some(rotation_mode.clone()),
            session_key,
            requested_provider,
            requested_region,
            residency_status: Some(residency_status),
            rotation_mode: Some(rotation_mode),
            sticky_ttl_seconds: sticky_ttl_seconds_from_expires_at(expires_at.as_deref(), now_ts),
            expires_at,
            last_verified_at: row.get("last_verified_at"),
            last_used_at: row.get("last_used_at"),
        }
    }))
}

async fn load_profile_health_summary(
    db: &DbPool,
    persona_id: &str,
) -> Result<Option<DesktopProfileHealthSummary>> {
    let row = sqlx::query(
        r#"SELECT
               status,
               continuity_score,
               active_session_count,
               login_risk_count,
               last_event_type,
               last_task_at,
               created_at
           FROM persona_health_snapshots
           WHERE persona_id = ?
           ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
           LIMIT 1"#,
    )
    .bind(persona_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| DesktopProfileHealthSummary {
        status: row.get("status"),
        continuity_score: row.get("continuity_score"),
        active_session_count: row.get("active_session_count"),
        login_risk_count: row.get("login_risk_count"),
        last_event_type: row.get("last_event_type"),
        last_task_at: row.get("last_task_at"),
        snapshot_at: row.get("created_at"),
    }))
}

async fn build_profile_row(db: &DbPool, base: &ProfileBaseRecord) -> Result<DesktopProfileRow> {
    let tags = collect_tags(&[base.fingerprint_tags.clone(), base.behavior_tags.clone()]);
    Ok(DesktopProfileRow {
        id: base.id.clone(),
        label: format!("{}/{}/{}", base.store_id, base.platform_id, base.id),
        store_id: base.store_id.clone(),
        platform_id: base.platform_id.clone(),
        device_family: base.device_family.clone(),
        status: base.status.clone(),
        country_anchor: base.country_anchor.clone(),
        region_anchor: base.region_anchor.clone(),
        locale: base.locale.clone(),
        timezone: base.timezone.clone(),
        group_labels: vec![base.store_id.clone()],
        tags,
        fingerprint_profile_id: base.fingerprint_profile_id.clone(),
        behavior_profile_id: base.behavior_profile_id.clone(),
        network_policy_id: base.network_policy_id.clone(),
        continuity_policy_id: base.continuity_policy_id.clone(),
        credential_ref: base.credential_ref.clone(),
        runtime: load_profile_runtime_summary(
            db,
            &base.status,
            &base.id,
            &base.fingerprint_profile_id,
        )
        .await?,
        proxy: load_profile_proxy_summary(db, &base.fingerprint_profile_id).await?,
        health: load_profile_health_summary(db, &base.id).await?,
        created_at: base.created_at.clone(),
        updated_at: base.updated_at.clone(),
    })
}

async fn load_recent_profile_tasks(db: &DbPool, persona_id: &str) -> Result<Vec<DesktopTaskItem>> {
    let rows = sqlx::query(
        r#"SELECT
               id, kind, status, priority, persona_id, platform_id, manual_gate_request_id,
               result_json, error_message, created_at, started_at, finished_at
           FROM tasks
           WHERE persona_id = ?
           ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
           LIMIT 6"#,
    )
    .bind(persona_id)
    .fetch_all(db)
    .await?;

    Ok(rows.iter().map(map_task_row).collect())
}

async fn load_recent_profile_logs(db: &DbPool, persona_id: &str) -> Result<Vec<DesktopLogItem>> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        r#"SELECT l.id, l.task_id, l.run_id, l.level, l.message, l.created_at
           FROM logs l
           JOIN tasks t ON t.id = l.task_id
           WHERE t.persona_id = ?
           ORDER BY CAST(l.created_at AS INTEGER) DESC, l.id DESC
           LIMIT 10"#,
    )
    .bind(persona_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, task_id, run_id, level, message, created_at)| DesktopLogItem {
                id,
                task_id,
                run_id,
                level,
                message,
                created_at,
            },
        )
        .collect())
}

async fn load_latest_platform_template_reference(
    db: &DbPool,
    platform_id: &str,
) -> Result<Option<DesktopEntityReference>> {
    let row = sqlx::query(
        r#"SELECT id, name, status
           FROM platform_templates
           WHERE platform_id = ?
           ORDER BY CAST(updated_at AS INTEGER) DESC, id DESC
           LIMIT 1"#,
    )
    .bind(platform_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| DesktopEntityReference {
        id: row.get("id"),
        name: row.get("name"),
        status: row.get("status"),
        version: None,
    }))
}

async fn load_latest_store_override_reference(
    db: &DbPool,
    store_id: &str,
    platform_id: &str,
) -> Result<Option<DesktopEntityReference>> {
    let row = sqlx::query(
        r#"SELECT id, platform_id, status
           FROM store_platform_overrides
           WHERE store_id = ? AND platform_id = ?
           ORDER BY CAST(updated_at AS INTEGER) DESC, id DESC
           LIMIT 1"#,
    )
    .bind(store_id)
    .bind(platform_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| DesktopEntityReference {
        id: row.get("id"),
        name: row.get("platform_id"),
        status: row.get("status"),
        version: None,
    }))
}

async fn load_profile_fingerprint_summary(
    db: &DbPool,
    fingerprint_profile_id: &str,
    target_region: Option<&str>,
    proxy_region: Option<&str>,
    exit_region: Option<&str>,
) -> Result<Option<DesktopProfileFingerprintSummary>> {
    let row = sqlx::query(
        r#"SELECT id, version, profile_json
           FROM fingerprint_profiles
           WHERE id = ?"#,
    )
    .bind(fingerprint_profile_id)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };

    let profile_id: String = row.get("id");
    let profile_version: i64 = row.get("version");
    let profile_json_text: String = row.get("profile_json");
    let profile_json =
        serde_json::from_str::<Value>(&profile_json_text).unwrap_or_else(|_| serde_json::json!({}));

    let declared_control_fields = first_family_declared_control_fields(&profile_json);
    let validation = validate_fingerprint_profile(&profile_json);
    let runtime_supported = runtime_supported_control_fields()
        .iter()
        .map(|field| (*field).to_string())
        .collect::<BTreeSet<_>>();
    let supported_fields = declared_control_fields
        .iter()
        .filter(|field| runtime_supported.contains(*field))
        .cloned()
        .collect::<Vec<_>>();
    let unsupported_fields = declared_control_fields
        .iter()
        .filter(|field| !runtime_supported.contains(*field))
        .cloned()
        .collect::<Vec<_>>();

    let consumption_projection =
        build_lightpanda_runtime_projection(&profile_id, profile_version, &profile_json);
    let consumption = consumption_projection.consumption;
    let consumption_declared_count = consumption.declared_count();
    let consumption_resolved_count = consumption.resolved_count();
    let consumption_applied_count = consumption.applied_count();
    let consumption_ignored_count = consumption.ignored_count();

    let consistency = assess_fingerprint_profile_consistency(
        target_region,
        proxy_region,
        exit_region,
        &profile_json,
    );
    let consistency_status = match consistency.overall_status {
        crate::network_identity::fingerprint_consistency::ConsistencyStatus::ExactMatch => {
            "exact_match"
        }
        crate::network_identity::fingerprint_consistency::ConsistencyStatus::SoftMatch => {
            "soft_match"
        }
        crate::network_identity::fingerprint_consistency::ConsistencyStatus::Mismatch => {
            "mismatch"
        }
        crate::network_identity::fingerprint_consistency::ConsistencyStatus::MissingContext => {
            "missing_context"
        }
        crate::network_identity::fingerprint_consistency::ConsistencyStatus::SuspiciousCombination => {
            "suspicious_combination"
        }
    }
    .to_string();

    Ok(Some(DesktopProfileFingerprintSummary {
        profile_id,
        profile_version,
        family_id: inferred_family_id(&profile_json),
        family_variant: inferred_family_variant(&profile_json),
        schema_kind: detect_fingerprint_schema_kind(&profile_json).to_string(),
        declared_control_fields: declared_control_fields.clone(),
        declared_control_count: declared_control_fields.len(),
        declared_sections: first_family_section_summaries(&profile_json)
            .into_iter()
            .map(|section| DesktopFingerprintSectionSummary {
                name: section.name,
                declared_count: section.declared_count,
                declared_fields: section.declared_fields,
            })
            .collect(),
        runtime_support: DesktopFingerprintRuntimeSupportSummary {
            supported_count: supported_fields.len(),
            unsupported_count: unsupported_fields.len(),
            supported_fields,
            unsupported_fields,
        },
        consistency: DesktopFingerprintConsistencySummary {
            status: consistency_status,
            coherence_score: consistency.coherence_score,
            hard_failure_count: consistency.hard_failure_count,
            soft_warning_count: consistency.soft_warning_count,
            risk_reasons: consistency.risk_reasons,
        },
        consumption: DesktopFingerprintConsumptionSummary {
            status: consumption.consumption_status,
            version: consumption.consumption_version,
            declared_count: consumption_declared_count,
            resolved_count: consumption_resolved_count,
            applied_count: consumption_applied_count,
            ignored_count: consumption_ignored_count,
            partial_support_warning: consumption.partial_support_warning,
        },
        validation_ok: validation.ok,
        validation_issues: validation
            .issues
            .into_iter()
            .map(|issue| {
                [Some(issue.field), Some(issue.level), Some(issue.message)]
                    .into_iter()
                    .flatten()
                    .filter(|value| !value.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .collect(),
    }))
}

pub async fn load_desktop_profile_page(
    db: &DbPool,
    query: DesktopProfilePageQuery,
) -> Result<DesktopProfilePage> {
    let page = sanitize_page(query.page);
    let page_size = sanitize_page_size(query.page_size, 50, 200);
    let search = query.search.map(|value| value.trim().to_string());
    let group_filters = normalize_filter_values(&query.group_filters);
    let tag_filters = normalize_filter_values(&query.tag_filters);
    let status_filters = normalize_filter_values(&query.status_filters);
    let platform_filters = normalize_filter_values(&query.platform_filters);
    let base_rows = load_profile_base_records(db).await?;

    let mut filtered = Vec::new();
    for base in base_rows {
        let tags = collect_tags(&[base.fingerprint_tags.clone(), base.behavior_tags.clone()]);
        let search_haystacks = vec![
            base.id.clone(),
            base.store_id.clone(),
            base.platform_id.clone(),
            base.country_anchor.clone(),
            base.region_anchor.clone().unwrap_or_default(),
            base.credential_ref.clone().unwrap_or_default(),
            tags.join(" "),
        ];
        if !matches_search(&search_haystacks, &search) {
            continue;
        }
        if !matches_filter(&base.store_id, &group_filters) {
            continue;
        }
        if !matches_any_filter(&tags, &tag_filters) {
            continue;
        }
        if !matches_filter(&base.status, &status_filters) {
            continue;
        }
        if !matches_filter(&base.platform_id, &platform_filters) {
            continue;
        }
        filtered.push(build_profile_row(db, &base).await?);
    }

    let total = filtered.len() as i64;
    let items = paginate_items(&filtered, page, page_size);

    Ok(DesktopProfilePage {
        page,
        page_size,
        total,
        items,
    })
}

pub async fn load_desktop_profile_detail(
    db: &DbPool,
    profile_id: &str,
) -> Result<DesktopProfileDetail> {
    let base_rows = load_profile_base_records(db).await?;
    let Some(base) = base_rows.into_iter().find(|item| item.id == profile_id) else {
        return Err(anyhow::anyhow!("profile not found: {profile_id}"));
    };

    let profile = build_profile_row(db, &base).await?;
    let fingerprint_profile =
        load_entity_reference(db, "fingerprint_profiles", &base.fingerprint_profile_id).await?;
    let behavior_profile = match base.behavior_profile_id.as_deref() {
        Some(id) => Some(load_entity_reference(db, "behavior_profiles", id).await?),
        None => None,
    };
    let network_policy =
        load_entity_reference(db, "network_policies", &base.network_policy_id).await?;
    let continuity_policy =
        load_entity_reference(db, "continuity_policies", &base.continuity_policy_id).await?;
    let target_region = base
        .region_anchor
        .as_deref()
        .or(Some(base.country_anchor.as_str()));
    let proxy_region = profile
        .proxy
        .as_ref()
        .and_then(|proxy| proxy.region.as_deref());
    let exit_region = profile
        .proxy
        .as_ref()
        .and_then(|proxy| proxy.country.as_deref())
        .or(proxy_region);
    let fingerprint_summary = load_profile_fingerprint_summary(
        db,
        &base.fingerprint_profile_id,
        target_region,
        proxy_region,
        exit_region,
    )
    .await?;

    Ok(DesktopProfileDetail {
        profile,
        fingerprint_profile,
        fingerprint_summary,
        behavior_profile,
        network_policy,
        continuity_policy,
        platform_template: load_latest_platform_template_reference(db, &base.platform_id).await?,
        store_platform_override: load_latest_store_override_reference(
            db,
            &base.store_id,
            &base.platform_id,
        )
        .await?,
        recent_tasks: load_recent_profile_tasks(db, &base.id).await?,
        recent_logs: load_recent_profile_logs(db, &base.id).await?,
    })
}

pub async fn create_desktop_profile(
    db: &DbPool,
    input: DesktopCreateProfileInput,
) -> Result<DesktopProfileMutationResult> {
    ensure_reference_exists(db, "fingerprint_profiles", &input.fingerprint_profile_id).await?;
    ensure_reference_exists(db, "network_policies", &input.network_policy_id).await?;
    ensure_reference_exists(db, "continuity_policies", &input.continuity_policy_id).await?;
    if let Some(behavior_profile_id) = input.behavior_profile_id.as_deref() {
        ensure_reference_exists(db, "behavior_profiles", behavior_profile_id).await?;
    }

    let now = now_ts_string();
    sqlx::query(
        r#"INSERT INTO persona_profiles (
               id, store_id, platform_id, device_family, country_anchor, region_anchor, locale,
               timezone, fingerprint_profile_id, behavior_profile_id, network_policy_id,
               continuity_policy_id, credential_ref, status, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.id)
    .bind(&input.store_id)
    .bind(&input.platform_id)
    .bind(input.device_family.as_deref().unwrap_or("desktop"))
    .bind(&input.country_anchor)
    .bind(&input.region_anchor)
    .bind(&input.locale)
    .bind(&input.timezone)
    .bind(&input.fingerprint_profile_id)
    .bind(&input.behavior_profile_id)
    .bind(&input.network_policy_id)
    .bind(&input.continuity_policy_id)
    .bind(&input.credential_ref)
    .bind(input.status.as_deref().unwrap_or("active"))
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await?;

    Ok(DesktopProfileMutationResult {
        action: "created".to_string(),
        profile: load_desktop_profile_detail(db, &input.id).await?,
        updated_at: now,
    })
}

pub async fn update_desktop_profile(
    db: &DbPool,
    input: DesktopUpdateProfileInput,
) -> Result<DesktopProfileMutationResult> {
    let row = sqlx::query(
        r#"SELECT
               store_id, platform_id, device_family, country_anchor, region_anchor, locale,
               timezone, fingerprint_profile_id, behavior_profile_id, network_policy_id,
               continuity_policy_id, credential_ref, status
           FROM persona_profiles
           WHERE id = ?"#,
    )
    .bind(&input.id)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else {
        return Err(anyhow::anyhow!("profile not found: {}", input.id));
    };

    let store_id = input.store_id.unwrap_or_else(|| row.get("store_id"));
    let platform_id = input.platform_id.unwrap_or_else(|| row.get("platform_id"));
    let device_family = input
        .device_family
        .unwrap_or_else(|| row.get("device_family"));
    let country_anchor = input
        .country_anchor
        .unwrap_or_else(|| row.get("country_anchor"));
    let region_anchor = input.region_anchor.or_else(|| row.get("region_anchor"));
    let locale = input.locale.unwrap_or_else(|| row.get("locale"));
    let timezone = input.timezone.unwrap_or_else(|| row.get("timezone"));
    let fingerprint_profile_id = input
        .fingerprint_profile_id
        .unwrap_or_else(|| row.get("fingerprint_profile_id"));
    let behavior_profile_id = match input.behavior_profile_id {
        Some(value) => Some(value),
        None => row.get("behavior_profile_id"),
    };
    let network_policy_id = input
        .network_policy_id
        .unwrap_or_else(|| row.get("network_policy_id"));
    let continuity_policy_id = input
        .continuity_policy_id
        .unwrap_or_else(|| row.get("continuity_policy_id"));
    let credential_ref = match input.credential_ref {
        Some(value) => Some(value),
        None => row.get("credential_ref"),
    };
    let status = input.status.unwrap_or_else(|| row.get("status"));

    ensure_reference_exists(db, "fingerprint_profiles", &fingerprint_profile_id).await?;
    ensure_reference_exists(db, "network_policies", &network_policy_id).await?;
    ensure_reference_exists(db, "continuity_policies", &continuity_policy_id).await?;
    if let Some(behavior_profile_id) = behavior_profile_id.as_deref() {
        ensure_reference_exists(db, "behavior_profiles", behavior_profile_id).await?;
    }

    let now = now_ts_string();
    sqlx::query(
        r#"UPDATE persona_profiles
           SET store_id = ?, platform_id = ?, device_family = ?, country_anchor = ?,
               region_anchor = ?, locale = ?, timezone = ?, fingerprint_profile_id = ?,
               behavior_profile_id = ?, network_policy_id = ?, continuity_policy_id = ?,
               credential_ref = ?, status = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&store_id)
    .bind(&platform_id)
    .bind(&device_family)
    .bind(&country_anchor)
    .bind(&region_anchor)
    .bind(&locale)
    .bind(&timezone)
    .bind(&fingerprint_profile_id)
    .bind(&behavior_profile_id)
    .bind(&network_policy_id)
    .bind(&continuity_policy_id)
    .bind(&credential_ref)
    .bind(&status)
    .bind(&now)
    .bind(&input.id)
    .execute(db)
    .await?;

    Ok(DesktopProfileMutationResult {
        action: "updated".to_string(),
        profile: load_desktop_profile_detail(db, &input.id).await?,
        updated_at: now,
    })
}

fn is_successish_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "ok" | "pass" | "passed" | "success" | "succeeded" | "healthy"
    )
}

fn is_failureish_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "fail" | "failed" | "error" | "timeout" | "timed_out" | "unhealthy"
    )
}

fn parse_i64_text(raw: Option<&str>) -> Option<i64> {
    raw.and_then(|value| value.parse::<i64>().ok())
}

async fn load_proxy_base_records(db: &DbPool) -> Result<Vec<ProxyBaseRecord>> {
    let rows = sqlx::query(
        r#"SELECT
               id,
               scheme,
               host,
               port,
               CASE
                 WHEN COALESCE(username, '') != '' OR COALESCE(password, '') != '' THEN 1
                 ELSE 0
               END AS has_credentials,
               provider,
               source_label,
               region,
               country,
               status,
               score,
               success_count,
               failure_count,
               last_checked_at,
               last_used_at,
               cooldown_until,
               last_seen_at,
               promoted_at,
               last_smoke_status,
               last_smoke_protocol_ok,
               last_smoke_upstream_ok,
               last_exit_ip,
               last_anonymity_level,
               last_exit_country,
               last_exit_region,
               last_verify_status,
               last_verify_geo_match_ok,
               last_verify_at,
               last_probe_latency_ms,
               last_probe_error,
               last_probe_error_category,
               last_verify_confidence,
               last_verify_score_delta,
               last_verify_source,
               cached_trust_score,
               proxy_health_score,
               proxy_health_grade,
               proxy_health_checked_at,
               proxy_health_summary_json,
               created_at,
               updated_at
           FROM proxies
           ORDER BY CAST(updated_at AS INTEGER) DESC, id DESC"#,
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| ProxyBaseRecord {
            id: row.get("id"),
            scheme: row.get("scheme"),
            host: row.get("host"),
            port: row.get("port"),
            has_credentials: row.get::<i64, _>("has_credentials") != 0,
            provider: row.get("provider"),
            source_label: row.get("source_label"),
            region: row.get("region"),
            country: row.get("country"),
            status: row.get("status"),
            score: row.get("score"),
            success_count: row.get("success_count"),
            failure_count: row.get("failure_count"),
            last_checked_at: row.get("last_checked_at"),
            last_used_at: row.get("last_used_at"),
            cooldown_until: row.get("cooldown_until"),
            last_seen_at: row.get("last_seen_at"),
            promoted_at: row.get("promoted_at"),
            last_smoke_status: row.get("last_smoke_status"),
            last_smoke_protocol_ok: parse_bool_flag(row.get("last_smoke_protocol_ok")),
            last_smoke_upstream_ok: parse_bool_flag(row.get("last_smoke_upstream_ok")),
            last_exit_ip: row.get("last_exit_ip"),
            last_anonymity_level: row.get("last_anonymity_level"),
            last_exit_country: row.get("last_exit_country"),
            last_exit_region: row.get("last_exit_region"),
            last_verify_status: row.get("last_verify_status"),
            last_verify_geo_match_ok: parse_bool_flag(row.get("last_verify_geo_match_ok")),
            last_verify_at: row.get("last_verify_at"),
            last_probe_latency_ms: row.get("last_probe_latency_ms"),
            last_probe_error: row.get("last_probe_error"),
            last_probe_error_category: row.get("last_probe_error_category"),
            last_verify_confidence: row.get("last_verify_confidence"),
            last_verify_score_delta: row.get("last_verify_score_delta"),
            last_verify_source: row.get("last_verify_source"),
            cached_trust_score: row.get("cached_trust_score"),
            proxy_health_score: row.get("proxy_health_score"),
            proxy_health_grade: row.get("proxy_health_grade"),
            proxy_health_checked_at: row.get("proxy_health_checked_at"),
            proxy_health_summary_json: row.get("proxy_health_summary_json"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect())
}

fn build_proxy_health_summary(base: &ProxyBaseRecord) -> Option<DesktopProxyHealthSummary> {
    let checked_at = base
        .proxy_health_checked_at
        .clone()
        .or_else(|| base.last_verify_at.clone())
        .or_else(|| base.last_checked_at.clone());
    if base.proxy_health_score.is_none()
        && base.proxy_health_grade.is_none()
        && base.cached_trust_score.is_none()
        && base.last_smoke_status.is_none()
        && base.last_verify_status.is_none()
        && base.last_verify_geo_match_ok.is_none()
        && base.last_probe_latency_ms.is_none()
        && checked_at.is_none()
    {
        return None;
    }

    Some(DesktopProxyHealthSummary {
        proxy_id: base.id.clone(),
        overall_score: base.proxy_health_score,
        grade: base.proxy_health_grade.clone(),
        trust_score: base.cached_trust_score,
        smoke_status: base.last_smoke_status.clone(),
        verify_status: base.last_verify_status.clone(),
        geo_match_ok: base.last_verify_geo_match_ok,
        latency_ms: base.last_probe_latency_ms,
        checked_at,
    })
}

fn derive_proxy_reachable(base: &ProxyBaseRecord) -> Option<bool> {
    match (base.last_smoke_protocol_ok, base.last_smoke_upstream_ok) {
        (Some(protocol_ok), Some(upstream_ok)) => Some(protocol_ok && upstream_ok),
        (Some(protocol_ok), None) => Some(protocol_ok),
        (None, Some(upstream_ok)) => Some(upstream_ok),
        (None, None) => base.last_smoke_status.as_deref().and_then(|status| {
            if is_successish_status(status) {
                Some(true)
            } else if is_failureish_status(status) {
                Some(false)
            } else {
                None
            }
        }),
    }
}

async fn load_proxy_usage_summary(
    db: &DbPool,
    base: &ProxyBaseRecord,
) -> Result<DesktopProxyUsageSummary> {
    let proxy_id = &base.id;
    let now_ts = now_ts_string().parse::<i64>().unwrap_or(0);
    let linked_profile_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(DISTINCT COALESCE(
               psb.persona_id,
               (
                 SELECT pp.id
                 FROM persona_profiles pp
                 WHERE pp.fingerprint_profile_id = psb.fingerprint_profile_id
                 ORDER BY CAST(pp.updated_at AS INTEGER) DESC, pp.id DESC
                 LIMIT 1
               )
           ))
           FROM proxy_session_bindings psb
           WHERE psb.proxy_id = ?"#,
    )
    .bind(proxy_id)
    .fetch_one(db)
    .await?;
    let active_session_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM proxy_session_bindings
           WHERE proxy_id = ?
             AND (
               expires_at IS NULL
               OR CAST(expires_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER)
             )"#,
    )
    .bind(proxy_id)
    .fetch_one(db)
    .await?;
    let last_used_at: Option<String> = sqlx::query_scalar(
        r#"SELECT MAX(last_used_at) FROM proxy_session_bindings WHERE proxy_id = ?"#,
    )
    .bind(proxy_id)
    .fetch_one(db)
    .await?;
    let latest_binding = sqlx::query(
        r#"SELECT
               session_key,
               requested_region,
               requested_provider,
               expires_at
           FROM proxy_session_bindings
           WHERE proxy_id = ?
           ORDER BY
             CASE
               WHEN expires_at IS NULL THEN 1
               WHEN CAST(expires_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER) THEN 0
               ELSE 2
             END ASC,
             CAST(last_used_at AS INTEGER) DESC,
             session_key DESC
           LIMIT 1"#,
    )
    .bind(proxy_id)
    .fetch_optional(db)
    .await?;

    let (session_key, requested_region, requested_provider, expires_at) =
        if let Some(row) = latest_binding {
            (
                row.get("session_key"),
                row.get("requested_region"),
                row.get("requested_provider"),
                row.get("expires_at"),
            )
        } else {
            (None, None, None, None)
        };
    let residency_status = build_proxy_residency_status(
        session_key.as_deref(),
        expires_at.as_deref(),
        Some(base.status.as_str()),
        requested_provider.as_deref(),
        base.provider.as_deref(),
        requested_region.as_deref(),
        base.region.as_deref(),
        now_ts,
    );
    let rotation_mode = build_proxy_rotation_mode(
        None,
        session_key.as_deref(),
        &residency_status,
        requested_provider.as_deref(),
        requested_region.as_deref(),
    );

    Ok(DesktopProxyUsageSummary {
        linked_profile_count,
        active_session_count,
        last_used_at,
        session_key,
        requested_region,
        requested_provider,
        residency_status: Some(residency_status),
        rotation_mode: Some(rotation_mode),
        sticky_ttl_seconds: sticky_ttl_seconds_from_expires_at(expires_at.as_deref(), now_ts),
        expires_at,
    })
}

async fn build_proxy_row(db: &DbPool, base: &ProxyBaseRecord) -> Result<DesktopProxyRow> {
    Ok(DesktopProxyRow {
        id: base.id.clone(),
        endpoint_label: format!("{}://{}:{}", base.scheme, base.host, base.port),
        scheme: base.scheme.clone(),
        host: base.host.clone(),
        port: base.port,
        has_credentials: base.has_credentials,
        provider: base.provider.clone(),
        source_label: base.source_label.clone(),
        region: base.region.clone(),
        country: base.country.clone(),
        status: base.status.clone(),
        score: base.score,
        success_count: base.success_count,
        failure_count: base.failure_count,
        last_checked_at: base.last_checked_at.clone(),
        last_used_at: base.last_used_at.clone(),
        cooldown_until: base.cooldown_until.clone(),
        last_seen_at: base.last_seen_at.clone(),
        promoted_at: base.promoted_at.clone(),
        health: build_proxy_health_summary(base),
        usage: load_proxy_usage_summary(db, base).await?,
        created_at: base.created_at.clone(),
        updated_at: base.updated_at.clone(),
    })
}

pub async fn load_desktop_proxy_page(
    db: &DbPool,
    query: DesktopProxyPageQuery,
) -> Result<DesktopProxyPage> {
    let page = sanitize_page(query.page);
    let page_size = sanitize_page_size(query.page_size, 50, 200);
    let search = query.search.map(|value| value.trim().to_string());
    let status_filters = normalize_filter_values(&query.status_filters);
    let region_filters = normalize_filter_values(&query.region_filters);
    let provider_filters = normalize_filter_values(&query.provider_filters);
    let source_filters = normalize_filter_values(&query.source_filters);
    let base_rows = load_proxy_base_records(db).await?;

    let mut filtered = Vec::new();
    for base in base_rows {
        let search_haystacks = vec![
            base.id.clone(),
            format!("{}://{}:{}", base.scheme, base.host, base.port),
            base.provider.clone().unwrap_or_default(),
            base.source_label.clone().unwrap_or_default(),
            base.region.clone().unwrap_or_default(),
            base.country.clone().unwrap_or_default(),
            base.status.clone(),
        ];
        if !matches_search(&search_haystacks, &search) {
            continue;
        }
        if !matches_filter(&base.status, &status_filters) {
            continue;
        }
        if !matches_filter(base.region.as_deref().unwrap_or(""), &region_filters) {
            continue;
        }
        if !matches_filter(base.provider.as_deref().unwrap_or(""), &provider_filters) {
            continue;
        }
        if !matches_filter(base.source_label.as_deref().unwrap_or(""), &source_filters) {
            continue;
        }
        filtered.push(build_proxy_row(db, &base).await?);
    }

    let total = filtered.len() as i64;
    let items = paginate_items(&filtered, page, page_size);

    Ok(DesktopProxyPage {
        page,
        page_size,
        total,
        items,
    })
}

pub async fn load_desktop_proxy_health(db: &DbPool, proxy_id: &str) -> Result<DesktopProxyHealth> {
    let base_rows = load_proxy_base_records(db).await?;
    let Some(base) = base_rows.into_iter().find(|item| item.id == proxy_id) else {
        return Err(anyhow::anyhow!("proxy not found: {proxy_id}"));
    };

    let checked_at = base
        .proxy_health_checked_at
        .clone()
        .or_else(|| base.last_verify_at.clone())
        .or_else(|| base.last_checked_at.clone());

    Ok(DesktopProxyHealth {
        proxy_id: base.id.clone(),
        overall_score: base.proxy_health_score,
        grade: base.proxy_health_grade.clone(),
        trust_score: base.cached_trust_score,
        smoke_status: base.last_smoke_status.clone(),
        verify_status: base.last_verify_status.clone(),
        geo_match_ok: base.last_verify_geo_match_ok,
        latency_ms: base.last_probe_latency_ms,
        checked_at,
        reachable: derive_proxy_reachable(&base),
        protocol_ok: base.last_smoke_protocol_ok,
        upstream_ok: base.last_smoke_upstream_ok,
        exit_ip: base.last_exit_ip.clone(),
        exit_country: base.last_exit_country.clone(),
        exit_region: base.last_exit_region.clone(),
        anonymity_level: base.last_anonymity_level.clone(),
        verify_confidence: base.last_verify_confidence,
        verify_score_delta: base.last_verify_score_delta,
        verify_source: base.last_verify_source.clone(),
        probe_error: base.last_probe_error.clone(),
        probe_error_category: base.last_probe_error_category.clone(),
        summary: parse_json_text(base.proxy_health_summary_json.as_deref()),
    })
}

pub async fn load_desktop_proxy_usage(
    db: &DbPool,
    proxy_id: &str,
) -> Result<Vec<DesktopProxyUsageItem>> {
    ensure_reference_exists(db, "proxies", proxy_id).await?;

    let rows = sqlx::query(
        r#"WITH usage AS (
               SELECT
                 psb.session_key,
                 COALESCE(
                   psb.persona_id,
                   (
                     SELECT pp.id
                     FROM persona_profiles pp
                     WHERE pp.fingerprint_profile_id = psb.fingerprint_profile_id
                     ORDER BY CAST(pp.updated_at AS INTEGER) DESC, pp.id DESC
                     LIMIT 1
                   )
                 ) AS resolved_profile_id,
                 psb.site_key,
                 psb.requested_region,
                 psb.requested_provider,
                 psb.last_used_at,
                 psb.last_success_at,
                 psb.last_failure_at,
                 psb.expires_at
               FROM proxy_session_bindings psb
               WHERE psb.proxy_id = ?
           )
           SELECT
               usage.session_key,
               usage.resolved_profile_id AS profile_id,
               CASE
                 WHEN pp.id IS NULL THEN NULL
                 ELSE printf('%s/%s/%s', pp.store_id, pp.platform_id, pp.id)
               END AS profile_label,
               pp.store_id,
               pp.platform_id,
               usage.site_key,
               CASE
                 WHEN usage.expires_at IS NOT NULL
                   AND CAST(usage.expires_at AS INTEGER) < CAST(strftime('%s','now') AS INTEGER)
                 THEN 'expired'
                 WHEN usage.last_failure_at IS NOT NULL
                   AND (
                     usage.last_success_at IS NULL
                     OR CAST(usage.last_failure_at AS INTEGER) > CAST(usage.last_success_at AS INTEGER)
                   )
                 THEN 'degraded'
                 ELSE 'active'
               END AS status,
               usage.requested_region,
               usage.requested_provider,
               usage.last_used_at,
               usage.last_success_at,
               usage.last_failure_at,
               usage.expires_at
           FROM usage
           LEFT JOIN persona_profiles pp ON pp.id = usage.resolved_profile_id
           ORDER BY CAST(usage.last_used_at AS INTEGER) DESC, usage.session_key DESC"#,
    )
    .bind(proxy_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| DesktopProxyUsageItem {
            session_key: row.get("session_key"),
            profile_id: row.get("profile_id"),
            profile_label: row.get("profile_label"),
            store_id: row.get("store_id"),
            platform_id: row.get("platform_id"),
            site_key: row.get("site_key"),
            status: row.get("status"),
            requested_region: row.get("requested_region"),
            requested_provider: row.get("requested_provider"),
            last_used_at: row.get("last_used_at"),
            last_success_at: row.get("last_success_at"),
            last_failure_at: row.get("last_failure_at"),
            expires_at: row.get("expires_at"),
        })
        .collect())
}

pub async fn run_desktop_proxy_batch_check(
    db: &DbPool,
    request: DesktopProxyBatchCheckRequest,
) -> Result<DesktopProxyBatchCheckResponse> {
    let now = now_ts_string();
    let now_i64 = now.parse::<i64>().unwrap_or(0);
    let limit = request.limit.unwrap_or(50).clamp(1, 200);
    let stale_after_seconds = request.stale_after_seconds.unwrap_or(3600).max(0);
    let task_timeout_seconds = request.task_timeout_seconds.unwrap_or(180).max(30);
    let recently_used_within_seconds = request.recently_used_within_seconds.unwrap_or(0).max(0);
    let min_score = request.min_score.unwrap_or(0.0);
    let only_stale = request.only_stale.unwrap_or(false);
    let failed_only = request.failed_only.unwrap_or(false);
    let max_per_provider = request.max_per_provider.unwrap_or(0).max(0);
    let proxy_id_filters = normalize_filter_values(&request.proxy_ids);
    let provider_filters = request.provider.clone().map(|value| vec![value]);
    let region_filters = request.region.clone().map(|value| vec![value]);
    let base_rows = load_proxy_base_records(db).await?;

    let mut requested_candidates = Vec::new();
    for base in base_rows {
        if !matches_filter(&base.id, &proxy_id_filters) {
            continue;
        }
        if !matches_filter(base.provider.as_deref().unwrap_or(""), &provider_filters) {
            continue;
        }
        if !matches_filter(base.region.as_deref().unwrap_or(""), &region_filters) {
            continue;
        }
        if base.score < min_score {
            continue;
        }
        if failed_only && !matches!(base.last_verify_status.as_deref(), Some("failed")) {
            continue;
        }
        if only_stale {
            let is_stale = match parse_i64_text(base.last_verify_at.as_deref()) {
                Some(last_verify_at) => last_verify_at <= now_i64 - stale_after_seconds,
                None => true,
            };
            if !is_stale {
                continue;
            }
        }
        if recently_used_within_seconds > 0 {
            if let Some(last_used_at) = parse_i64_text(base.last_used_at.as_deref()) {
                if last_used_at >= now_i64 - recently_used_within_seconds {
                    continue;
                }
            }
        }
        requested_candidates.push(base);
    }

    let requested_count = requested_candidates.len() as i64;
    let mut accepted_count = 0_i64;
    let mut provider_counts: BTreeMap<String, i64> = BTreeMap::new();
    let mut provider_summary_map: BTreeMap<String, DesktopProxyBatchCheckProviderSummary> =
        BTreeMap::new();

    for base in requested_candidates {
        if accepted_count >= limit {
            break;
        }

        let provider_key = base
            .provider
            .clone()
            .unwrap_or_else(|| "unassigned".to_string());
        let current_provider_count = provider_counts.get(&provider_key).copied().unwrap_or(0);
        let summary = provider_summary_map.entry(provider_key.clone()).or_insert(
            DesktopProxyBatchCheckProviderSummary {
                provider: provider_key.clone(),
                accepted: 0,
                skipped_due_to_cap: 0,
            },
        );

        if max_per_provider > 0 && current_provider_count >= max_per_provider {
            summary.skipped_due_to_cap += 1;
            continue;
        }

        accepted_count += 1;
        provider_counts.insert(provider_key.clone(), current_provider_count + 1);
        summary.accepted += 1;
    }

    let skipped_count = (requested_count - accepted_count).max(0);
    let provider_summary = provider_summary_map.into_values().collect::<Vec<_>>();
    let filters = serde_json::json!({
        "proxyIds": request.proxy_ids,
        "provider": request.provider,
        "region": request.region,
        "limit": limit,
        "onlyStale": only_stale,
        "staleAfterSeconds": stale_after_seconds,
        "taskTimeoutSeconds": task_timeout_seconds,
        "minScore": min_score,
        "recentlyUsedWithinSeconds": recently_used_within_seconds,
        "failedOnly": failed_only,
        "maxPerProvider": max_per_provider,
    });
    let batch_id = format!("verify-batch-{}", Uuid::new_v4());

    sqlx::query(
        r#"INSERT INTO verify_batches (
               id, status, requested_count, accepted_count, skipped_count, stale_after_seconds,
               task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&batch_id)
    .bind("staged")
    .bind(requested_count)
    .bind(accepted_count)
    .bind(skipped_count)
    .bind(stale_after_seconds)
    .bind(task_timeout_seconds)
    .bind(serde_json::to_string(&provider_summary)?)
    .bind(filters.to_string())
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await?;

    Ok(DesktopProxyBatchCheckResponse {
        batch_id,
        status: "staged".to_string(),
        requested_count,
        accepted_count,
        skipped_count,
        stale_after_seconds,
        task_timeout_seconds,
        provider_summary,
        filters: Some(filters),
        created_at: now.clone(),
        updated_at: now,
    })
}

#[derive(Debug, Clone)]
struct TemplateBaseRecord {
    id: String,
    name: String,
    platform_id: String,
    store_id: Option<String>,
    source: String,
    status: String,
    readiness_level: String,
    preferred_locale: Option<String>,
    preferred_timezone: Option<String>,
    allowed_regions: Vec<String>,
    warm_paths: Vec<String>,
    revisit_paths: Vec<String>,
    stateful_paths: Vec<String>,
    write_operation_paths: Vec<String>,
    high_risk_paths: Vec<String>,
    event_chain_templates: Option<Value>,
    created_at: String,
    updated_at: String,
}

fn derive_store_override_readiness(
    warm_paths: &[String],
    revisit_paths: &[String],
    stateful_paths: &[String],
    high_risk_paths: &[String],
    event_chain_templates: Option<&Value>,
) -> String {
    if template_step_count(event_chain_templates) > 0 {
        return "sample_ready".to_string();
    }
    if !warm_paths.is_empty()
        || !revisit_paths.is_empty()
        || !stateful_paths.is_empty()
        || !high_risk_paths.is_empty()
    {
        return "baseline".to_string();
    }
    "draft".to_string()
}

fn build_template_metadata(base: &TemplateBaseRecord) -> DesktopTemplateMetadata {
    let variable_definitions = template_variable_definitions(base.event_chain_templates.as_ref());
    DesktopTemplateMetadata {
        id: base.id.clone(),
        name: base.name.clone(),
        platform_id: base.platform_id.clone(),
        store_id: base.store_id.clone(),
        source: base.source.clone(),
        status: base.status.clone(),
        readiness_level: base.readiness_level.clone(),
        preferred_locale: base.preferred_locale.clone(),
        preferred_timezone: base.preferred_timezone.clone(),
        allowed_regions: base.allowed_regions.clone(),
        coverage: DesktopTemplateCoverageSummary {
            warm_path_count: base.warm_paths.len() as i64,
            revisit_path_count: base.revisit_paths.len() as i64,
            stateful_path_count: base.stateful_paths.len() as i64,
            write_operation_path_count: base.write_operation_paths.len() as i64,
            high_risk_path_count: base.high_risk_paths.len() as i64,
            variable_count: variable_definitions.len() as i64,
            step_count: template_step_count(base.event_chain_templates.as_ref()),
        },
        variable_definitions,
        created_at: base.created_at.clone(),
        updated_at: base.updated_at.clone(),
    }
}

async fn load_template_base_records(db: &DbPool) -> Result<Vec<TemplateBaseRecord>> {
    let platform_rows = sqlx::query(
        r#"SELECT
               id,
               name,
               platform_id,
               status,
               readiness_level,
               preferred_locale,
               preferred_timezone,
               allowed_regions_json,
               warm_paths_json,
               revisit_paths_json,
               stateful_paths_json,
               write_operation_paths_json,
               high_risk_paths_json,
               event_chain_templates_json,
               created_at,
               updated_at
           FROM platform_templates
           ORDER BY CAST(updated_at AS INTEGER) DESC, id DESC"#,
    )
    .fetch_all(db)
    .await?;
    let override_rows = sqlx::query(
        r#"SELECT
               id,
               store_id,
               platform_id,
               status,
               warm_paths_json,
               revisit_paths_json,
               stateful_paths_json,
               high_risk_paths_json,
               event_chain_templates_json,
               created_at,
               updated_at
           FROM store_platform_overrides
           ORDER BY CAST(updated_at AS INTEGER) DESC, id DESC"#,
    )
    .fetch_all(db)
    .await?;

    let mut items = platform_rows
        .into_iter()
        .map(|row| TemplateBaseRecord {
            id: row.get("id"),
            name: row.get("name"),
            platform_id: row.get("platform_id"),
            store_id: None,
            source: "platform_template".to_string(),
            status: row.get("status"),
            readiness_level: row.get("readiness_level"),
            preferred_locale: row.get("preferred_locale"),
            preferred_timezone: row.get("preferred_timezone"),
            allowed_regions: parse_string_array_text(
                row.get::<Option<String>, _>("allowed_regions_json")
                    .as_deref(),
            ),
            warm_paths: parse_string_array_text(
                row.get::<Option<String>, _>("warm_paths_json").as_deref(),
            ),
            revisit_paths: parse_string_array_text(
                row.get::<Option<String>, _>("revisit_paths_json")
                    .as_deref(),
            ),
            stateful_paths: parse_string_array_text(
                row.get::<Option<String>, _>("stateful_paths_json")
                    .as_deref(),
            ),
            write_operation_paths: parse_string_array_text(
                row.get::<Option<String>, _>("write_operation_paths_json")
                    .as_deref(),
            ),
            high_risk_paths: parse_string_array_text(
                row.get::<Option<String>, _>("high_risk_paths_json")
                    .as_deref(),
            ),
            event_chain_templates: parse_json_text(
                row.get::<Option<String>, _>("event_chain_templates_json")
                    .as_deref(),
            ),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect::<Vec<_>>();

    for row in override_rows {
        let warm_paths =
            parse_string_array_text(row.get::<Option<String>, _>("warm_paths_json").as_deref());
        let revisit_paths = parse_string_array_text(
            row.get::<Option<String>, _>("revisit_paths_json")
                .as_deref(),
        );
        let stateful_paths = parse_string_array_text(
            row.get::<Option<String>, _>("stateful_paths_json")
                .as_deref(),
        );
        let high_risk_paths = parse_string_array_text(
            row.get::<Option<String>, _>("high_risk_paths_json")
                .as_deref(),
        );
        let event_chain_templates = parse_json_text(
            row.get::<Option<String>, _>("event_chain_templates_json")
                .as_deref(),
        );
        let store_id: String = row.get("store_id");
        let platform_id: String = row.get("platform_id");

        items.push(TemplateBaseRecord {
            id: row.get("id"),
            name: format!("{store_id}/{platform_id} override"),
            platform_id,
            store_id: Some(store_id),
            source: "store_platform_override".to_string(),
            status: row.get("status"),
            readiness_level: derive_store_override_readiness(
                &warm_paths,
                &revisit_paths,
                &stateful_paths,
                &high_risk_paths,
                event_chain_templates.as_ref(),
            ),
            preferred_locale: None,
            preferred_timezone: None,
            allowed_regions: Vec::new(),
            warm_paths,
            revisit_paths,
            stateful_paths,
            write_operation_paths: Vec::new(),
            high_risk_paths,
            event_chain_templates,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        });
    }

    Ok(items)
}

pub async fn load_template_metadata_by_identity(
    db: &DbPool,
    id: &str,
    store_id: Option<&str>,
) -> Result<DesktopTemplateMetadata> {
    let base_rows = load_template_base_records(db).await?;
    let Some(base) = base_rows.into_iter().find(|item| {
        item.id == id
            && match (store_id, item.store_id.as_deref()) {
                (Some(expected_store_id), Some(actual_store_id)) => {
                    expected_store_id == actual_store_id
                }
                (Some(_), None) => false,
                (None, Some(_)) => false,
                (None, None) => true,
            }
    }) else {
        return Err(anyhow::anyhow!("template not found: {id}"));
    };

    Ok(build_template_metadata(&base))
}

pub async fn load_desktop_template_metadata_page(
    db: &DbPool,
    query: DesktopTemplateMetadataPageQuery,
) -> Result<DesktopTemplateMetadataPage> {
    let page = sanitize_page(query.page);
    let page_size = sanitize_page_size(query.page_size, 50, 200);
    let search = query.search.map(|value| value.trim().to_string());
    let platform_filters = normalize_filter_values(&query.platform_filters);
    let readiness_filters = normalize_filter_values(&query.readiness_filters);
    let status_filters = normalize_filter_values(&query.status_filters);
    let source_filters = normalize_filter_values(&query.source_filters);
    let base_rows = load_template_base_records(db).await?;

    let mut filtered = Vec::new();
    for base in base_rows {
        let search_haystacks = vec![
            base.id.clone(),
            base.name.clone(),
            base.platform_id.clone(),
            base.store_id.clone().unwrap_or_default(),
            base.source.clone(),
        ];
        if !matches_search(&search_haystacks, &search) {
            continue;
        }
        if !matches_filter(&base.platform_id, &platform_filters) {
            continue;
        }
        if !matches_filter(&base.readiness_level, &readiness_filters) {
            continue;
        }
        if !matches_filter(&base.status, &status_filters) {
            continue;
        }
        if !matches_filter(&base.source, &source_filters) {
            continue;
        }
        filtered.push(build_template_metadata(&base));
    }

    filtered.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.id.cmp(&left.id))
    });

    let total = filtered.len() as i64;
    let items = paginate_items(&filtered, page, page_size);

    Ok(DesktopTemplateMetadataPage {
        page,
        page_size,
        total,
        items,
    })
}

pub async fn save_desktop_template(
    db: &DbPool,
    input: DesktopTemplateUpsertInput,
) -> Result<DesktopTemplateMutationResult> {
    if input.id.trim().is_empty() {
        return Err(anyhow::anyhow!("template id is required"));
    }
    if input.name.trim().is_empty() {
        return Err(anyhow::anyhow!("template name is required"));
    }
    if input.platform_id.trim().is_empty() {
        return Err(anyhow::anyhow!("template platform_id is required"));
    }

    let now = now_ts_string();
    if let Some(store_id) = input.store_id.clone() {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM store_platform_overrides WHERE id = ? AND store_id = ?",
        )
        .bind(&input.id)
        .bind(&store_id)
        .fetch_one(db)
        .await?;
        if exists > 0 {
            return Err(anyhow::anyhow!(
                "store platform override already exists: {}",
                input.id
            ));
        }

        sqlx::query(
            r#"INSERT INTO store_platform_overrides (
                   id, store_id, platform_id, admin_origin, entry_origin, entry_paths_json,
                   warm_paths_json, revisit_paths_json, stateful_paths_json, high_risk_paths_json,
                   recovery_steps_json, login_loss_signals_json, identity_markers_json,
                   behavior_defaults_json, event_chain_templates_json, page_semantics_json,
                   status, created_at, updated_at
               ) VALUES (?, ?, ?, NULL, NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&input.id)
        .bind(&store_id)
        .bind(&input.platform_id)
        .bind(string_array_json(input.warm_paths.as_ref()))
        .bind(string_array_json(input.revisit_paths.as_ref()))
        .bind(string_array_json(input.stateful_paths.as_ref()))
        .bind(string_array_json(input.high_risk_paths.as_ref()))
        .bind(json_string(input.recovery_steps.as_ref()))
        .bind(json_string(input.login_loss_signals.as_ref()))
        .bind(json_string(input.identity_markers.as_ref()))
        .bind(json_string(input.behavior_defaults.as_ref()))
        .bind(json_string(input.event_chain_templates.as_ref()))
        .bind(json_string(input.page_semantics.as_ref()))
        .bind(input.status.as_deref().unwrap_or("draft"))
        .bind(&now)
        .bind(&now)
        .execute(db)
        .await?;

        return Ok(DesktopTemplateMutationResult {
            action: "created".to_string(),
            template: load_template_metadata_by_identity(db, &input.id, Some(&store_id)).await?,
            updated_at: now,
        });
    }

    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM platform_templates WHERE id = ?")
        .bind(&input.id)
        .fetch_one(db)
        .await?;
    if exists > 0 {
        return Err(anyhow::anyhow!(
            "platform template already exists: {}",
            input.id
        ));
    }

    sqlx::query(
        r#"INSERT INTO platform_templates (
               id, platform_id, name, warm_paths_json, revisit_paths_json, stateful_paths_json,
               write_operation_paths_json, high_risk_paths_json, allowed_regions_json,
               preferred_locale, preferred_timezone, continuity_checks_json, identity_markers_json,
               login_loss_signals_json, recovery_steps_json, behavior_defaults_json,
               event_chain_templates_json, page_semantics_json, readiness_level, status,
               created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&input.id)
    .bind(&input.platform_id)
    .bind(&input.name)
    .bind(string_array_json(input.warm_paths.as_ref()))
    .bind(string_array_json(input.revisit_paths.as_ref()))
    .bind(string_array_json(input.stateful_paths.as_ref()))
    .bind(string_array_json(input.write_operation_paths.as_ref()))
    .bind(string_array_json(input.high_risk_paths.as_ref()))
    .bind(string_array_json(input.allowed_regions.as_ref()))
    .bind(&input.preferred_locale)
    .bind(&input.preferred_timezone)
    .bind(json_string(input.continuity_checks.as_ref()))
    .bind(json_string(input.identity_markers.as_ref()))
    .bind(json_string(input.login_loss_signals.as_ref()))
    .bind(json_string(input.recovery_steps.as_ref()))
    .bind(json_string(input.behavior_defaults.as_ref()))
    .bind(json_string(input.event_chain_templates.as_ref()))
    .bind(json_string(input.page_semantics.as_ref()))
    .bind(input.readiness_level.as_deref().unwrap_or("draft"))
    .bind(input.status.as_deref().unwrap_or("draft"))
    .bind(&now)
    .bind(&now)
    .execute(db)
    .await?;

    Ok(DesktopTemplateMutationResult {
        action: "created".to_string(),
        template: load_template_metadata_by_identity(db, &input.id, None).await?,
        updated_at: now,
    })
}

pub async fn update_desktop_template(
    db: &DbPool,
    input: DesktopTemplateUpsertInput,
) -> Result<DesktopTemplateMutationResult> {
    let now = now_ts_string();
    if let Some(store_id) = input.store_id.clone() {
        let row = sqlx::query(
            r#"SELECT
                   platform_id,
                   status,
                   warm_paths_json,
                   revisit_paths_json,
                   stateful_paths_json,
                   high_risk_paths_json,
                   recovery_steps_json,
                   login_loss_signals_json,
                   identity_markers_json,
                   behavior_defaults_json,
                   event_chain_templates_json,
                   page_semantics_json
               FROM store_platform_overrides
               WHERE id = ? AND store_id = ?"#,
        )
        .bind(&input.id)
        .bind(&store_id)
        .fetch_optional(db)
        .await?;
        let Some(row) = row else {
            return Err(anyhow::anyhow!(
                "store platform override not found: {}",
                input.id
            ));
        };

        let platform_id = if input.platform_id.trim().is_empty() {
            row.get("platform_id")
        } else {
            input.platform_id.clone()
        };
        let status = input.status.unwrap_or_else(|| row.get("status"));
        let warm_paths = input.warm_paths.unwrap_or_else(|| {
            parse_string_array_text(row.get::<Option<String>, _>("warm_paths_json").as_deref())
        });
        let revisit_paths = input.revisit_paths.unwrap_or_else(|| {
            parse_string_array_text(
                row.get::<Option<String>, _>("revisit_paths_json")
                    .as_deref(),
            )
        });
        let stateful_paths = input.stateful_paths.unwrap_or_else(|| {
            parse_string_array_text(
                row.get::<Option<String>, _>("stateful_paths_json")
                    .as_deref(),
            )
        });
        let high_risk_paths = input.high_risk_paths.unwrap_or_else(|| {
            parse_string_array_text(
                row.get::<Option<String>, _>("high_risk_paths_json")
                    .as_deref(),
            )
        });
        let recovery_steps = input.recovery_steps.or_else(|| {
            parse_json_text(
                row.get::<Option<String>, _>("recovery_steps_json")
                    .as_deref(),
            )
        });
        let login_loss_signals = input.login_loss_signals.or_else(|| {
            parse_json_text(
                row.get::<Option<String>, _>("login_loss_signals_json")
                    .as_deref(),
            )
        });
        let identity_markers = input.identity_markers.or_else(|| {
            parse_json_text(
                row.get::<Option<String>, _>("identity_markers_json")
                    .as_deref(),
            )
        });
        let behavior_defaults = input.behavior_defaults.or_else(|| {
            parse_json_text(
                row.get::<Option<String>, _>("behavior_defaults_json")
                    .as_deref(),
            )
        });
        let event_chain_templates = input.event_chain_templates.or_else(|| {
            parse_json_text(
                row.get::<Option<String>, _>("event_chain_templates_json")
                    .as_deref(),
            )
        });
        let page_semantics = input.page_semantics.or_else(|| {
            parse_json_text(
                row.get::<Option<String>, _>("page_semantics_json")
                    .as_deref(),
            )
        });

        sqlx::query(
            r#"UPDATE store_platform_overrides
               SET platform_id = ?, warm_paths_json = ?, revisit_paths_json = ?,
                   stateful_paths_json = ?, high_risk_paths_json = ?, recovery_steps_json = ?,
                   login_loss_signals_json = ?, identity_markers_json = ?, behavior_defaults_json = ?,
                   event_chain_templates_json = ?, page_semantics_json = ?, status = ?, updated_at = ?
               WHERE id = ? AND store_id = ?"#,
        )
        .bind(&platform_id)
        .bind(string_array_json(Some(&warm_paths)))
        .bind(string_array_json(Some(&revisit_paths)))
        .bind(string_array_json(Some(&stateful_paths)))
        .bind(string_array_json(Some(&high_risk_paths)))
        .bind(json_string(recovery_steps.as_ref()))
        .bind(json_string(login_loss_signals.as_ref()))
        .bind(json_string(identity_markers.as_ref()))
        .bind(json_string(behavior_defaults.as_ref()))
        .bind(json_string(event_chain_templates.as_ref()))
        .bind(json_string(page_semantics.as_ref()))
        .bind(&status)
        .bind(&now)
        .bind(&input.id)
        .bind(&store_id)
        .execute(db)
        .await?;

        return Ok(DesktopTemplateMutationResult {
            action: "updated".to_string(),
            template: load_template_metadata_by_identity(db, &input.id, Some(&store_id)).await?,
            updated_at: now,
        });
    }

    let row = sqlx::query(
        r#"SELECT
               name,
               platform_id,
               status,
               readiness_level,
               allowed_regions_json,
               preferred_locale,
               preferred_timezone,
               warm_paths_json,
               revisit_paths_json,
               stateful_paths_json,
               write_operation_paths_json,
               high_risk_paths_json,
               continuity_checks_json,
               identity_markers_json,
               login_loss_signals_json,
               recovery_steps_json,
               behavior_defaults_json,
               event_chain_templates_json,
               page_semantics_json
           FROM platform_templates
           WHERE id = ?"#,
    )
    .bind(&input.id)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else {
        return Err(anyhow::anyhow!("platform template not found: {}", input.id));
    };

    let name = if input.name.trim().is_empty() {
        row.get("name")
    } else {
        input.name.clone()
    };
    let platform_id = if input.platform_id.trim().is_empty() {
        row.get("platform_id")
    } else {
        input.platform_id.clone()
    };
    let status = input.status.unwrap_or_else(|| row.get("status"));
    let readiness_level = input
        .readiness_level
        .unwrap_or_else(|| row.get("readiness_level"));
    let allowed_regions = input.allowed_regions.unwrap_or_else(|| {
        parse_string_array_text(
            row.get::<Option<String>, _>("allowed_regions_json")
                .as_deref(),
        )
    });
    let preferred_locale = input
        .preferred_locale
        .or_else(|| row.get("preferred_locale"));
    let preferred_timezone = input
        .preferred_timezone
        .or_else(|| row.get("preferred_timezone"));
    let warm_paths = input.warm_paths.unwrap_or_else(|| {
        parse_string_array_text(row.get::<Option<String>, _>("warm_paths_json").as_deref())
    });
    let revisit_paths = input.revisit_paths.unwrap_or_else(|| {
        parse_string_array_text(
            row.get::<Option<String>, _>("revisit_paths_json")
                .as_deref(),
        )
    });
    let stateful_paths = input.stateful_paths.unwrap_or_else(|| {
        parse_string_array_text(
            row.get::<Option<String>, _>("stateful_paths_json")
                .as_deref(),
        )
    });
    let write_operation_paths = input.write_operation_paths.unwrap_or_else(|| {
        parse_string_array_text(
            row.get::<Option<String>, _>("write_operation_paths_json")
                .as_deref(),
        )
    });
    let high_risk_paths = input.high_risk_paths.unwrap_or_else(|| {
        parse_string_array_text(
            row.get::<Option<String>, _>("high_risk_paths_json")
                .as_deref(),
        )
    });
    let continuity_checks = input.continuity_checks.or_else(|| {
        parse_json_text(
            row.get::<Option<String>, _>("continuity_checks_json")
                .as_deref(),
        )
    });
    let identity_markers = input.identity_markers.or_else(|| {
        parse_json_text(
            row.get::<Option<String>, _>("identity_markers_json")
                .as_deref(),
        )
    });
    let login_loss_signals = input.login_loss_signals.or_else(|| {
        parse_json_text(
            row.get::<Option<String>, _>("login_loss_signals_json")
                .as_deref(),
        )
    });
    let recovery_steps = input.recovery_steps.or_else(|| {
        parse_json_text(
            row.get::<Option<String>, _>("recovery_steps_json")
                .as_deref(),
        )
    });
    let behavior_defaults = input.behavior_defaults.or_else(|| {
        parse_json_text(
            row.get::<Option<String>, _>("behavior_defaults_json")
                .as_deref(),
        )
    });
    let event_chain_templates = input.event_chain_templates.or_else(|| {
        parse_json_text(
            row.get::<Option<String>, _>("event_chain_templates_json")
                .as_deref(),
        )
    });
    let page_semantics = input.page_semantics.or_else(|| {
        parse_json_text(
            row.get::<Option<String>, _>("page_semantics_json")
                .as_deref(),
        )
    });

    sqlx::query(
        r#"UPDATE platform_templates
           SET platform_id = ?, name = ?, warm_paths_json = ?, revisit_paths_json = ?,
               stateful_paths_json = ?, write_operation_paths_json = ?, high_risk_paths_json = ?,
               allowed_regions_json = ?, preferred_locale = ?, preferred_timezone = ?,
               continuity_checks_json = ?, identity_markers_json = ?, login_loss_signals_json = ?,
               recovery_steps_json = ?, behavior_defaults_json = ?, event_chain_templates_json = ?,
               page_semantics_json = ?, readiness_level = ?, status = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&platform_id)
    .bind(&name)
    .bind(string_array_json(Some(&warm_paths)))
    .bind(string_array_json(Some(&revisit_paths)))
    .bind(string_array_json(Some(&stateful_paths)))
    .bind(string_array_json(Some(&write_operation_paths)))
    .bind(string_array_json(Some(&high_risk_paths)))
    .bind(string_array_json(Some(&allowed_regions)))
    .bind(&preferred_locale)
    .bind(&preferred_timezone)
    .bind(json_string(continuity_checks.as_ref()))
    .bind(json_string(identity_markers.as_ref()))
    .bind(json_string(login_loss_signals.as_ref()))
    .bind(json_string(recovery_steps.as_ref()))
    .bind(json_string(behavior_defaults.as_ref()))
    .bind(json_string(event_chain_templates.as_ref()))
    .bind(json_string(page_semantics.as_ref()))
    .bind(&readiness_level)
    .bind(&status)
    .bind(&now)
    .bind(&input.id)
    .execute(db)
    .await?;

    Ok(DesktopTemplateMutationResult {
        action: "updated".to_string(),
        template: load_template_metadata_by_identity(db, &input.id, None).await?,
        updated_at: now,
    })
}

pub async fn delete_desktop_template(
    db: &DbPool,
    input: DesktopTemplateDeleteInput,
) -> Result<DesktopTemplateMutationResult> {
    let template =
        load_template_metadata_by_identity(db, &input.id, input.store_id.as_deref()).await?;
    let now = now_ts_string();

    if let Some(store_id) = input.store_id {
        sqlx::query("DELETE FROM store_platform_overrides WHERE id = ? AND store_id = ?")
            .bind(&input.id)
            .bind(&store_id)
            .execute(db)
            .await?;
    } else {
        sqlx::query("DELETE FROM platform_templates WHERE id = ?")
            .bind(&input.id)
            .execute(db)
            .await?;
    }

    Ok(DesktopTemplateMutationResult {
        action: "deleted".to_string(),
        template,
        updated_at: now,
    })
}

fn sanitize_manifest_file_fragment(raw: &str) -> String {
    raw.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

pub async fn compile_desktop_template_run(
    db: &DbPool,
    database_url: &str,
    request: DesktopCompileTemplateRunRequest,
) -> Result<DesktopCompileTemplateRunResult> {
    let template_id = request.template_id.trim().to_string();
    if template_id.is_empty() {
        return Err(anyhow::anyhow!("template_id is required"));
    }

    let Some(variable_bindings) = request.variable_bindings.as_object().cloned() else {
        return Err(anyhow::anyhow!(
            "variable_bindings must be a JSON object keyed by variable name"
        ));
    };

    let template =
        load_template_metadata_by_identity(db, &template_id, request.store_id.as_deref()).await?;

    let missing_required_variables = template
        .variable_definitions
        .iter()
        .filter(|definition| {
            definition.required && !variable_bindings.contains_key(&definition.key)
        })
        .map(|definition| definition.key.clone())
        .collect::<Vec<_>>();
    if !missing_required_variables.is_empty() {
        return Err(anyhow::anyhow!(
            "missing required variable bindings: {}",
            missing_required_variables.join(", ")
        ));
    }

    let mut requested_profile_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for profile_id in request.profile_ids {
        let normalized = profile_id.trim();
        if !normalized.is_empty() && seen.insert(normalized.to_string()) {
            requested_profile_ids.push(normalized.to_string());
        }
    }
    if requested_profile_ids.is_empty() {
        return Err(anyhow::anyhow!(
            "profile_ids must contain at least one profile id"
        ));
    }

    let profiles_by_id = load_profile_base_records(db)
        .await?
        .into_iter()
        .map(|profile| (profile.id.clone(), profile))
        .collect::<BTreeMap<_, _>>();

    let missing_profile_ids = requested_profile_ids
        .iter()
        .filter(|profile_id| !profiles_by_id.contains_key(*profile_id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing_profile_ids.is_empty() {
        return Err(anyhow::anyhow!(
            "profiles not found: {}",
            missing_profile_ids.join(", ")
        ));
    }

    let mut accepted_profile_ids = Vec::new();
    let mut rejected_profile_ids = Vec::new();

    for profile_id in requested_profile_ids {
        let Some(profile) = profiles_by_id.get(&profile_id) else {
            continue;
        };

        let store_matches = template
            .store_id
            .as_deref()
            .map(|store_id| store_id == profile.store_id)
            .unwrap_or(true);
        let platform_matches = profile.platform_id == template.platform_id;

        if store_matches && platform_matches {
            accepted_profile_ids.push(profile.id.clone());
        } else {
            rejected_profile_ids.push(profile.id.clone());
        }
    }

    if accepted_profile_ids.is_empty() {
        return Err(anyhow::anyhow!(
            "no profiles matched template {} (platform {}{})",
            template.id,
            template.platform_id,
            template
                .store_id
                .as_deref()
                .map(|store_id| format!(", store {store_id}"))
                .unwrap_or_default()
        ));
    }

    let variable_keys = variable_bindings.keys().cloned().collect::<Vec<_>>();
    let compiled_at = now_ts_string();
    let dry_run = request.dry_run.unwrap_or(false);
    let manifest_dir = asset_workspace_root_from_database_url(database_url).join("compiled-runs");
    fs::create_dir_all(&manifest_dir)?;

    let manifest_path = manifest_dir.join(format!(
        "{}-{}.json",
        sanitize_manifest_file_fragment(&template.id),
        compiled_at
    ));
    let manifest_payload = serde_json::json!({
        "templateId": template.id,
        "templateName": template.name,
        "storeId": template.store_id,
        "platformId": template.platform_id,
        "source": template.source,
        "dryRun": dry_run,
        "compiledAt": compiled_at,
        "acceptedProfileIds": accepted_profile_ids,
        "rejectedProfileIds": rejected_profile_ids,
        "variableBindings": variable_bindings,
        "variableKeys": variable_keys,
    });
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_payload)?,
    )?;

    let accepted_profile_count = manifest_payload["acceptedProfileIds"]
        .as_array()
        .map(|items| items.len() as i64)
        .unwrap_or(0);
    let status = if dry_run {
        "dry_run_ready".to_string()
    } else if manifest_payload["rejectedProfileIds"]
        .as_array()
        .map(|items| items.is_empty())
        .unwrap_or(true)
    {
        "manifest_written".to_string()
    } else {
        "manifest_written_partial".to_string()
    };

    Ok(DesktopCompileTemplateRunResult {
        template_id: template_id.clone(),
        store_id: template.store_id,
        accepted_profile_count,
        accepted_profile_ids: manifest_payload["acceptedProfileIds"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
            .collect(),
        variable_keys: manifest_payload["variableKeys"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|value| value.as_str().map(ToOwned::to_owned))
            .collect(),
        manifest_path: manifest_path.to_string_lossy().to_string(),
        dry_run,
        status: status.clone(),
        compiled_at,
        message: if status == "manifest_written_partial" {
            format!(
                "Compile manifest written with {} accepted profiles and {} rejected profiles.",
                accepted_profile_count,
                manifest_payload["rejectedProfileIds"]
                    .as_array()
                    .map(|items| items.len())
                    .unwrap_or(0)
            )
        } else if dry_run {
            format!(
                "Dry-run compile manifest written for {} profiles.",
                accepted_profile_count
            )
        } else {
            format!(
                "Compile manifest written for {} profiles.",
                accepted_profile_count
            )
        },
    })
}

async fn insert_desktop_task_log(
    db: &DbPool,
    task_id: &str,
    run_id: Option<&str>,
    level: &str,
    message: &str,
    created_at: &str,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO logs (id, task_id, run_id, level, message, created_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
    )
    .bind(format!("desktop-log-{}", Uuid::new_v4()))
    .bind(task_id)
    .bind(run_id)
    .bind(level)
    .bind(message)
    .bind(created_at)
    .execute(db)
    .await?;

    Ok(())
}

async fn latest_run_id_for_task(db: &DbPool, task_id: &str) -> Result<Option<String>> {
    sqlx::query_scalar::<_, String>(
        r#"SELECT id
           FROM runs
           WHERE task_id = ?
           ORDER BY attempt DESC, id DESC
           LIMIT 1"#,
    )
    .bind(task_id)
    .fetch_optional(db)
    .await
    .map_err(Into::into)
}

fn pick_message_from_json(root: Option<&Value>) -> Option<String> {
    root.and_then(|value| {
        pick_string(
            value,
            &[
                &["message"],
                &["error_message"],
                &["error", "message"],
                &["summary"],
            ],
        )
    })
}

fn push_timeline_entry(
    timeline: &mut Vec<DesktopRunTimelineEntry>,
    id: impl Into<String>,
    label: impl Into<String>,
    status: impl Into<String>,
    detail: Option<String>,
    created_at: Option<String>,
) {
    timeline.push(DesktopRunTimelineEntry {
        id: id.into(),
        label: label.into(),
        status: status.into(),
        detail,
        created_at,
    });
}

pub async fn launch_desktop_template_run(
    db: &DbPool,
    database_url: &str,
    request: DesktopLaunchTemplateRunRequest,
) -> Result<DesktopLaunchTemplateRunResult> {
    if request.dry_run.unwrap_or(false) {
        return Err(anyhow::anyhow!(
            "launchTemplateRun does not accept dry_run=true; use compileTemplateRun for dry-run preflight"
        ));
    }

    let compile = compile_desktop_template_run(
        db,
        database_url,
        DesktopCompileTemplateRunRequest {
            template_id: request.template_id.clone(),
            store_id: request.store_id.clone(),
            profile_ids: request.profile_ids.clone(),
            variable_bindings: request.variable_bindings.clone(),
            dry_run: Some(false),
        },
    )
    .await?;

    let launched_at = now_ts_string();
    let task_kind = "template_run";
    let launch_mode = request.mode.unwrap_or_else(|| "queue".to_string());
    let launch_note = request.launch_note.clone();
    let source_run_id = request.source_run_id.clone();
    let recorder_session_id = request.recorder_session_id.clone();
    let target_scope = request.target_scope.clone();
    let variable_bindings = request.variable_bindings.clone();
    let template =
        load_template_metadata_by_identity(db, &request.template_id, request.store_id.as_deref())
            .await?;
    let profiles_by_id = load_profile_base_records(db)
        .await?
        .into_iter()
        .map(|profile| (profile.id.clone(), profile))
        .collect::<BTreeMap<_, _>>();

    let mut task_ids = Vec::new();
    for profile_id in &compile.accepted_profile_ids {
        let Some(profile) = profiles_by_id.get(profile_id) else {
            continue;
        };

        let task_id = format!("desktop-launch-{}", Uuid::new_v4());
        let input_json = serde_json::json!({
            "template_id": template.id,
            "flow_template_id": template.id,
            "template_name": template.name,
            "platform_id": template.platform_id,
            "store_id": template.store_id,
            "profile_id": profile.id,
            "persona_id": profile.id,
            "fingerprint_profile_id": profile.fingerprint_profile_id,
            "behavior_profile_id": profile.behavior_profile_id,
            "compiled_manifest_path": compile.manifest_path.clone(),
            "compiled_at": compile.compiled_at.clone(),
            "launch_mode": launch_mode.clone(),
            "launch_note": launch_note.clone(),
            "source_run_id": source_run_id.clone(),
            "recorder_session_id": recorder_session_id.clone(),
            "target_scope": target_scope.clone(),
            "variable_bindings": variable_bindings.clone(),
            "timeout_seconds": 180,
            "url": null,
            "script": null,
            "source": "desktop_workbench",
        });

        sqlx::query(
            r#"INSERT INTO tasks (
                   id, kind, status, input_json, priority, created_at, queued_at,
                   persona_id, platform_id, fingerprint_profile_id, behavior_profile_id
               ) VALUES (?, ?, 'queued', ?, 10, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&task_id)
        .bind(task_kind)
        .bind(input_json.to_string())
        .bind(&launched_at)
        .bind(&launched_at)
        .bind(&profile.id)
        .bind(&profile.platform_id)
        .bind(&profile.fingerprint_profile_id)
        .bind(&profile.behavior_profile_id)
        .execute(db)
        .await?;

        insert_desktop_task_log(
            db,
            &task_id,
            None,
            "INFO",
            &format!(
                "desktop launch queued from template {} for profile {}",
                template.name, profile.id
            ),
            &launched_at,
        )
        .await?;

        sqlx::query(
            r#"INSERT INTO continuity_events (
                   id, persona_id, store_id, platform_id, task_id, run_id, event_type, severity, event_json, created_at
               ) VALUES (?, ?, ?, ?, ?, NULL, ?, 'info', ?, ?)"#,
        )
        .bind(format!("desktop-event-{}", Uuid::new_v4()))
        .bind(&profile.id)
        .bind(&profile.store_id)
        .bind(&profile.platform_id)
        .bind(&task_id)
        .bind("desktop_template_run_launch_requested")
        .bind(
            serde_json::json!({
                "templateId": template.id,
                "manifestPath": compile.manifest_path.clone(),
                "launchMode": launch_mode.clone(),
                "sourceRunId": source_run_id.clone(),
                "recorderSessionId": recorder_session_id.clone(),
            })
            .to_string(),
        )
        .bind(&launched_at)
        .execute(db)
        .await?;

        touch_profile_updated_at(db, &profile.id, &launched_at).await?;
        task_ids.push(task_id);
    }

    let anchor_task_id = task_ids.first().cloned();
    let anchor_run_id = anchor_task_id
        .clone()
        .unwrap_or_else(|| format!("desktop-launch-anchor-{}", Uuid::new_v4()));
    let manifest_path = compile.manifest_path.clone();
    let launch_summary = DesktopLaunchTemplateRunSummary {
        template_id: template.id.clone(),
        launch_kind: "template_run_fanout".to_string(),
        launch_mode: launch_mode.clone(),
        primary_task_id: anchor_task_id.clone(),
        task_count: task_ids.len() as i64,
        accepted_profile_count: compile.accepted_profile_count,
        accepted_profile_ids: compile.accepted_profile_ids.clone(),
        source_run_id: source_run_id.clone(),
        recorder_session_id: recorder_session_id.clone(),
        target_scope: target_scope.clone(),
        launch_note: launch_note.clone(),
        compiled_at: compile.compiled_at.clone(),
        launched_at: launched_at.clone(),
        manifest_path: manifest_path.clone(),
    };

    Ok(DesktopLaunchTemplateRunResult {
        run_id: anchor_run_id,
        task_id: anchor_task_id,
        status: "queued".to_string(),
        message: format!(
            "Queued {} template_run tasks from compile manifest.",
            task_ids.len()
        ),
        manual_gate_request_id: None,
        launched_at: launched_at.clone(),
        accepted_profile_count: compile.accepted_profile_count,
        accepted_profile_ids: compile.accepted_profile_ids,
        task_ids: task_ids.clone(),
        task_count: task_ids.len() as i64,
        manifest_path: manifest_path.clone(),
        launch_summary: launch_summary.clone(),
        raw: Some(serde_json::json!({
            "taskKind": task_kind,
            "taskIds": task_ids,
            "templateId": template.id,
            "launchMode": launch_mode,
            "compiledAt": compile.compiled_at.clone(),
            "manifestPath": manifest_path,
            "launchSummary": launch_summary,
        })),
    })
}

pub async fn read_desktop_run_detail(
    db: &DbPool,
    query: DesktopReadRunDetailQuery,
) -> Result<DesktopRunDetail> {
    let requested_task_id = query
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let requested_run_id = query
        .run_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    if requested_task_id.is_none() && requested_run_id.is_none() {
        return Err(anyhow::anyhow!("readRunDetail requires task_id or run_id"));
    }

    let task_id = if let Some(task_id) = requested_task_id.clone() {
        task_id
    } else if let Some(run_id) = requested_run_id.as_deref() {
        if let Some(task_id) =
            sqlx::query_scalar::<_, String>(r#"SELECT task_id FROM runs WHERE id = ? LIMIT 1"#)
                .bind(run_id)
                .fetch_optional(db)
                .await?
        {
            task_id
        } else {
            run_id.to_string()
        }
    } else {
        return Err(anyhow::anyhow!("unable to resolve run detail anchor"));
    };

    let task_row = sqlx::query(
        r#"SELECT
               t.id, t.kind, t.status, t.manual_gate_request_id, mg.status AS manual_gate_status,
               t.result_json, t.error_message, t.created_at, t.started_at, t.finished_at
           FROM tasks t
           LEFT JOIN manual_gate_requests mg ON mg.id = t.manual_gate_request_id
           WHERE t.id = ?"#,
    )
    .bind(&task_id)
    .fetch_optional(db)
    .await?;
    let Some(task_row) = task_row else {
        return Err(anyhow::anyhow!("task not found for run detail: {task_id}"));
    };

    let run_row = if let Some(run_id) = requested_run_id.as_deref() {
        sqlx::query(
            r#"SELECT id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message, result_json
               FROM runs
               WHERE id = ?
               LIMIT 1"#,
        )
        .bind(run_id)
        .fetch_optional(db)
        .await?
    } else {
        sqlx::query(
            r#"SELECT id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message, result_json
               FROM runs
               WHERE task_id = ?
               ORDER BY attempt DESC, id DESC
               LIMIT 1"#,
        )
        .bind(&task_id)
        .fetch_optional(db)
        .await?
    };

    let task_result =
        parse_result_value(task_row.get::<Option<String>, _>("result_json").as_deref());
    let task_error = task_row.get::<Option<String>, _>("error_message");
    let task_status: String = task_row.get("status");
    let manual_gate_request_id: Option<String> = task_row.get("manual_gate_request_id");
    let manual_gate_status: Option<String> = task_row.get("manual_gate_status");
    let task_created_at: String = task_row.get("created_at");
    let task_started_at: Option<String> = task_row.get("started_at");
    let task_finished_at: Option<String> = task_row.get("finished_at");
    let task_kind: String = task_row.get("kind");

    let run_id = run_row
        .as_ref()
        .map(|row| row.get::<String, _>("id"))
        .or_else(|| requested_run_id.clone())
        .unwrap_or_else(|| task_id.clone());
    let run_status = run_row
        .as_ref()
        .map(|row| row.get::<String, _>("status"))
        .unwrap_or_else(|| task_status.clone());
    let run_result = run_row
        .as_ref()
        .and_then(|row| parse_result_value(row.get::<Option<String>, _>("result_json").as_deref()));
    let run_error = run_row
        .as_ref()
        .and_then(|row| row.get::<Option<String>, _>("error_message"));
    let latest_run_started_at = run_row
        .as_ref()
        .and_then(|row| row.get::<Option<String>, _>("started_at"));
    let latest_run_finished_at = run_row
        .as_ref()
        .and_then(|row| row.get::<Option<String>, _>("finished_at"));

    let artifact_rows = sqlx::query(
        r#"SELECT id, kind, storage_path, metadata_json, created_at
           FROM artifacts
           WHERE task_id = ?
           ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
           LIMIT 20"#,
    )
    .bind(&task_id)
    .fetch_all(db)
    .await?;
    let artifacts = artifact_rows
        .into_iter()
        .map(|row| {
            let metadata =
                parse_result_value(row.get::<Option<String>, _>("metadata_json").as_deref());
            let kind: String = row.get("kind");
            DesktopRunArtifact {
                id: row.get("id"),
                label: metadata
                    .as_ref()
                    .and_then(|value| pick_string(value, &[&["label"], &["title"], &["kind"]]))
                    .unwrap_or_else(|| kind.clone()),
                path: row.get("storage_path"),
                status: metadata
                    .as_ref()
                    .and_then(|value| pick_string(value, &[&["status"]])),
                created_at: row.get("created_at"),
            }
        })
        .collect::<Vec<_>>();

    let log_rows = sqlx::query_as::<_, (String, Option<String>, String, String, String)>(
        r#"SELECT id, run_id, level, message, created_at
           FROM logs
           WHERE task_id = ?
           ORDER BY CAST(created_at AS INTEGER) ASC, id ASC
           LIMIT 40"#,
    )
    .bind(&task_id)
    .fetch_all(db)
    .await?;

    let log_count = log_rows.len() as i64;
    let mut timeline = Vec::new();
    push_timeline_entry(
        &mut timeline,
        format!("task-created-{task_id}"),
        "Task created",
        task_status.clone(),
        Some(format!("{} queued into desktop automation lane", task_kind)),
        Some(task_created_at.clone()),
    );
    if let Some(queued_at) = task_started_at.clone() {
        push_timeline_entry(
            &mut timeline,
            format!("task-started-{task_id}"),
            "Task started",
            "running",
            None,
            Some(queued_at),
        );
    }
    if let Some(run_row) = run_row.as_ref() {
        let attempt: i64 = run_row.get("attempt");
        let runner_kind: String = run_row.get("runner_kind");
        push_timeline_entry(
            &mut timeline,
            format!("run-{run_id}-attempt-{attempt}"),
            format!("Run attempt {attempt}"),
            run_status.clone(),
            Some(format!("runner={runner_kind}")),
            latest_run_started_at.clone(),
        );
        if latest_run_finished_at.is_some() {
            push_timeline_entry(
                &mut timeline,
                format!("run-{run_id}-finished"),
                "Run finished",
                run_status.clone(),
                run_error
                    .clone()
                    .or_else(|| pick_message_from_json(run_result.as_ref())),
                latest_run_finished_at.clone(),
            );
        }
    }
    if let Some(finished_at) = task_finished_at.clone() {
        push_timeline_entry(
            &mut timeline,
            format!("task-finished-{task_id}"),
            "Task finished",
            task_status.clone(),
            task_error
                .clone()
                .or_else(|| pick_message_from_json(task_result.as_ref())),
            Some(finished_at),
        );
    }
    if !artifacts.is_empty() {
        push_timeline_entry(
            &mut timeline,
            format!("run-artifacts-{run_id}"),
            "Run artifacts indexed",
            "ready",
            Some(format!("{} artifacts linked to this run", artifacts.len())),
            latest_run_finished_at
                .clone()
                .or(task_finished_at.clone())
                .or(latest_run_started_at.clone())
                .or(task_started_at.clone())
                .or(Some(task_created_at.clone())),
        );
    }
    if !log_rows.is_empty() {
        push_timeline_entry(
            &mut timeline,
            format!("run-logs-{run_id}"),
            "Run logs indexed",
            "ready",
            Some(format!("{} logs captured for this run", log_rows.len())),
            latest_run_finished_at
                .clone()
                .or(task_finished_at.clone())
                .or(latest_run_started_at.clone())
                .or(task_started_at.clone())
                .or(Some(task_created_at.clone())),
        );
    }
    for (id, log_run_id, level, message, created_at) in &log_rows {
        let label = if let Some(log_run_id) = log_run_id {
            format!("Log {level} ({log_run_id})")
        } else {
            format!("Log {level}")
        };
        push_timeline_entry(
            &mut timeline,
            id.clone(),
            label,
            level.to_ascii_lowercase(),
            Some(message.clone()),
            Some(created_at.clone()),
        );
    }
    timeline.sort_by(|left, right| left.created_at.cmp(&right.created_at));
    let run_attempt = run_row.as_ref().map(|row| row.get("attempt"));
    let runner_kind = run_row
        .as_ref()
        .map(|row| row.get::<String, _>("runner_kind"));
    let artifact_count = artifacts.len() as i64;
    let timeline_count = timeline.len() as i64;
    let summary = DesktopRunDetailSummary {
        task_status: task_status.clone(),
        run_status: run_status.clone(),
        run_attempt,
        runner_kind: runner_kind.clone(),
        artifact_count,
        log_count,
        timeline_count,
    };

    let message = pick_message_from_json(run_result.as_ref())
        .or_else(|| pick_message_from_json(task_result.as_ref()))
        .or(run_error.clone())
        .or(task_error.clone());
    let headline = task_result
        .as_ref()
        .and_then(|value| pick_string(value, &[&["title"], &["template_name"], &["headline"]]))
        .or_else(|| {
            run_result
                .as_ref()
                .and_then(|value| pick_string(value, &[&["title"], &["headline"]]))
        })
        .unwrap_or_else(|| format!("{} / {}", task_kind, task_id));
    let updated_at_label = latest_run_finished_at
        .clone()
        .or(task_finished_at.clone())
        .or(latest_run_started_at.clone())
        .or(task_started_at.clone())
        .or(Some(task_created_at.clone()));

    Ok(DesktopRunDetail {
        run_id,
        task_id: Some(task_id.clone()),
        status: run_status.clone(),
        headline,
        message: message.clone(),
        failure_reason: if matches!(
            run_status.as_str(),
            "failed" | "timed_out" | "cancelled" | "blocked"
        ) {
            run_error.or(task_error)
        } else {
            None
        },
        manual_gate_request_id,
        manual_gate_status,
        updated_at_label,
        created_at_label: Some(task_created_at),
        task_status: task_status.clone(),
        run_attempt,
        runner_kind,
        artifact_count,
        log_count,
        timeline_count,
        artifacts,
        timeline,
        summary: summary.clone(),
        raw: Some(serde_json::json!({
            "taskStatus": task_status.clone(),
            "taskResult": task_result,
            "runResult": run_result,
            "message": message,
            "summary": summary,
        })),
    })
}

pub async fn retry_desktop_task(db: &DbPool, task_id: &str) -> Result<DesktopTaskWriteResult> {
    let Some(status) = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(task_id)
        .fetch_optional(db)
        .await?
    else {
        return Err(anyhow::anyhow!("task not found: {task_id}"));
    };

    if status == "queued" {
        return Ok(DesktopTaskWriteResult {
            task_id: task_id.to_string(),
            status,
            message: "task already queued; retry treated as idempotent".to_string(),
            updated_at: now_ts_string(),
            run_id: latest_run_id_for_task(db, task_id).await?,
            manual_gate_request_id: None,
        });
    }

    if !matches!(status.as_str(), "failed" | "timed_out") {
        return Err(anyhow::anyhow!(
            "task status does not allow retry now: {status}"
        ));
    }

    let queued_at = now_ts_string();
    let rows_affected = sqlx::query(
        r#"UPDATE tasks
           SET status = 'queued', queued_at = ?, started_at = NULL, finished_at = NULL,
               runner_id = NULL, heartbeat_at = NULL, result_json = NULL, error_message = NULL
           WHERE id = ? AND status IN ('failed', 'timed_out')"#,
    )
    .bind(&queued_at)
    .bind(task_id)
    .execute(db)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        return Err(anyhow::anyhow!(
            "task status changed before retry could be applied: {task_id}"
        ));
    }

    insert_desktop_task_log(
        db,
        task_id,
        None,
        "INFO",
        "task re-queued for retry",
        &queued_at,
    )
    .await?;

    Ok(DesktopTaskWriteResult {
        task_id: task_id.to_string(),
        status: "queued".to_string(),
        message: "task re-queued for retry".to_string(),
        updated_at: queued_at,
        run_id: latest_run_id_for_task(db, task_id).await?,
        manual_gate_request_id: None,
    })
}

pub async fn cancel_desktop_task(
    db: &DbPool,
    runner: &dyn TaskRunner,
    task_id: &str,
) -> Result<DesktopTaskWriteResult> {
    let Some(status) = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(task_id)
        .fetch_optional(db)
        .await?
    else {
        return Err(anyhow::anyhow!("task not found: {task_id}"));
    };

    let updated_at = now_ts_string();
    let latest_run_id = latest_run_id_for_task(db, task_id).await?;

    if status == "queued" {
        sqlx::query(
            r#"UPDATE tasks
               SET status = 'cancelled', finished_at = ?, runner_id = NULL, heartbeat_at = NULL,
                   error_message = ?
               WHERE id = ?"#,
        )
        .bind(&updated_at)
        .bind("task cancelled while queued")
        .bind(task_id)
        .execute(db)
        .await?;

        insert_desktop_task_log(
            db,
            task_id,
            None,
            "WARN",
            "task cancelled while queued",
            &updated_at,
        )
        .await?;

        return Ok(DesktopTaskWriteResult {
            task_id: task_id.to_string(),
            status: "cancelled".to_string(),
            message: "task cancelled while queued".to_string(),
            updated_at,
            run_id: latest_run_id,
            manual_gate_request_id: None,
        });
    }

    if status == "running" {
        let cancel_result = runner.cancel_running(task_id).await;
        if !cancel_result.accepted {
            return Err(anyhow::anyhow!(
                "running task cancel was not accepted by runner {}: {}",
                runner.name(),
                cancel_result.message
            ));
        }

        let cancel_result_json = serde_json::json!({
            "runner": runner.name(),
            "ok": false,
            "status": "cancelled",
            "error_kind": "runner_cancelled",
            "failure_scope": "runner_cancelled",
            "execution_stage": "action",
            "task_id": task_id,
            "message": "task cancelled while running",
        })
        .to_string();
        let cancel_message = format!("task cancelled while running; {}", cancel_result.message);

        sqlx::query(
            r#"UPDATE tasks
               SET status = 'cancelled', finished_at = ?, runner_id = NULL, heartbeat_at = NULL,
                   result_json = COALESCE(result_json, ?), error_message = ?
               WHERE id = ?"#,
        )
        .bind(&updated_at)
        .bind(&cancel_result_json)
        .bind(&cancel_message)
        .bind(task_id)
        .execute(db)
        .await?;

        if let Some(run_id) = latest_run_id.as_deref() {
            sqlx::query(
                r#"UPDATE runs
                   SET status = 'cancelled', finished_at = ?, error_message = ?,
                       result_json = COALESCE(result_json, ?)
                   WHERE id = ? AND status = 'running'"#,
            )
            .bind(&updated_at)
            .bind(&cancel_message)
            .bind(&cancel_result_json)
            .bind(run_id)
            .execute(db)
            .await?;
        }

        insert_desktop_task_log(
            db,
            task_id,
            latest_run_id.as_deref(),
            "WARN",
            &cancel_message,
            &updated_at,
        )
        .await?;

        return Ok(DesktopTaskWriteResult {
            task_id: task_id.to_string(),
            status: "cancelled".to_string(),
            message: cancel_message,
            updated_at,
            run_id: latest_run_id,
            manual_gate_request_id: None,
        });
    }

    Err(anyhow::anyhow!(
        "task status does not allow cancel: {status}"
    ))
}

pub async fn confirm_desktop_manual_gate(
    db: &DbPool,
    request: DesktopManualGateActionRequest,
) -> Result<DesktopTaskWriteResult> {
    let gate_id = request.manual_gate_request_id.trim().to_string();
    if gate_id.is_empty() {
        return Err(anyhow::anyhow!("manual_gate_request_id is required"));
    }

    let Some((task_id, gate_status)) = sqlx::query_as::<_, (String, String)>(
        r#"SELECT task_id, status FROM manual_gate_requests WHERE id = ?"#,
    )
    .bind(&gate_id)
    .fetch_optional(db)
    .await?
    else {
        return Err(anyhow::anyhow!("manual gate not found: {gate_id}"));
    };

    if gate_status == "confirmed" {
        return Ok(DesktopTaskWriteResult {
            task_id,
            status: "confirmed".to_string(),
            message: "manual gate already confirmed".to_string(),
            updated_at: now_ts_string(),
            run_id: None,
            manual_gate_request_id: Some(gate_id),
        });
    }
    if gate_status != "pending" {
        return Err(anyhow::anyhow!(
            "manual gate cannot be confirmed from status: {gate_status}"
        ));
    }

    let now = now_ts_string();
    sqlx::query(
        r#"UPDATE manual_gate_requests
           SET status = 'confirmed', resolution_note = ?, updated_at = ?, resolved_at = ?
           WHERE id = ? AND status = 'pending'"#,
    )
    .bind(&request.note)
    .bind(&now)
    .bind(&now)
    .bind(&gate_id)
    .execute(db)
    .await?;
    sqlx::query(
        r#"UPDATE tasks
           SET status = 'queued', queued_at = ?, finished_at = NULL, error_message = NULL
           WHERE id = ? AND status = 'pending'"#,
    )
    .bind(&now)
    .bind(&task_id)
    .execute(db)
    .await?;
    insert_desktop_task_log(
        db,
        &task_id,
        None,
        "INFO",
        "manual gate confirmed; task queued for execution",
        &now,
    )
    .await?;

    Ok(DesktopTaskWriteResult {
        task_id,
        status: "confirmed".to_string(),
        message: "manual gate confirmed; task queued for execution".to_string(),
        updated_at: now,
        run_id: None,
        manual_gate_request_id: Some(gate_id),
    })
}

pub async fn reject_desktop_manual_gate(
    db: &DbPool,
    request: DesktopManualGateActionRequest,
) -> Result<DesktopTaskWriteResult> {
    let gate_id = request.manual_gate_request_id.trim().to_string();
    if gate_id.is_empty() {
        return Err(anyhow::anyhow!("manual_gate_request_id is required"));
    }

    let Some((task_id, gate_status)) = sqlx::query_as::<_, (String, String)>(
        r#"SELECT task_id, status FROM manual_gate_requests WHERE id = ?"#,
    )
    .bind(&gate_id)
    .fetch_optional(db)
    .await?
    else {
        return Err(anyhow::anyhow!("manual gate not found: {gate_id}"));
    };

    if gate_status == "rejected" {
        return Ok(DesktopTaskWriteResult {
            task_id,
            status: "rejected".to_string(),
            message: "manual gate already rejected".to_string(),
            updated_at: now_ts_string(),
            run_id: None,
            manual_gate_request_id: Some(gate_id),
        });
    }
    if gate_status != "pending" {
        return Err(anyhow::anyhow!(
            "manual gate cannot be rejected from status: {gate_status}"
        ));
    }

    let now = now_ts_string();
    let rejection_message = request
        .note
        .clone()
        .unwrap_or_else(|| "manual gate rejected".to_string());
    sqlx::query(
        r#"UPDATE manual_gate_requests
           SET status = 'rejected', resolution_note = ?, updated_at = ?, resolved_at = ?
           WHERE id = ? AND status = 'pending'"#,
    )
    .bind(&request.note)
    .bind(&now)
    .bind(&now)
    .bind(&gate_id)
    .execute(db)
    .await?;
    sqlx::query(
        r#"UPDATE tasks
           SET status = 'cancelled', queued_at = NULL, finished_at = ?,
               result_json = COALESCE(result_json, ?), error_message = ?
           WHERE id = ? AND status = 'pending'"#,
    )
    .bind(&now)
    .bind(
        serde_json::json!({
            "status": "cancelled",
            "error_kind": "manual_gate_rejected",
            "failure_scope": "manual_gate",
            "message": rejection_message,
            "manual_gate_request_id": gate_id,
        })
        .to_string(),
    )
    .bind(&rejection_message)
    .bind(&task_id)
    .execute(db)
    .await?;
    insert_desktop_task_log(
        db,
        &task_id,
        None,
        "WARN",
        "manual gate rejected; task cancelled",
        &now,
    )
    .await?;

    Ok(DesktopTaskWriteResult {
        task_id,
        status: "rejected".to_string(),
        message: "manual gate rejected; task cancelled".to_string(),
        updated_at: now,
        run_id: None,
        manual_gate_request_id: Some(request.manual_gate_request_id),
    })
}

pub async fn change_desktop_proxy_ip(
    db: &DbPool,
    request: DesktopProxyChangeIpRequest,
) -> Result<DesktopProxyChangeIpResult> {
    let proxy_id = request.proxy_id.trim().to_string();
    if proxy_id.is_empty() {
        return Err(anyhow::anyhow!("proxy_id is required"));
    }
    let proxy_row = sqlx::query(
        r#"SELECT id, provider, region, status, source_label
           FROM proxies
           WHERE id = ?"#,
    )
    .bind(&proxy_id)
    .fetch_optional(db)
    .await?;
    let Some(proxy_row) = proxy_row else {
        return Err(anyhow::anyhow!("proxy not found: {proxy_id}"));
    };

    let proxy_provider: Option<String> = proxy_row.get("provider");
    let proxy_region: Option<String> = proxy_row.get("region");
    let proxy_source_label: Option<String> = proxy_row.get("source_label");
    let proxy_status: String = proxy_row.get("status");
    let requested_mode = normalized_optional_text(request.mode.clone());
    let requested_session_key = normalized_optional_text(request.session_key.clone());
    let requested_provider = normalized_optional_text(request.requested_provider.clone());
    let requested_region = normalized_optional_text(request.requested_region.clone());
    let requested_sticky_ttl_seconds = request.sticky_ttl_seconds.filter(|ttl| *ttl > 0);

    let binding_row = if let Some(session_key) = requested_session_key.as_ref() {
        sqlx::query(
            r#"SELECT
                   session_key,
                   provider,
                   region,
                   requested_provider,
                   requested_region,
                   expires_at
               FROM proxy_session_bindings
               WHERE proxy_id = ? AND session_key = ?
               LIMIT 1"#,
        )
        .bind(&proxy_id)
        .bind(session_key)
        .fetch_optional(db)
        .await?
    } else {
        sqlx::query(
            r#"SELECT
                   session_key,
                   provider,
                   region,
                   requested_provider,
                   requested_region,
                   expires_at
               FROM proxy_session_bindings
               WHERE proxy_id = ?
               ORDER BY
                 CASE
                   WHEN expires_at IS NULL THEN 1
                   WHEN CAST(expires_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER) THEN 0
                   ELSE 2
                 END ASC,
                 CAST(last_used_at AS INTEGER) DESC,
                 session_key DESC
               LIMIT 1"#,
        )
        .bind(&proxy_id)
        .fetch_optional(db)
        .await?
    };

    let mut binding_session_key: Option<String> = None;
    let mut binding_provider: Option<String> = None;
    let mut binding_region: Option<String> = None;
    let mut binding_requested_provider: Option<String> = None;
    let mut binding_requested_region: Option<String> = None;
    let mut binding_expires_at: Option<String> = None;

    if let Some(row) = binding_row {
        binding_session_key = row.get("session_key");
        binding_provider = row.get("provider");
        binding_region = row.get("region");
        binding_requested_provider = row.get("requested_provider");
        binding_requested_region = row.get("requested_region");
        binding_expires_at = row.get("expires_at");
    }

    let updated_at = now_ts_string();
    let now_ts = updated_at.parse::<i64>().unwrap_or(0);
    let session_key = requested_session_key.or(binding_session_key);
    let requested_provider = requested_provider
        .or(binding_requested_provider)
        .or(binding_provider)
        .or_else(|| proxy_provider.clone());
    let requested_region = requested_region
        .or(binding_requested_region)
        .or(binding_region)
        .or_else(|| proxy_region.clone());
    let expires_at = if session_key.is_some() {
        requested_sticky_ttl_seconds
            .map(|ttl| (now_ts + ttl).to_string())
            .or(binding_expires_at)
    } else {
        None
    };

    let residency_status = build_proxy_residency_status(
        session_key.as_deref(),
        expires_at.as_deref(),
        Some(proxy_status.as_str()),
        requested_provider.as_deref(),
        proxy_provider.as_deref(),
        requested_region.as_deref(),
        proxy_region.as_deref(),
        now_ts,
    );
    let rotation_mode = build_proxy_rotation_mode(
        requested_mode.as_deref(),
        session_key.as_deref(),
        &residency_status,
        requested_provider.as_deref(),
        requested_region.as_deref(),
    );
    let sticky_ttl_seconds = requested_sticky_ttl_seconds
        .or_else(|| sticky_ttl_seconds_from_expires_at(expires_at.as_deref(), now_ts));
    let task_id = format!("desktop-proxy-change-{}", Uuid::new_v4());
    let note = format!(
        "Prepared {rotation_mode} with residency {residency_status}; session={}, requested provider={}, requested region={}.",
        session_key.as_deref().unwrap_or("none"),
        requested_provider.as_deref().unwrap_or("inherit"),
        requested_region.as_deref().unwrap_or("inherit"),
    );

    let input_json = serde_json::json!({
        "proxy_id": proxy_id.clone(),
        "action": "change_proxy_ip",
        "source": "desktop_workbench",
        "mode": rotation_mode.clone(),
        "session_key": session_key.clone(),
        "requested_provider": requested_provider.clone(),
        "requested_region": requested_region.clone(),
        "sticky_ttl_seconds": sticky_ttl_seconds,
        "residency_status": residency_status.clone(),
        "rotation_mode": rotation_mode.clone(),
        "expires_at": expires_at.clone(),
        "note": note.clone(),
        "proxy_source_label": proxy_source_label.clone(),
    });
    let build_failed_result_json =
        |error_kind: &str,
         rollback_signal: &str,
         failure_message: &str,
         source_label: Option<&str>,
         status_code: Option<u16>,
         response_excerpt: Option<&String>| {
            serde_json::json!({
                "proxyId": proxy_id.clone(),
                "status": "failed_provider_rotation",
                "mode": rotation_mode.clone(),
                "sessionKey": session_key.clone(),
                "requestedProvider": requested_provider.clone(),
                "requestedRegion": requested_region.clone(),
                "stickyTtlSeconds": sticky_ttl_seconds,
                "note": note.clone(),
                "residencyStatus": residency_status.clone(),
                "rotationMode": rotation_mode.clone(),
                "trackingTaskId": task_id.clone(),
                "expiresAt": expires_at.clone(),
                "updatedAt": updated_at.clone(),
                "executionStatus": "failed",
                "errorKind": error_kind,
                "rollbackSignal": rollback_signal,
                "message": failure_message,
                "providerRefresh": {
                    "sourceLabel": source_label,
                    "providerKey": Value::Null,
                    "statusCode": status_code,
                    "providerRequestId": Value::Null,
                    "responseExcerpt": response_excerpt.cloned(),
                },
            })
        };

    let provider_key = requested_provider
        .clone()
        .or_else(|| normalized_optional_text(proxy_provider.clone()))
        .or_else(|| normalized_optional_text(proxy_source_label.clone()));
    let Some(provider_key) = provider_key else {
        let failure_message =
            "provider refresh cannot run because provider/source identity is missing from request and proxy record";
        let failure_result_json = build_failed_result_json(
            "provider_missing",
            "binding_not_applied",
            failure_message,
            None,
            None,
            None,
        );
        insert_change_proxy_ip_task(
            db,
            &task_id,
            "failed",
            &input_json,
            &failure_result_json,
            Some(failure_message),
            &updated_at,
            None,
        )
        .await?;
        let _ =
            insert_desktop_task_log(db, &task_id, None, "WARN", failure_message, &updated_at).await;
        return Err(anyhow::anyhow!(failure_message));
    };

    let mut template_variables = BTreeMap::<String, String>::new();
    template_variables.insert("proxy_id".to_string(), proxy_id.clone());
    template_variables.insert("provider".to_string(), provider_key.clone());
    template_variables.insert("mode".to_string(), rotation_mode.clone());
    template_variables.insert("rotation_mode".to_string(), rotation_mode.clone());
    template_variables.insert("residency_status".to_string(), residency_status.clone());
    template_variables.insert("now_ts".to_string(), updated_at.clone());
    if let Some(value) = session_key.as_deref() {
        template_variables.insert("session_key".to_string(), value.to_string());
    }
    if let Some(value) = requested_provider.as_deref() {
        template_variables.insert("requested_provider".to_string(), value.to_string());
    }
    if let Some(value) = requested_region.as_deref() {
        template_variables.insert("requested_region".to_string(), value.to_string());
    }
    if let Some(value) = proxy_region.as_deref().and_then(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then_some(trimmed.to_string())
    }) {
        template_variables.insert("proxy_region".to_string(), value);
    }
    if let Some(value) = proxy_source_label.as_deref().and_then(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then_some(trimmed.to_string())
    }) {
        template_variables.insert("proxy_source_label".to_string(), value);
    }

    let refresh_spec = match resolve_provider_refresh_spec(
        db,
        &normalize_lookup_key(&provider_key),
        proxy_source_label.as_deref(),
        &template_variables,
    )
    .await
    {
        Ok(spec) => spec,
        Err(failure) => {
            let failure_result_json = build_failed_result_json(
                &failure.error_kind,
                &failure.rollback_signal,
                &failure.message,
                failure.source_label.as_deref(),
                failure.status_code,
                failure.response_excerpt.as_ref(),
            );
            insert_change_proxy_ip_task(
                db,
                &task_id,
                "failed",
                &input_json,
                &failure_result_json,
                Some(&failure.message),
                &updated_at,
                None,
            )
            .await?;
            let _ =
                insert_desktop_task_log(db, &task_id, None, "WARN", &failure.message, &updated_at)
                    .await;
            return Err(anyhow::anyhow!(failure.message));
        }
    };

    let refresh_result = match execute_provider_refresh(&refresh_spec).await {
        Ok(result) => result,
        Err(failure) => {
            let failure_result_json = build_failed_result_json(
                &failure.error_kind,
                &failure.rollback_signal,
                &failure.message,
                failure.source_label.as_deref(),
                failure.status_code,
                failure.response_excerpt.as_ref(),
            );
            insert_change_proxy_ip_task(
                db,
                &task_id,
                "failed",
                &input_json,
                &failure_result_json,
                Some(&failure.message),
                &updated_at,
                None,
            )
            .await?;
            let _ =
                insert_desktop_task_log(db, &task_id, None, "WARN", &failure.message, &updated_at)
                    .await;
            return Err(anyhow::anyhow!(failure.message));
        }
    };

    if let Some(session_key_value) = session_key.as_deref() {
        if let Err(err) = sqlx::query(
            r#"INSERT INTO proxy_session_bindings (
                   session_key, proxy_id, provider, region, requested_region, requested_provider,
                   last_used_at, expires_at, created_at, updated_at
               ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(session_key) DO UPDATE SET
                 proxy_id = excluded.proxy_id,
                 provider = COALESCE(excluded.provider, proxy_session_bindings.provider),
                 region = COALESCE(excluded.region, proxy_session_bindings.region),
                 requested_region = COALESCE(excluded.requested_region, proxy_session_bindings.requested_region),
                 requested_provider = COALESCE(excluded.requested_provider, proxy_session_bindings.requested_provider),
                 last_used_at = excluded.last_used_at,
                 expires_at = COALESCE(excluded.expires_at, proxy_session_bindings.expires_at),
                 updated_at = excluded.updated_at"#,
        )
        .bind(session_key_value)
        .bind(&proxy_id)
        .bind(requested_provider.as_deref().or(proxy_provider.as_deref()))
        .bind(requested_region.as_deref().or(proxy_region.as_deref()))
        .bind(&requested_region)
        .bind(&requested_provider)
        .bind(&updated_at)
        .bind(&expires_at)
        .bind(&updated_at)
        .bind(&updated_at)
        .execute(db)
        .await
        {
            let failure_message = format!(
                "provider refresh succeeded, but local session binding update failed: {err}"
            );
            let failure_result_json = build_failed_result_json(
                "local_binding_write_failed",
                "provider_refreshed_manual_rebind_required",
                &failure_message,
                Some(&refresh_result.source_label),
                Some(refresh_result.status_code),
                refresh_result.response_excerpt.as_ref(),
            );
            insert_change_proxy_ip_task(
                db,
                &task_id,
                "failed",
                &input_json,
                &failure_result_json,
                Some(&failure_message),
                &updated_at,
                None,
            )
            .await?;
            let _ = insert_desktop_task_log(
                db,
                &task_id,
                None,
                "WARN",
                &failure_message,
                &updated_at,
            )
            .await;
            return Err(anyhow::anyhow!(failure_message));
        }
    }

    if let Err(err) = sqlx::query(r#"UPDATE proxies SET updated_at = ? WHERE id = ?"#)
        .bind(&updated_at)
        .bind(&proxy_id)
        .execute(db)
        .await
    {
        let failure_message =
            format!("provider refresh succeeded, but local proxy timestamp update failed: {err}");
        let failure_result_json = build_failed_result_json(
            "local_proxy_update_failed",
            "provider_refreshed_manual_rebind_required",
            &failure_message,
            Some(&refresh_result.source_label),
            Some(refresh_result.status_code),
            refresh_result.response_excerpt.as_ref(),
        );
        insert_change_proxy_ip_task(
            db,
            &task_id,
            "failed",
            &input_json,
            &failure_result_json,
            Some(&failure_message),
            &updated_at,
            None,
        )
        .await?;
        let _ = insert_desktop_task_log(db, &task_id, None, "WARN", &failure_message, &updated_at)
            .await;
        return Err(anyhow::anyhow!(failure_message));
    }

    let status = if rotation_mode.contains("sticky") {
        "accepted_sticky_rotation".to_string()
    } else {
        "accepted_provider_rotation".to_string()
    };
    let provider_request_suffix = refresh_result
        .provider_request_id
        .as_deref()
        .map(|request_id| format!(" provider_request_id={request_id}."))
        .unwrap_or_default();
    let message = format!(
        "Provider-side write accepted via source '{}' (provider '{}', http_status={}). Exit-IP drift still needs later detail/health refresh observation.{}",
        refresh_result.source_label,
        refresh_result.provider_key,
        refresh_result.status_code,
        provider_request_suffix
    );
    let success_result_json = serde_json::json!({
        "proxyId": proxy_id.clone(),
        "status": status.clone(),
        "mode": rotation_mode.clone(),
        "sessionKey": session_key.clone(),
        "requestedProvider": requested_provider.clone(),
        "requestedRegion": requested_region.clone(),
        "stickyTtlSeconds": sticky_ttl_seconds,
        "note": note.clone(),
        "residencyStatus": residency_status.clone(),
        "rotationMode": rotation_mode.clone(),
        "trackingTaskId": task_id.clone(),
        "expiresAt": expires_at.clone(),
        "updatedAt": updated_at.clone(),
        "executionStatus": "accepted",
        "errorKind": Value::Null,
        "rollbackSignal": "none",
        "message": message.clone(),
        "providerRefresh": {
            "sourceLabel": refresh_result.source_label.clone(),
            "providerKey": refresh_result.provider_key.clone(),
            "statusCode": refresh_result.status_code,
            "providerRequestId": refresh_result.provider_request_id.clone(),
            "responseExcerpt": refresh_result.response_excerpt.clone(),
        },
    });

    insert_change_proxy_ip_task(
        db,
        &task_id,
        "succeeded",
        &input_json,
        &success_result_json,
        None,
        &updated_at,
        Some(&updated_at),
    )
    .await?;
    let _ = insert_desktop_task_log(db, &task_id, None, "INFO", &message, &updated_at).await;

    Ok(DesktopProxyChangeIpResult {
        proxy_id,
        status,
        mode: rotation_mode.clone(),
        session_key,
        requested_provider,
        requested_region,
        sticky_ttl_seconds,
        note,
        residency_status,
        rotation_mode,
        tracking_task_id: task_id,
        expires_at,
        updated_at,
        message,
        execution_status: "accepted".to_string(),
        rollback_signal: "none".to_string(),
        error_kind: None,
        provider_refresh: DesktopProxyChangeIpProviderRefresh {
            source_label: refresh_result.source_label,
            provider_key: refresh_result.provider_key,
            status_code: refresh_result.status_code,
            provider_request_id: refresh_result.provider_request_id,
            response_excerpt: refresh_result.response_excerpt,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db::init::init_db;
    use axum::{routing::post, Json, Router};
    use std::sync::Arc;
    use tokio::net::TcpListener;

    async fn seed_launch_test_fixtures(db: &DbPool) {
        sqlx::query(
            r#"INSERT INTO fingerprint_profiles (
                   id, name, version, status, tags_json, profile_json, created_at, updated_at
               ) VALUES (
                   'fp-launch-test', 'Launch Test Fingerprint', 1, 'active', NULL,
                   '{"browser":{"name":"chrome","version":"125"}}', '2000000000', '2000000000'
               )"#,
        )
        .execute(db)
        .await
        .expect("seed fingerprint profile");

        sqlx::query(
            r#"INSERT INTO network_policies (
                   id, name, country_anchor, region_anchor, allow_same_country_fallback,
                   allow_same_region_fallback, provider_preference, allowed_regions_json,
                   network_policy_json, status, created_at, updated_at
               ) VALUES (
                   'np-launch-test', 'Launch Test Network', 'US', 'us-east', 1, 1, NULL, NULL,
                   '{"mode":"required_proxy"}', 'active', '2000000000', '2000000000'
               )"#,
        )
        .execute(db)
        .await
        .expect("seed network policy");

        sqlx::query(
            r#"INSERT INTO continuity_policies (
                   id, name, session_ttl_seconds, heartbeat_interval_seconds, site_group_mode,
                   recovery_enabled, protect_on_login_loss, policy_json, status, created_at, updated_at
               ) VALUES (
                   'cp-launch-test', 'Launch Test Continuity', 86400, 300, 'host', 1, 1, NULL,
                   'active', '2000000000', '2000000000'
               )"#,
        )
        .execute(db)
        .await
        .expect("seed continuity policy");

        sqlx::query(
            r#"INSERT INTO platform_templates (
                   id, platform_id, name, warm_paths_json, revisit_paths_json, stateful_paths_json,
                   write_operation_paths_json, high_risk_paths_json, readiness_level, status,
                   created_at, updated_at, event_chain_templates_json
               ) VALUES (
                   'tpl-launch-test', 'platform-launch-test', 'Launch Test Template',
                   '[]', '[]', '[]', '[]', '[]', 'ready', 'active',
                   '2000000000', '2000000000', '{"steps":[{"action":"open"}]}'
               )"#,
        )
        .execute(db)
        .await
        .expect("seed template");

        sqlx::query(
            r#"INSERT INTO persona_profiles (
                   id, store_id, platform_id, country_anchor, region_anchor, locale, timezone,
                   fingerprint_profile_id, behavior_profile_id, network_policy_id, continuity_policy_id,
                   status, created_at, updated_at
               ) VALUES (
                   'persona-launch-test', 'store-launch-test', 'platform-launch-test', 'US', 'us-east',
                   'en-US', 'Asia/Shanghai', 'fp-launch-test', NULL, 'np-launch-test',
                   'cp-launch-test', 'active', '2000000000', '2000000000'
               )"#,
        )
        .execute(db)
        .await
        .expect("seed persona profile");
    }

    #[tokio::test]
    async fn change_proxy_ip_fails_without_provider_refresh_config_and_records_failed_task() {
        let db_url = format!(
            "sqlite:///tmp/persona_pilot_desktop_change_ip_failure_{}.db",
            Uuid::new_v4()
        );
        let db = init_db(&db_url).await.expect("init db");

        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, provider, region, status, score, success_count, failure_count, created_at, updated_at
               ) VALUES (
                   'proxy-change-ip-test', 'http', '127.0.0.1', 8080, 'pool-missing', 'us-east',
                   'active', 1.0, 0, 0, '1', '1'
               )"#,
        )
        .execute(&db)
        .await
        .expect("seed proxy");

        let result = change_desktop_proxy_ip(
            &db,
            DesktopProxyChangeIpRequest {
                proxy_id: "proxy-change-ip-test".to_string(),
                mode: None,
                session_key: None,
                requested_provider: None,
                requested_region: None,
                sticky_ttl_seconds: None,
            },
        )
        .await;

        let error_message = result
            .err()
            .map(|err| err.to_string())
            .unwrap_or_else(|| "missing error".to_string());
        assert!(
            error_message.contains("provider refresh config"),
            "expected provider refresh config error, got: {error_message}"
        );

        let task_row = sqlx::query(
            r#"SELECT status, error_message, result_json
               FROM tasks
               WHERE kind = 'change_proxy_ip'
               ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
               LIMIT 1"#,
        )
        .fetch_one(&db)
        .await
        .expect("load latest change_proxy_ip task");

        let status: String = task_row.get("status");
        let stored_error: Option<String> = task_row.get("error_message");
        let result_json_raw: String = task_row.get("result_json");
        let result_json = serde_json::from_str::<Value>(&result_json_raw)
            .expect("decode change_proxy_ip result_json");

        assert_eq!(status, "failed");
        assert!(
            stored_error
                .as_deref()
                .is_some_and(|message| message.contains("provider refresh config")),
            "expected stored failure message, got: {:?}",
            stored_error
        );
        assert_eq!(
            result_json.get("executionStatus").and_then(Value::as_str),
            Some("failed")
        );
        assert_eq!(
            result_json.get("errorKind").and_then(Value::as_str),
            Some("provider_refresh_config_missing")
        );
        assert_eq!(
            result_json.get("rollbackSignal").and_then(Value::as_str),
            Some("binding_not_applied")
        );
        assert!(
            result_json
                .get("providerRefresh")
                .and_then(|value| value.get("providerKey"))
                .is_some_and(Value::is_null),
            "expected providerRefresh.providerKey to be explicit null in failure payload"
        );
        assert!(
            result_json
                .get("providerRefresh")
                .and_then(|value| value.get("providerRequestId"))
                .is_some_and(Value::is_null),
            "expected providerRefresh.providerRequestId to be explicit null in failure payload"
        );
    }

    async fn spawn_provider_refresh_test_server(response_body: Value) -> String {
        let response_body = Arc::new(response_body);
        let app = Router::new().route(
            "/refresh",
            post({
                let response_body = Arc::clone(&response_body);
                move || {
                    let response_body = Arc::clone(&response_body);
                    async move { Json((*response_body).clone()) }
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind provider refresh test server");
        let addr = listener.local_addr().expect("provider refresh test addr");
        tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("serve provider refresh test server");
        });
        format!("http://{addr}/refresh")
    }

    #[tokio::test]
    async fn change_proxy_ip_succeeds_with_provider_refresh_config_and_records_success_task() {
        let db_url = format!(
            "sqlite:///tmp/persona_pilot_desktop_change_ip_success_{}.db",
            Uuid::new_v4()
        );
        let db = init_db(&db_url).await.expect("init db");
        let refresh_url = spawn_provider_refresh_test_server(serde_json::json!({
            "ok": true,
            "requestId": "req-123",
        }))
        .await;

        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, provider, region, source_label, status, score, success_count, failure_count, created_at, updated_at
               ) VALUES (
                   'proxy-change-ip-success', 'http', '127.0.0.1', 8080, 'pool-live', 'us-east',
                   'pool-live', 'active', 1.0, 0, 0, '1', '1'
               )"#,
        )
        .execute(&db)
        .await
        .expect("seed success proxy");

        sqlx::query(
            r#"INSERT INTO proxy_harvest_sources (
                   source_label, source_kind, enabled, config_json, interval_seconds, base_proxy_score, created_at, updated_at
               ) VALUES (?, 'json_url', 1, ?, 300, 1.0, '1', '1')"#,
        )
        .bind("pool-live")
        .bind(
            serde_json::json!({
                "provider_refresh": {
                    "enabled": true,
                    "method": "POST",
                    "url": refresh_url,
                    "success_path": "ok",
                    "success_equals": true,
                    "request_id_path": "requestId",
                }
            })
            .to_string(),
        )
        .execute(&db)
        .await
        .expect("seed success provider refresh source");

        let result = change_desktop_proxy_ip(
            &db,
            DesktopProxyChangeIpRequest {
                proxy_id: "proxy-change-ip-success".to_string(),
                mode: None,
                session_key: Some("session-success-1".to_string()),
                requested_provider: None,
                requested_region: None,
                sticky_ttl_seconds: Some(600),
            },
        )
        .await
        .expect("change proxy ip success");

        assert_eq!(result.status, "accepted_sticky_rotation");
        assert_eq!(result.tracking_task_id.is_empty(), false);
        assert_eq!(result.execution_status, "accepted");
        assert_eq!(result.rollback_signal, "none");
        assert!(result.error_kind.is_none());
        assert_eq!(result.provider_refresh.source_label, "pool-live");
        assert_eq!(result.provider_refresh.provider_key, "pool-live");
        assert_eq!(result.provider_refresh.status_code, 200);
        assert_eq!(
            result.provider_refresh.provider_request_id.as_deref(),
            Some("req-123")
        );
        assert!(
            result.provider_refresh.response_excerpt.is_some(),
            "expected provider response excerpt for accepted refresh"
        );
        assert!(
            result
                .message
                .contains("Exit-IP drift still needs later detail/health refresh observation."),
            "unexpected result message: {}",
            result.message
        );

        let task_row = sqlx::query(
            r#"SELECT status, error_message, result_json
               FROM tasks
               WHERE kind = 'change_proxy_ip'
               ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
               LIMIT 1"#,
        )
        .fetch_one(&db)
        .await
        .expect("load success change_proxy_ip task");

        let task_status: String = task_row.get("status");
        let task_error: Option<String> = task_row.get("error_message");
        let result_json_raw: String = task_row.get("result_json");
        let result_json =
            serde_json::from_str::<Value>(&result_json_raw).expect("decode success result_json");

        assert_eq!(task_status, "succeeded");
        assert!(task_error.is_none(), "unexpected task error: {:?}", task_error);
        assert_eq!(
            result_json.get("executionStatus").and_then(Value::as_str),
            Some("accepted")
        );
        assert_eq!(
            result_json.get("status").and_then(Value::as_str),
            Some("accepted_sticky_rotation")
        );
        assert_eq!(
            result_json
                .get("providerRefresh")
                .and_then(|value| value.get("providerRequestId"))
                .and_then(Value::as_str),
            Some("req-123")
        );

        let binding_row = sqlx::query(
            r#"SELECT proxy_id, requested_provider, requested_region
               FROM proxy_session_bindings
               WHERE session_key = 'session-success-1'"#,
        )
        .fetch_one(&db)
        .await
        .expect("load session binding");
        let binding_proxy_id: String = binding_row.get("proxy_id");
        let binding_provider: Option<String> = binding_row.get("requested_provider");
        let binding_region: Option<String> = binding_row.get("requested_region");
        assert_eq!(binding_proxy_id, "proxy-change-ip-success");
        assert_eq!(binding_provider.as_deref(), Some("pool-live"));
        assert_eq!(binding_region.as_deref(), Some("us-east"));

        let proxy_updated_at: String = sqlx::query_scalar(
            r#"SELECT updated_at FROM proxies WHERE id = 'proxy-change-ip-success'"#,
        )
        .fetch_one(&db)
        .await
        .expect("load updated proxy timestamp");
        assert_ne!(proxy_updated_at, "1");
    }

    #[tokio::test]
    async fn change_proxy_ip_fails_when_provider_refresh_success_check_does_not_match() {
        let db_url = format!(
            "sqlite:///tmp/persona_pilot_desktop_change_ip_success_check_{}.db",
            Uuid::new_v4()
        );
        let db = init_db(&db_url).await.expect("init db");
        let refresh_url = spawn_provider_refresh_test_server(serde_json::json!({
            "ok": false,
            "requestId": "req-bad",
        }))
        .await;

        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, provider, region, source_label, status, score, success_count, failure_count, created_at, updated_at
               ) VALUES (
                   'proxy-change-ip-check', 'http', '127.0.0.1', 8080, 'pool-check', 'us-east',
                   'pool-check', 'active', 1.0, 0, 0, '1', '1'
               )"#,
        )
        .execute(&db)
        .await
        .expect("seed failure proxy");

        sqlx::query(
            r#"INSERT INTO proxy_harvest_sources (
                   source_label, source_kind, enabled, config_json, interval_seconds, base_proxy_score, created_at, updated_at
               ) VALUES (?, 'json_url', 1, ?, 300, 1.0, '1', '1')"#,
        )
        .bind("pool-check")
        .bind(
            serde_json::json!({
                "provider_refresh": {
                    "enabled": true,
                    "method": "POST",
                    "url": refresh_url,
                    "success_path": "ok",
                    "success_equals": true,
                }
            })
            .to_string(),
        )
        .execute(&db)
        .await
        .expect("seed failure provider refresh source");

        let result = change_desktop_proxy_ip(
            &db,
            DesktopProxyChangeIpRequest {
                proxy_id: "proxy-change-ip-check".to_string(),
                mode: None,
                session_key: None,
                requested_provider: None,
                requested_region: None,
                sticky_ttl_seconds: None,
            },
        )
        .await;

        let error_message = result
            .err()
            .map(|err| err.to_string())
            .unwrap_or_else(|| "missing error".to_string());
        assert!(
            error_message.contains("success_path"),
            "expected success_path validation error, got: {error_message}"
        );

        let task_row = sqlx::query(
            r#"SELECT status, error_message, result_json
               FROM tasks
               WHERE kind = 'change_proxy_ip'
               ORDER BY CAST(created_at AS INTEGER) DESC, id DESC
               LIMIT 1"#,
        )
        .fetch_one(&db)
        .await
        .expect("load failed success-check task");

        let task_status: String = task_row.get("status");
        let task_error: Option<String> = task_row.get("error_message");
        let result_json_raw: String = task_row.get("result_json");
        let result_json =
            serde_json::from_str::<Value>(&result_json_raw).expect("decode failed result_json");

        assert_eq!(task_status, "failed");
        assert!(
            task_error
                .as_deref()
                .is_some_and(|message| message.contains("success_path")),
            "expected stored success_path failure, got: {:?}",
            task_error
        );
        assert_eq!(
            result_json.get("executionStatus").and_then(Value::as_str),
            Some("failed")
        );
        assert_eq!(
            result_json.get("errorKind").and_then(Value::as_str),
            Some("provider_refresh_success_check_failed")
        );
        assert_eq!(
            result_json.get("rollbackSignal").and_then(Value::as_str),
            Some("binding_not_applied")
        );
    }

    #[tokio::test]
    async fn launch_template_run_exposes_fanout_summary_and_read_run_detail_tracks_artifacts_and_logs(
    ) {
        let db_url = format!(
            "sqlite:///tmp/persona_pilot_desktop_semantics_{}.db",
            Uuid::new_v4()
        );
        let db = init_db(&db_url).await.expect("init db");
        seed_launch_test_fixtures(&db).await;

        let launch = launch_desktop_template_run(
            &db,
            &db_url,
            DesktopLaunchTemplateRunRequest {
                template_id: "tpl-launch-test".to_string(),
                store_id: None,
                profile_ids: vec!["persona-launch-test".to_string()],
                variable_bindings: serde_json::json!({}),
                dry_run: Some(false),
                mode: Some("queue".to_string()),
                launch_note: Some("smoke launch".to_string()),
                source_run_id: Some("source-run-001".to_string()),
                recorder_session_id: Some("recorder-session-001".to_string()),
                target_scope: Some("profile".to_string()),
            },
        )
        .await
        .expect("launch template run");

        assert_eq!(launch.task_count, 1);
        assert_eq!(launch.accepted_profile_count, 1);
        assert_eq!(launch.launch_summary.launch_kind, "template_run_fanout");
        assert_eq!(
            launch.launch_summary.primary_task_id.as_deref(),
            launch.task_id.as_deref()
        );
        assert_eq!(
            launch.launch_summary.accepted_profile_ids,
            vec!["persona-launch-test".to_string()]
        );
        assert_eq!(
            launch.launch_summary.launch_note.as_deref(),
            Some("smoke launch")
        );

        let task_id = launch.task_id.expect("launch task id");
        let run_id = format!("run-launch-test-{}", Uuid::new_v4());
        sqlx::query(
            r#"UPDATE tasks
               SET status = 'succeeded', started_at = '2000000001', finished_at = '2000000004',
                   result_json = ?, error_message = NULL
               WHERE id = ?"#,
        )
        .bind(
            serde_json::json!({
                "title": "Launch test complete",
                "message": "template fanout finished",
            })
            .to_string(),
        )
        .bind(&task_id)
        .execute(&db)
        .await
        .expect("update task result");

        sqlx::query(
            r#"INSERT INTO runs (
                   id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message, result_json
               ) VALUES (?, ?, 'succeeded', 1, 'lightpanda', ?, ?, NULL, ?)"#,
        )
        .bind(&run_id)
        .bind(&task_id)
        .bind("2000000001")
        .bind("2000000004")
        .bind(
            serde_json::json!({
                "headline": "Launch test complete",
                "message": "template fanout finished",
            })
            .to_string(),
        )
        .execute(&db)
        .await
        .expect("insert run");

        sqlx::query(
            r#"INSERT INTO artifacts (
                   id, task_id, run_id, kind, storage_path, metadata_json, created_at
               ) VALUES (?, ?, ?, 'screenshot', ?, ?, '2000000002')"#,
        )
        .bind(format!("artifact-{}", Uuid::new_v4()))
        .bind(&task_id)
        .bind(&run_id)
        .bind("D:/SelfMadeTool/persona-pilot/data/artifacts/launch-test.png")
        .bind(
            serde_json::json!({
                "label": "Launch proof",
                "status": "ready",
            })
            .to_string(),
        )
        .execute(&db)
        .await
        .expect("insert artifact");

        sqlx::query(
            r#"INSERT INTO logs (
                   id, task_id, run_id, level, message, created_at
               ) VALUES (?, ?, ?, 'INFO', ?, '2000000002')"#,
        )
        .bind(format!("log-{}", Uuid::new_v4()))
        .bind(&task_id)
        .bind(&run_id)
        .bind("launch test log")
        .execute(&db)
        .await
        .expect("insert log");

        let detail = read_desktop_run_detail(
            &db,
            DesktopReadRunDetailQuery {
                run_id: Some(run_id.clone()),
                task_id: None,
            },
        )
        .await
        .expect("read run detail");

        assert_eq!(detail.run_id, run_id);
        assert_eq!(detail.task_id.as_deref(), Some(task_id.as_str()));
        assert_eq!(detail.task_status, "succeeded");
        assert_eq!(detail.summary.run_status, "succeeded");
        assert_eq!(detail.summary.artifact_count, 1);
        assert_eq!(detail.summary.log_count, 2);
        assert!(detail
            .timeline
            .iter()
            .any(|entry| entry.label == "Run artifacts indexed"));
        assert!(detail
            .timeline
            .iter()
            .any(|entry| entry.label == "Run logs indexed"));
        assert!(detail
            .timeline
            .iter()
            .any(|entry| entry.detail.as_deref() == Some("launch test log")));
        assert_eq!(
            detail.summary.timeline_count as usize,
            detail.timeline.len()
        );
        assert!(detail
            .raw
            .as_ref()
            .and_then(|value| value.get("summary"))
            .is_some());
    }
}
