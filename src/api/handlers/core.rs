use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use reqwest::Url;
use std::{net::SocketAddr, time::{Instant, SystemTime, UNIX_EPOCH}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::{
    network_identity::validator::validate_fingerprint_profile,
    network_identity::{
        proxy_growth::{
            assess_proxy_pool_health, proxy_pool_growth_policy_from_env,
            proxy_replenish_global_batch_limit_from_env,
            proxy_replenish_region_batch_limit_from_env,
            proxy_replenish_total_batch_limit_from_env, ProxyPoolInventorySnapshot,
        },
        proxy_harvest::{load_proxy_harvest_metrics, proxy_runtime_mode_from_env},
    },
    db::init::{provider_risk_version_state_for_proxy, refresh_cached_trust_score_for_proxy, refresh_proxy_trust_views_for_scope},
    app::state::AppState,
    domain::{
        run::{RUN_STATUS_CANCELLED, RUN_STATUS_RUNNING},
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_PENDING, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING,
            TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
};

use crate::api::{
    dto::*,
    explainability::{build_task_explainability, content_bool_field, content_i64_field, content_string_field, enrich_summary_artifacts, latest_browser_ready_tasks, latest_execution_summaries},
};

fn perf_probe_enabled() -> bool {
    std::env::var("PP_PERF_PROBE")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

fn perf_probe_log(event: &str, fields: &[(&str, String)]) {
    if !perf_probe_enabled() {
        return;
    }
    let detail = fields
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(" ");
    if detail.is_empty() {
        eprintln!("perf_probe event={}", event);
    } else {
        eprintln!("perf_probe event={} {}", event, detail);
    }
}

fn now_ts_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

const HEARTBEAT_METRICS_WINDOW_SECONDS: i64 = 86_400;

fn normalize_origin_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parsed = Url::parse(trimmed).ok()?;
    let scheme = parsed.scheme();
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    if parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path() != "/"
    {
        return None;
    }
    Some(parsed.origin().ascii_serialization())
}

fn round_ratio_percent(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        ((numerator as f64) * 1_000_000.0 / denominator as f64).round() / 10_000.0
    }
}

async fn send_telegram_message_if_configured(text: &str) {
    let bot_token = match std::env::var("PERSONA_PILOT_TELEGRAM_BOT_TOKEN") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return,
    };
    let chat_id = match std::env::var("PERSONA_PILOT_TELEGRAM_CHAT_ID") {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return,
    };
    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let _ = reqwest::Client::new()
        .post(url)
        .json(&json!({
            "chat_id": chat_id,
            "text": text,
            "disable_web_page_preview": true,
        }))
        .send()
        .await;
}

async fn notify_continuity_event_if_needed(
    event_type: &str,
    severity: &str,
    persona_id: Option<&str>,
    store_id: Option<&str>,
    platform_id: Option<&str>,
    task_id: Option<&str>,
    event_json: Option<&Value>,
) {
    let should_notify = matches!(
        event_type,
        "continuity_broken"
            | "login_risk_detected"
            | "manual_gate_requested"
            | "manual_gate_confirmed"
            | "manual_gate_rejected"
    );
    if !should_notify {
        return;
    }

    let mut lines = vec![format!("[{}] {}", severity.to_ascii_uppercase(), event_type)];
    if let Some(value) = persona_id {
        lines.push(format!("persona={value}"));
    }
    if let Some(value) = store_id {
        lines.push(format!("store={value}"));
    }
    if let Some(value) = platform_id {
        lines.push(format!("platform={value}"));
    }
    if let Some(value) = task_id {
        lines.push(format!("task={value}"));
    }
    if let Some(value) = event_json {
        if let Some(summary) = value.get("reason").and_then(|item| item.as_str()) {
            lines.push(format!("reason={summary}"));
        } else if let Some(summary) = value.get("matched_signal").and_then(|item| item.as_str()) {
            lines.push(format!("signal={summary}"));
        } else if let Some(summary) = value
            .get("manual_gate_request_id")
            .and_then(|item| item.as_str())
        {
            lines.push(format!("manual_gate={summary}"));
        }
    }

    send_telegram_message_if_configured(&lines.join("\n")).await;
}

fn sanitize_limit(limit: Option<i64>, default_value: i64, max_value: i64) -> i64 {
    match limit {
        Some(value) if value > 0 => value.min(max_value),
        _ => default_value,
    }
}

fn sanitize_offset(offset: Option<i64>) -> i64 {
    match offset {
        Some(value) if value > 0 => value,
        _ => 0,
    }
}

const BROWSER_PROXY_REQUIRED_MESSAGE: &str =
    "direct mode is forbidden for browser tasks; browser access must use proxy pool";
const HOT_REGION_WINDOW_SECONDS: i64 = 600;
const HOT_REGION_LIMIT: i64 = 5;
const ACTIVE_INVENTORY_MIN: i64 = 5;
const SOURCE_CONCENTRATION_TARGET_CAP_PERCENT: f64 = 75.0;

fn is_browser_task_kind(kind: &str) -> bool {
    matches!(
        kind,
        "open_page"
            | "get_html"
            | "get_title"
            | "get_final_url"
            | "extract_text"
            | "execute_behavior_flow"
    )
}

fn payload_json_field(payload: &Value, key: &str) -> Option<Value> {
    payload.get(key).cloned().filter(|value| !value.is_null())
}

fn payload_string_field_optional(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn is_candidate_proxy_status(status: &str) -> bool {
    matches!(status, "candidate" | "candidate_rejected")
}

fn normalize_optional_task_text(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn normalize_proxy_mode_for_task(raw: Option<&str>, fallback: &str) -> String {
    normalize_optional_task_text(raw)
        .map(|value| value.replace('-', "_").to_ascii_lowercase())
        .unwrap_or_else(|| fallback.replace('-', "_").to_ascii_lowercase())
}

fn requested_region_from_policy_json(policy_json: Option<&Value>) -> Option<String> {
    let policy = policy_json?;
    normalize_optional_task_text(
        policy
            .get("region")
            .and_then(|value| value.as_str())
            .or_else(|| {
                policy
                    .get("requested_region")
                    .and_then(|value| value.as_str())
            }),
    )
}

fn task_typed_proxy_columns(
    explicit_proxy_id: Option<&str>,
    policy_json: Option<&Value>,
    proxy_mode: &str,
) -> (Option<String>, Option<String>, String) {
    let proxy_id = normalize_optional_task_text(explicit_proxy_id).or_else(|| {
        policy_json
            .and_then(|policy| policy.get("proxy_id"))
            .and_then(|value| value.as_str())
            .and_then(|value| normalize_optional_task_text(Some(value)))
    });
    let requested_region = requested_region_from_policy_json(policy_json);
    let normalized_mode = normalize_proxy_mode_for_task(Some(proxy_mode), proxy_mode);
    (proxy_id, requested_region, normalized_mode)
}

fn round_percent(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        ((numerator as f64) * 10000.0 / denominator as f64).round() / 100.0
    }
}

fn normalized_balance_label(raw: Option<&str>, fallback: &str) -> String {
    normalize_optional_task_text(raw).unwrap_or_else(|| fallback.to_string())
}

fn normalize_browser_task_request(
    mut payload: CreateTaskRequest,
) -> Result<CreateTaskRequest, (StatusCode, String)> {
    if !is_browser_task_kind(payload.kind.as_str()) {
        return Ok(payload);
    }

    let mut policy_obj = match payload.network_policy_json.take() {
        Some(Value::Object(obj)) => obj,
        Some(Value::Null) | None => serde_json::Map::new(),
        Some(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "network_policy_json must be an object for browser tasks".to_string(),
            ))
        }
    };

    if policy_obj
        .get("mode")
        .and_then(|v| v.as_str())
        .map(|mode| mode.eq_ignore_ascii_case("direct"))
        .unwrap_or(false)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            BROWSER_PROXY_REQUIRED_MESSAGE.to_string(),
        ));
    }

    policy_obj.insert("mode".to_string(), json!("required_proxy"));
    policy_obj.insert("require_proxy".to_string(), json!(true));
    if let Some(proxy_id) = payload.proxy_id.as_deref() {
        policy_obj
            .entry("proxy_id".to_string())
            .or_insert_with(|| json!(proxy_id));
    }

    payload.network_policy_json = Some(Value::Object(policy_obj));
    Ok(payload)
}

#[derive(Debug, Clone)]
struct ResolvedNetworkPolicyModel {
    id: String,
    region_anchor: Option<String>,
    allow_same_country_fallback: bool,
    allow_same_region_fallback: bool,
    provider_preference: Option<String>,
    network_policy_json: Value,
}

#[derive(Debug, Clone)]
struct ResolvedContinuityPolicyModel {
    id: String,
    session_ttl_seconds: i64,
    heartbeat_interval_seconds: i64,
    site_group_mode: String,
    recovery_enabled: bool,
    protect_on_login_loss: bool,
}

#[derive(Debug, Clone)]
struct ResolvedPlatformTemplateModel {
    id: String,
    platform_id: String,
    readiness_level: String,
    warm_paths_json: Value,
    revisit_paths_json: Value,
    stateful_paths_json: Value,
    write_operation_paths_json: Value,
    high_risk_paths_json: Value,
    continuity_checks_json: Value,
    identity_markers_json: Value,
    identity_markers_source: Option<String>,
    login_loss_signals_json: Value,
    recovery_steps_json: Value,
    behavior_defaults_json: Value,
    event_chain_templates_json: Value,
    page_semantics_json: Value,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct StorePlatformOverrideLookupRow {
    admin_origin: Option<String>,
    entry_origin: Option<String>,
    entry_paths_json: Option<String>,
    warm_paths_json: Option<String>,
    revisit_paths_json: Option<String>,
    stateful_paths_json: Option<String>,
    high_risk_paths_json: Option<String>,
    recovery_steps_json: Option<String>,
    login_loss_signals_json: Option<String>,
    identity_markers_json: Option<String>,
    behavior_defaults_json: Option<String>,
    event_chain_templates_json: Option<String>,
    page_semantics_json: Option<String>,
    status: String,
}

#[derive(Debug, Clone)]
struct ResolvedPersonaBundle {
    persona_id: String,
    store_id: String,
    platform_id: String,
    device_family: String,
    country_anchor: String,
    region_anchor: Option<String>,
    locale: String,
    timezone: String,
    fingerprint_profile_id: String,
    behavior_profile_id: Option<String>,
    credential_ref: Option<String>,
    network_policy: ResolvedNetworkPolicyModel,
    continuity_policy: ResolvedContinuityPolicyModel,
    platform_template: Option<ResolvedPlatformTemplateModel>,
    resolved_admin_origin: Option<String>,
    resolved_entry_origin: Option<String>,
    resolved_entry_paths: Vec<String>,
    default_platform_origin: Option<String>,
    origin_source: Option<String>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct ResolvedPersonaLookupRow {
    persona_id: String,
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
    persona_status: String,
    network_policy_region_anchor: Option<String>,
    allow_same_country_fallback: i64,
    allow_same_region_fallback: i64,
    provider_preference: Option<String>,
    network_policy_json: String,
    network_status: String,
    continuity_policy_lookup_id: String,
    session_ttl_seconds: i64,
    heartbeat_interval_seconds: i64,
    site_group_mode: String,
    recovery_enabled: i64,
    protect_on_login_loss: i64,
    continuity_status: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct HeartbeatPersonaCandidateRow {
    persona_id: String,
    store_id: String,
    platform_id: String,
    heartbeat_interval_seconds: i64,
}

fn normalize_status(raw: Option<String>, default_value: &str) -> String {
    raw.unwrap_or_else(|| default_value.to_string())
}

fn parse_json_text(raw: Option<String>, fallback: Value) -> Value {
    raw.and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .unwrap_or(fallback)
}

fn parse_optional_json_text(raw: Option<String>) -> Option<Value> {
    raw.and_then(|value| serde_json::from_str::<Value>(&value).ok())
}

fn merge_json_objects(base: Value, overlay: Value) -> Value {
    let mut base_obj = match base {
        Value::Object(obj) => obj,
        _ => serde_json::Map::new(),
    };
    if let Value::Object(overlay_obj) = overlay {
        for (key, value) in overlay_obj {
            base_obj.insert(key, value);
        }
    }
    Value::Object(base_obj)
}

fn path_patterns_from_value(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().map(str::trim))
        .filter(|item| !item.is_empty())
        .map(|item| item.to_string())
        .collect()
}

fn url_matches_any_path(url: &str, patterns: &[String]) -> bool {
    let parsed = Url::parse(url).ok();
    let path = parsed
        .as_ref()
        .map(|value| value.path().to_string())
        .unwrap_or_else(|| url.to_string());
    patterns.iter().any(|pattern| {
        url.contains(pattern) || path.starts_with(pattern)
    })
}

fn default_platform_origin(platform_id: &str) -> Option<&'static str> {
    match platform_id.trim().to_ascii_lowercase().as_str() {
        "amazon" | "amazon_seller_central" | "amazon-seller-central" => {
            Some("https://sellercentral.amazon.com")
        }
        "ebay" | "ebay_seller_hub" | "ebay-seller-hub" => Some("https://www.ebay.com"),
        "shopify" | "shopify_admin" | "shopify-admin" => Some("https://admin.shopify.com"),
        "walmart" | "walmart_seller_center" | "walmart-seller-center" => {
            Some("https://seller.walmart.com")
        }
        "tiktok_shop" | "tiktok-shop" | "tiktokshop" => Some("https://seller-us.tiktok.com"),
        "xiaohongshu" | "xhs" => Some("https://seller.xiaohongshu.com"),
        "independent_site" | "independent-site" | "independent" => Some("https://example.com"),
        _ => None,
    }
}

fn is_xiaohongshu_platform(platform_id: &str) -> bool {
    matches!(platform_id.trim().to_ascii_lowercase().as_str(), "xiaohongshu" | "xhs")
}

fn is_sample_ready_template(template: &ResolvedPlatformTemplateModel) -> bool {
    template
        .readiness_level
        .trim()
        .eq_ignore_ascii_case("sample_ready")
}

fn heartbeat_task_kind_for_template(template: &ResolvedPlatformTemplateModel) -> &'static str {
    if is_sample_ready_template(template) && is_xiaohongshu_platform(&template.platform_id) {
        "extract_text"
    } else {
        "open_page"
    }
}

fn resolve_heartbeat_target_candidates(
    bundle: &ResolvedPersonaBundle,
    template: &ResolvedPlatformTemplateModel,
) -> Vec<String> {
    if !bundle.resolved_entry_paths.is_empty() {
        return bundle.resolved_entry_paths.clone();
    }
    [
        &template.revisit_paths_json,
        &template.warm_paths_json,
        &template.stateful_paths_json,
    ]
    .into_iter()
    .find_map(|value| {
        let paths = path_patterns_from_value(value);
        (!paths.is_empty()).then_some(paths)
    })
    .unwrap_or_default()
}

async fn heartbeat_target_index_for_persona(
    state: &AppState,
    persona_id: &str,
    candidate_count: usize,
) -> Result<usize, (StatusCode, String)> {
    if candidate_count <= 1 {
        return Ok(0);
    }
    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM continuity_events
           WHERE persona_id = ?
             AND event_type IN ('heartbeat_scheduled', 'heartbeat_skipped', 'heartbeat_failed')"#,
    )
    .bind(persona_id)
    .fetch_one(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load heartbeat event count for persona {persona_id}: {err}"),
        )
    })?;
    Ok((total.rem_euclid(candidate_count as i64)) as usize)
}

async fn resolve_heartbeat_target_path(
    state: &AppState,
    bundle: &ResolvedPersonaBundle,
    template: &ResolvedPlatformTemplateModel,
) -> Result<Option<String>, (StatusCode, String)> {
    let candidates = resolve_heartbeat_target_candidates(bundle, template);
    if candidates.is_empty() {
        return Ok(None);
    }
    if is_sample_ready_template(template) && candidates.len() > 1 {
        let index = heartbeat_target_index_for_persona(state, &bundle.persona_id, candidates.len())
            .await?;
        return Ok(candidates.get(index).cloned());
    }
    Ok(candidates.first().cloned())
}

fn join_origin_with_target(origin: &str, target_path: &str) -> Option<String> {
    let trimmed = target_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some(trimmed.to_string());
    }
    let normalized_path = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    Url::parse(origin)
        .ok()
        .and_then(|value| value.join(&normalized_path).ok())
        .map(|value| value.to_string())
}

fn resolve_heartbeat_target_url(
    bundle: &ResolvedPersonaBundle,
    target_path: &str,
) -> Option<(String, String)> {
    let trimmed = target_path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Some((
            trimmed.to_string(),
            bundle
                .origin_source
                .clone()
                .or_else(|| {
                    bundle
                        .default_platform_origin
                        .as_ref()
                        .map(|_| "platform_default".to_string())
                })
                .unwrap_or_else(|| "platform_default".to_string()),
        ));
    }

    let candidates = [
        (
            bundle.resolved_entry_origin.as_deref(),
            bundle.origin_source.as_deref(),
        ),
        (
            bundle.resolved_admin_origin.as_deref(),
            Some("store_override_admin"),
        ),
        (
            bundle.default_platform_origin.as_deref(),
            Some("platform_default"),
        ),
    ];
    for (origin, source) in candidates {
        let Some(origin) = origin else {
            continue;
        };
        if let Some(target_url) = join_origin_with_target(origin, trimmed) {
            return Some((target_url, source.unwrap_or("platform_default").to_string()));
        }
    }
    None
}

async fn persona_has_inflight_task(
    state: &AppState,
    persona_id: &str,
) -> Result<bool, (StatusCode, String)> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM tasks WHERE persona_id = ? AND status IN ('pending', 'queued', 'running')"#,
    )
    .bind(persona_id)
    .fetch_one(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to inspect in-flight task state for persona {persona_id}: {err}"),
        )
    })?;
    Ok(count > 0)
}

async fn latest_persona_task_activity_ts(
    state: &AppState,
    persona_id: &str,
) -> Result<Option<i64>, (StatusCode, String)> {
    sqlx::query_scalar(
        r#"SELECT MAX(CAST(COALESCE(finished_at, started_at, queued_at, created_at) AS INTEGER)) FROM tasks WHERE persona_id = ?"#,
    )
    .bind(persona_id)
    .fetch_one(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to inspect latest task activity for persona {persona_id}: {err}"),
        )
    })
}

fn merged_template_value(
    base: &Value,
    override_value: Option<&Value>,
) -> Value {
    override_value.cloned().unwrap_or_else(|| base.clone())
}

async fn resolve_persona_bundle(
    state: &AppState,
    persona_id: &str,
) -> Result<ResolvedPersonaBundle, (StatusCode, String)> {
    let row = sqlx::query_as::<_, ResolvedPersonaLookupRow>(
        r#"
        SELECT
            p.id AS persona_id,
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
            p.status AS persona_status,
            np.region_anchor AS network_policy_region_anchor,
            np.allow_same_country_fallback,
            np.allow_same_region_fallback,
            np.provider_preference,
            np.network_policy_json,
            np.status AS network_status,
            cp.id AS continuity_policy_lookup_id,
            cp.session_ttl_seconds,
            cp.heartbeat_interval_seconds,
            cp.site_group_mode,
            cp.recovery_enabled,
            cp.protect_on_login_loss,
            cp.status AS continuity_status
        FROM persona_profiles p
        JOIN network_policies np ON np.id = p.network_policy_id
        JOIN continuity_policies cp ON cp.id = p.continuity_policy_id
        WHERE p.id = ?
        "#,
    )
    .bind(persona_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to resolve persona profile: {err}"),
        )
    })?;

    let Some(row) = row else {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("persona profile not found: {persona_id}"),
        ));
    };

    if !matches!(row.persona_status.as_str(), "active" | "degraded") {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("persona profile is not runnable: {}", row.persona_id),
        ));
    }
    if row.network_status != "active" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("network policy is not active: {}", row.network_policy_id),
        ));
    }
    if row.continuity_status != "active" {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "continuity policy is not active: {}",
                row.continuity_policy_id
            ),
        ));
    }

    let template_row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            String,
        ),
    >(
        r#"
        SELECT
            id,
            platform_id,
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
            page_semantics_json,
            readiness_level,
            status
        FROM platform_templates
        WHERE platform_id = ?
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(&row.platform_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load platform template: {err}"),
        )
    })?;

    let override_row = sqlx::query_as::<_, StorePlatformOverrideLookupRow>(
        r#"
        SELECT
            admin_origin,
            entry_origin,
            entry_paths_json,
            warm_paths_json,
            revisit_paths_json,
            stateful_paths_json,
            high_risk_paths_json,
            recovery_steps_json,
            login_loss_signals_json,
            identity_markers_json,
            behavior_defaults_json,
            event_chain_templates_json,
            page_semantics_json,
            status
        FROM store_platform_overrides
        WHERE store_id = ? AND platform_id = ?
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(&row.store_id)
    .bind(&row.platform_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load store platform override: {err}"),
        )
    })?;

    let override_active = override_row
        .as_ref()
        .map(|row| row.status.as_str() == "active")
        .unwrap_or(false);
    let override_admin_origin = override_active
        .then(|| {
            override_row
                .as_ref()
                .and_then(|row| row.admin_origin.as_deref().and_then(normalize_origin_value))
        })
        .flatten();
    let override_entry_origin = override_active
        .then(|| {
            override_row
                .as_ref()
                .and_then(|row| row.entry_origin.as_deref().and_then(normalize_origin_value))
        })
        .flatten();
    let override_entry_paths = override_active
        .then(|| {
            override_row
                .as_ref()
                .and_then(|row| parse_optional_json_text(row.entry_paths_json.clone()))
        })
        .flatten()
        .map(|value| path_patterns_from_value(&value))
        .unwrap_or_default();
    let default_origin = default_platform_origin(&row.platform_id).map(str::to_string);
    let resolved_admin_origin = override_admin_origin.clone().or_else(|| default_origin.clone());
    let resolved_entry_origin = override_entry_origin
        .clone()
        .or_else(|| resolved_admin_origin.clone());
    let origin_source = if override_entry_origin.is_some() {
        Some("store_override_entry".to_string())
    } else if override_admin_origin.is_some() {
        Some("store_override_admin".to_string())
    } else if default_origin.is_some() {
        Some("platform_default".to_string())
    } else {
        None
    };

    let platform_template = template_row.and_then(
        |(
            template_id,
            template_platform_id,
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
            page_semantics_json,
            readiness_level,
            template_status,
        )| {
            if template_status != "active" {
                return None;
            }
            let override_warm = override_active
                .then(|| {
                    override_row
                        .as_ref()
                        .and_then(|row| parse_optional_json_text(row.warm_paths_json.clone()))
                })
                .flatten();
            let override_revisit = override_active
                .then(|| {
                    override_row
                        .as_ref()
                        .and_then(|row| parse_optional_json_text(row.revisit_paths_json.clone()))
                })
                .flatten();
            let override_stateful = override_active
                .then(|| {
                    override_row
                        .as_ref()
                        .and_then(|row| parse_optional_json_text(row.stateful_paths_json.clone()))
                })
                .flatten();
            let override_high_risk = override_active
                .then(|| {
                    override_row
                        .as_ref()
                        .and_then(|row| parse_optional_json_text(row.high_risk_paths_json.clone()))
                })
                .flatten();
            let override_recovery = override_active
                .then(|| {
                    override_row
                        .as_ref()
                        .and_then(|row| parse_optional_json_text(row.recovery_steps_json.clone()))
                })
                .flatten();
            let override_login_signals = override_active
                .then(|| {
                    override_row.as_ref().and_then(|row| {
                        parse_optional_json_text(row.login_loss_signals_json.clone())
                    })
                })
                .flatten();
            let override_identity_markers = override_active
                .then(|| {
                    override_row
                        .as_ref()
                        .and_then(|row| parse_optional_json_text(row.identity_markers_json.clone()))
                })
                .flatten();
            let override_behavior_defaults = override_active
                .then(|| {
                    override_row
                        .as_ref()
                        .and_then(|row| parse_optional_json_text(row.behavior_defaults_json.clone()))
                })
                .flatten();
            let override_event_chain_templates = override_active
                .then(|| {
                    override_row.as_ref().and_then(|row| {
                        parse_optional_json_text(row.event_chain_templates_json.clone())
                    })
                })
                .flatten();
            let override_page_semantics = override_active
                .then(|| {
                    override_row
                        .as_ref()
                        .and_then(|row| parse_optional_json_text(row.page_semantics_json.clone()))
                })
                .flatten();
            let base_identity_markers = parse_json_text(identity_markers_json.clone(), json!([]));
            let resolved_identity_markers =
                merged_template_value(&base_identity_markers, override_identity_markers.as_ref());
            let base_identity_markers_present = base_identity_markers
                .as_array()
                .map(|items| !items.is_empty())
                .unwrap_or(false);
            let override_identity_markers_present = override_identity_markers
                .as_ref()
                .and_then(Value::as_array)
                .map(|items| !items.is_empty())
                .unwrap_or(false);
            Some(ResolvedPlatformTemplateModel {
                id: template_id,
                platform_id: template_platform_id,
                readiness_level,
                warm_paths_json: merged_template_value(
                    &parse_json_text(Some(warm_paths_json), json!([])),
                    override_warm.as_ref(),
                ),
                revisit_paths_json: merged_template_value(
                    &parse_json_text(Some(revisit_paths_json), json!([])),
                    override_revisit.as_ref(),
                ),
                stateful_paths_json: merged_template_value(
                    &parse_json_text(Some(stateful_paths_json), json!([])),
                    override_stateful.as_ref(),
                ),
                write_operation_paths_json: parse_json_text(
                    Some(write_operation_paths_json),
                    json!([]),
                ),
                high_risk_paths_json: merged_template_value(
                    &parse_json_text(Some(high_risk_paths_json), json!([])),
                    override_high_risk.as_ref(),
                ),
                continuity_checks_json: parse_json_text(continuity_checks_json, json!([])),
                identity_markers_json: resolved_identity_markers,
                identity_markers_source: if override_identity_markers_present {
                    Some("store_override".to_string())
                } else if base_identity_markers_present {
                    Some("platform_template".to_string())
                } else {
                    None
                },
                login_loss_signals_json: merged_template_value(
                    &parse_json_text(login_loss_signals_json, json!([])),
                    override_login_signals.as_ref(),
                ),
                recovery_steps_json: merged_template_value(
                    &parse_json_text(recovery_steps_json, json!([])),
                    override_recovery.as_ref(),
                ),
                behavior_defaults_json: merged_template_value(
                    &parse_json_text(behavior_defaults_json, json!({})),
                    override_behavior_defaults.as_ref(),
                ),
                event_chain_templates_json: merged_template_value(
                    &parse_json_text(event_chain_templates_json, json!({})),
                    override_event_chain_templates.as_ref(),
                ),
                page_semantics_json: merged_template_value(
                    &parse_json_text(page_semantics_json, json!({})),
                    override_page_semantics.as_ref(),
                ),
            })
        },
    );

    Ok(ResolvedPersonaBundle {
        persona_id: row.persona_id,
        store_id: row.store_id,
        platform_id: row.platform_id,
        device_family: row.device_family,
        country_anchor: row.country_anchor,
        region_anchor: row.region_anchor,
        locale: row.locale,
        timezone: row.timezone,
        fingerprint_profile_id: row.fingerprint_profile_id,
        behavior_profile_id: row.behavior_profile_id,
        credential_ref: row.credential_ref,
        network_policy: ResolvedNetworkPolicyModel {
            id: row.network_policy_id,
            region_anchor: row.network_policy_region_anchor,
            allow_same_country_fallback: row.allow_same_country_fallback != 0,
            allow_same_region_fallback: row.allow_same_region_fallback != 0,
            provider_preference: row.provider_preference,
            network_policy_json: parse_json_text(Some(row.network_policy_json), json!({})),
        },
        continuity_policy: ResolvedContinuityPolicyModel {
            id: row.continuity_policy_lookup_id,
            session_ttl_seconds: row.session_ttl_seconds,
            heartbeat_interval_seconds: row.heartbeat_interval_seconds,
            site_group_mode: row.site_group_mode,
            recovery_enabled: row.recovery_enabled != 0,
            protect_on_login_loss: row.protect_on_login_loss != 0,
        },
        platform_template,
        resolved_admin_origin,
        resolved_entry_origin,
        resolved_entry_paths: override_entry_paths,
        default_platform_origin: default_origin,
        origin_source,
    })
}


fn manual_gate_category_from_inputs(
    platform_id: Option<&str>,
    task_kind: &str,
    requested_operation_kind: Option<&str>,
    requested_url: Option<&str>,
) -> String {
    let normalized_operation = requested_operation_kind
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let normalized_url = requested_url
        .and_then(|value| Url::parse(value).ok())
        .map(|url| url.path().to_ascii_lowercase())
        .unwrap_or_default();
    let normalized_platform = platform_id
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();
    let normalized_task_kind = task_kind.trim().to_ascii_lowercase();

    let category = normalized_operation
        .as_deref()
        .and_then(|value| match value {
            "content_publish" => Some("content_publish"),
            "listing_publish" => Some("listing_publish"),
            "price_inventory_change" => Some("price_inventory_change"),
            "finance_payout" => Some("finance_payout"),
            "security_account" => Some("security_account"),
            "permissions_team" => Some("permissions_team"),
            _ => None,
        })
        .or_else(|| {
            let path = normalized_url.as_str();
            if path.contains("/finance")
                || path.contains("/payout")
                || path.contains("/withdraw")
                || path.contains("/settlement")
                || path.contains("/billing")
            {
                Some("finance_payout")
            } else if path.contains("/permissions")
                || path.contains("/team")
                || path.contains("/staff")
                || path.contains("/users")
                || path.contains("/roles")
            {
                Some("permissions_team")
            } else if path.contains("/security")
                || path.contains("/account")
                || path.contains("/recovery")
                || path.contains("/2fa")
            {
                Some("security_account")
            } else if path.contains("/price")
                || path.contains("/inventory")
                || path.contains("/stock")
                || path.contains("/sku")
            {
                Some("price_inventory_change")
            } else if path.contains("/publish")
                || path.contains("/notes")
                || path.contains("/content")
                || (normalized_platform == "xiaohongshu" && path.contains("/note"))
            {
                Some("content_publish")
            } else if path.contains("/listing")
                || path.contains("/product")
                || path.contains("/products")
                || path.contains("/item")
                || normalized_task_kind.contains("listing")
            {
                Some("listing_publish")
            } else {
                None
            }
        })
        .unwrap_or("security_account");

    category.to_string()
}

fn heartbeat_reason_bucket(reason: &str) -> &'static str {
    match reason {
        "persona_has_active_task" => "active_task",
        "recent_persona_activity" | "heartbeat_not_due" => "recent_activity",
        "platform_template_missing" | "missing_platform_template" => "template_missing",
        "heartbeat_target_missing" | "missing_heartbeat_path" => "no_target",
        "heartbeat_target_is_high_risk" => "high_risk_target",
        "heartbeat_target_origin_unresolved" | "missing_platform_origin" => "origin_unresolved",
        "heartbeat_due" => "scheduled_due",
        _ => "other",
    }
}

fn heartbeat_platform_cap_from_env() -> usize {
    std::env::var("PERSONA_PILOT_HEARTBEAT_PLATFORM_CAP")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3)
}

fn heartbeat_item(
    persona_id: String,
    store_id: String,
    platform_id: String,
    status: &str,
    reason: &str,
    task_id: Option<String>,
    target_url: Option<String>,
    heartbeat_interval_seconds: i64,
) -> ContinuityHeartbeatTickItemResponse {
    ContinuityHeartbeatTickItemResponse {
        persona_id,
        store_id,
        platform_id,
        status: status.to_string(),
        reason: reason.to_string(),
        task_id,
        target_url,
        heartbeat_interval_seconds,
    }
}

async fn append_heartbeat_event(
    state: &AppState,
    persona_id: &str,
    store_id: &str,
    platform_id: &str,
    task_id: Option<&str>,
    event_type: &str,
    severity: &str,
    reason: &str,
    heartbeat_interval_seconds: i64,
    target_path: Option<&str>,
    target_url: Option<&str>,
    origin_source: Option<&str>,
    probe_action: Option<&str>,
) -> Result<(), (StatusCode, String)> {
    append_continuity_event(
        state,
        Some(persona_id),
        Some(store_id),
        Some(platform_id),
        task_id,
        None,
        event_type,
        severity,
        Some(&json!({
            "reason": reason,
            "reason_bucket": heartbeat_reason_bucket(reason),
            "heartbeat_interval_seconds": heartbeat_interval_seconds,
            "target_path": target_path,
            "target_url": target_url,
            "origin_source": origin_source,
            "probe_action": probe_action,
            "task_id": task_id,
        })),
    )
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to record {event_type} event: {err}"),
        )
    })
}

async fn recent_heartbeat_failure_streak_24h(
    state: &AppState,
    persona_id: &str,
) -> anyhow::Result<i64> {
    let rows = sqlx::query_as::<_, (String,)>(
        r#"SELECT event_type
           FROM continuity_events
           WHERE persona_id = ?
             AND event_type IN ('heartbeat_scheduled', 'heartbeat_failed', 'heartbeat_skipped')
             AND CAST(created_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER) - ?
           ORDER BY CAST(created_at AS INTEGER) DESC, rowid DESC"#,
    )
    .bind(persona_id)
    .bind(HEARTBEAT_METRICS_WINDOW_SECONDS)
    .fetch_all(&state.db)
    .await?;

    let mut streak = 0_i64;
    for (event_type,) in rows {
        match event_type.as_str() {
            "heartbeat_failed" => streak += 1,
            "heartbeat_skipped" => {}
            "heartbeat_scheduled" => break,
            _ => break,
        }
        if streak >= 3 {
            return Ok(streak);
        }
    }

    Ok(streak)
}

pub async fn append_continuity_event(
    state: &AppState,
    persona_id: Option<&str>,
    store_id: Option<&str>,
    platform_id: Option<&str>,
    task_id: Option<&str>,
    run_id: Option<&str>,
    event_type: &str,
    severity: &str,
    event_json: Option<&Value>,
) -> anyhow::Result<()> {
    let event_id = format!("evt-{}", Uuid::new_v4());
    let created_at = now_ts_string();
    sqlx::query(
        r#"INSERT INTO continuity_events (
               id, persona_id, store_id, platform_id, task_id, run_id,
               event_type, severity, event_json, created_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&event_id)
    .bind(persona_id)
    .bind(store_id)
    .bind(platform_id)
    .bind(task_id)
    .bind(run_id)
    .bind(event_type)
    .bind(severity)
    .bind(event_json.map(Value::to_string))
    .bind(&created_at)
    .execute(&state.db)
    .await?;

    if let (Some(persona_id), Some(store_id), Some(platform_id)) = (persona_id, store_id, platform_id) {
        record_persona_health_snapshot(
            state,
            persona_id,
            store_id,
            platform_id,
            event_type,
            task_id,
            event_json,
        )
        .await?;
    }

    notify_continuity_event_if_needed(
        event_type,
        severity,
        persona_id,
        store_id,
        platform_id,
        task_id,
        event_json,
    )
    .await;

    Ok(())
}

async fn record_persona_health_snapshot(
    state: &AppState,
    persona_id: &str,
    store_id: &str,
    platform_id: &str,
    event_type: &str,
    task_id: Option<&str>,
    event_json: Option<&Value>,
) -> anyhow::Result<()> {
    let active_session_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM proxy_session_bindings WHERE persona_id = ?")
            .bind(persona_id)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

    let login_risk_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM continuity_events WHERE persona_id = ? AND event_type IN ('login_risk_detected', 'continuity_broken')",
    )
    .bind(persona_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let continuity_score = (active_session_count as f64 * 10.0) - (login_risk_count as f64 * 5.0);
    let current_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM persona_profiles WHERE id = ?")
            .bind(persona_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);
    let pending_manual_gate_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM manual_gate_requests WHERE persona_id = ? AND status = 'pending'",
    )
    .bind(persona_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);
    let heartbeat_failure_streak = recent_heartbeat_failure_streak_24h(state, persona_id)
        .await
        .unwrap_or(0);
    let last_task_at = latest_persona_task_activity_ts(state, persona_id)
        .await
        .ok()
        .flatten()
        .map(|value| value.to_string());
    let heartbeat_rows = sqlx::query_as::<_, (String, Option<String>, String)>(
        r#"SELECT event_type, event_json, created_at
           FROM continuity_events
           WHERE persona_id = ?
             AND event_type IN ('heartbeat_scheduled', 'heartbeat_skipped', 'heartbeat_failed')
             AND CAST(created_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER) - ?
           ORDER BY CAST(created_at AS INTEGER) DESC, rowid DESC"#,
    )
    .bind(persona_id)
    .bind(HEARTBEAT_METRICS_WINDOW_SECONDS)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let mut heartbeat_scheduled_count = 0_i64;
    let mut heartbeat_skipped_count = 0_i64;
    let mut heartbeat_failed_count = 0_i64;
    let mut heartbeat_skip_breakdown = serde_json::Map::new();
    for (row_event_type, row_event_json, _) in &heartbeat_rows {
        let parsed_json = row_event_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
        match row_event_type.as_str() {
            "heartbeat_scheduled" => heartbeat_scheduled_count += 1,
            "heartbeat_skipped" => {
                heartbeat_skipped_count += 1;
                if let Some(bucket) = parsed_json
                    .as_ref()
                    .and_then(|value| value.get("reason_bucket"))
                    .and_then(|value| value.as_str())
                {
                    let current = heartbeat_skip_breakdown
                        .get(bucket)
                        .and_then(|value| value.as_i64())
                        .unwrap_or(0);
                    heartbeat_skip_breakdown.insert(bucket.to_string(), json!(current + 1));
                }
            }
            "heartbeat_failed" => heartbeat_failed_count += 1,
            _ => {}
        }
    }
    let heartbeat_total =
        heartbeat_scheduled_count + heartbeat_skipped_count + heartbeat_failed_count;
    let heartbeat_success_ratio = if heartbeat_total > 0 {
        heartbeat_scheduled_count as f64 / heartbeat_total as f64
    } else {
        0.0
    };
    let latest_heartbeat_json = heartbeat_rows
        .first()
        .and_then(|(_, event_json, _)| event_json.as_deref())
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    let continuity_probe_rows = sqlx::query_as::<_, (String, Option<String>, String)>(
        r#"SELECT event_type, event_json, created_at
           FROM continuity_events
           WHERE persona_id = ?
             AND event_type IN ('browser_action_succeeded', 'browser_action_failed', 'heartbeat_failed', 'login_risk_detected', 'region_drift')
             AND CAST(created_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER) - ?
           ORDER BY CAST(created_at AS INTEGER) DESC, rowid DESC"#,
    )
    .bind(persona_id)
    .bind(HEARTBEAT_METRICS_WINDOW_SECONDS)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();
    let mut continuity_check_total = 0_i64;
    let mut continuity_check_failed_count = 0_i64;
    let mut continuity_check_skipped_count = 0_i64;
    let mut last_probe_action = Value::Null;
    let mut last_probe_path = Value::Null;
    let mut last_continuity_check_results = Value::Null;
    for (row_event_type, row_event_json, _) in &continuity_probe_rows {
        let Some(parsed_json) = row_event_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        else {
            continue;
        };
        if parsed_json
            .get("probe_action")
            .and_then(|value| value.as_str())
            .is_none()
        {
            continue;
        }
        continuity_check_total += 1;
        let failed_checks = parsed_json
            .get("failed_checks")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let skipped_checks = parsed_json
            .get("skipped_checks")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let matched_identity_marker = parsed_json
            .get("matched_identity_marker")
            .cloned()
            .unwrap_or(Value::Null);
        let configured_identity_markers = parsed_json
            .get("configured_identity_markers")
            .cloned()
            .unwrap_or_else(|| json!([]));
        let identity_markers_source = parsed_json
            .get("identity_markers_source")
            .cloned()
            .unwrap_or(Value::Null);
        continuity_check_skipped_count += skipped_checks.len() as i64;
        let has_failures =
            !failed_checks.is_empty() || matches!(row_event_type.as_str(), "browser_action_failed");
        if has_failures {
            continuity_check_failed_count += 1;
        }
        if last_probe_action.is_null() {
            last_probe_action = parsed_json
                .get("probe_action")
                .cloned()
                .unwrap_or(Value::Null);
            last_probe_path = parsed_json
                .get("probe_path")
                .cloned()
                .unwrap_or(Value::Null);
            last_continuity_check_results = json!({
                "passed_checks": parsed_json.get("passed_checks").cloned().unwrap_or_else(|| json!([])),
                "failed_checks": failed_checks,
                "skipped_checks": skipped_checks,
                "matched_identity_marker": matched_identity_marker.clone(),
                "configured_identity_markers": configured_identity_markers.clone(),
                "identity_markers_source": identity_markers_source.clone(),
                "evidence_summary": parsed_json.get("evidence_summary").cloned().unwrap_or(Value::Null),
            });
        } else if !matched_identity_marker.is_null() {
            if let Some(last_results) = last_continuity_check_results.as_object_mut() {
                let current_marker_is_missing = last_results
                    .get("matched_identity_marker")
                    .map(Value::is_null)
                    .unwrap_or(true);
                if current_marker_is_missing {
                    last_results.insert(
                        "matched_identity_marker".to_string(),
                        matched_identity_marker,
                    );
                    last_results.insert(
                        "configured_identity_markers".to_string(),
                        configured_identity_markers,
                    );
                    last_results.insert(
                        "identity_markers_source".to_string(),
                        identity_markers_source,
                    );
                }
            }
        } else if let Some(last_results) = last_continuity_check_results.as_object_mut() {
            let current_markers_missing = last_results
                .get("configured_identity_markers")
                .and_then(Value::as_array)
                .map(|items| items.is_empty())
                .unwrap_or(true);
            if current_markers_missing
                && configured_identity_markers
                    .as_array()
                    .map(|items| !items.is_empty())
                    .unwrap_or(false)
            {
                last_results.insert(
                    "configured_identity_markers".to_string(),
                    configured_identity_markers,
                );
            }
            let current_source_missing = last_results
                .get("identity_markers_source")
                .map(Value::is_null)
                .unwrap_or(true);
            if current_source_missing && !identity_markers_source.is_null() {
                last_results.insert(
                    "identity_markers_source".to_string(),
                    identity_markers_source,
                );
            }
        }
    }
    let continuity_check_success_ratio = if continuity_check_total > 0 {
        (continuity_check_total - continuity_check_failed_count) as f64
            / continuity_check_total as f64
    } else {
        0.0
    };
    let snapshot_json = json!({
        "heartbeat_window_24h": heartbeat_total,
        "heartbeat_success_ratio_24h": heartbeat_success_ratio,
        "heartbeat_failed_count_24h": heartbeat_failed_count,
        "heartbeat_skip_breakdown_24h": heartbeat_skip_breakdown,
        "last_heartbeat_reason": latest_heartbeat_json.as_ref().and_then(|value| value.get("reason")).cloned().unwrap_or(Value::Null),
        "last_heartbeat_target_url": latest_heartbeat_json.as_ref().and_then(|value| value.get("target_url")).cloned().unwrap_or(Value::Null),
        "last_origin_source": latest_heartbeat_json.as_ref().and_then(|value| value.get("origin_source")).cloned().unwrap_or(Value::Null),
        "current_event_type": event_type,
        "current_event_context": event_json.cloned().unwrap_or(Value::Null),
        "last_task_id": task_id,
        "heartbeat_failed_streak_24h": heartbeat_failure_streak,
        "manual_gate_pending": pending_manual_gate_count > 0,
        "last_continuity_check_results": last_continuity_check_results,
        "continuity_check_success_ratio_24h": continuity_check_success_ratio,
        "continuity_check_failed_count_24h": continuity_check_failed_count,
        "continuity_check_skipped_count_24h": continuity_check_skipped_count,
        "last_probe_action": last_probe_action,
        "last_probe_path": last_probe_path,
    });

    let direct_frozen = matches!(
        event_type,
        "login_risk_detected" | "continuity_broken" | "region_drift" | "restore_failed_after_retry"
    );
    let status = if matches!(current_status.as_deref(), Some("frozen")) || direct_frozen {
        "frozen"
    } else if heartbeat_failure_streak >= 3 || matches!(event_type, "recovery_failed") {
        "degraded"
    } else if matches!(
        event_type,
        "heartbeat_scheduled" | "continuity_restored" | "continuity_persisted"
    ) {
        "active"
    } else {
        current_status.as_deref().unwrap_or("active")
    };
    let created_at = now_ts_string();
    sqlx::query(
        r#"INSERT INTO persona_health_snapshots (
               id, persona_id, store_id, platform_id, status,
               active_session_count, continuity_score, login_risk_count,
               last_event_type, last_task_at, snapshot_json, created_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(format!("phs-{}", Uuid::new_v4()))
    .bind(persona_id)
    .bind(store_id)
    .bind(platform_id)
    .bind(status)
    .bind(active_session_count)
    .bind(continuity_score.max(0.0))
    .bind(login_risk_count)
    .bind(event_type)
    .bind(last_task_at)
    .bind(snapshot_json.to_string())
    .bind(&created_at)
    .execute(&state.db)
    .await?;

    sqlx::query("UPDATE persona_profiles SET status = ?, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(&created_at)
        .bind(persona_id)
        .execute(&state.db)
        .await?;

    Ok(())
}

fn merge_task_execution_identity(
    mut identity: ExecutionIdentity,
    persona_id: Option<String>,
    platform_id: Option<String>,
) -> ExecutionIdentity {
    if identity.persona_id.is_none() {
        identity.persona_id = persona_id;
    }
    if identity.platform_id.is_none() {
        identity.platform_id = platform_id;
    }
    identity
}

fn parsed_payload<'a>(parsed: Option<&'a Value>) -> Option<&'a Value> {
    parsed.and_then(|value| value.get("payload"))
}

fn payload_string_from_parsed(parsed: Option<&Value>, key: &str) -> Option<String> {
    parsed_payload(parsed)
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn value_from_parsed(parsed: Option<&Value>, key: &str) -> Option<Value> {
    parsed.and_then(|value| value.get(key)).cloned()
}

fn i64_from_parsed(parsed: Option<&Value>, key: &str) -> Option<i64> {
    parsed.and_then(|value| value.get(key)).and_then(|value| value.as_i64())
}

fn f64_from_parsed(parsed: Option<&Value>, key: &str) -> Option<f64> {
    parsed.and_then(|value| value.get(key)).and_then(|value| value.as_f64())
}

fn string_vec_from_parsed(parsed: Option<&Value>, key: &str) -> Option<Vec<String>> {
    let items = parsed
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_array())?;
    Some(
        items
            .iter()
            .filter_map(|value| value.as_str().map(|value| value.to_string()))
            .collect(),
    )
}

fn build_task_response_from_row(
    id: String,
    kind: String,
    status: String,
    priority: i32,
    persona_id: Option<String>,
    platform_id: Option<String>,
    manual_gate_request_id: Option<String>,
    manual_gate_status: Option<String>,
    fingerprint_profile_id: Option<String>,
    fingerprint_profile_version: Option<i64>,
    result_json: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
) -> TaskResponse {
    let parsed = result_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    let explainability = build_task_explainability(
        fingerprint_profile_id.as_deref(),
        fingerprint_profile_version,
        result_json.as_deref(),
        Some(&id),
        Some(&kind),
        Some(&status),
        finished_at.as_deref().or(started_at.as_deref()),
    );
    let execution_identity = merge_task_execution_identity(
        explainability.execution_identity,
        persona_id.clone(),
        platform_id.clone(),
    );

    TaskResponse {
        fingerprint_resolution_status: explainability.fingerprint_resolution_status,
        proxy_id: explainability.proxy_id,
        proxy_provider: explainability.proxy_provider,
        proxy_region: explainability.proxy_region,
        proxy_resolution_status: explainability.proxy_resolution_status,
        trust_score_total: explainability.trust_score_total,
        selection_reason_summary: explainability.selection_reason_summary,
        selection_explain: explainability.selection_explain,
        fingerprint_runtime_explain: explainability.fingerprint_runtime_explain,
        execution_identity: Some(execution_identity),
        identity_network_explain: explainability.identity_network_explain,
        winner_vs_runner_up_diff: explainability.winner_vs_runner_up_diff,
        failure_scope: explainability.failure_scope,
        browser_failure_signal: explainability.browser_failure_signal,
        summary_artifacts: explainability.summary_artifacts,
        title: content_string_field(parsed.as_ref(), "title"),
        final_url: content_string_field(parsed.as_ref(), "final_url"),
        content_preview: content_string_field(parsed.as_ref(), "content_preview"),
        content_length: content_i64_field(parsed.as_ref(), "content_length"),
        content_truncated: content_bool_field(parsed.as_ref(), "content_truncated"),
        content_kind: content_string_field(parsed.as_ref(), "content_kind"),
        content_source_action: content_string_field(parsed.as_ref(), "content_source_action"),
        content_ready: content_bool_field(parsed.as_ref(), "content_ready"),
        id,
        kind,
        status,
        priority,
        started_at,
        finished_at,
        persona_id,
        platform_id,
        behavior_profile_id: payload_string_from_parsed(parsed.as_ref(), "behavior_profile_id"),
        flow_template_id: payload_string_from_parsed(parsed.as_ref(), "flow_template_id"),
        humanize_level: payload_string_from_parsed(parsed.as_ref(), "humanize_level"),
        event_trace_level: payload_string_from_parsed(parsed.as_ref(), "event_trace_level"),
        manual_gate_request_id,
        manual_gate_status,
        fingerprint_profile_id,
        fingerprint_profile_version,
        interaction_trace_summary: value_from_parsed(parsed.as_ref(), "interaction_trace_summary"),
        event_counts: value_from_parsed(parsed.as_ref(), "event_counts"),
        distinct_event_types: string_vec_from_parsed(parsed.as_ref(), "distinct_event_types"),
        fingerprint_applied_verified_count: i64_from_parsed(parsed.as_ref(), "fingerprint_applied_verified_count"),
        deep_fingerprint_applied_verified_count: i64_from_parsed(parsed.as_ref(), "deep_fingerprint_applied_verified_count"),
        site_behavior_state: value_from_parsed(parsed.as_ref(), "site_behavior_state"),
        source_execution_share: f64_from_parsed(parsed.as_ref(), "source_execution_share"),
        proxy_execution_share: f64_from_parsed(parsed.as_ref(), "proxy_execution_share"),
    }
}

fn map_persona_profile_row(
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
) -> PersonaProfileResponse {
    PersonaProfileResponse {
        id,
        store_id,
        platform_id,
        device_family,
        country_anchor,
        region_anchor,
        locale,
        timezone,
        fingerprint_profile_id,
        behavior_profile_id,
        network_policy_id,
        continuity_policy_id,
        credential_ref,
        status,
        created_at,
        updated_at,
    }
}

fn map_network_policy_row(
    id: String,
    name: String,
    country_anchor: String,
    region_anchor: Option<String>,
    allow_same_country_fallback: i64,
    allow_same_region_fallback: i64,
    provider_preference: Option<String>,
    allowed_regions_json: Option<String>,
    network_policy_json: String,
    status: String,
    created_at: String,
    updated_at: String,
) -> NetworkPolicyResponse {
    NetworkPolicyResponse {
        id,
        name,
        country_anchor,
        region_anchor,
        allow_same_country_fallback: allow_same_country_fallback != 0,
        allow_same_region_fallback: allow_same_region_fallback != 0,
        provider_preference,
        allowed_regions_json: parse_optional_json_text(allowed_regions_json),
        network_policy_json: parse_json_text(Some(network_policy_json), json!({})),
        status,
        created_at,
        updated_at,
    }
}

fn map_continuity_policy_row(
    id: String,
    name: String,
    session_ttl_seconds: i64,
    heartbeat_interval_seconds: i64,
    site_group_mode: String,
    recovery_enabled: i64,
    protect_on_login_loss: i64,
    policy_json: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
) -> ContinuityPolicyResponse {
    ContinuityPolicyResponse {
        id,
        name,
        session_ttl_seconds,
        heartbeat_interval_seconds,
        site_group_mode,
        recovery_enabled: recovery_enabled != 0,
        protect_on_login_loss: protect_on_login_loss != 0,
        policy_json: parse_optional_json_text(policy_json),
        status,
        created_at,
        updated_at,
    }
}

fn map_platform_template_row(
    id: String,
    platform_id: String,
    name: String,
    warm_paths_json: String,
    revisit_paths_json: String,
    stateful_paths_json: String,
    write_operation_paths_json: String,
    high_risk_paths_json: String,
    allowed_regions_json: String,
    preferred_locale: Option<String>,
    preferred_timezone: Option<String>,
    continuity_checks_json: Option<String>,
    identity_markers_json: Option<String>,
    login_loss_signals_json: Option<String>,
    recovery_steps_json: Option<String>,
    behavior_defaults_json: Option<String>,
    event_chain_templates_json: Option<String>,
    page_semantics_json: Option<String>,
    readiness_level: String,
    status: String,
    created_at: String,
    updated_at: String,
) -> PlatformTemplateResponse {
    PlatformTemplateResponse {
        id,
        platform_id,
        name,
        warm_paths_json: parse_json_text(Some(warm_paths_json), json!([])),
        revisit_paths_json: parse_json_text(Some(revisit_paths_json), json!([])),
        stateful_paths_json: parse_json_text(Some(stateful_paths_json), json!([])),
        write_operation_paths_json: parse_json_text(Some(write_operation_paths_json), json!([])),
        high_risk_paths_json: parse_json_text(Some(high_risk_paths_json), json!([])),
        allowed_regions_json: parse_json_text(Some(allowed_regions_json), json!([])),
        preferred_locale,
        preferred_timezone,
        continuity_checks_json: parse_optional_json_text(continuity_checks_json),
        identity_markers_json: parse_optional_json_text(identity_markers_json),
        login_loss_signals_json: parse_optional_json_text(login_loss_signals_json),
        recovery_steps_json: parse_optional_json_text(recovery_steps_json),
        behavior_defaults_json: parse_optional_json_text(behavior_defaults_json),
        event_chain_templates_json: parse_optional_json_text(event_chain_templates_json),
        page_semantics_json: parse_optional_json_text(page_semantics_json),
        readiness_level,
        status,
        created_at,
        updated_at,
    }
}

fn map_store_platform_override_row(
    id: String,
    store_id: String,
    platform_id: String,
    admin_origin: Option<String>,
    entry_origin: Option<String>,
    entry_paths_json: Option<String>,
    warm_paths_json: Option<String>,
    revisit_paths_json: Option<String>,
    stateful_paths_json: Option<String>,
    high_risk_paths_json: Option<String>,
    recovery_steps_json: Option<String>,
    login_loss_signals_json: Option<String>,
    identity_markers_json: Option<String>,
    behavior_defaults_json: Option<String>,
    event_chain_templates_json: Option<String>,
    page_semantics_json: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
) -> StorePlatformOverrideResponse {
    StorePlatformOverrideResponse {
        id,
        store_id,
        platform_id,
        admin_origin,
        entry_origin,
        entry_paths_json: parse_optional_json_text(entry_paths_json),
        warm_paths_json: parse_optional_json_text(warm_paths_json),
        revisit_paths_json: parse_optional_json_text(revisit_paths_json),
        stateful_paths_json: parse_optional_json_text(stateful_paths_json),
        high_risk_paths_json: parse_optional_json_text(high_risk_paths_json),
        recovery_steps_json: parse_optional_json_text(recovery_steps_json),
        login_loss_signals_json: parse_optional_json_text(login_loss_signals_json),
        identity_markers_json: parse_optional_json_text(identity_markers_json),
        behavior_defaults_json: parse_optional_json_text(behavior_defaults_json),
        event_chain_templates_json: parse_optional_json_text(event_chain_templates_json),
        page_semantics_json: parse_optional_json_text(page_semantics_json),
        status,
        created_at,
        updated_at,
    }
}

fn map_behavior_profile_row(
    id: String,
    name: String,
    description: Option<String>,
    mouse_json: Option<String>,
    keyboard_json: Option<String>,
    scroll_json: Option<String>,
    dwell_json: Option<String>,
    navigation_json: Option<String>,
    input_json: Option<String>,
    action_preference_json: Option<String>,
    humanize_defaults_json: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
) -> BehaviorProfileResponse {
    BehaviorProfileResponse {
        id,
        name,
        description,
        mouse_json: parse_optional_json_text(mouse_json),
        keyboard_json: parse_optional_json_text(keyboard_json),
        scroll_json: parse_optional_json_text(scroll_json),
        dwell_json: parse_optional_json_text(dwell_json),
        navigation_json: parse_optional_json_text(navigation_json),
        input_json: parse_optional_json_text(input_json),
        action_preference_json: parse_optional_json_text(action_preference_json),
        humanize_defaults_json: parse_optional_json_text(humanize_defaults_json),
        status,
        created_at,
        updated_at,
    }
}

fn map_manual_gate_row(
    id: String,
    task_id: String,
    persona_id: Option<String>,
    store_id: Option<String>,
    platform_id: Option<String>,
    requested_action_kind: String,
    requested_url: Option<String>,
    reason_code: String,
    reason_summary: String,
    status: String,
    resolution_note: Option<String>,
    created_at: String,
    updated_at: String,
    resolved_at: Option<String>,
) -> ManualGateResponse {
    ManualGateResponse {
        id,
        task_id,
        persona_id,
        store_id,
        platform_id,
        requested_action_kind,
        requested_url,
        reason_code,
        reason_summary,
        status,
        resolution_note,
        created_at,
        updated_at,
        resolved_at,
    }
}

fn map_continuity_event_row(
    id: String,
    persona_id: Option<String>,
    store_id: Option<String>,
    platform_id: Option<String>,
    task_id: Option<String>,
    run_id: Option<String>,
    event_type: String,
    severity: String,
    event_json: Option<String>,
    created_at: String,
) -> ContinuityEventResponse {
    ContinuityEventResponse {
        id,
        persona_id,
        store_id,
        platform_id,
        task_id,
        run_id,
        event_type,
        severity,
        event_json: parse_optional_json_text(event_json),
        created_at,
    }
}

fn map_persona_health_snapshot_row(
    id: String,
    persona_id: String,
    store_id: String,
    platform_id: String,
    status: String,
    active_session_count: i64,
    continuity_score: f64,
    login_risk_count: i64,
    last_event_type: Option<String>,
    last_task_at: Option<String>,
    snapshot_json: Option<String>,
    created_at: String,
) -> PersonaHealthSnapshotResponse {
    PersonaHealthSnapshotResponse {
        id,
        persona_id,
        store_id,
        platform_id,
        status,
        active_session_count,
        continuity_score,
        login_risk_count,
        last_event_type,
        last_task_at,
        snapshot_json: parse_optional_json_text(snapshot_json),
        created_at,
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct PlatformTemplateRow {
    id: String,
    platform_id: String,
    name: String,
    warm_paths_json: String,
    revisit_paths_json: String,
    stateful_paths_json: String,
    write_operation_paths_json: String,
    high_risk_paths_json: String,
    allowed_regions_json: String,
    preferred_locale: Option<String>,
    preferred_timezone: Option<String>,
    continuity_checks_json: Option<String>,
    identity_markers_json: Option<String>,
    login_loss_signals_json: Option<String>,
    recovery_steps_json: Option<String>,
    behavior_defaults_json: Option<String>,
    event_chain_templates_json: Option<String>,
    page_semantics_json: Option<String>,
    readiness_level: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct StorePlatformOverrideRow {
    id: String,
    store_id: String,
    platform_id: String,
    admin_origin: Option<String>,
    entry_origin: Option<String>,
    entry_paths_json: Option<String>,
    warm_paths_json: Option<String>,
    revisit_paths_json: Option<String>,
    stateful_paths_json: Option<String>,
    high_risk_paths_json: Option<String>,
    recovery_steps_json: Option<String>,
    login_loss_signals_json: Option<String>,
    identity_markers_json: Option<String>,
    behavior_defaults_json: Option<String>,
    event_chain_templates_json: Option<String>,
    page_semantics_json: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(sqlx::FromRow)]
struct ProxyRow {

    id: String,
    scheme: String,
    host: String,
    port: i64,
    username: Option<String>,
    region: Option<String>,
    country: Option<String>,
    provider: Option<String>,
    status: String,
    score: f64,
    success_count: i64,
    failure_count: i64,
    last_checked_at: Option<String>,
    last_used_at: Option<String>,
    cooldown_until: Option<String>,
    last_smoke_status: Option<String>,
    last_smoke_protocol_ok: Option<i64>,
    last_smoke_upstream_ok: Option<i64>,
    last_exit_ip: Option<String>,
    last_anonymity_level: Option<String>,
    last_smoke_at: Option<String>,
    last_verify_status: Option<String>,
    last_verify_geo_match_ok: Option<i64>,
    last_exit_country: Option<String>,
    last_exit_region: Option<String>,
    last_verify_at: Option<String>,
    last_probe_latency_ms: Option<i64>,
    last_probe_error: Option<String>,
    last_probe_error_category: Option<String>,
    last_verify_confidence: Option<f64>,
    last_verify_score_delta: Option<i64>,
    last_verify_source: Option<String>,
    created_at: String,
    updated_at: String,
}


pub async fn run_proxy_verify_probe(
    state: &AppState,
    proxy_id: &str,
) -> Result<ProxyVerifyResponse, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, i64, Option<String>, Option<String>, String)>(
        r#"SELECT host, port, country, region, status FROM proxies WHERE id = ?"#,
    )
        .bind(proxy_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load proxy for verify: {err}")))?;

    let Some((host, port, expected_country, expected_region, prior_proxy_status)) = row else {
        return Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}")));
    };

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|err| (StatusCode::BAD_REQUEST, format!("proxy address is invalid for verify: {err}")))?;

    let started = Instant::now();
    let mut stream = tokio::time::timeout(std::time::Duration::from_secs(5), tokio::net::TcpStream::connect(addr))
        .await
        .ok()
        .and_then(|result| result.ok());
    let reachable = stream.is_some();
    let mut protocol_ok = false;
    let mut upstream_ok = false;
    let mut exit_ip: Option<String> = None;
    let mut exit_country: Option<String> = None;
    let mut exit_region: Option<String> = None;
    let mut geo_match_ok: Option<bool> = None;
    let mut region_match_ok: Option<bool> = None;
    let mut identity_fields_complete: Option<bool> = None;
    let mut exit_ip_public: Option<bool> = None;
    let mut anonymity_level: Option<String> = None;
    let mut invalid_identity_echo = false;
    let mut probe_error: Option<String> = if reachable { None } else { Some("proxy verify tcp connect failed".to_string()) };
    let mut probe_error_category: Option<String> = if reachable { None } else { Some("connect_failed".to_string()) };
    let mut verify_message = if reachable {
        "tcp connect succeeded but verify probe did not complete".to_string()
    } else {
        "proxy verify tcp connect failed".to_string()
    };

    if let Some(stream_ref) = stream.as_mut() {
        let probe = b"CONNECT verify.example:443 HTTP/1.1
Host: verify.example:443

";
        if tokio::time::timeout(std::time::Duration::from_secs(5), stream_ref.write_all(probe)).await.ok().is_some() {
            let mut buf = [0_u8; 1024];
            if let Ok(Ok(n)) = tokio::time::timeout(std::time::Duration::from_secs(5), stream_ref.read(&mut buf)).await {
                if n > 0 {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let text_lower = text.to_ascii_lowercase();
                    let is_http_response = text_lower.starts_with("http/1.1") || text_lower.starts_with("http/1.0");
                    let connect_established = is_http_response
                        && (text_lower.starts_with("http/1.1 200")
                            || text_lower.starts_with("http/1.0 200")
                            || text_lower.contains("connection established"));
                    if is_http_response {
                        protocol_ok = true;
                        let has_via = text_lower.contains("via:");
                        let has_forwarded = text_lower.contains("forwarded:") || text_lower.contains("x-forwarded-for:");
                        anonymity_level = Some(if has_forwarded { "transparent".to_string() } else if has_via { "anonymous".to_string() } else { "elite".to_string() });
                        let raw_exit_ip = parse_probe_field(&text, "ip");
                        exit_ip = raw_exit_ip.as_deref().filter(|v| looks_like_ip(v)).map(str::to_string);
                        invalid_identity_echo = raw_exit_ip.is_some() && exit_ip.is_none();
                        exit_country = parse_probe_field(&text, "country");
                        exit_region = parse_probe_field(&text, "region");
                        exit_ip_public = exit_ip.as_deref().map(ip_is_public);
                        identity_fields_complete = Some(exit_ip.is_some() && exit_country.is_some() && exit_region.is_some());
                        let identity_echo_ok =
                            identity_fields_complete.unwrap_or(false) && exit_ip_public != Some(false);
                        upstream_ok = identity_echo_ok || connect_established;
                        geo_match_ok = exit_country.as_ref().and_then(|actual| {
                            expected_country
                                .as_ref()
                                .map(|expected| actual.eq_ignore_ascii_case(expected))
                        });
                        region_match_ok = exit_region.as_ref().and_then(|actual| {
                            expected_region
                                .as_ref()
                                .map(|expected| actual.eq_ignore_ascii_case(expected))
                        });
                        verify_message = if connect_established && !identity_echo_ok {
                            format!(
                                "proxy verify established CONNECT tunnel without identity echo fields anonymity={:?}",
                                anonymity_level
                            )
                        } else {
                            format!(
                                "proxy verify completed ip={:?} public_ip={:?} country={:?} region={:?} region_match={:?}",
                                exit_ip, exit_ip_public, exit_country, exit_region, region_match_ok
                            )
                        };
                        probe_error = None;
                        probe_error_category = None;
                    } else {
                        verify_message = format!("proxy verify got non-http response: {text_lower}");
                        probe_error = Some(verify_message.clone());
                        probe_error_category = Some("protocol_invalid".to_string());
                    }
                }
            }
        }
    }

    if reachable && protocol_ok && invalid_identity_echo {
        upstream_ok = false;
        probe_error = Some("verify probe returned invalid exit ip".to_string());
        probe_error_category = Some("invalid_exit_ip".to_string());
    } else if reachable && protocol_ok && exit_ip_public == Some(false) {
        upstream_ok = false;
        probe_error = Some("verify probe reported non-public exit ip".to_string());
        probe_error_category = Some("exit_ip_not_public".to_string());
    } else if reachable && protocol_ok && !upstream_ok {
        probe_error = Some("verify probe did not receive upstream identity fields".to_string());
        probe_error_category = Some("upstream_missing".to_string());
    }
    let latency_ms = Some(started.elapsed().as_millis());
    let latency_ms_i64 = latency_ms.and_then(|v| i64::try_from(v).ok());
    let status = if reachable && protocol_ok && upstream_ok { "ok" } else { "failed" };
    let verification_confidence = Some(if reachable && protocol_ok && upstream_ok && geo_match_ok == Some(true) && region_match_ok != Some(false) && anonymity_level.as_deref() == Some("elite") {
        0.98
    } else if reachable && protocol_ok && upstream_ok && geo_match_ok == Some(true) && region_match_ok != Some(false) {
        0.95
    } else if reachable && protocol_ok && upstream_ok && geo_match_ok == Some(true) {
        0.86
    } else if reachable && protocol_ok && upstream_ok {
        0.68
    } else if reachable && protocol_ok {
        0.45
    } else if reachable {
        0.20
    } else {
        0.05
    });
    let verification_score_delta = Some(
        (if status == "ok" { 8 } else { -8 })
        + (if geo_match_ok == Some(true) { 4 } else if geo_match_ok == Some(false) { -4 } else { 0 })
        + (if region_match_ok == Some(true) { 2 } else if region_match_ok == Some(false) { -2 } else { 0 })
        + (if identity_fields_complete == Some(true) { 1 } else { -1 })
        + (if exit_ip_public == Some(true) { 1 } else if exit_ip_public == Some(false) { -3 } else { 0 })
        + match anonymity_level.as_deref() {
            Some("elite") => 2,
            Some("anonymous") => -1,
            Some("transparent") => -3,
            _ => 0,
        }
    );
    let (risk_level, risk_reasons) = compute_verify_risk_summary(
        reachable,
        protocol_ok,
        upstream_ok,
        geo_match_ok,
        region_match_ok,
        identity_fields_complete,
        exit_ip_public,
        anonymity_level.as_deref(),
        probe_error_category.as_deref(),
    );
    let (failure_stage, failure_stage_detail) = classify_verify_failure_stage(
        reachable,
        protocol_ok,
        upstream_ok,
        probe_error_category.as_deref(),
        &risk_reasons,
    );
    let verification_class = classify_verification_class(
        status,
        risk_level.as_deref(),
        failure_stage.as_deref(),
    );
    let recommended_action = recommend_verify_action(
        verification_class.as_deref(),
        risk_level.as_deref(),
        failure_stage.as_deref(),
        failure_stage_detail.as_deref(),
    );
    let verify_source = Some("local_verify".to_string());
    let now = now_ts_string();
    let candidate_cooldown_until = (now.parse::<i64>().unwrap_or(0) + 1800).to_string();
    let is_candidate_family = is_candidate_proxy_status(prior_proxy_status.as_str());
    let next_proxy_status = if is_candidate_family {
        if status == "ok" {
            "active"
        } else {
            "candidate_rejected"
        }
    } else {
        prior_proxy_status.as_str()
    };
    let next_promoted_at = if is_candidate_family && status == "ok" {
        Some(now.clone())
    } else {
        None
    };
    let next_cooldown_until = if is_candidate_family {
        if status == "ok" {
            None
        } else {
            Some(candidate_cooldown_until)
        }
    } else {
        None
    };
    let should_update_cooldown = if is_candidate_family { 1_i64 } else { 0_i64 };
    sqlx::query(r#"UPDATE proxies SET last_checked_at = ?, last_verify_status = ?, last_verify_geo_match_ok = ?, last_exit_ip = ?, last_exit_country = ?, last_exit_region = ?, last_anonymity_level = ?, last_verify_at = ?, last_probe_latency_ms = ?, last_probe_error = ?, last_probe_error_category = ?, last_verify_confidence = ?, last_verify_score_delta = ?, last_verify_source = ?, status = ?, promoted_at = COALESCE(?, promoted_at), cooldown_until = CASE WHEN ? != 0 THEN ? ELSE cooldown_until END, score = MAX(0.0, score + (? / 100.0)), updated_at = ? WHERE id = ?"#)
        .bind(&now)
        .bind(status)
        .bind(geo_match_ok.map(|v| if v { 1_i64 } else { 0_i64 }))
        .bind(&exit_ip)
        .bind(&exit_country)
        .bind(&exit_region)
        .bind(&anonymity_level)
        .bind(&now)
        .bind(&latency_ms_i64)
        .bind(&probe_error)
        .bind(&probe_error_category)
        .bind(verification_confidence)
        .bind(verification_score_delta)
        .bind(&verify_source)
        .bind(next_proxy_status)
        .bind(&next_promoted_at)
        .bind(should_update_cooldown)
        .bind(&next_cooldown_until)
        .bind(verification_score_delta.unwrap_or(0) as f64)
        .bind(&now)
        .bind(proxy_id)
        .execute(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to persist proxy verify result: {err}")))?;
    let provider_region = sqlx::query_as::<_, (Option<String>, Option<String>)>("SELECT provider, region FROM proxies WHERE id = ?")
        .bind(proxy_id)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load proxy provider/region after verify: {err}")))?;
    refresh_proxy_trust_views_for_scope(&state.db, proxy_id, provider_region.0.as_deref(), provider_region.1.as_deref())
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to refresh scoped trust views after verify: {err}")))?;

    Ok(ProxyVerifyResponse {
        id: proxy_id.to_string(),
        reachable,
        protocol_ok,
        upstream_ok,
        exit_ip,
        exit_country,
        exit_region,
        geo_match_ok,
        region_match_ok,
        identity_fields_complete,
        risk_level,
        risk_reasons,
        failure_stage,
        failure_stage_detail,
        anonymity_level,
        latency_ms,
        probe_error,
        probe_error_category,
        verification_confidence,
        verification_class,
        recommended_action,
        verification_score_delta,
        verify_source,
        status: status.to_string(),
        message: verify_message,
    })
}





fn recommend_verify_action(
    verification_class: Option<&str>,
    risk_level: Option<&str>,
    failure_stage: Option<&str>,
    failure_stage_detail: Option<&str>,
) -> Option<String> {
    if verification_class == Some("trusted") {
        return Some("use".to_string());
    }
    if verification_class == Some("conditional") {
        return Some("use_with_caution".to_string());
    }
    if verification_class == Some("rejected") {
        if matches!(failure_stage, Some("connect") | Some("protocol") | Some("identity")) {
            return Some("retry_later".to_string());
        }
        if matches!(failure_stage_detail, Some("transparent_proxy") | Some("exit_ip_not_public")) || risk_level == Some("high") {
            return Some("quarantine".to_string());
        }
        return Some("retry_later".to_string());
    }
    None
}

fn classify_verification_class(
    status: &str,
    risk_level: Option<&str>,
    failure_stage: Option<&str>,
) -> Option<String> {
    if status != "ok" {
        return Some("rejected".to_string());
    }
    if failure_stage.is_some() {
        return Some("rejected".to_string());
    }
    match risk_level {
        Some("low") => Some("trusted".to_string()),
        Some("medium") | Some("high") => Some("conditional".to_string()),
        _ => Some("conditional".to_string()),
    }
}

fn classify_verify_failure_stage(
    reachable: bool,
    protocol_ok: bool,
    upstream_ok: bool,
    probe_error_category: Option<&str>,
    risk_reasons: &[String],
) -> (Option<String>, Option<String>) {
    if !reachable {
        return (Some("connect".to_string()), Some("tcp_connect_failed".to_string()));
    }
    if reachable && !protocol_ok {
        return (Some("protocol".to_string()), Some(probe_error_category.unwrap_or("protocol_invalid").to_string()));
    }
    if probe_error_category == Some("exit_ip_not_public") {
        return (Some("risk".to_string()), Some("exit_ip_not_public".to_string()));
    }
    if !upstream_ok {
        return (Some("identity".to_string()), Some(probe_error_category.unwrap_or("upstream_missing").to_string()));
    }
    if risk_reasons.iter().any(|r| matches!(r.as_str(), "transparent_proxy" | "anonymous_proxy" | "geo_mismatch" | "region_mismatch")) {
        let detail = if risk_reasons.iter().any(|r| r == "transparent_proxy") {
            "transparent_proxy"
        } else if risk_reasons.iter().any(|r| r == "anonymous_proxy") {
            "anonymous_proxy"
        } else if risk_reasons.iter().any(|r| r == "geo_mismatch") {
            "geo_mismatch"
        } else {
            "region_mismatch"
        };
        return (Some("risk".to_string()), Some(detail.to_string()));
    }
    (None, None)
}

fn compute_verify_risk_summary(
    reachable: bool,
    protocol_ok: bool,
    upstream_ok: bool,
    geo_match_ok: Option<bool>,
    region_match_ok: Option<bool>,
    identity_fields_complete: Option<bool>,
    exit_ip_public: Option<bool>,
    anonymity_level: Option<&str>,
    probe_error_category: Option<&str>,
) -> (Option<String>, Vec<String>) {
    let mut reasons = Vec::new();
    if !reachable {
        reasons.push("connect_failed".to_string());
    }
    if reachable && !protocol_ok {
        reasons.push("protocol_invalid".to_string());
    }
    if probe_error_category == Some("exit_ip_not_public") || exit_ip_public == Some(false) {
        reasons.push("exit_ip_not_public".to_string());
    }
    if upstream_ok == false {
        reasons.push("identity_incomplete".to_string());
    } else if identity_fields_complete == Some(false) {
        reasons.push("identity_incomplete".to_string());
    }
    if geo_match_ok == Some(false) {
        reasons.push("geo_mismatch".to_string());
    }
    if region_match_ok == Some(false) {
        reasons.push("region_mismatch".to_string());
    }
    match anonymity_level {
        Some("transparent") => reasons.push("transparent_proxy".to_string()),
        Some("anonymous") => reasons.push("anonymous_proxy".to_string()),
        _ => {}
    }
    reasons.sort();
    reasons.dedup();

    let risk_level = if reasons.iter().any(|r| matches!(r.as_str(), "connect_failed" | "protocol_invalid" | "exit_ip_not_public" | "transparent_proxy")) {
        Some("high".to_string())
    } else if reasons.iter().any(|r| matches!(r.as_str(), "identity_incomplete" | "geo_mismatch" | "region_mismatch" | "anonymous_proxy")) {
        Some("medium".to_string())
    } else if reachable && protocol_ok && upstream_ok {
        Some("low".to_string())
    } else {
        None
    };

    (risk_level, reasons)
}

fn looks_like_ip(value: &str) -> bool {
    value.parse::<std::net::IpAddr>().is_ok()
}

fn ip_is_public(value: &str) -> bool {
    match value.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            !(ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_broadcast() || ip.is_documentation() || ip.is_unspecified())
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            !(ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local() || ip.is_unicast_link_local())
        }
        Err(_) => false,
    }
}

fn parse_probe_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let idx = text.find(&needle)?;
    let value = text[idx + needle.len()..].lines().next()?.trim();
    if value.is_empty() { None } else { Some(value.to_string()) }
}

fn map_proxy_row(row: ProxyRow) -> ProxyResponse {
    ProxyResponse {
        id: row.id,
        scheme: row.scheme,
        host: row.host,
        port: row.port,
        username: row.username,
        region: row.region,
        country: row.country,
        provider: row.provider,
        status: row.status,
        score: row.score,
        success_count: row.success_count,
        failure_count: row.failure_count,
        last_checked_at: row.last_checked_at,
        last_used_at: row.last_used_at,
        cooldown_until: row.cooldown_until,
        last_smoke_status: row.last_smoke_status,
        last_smoke_protocol_ok: row.last_smoke_protocol_ok.map(|v| v != 0),
        last_smoke_upstream_ok: row.last_smoke_upstream_ok.map(|v| v != 0),
        last_exit_ip: row.last_exit_ip,
        last_anonymity_level: row.last_anonymity_level,
        last_smoke_at: row.last_smoke_at,
        last_verify_status: row.last_verify_status,
        last_verify_geo_match_ok: row.last_verify_geo_match_ok.map(|v| v != 0),
        last_exit_country: row.last_exit_country,
        last_exit_region: row.last_exit_region,
        last_verify_at: row.last_verify_at,
        last_probe_latency_ms: row.last_probe_latency_ms,
        last_probe_error: row.last_probe_error,
        last_probe_error_category: row.last_probe_error_category,
        last_verify_confidence: row.last_verify_confidence,
        last_verify_score_delta: row.last_verify_score_delta,
        last_verify_source: row.last_verify_source,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

const PROXY_ROW_SELECT_SQL: &str = r#"SELECT id, scheme, host, port, username, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, last_probe_latency_ms, last_probe_error, last_probe_error_category, last_verify_confidence, last_verify_score_delta, last_verify_source, created_at, updated_at FROM proxies"#;

async fn load_proxy_row_by_id(
    state: &AppState,
    proxy_id: &str,
) -> Result<Option<ProxyRow>, (StatusCode, String)> {
    sqlx::query_as::<_, ProxyRow>(&format!("{PROXY_ROW_SELECT_SQL} WHERE id = ?"))
        .bind(proxy_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to fetch proxy: {err}"),
            )
        })
}

async fn load_proxy_row_by_endpoint_key(
    state: &AppState,
    payload: &CreateProxyRequest,
) -> Result<Option<ProxyRow>, (StatusCode, String)> {
    sqlx::query_as::<_, ProxyRow>(&format!(
        "{PROXY_ROW_SELECT_SQL}
         WHERE scheme = ?
           AND host = ?
           AND port = ?
           AND ((username IS NULL AND ? IS NULL) OR username = ?)
           AND ((provider IS NULL AND ? IS NULL) OR provider = ?)
           AND ((region IS NULL AND ? IS NULL) OR region = ?)
         ORDER BY
           CASE status
             WHEN 'active' THEN 0
             WHEN 'candidate' THEN 1
             WHEN 'candidate_rejected' THEN 2
             ELSE 3
           END ASC,
           created_at ASC
         LIMIT 1"
    ))
    .bind(&payload.scheme)
    .bind(&payload.host)
    .bind(payload.port)
    .bind(&payload.username)
    .bind(&payload.username)
    .bind(&payload.provider)
    .bind(&payload.provider)
    .bind(&payload.region)
    .bind(&payload.region)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch candidate proxy by endpoint key: {err}"),
        )
    })
}

async fn create_or_refresh_candidate_proxy(
    state: &AppState,
    payload: &CreateProxyRequest,
) -> Result<(StatusCode, Json<ProxyResponse>), (StatusCode, String)> {
    let now = now_ts_string();
    if let Some(existing) = load_proxy_row_by_endpoint_key(state, payload).await? {
        if existing.status == "active" {
            sqlx::query(
                r#"UPDATE proxies
                   SET last_seen_at = ?, source_label = COALESCE(?, source_label), updated_at = ?
                   WHERE id = ?"#,
            )
            .bind(&now)
            .bind("api_create_proxy")
            .bind(&now)
            .bind(&existing.id)
            .execute(&state.db)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to refresh active proxy candidate sighting: {err}"),
                )
            })?;
        } else {
            sqlx::query(
                r#"UPDATE proxies
                   SET last_seen_at = ?,
                       source_label = ?,
                       score = MAX(score, ?),
                       country = COALESCE(country, ?),
                       password = COALESCE(password, ?),
                       updated_at = ?
                   WHERE id = ?"#,
            )
            .bind(&now)
            .bind("api_create_proxy")
            .bind(payload.score.unwrap_or(1.0))
            .bind(&payload.country)
            .bind(&payload.password)
            .bind(&now)
            .bind(&existing.id)
            .execute(&state.db)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to refresh candidate proxy: {err}"),
                )
            })?;
        }

        let refreshed = load_proxy_row_by_id(state, &existing.id)
            .await?
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("proxy disappeared after candidate refresh: {}", existing.id),
                )
            })?;
        return Ok((StatusCode::OK, Json(map_proxy_row(refreshed))));
    }

    let status = payload
        .status
        .clone()
        .filter(|value| is_candidate_proxy_status(value))
        .unwrap_or_else(|| "candidate".to_string());
    let score = payload.score.unwrap_or(1.0);
    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider,
               status, score, success_count, failure_count, last_checked_at, last_used_at,
               cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok,
               last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status,
               last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at,
               last_probe_latency_ms, last_probe_error, last_probe_error_category,
               last_verify_confidence, last_verify_score_delta, last_verify_source,
               source_label, last_seen_at, promoted_at, created_at, updated_at
           ) VALUES (
               ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL,
               NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL,
               NULL, ?, ?, NULL, ?, ?
           )"#,
    )
    .bind(&payload.id)
    .bind(&payload.scheme)
    .bind(&payload.host)
    .bind(payload.port)
    .bind(&payload.username)
    .bind(&payload.password)
    .bind(&payload.region)
    .bind(&payload.country)
    .bind(&payload.provider)
    .bind(&status)
    .bind(score)
    .bind("api_create_proxy")
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create candidate proxy: {err}"),
        )
    })?;

    let created = load_proxy_row_by_id(state, &payload.id)
        .await?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("candidate proxy missing after create: {}", payload.id),
            )
        })?;
    Ok((StatusCode::CREATED, Json(map_proxy_row(created))))
}

fn build_proxy_metrics(tasks: &[TaskResponse]) -> ProxyMetricsResponse {
    let mut metrics = ProxyMetricsResponse {
        scope: "latest_tasks_window".to_string(),
        sample_size: i64::try_from(tasks.len()).unwrap_or(0),
        direct: 0,
        resolved: 0,
        resolved_sticky: 0,
        unresolved: 0,
        none: 0,
    };
    for task in tasks {
        match task.proxy_resolution_status.as_deref() {
            Some("direct") => metrics.direct += 1,
            Some("resolved") => metrics.resolved += 1,
            Some("resolved_sticky") => metrics.resolved_sticky += 1,
            Some("unresolved") => metrics.unresolved += 1,
            _ => metrics.none += 1,
        }
    }
    metrics
}

fn build_fingerprint_metrics(tasks: &[TaskResponse]) -> FingerprintMetricsResponse {
    let mut metrics = FingerprintMetricsResponse {
        scope: "latest_tasks_window".to_string(),
        sample_size: i64::try_from(tasks.len()).unwrap_or(0),
        pending: 0,
        resolved: 0,
        downgraded: 0,
        none: 0,
    };

    for task in tasks {
        match task.fingerprint_resolution_status.as_deref() {
            Some("pending") => metrics.pending += 1,
            Some("resolved") => metrics.resolved += 1,
            Some("downgraded") => metrics.downgraded += 1,
            _ => metrics.none += 1,
        }
    }

    metrics
}

async fn insert_task_log(
    state: &AppState,
    task_id: &str,
    run_id: Option<&str>,
    level: &str,
    message: &str,
) -> Result<(), (StatusCode, String)> {
    let log_id = format!("log-{}", Uuid::new_v4());
    let created_at = now_ts_string();
    sqlx::query(
        r#"
        INSERT INTO logs (id, task_id, run_id, level, message, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(log_id)
    .bind(task_id)
    .bind(run_id)
    .bind(level)
    .bind(message)
    .bind(created_at)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to insert cancel log: {err}"),
        )
    })?;
    Ok(())
}

async fn load_counts(state: &AppState) -> Result<TaskStatusCounts, (StatusCode, String)> {
    let (total, pending, queued, running, succeeded, failed, timed_out, cancelled): (i64, i64, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            r#"SELECT
                   COUNT(*) AS total,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS pending,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS queued,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS running,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS succeeded,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS failed,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS timed_out,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS cancelled
               FROM tasks"#,
        )
        .bind(TASK_STATUS_PENDING)
        .bind(TASK_STATUS_QUEUED)
        .bind(TASK_STATUS_RUNNING)
        .bind(TASK_STATUS_SUCCEEDED)
        .bind(TASK_STATUS_FAILED)
        .bind(TASK_STATUS_TIMED_OUT)
        .bind(TASK_STATUS_CANCELLED)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate task counts: {err}")))?;

    Ok(TaskStatusCounts { total, pending, queued, running, succeeded, failed, timed_out, cancelled })
}



async fn map_verify_batch_row(
    state: &AppState,
    id: String,
    status: String,
    requested_count: i64,
    accepted_count: i64,
    skipped_count: i64,
    stale_after_seconds: i64,
    task_timeout_seconds: i64,
    provider_summary_json: Option<String>,
    filters_json: Option<String>,
    created_at: String,
    updated_at: String,
) -> Result<VerifyBatchResponse, (StatusCode, String)> {
    let (queued_count, running_count, succeeded_count, failed_count): (i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
               COALESCE(SUM(CASE WHEN status = 'queued' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN status = 'running' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN status = 'succeeded' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN status IN ('failed', 'timed_out', 'cancelled') THEN 1 ELSE 0 END), 0)
           FROM tasks
           WHERE kind = 'verify_proxy' AND json_extract(input_json, '$.verify_batch_id') = ?"#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate verify batch task counts: {err}")))?;

    let derived_status = if accepted_count == 0 {
        status.clone()
    } else if queued_count > 0 || running_count > 0 {
        "running".to_string()
    } else if succeeded_count + failed_count >= accepted_count {
        "completed".to_string()
    } else {
        status.clone()
    };

    Ok(VerifyBatchResponse {
        id,
        status: derived_status,
        requested_count,
        accepted_count,
        skipped_count,
        queued_count,
        running_count,
        succeeded_count,
        failed_count,
        stale_after_seconds,
        task_timeout_seconds,
        provider_summary_json: provider_summary_json.and_then(|v| serde_json::from_str(&v).ok()),
        filters_json: filters_json.and_then(|v| serde_json::from_str(&v).ok()),
        created_at,
        updated_at,
    })
}

async fn load_verify_metrics(state: &AppState) -> Result<VerifyMetricsResponse, (StatusCode, String)> {
    let (verified_ok, verified_failed, geo_match_ok, stale_or_missing_verify): (i64, i64, i64, i64) =
        sqlx::query_as(
            r#"SELECT
                   COALESCE(SUM(CASE WHEN last_verify_status = 'ok' THEN 1 ELSE 0 END), 0) AS verified_ok,
                   COALESCE(SUM(CASE WHEN last_verify_status = 'failed' THEN 1 ELSE 0 END), 0) AS verified_failed,
                   COALESCE(SUM(CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 1 ELSE 0 END), 0) AS geo_match_ok,
                   COALESCE(SUM(CASE WHEN last_verify_at IS NULL OR last_verify_status IS NULL THEN 1 ELSE 0 END), 0) AS stale_or_missing_verify
               FROM proxies"#,
        )
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate verify metrics: {err}")))?;

    Ok(VerifyMetricsResponse { verified_ok, verified_failed, geo_match_ok, stale_or_missing_verify })
}

async fn load_proxy_pool_status_summary(
    state: &AppState,
) -> Result<crate::api::dto::ProxyPoolStatusSummary, (StatusCode, String)> {
    let now_ts = now_ts_string().parse::<i64>().unwrap_or_default();
    let mode = state.proxy_runtime_mode.clone();
    let rows = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"SELECT
               p.status,
               p.source_label,
               s.source_tier,
               s.for_demo,
               s.for_prod,
               p.region,
               s.quarantine_until
           FROM proxies p
           LEFT JOIN proxy_harvest_sources s ON s.source_label = p.source_label"#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to aggregate proxy pool status: {err}"),
        )
    })?;
    let mut total = 0_i64;
    let mut active = 0_i64;
    let mut candidate = 0_i64;
    let mut candidate_rejected = 0_i64;
    let mut active_by_source = std::collections::BTreeMap::<String, i64>::new();
    let mut active_by_region = std::collections::BTreeMap::<String, i64>::new();
    for (status, source_label, source_tier, for_demo, for_prod, region, quarantine_until) in rows {
        let metadata = crate::network_identity::proxy_harvest::ProxySourceRuntimeMetadata {
            source_label: source_label.clone(),
            source_tier,
            for_demo: for_demo.unwrap_or(1) != 0,
            for_prod: for_prod.unwrap_or(0) != 0,
            validation_mode: None,
            expected_geo_quality: None,
            cost_class: None,
            quarantine_until,
        };
        if !crate::network_identity::proxy_harvest::proxy_source_is_eligible_for_mode(
            &mode,
            &metadata,
            Some(now_ts),
        ) {
            continue;
        }
        total += 1;
        match status.as_str() {
            "active" => {
                active += 1;
                let source_key = source_label
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .unwrap_or("__unknown_source__")
                    .to_string();
                *active_by_source.entry(source_key).or_insert(0) += 1;
                if let Some(region_key) = normalize_optional_task_text(region.as_deref()) {
                    *active_by_region.entry(region_key).or_insert(0) += 1;
                }
            }
            "candidate" => candidate += 1,
            "candidate_rejected" => candidate_rejected += 1,
            _ => {}
        }
    }
    let hot_regions = load_hot_browser_regions(state).await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load hot regions: {err}"),
        )
    })?;
    let recent_hot_rows =
        load_recent_hot_browser_regions(state, HOT_REGION_WINDOW_SECONDS, HOT_REGION_LIMIT)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to load recent hot regions: {err}"),
                )
            })?;
    let recent_hot_regions = recent_hot_rows
        .iter()
        .map(|(region, _)| region.clone())
        .collect::<Vec<_>>();
    let recent_hot_region_counts = recent_hot_rows
        .iter()
        .map(|(region, count)| (region.clone(), *count))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut source_counts_desc = active_by_source.values().copied().collect::<Vec<_>>();
    source_counts_desc.sort_by(|left, right| right.cmp(left));
    let source_top1 = source_counts_desc.first().copied().unwrap_or(0);
    let source_top3 = source_counts_desc.iter().take(3).sum::<i64>();
    let active_sources_with_min_inventory = i64::try_from(
        active_by_source
            .values()
            .filter(|count| **count >= ACTIVE_INVENTORY_MIN)
            .count(),
    )
    .unwrap_or(0);
    let active_regions_with_min_inventory = i64::try_from(
        active_by_region
            .values()
            .filter(|count| **count >= ACTIVE_INVENTORY_MIN)
            .count(),
    )
    .unwrap_or(0);
    let policy = proxy_pool_growth_policy_from_env();
    let mut region_shortages = Vec::new();
    let shortage_basis = if recent_hot_regions.is_empty() {
        &hot_regions
    } else {
        &recent_hot_regions
    };
    for region in shortage_basis {
        let available_in_region = active_by_region.get(region).copied().unwrap_or(0);
        if available_in_region < policy.min_available_per_region {
            region_shortages.push(region.clone());
        }
    }
    Ok(crate::api::dto::ProxyPoolStatusSummary {
        mode,
        total,
        active,
        candidate,
        candidate_rejected,
        active_ratio_percent: if total <= 0 {
            0.0
        } else {
            round_percent(active, total)
        },
        reported_active_ratio_percent: if total <= 0 {
            0.0
        } else {
            round_percent(active, total)
        },
        effective_active_ratio_percent: if total <= 0 {
            0.0
        } else {
            round_percent(active, total)
        },
        active_ratio_percent_effective: if total <= 0 {
            0.0
        } else {
            round_percent(active, total)
        },
        active_ratio_percent_derived: if total <= 0 {
            0.0
        } else {
            round_percent(active, total)
        },
        eligible_pool_total: total,
        fresh_candidate_total: candidate,
        recent_rejected_total: candidate_rejected,
        hot_regions,
        recent_hot_regions,
        recent_hot_region_counts,
        hot_region_window_seconds: HOT_REGION_WINDOW_SECONDS,
        source_concentration_top1_percent: round_percent(source_top1, active),
        source_concentration_top3_percent: round_percent(source_top3, active),
        active_sources_with_min_inventory,
        active_regions_with_min_inventory,
        region_shortages,
    })
}

async fn load_proxy_replenish_metrics(
    state: &AppState,
) -> Result<crate::api::dto::ProxyReplenishMetricsSummary, (StatusCode, String)> {
    let recent_batches: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM verify_batches WHERE json_extract(filters_json, '$.reason') = 'replenish_mvp'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count replenish batches: {err}")))?;
    let promoted_active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proxies WHERE status = 'active' AND promoted_at IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count promoted proxies: {err}")))?;
    let rejected_total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proxies WHERE status = 'candidate_rejected'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count rejected proxies: {err}")))?;
    let fallback_total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM tasks
           WHERE result_json IS NOT NULL
             AND json_extract(result_json, '$.payload.network_policy_json.selection_explain.fallback_reason') = 'region_shortage_fallback_to_any_active'"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count fallback tasks: {err}")))?;
    let decision_total = promoted_active + rejected_total;
    Ok(crate::api::dto::ProxyReplenishMetricsSummary {
        recent_batches,
        promotion_rate: if decision_total == 0 { 0.0 } else { promoted_active as f64 / decision_total as f64 },
        reject_rate: if decision_total == 0 { 0.0 } else { rejected_total as f64 / decision_total as f64 },
        fallback_rate: if counts_like_browser_total(state).await? == 0 {
            0.0
        } else {
            fallback_total as f64 / counts_like_browser_total(state).await? as f64
        },
    })
}

async fn counts_like_browser_total(state: &AppState) -> Result<i64, (StatusCode, String)> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE kind IN ('open_page', 'get_html', 'get_title', 'get_final_url', 'extract_text')",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count browser tasks: {err}")))
}

async fn load_identity_session_metrics(
    state: &AppState,
) -> Result<crate::api::dto::IdentitySessionMetricsSummary, (StatusCode, String)> {
    let active_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM proxy_session_bindings")
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count sessions: {err}")))?;
    let reused_sessions: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM tasks WHERE result_json IS NOT NULL AND json_extract(result_json, '$.identity_session_status') = 'auto_reused'"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count reused sessions: {err}")))?;
    let created_sessions: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM tasks WHERE result_json IS NOT NULL AND json_extract(result_json, '$.identity_session_status') = 'auto_created'"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count created sessions: {err}")))?;
    let cookie_restore_count: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CAST(json_extract(result_json, '$.cookie_restore_count') AS INTEGER)), 0) FROM tasks WHERE result_json IS NOT NULL"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate cookie restore count: {err}")))?;
    let cookie_persist_count: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CAST(json_extract(result_json, '$.cookie_persist_count') AS INTEGER)), 0) FROM tasks WHERE result_json IS NOT NULL"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate cookie persist count: {err}")))?;
    let local_storage_restore_count: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CAST(json_extract(result_json, '$.local_storage_restore_count') AS INTEGER)), 0) FROM tasks WHERE result_json IS NOT NULL"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate local storage restore count: {err}")))?;
    let local_storage_persist_count: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CAST(json_extract(result_json, '$.local_storage_persist_count') AS INTEGER)), 0) FROM tasks WHERE result_json IS NOT NULL"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate local storage persist count: {err}")))?;
    let session_storage_restore_count: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CAST(json_extract(result_json, '$.session_storage_restore_count') AS INTEGER)), 0) FROM tasks WHERE result_json IS NOT NULL"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate session storage restore count: {err}")))?;
    let session_storage_persist_count: i64 = sqlx::query_scalar(
        r#"SELECT COALESCE(SUM(CAST(json_extract(result_json, '$.session_storage_persist_count') AS INTEGER)), 0) FROM tasks WHERE result_json IS NOT NULL"#,
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate session storage persist count: {err}")))?;
    Ok(crate::api::dto::IdentitySessionMetricsSummary {
        active_sessions,
        reused_sessions,
        created_sessions,
        cookie_restore_count,
        cookie_persist_count,
        local_storage_restore_count,
        local_storage_persist_count,
        session_storage_restore_count,
        session_storage_persist_count,
    })
}

async fn load_proxy_site_metrics(
    state: &AppState,
) -> Result<crate::api::dto::ProxySiteMetricsSummary, (StatusCode, String)> {
    let tracked_sites: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT site_key) FROM proxy_site_stats WHERE site_key IS NOT NULL AND site_key != ''",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count tracked proxy sites: {err}")))?;
    let site_records: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM proxy_site_stats")
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count proxy site records: {err}")))?;
    let top_failing_rows = sqlx::query_as::<_, (String, i64)>(
        r#"SELECT site_key, SUM(failure_count) AS failures
           FROM proxy_site_stats
           GROUP BY site_key
           HAVING failures > 0
           ORDER BY failures DESC, site_key ASC
           LIMIT 5"#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load top failing proxy sites: {err}")))?;
    Ok(crate::api::dto::ProxySiteMetricsSummary {
        tracked_sites,
        site_records,
        top_failing_sites: top_failing_rows
            .into_iter()
            .map(|(site_key, failures)| format!("{site_key}:{failures}"))
            .collect(),
    })
}

async fn load_continuity_event_metrics(
    state: &AppState,
) -> Result<ContinuityEventMetricsResponse, (StatusCode, String)> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM continuity_events")
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count continuity events: {err}")))?;
    let latest_window_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM continuity_events WHERE CAST(created_at AS INTEGER) >= CAST(strftime('%s','now') AS INTEGER) - 86400",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count recent continuity events: {err}")))?;
    let broken_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM continuity_events WHERE event_type IN ('continuity_broken', 'login_risk_detected')",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count broken continuity events: {err}")))?;
    let manual_gate_pending_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM manual_gate_requests WHERE status = 'pending'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count pending manual gates: {err}")))?;

    Ok(ContinuityEventMetricsResponse {
        total,
        latest_window_count,
        broken_count,
        manual_gate_pending_count,
    })
}

async fn load_persona_status_overview(
    state: &AppState,
) -> Result<PersonaStatusOverviewResponse, (StatusCode, String)> {
    let active_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM persona_profiles WHERE status = 'active'")
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count active personas: {err}")))?;
    let degraded_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM persona_profiles WHERE status = 'degraded'")
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count degraded personas: {err}")))?;
    let frozen_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM persona_profiles WHERE status = 'frozen'")
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count frozen personas: {err}")))?;

    Ok(PersonaStatusOverviewResponse {
        active_count,
        degraded_count,
        frozen_count,
    })
}

async fn load_latest_continuity_events(
    state: &AppState,
    limit: i64,
) -> Result<Vec<ContinuityEventResponse>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (
        String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>,
        String, String, Option<String>, String,
    )>(
        r#"SELECT id, persona_id, store_id, platform_id, task_id, run_id, event_type, severity, event_json, created_at
           FROM continuity_events
           ORDER BY created_at DESC, id DESC
           LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load latest continuity events: {err}")))?;

    Ok(rows
        .into_iter()
        .map(|(id, persona_id, store_id, platform_id, task_id, run_id, event_type, severity, event_json, created_at)| {
            map_continuity_event_row(
                id,
                persona_id,
                store_id,
                platform_id,
                task_id,
                run_id,
                event_type,
                severity,
                event_json,
                created_at,
            )
        })
        .collect())
}

async fn load_latest_persona_health_snapshots(
    state: &AppState,
    limit: i64,
) -> Result<Vec<PersonaHealthSnapshotResponse>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (
        String, String, String, String, String, i64, f64, i64, Option<String>, Option<String>, Option<String>, String,
    )>(
        r#"SELECT id, persona_id, store_id, platform_id, status, active_session_count, continuity_score,
                  login_risk_count, last_event_type, last_task_at, snapshot_json, created_at
           FROM persona_health_snapshots
           ORDER BY created_at DESC, id DESC
           LIMIT ?"#,
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load latest persona health snapshots: {err}")))?;

    Ok(rows
        .into_iter()
        .map(|(id, persona_id, store_id, platform_id, status, active_session_count, continuity_score, login_risk_count, last_event_type, last_task_at, snapshot_json, created_at)| {
            map_persona_health_snapshot_row(
                id,
                persona_id,
                store_id,
                platform_id,
                status,
                active_session_count,
                continuity_score,
                login_risk_count,
                last_event_type,
                last_task_at,
                snapshot_json,
                created_at,
            )
        })
        .collect())
}

pub async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    let counts = load_counts(&state).await?;
    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        service: "PersonaPilot".to_string(),
        queue_len: counts.queued as usize,
        counts,
    }))
}

pub async fn status(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    let started = Instant::now();
    let counts = load_counts(&state).await?;
    let limit = sanitize_limit(query.limit, 5, 100);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (
        String, String, String, i32, Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>,
    )>(
        r#"SELECT
               t.id,
               t.kind,
               t.status,
               t.priority,
               t.persona_id,
               t.platform_id,
               t.manual_gate_request_id,
               mg.status,
               t.fingerprint_profile_id,
               t.fingerprint_profile_version,
               t.result_json,
               t.started_at,
               t.finished_at
           FROM tasks t
           LEFT JOIN manual_gate_requests mg ON mg.id = t.manual_gate_request_id
           ORDER BY t.created_at DESC, t.id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch latest tasks: {err}"),
        )
    })?;

    let latest_tasks: Vec<TaskResponse> = rows
        .into_iter()
        .map(|(id, kind, status, priority, persona_id, platform_id, manual_gate_request_id, manual_gate_status, fingerprint_profile_id, fingerprint_profile_version, result_json, started_at, finished_at)| {
            build_task_response_from_row(
                id,
                kind,
                status,
                priority,
                persona_id,
                platform_id,
                manual_gate_request_id,
                manual_gate_status,
                fingerprint_profile_id,
                fingerprint_profile_version,
                result_json,
                started_at,
                finished_at,
            )
        })
        .collect();

    let fingerprint_metrics = build_fingerprint_metrics(&latest_tasks);
    let proxy_metrics = build_proxy_metrics(&latest_tasks);
    let verify_metrics = load_verify_metrics(&state).await?;
    let proxy_pool_status = load_proxy_pool_status_summary(&state).await?;
    let proxy_replenish_metrics = load_proxy_replenish_metrics(&state).await?;
    let proxy_harvest_metrics = load_proxy_harvest_metrics(&state)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load proxy harvest metrics: {err}")))?;
    let proxy_site_metrics = load_proxy_site_metrics(&state).await?;
    let identity_session_metrics = load_identity_session_metrics(&state).await?;
    let continuity_event_metrics = load_continuity_event_metrics(&state).await?;
    let persona_overview = load_persona_status_overview(&state).await?;
    let latest_execution_summaries = latest_execution_summaries(&latest_tasks);
    let latest_continuity_events = load_latest_continuity_events(&state, 10).await?;
    let latest_persona_health_snapshots = load_latest_persona_health_snapshots(&state, 10).await?;
    let latest_browser_tasks = latest_browser_ready_tasks(&latest_tasks, 3);

    let response = StatusResponse {
        service: "PersonaPilot".to_string(),
        mode: crate::network_identity::proxy_harvest::proxy_runtime_mode_from_env(),
        queue_len: counts.queued as usize,
        counts,
        worker: WorkerStatusResponse {
            worker_count: state.worker_count,
            queue_mode: "db_first_with_memory_compat".to_string(),
            reclaim_after_seconds: crate::runner::runner_reclaim_seconds_from_env(),
            heartbeat_interval_seconds: crate::runner::runner_heartbeat_interval_seconds_from_env(),
            claim_retry_limit: crate::runner::runner_claim_retry_limit_from_env(),
            idle_backoff_min_ms: crate::runner::runner_idle_backoff_min_ms_from_env(),
            idle_backoff_max_ms: crate::runner::runner_idle_backoff_max_ms_from_env(),
            fingerprint_medium_max_concurrency: state.worker_count.max(2),
            fingerprint_heavy_max_concurrency: state.worker_count.clamp(1, 2),
        },
        fingerprint_metrics,
        proxy_metrics,
        verify_metrics,
        proxy_pool_status,
        proxy_replenish_metrics,
        proxy_harvest_metrics,
        proxy_site_metrics,
        identity_session_metrics,
        continuity_event_metrics,
        heartbeat_metrics: HeartbeatMetricsResponse {
            evaluated_count: 0,
            scheduled_count: 0,
            skipped_count: 0,
            failed_count: 0,
            skip_active_task_count: 0,
            skip_recent_activity_count: 0,
            skip_template_missing_count: 0,
            skip_no_target_count: 0,
            skip_high_risk_count: 0,
            skip_origin_unresolved_count: 0,
        },
        persona_overview,
        latest_execution_summaries,
        latest_continuity_events,
        latest_persona_health_snapshots,
        latest_tasks,
        latest_browser_tasks,
    };
    perf_probe_log(
        "api_status",
        &[("elapsed_ms", started.elapsed().as_millis().to_string()), ("latest_task_count", response.latest_tasks.len().to_string()), ("latest_summary_count", response.latest_execution_summaries.len().to_string())],
    );
    Ok(Json(response))
}

async fn load_cached_trust_score_row(
    state: &AppState,
    proxy_id: &str,
) -> Result<Option<(Option<i64>, Option<String>)>, (StatusCode, String)> {
    sqlx::query_as::<_, (Option<i64>, Option<String>)>(
        "SELECT cached_trust_score, trust_score_cached_at FROM proxies WHERE id = ?"
    )
    .bind(proxy_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load cached trust score: {err}")))
}

pub async fn check_proxy_trust_cache(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxyTrustCacheCheckResponse>, (StatusCode, String)> {
    let now = now_ts_string();
    let cached_row = load_cached_trust_score_row(&state, &proxy_id).await?;
    let Some((cached_trust_score, cached_at)) = cached_row else {
        return Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}")));
    };

    let recomputed_trust_score = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT CAST(({}) AS INTEGER) FROM proxies WHERE id = ?",
        crate::network_identity::proxy_selection::proxy_trust_score_sql_with_tuning(&state.proxy_selection_tuning)
    ))
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&proxy_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to recompute trust score: {err}")))?;

    let delta = match (cached_trust_score, recomputed_trust_score) {
        (Some(cached), Some(recomputed)) => Some(recomputed - cached),
        _ => None,
    };
    let in_sync = delta.unwrap_or(0) == 0;

    Ok(Json(ProxyTrustCacheCheckResponse {
        proxy_id,
        cached_trust_score,
        recomputed_trust_score,
        delta,
        in_sync,
        cached_at,
    }))
}

async fn collect_trust_cache_scan_items(
    state: &AppState,
) -> Result<Vec<ProxyTrustCacheScanItem>, (StatusCode, String)> {
    let now = now_ts_string();
    let trust_sql = crate::network_identity::proxy_selection::proxy_trust_score_sql_with_tuning(&state.proxy_selection_tuning);
    let rows = sqlx::query_as::<_, (String, Option<String>, Option<i64>, Option<String>, Option<i64>)>(&format!(
        "SELECT id, provider, cached_trust_score, trust_score_cached_at, CAST(({}) AS INTEGER) FROM proxies ORDER BY created_at ASC, id ASC",
        trust_sql
    ))
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to scan trust cache: {err}")))?;

    Ok(rows.into_iter().map(|(proxy_id, provider, cached_trust_score, cached_at, recomputed_trust_score)| {
        let delta = match (cached_trust_score, recomputed_trust_score) {
            (Some(cached), Some(recomputed)) => Some(recomputed - cached),
            _ => None,
        };
        let in_sync = delta.unwrap_or(0) == 0;
        ProxyTrustCacheScanItem {
            proxy_id,
            provider,
            cached_trust_score,
            recomputed_trust_score,
            delta,
            in_sync,
            cached_at,
        }
    }).collect())
}

fn apply_trust_cache_scan_filters(
    mut items: Vec<ProxyTrustCacheScanItem>,
    query: &ProxyTrustCacheScanQuery,
) -> Vec<ProxyTrustCacheScanItem> {
    if query.only_drifted.unwrap_or(false) {
        items.retain(|item| !item.in_sync);
    }
    if let Some(provider) = query.provider.as_deref() {
        items.retain(|item| item.provider.as_deref() == Some(provider));
    }
    if let Some(limit) = query.limit {
        items.truncate(limit);
    }
    items
}

pub async fn scan_proxy_trust_cache(
    State(state): State<AppState>,
    Query(query): Query<ProxyTrustCacheScanQuery>,
) -> Result<Json<ProxyTrustCacheScanResponse>, (StatusCode, String)> {
    let items = apply_trust_cache_scan_filters(collect_trust_cache_scan_items(&state).await?, &query);
    let drifted = items.iter().filter(|item| !item.in_sync).count();
    Ok(Json(ProxyTrustCacheScanResponse {
        total: items.len(),
        drifted,
        items,
    }))
}

pub async fn maintain_proxy_trust_cache(
    State(state): State<AppState>,
    Query(query): Query<ProxyTrustCacheScanQuery>,
) -> Result<Json<ProxyTrustCacheMaintenanceResponse>, (StatusCode, String)> {
    let before = apply_trust_cache_scan_filters(collect_trust_cache_scan_items(&state).await?, &query);
    let drifted_before = before.iter().filter(|item| !item.in_sync).count();
    let mut repaired = 0usize;
    for item in before.iter().filter(|item| !item.in_sync) {
        refresh_cached_trust_score_for_proxy(&state.db, &item.proxy_id)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to maintain cached trust score for {}: {err}", item.proxy_id)))?;
        repaired += 1;
    }
    let after = collect_trust_cache_scan_items(&state).await?;
    let remaining_drifted = after.iter().filter(|item| !item.in_sync).count();
    Ok(Json(ProxyTrustCacheMaintenanceResponse {
        scanned_before: before.len(),
        drifted_before,
        repaired,
        remaining_drifted,
        ok: remaining_drifted == 0,
    }))
}

pub async fn repair_proxy_trust_cache_batch(
    State(state): State<AppState>,
    Query(query): Query<ProxyTrustCacheScanQuery>,
) -> Result<Json<ProxyTrustCacheRepairBatchResponse>, (StatusCode, String)> {
    let before = apply_trust_cache_scan_filters(collect_trust_cache_scan_items(&state).await?, &query);
    let mut repaired = 0usize;
    for item in before.iter().filter(|item| !item.in_sync) {
        refresh_cached_trust_score_for_proxy(&state.db, &item.proxy_id)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to repair cached trust score for {}: {err}", item.proxy_id)))?;
        repaired += 1;
    }
    let after = collect_trust_cache_scan_items(&state).await?;
    let remaining_drifted = after.iter().filter(|item| !item.in_sync).count();
    Ok(Json(ProxyTrustCacheRepairBatchResponse {
        scanned: before.len(),
        repaired,
        remaining_drifted,
        items: after,
    }))
}

pub async fn repair_proxy_trust_cache(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxyTrustCacheRepairResponse>, (StatusCode, String)> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM proxies WHERE id = ?")
        .bind(&proxy_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to check proxy existence: {err}")))?;
    if exists.is_none() {
        return Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}")));
    }

    refresh_cached_trust_score_for_proxy(&state.db, &proxy_id)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to refresh cached trust score: {err}")))?;

    let now = now_ts_string();
    let cached_row = load_cached_trust_score_row(&state, &proxy_id)
        .await?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("proxy not found after repair: {proxy_id}")))?;

    let recomputed_trust_score = sqlx::query_scalar::<_, i64>(&format!(
        "SELECT CAST(({}) AS INTEGER) FROM proxies WHERE id = ?",
        crate::network_identity::proxy_selection::proxy_trust_score_sql_with_tuning(&state.proxy_selection_tuning)
    ))
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&proxy_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to recompute trust score after repair: {err}")))?;

    let delta = match (cached_row.0, recomputed_trust_score) {
        (Some(cached), Some(recomputed)) => Some(recomputed - cached),
        _ => None,
    };
    let in_sync = delta.unwrap_or(0) == 0;

    Ok(Json(ProxyTrustCacheRepairResponse {
        proxy_id,
        cached_trust_score: cached_row.0,
        recomputed_trust_score,
        delta,
        in_sync,
        repaired: true,
        cached_at: cached_row.1,
    }))
}

pub async fn explain_proxy_selection(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxySelectionExplainResponse>, (StatusCode, String)> {
    let started = Instant::now();
    let now = now_ts_string();
    let row = sqlx::query(
        "SELECT id, provider, region, score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, last_verify_confidence, last_verify_score_delta, last_verify_source, last_anonymity_level, last_probe_latency_ms, last_probe_error_category, last_exit_region FROM proxies WHERE id = ?"
    )
    .bind(&proxy_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load proxy explain row: {err}")))?;
    let Some(row) = row else {
        return Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}")));
    };
    let id: String = row.try_get("id").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode id: {err}")))?;
    let provider: Option<String> = row.try_get("provider").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode provider: {err}")))?;
    let region: Option<String> = row.try_get("region").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode region: {err}")))?;
    let score: f64 = row.try_get("score").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode score: {err}")))?;
    let success_count: i64 = row.try_get("success_count").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode success_count: {err}")))?;
    let failure_count: i64 = row.try_get("failure_count").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode failure_count: {err}")))?;
    let last_verify_status: Option<String> = row.try_get("last_verify_status").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_verify_status: {err}")))?;
    let last_verify_geo_match_ok: Option<i64> = row.try_get("last_verify_geo_match_ok").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_verify_geo_match_ok: {err}")))?;
    let last_smoke_upstream_ok: Option<i64> = row.try_get("last_smoke_upstream_ok").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_smoke_upstream_ok: {err}")))?;
    let last_verify_at: Option<String> = row.try_get("last_verify_at").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_verify_at: {err}")))?;
    let last_verify_confidence: Option<f64> = row.try_get("last_verify_confidence").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_verify_confidence: {err}")))?;
    let last_verify_score_delta: Option<i64> = row.try_get("last_verify_score_delta").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_verify_score_delta: {err}")))?;
    let last_verify_source: Option<String> = row.try_get("last_verify_source").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_verify_source: {err}")))?;
    let last_anonymity_level: Option<String> = row.try_get("last_anonymity_level").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_anonymity_level: {err}")))?;
    let last_probe_latency_ms: Option<i64> = row.try_get("last_probe_latency_ms").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_probe_latency_ms: {err}")))?;
    let last_probe_error_category: Option<String> = row.try_get("last_probe_error_category").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_probe_error_category: {err}")))?;
    let last_exit_region: Option<String> = row.try_get("last_exit_region").map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to decode last_exit_region: {err}")))?;

    let provider_risk_hit: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM provider_risk_snapshots s JOIN proxies p ON p.provider = s.provider WHERE p.id = ? AND s.risk_hit != 0)")
        .bind(&proxy_id)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to compute provider risk: {err}")))?;
    let provider_region_cluster_hit: i64 = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM provider_region_risk_snapshots s JOIN proxies p ON p.provider = s.provider AND p.region = s.region WHERE p.id = ? AND s.risk_hit != 0)")
        .bind(&proxy_id)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to compute provider region risk: {err}")))?;
    let region_match_ok = match (last_exit_region.as_deref(), region.as_deref()) {
        (Some(actual), Some(expected)) => Some(actual.eq_ignore_ascii_case(expected)),
        _ => None,
    };
    refresh_proxy_trust_views_for_scope(&state.db, &proxy_id, provider.as_deref(), region.as_deref())
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to refresh proxy trust views for explain: {err}")))?;
    let now_ts = now.parse::<i64>().unwrap_or_default();
    let components = crate::runner::engine::computed_trust_score_components(
        &state.proxy_selection_tuning,
        score,
        success_count,
        failure_count,
        last_verify_status.as_deref(),
        last_verify_geo_match_ok.unwrap_or(0) != 0,
        region_match_ok,
        last_smoke_upstream_ok.unwrap_or(0) != 0,
        last_verify_at.as_ref().and_then(|v: &String| v.parse::<i64>().ok()),
        last_verify_confidence,
        last_verify_score_delta,
        last_verify_source.as_deref(),
        last_anonymity_level.as_deref(),
        last_probe_latency_ms,
        last_probe_error_category.as_deref(),
        provider_risk_hit != 0,
        provider_region_cluster_hit != 0,
        now_ts,
        None,
    );
    let cached_row = load_cached_trust_score_row(&state, &proxy_id).await?;
    let trust_score_total = cached_row.as_ref().and_then(|row| row.0);
    let candidate_rank_preview = crate::runner::engine::compute_candidate_preview_with_reasons(&state, &now, provider.as_deref(), region.as_deref(), 0.0_f64, None, None)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to build candidate preview with reasons: {err}")))?;
    let candidate_rank_preview = if candidate_rank_preview.is_empty() {
        crate::runner::engine::compute_candidate_preview_with_reasons(&state, &now, None, None, 0.0_f64, None, None)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to build fallback candidate preview with reasons: {err}")))?
    } else {
        candidate_rank_preview
    };

    let cached_at = cached_row.as_ref().and_then(|row| row.1.clone());
    let selection_reason_summary = format!(
        "proxy {} currently scores {:?} (cache_at={:?}); verify_status={:?}, geo_match={}, upstream_ok={}, provider_risk={}, provider_region_cluster={}",
        id,
        trust_score_total,
        cached_at,
        last_verify_status,
        last_verify_geo_match_ok.unwrap_or(0) != 0,
        last_smoke_upstream_ok.unwrap_or(0) != 0,
        provider_risk_hit != 0,
        provider_region_cluster_hit != 0,
    );

    let winner_vs_runner_up_diff = candidate_rank_preview.first().and_then(|item| item.winner_vs_runner_up_diff.clone());
    let (provider_risk_version_current, provider_risk_version_seen, provider_risk_version_status) =
        provider_risk_version_state_for_proxy(&state.db, &id)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load provider risk version state: {err}")))?;
    let response = ProxySelectionExplainResponse {
        proxy_id: id,
        mode: crate::network_identity::proxy_harvest::proxy_runtime_mode_from_env(),
        source_tier: None,
        verification_path: None,
        continuity_mode: None,
        consumption_source_of_truth: None,
        trust_score_total,
        trust_score_cached_at: cached_at,
        explain_generated_at: now_ts.to_string(),
        explain_source: "proxy_trust_cache+candidate_preview".to_string(),
        provider_risk_version_current,
        provider_risk_version_seen,
        provider_risk_version_status,
        selection_reason_summary,
        trust_score_components: components,
        candidate_rank_preview,
        winner_vs_runner_up_diff,
    };
    perf_probe_log(
        "api_proxy_explain",
        &[("proxy_id", response.proxy_id.clone()), ("elapsed_ms", started.elapsed().as_millis().to_string()), ("candidate_count", response.candidate_rank_preview.len().to_string())],
    );
    Ok(Json(response))
}

fn create_task_response_from_payload(
    task_id: String,
    payload: &CreateTaskRequest,
    profile_version: Option<i64>,
    task_status: &str,
    platform_id: Option<String>,
    manual_gate_request_id: Option<String>,
    manual_gate_status: Option<String>,
) -> (StatusCode, Json<TaskResponse>) {
    let priority = payload.priority.unwrap_or(0);
    (
        StatusCode::CREATED,
        Json(TaskResponse {
            id: task_id,
            kind: payload.kind.clone(),
            status: task_status.to_string(),
            priority,
            started_at: None,
            finished_at: None,
            summary_artifacts: Vec::new(),
            persona_id: payload.persona_id.clone(),
            platform_id: payload.platform_id.clone().or(platform_id),
            behavior_profile_id: payload.behavior_profile_id.clone(),
            flow_template_id: payload.flow_template_id.clone(),
            humanize_level: payload.humanize_level.clone(),
            event_trace_level: payload.event_trace_level.clone(),
            manual_gate_request_id,
            manual_gate_status,
            fingerprint_profile_id: payload.fingerprint_profile_id.clone(),
            fingerprint_profile_version: profile_version,
            fingerprint_resolution_status: profile_version.map(|_| "pending".to_string()),
            proxy_id: None,
            proxy_provider: None,
            proxy_region: None,
            proxy_resolution_status: payload.network_policy_json.as_ref().and_then(|v| v.get("mode")).and_then(|v| v.as_str()).map(|mode| if mode == "direct" { "direct".to_string() } else { "pending".to_string() }),
            trust_score_total: None,
            selection_reason_summary: None,
            selection_explain: None,
            fingerprint_runtime_explain: None,
            execution_identity: None,
            identity_network_explain: None,
            winner_vs_runner_up_diff: None,
            failure_scope: None,
            browser_failure_signal: None,
            title: None,
            final_url: None,
            content_preview: None,
            content_length: None,
            content_truncated: None,
            content_kind: None,
            content_source_action: None,
            content_ready: None,
            interaction_trace_summary: None,
            event_counts: None,
            distinct_event_types: None,
            fingerprint_applied_verified_count: None,
            deep_fingerprint_applied_verified_count: None,
            site_behavior_state: None,
            source_execution_share: None,
            proxy_execution_share: None,
        }),
    )
}

async fn create_task_from_payload(
    state: &AppState,
    payload: CreateTaskRequest,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    let mut payload = normalize_browser_task_request(payload)?;

    if payload.kind.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "kind is required".to_string()));
    }

    let task_id = format!("task-{}", Uuid::new_v4());
    let priority = payload.priority.unwrap_or(0);
    let resolved_persona = if let Some(persona_id) = payload.persona_id.as_deref() {
        Some(resolve_persona_bundle(state, persona_id).await?)
    } else {
        None
    };

    if let Some(bundle) = resolved_persona.as_ref() {
        if bundle.device_family != "desktop" {
            return Err((StatusCode::BAD_REQUEST, format!("unsupported persona device_family for this phase: {}", bundle.device_family)));
        }
        if let Some(explicit_fp) = payload.fingerprint_profile_id.as_deref() {
            if explicit_fp != bundle.fingerprint_profile_id {
                return Err((StatusCode::BAD_REQUEST, format!("persona {} resolves fingerprint_profile_id={} but request provided {}", bundle.persona_id, bundle.fingerprint_profile_id, explicit_fp)));
            }
        }
        payload.fingerprint_profile_id = Some(bundle.fingerprint_profile_id.clone());
        if payload.behavior_profile_id.is_none() {
            payload.behavior_profile_id = bundle.behavior_profile_id.clone();
        }

        let mut final_network_policy = merge_json_objects(
            bundle.network_policy.network_policy_json.clone(),
            payload.network_policy_json.clone().unwrap_or_else(|| json!({})),
        );
        if let Value::Object(obj) = &mut final_network_policy {
            obj.insert("country_anchor".to_string(), json!(bundle.country_anchor));
            obj.insert(
                "region_anchor".to_string(),
                bundle
                    .region_anchor
                    .as_ref()
                    .map_or(Value::Null, |value| json!(value)),
            );
            obj.insert("allow_same_country_fallback".to_string(), json!(bundle.network_policy.allow_same_country_fallback));
            obj.insert(
                "allow_same_region_fallback".to_string(),
                json!(bundle.network_policy.allow_same_region_fallback),
            );
            obj.insert("network_policy_id".to_string(), json!(bundle.network_policy.id));
            obj.insert("continuity_policy_id".to_string(), json!(bundle.continuity_policy.id));
            obj.insert("persona_id".to_string(), json!(bundle.persona_id));
            obj.insert("store_id".to_string(), json!(bundle.store_id));
            obj.insert("platform_id".to_string(), json!(bundle.platform_id));
            if let Some(provider_preference) = bundle.network_policy.provider_preference.as_deref() {
                obj.entry("provider".to_string())
                    .or_insert_with(|| json!(provider_preference));
            }
        }
        payload.network_policy_json = Some(final_network_policy);
    }

    let profile_version = if let Some(profile_id) = payload.fingerprint_profile_id.as_deref() {
        let version = sqlx::query_scalar::<_, i64>(r#"SELECT version FROM fingerprint_profiles WHERE id = ? AND status = 'active'"#)
            .bind(profile_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to resolve fingerprint profile version: {err}")))?;
        if version.is_none() {
            return Err((StatusCode::BAD_REQUEST, "fingerprint profile not found or inactive".to_string()));
        }
        version
    } else {
        None
    };

    let platform_id = payload
        .platform_id
        .clone()
        .or_else(|| resolved_persona.as_ref().map(|bundle| bundle.platform_id.clone()));
    let created_at = now_ts_string();
    let mut manual_gate_request_id: Option<String> = None;
    let mut manual_gate_status: Option<String> = None;
    let mut task_status = TASK_STATUS_QUEUED.to_string();
    let mut queued_at = Some(created_at.clone());

    let platform_template_json = resolved_persona
        .as_ref()
        .and_then(|bundle| bundle.platform_template.as_ref().map(|template| {
            json!({
                "id": template.id,
                "platform_id": template.platform_id,
                "readiness_level": template.readiness_level,
                "warm_paths": template.warm_paths_json,
                "revisit_paths": template.revisit_paths_json,
                "stateful_paths": template.stateful_paths_json,
                "write_operation_paths": template.write_operation_paths_json,
                "high_risk_paths": template.high_risk_paths_json,
                "continuity_checks": template.continuity_checks_json,
                "identity_markers": template.identity_markers_json,
                "identity_markers_source": template.identity_markers_source,
                "login_loss_signals": template.login_loss_signals_json,
                "recovery_steps": template.recovery_steps_json,
                "behavior_defaults": template.behavior_defaults_json,
                "event_chain_templates": template.event_chain_templates_json,
                "page_semantics": template.page_semantics_json,
            })
        }));

    let force_manual_gate = matches!(
        payload.manual_gate_policy.as_deref(),
        Some("always" | "force" | "required")
    ) || matches!(
        payload.requested_operation_kind.as_deref(),
        Some("high_risk_write" | "credential_change" | "security_change" | "billing_change")
    );
    if let Some(bundle) = resolved_persona.as_ref() {
        let url_is_high_risk = match (
            payload.url.as_deref(),
            resolved_persona
                .as_ref()
                .and_then(|item| item.platform_template.as_ref()),
        ) {
            (Some(url), Some(template)) => {
                let high_risk_paths = path_patterns_from_value(&template.high_risk_paths_json);
                url_matches_any_path(url, &high_risk_paths)
            }
            _ => false,
        };
        if force_manual_gate || url_is_high_risk {
            manual_gate_request_id = Some(format!("gate-{}", Uuid::new_v4()));
            manual_gate_status = Some("pending".to_string());
            task_status = TASK_STATUS_PENDING.to_string();
            queued_at = None;
            perf_probe_log(
                "manual_gate_requested",
                &[
                    ("persona_id", bundle.persona_id.clone()),
                    ("platform_id", bundle.platform_id.clone()),
                    ("forced", force_manual_gate.to_string()),
                ],
            );
        }
    }

    let network_policy_value = payload.network_policy_json.clone();
    let network_policy_json = network_policy_value.as_ref().map(|v| v.to_string());
    let (task_proxy_id, task_requested_region, task_proxy_mode) = task_typed_proxy_columns(
        payload.proxy_id.as_deref(),
        network_policy_value.as_ref(),
        &state.proxy_runtime_mode,
    );
    let input_json = serde_json::json!({
        "url": payload.url,
        "script": payload.script,
        "timeout_seconds": payload.timeout_seconds,
        "persona_id": payload.persona_id,
        "store_id": resolved_persona.as_ref().map(|bundle| bundle.store_id.clone()),
        "platform_id": platform_id,
        "behavior_profile_id": payload.behavior_profile_id.clone().or_else(|| resolved_persona.as_ref().and_then(|bundle| bundle.behavior_profile_id.clone())),
        "device_family": resolved_persona.as_ref().map(|bundle| bundle.device_family.clone()),
        "country_anchor": resolved_persona.as_ref().map(|bundle| bundle.country_anchor.clone()),
        "region_anchor": resolved_persona.as_ref().and_then(|bundle| bundle.region_anchor.clone()),
        "locale": resolved_persona.as_ref().map(|bundle| bundle.locale.clone()),
        "timezone": resolved_persona.as_ref().map(|bundle| bundle.timezone.clone()),
        "credential_ref": resolved_persona.as_ref().and_then(|bundle| bundle.credential_ref.clone()),
        "network_policy_id": resolved_persona.as_ref().map(|bundle| bundle.network_policy.id.clone()),
        "continuity_policy_id": resolved_persona.as_ref().map(|bundle| bundle.continuity_policy.id.clone()),
        "platform_template_id": resolved_persona.as_ref().and_then(|bundle| bundle.platform_template.as_ref().map(|template| template.id.clone())),
        "platform_template": platform_template_json,
        "flow_template_id": payload.flow_template_id.clone(),
        "humanize_level": payload.humanize_level.clone(),
        "event_trace_level": payload.event_trace_level.clone(),
        "flow_json": payload.flow_json.clone(),
        "capture_kind": payload.capture_kind.clone(),
        "continuity_context": resolved_persona.as_ref().map(|bundle| json!({
            "session_ttl_seconds": bundle.continuity_policy.session_ttl_seconds,
            "heartbeat_interval_seconds": bundle.continuity_policy.heartbeat_interval_seconds,
            "site_group_mode": bundle.continuity_policy.site_group_mode,
            "recovery_enabled": bundle.continuity_policy.recovery_enabled,
            "protect_on_login_loss": bundle.continuity_policy.protect_on_login_loss,
        })),
        "requested_operation_kind": payload.requested_operation_kind,
        "manual_gate_policy": payload.manual_gate_policy,
        "fingerprint_profile_id": payload.fingerprint_profile_id,
        "fingerprint_profile_version": profile_version,
        "proxy_id": task_proxy_id,
        "requested_region": task_requested_region,
        "proxy_mode": task_proxy_mode,
        "network_policy_json": network_policy_value
    })
    .to_string();

    sqlx::query(
        r#"
        INSERT INTO tasks (
            id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
            persona_id, platform_id, proxy_id, requested_region, proxy_mode, manual_gate_request_id,
            priority, created_at, queued_at, started_at, finished_at, fingerprint_profile_id,
            fingerprint_profile_version, result_json, error_message
        ) VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, NULL, NULL)
        "#,
    )
    .bind(&task_id)
    .bind(&payload.kind)
    .bind(&task_status)
    .bind(&input_json)
    .bind(&network_policy_json)
    .bind(&payload.persona_id)
    .bind(&platform_id)
    .bind(&task_proxy_id)
    .bind(&task_requested_region)
    .bind(&task_proxy_mode)
    .bind(&manual_gate_request_id)
    .bind(priority)
    .bind(&created_at)
    .bind(&queued_at)
    .bind(&payload.fingerprint_profile_id)
    .bind(profile_version)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to insert task: {err}"),
        )
    })?;

    if let Some(bundle) = resolved_persona.as_ref() {
        append_continuity_event(
            state,
            Some(&bundle.persona_id),
            Some(&bundle.store_id),
            Some(&bundle.platform_id),
            Some(&task_id),
            None,
            "persona_selected",
            "info",
            Some(&json!({
                "fingerprint_profile_id": bundle.fingerprint_profile_id,
                "network_policy_id": bundle.network_policy.id,
                "continuity_policy_id": bundle.continuity_policy.id,
            })),
        )
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to record persona_selected event: {err}")))?;

        append_continuity_event(
            state,
            Some(&bundle.persona_id),
            Some(&bundle.store_id),
            Some(&bundle.platform_id),
            Some(&task_id),
            None,
            "fingerprint_resolved",
            "info",
            Some(&json!({
                "fingerprint_profile_id": bundle.fingerprint_profile_id,
                "fingerprint_profile_version": profile_version,
            })),
        )
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to record fingerprint_resolved event: {err}")))?;
    }

    if let (Some(bundle), Some(gate_id)) = (resolved_persona.as_ref(), manual_gate_request_id.as_ref()) {
        let gate_reason_summary = "requested path matched platform high_risk_paths and requires manual confirmation";
        let requested_action_kind = manual_gate_category_from_inputs(
            Some(&bundle.platform_id),
            &payload.kind,
            payload.requested_operation_kind.as_deref(),
            payload.url.as_deref(),
        );
        sqlx::query(
            r#"INSERT INTO manual_gate_requests (
                   id, task_id, persona_id, store_id, platform_id, requested_action_kind,
                   requested_url, reason_code, reason_summary, status, resolution_note,
                   created_at, updated_at, resolved_at
               )
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', NULL, ?, ?, NULL)"#,
        )
        .bind(gate_id)
        .bind(&task_id)
        .bind(&bundle.persona_id)
        .bind(&bundle.store_id)
        .bind(&bundle.platform_id)
        .bind(&requested_action_kind)
        .bind(&payload.url)
        .bind("high_risk_path")
        .bind(gate_reason_summary)
        .bind(&created_at)
        .bind(&created_at)
        .execute(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to insert manual gate request: {err}")))?;

        append_continuity_event(
            state,
            Some(&bundle.persona_id),
            Some(&bundle.store_id),
            Some(&bundle.platform_id),
            Some(&task_id),
            None,
            "manual_gate_requested",
            "warning",
            Some(&json!({
                "manual_gate_request_id": gate_id,
                "requested_url": payload.url,
                "requested_action_kind": requested_action_kind,
            })),
        )
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to record manual_gate_requested event: {err}")))?;
    }

    Ok(create_task_response_from_payload(
        task_id,
        &payload,
        profile_version,
        &task_status,
        platform_id,
        manual_gate_request_id,
        manual_gate_status,
    ))
}

pub async fn run_persona_heartbeat_tick(
    state: &AppState,
) -> Result<ContinuityHeartbeatTickResponse, (StatusCode, String)> {
    let ticked_at = now_ts_string();
    let now_ts = ticked_at.parse::<i64>().unwrap_or_default();
    let candidates = sqlx::query_as::<_, HeartbeatPersonaCandidateRow>(
        r#"
        SELECT
            p.id AS persona_id,
            p.store_id,
            p.platform_id,
            cp.heartbeat_interval_seconds
        FROM persona_profiles p
        JOIN continuity_policies cp ON cp.id = p.continuity_policy_id
        WHERE p.status IN ('active', 'degraded')
          AND cp.status = 'active'
          AND NOT EXISTS (
              SELECT 1
              FROM manual_gate_requests mg
              WHERE mg.persona_id = p.id
                AND mg.status = 'pending'
          )
        ORDER BY
            COALESCE((
                SELECT MAX(CAST(e.created_at AS INTEGER))
                FROM continuity_events e
                WHERE e.persona_id = p.id
                  AND e.event_type IN ('heartbeat_scheduled', 'heartbeat_failed', 'heartbeat_skipped')
            ), 0) ASC,
            p.platform_id ASC,
            p.created_at ASC,
            p.id ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load persona heartbeat candidates: {err}"),
        )
    })?;

    let mut items = Vec::with_capacity(candidates.len());
    let mut scheduled_count = 0_i64;
    let mut skipped_count = 0_i64;
    let platform_cap = heartbeat_platform_cap_from_env();
    let mut scheduled_per_platform = std::collections::BTreeMap::<String, usize>::new();

    for candidate in candidates {
        let heartbeat_interval_seconds = candidate.heartbeat_interval_seconds.max(60);
        let platform_scheduled = scheduled_per_platform
            .get(&candidate.platform_id)
            .copied()
            .unwrap_or(0);
        if platform_scheduled >= platform_cap {
            let item = heartbeat_item(
                candidate.persona_id.clone(),
                candidate.store_id.clone(),
                candidate.platform_id.clone(),
                "skipped",
                "platform_cap_reached",
                None,
                None,
                heartbeat_interval_seconds,
            );
            append_heartbeat_event(
                state,
                &candidate.persona_id,
                &candidate.store_id,
                &candidate.platform_id,
                None,
                "heartbeat_skipped",
                "info",
                &item.reason,
                heartbeat_interval_seconds,
                None,
                None,
                None,
                None,
            )
            .await?;
            items.push(item);
            skipped_count += 1;
            continue;
        }
        if persona_has_inflight_task(state, &candidate.persona_id).await? {
            let item = heartbeat_item(
                candidate.persona_id.clone(),
                candidate.store_id.clone(),
                candidate.platform_id.clone(),
                "skipped",
                "persona_has_active_task",
                None,
                None,
                heartbeat_interval_seconds,
            );
            append_heartbeat_event(
                state,
                &candidate.persona_id,
                &candidate.store_id,
                &candidate.platform_id,
                None,
                "heartbeat_skipped",
                "info",
                &item.reason,
                heartbeat_interval_seconds,
                None,
                None,
                None,
                None,
            )
            .await?;
            items.push(item);
            skipped_count += 1;
            continue;
        }

        if let Some(last_activity_ts) =
            latest_persona_task_activity_ts(state, &candidate.persona_id).await?
        {
            let elapsed_seconds = now_ts.saturating_sub(last_activity_ts);
            if elapsed_seconds < heartbeat_interval_seconds {
                let item = heartbeat_item(
                    candidate.persona_id.clone(),
                    candidate.store_id.clone(),
                    candidate.platform_id.clone(),
                    "skipped",
                    "recent_persona_activity",
                    None,
                    None,
                    heartbeat_interval_seconds,
                );
                append_heartbeat_event(
                    state,
                    &candidate.persona_id,
                    &candidate.store_id,
                    &candidate.platform_id,
                    None,
                    "heartbeat_skipped",
                    "info",
                    &item.reason,
                    heartbeat_interval_seconds,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;
                items.push(item);
                skipped_count += 1;
                continue;
            }
        }

        let bundle = match resolve_persona_bundle(state, &candidate.persona_id).await {
            Ok(value) => value,
            Err((status, message)) if status == StatusCode::BAD_REQUEST => {
                let reason = if message.contains("not active") || message.contains("not found") {
                    "persona_not_active"
                } else {
                    "resolve_persona_failed"
                };
                let item = heartbeat_item(
                    candidate.persona_id.clone(),
                    candidate.store_id.clone(),
                    candidate.platform_id.clone(),
                    "failed",
                    reason,
                    None,
                    None,
                    heartbeat_interval_seconds,
                );
                append_heartbeat_event(
                    state,
                    &candidate.persona_id,
                    &candidate.store_id,
                    &candidate.platform_id,
                    None,
                    "heartbeat_failed",
                    "warning",
                    &item.reason,
                    heartbeat_interval_seconds,
                    None,
                    None,
                    None,
                    None,
                )
                .await?;
                items.push(item);
                continue;
            }
            Err(err) => return Err(err),
        };

        let Some(template) = bundle.platform_template.as_ref() else {
            let item = heartbeat_item(
                bundle.persona_id.clone(),
                bundle.store_id.clone(),
                bundle.platform_id.clone(),
                "skipped",
                "platform_template_missing",
                None,
                None,
                heartbeat_interval_seconds,
            );
            append_heartbeat_event(
                state,
                &bundle.persona_id,
                &bundle.store_id,
                &bundle.platform_id,
                None,
                "heartbeat_skipped",
                "info",
                &item.reason,
                heartbeat_interval_seconds,
                None,
                None,
                bundle.origin_source.as_deref(),
                None,
            )
            .await?;
            items.push(item);
            skipped_count += 1;
            continue;
        };

        let heartbeat_task_kind = heartbeat_task_kind_for_template(template);
        let Some(target_path) = resolve_heartbeat_target_path(state, &bundle, template).await? else {
            let item = heartbeat_item(
                bundle.persona_id.clone(),
                bundle.store_id.clone(),
                bundle.platform_id.clone(),
                "skipped",
                "heartbeat_target_missing",
                None,
                None,
                heartbeat_interval_seconds,
            );
            append_heartbeat_event(
                state,
                &bundle.persona_id,
                &bundle.store_id,
                &bundle.platform_id,
                None,
                "heartbeat_skipped",
                "info",
                &item.reason,
                heartbeat_interval_seconds,
                None,
                None,
                bundle.origin_source.as_deref(),
                Some(heartbeat_task_kind),
            )
            .await?;
            items.push(item);
            skipped_count += 1;
            continue;
        };

        let high_risk_paths = path_patterns_from_value(&template.high_risk_paths_json);
        if url_matches_any_path(&target_path, &high_risk_paths) {
            let item = heartbeat_item(
                bundle.persona_id.clone(),
                bundle.store_id.clone(),
                bundle.platform_id.clone(),
                "skipped",
                "heartbeat_target_is_high_risk",
                None,
                None,
                heartbeat_interval_seconds,
            );
            append_heartbeat_event(
                state,
                &bundle.persona_id,
                &bundle.store_id,
                &bundle.platform_id,
                None,
                "heartbeat_skipped",
                "info",
                &item.reason,
                heartbeat_interval_seconds,
                Some(&target_path),
                None,
                bundle.origin_source.as_deref(),
                Some(heartbeat_task_kind),
            )
            .await?;
            items.push(item);
            skipped_count += 1;
            continue;
        }

        let Some((target_url, origin_source)) = resolve_heartbeat_target_url(&bundle, &target_path)
        else {
            let item = heartbeat_item(
                bundle.persona_id.clone(),
                bundle.store_id.clone(),
                bundle.platform_id.clone(),
                "failed",
                "heartbeat_target_origin_unresolved",
                None,
                None,
                heartbeat_interval_seconds,
            );
            append_heartbeat_event(
                state,
                &bundle.persona_id,
                &bundle.store_id,
                &bundle.platform_id,
                None,
                "heartbeat_failed",
                "warning",
                &item.reason,
                heartbeat_interval_seconds,
                Some(&target_path),
                None,
                bundle.origin_source.as_deref(),
                Some(heartbeat_task_kind),
            )
            .await?;
            items.push(item);
            continue;
        };

        if url_matches_any_path(&target_url, &high_risk_paths) {
            let item = heartbeat_item(
                bundle.persona_id.clone(),
                bundle.store_id.clone(),
                bundle.platform_id.clone(),
                "skipped",
                "heartbeat_target_is_high_risk",
                None,
                Some(target_url.clone()),
                heartbeat_interval_seconds,
            );
            append_heartbeat_event(
                state,
                &bundle.persona_id,
                &bundle.store_id,
                &bundle.platform_id,
                None,
                "heartbeat_skipped",
                "info",
                &item.reason,
                heartbeat_interval_seconds,
                Some(&target_path),
                Some(&target_url),
                Some(&origin_source),
                Some(heartbeat_task_kind),
            )
            .await?;
            items.push(item);
            skipped_count += 1;
            continue;
        }

        let task_payload = CreateTaskRequest {
            kind: heartbeat_task_kind.to_string(),
            url: Some(target_url.clone()),
            script: None,
            timeout_seconds: Some(45),
            priority: Some(-10),
            persona_id: Some(bundle.persona_id.clone()),
            platform_id: Some(bundle.platform_id.clone()),
            fingerprint_profile_id: None,
            behavior_profile_id: bundle.behavior_profile_id.clone(),
            flow_template_id: Some("observe_short".to_string()),
            humanize_level: Some("balanced".to_string()),
            event_trace_level: Some("summary".to_string()),
            proxy_id: None,
            network_policy_json: None,
            requested_operation_kind: Some("heartbeat_revisit".to_string()),
            manual_gate_policy: None,
            flow_json: None,
            capture_kind: None,
        };

        let (_, Json(task_response)) = match create_task_from_payload(state, task_payload).await {
            Ok(value) => value,
            Err((status, message)) if status == StatusCode::BAD_REQUEST => {
                let _ = message;
                let item = heartbeat_item(
                    bundle.persona_id.clone(),
                    bundle.store_id.clone(),
                    bundle.platform_id.clone(),
                    "failed",
                    "heartbeat_task_create_failed",
                    None,
                    Some(target_url.clone()),
                    heartbeat_interval_seconds,
                );
                append_heartbeat_event(
                    state,
                    &bundle.persona_id,
                    &bundle.store_id,
                    &bundle.platform_id,
                    None,
                    "heartbeat_failed",
                    "warning",
                    &item.reason,
                    heartbeat_interval_seconds,
                    Some(&target_path),
                    Some(&target_url),
                    Some(&origin_source),
                    Some(heartbeat_task_kind),
                )
                .await?;
                items.push(item);
                continue;
            }
            Err(err) => return Err(err),
        };

        let scheduled_task_id = task_response.id.clone();
        append_heartbeat_event(
            state,
            &bundle.persona_id,
            &bundle.store_id,
            &bundle.platform_id,
            Some(&scheduled_task_id),
            "heartbeat_scheduled",
            "info",
            "heartbeat_due",
            heartbeat_interval_seconds,
            Some(&target_path),
            Some(&target_url),
            Some(&origin_source),
            Some(heartbeat_task_kind),
        )
        .await?;

        scheduled_count += 1;
        *scheduled_per_platform
            .entry(candidate.platform_id.clone())
            .or_insert(0) += 1;
        items.push(heartbeat_item(
            bundle.persona_id,
            bundle.store_id,
            bundle.platform_id,
            "scheduled",
            "heartbeat_due",
            Some(task_response.id),
            Some(target_url),
            heartbeat_interval_seconds,
        ));
    }

    let evaluated_count = i64::try_from(items.len()).unwrap_or(0);
    Ok(ContinuityHeartbeatTickResponse {
        ticked_at,
        evaluated_count,
        scheduled_count,
        skipped_count,
        items,
    })
}

pub async fn trigger_persona_heartbeat_tick(
    State(state): State<AppState>,
) -> Result<Json<ContinuityHeartbeatTickResponse>, (StatusCode, String)> {
    run_persona_heartbeat_tick(&state).await.map(Json)
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    create_task_from_payload(&state, payload).await
}

pub async fn browser_open(
    State(state): State<AppState>,
    Json(payload): Json<BrowserOpenRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    if payload.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "url is required".to_string()));
    }
    let task_payload = CreateTaskRequest {
        kind: "open_page".to_string(),
        url: Some(payload.url),
        script: None,
        timeout_seconds: payload.timeout_seconds,
        priority: payload.priority,
        persona_id: payload.persona_id,
        platform_id: payload.platform_id,
        fingerprint_profile_id: payload.fingerprint_profile_id,
        behavior_profile_id: payload.behavior_profile_id,
        flow_template_id: payload.flow_template_id,
        humanize_level: payload.humanize_level,
        event_trace_level: payload.event_trace_level,
        proxy_id: payload.proxy_id,
        network_policy_json: payload.network_policy_json,
        requested_operation_kind: payload.requested_operation_kind,
        manual_gate_policy: payload.manual_gate_policy,
        flow_json: None,
        capture_kind: None,
    };
    create_task_from_payload(&state, task_payload).await
}

pub async fn browser_get_html(
    State(state): State<AppState>,
    Json(payload): Json<BrowserGetHtmlRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    if payload.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "url is required".to_string()));
    }
    let task_payload = CreateTaskRequest {
        kind: "get_html".to_string(),
        url: Some(payload.url),
        script: None,
        timeout_seconds: payload.timeout_seconds,
        priority: payload.priority,
        persona_id: payload.persona_id,
        platform_id: payload.platform_id,
        fingerprint_profile_id: payload.fingerprint_profile_id,
        behavior_profile_id: payload.behavior_profile_id,
        flow_template_id: payload.flow_template_id,
        humanize_level: payload.humanize_level,
        event_trace_level: payload.event_trace_level,
        proxy_id: payload.proxy_id,
        network_policy_json: payload.network_policy_json,
        requested_operation_kind: payload.requested_operation_kind,
        manual_gate_policy: payload.manual_gate_policy,
        flow_json: None,
        capture_kind: None,
    };
    create_task_from_payload(&state, task_payload).await
}

pub async fn browser_get_title(
    State(state): State<AppState>,
    Json(payload): Json<BrowserGetTitleRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    if payload.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "url is required".to_string()));
    }
    let task_payload = CreateTaskRequest {
        kind: "get_title".to_string(),
        url: Some(payload.url),
        script: None,
        timeout_seconds: payload.timeout_seconds,
        priority: payload.priority,
        persona_id: payload.persona_id,
        platform_id: payload.platform_id,
        fingerprint_profile_id: payload.fingerprint_profile_id,
        behavior_profile_id: payload.behavior_profile_id,
        flow_template_id: payload.flow_template_id,
        humanize_level: payload.humanize_level,
        event_trace_level: payload.event_trace_level,
        proxy_id: payload.proxy_id,
        network_policy_json: payload.network_policy_json,
        requested_operation_kind: payload.requested_operation_kind,
        manual_gate_policy: payload.manual_gate_policy,
        flow_json: None,
        capture_kind: None,
    };
    create_task_from_payload(&state, task_payload).await
}

pub async fn browser_get_final_url(
    State(state): State<AppState>,
    Json(payload): Json<BrowserGetFinalUrlRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    if payload.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "url is required".to_string()));
    }
    let task_payload = CreateTaskRequest {
        kind: "get_final_url".to_string(),
        url: Some(payload.url),
        script: None,
        timeout_seconds: payload.timeout_seconds,
        priority: payload.priority,
        persona_id: payload.persona_id,
        platform_id: payload.platform_id,
        fingerprint_profile_id: payload.fingerprint_profile_id,
        behavior_profile_id: payload.behavior_profile_id,
        flow_template_id: payload.flow_template_id,
        humanize_level: payload.humanize_level,
        event_trace_level: payload.event_trace_level,
        proxy_id: payload.proxy_id,
        network_policy_json: payload.network_policy_json,
        requested_operation_kind: payload.requested_operation_kind,
        manual_gate_policy: payload.manual_gate_policy,
        flow_json: None,
        capture_kind: None,
    };
    create_task_from_payload(&state, task_payload).await
}

pub async fn browser_extract_text(
    State(state): State<AppState>,
    Json(payload): Json<BrowserExtractTextRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    if payload.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "url is required".to_string()));
    }
    let task_payload = CreateTaskRequest {
        kind: "extract_text".to_string(),
        url: Some(payload.url),
        script: None,
        timeout_seconds: payload.timeout_seconds,
        priority: payload.priority,
        persona_id: payload.persona_id,
        platform_id: payload.platform_id,
        fingerprint_profile_id: payload.fingerprint_profile_id,
        behavior_profile_id: payload.behavior_profile_id,
        flow_template_id: payload.flow_template_id,
        humanize_level: payload.humanize_level,
        event_trace_level: payload.event_trace_level,
        proxy_id: payload.proxy_id,
        network_policy_json: payload.network_policy_json,
        requested_operation_kind: payload.requested_operation_kind,
        manual_gate_policy: payload.manual_gate_policy,
        flow_json: None,
        capture_kind: None,
    };
    create_task_from_payload(&state, task_payload).await
}

pub async fn browser_flow(
    State(state): State<AppState>,
    Json(payload): Json<BrowserFlowRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    if payload.url.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "url is required".to_string()));
    }
    let task_payload = CreateTaskRequest {
        kind: "execute_behavior_flow".to_string(),
        url: Some(payload.url),
        script: None,
        timeout_seconds: payload.timeout_seconds,
        priority: payload.priority,
        persona_id: payload.persona_id,
        platform_id: payload.platform_id,
        fingerprint_profile_id: payload.fingerprint_profile_id,
        behavior_profile_id: payload.behavior_profile_id,
        flow_template_id: payload.flow_template_id,
        humanize_level: payload.humanize_level,
        event_trace_level: payload.event_trace_level,
        proxy_id: payload.proxy_id,
        network_policy_json: payload.network_policy_json,
        requested_operation_kind: payload.requested_operation_kind,
        manual_gate_policy: payload.manual_gate_policy,
        flow_json: payload.flow_json,
        capture_kind: payload.capture_kind,
    };
    create_task_from_payload(&state, task_payload).await
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResponse>, (StatusCode, String)> {
    if task_id.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "task id is required".to_string()));
    }

    let row = sqlx::query_as::<_, (
        String, String, String, i32, Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>,
    )>(
        r#"SELECT
               t.id,
               t.kind,
               t.status,
               t.priority,
               t.persona_id,
               t.platform_id,
               t.manual_gate_request_id,
               mg.status,
               t.fingerprint_profile_id,
               t.fingerprint_profile_version,
               t.result_json,
               t.started_at,
               t.finished_at
           FROM tasks t
           LEFT JOIN manual_gate_requests mg ON mg.id = t.manual_gate_request_id
           WHERE t.id = ?"#,
    )
    .bind(&task_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch task: {err}"),
        )
    })?;

    match row {
        Some((id, kind, status, priority, persona_id, platform_id, manual_gate_request_id, manual_gate_status, fingerprint_profile_id, fingerprint_profile_version, result_json, started_at, finished_at)) => Ok(Json(
            build_task_response_from_row(
                id,
                kind,
                status,
                priority,
                persona_id,
                platform_id,
                manual_gate_request_id,
                manual_gate_status,
                fingerprint_profile_id,
                fingerprint_profile_version,
                result_json,
                started_at,
                finished_at,
            ),
        )),
        None => Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}"))),
    }
}

pub async fn get_task_runs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<RunResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, String, i32, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>)>(
        r#"SELECT r.id, r.task_id, r.status, r.attempt, r.runner_kind, r.started_at, r.finished_at, r.error_message, r.result_json, t.kind, t.status, t.fingerprint_profile_id, t.fingerprint_profile_version FROM runs r LEFT JOIN tasks t ON t.id = r.task_id WHERE r.task_id = ? ORDER BY r.attempt DESC LIMIT ? OFFSET ?"#,
    )
    .bind(&task_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch runs: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message, result_json, task_kind, task_status, fingerprint_profile_id, fingerprint_profile_version)| {
                    let summary_timestamp = finished_at.as_deref().or(started_at.as_deref()).map(|v| v.to_string());
                    let explainability = build_task_explainability(
                        fingerprint_profile_id.as_deref(),
                        fingerprint_profile_version,
                        result_json.as_deref(),
                        Some(&task_id),
                        task_kind.as_deref(),
                        task_status.as_deref(),
                        summary_timestamp.as_deref(),
                    );
                    let parsed = result_json.as_deref().and_then(|raw| serde_json::from_str::<Value>(raw).ok());
                    RunResponse {
                        id: id.clone(),
                        task_id: task_id.clone(),
                        status,
                        attempt,
                        runner_kind,
                        started_at,
                        finished_at,
                        error_message,
                        summary_artifacts: enrich_summary_artifacts(
                            explainability.summary_artifacts,
                            Some(&task_id),
                            task_kind.as_deref(),
                            task_status.as_deref(),
                            Some(&id),
                            Some(attempt),
                            summary_timestamp.as_deref(),
                        ),
                        proxy_id: explainability.proxy_id,
                        proxy_provider: explainability.proxy_provider,
                        proxy_region: explainability.proxy_region,
                        proxy_resolution_status: explainability.proxy_resolution_status,
                        trust_score_total: explainability.trust_score_total,
                        selection_reason_summary: explainability.selection_reason_summary,
                        selection_explain: explainability.selection_explain,
                        fingerprint_runtime_explain: explainability.fingerprint_runtime_explain,
                        execution_identity: Some(explainability.execution_identity),
                        identity_network_explain: explainability.identity_network_explain,
                        winner_vs_runner_up_diff: explainability.winner_vs_runner_up_diff,
                        failure_scope: explainability.failure_scope,
                        browser_failure_signal: explainability.browser_failure_signal,
                        title: content_string_field(parsed.as_ref(), "title"),
                        final_url: content_string_field(parsed.as_ref(), "final_url"),
                        content_preview: content_string_field(parsed.as_ref(), "content_preview"),
                        content_length: content_i64_field(parsed.as_ref(), "content_length"),
                        content_truncated: content_bool_field(parsed.as_ref(), "content_truncated"),
                        content_kind: content_string_field(parsed.as_ref(), "content_kind"),
                        content_source_action: content_string_field(parsed.as_ref(), "content_source_action"),
                        content_ready: content_bool_field(parsed.as_ref(), "content_ready"),
                        interaction_trace_summary: value_from_parsed(parsed.as_ref(), "interaction_trace_summary"),
                        event_counts: value_from_parsed(parsed.as_ref(), "event_counts"),
                        distinct_event_types: string_vec_from_parsed(parsed.as_ref(), "distinct_event_types"),
                        fingerprint_applied_verified_count: i64_from_parsed(parsed.as_ref(), "fingerprint_applied_verified_count"),
                        deep_fingerprint_applied_verified_count: i64_from_parsed(parsed.as_ref(), "deep_fingerprint_applied_verified_count"),
                        site_behavior_state: value_from_parsed(parsed.as_ref(), "site_behavior_state"),
                        behavior_profile_id: payload_string_from_parsed(parsed.as_ref(), "behavior_profile_id"),
                        flow_template_id: payload_string_from_parsed(parsed.as_ref(), "flow_template_id"),
                        humanize_level: payload_string_from_parsed(parsed.as_ref(), "humanize_level"),
                        event_trace_level: payload_string_from_parsed(parsed.as_ref(), "event_trace_level"),
                    }
                },
            )
            .collect(),
    ))
}

pub async fn get_task_logs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<LogResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 50, 500);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        r#"SELECT id, task_id, run_id, level, message, created_at FROM logs WHERE task_id = ? ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#,
    )
    .bind(&task_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch logs: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, task_id, run_id, level, message, created_at)| LogResponse {
                id,
                task_id,
                run_id,
                level,
                message,
                created_at,
            })
            .collect(),
    ))
}

pub async fn retry_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<RetryTaskResponse>, (StatusCode, String)> {
    let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read task status before retry: {err}"),
            )
        })?;

    let Some(status) = current_status else {
        return Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}")));
    };

    if status == TASK_STATUS_QUEUED {
        return Ok(Json(RetryTaskResponse {
            id: task_id,
            status: TASK_STATUS_QUEUED.to_string(),
            message: "task already queued; retry treated as idempotent".to_string(),
        }));
    }

    if status != TASK_STATUS_FAILED && status != TASK_STATUS_TIMED_OUT {
        return Err((
            StatusCode::CONFLICT,
            format!("task status does not allow retry now: {status}"),
        ));
    }

    let queued_at = now_ts_string();
    let retry_sql = format!(
        "UPDATE tasks SET status = ?, queued_at = ?, started_at = NULL, finished_at = NULL, runner_id = NULL, heartbeat_at = NULL, result_json = NULL, error_message = NULL WHERE id = ? AND status IN ('{}', '{}')",
        TASK_STATUS_FAILED, TASK_STATUS_TIMED_OUT,
    );
    let result = sqlx::query(&retry_sql)
        .bind(TASK_STATUS_QUEUED)
        .bind(&queued_at)
        .bind(&task_id)
        .execute(&state.db)
        .await;

    let result = match result {
        Ok(result) => result,
        Err(err) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to retry task: {err}"),
            ));
        }
    };

    if result.rows_affected() == 0 {

        let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to read task status after retry conflict: {err}"),
                )
            })?;

        let Some(status) = current_status else {
            return Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}")));
        };

        if status == TASK_STATUS_QUEUED {
            return Ok(Json(RetryTaskResponse {
                id: task_id,
                status: TASK_STATUS_QUEUED.to_string(),
                message: "task already queued after retry race; treated as idempotent".to_string(),
            }));
        }

        return Err((
            StatusCode::CONFLICT,
            format!("task status does not allow retry now: {status}"),
        ));
    }

    let message = "task re-queued for retry".to_string();

    Ok(Json(RetryTaskResponse {
        id: task_id,
        status: TASK_STATUS_QUEUED.to_string(),
        message,
    }))
}

pub async fn cancel_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<CancelTaskResponse>, (StatusCode, String)> {
    let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read task status: {err}"),
            )
        })?;

    let Some(status) = current_status else {
        return Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}")));
    };

    if status == TASK_STATUS_QUEUED {
        let finished_at = now_ts_string();
        sqlx::query(r#"UPDATE tasks SET status = ?, finished_at = ?, runner_id = NULL, heartbeat_at = NULL, error_message = ? WHERE id = ?"#)
            .bind(TASK_STATUS_CANCELLED)
            .bind(&finished_at)
            .bind("task cancelled while queued")
            .bind(&task_id)
            .execute(&state.db)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to cancel task: {err}"),
                )
            })?;

        insert_task_log(
            &state,
            &task_id,
            None,
            "warn",
            "task cancelled while queued",
        )
        .await?;

        return Ok(Json(CancelTaskResponse {
            id: task_id,
            status: TASK_STATUS_CANCELLED.to_string(),
            message: "task cancelled while queued".to_string(),
        }));
    }

    if status == TASK_STATUS_RUNNING {
        let cancel = state.runner.cancel_running(&task_id).await;
        if !cancel.accepted {
            return Err((StatusCode::CONFLICT, cancel.message));
        }

        let finished_at = now_ts_string();
        let cancel_result_json = serde_json::json!({
            "runner": state.runner.name(),
            "ok": false,
            "status": "cancelled",
            "error_kind": "runner_cancelled",
            "failure_scope": "runner_cancelled",
            "execution_stage": "action",
            "task_id": task_id,
            "message": "task cancelled while running"
        }).to_string();

        sqlx::query(r#"UPDATE tasks SET status = ?, finished_at = ?, runner_id = NULL, heartbeat_at = NULL, result_json = ?, error_message = ? WHERE id = ?"#)
            .bind(TASK_STATUS_CANCELLED)
            .bind(&finished_at)
            .bind(&cancel_result_json)
            .bind("task cancelled while running")
            .bind(&task_id)
            .execute(&state.db)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to mark running task as cancelled: {err}"),
                )
            })?;

        let running_run_id = sqlx::query_scalar::<_, String>(
            &format!(
                "SELECT id FROM runs WHERE task_id = ? AND status = '{}' ORDER BY attempt DESC LIMIT 1",
                RUN_STATUS_RUNNING,
            ),
        )
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to fetch running run for cancel: {err}"),
            )
        })?;

        if let Some(run_id) = running_run_id.as_deref() {
            sqlx::query(r#"UPDATE runs SET status = ?, finished_at = ?, error_message = ?, result_json = COALESCE(result_json, ?) WHERE id = ?"#)
                .bind(RUN_STATUS_CANCELLED)
                .bind(&finished_at)
                .bind("task cancelled while running")
                .bind(&cancel_result_json)
                .bind(run_id)
                .execute(&state.db)
                .await
                .map_err(|err| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("failed to mark latest run as cancelled: {err}"),
                    )
                })?;

            insert_task_log(
                &state,
                &task_id,
                Some(run_id),
                "warn",
                &format!("task cancelled while running; {}", cancel.message),
            )
            .await?;
        } else {
            insert_task_log(
                &state,
                &task_id,
                None,
                "warn",
                &format!("task cancelled while running; {}", cancel.message),
            )
            .await?;
        }

        return Ok(Json(CancelTaskResponse {
            id: task_id,
            status: TASK_STATUS_CANCELLED.to_string(),
            message: cancel.message,
        }));
    }

    Err((
        StatusCode::BAD_REQUEST,
        format!("task status does not allow cancel: {status}"),
    ))
}


pub async fn create_fingerprint_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreateFingerprintProfileRequest>,
) -> Result<(StatusCode, Json<FingerprintProfileResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "fingerprint profile id and name are required".to_string()));
    }

    let validation = validate_fingerprint_profile(&payload.profile_json);
    let now = now_ts_string();
    let profile_json = payload.profile_json.to_string();

    sqlx::query(
        r#"
        INSERT INTO fingerprint_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
        VALUES (?, ?, 1, 'active', ?, ?, ?, ?)
        "#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(&payload.tags_json)
    .bind(&profile_json)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create fingerprint profile: {err}")))?;

    Ok((
        StatusCode::CREATED,
        Json(FingerprintProfileResponse {
            id: payload.id,
            name: payload.name,
            version: 1,
            status: "active".to_string(),
            tags_json: payload.tags_json,
            profile_json: payload.profile_json,
            validation_ok: validation.ok,
            validation_issues: validation.issues,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_fingerprint_profiles(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<FingerprintProfileResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, i64, String, Option<String>, String, String, String)>(
        r#"SELECT id, name, version, status, tags_json, profile_json, created_at, updated_at FROM fingerprint_profiles ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list fingerprint profiles: {err}")))?;

    let items = rows.into_iter().map(|(id, name, version, status, tags_json, profile_json, created_at, updated_at)| {
        let profile_json = serde_json::from_str(&profile_json).unwrap_or_else(|_| serde_json::json!({}));
        let validation = validate_fingerprint_profile(&profile_json);
        FingerprintProfileResponse { id, name, version, status, tags_json, profile_json, validation_ok: validation.ok, validation_issues: validation.issues, created_at, updated_at }
    }).collect();

    Ok(Json(items))
}

pub async fn get_fingerprint_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<FingerprintProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, String, i64, String, Option<String>, String, String, String)>(
        r#"SELECT id, name, version, status, tags_json, profile_json, created_at, updated_at FROM fingerprint_profiles WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch fingerprint profile: {err}")))?;

    match row {
        Some((id, name, version, status, tags_json, profile_json, created_at, updated_at)) => {
            let profile_json = serde_json::from_str(&profile_json).unwrap_or_else(|_| serde_json::json!({}));
            let validation = validate_fingerprint_profile(&profile_json);
            Ok(Json(FingerprintProfileResponse { id, name, version, status, tags_json, profile_json, validation_ok: validation.ok, validation_issues: validation.issues, created_at, updated_at }))
        }
        None => Err((StatusCode::NOT_FOUND, format!("fingerprint profile not found: {profile_id}"))),
    }
}


pub async fn create_proxy(
    State(state): State<AppState>,
    Json(payload): Json<CreateProxyRequest>,
) -> Result<(StatusCode, Json<ProxyResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.scheme.trim().is_empty() || payload.host.trim().is_empty() || payload.port <= 0 {
        return Err((StatusCode::BAD_REQUEST, "proxy id/scheme/host/port are required".to_string()));
    }

    if payload
        .status
        .as_deref()
        .map(is_candidate_proxy_status)
        .unwrap_or(false)
    {
        return create_or_refresh_candidate_proxy(&state, &payload).await;
    }

    let now = now_ts_string();
    let status = payload.status.unwrap_or_else(|| "active".to_string());
    let score = payload.score.unwrap_or(1.0);
    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, last_probe_latency_ms, last_probe_error, last_probe_error_category, last_verify_confidence, last_verify_score_delta, last_verify_source, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, ?, ?)"#)
        .bind(&payload.id).bind(&payload.scheme).bind(&payload.host).bind(payload.port)
        .bind(&payload.username).bind(&payload.password).bind(&payload.region).bind(&payload.country).bind(&payload.provider)
        .bind(&status).bind(score).bind(&now).bind(&now)
        .execute(&state.db).await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create proxy: {err}")))?;
    Ok((StatusCode::CREATED, Json(ProxyResponse {
        id: payload.id, scheme: payload.scheme, host: payload.host, port: payload.port, username: payload.username,
        region: payload.region, country: payload.country, provider: payload.provider, status, score, success_count: 0, failure_count: 0,
        last_checked_at: None, last_used_at: None, cooldown_until: None,
        last_smoke_status: None, last_smoke_protocol_ok: None, last_smoke_upstream_ok: None,
        last_exit_ip: None, last_anonymity_level: None, last_smoke_at: None,
        last_verify_status: None, last_verify_geo_match_ok: None, last_exit_country: None, last_exit_region: None, last_verify_at: None,
        last_probe_latency_ms: None, last_probe_error: None, last_probe_error_category: None, last_verify_confidence: None, last_verify_score_delta: None, last_verify_source: None,
        created_at: now.clone(), updated_at: now,
    })))
}

pub async fn list_proxies(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<ProxyResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, ProxyRow>(r#"SELECT id, scheme, host, port, username, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, last_probe_latency_ms, last_probe_error, last_probe_error_category, last_verify_confidence, last_verify_score_delta, last_verify_source, created_at, updated_at FROM proxies ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list proxies: {err}")))?;
    Ok(Json(rows.into_iter().map(map_proxy_row).collect()))
}

pub async fn get_proxy(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxyResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, ProxyRow>(r#"SELECT id, scheme, host, port, username, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, last_probe_latency_ms, last_probe_error, last_probe_error_category, last_verify_confidence, last_verify_score_delta, last_verify_source, created_at, updated_at FROM proxies WHERE id = ?"#)
        .bind(&proxy_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch proxy: {err}")))?;
    match row {
        Some(row) => Ok(Json(map_proxy_row(row))),
        None => Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}"))),
    }
}


pub async fn smoke_test_proxy(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxySmokeResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, i64)>(r#"SELECT host, port FROM proxies WHERE id = ?"#)
        .bind(&proxy_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load proxy for smoke test: {err}")))?;

    let Some((host, port)) = row else {
        return Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}")));
    };

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|err| (StatusCode::BAD_REQUEST, format!("proxy address is invalid for smoke test: {err}")))?;

    let started = Instant::now();
    let mut stream = tokio::time::timeout(std::time::Duration::from_secs(3), tokio::net::TcpStream::connect(addr))
        .await
        .ok()
        .and_then(|result| result.ok());
    let reachable = stream.is_some();
    let mut protocol_ok = false;
    let mut upstream_ok = false;
    let mut exit_ip: Option<String> = None;
    let mut anonymity_level: Option<String> = None;
    let mut smoke_message = if reachable {
        "tcp connect succeeded but proxy protocol not validated".to_string()
    } else {
        "tcp smoke test failed".to_string()
    };

    if let Some(stream_ref) = stream.as_mut() {
        let probe = b"CONNECT example.com:443 HTTP/1.1
Host: example.com:443

";
        if tokio::time::timeout(std::time::Duration::from_secs(3), stream_ref.write_all(probe)).await.ok().is_some() {
            let mut buf = [0_u8; 512];
            if let Ok(Ok(n)) = tokio::time::timeout(std::time::Duration::from_secs(3), stream_ref.read(&mut buf)).await {
                if n > 0 {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let text_lower = text.to_ascii_lowercase();
                    if text_lower.contains("http/1.1") || text_lower.contains("http/1.0") {
                        protocol_ok = true;
                        let has_via = text_lower.contains("via:");
                        let has_forwarded = text_lower.contains("forwarded:") || text_lower.contains("x-forwarded-for:");
                        anonymity_level = Some(if has_forwarded { "transparent".to_string() } else if has_via { "anonymous".to_string() } else { "elite".to_string() });
                        if let Some(idx) = text.find("ip=") {
                            let ip = text[idx + 3..].lines().next().unwrap_or("").trim().to_string();
                            if !ip.is_empty() {
                                upstream_ok = true;
                                exit_ip = Some(ip.clone());
                                smoke_message = format!("http proxy smoke test got upstream ip={ip}");
                            }
                        }
                        if !upstream_ok {
                            smoke_message = "http connect smoke test received proxy response".to_string();
                        }
                    } else {
                        smoke_message = format!("tcp connect ok but proxy response was not http-like: {text_lower}");
                    }
                }
            }
        }
    }

    let latency_ms = Some(started.elapsed().as_millis());
    let now = now_ts_string();

    if reachable && protocol_ok {
        sqlx::query(r#"UPDATE proxies SET last_checked_at = ?, cooldown_until = NULL, last_smoke_status = ?, last_smoke_protocol_ok = ?, last_smoke_upstream_ok = ?, last_exit_ip = ?, last_anonymity_level = ?, last_smoke_at = ?, updated_at = ? WHERE id = ?"#)
            .bind(&now)
            .bind("ok")
            .bind(1_i64)
            .bind(if upstream_ok { 1_i64 } else { 0_i64 })
            .bind(&exit_ip)
            .bind(&anonymity_level)
            .bind(&now)
            .bind(&now)
            .bind(&proxy_id)
            .execute(&state.db)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update proxy after smoke success: {err}")))?;

        Ok(Json(ProxySmokeResponse {
            id: proxy_id,
            reachable: true,
            protocol_ok: true,
            upstream_ok,
            exit_ip,
            anonymity_level,
            latency_ms,
            status: "ok".to_string(),
            message: smoke_message,
        }))
    } else {
        let cooldown_until = (now.parse::<u64>().unwrap_or(0) + 60).to_string();
        sqlx::query(r#"UPDATE proxies SET failure_count = failure_count + 1, last_checked_at = ?, cooldown_until = ?, last_smoke_status = ?, last_smoke_protocol_ok = ?, last_smoke_upstream_ok = ?, last_exit_ip = ?, last_anonymity_level = ?, last_smoke_at = ?, updated_at = ? WHERE id = ?"#)
            .bind(&now)
            .bind(&cooldown_until)
            .bind("failed")
            .bind(if protocol_ok { 1_i64 } else { 0_i64 })
            .bind(if upstream_ok { 1_i64 } else { 0_i64 })
            .bind(&exit_ip)
            .bind(&anonymity_level)
            .bind(&now)
            .bind(&now)
            .bind(&proxy_id)
            .execute(&state.db)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update proxy after smoke failure: {err}")))?;

        Ok(Json(ProxySmokeResponse {
            id: proxy_id,
            reachable,
            protocol_ok,
            upstream_ok,
            exit_ip,
            anonymity_level,
            latency_ms,
            status: "failed".to_string(),
            message: smoke_message,
        }))
    }
}




pub async fn list_verify_batches(
    State(state): State<AppState>,
    Query(query): Query<VerifyBatchListQuery>,
) -> Result<Json<Vec<VerifyBatchResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, i64, i64, i64, i64, i64, Option<String>, Option<String>, String, String)>(
        r#"SELECT id, status, requested_count, accepted_count, skipped_count, stale_after_seconds, task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at
           FROM verify_batches ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list verify batches: {err}")))?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(map_verify_batch_row(&state, row.0,row.1,row.2,row.3,row.4,row.5,row.6,row.7,row.8,row.9,row.10).await?);
    }
    Ok(Json(items))
}

pub async fn get_verify_batch(
    State(state): State<AppState>,
    Path(batch_id): Path<String>,
) -> Result<Json<VerifyBatchResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, String, i64, i64, i64, i64, i64, Option<String>, Option<String>, String, String)>(
        r#"SELECT id, status, requested_count, accepted_count, skipped_count, stale_after_seconds, task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at
           FROM verify_batches WHERE id = ?"#,
    )
    .bind(&batch_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch verify batch: {err}")))?;

    match row {
        Some(row) => Ok(Json(map_verify_batch_row(&state, row.0,row.1,row.2,row.3,row.4,row.5,row.6,row.7,row.8,row.9,row.10).await?)),
        None => Err((StatusCode::NOT_FOUND, format!("verify batch not found: {batch_id}"))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyReplenishBatchSummary {
    pub batch_id: String,
    pub reason: String,
    pub target_region: Option<String>,
    pub accepted: i64,
    pub proxy_ids: Vec<String>,
}

async fn build_proxy_inventory_snapshot(
    state: &AppState,
    target_region: Option<&str>,
) -> anyhow::Result<ProxyPoolInventorySnapshot> {
    let mode = proxy_runtime_mode_from_env();
    let now = now_ts_string();
    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM proxies p
           LEFT JOIN proxy_harvest_sources s ON s.source_label = p.source_label
           WHERE (
             (? = 'prod_live' AND (p.source_label IS NULL OR TRIM(p.source_label) = '' OR COALESCE(s.for_prod, 0) != 0))
             OR (? != 'prod_live' AND COALESCE(s.for_demo, 1) != 0)
           )
             AND (s.quarantine_until IS NULL OR CAST(s.quarantine_until AS INTEGER) <= CAST(? AS INTEGER))"#,
    )
    .bind(&mode)
    .bind(&mode)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let available: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM proxies p
           LEFT JOIN proxy_harvest_sources s ON s.source_label = p.source_label
           WHERE p.status = 'active'
             AND (
               (? = 'prod_live' AND (p.source_label IS NULL OR TRIM(p.source_label) = '' OR COALESCE(s.for_prod, 0) != 0))
               OR (? != 'prod_live' AND COALESCE(s.for_demo, 1) != 0)
             )
             AND (s.quarantine_until IS NULL OR CAST(s.quarantine_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (p.cooldown_until IS NULL OR CAST(p.cooldown_until AS INTEGER) <= CAST(? AS INTEGER))"#,
    )
    .bind(&mode)
    .bind(&mode)
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let available_in_region: i64 = match target_region {
        Some(region) => {
            sqlx::query_scalar(
                r#"SELECT COUNT(*)
                   FROM proxies p
                   LEFT JOIN proxy_harvest_sources s ON s.source_label = p.source_label
                   WHERE p.status = 'active'
                     AND p.region = ?
                     AND (
                       (? = 'prod_live' AND (p.source_label IS NULL OR TRIM(p.source_label) = '' OR COALESCE(s.for_prod, 0) != 0))
                       OR (? != 'prod_live' AND COALESCE(s.for_demo, 1) != 0)
                     )
                     AND (s.quarantine_until IS NULL OR CAST(s.quarantine_until AS INTEGER) <= CAST(? AS INTEGER))
                     AND (p.cooldown_until IS NULL OR CAST(p.cooldown_until AS INTEGER) <= CAST(? AS INTEGER))"#,
            )
            .bind(region)
            .bind(&mode)
            .bind(&mode)
            .bind(&now)
            .bind(&now)
            .fetch_one(&state.db)
            .await?
        }
        None => 0,
    };
    let inflight_tasks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE status IN ('queued', 'running')")
            .fetch_one(&state.db)
            .await?;
    Ok(ProxyPoolInventorySnapshot {
        total,
        available,
        region: target_region.map(str::to_string),
        available_in_region,
        inflight_tasks,
    })
}

async fn load_hot_browser_region_rows(
    state: &AppState,
    recent_window_seconds: Option<i64>,
    limit: i64,
) -> anyhow::Result<Vec<(String, i64)>> {
    let recent_filter = if recent_window_seconds.is_some() {
        "AND CAST(COALESCE(started_at, queued_at, finished_at, created_at, '0') AS INTEGER) >= CAST(? AS INTEGER) - ?"
    } else {
        ""
    };
    let status_filter = if recent_window_seconds.is_some() {
        ""
    } else {
        "AND status IN ('queued', 'running')"
    };
    let sql = format!(
        r#"SELECT region, COUNT(*) AS demand
           FROM (
               SELECT COALESCE(
                          NULLIF(TRIM(requested_region), ''),
                          NULLIF(TRIM(CAST(json_extract(input_json, '$.requested_region') AS TEXT)), ''),
                          NULLIF(TRIM(CAST(json_extract(input_json, '$.network_policy_json.region') AS TEXT)), ''),
                          NULLIF(TRIM(CAST(json_extract(network_policy_json, '$.region') AS TEXT)), '')
                      ) AS region
               FROM tasks
               WHERE kind IN ('open_page', 'get_html', 'get_title', 'get_final_url', 'extract_text')
                 {status_filter}
                 AND LOWER(REPLACE(COALESCE(proxy_mode, CAST(json_extract(input_json, '$.proxy_mode') AS TEXT), ?), '-', '_')) = ?
                 {recent_filter}
           ) demand_rows
           WHERE region IS NOT NULL AND TRIM(region) != ''
           GROUP BY region
           ORDER BY demand DESC, region ASC
           LIMIT ?"#,
    );
    let mut query = sqlx::query_as::<_, (String, i64)>(&sql)
        .bind(&state.proxy_runtime_mode)
        .bind(&state.proxy_runtime_mode);
    if let Some(window_seconds) = recent_window_seconds {
        let now = now_ts_string();
        query = query.bind(now).bind(window_seconds);
    }
    Ok(query.bind(limit.max(1)).fetch_all(&state.db).await?)
}

async fn load_hot_browser_regions(state: &AppState) -> anyhow::Result<Vec<String>> {
    Ok(load_hot_browser_region_rows(state, None, 3)
        .await?
        .into_iter()
        .map(|(region, _)| region)
        .collect())
}

async fn load_recent_hot_browser_regions(
    state: &AppState,
    window_seconds: i64,
    limit: i64,
) -> anyhow::Result<Vec<(String, i64)>> {
    load_hot_browser_region_rows(state, Some(window_seconds.max(1)), limit).await
}

#[derive(Debug, Clone, Default)]
struct ActiveProxyBalanceSnapshot {
    total_active: i64,
    top1_source_key: Option<String>,
    top1_provider_key: Option<String>,
    active_by_source: std::collections::BTreeMap<String, i64>,
    active_by_provider: std::collections::BTreeMap<String, i64>,
    active_by_region: std::collections::BTreeMap<String, i64>,
    recent_hot_regions: Vec<String>,
}

#[derive(Debug, Clone)]
struct BalanceCandidateRow {
    proxy_id: String,
    provider: Option<String>,
    provider_key: String,
    source_key: String,
    region_key: Option<String>,
    original_index: usize,
}

async fn load_active_proxy_balance_snapshot(
    state: &AppState,
) -> anyhow::Result<ActiveProxyBalanceSnapshot> {
    let now_ts = now_ts_string().parse::<i64>().unwrap_or_default();
    let rows = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<String>,
        ),
    >(
        r#"SELECT
               p.source_label,
               s.source_tier,
               p.region,
               p.provider,
               s.for_demo,
               s.for_prod,
               s.quarantine_until
           FROM proxies p
           LEFT JOIN proxy_harvest_sources s ON s.source_label = p.source_label
           WHERE p.status = 'active'"#,
    )
    .fetch_all(&state.db)
    .await?;

    let mut snapshot = ActiveProxyBalanceSnapshot::default();
    for (source_label, source_tier, region, provider, for_demo, for_prod, quarantine_until) in rows
    {
        let metadata = crate::network_identity::proxy_harvest::ProxySourceRuntimeMetadata {
            source_label: source_label.clone(),
            source_tier,
            for_demo: for_demo.unwrap_or(1) != 0,
            for_prod: for_prod.unwrap_or(0) != 0,
            validation_mode: None,
            expected_geo_quality: None,
            cost_class: None,
            quarantine_until,
        };
        if !crate::network_identity::proxy_harvest::proxy_source_is_eligible_for_mode(
            &state.proxy_runtime_mode,
            &metadata,
            Some(now_ts),
        ) {
            continue;
        }
        snapshot.total_active += 1;
        let source_key = normalized_balance_label(source_label.as_deref(), "__unknown_source__");
        *snapshot.active_by_source.entry(source_key).or_insert(0) += 1;
        let provider_key = normalized_balance_label(provider.as_deref(), "__unknown_provider__");
        *snapshot.active_by_provider.entry(provider_key).or_insert(0) += 1;
        if let Some(region_key) = normalize_optional_task_text(region.as_deref()) {
            *snapshot.active_by_region.entry(region_key).or_insert(0) += 1;
        }
    }
    snapshot.top1_source_key = snapshot
        .active_by_source
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(key, _)| key.clone());
    snapshot.top1_provider_key = snapshot
        .active_by_provider
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(key, _)| key.clone());
    snapshot.recent_hot_regions = load_recent_hot_browser_regions(
        state,
        HOT_REGION_WINDOW_SECONDS,
        HOT_REGION_LIMIT,
    )
    .await?
    .into_iter()
    .map(|(region, _)| region)
    .collect();
    Ok(snapshot)
}

fn source_is_overweight(snapshot: &ActiveProxyBalanceSnapshot, source_key: &str) -> bool {
    if snapshot.total_active <= 0 {
        return false;
    }
    let top1_key = match snapshot.top1_source_key.as_deref() {
        Some(value) => value,
        None => return false,
    };
    if top1_key != source_key {
        return false;
    }
    let top1_count = snapshot
        .active_by_source
        .get(source_key)
        .copied()
        .unwrap_or(0);
    if top1_count <= 0 {
        return false;
    }
    round_percent(top1_count, snapshot.total_active) > SOURCE_CONCENTRATION_TARGET_CAP_PERCENT
}

fn provider_is_overweight(snapshot: &ActiveProxyBalanceSnapshot, provider_key: &str) -> bool {
    if snapshot.total_active <= 0 {
        return false;
    }
    let top1_key = match snapshot.top1_provider_key.as_deref() {
        Some(value) => value,
        None => return false,
    };
    if top1_key != provider_key {
        return false;
    }
    let top1_count = snapshot
        .active_by_provider
        .get(provider_key)
        .copied()
        .unwrap_or(0);
    if top1_count <= 0 {
        return false;
    }
    round_percent(top1_count, snapshot.total_active) > SOURCE_CONCENTRATION_TARGET_CAP_PERCENT
}

fn sort_balance_candidates(
    mut rows: Vec<BalanceCandidateRow>,
    snapshot: &ActiveProxyBalanceSnapshot,
    target_region: Option<&str>,
) -> Vec<BalanceCandidateRow> {
    let target_region_key =
        target_region.and_then(|value| normalize_optional_task_text(Some(value)));
    let has_understocked_source = rows.iter().any(|row| {
        snapshot
            .active_by_source
            .get(&row.source_key)
            .copied()
            .unwrap_or(0)
            < ACTIVE_INVENTORY_MIN
    });
    if has_understocked_source {
        let filtered_rows = rows
            .iter()
            .filter(|row| {
                snapshot
                    .active_by_source
                    .get(&row.source_key)
                    .copied()
                    .unwrap_or(0)
                    < ACTIVE_INVENTORY_MIN
            })
            .cloned()
            .collect::<Vec<_>>();
        if !filtered_rows.is_empty() {
            rows = filtered_rows;
        }
    }
    let has_non_overweight_source = rows
        .iter()
        .any(|row| !source_is_overweight(snapshot, &row.source_key));
    if has_non_overweight_source {
        let filtered_rows = rows
            .iter()
            .filter(|row| !source_is_overweight(snapshot, &row.source_key))
            .cloned()
            .collect::<Vec<_>>();
        if !filtered_rows.is_empty() {
            rows = filtered_rows;
        }
    }
    let has_understocked_provider = rows.iter().any(|row| {
        snapshot
            .active_by_provider
            .get(&row.provider_key)
            .copied()
            .unwrap_or(0)
            < ACTIVE_INVENTORY_MIN
    });
    if has_understocked_provider {
        let filtered_rows = rows
            .iter()
            .filter(|row| {
                snapshot
                    .active_by_provider
                    .get(&row.provider_key)
                    .copied()
                    .unwrap_or(0)
                    < ACTIVE_INVENTORY_MIN
            })
            .cloned()
            .collect::<Vec<_>>();
        if !filtered_rows.is_empty() {
            rows = filtered_rows;
        }
    }
    let has_non_overweight_provider = rows
        .iter()
        .any(|row| !provider_is_overweight(snapshot, &row.provider_key));
    if has_non_overweight_provider {
        let filtered_rows = rows
            .iter()
            .filter(|row| !provider_is_overweight(snapshot, &row.provider_key))
            .cloned()
            .collect::<Vec<_>>();
        if !filtered_rows.is_empty() {
            rows = filtered_rows;
        }
    }
    rows.sort_by(|left, right| {
        let left_source_count = snapshot
            .active_by_source
            .get(&left.source_key)
            .copied()
            .unwrap_or(0);
        let right_source_count = snapshot
            .active_by_source
            .get(&right.source_key)
            .copied()
            .unwrap_or(0);
        let left_provider_count = snapshot
            .active_by_provider
            .get(&left.provider_key)
            .copied()
            .unwrap_or(0);
        let right_provider_count = snapshot
            .active_by_provider
            .get(&right.provider_key)
            .copied()
            .unwrap_or(0);
        let left_region_count = left
            .region_key
            .as_ref()
            .and_then(|value| snapshot.active_by_region.get(value))
            .copied()
            .unwrap_or(0);
        let right_region_count = right
            .region_key
            .as_ref()
            .and_then(|value| snapshot.active_by_region.get(value))
            .copied()
            .unwrap_or(0);
        let left_is_hot = left
            .region_key
            .as_ref()
            .map(|value| snapshot.recent_hot_regions.iter().any(|item| item == value))
            .unwrap_or(false);
        let right_is_hot = right
            .region_key
            .as_ref()
            .map(|value| snapshot.recent_hot_regions.iter().any(|item| item == value))
            .unwrap_or(false);
        let left_matches_target = target_region_key
            .as_ref()
            .zip(left.region_key.as_ref())
            .map(|(target, current)| target == current)
            .unwrap_or(false);
        let right_matches_target = target_region_key
            .as_ref()
            .zip(right.region_key.as_ref())
            .map(|(target, current)| target == current)
            .unwrap_or(false);
        let left_key = (
            if left_matches_target { 0 } else { 1 },
            if left_source_count < ACTIVE_INVENTORY_MIN { 0 } else { 1 },
            if source_is_overweight(snapshot, &left.source_key) {
                1
            } else {
                0
            },
            left_source_count,
            if left_provider_count < ACTIVE_INVENTORY_MIN {
                0
            } else {
                1
            },
            if provider_is_overweight(snapshot, &left.provider_key) {
                1
            } else {
                0
            },
            left_provider_count,
            if target_region_key.is_none() && left_is_hot { 0 } else { 1 },
            if left_region_count < ACTIVE_INVENTORY_MIN {
                0
            } else {
                1
            },
            left_region_count,
            left.original_index,
        );
        let right_key = (
            if right_matches_target { 0 } else { 1 },
            if right_source_count < ACTIVE_INVENTORY_MIN {
                0
            } else {
                1
            },
            if source_is_overweight(snapshot, &right.source_key) {
                1
            } else {
                0
            },
            right_source_count,
            if right_provider_count < ACTIVE_INVENTORY_MIN {
                0
            } else {
                1
            },
            if provider_is_overweight(snapshot, &right.provider_key) {
                1
            } else {
                0
            },
            right_provider_count,
            if target_region_key.is_none() && right_is_hot {
                0
            } else {
                1
            },
            if right_region_count < ACTIVE_INVENTORY_MIN {
                0
            } else {
                1
            },
            right_region_count,
            right.original_index,
        );
        left_key.cmp(&right_key)
    });
    rows
}

async fn recent_replenish_batch_exists(
    state: &AppState,
    reason: &str,
    target_region: Option<&str>,
    now: &str,
) -> anyhow::Result<bool> {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM verify_batches
           WHERE CAST(created_at AS INTEGER) >= CAST(? AS INTEGER) - 300
             AND json_extract(filters_json, '$.reason') = ?
             AND (
               (? IS NULL AND json_extract(filters_json, '$.target_region') IS NULL)
               OR json_extract(filters_json, '$.target_region') = ?
             )"#,
    )
    .bind(now)
    .bind(reason)
    .bind(target_region)
    .bind(target_region)
    .fetch_one(&state.db)
    .await?;
    Ok(count > 0)
}

async fn select_replenish_candidate_rows(
    state: &AppState,
    target_region: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<(String, Option<String>)>> {
    let sql = if target_region.is_some() {
        r#"SELECT p.id, p.provider, p.source_label, p.region, s.source_tier, s.for_demo, s.for_prod, s.quarantine_until
           FROM proxies p
           LEFT JOIN proxy_harvest_sources s ON s.source_label = p.source_label
           LEFT JOIN (
               SELECT source_label, COUNT(*) AS active_source_count
               FROM proxies
               WHERE status = 'active'
               GROUP BY source_label
           ) sa ON sa.source_label = p.source_label
           LEFT JOIN (
               SELECT provider, COUNT(*) AS active_provider_count
               FROM proxies
               WHERE status = 'active' AND provider IS NOT NULL AND TRIM(provider) != ''
               GROUP BY provider
           ) pa ON pa.provider = p.provider
           LEFT JOIN (
               SELECT region, COUNT(*) AS active_region_count
               FROM proxies
               WHERE status = 'active'
               GROUP BY region
           ) ra ON ra.region = p.region
           WHERE p.status IN ('candidate', 'candidate_rejected')
             AND p.region = ?
             AND (p.cooldown_until IS NULL OR CAST(p.cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
           ORDER BY
             CASE WHEN COALESCE(sa.active_source_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(sa.active_source_count, 0) ASC,
             CASE WHEN COALESCE(pa.active_provider_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(pa.active_provider_count, 0) ASC,
             CASE WHEN COALESCE(ra.active_region_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(ra.active_region_count, 0) ASC,
             CASE
               WHEN p.provider IS NULL OR TRIM(p.provider) = '' OR p.region IS NULL OR TRIM(p.region) = '' THEN 1
               ELSE 0
             END ASC,
             COALESCE(s.health_score, 0.0) DESC,
             CASE p.status WHEN 'candidate' THEN 0 ELSE 1 END ASC,
             CASE
               WHEN p.last_probe_error_category = 'connect_failed' THEN 2
               WHEN p.last_probe_error_category = 'upstream_missing' THEN 1
               ELSE 0
             END ASC,
             COALESCE(CAST(p.last_seen_at AS INTEGER), CAST(p.created_at AS INTEGER)) DESC,
             p.created_at DESC,
             p.id ASC
           LIMIT ?"#
    } else {
        r#"SELECT p.id, p.provider, p.source_label, p.region, s.source_tier, s.for_demo, s.for_prod, s.quarantine_until
           FROM proxies p
           LEFT JOIN proxy_harvest_sources s ON s.source_label = p.source_label
           LEFT JOIN (
               SELECT source_label, COUNT(*) AS active_source_count
               FROM proxies
               WHERE status = 'active'
               GROUP BY source_label
           ) sa ON sa.source_label = p.source_label
           LEFT JOIN (
               SELECT provider, COUNT(*) AS active_provider_count
               FROM proxies
               WHERE status = 'active' AND provider IS NOT NULL AND TRIM(provider) != ''
               GROUP BY provider
           ) pa ON pa.provider = p.provider
           LEFT JOIN (
               SELECT region, COUNT(*) AS active_region_count
               FROM proxies
               WHERE status = 'active'
               GROUP BY region
           ) ra ON ra.region = p.region
           WHERE p.status IN ('candidate', 'candidate_rejected')
             AND (p.cooldown_until IS NULL OR CAST(p.cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
           ORDER BY
             CASE WHEN COALESCE(sa.active_source_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(sa.active_source_count, 0) ASC,
             CASE WHEN COALESCE(pa.active_provider_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(pa.active_provider_count, 0) ASC,
             CASE WHEN COALESCE(ra.active_region_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(ra.active_region_count, 0) ASC,
             CASE
               WHEN p.provider IS NULL OR TRIM(p.provider) = '' OR p.region IS NULL OR TRIM(p.region) = '' THEN 1
               ELSE 0
             END ASC,
             COALESCE(s.health_score, 0.0) DESC,
             CASE p.status WHEN 'candidate' THEN 0 ELSE 1 END ASC,
             CASE
               WHEN p.last_probe_error_category = 'connect_failed' THEN 2
               WHEN p.last_probe_error_category = 'upstream_missing' THEN 1
               ELSE 0
             END ASC,
             COALESCE(CAST(p.last_seen_at AS INTEGER), CAST(p.created_at AS INTEGER)) DESC,
             p.created_at DESC,
             p.id ASC
           LIMIT ?"#
    };
    let now = now_ts_string();
    let now_ts = now.parse::<i64>().unwrap_or_default();
    let balance_snapshot = load_active_proxy_balance_snapshot(state).await?;
    let raw_rows = if let Some(region) = target_region {
        sqlx::query_as::<
            _,
            (
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<i64>,
                Option<i64>,
                Option<String>,
            ),
        >(sql)
        .bind(region)
        .bind(&now)
        .bind(limit.max(1) * 4)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<
            _,
            (
                String,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<i64>,
                Option<i64>,
                Option<String>,
            ),
        >(sql)
        .bind(&now)
        .bind(limit.max(1) * 4)
        .fetch_all(&state.db)
        .await?
    };
    let eligible_rows = raw_rows
        .into_iter()
        .enumerate()
        .filter_map(
            |(
                original_index,
                (
                proxy_id,
                provider,
                source_label,
                region,
                source_tier,
                for_demo,
                for_prod,
                quarantine_until,
            ),
            )| {
                let metadata = crate::network_identity::proxy_harvest::ProxySourceRuntimeMetadata {
                    source_label: source_label.clone(),
                    source_tier,
                    for_demo: for_demo.unwrap_or(1) != 0,
                    for_prod: for_prod.unwrap_or(0) != 0,
                    validation_mode: None,
                    expected_geo_quality: None,
                    cost_class: None,
                    quarantine_until,
                };
                crate::network_identity::proxy_harvest::proxy_source_is_eligible_for_mode(
                    &state.proxy_runtime_mode,
                    &metadata,
                    Some(now_ts),
                )
                .then_some({
                    let provider_key =
                        normalized_balance_label(provider.as_deref(), "__unknown_provider__");
                    BalanceCandidateRow {
                        proxy_id,
                        provider,
                        provider_key,
                        source_key: normalized_balance_label(
                            source_label.as_deref(),
                            "__unknown_source__",
                        ),
                        region_key: normalize_optional_task_text(region.as_deref()),
                        original_index,
                    }
                })
            },
        )
        .collect::<Vec<_>>();
    Ok(sort_balance_candidates(eligible_rows, &balance_snapshot, target_region)
        .into_iter()
        .take(limit.max(1) as usize)
        .map(|row| (row.proxy_id, row.provider))
        .collect())
}

async fn schedule_replenish_verify_batch(
    state: &AppState,
    proxy_rows: &[(String, Option<String>)],
    reason: &str,
    target_region: Option<&str>,
    task_timeout_seconds: i64,
) -> anyhow::Result<Option<ProxyReplenishBatchSummary>> {
    if proxy_rows.is_empty() {
        return Ok(None);
    }

    let now = now_ts_string();
    let batch_id = format!("verify-batch-{}", Uuid::new_v4());
    let mut per_provider_counts = std::collections::BTreeMap::<String, i64>::new();
    let mut proxy_ids = Vec::with_capacity(proxy_rows.len());
    let mut tx = state.db.begin().await?;
    for (proxy_id, provider) in proxy_rows {
        let task_id = format!("task-{}", Uuid::new_v4());
        let created_at = now_ts_string();
        let claimed_proxy_id = sqlx::query_scalar::<_, String>(
            r#"UPDATE proxies
               SET last_seen_at = ?, updated_at = ?
               WHERE id = ?
                 AND status IN ('candidate', 'candidate_rejected')
               RETURNING id"#,
        )
        .bind(&created_at)
        .bind(&created_at)
        .bind(proxy_id)
        .fetch_optional(&mut *tx)
        .await?;
        if claimed_proxy_id.is_none() {
            continue;
        }
        let proxy_mode = normalize_proxy_mode_for_task(
            Some(&state.proxy_runtime_mode),
            &state.proxy_runtime_mode,
        );
        let requested_region =
            target_region.and_then(|value| normalize_optional_task_text(Some(value)));
        let input_json = serde_json::json!({
            "url": null,
            "script": null,
            "timeout_seconds": task_timeout_seconds,
            "fingerprint_profile_id": null,
            "fingerprint_profile_version": null,
            "proxy_id": proxy_id,
            "requested_region": requested_region,
            "proxy_mode": proxy_mode,
            "verify_batch_id": batch_id,
            "network_policy_json": null,
        })
        .to_string();
        sqlx::query(
            r#"INSERT INTO tasks (
                   id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
                   proxy_id, requested_region, proxy_mode,
                   priority, created_at, queued_at, started_at, finished_at, fingerprint_profile_id,
                   fingerprint_profile_version, runner_id, heartbeat_at, result_json, error_message
               ) VALUES (?, 'verify_proxy', ?, ?, NULL, NULL, ?, ?, ?, 0, ?, ?, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)"#,
        )
        .bind(&task_id)
        .bind(TASK_STATUS_QUEUED)
        .bind(&input_json)
        .bind(proxy_id)
        .bind(&requested_region)
        .bind(&proxy_mode)
        .bind(&created_at)
        .bind(&created_at)
        .execute(&mut *tx)
        .await?;
        let provider_key = provider.clone().unwrap_or_else(|| "__none__".to_string());
        *per_provider_counts.entry(provider_key).or_insert(0) += 1;
        proxy_ids.push(proxy_id.clone());
    }

    let accepted = i64::try_from(proxy_ids.len()).unwrap_or(0);
    if accepted == 0 {
        tx.rollback().await?;
        return Ok(None);
    }

    let provider_summary: Vec<ProxyVerifyBatchProviderSummary> = per_provider_counts
        .into_iter()
        .map(|(provider, accepted)| ProxyVerifyBatchProviderSummary {
            provider,
            accepted,
            skipped_due_to_cap: 0,
        })
        .collect();
    let provider_summary_json = serde_json::to_string(&provider_summary)?;
    let filters_json = serde_json::json!({
        "reason": reason,
        "candidate_mode": true,
        "target_region": target_region,
        "limit": proxy_rows.len(),
        "task_timeout_seconds": task_timeout_seconds,
        "mode": state.proxy_runtime_mode,
    })
    .to_string();
    sqlx::query(
        r#"INSERT INTO verify_batches (id, status, requested_count, accepted_count, skipped_count, stale_after_seconds, task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at)
           VALUES (?, 'scheduled', ?, ?, ?, 0, ?, ?, ?, ?, ?)"#,
    )
    .bind(&batch_id)
    .bind(i64::try_from(proxy_rows.len()).unwrap_or(0))
    .bind(accepted)
    .bind(i64::try_from(proxy_rows.len()).unwrap_or(0) - accepted)
    .bind(task_timeout_seconds)
    .bind(&provider_summary_json)
    .bind(&filters_json)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Some(ProxyReplenishBatchSummary {
        batch_id,
        reason: reason.to_string(),
        target_region: target_region.map(str::to_string),
        accepted,
        proxy_ids,
    }))
}

pub async fn run_proxy_replenish_mvp_tick(
    state: &AppState,
) -> anyhow::Result<Vec<ProxyReplenishBatchSummary>> {
    let policy = proxy_pool_growth_policy_from_env();
    let now = now_ts_string();
    let mut scheduled_batches = Vec::new();
    let mut reserved_proxy_ids = std::collections::HashSet::<String>::new();
    let mut remaining_budget = proxy_replenish_total_batch_limit_from_env();
    let region_batch_limit = proxy_replenish_region_batch_limit_from_env();
    let global_batch_limit = proxy_replenish_global_batch_limit_from_env();

    for region in load_hot_browser_regions(state).await? {
        if remaining_budget <= 0 {
            break;
        }

        let snapshot = build_proxy_inventory_snapshot(state, Some(region.as_str())).await?;
        let health = assess_proxy_pool_health(&snapshot, &policy);
        if !health.below_min_region {
            continue;
        }
        if recent_replenish_batch_exists(state, "replenish_mvp", Some(region.as_str()), &now).await? {
            continue;
        }

        let selected = select_replenish_candidate_rows(state, Some(region.as_str()), remaining_budget.min(region_batch_limit)).await?
            .into_iter()
            .filter(|(proxy_id, _)| !reserved_proxy_ids.contains(proxy_id))
            .take(remaining_budget.min(region_batch_limit) as usize)
            .collect::<Vec<_>>();
        if let Some(summary) = schedule_replenish_verify_batch(state, &selected, "replenish_mvp", Some(region.as_str()), 5).await? {
            remaining_budget -= summary.accepted;
            reserved_proxy_ids.extend(summary.proxy_ids.iter().cloned());
            scheduled_batches.push(summary);
        }
    }

    if remaining_budget > 0 {
        let snapshot = build_proxy_inventory_snapshot(state, None).await?;
        let health = assess_proxy_pool_health(&snapshot, &policy);
        if (health.below_min_ratio || health.below_min_total)
            && !recent_replenish_batch_exists(state, "replenish_mvp", None, &now).await?
        {
            let selected = select_replenish_candidate_rows(state, None, remaining_budget.min(global_batch_limit)).await?
                .into_iter()
                .filter(|(proxy_id, _)| !reserved_proxy_ids.contains(proxy_id))
                .take(remaining_budget.min(global_batch_limit) as usize)
                .collect::<Vec<_>>();
            if let Some(summary) =
                schedule_replenish_verify_batch(state, &selected, "replenish_mvp", None, 5).await?
            {
                scheduled_batches.push(summary);
            }
        }
    }

    Ok(scheduled_batches)
}

pub async fn verify_batch_proxies(
    State(state): State<AppState>,
    Json(payload): Json<ProxyVerifyBatchRequest>,
) -> Result<(StatusCode, Json<ProxyVerifyBatchResponse>), (StatusCode, String)> {
    let requested = sanitize_limit(payload.limit, 20, 200);
    let min_score = payload.min_score.unwrap_or(0.0);
    let only_stale = payload.only_stale.unwrap_or(true);
    let stale_after_seconds = payload.stale_after_seconds.unwrap_or(3600).max(60);
    let task_timeout_seconds = payload.task_timeout_seconds.unwrap_or(5).max(1);
    let recently_used_within_seconds = payload.recently_used_within_seconds.unwrap_or(0).max(0);
    let failed_only = payload.failed_only.unwrap_or(false);
    let max_per_provider = payload.max_per_provider.unwrap_or(requested).max(1);
    let now = now_ts_string();
    let now_ts = now.parse::<i64>().unwrap_or_default();
    let mode = state.proxy_runtime_mode.clone();
    let batch_id = format!("verify-batch-{}", Uuid::new_v4());
    let mut tx = state.db.begin().await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to start verify batch transaction: {err}"),
        )
    })?;
    let rows = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<String>,
        ),
    >(
        r#"SELECT
               p.id,
               p.provider,
               p.source_label,
               p.region,
               s.source_tier,
               s.for_demo,
               s.for_prod,
               s.quarantine_until
           FROM proxies p
           LEFT JOIN proxy_harvest_sources s ON s.source_label = p.source_label
           LEFT JOIN (
               SELECT source_label, COUNT(*) AS active_source_count
               FROM proxies
               WHERE status = 'active'
               GROUP BY source_label
           ) sa ON sa.source_label = p.source_label
           LEFT JOIN (
               SELECT provider, COUNT(*) AS active_provider_count
               FROM proxies
               WHERE status = 'active' AND provider IS NOT NULL AND TRIM(provider) != ''
               GROUP BY provider
           ) pa ON pa.provider = p.provider
           LEFT JOIN (
               SELECT region, COUNT(*) AS active_region_count
               FROM proxies
               WHERE status = 'active'
               GROUP BY region
           ) ra ON ra.region = p.region
           WHERE p.status = 'active'
             AND (? IS NULL OR p.provider = ?)
             AND (? IS NULL OR p.region = ?)
             AND score >= ?
             AND (
               p.source_label IS NULL
               OR (
                 ? = 'prod_live'
                 AND COALESCE(s.for_prod, 0) != 0
               )
               OR (
                 ? != 'prod_live'
                 AND COALESCE(s.for_demo, 1) != 0
               )
             )
             AND (
               s.quarantine_until IS NULL
               OR CAST(s.quarantine_until AS INTEGER) <= CAST(? AS INTEGER)
             )
             AND (
               ? = 0
               OR last_verify_at IS NULL
               OR last_verify_status IS NULL
               OR last_verify_status != 'ok'
               OR CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - ?
             )
             AND (
               ? = 0
               OR CAST(COALESCE(last_used_at, '0') AS INTEGER) >= CAST(? AS INTEGER) - ?
             )
             AND (
               ? = 0
               OR last_verify_status = 'failed'
             )
           ORDER BY
             CASE WHEN COALESCE(sa.active_source_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(sa.active_source_count, 0) ASC,
             CASE WHEN COALESCE(pa.active_provider_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(pa.active_provider_count, 0) ASC,
             CASE WHEN COALESCE(ra.active_region_count, 0) < 5 THEN 0 ELSE 1 END ASC,
             COALESCE(ra.active_region_count, 0) ASC,
             CASE WHEN last_verify_status = 'ok' THEN 1 ELSE 0 END ASC,
             COALESCE(last_verify_at, '0') ASC,
             p.created_at ASC
           LIMIT ?"#,
    )
    .bind(&payload.provider)
    .bind(&payload.provider)
    .bind(&payload.region)
    .bind(&payload.region)
    .bind(min_score)
    .bind(&mode)
    .bind(&mode)
    .bind(&now)
    .bind(if only_stale { 1_i64 } else { 0_i64 })
    .bind(&now)
    .bind(stale_after_seconds)
    .bind(if recently_used_within_seconds > 0 {
        1_i64
    } else {
        0_i64
    })
    .bind(&now)
    .bind(recently_used_within_seconds)
    .bind(if failed_only { 1_i64 } else { 0_i64 })
    .bind(requested.saturating_mul(4))
    .fetch_all(&mut *tx)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to select proxies for verify batch: {err}"),
        )
    })?;

    let balance_snapshot = load_active_proxy_balance_snapshot(&state)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to load active proxy balance snapshot: {err}"),
            )
        })?;
    let eligible_rows = rows
        .into_iter()
        .enumerate()
        .filter_map(
            |(
                original_index,
                (
                    proxy_id,
                    provider,
                    source_label,
                    region,
                    source_tier,
                    for_demo,
                    for_prod,
                    quarantine_until,
                ),
            )| {
                let metadata = crate::network_identity::proxy_harvest::ProxySourceRuntimeMetadata {
                    source_label: source_label.clone(),
                    source_tier,
                    for_demo: for_demo.unwrap_or(1) != 0,
                    for_prod: for_prod.unwrap_or(0) != 0,
                    validation_mode: None,
                    expected_geo_quality: None,
                    cost_class: None,
                    quarantine_until,
                };
                crate::network_identity::proxy_harvest::proxy_source_is_eligible_for_mode(
                    &mode,
                    &metadata,
                    Some(now_ts),
                )
                .then_some({
                    let provider_key =
                        normalized_balance_label(provider.as_deref(), "__unknown_provider__");
                    BalanceCandidateRow {
                        proxy_id,
                        provider,
                        provider_key,
                        source_key: normalized_balance_label(
                            source_label.as_deref(),
                            "__unknown_source__",
                        ),
                        region_key: normalize_optional_task_text(region.as_deref()),
                        original_index,
                    }
                })
            },
        )
        .collect::<Vec<_>>();
    let sorted_rows = sort_balance_candidates(
        eligible_rows,
        &balance_snapshot,
        payload.region.as_deref(),
    );
    let mut accepted = 0_i64;
    let mut per_provider_counts = std::collections::BTreeMap::<String, i64>::new();
    let mut per_provider_skipped = std::collections::BTreeMap::<String, i64>::new();
    for row in &sorted_rows {
        if accepted >= requested {
            break;
        }
        let provider_key = row
            .provider
            .clone()
            .unwrap_or_else(|| "__none__".to_string());
        let current = *per_provider_counts.get(&provider_key).unwrap_or(&0);
        if current >= max_per_provider {
            *per_provider_skipped.entry(provider_key).or_insert(0) += 1;
            continue;
        }
        let task_id = format!("task-{}", Uuid::new_v4());
        let created_at = now_ts_string();
        let claimed_proxy_id = sqlx::query_scalar::<_, String>(
            r#"UPDATE proxies
               SET last_seen_at = ?, updated_at = ?
               WHERE id = ?
                 AND status = 'active'
               RETURNING id"#,
        )
        .bind(&created_at)
        .bind(&created_at)
        .bind(&row.proxy_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to claim verify proxy: {err}"),
            )
        })?;
        if claimed_proxy_id.is_none() {
            continue;
        }
        let proxy_mode = normalize_proxy_mode_for_task(Some(&mode), &mode);
        let requested_region = payload
            .region
            .as_deref()
            .and_then(|value| normalize_optional_task_text(Some(value)));
        let input_json = serde_json::json!({
            "url": null,
            "script": null,
            "timeout_seconds": task_timeout_seconds,
            "fingerprint_profile_id": null,
            "fingerprint_profile_version": null,
            "proxy_id": &row.proxy_id,
            "requested_region": requested_region,
            "proxy_mode": proxy_mode,
            "verify_batch_id": batch_id,
            "network_policy_json": null,
        })
        .to_string();
        sqlx::query(
            r#"INSERT INTO tasks (
                id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
                proxy_id, requested_region, proxy_mode,
                priority, created_at, queued_at, started_at, finished_at, fingerprint_profile_id,
                fingerprint_profile_version, runner_id, heartbeat_at, result_json, error_message
            ) VALUES (?, 'verify_proxy', ?, ?, NULL, NULL, ?, ?, ?, 0, ?, ?, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)"#,
        )
        .bind(&task_id)
        .bind(TASK_STATUS_QUEUED)
        .bind(&input_json)
        .bind(&row.proxy_id)
        .bind(&requested_region)
        .bind(&proxy_mode)
        .bind(&created_at)
        .bind(&created_at)
        .execute(&mut *tx)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to enqueue verify task: {err}")))?;
        accepted += 1;
        per_provider_counts.insert(provider_key, current + 1);
    }

    let provider_summary: Vec<ProxyVerifyBatchProviderSummary> = per_provider_counts
        .into_iter()
        .map(|(provider, accepted)| ProxyVerifyBatchProviderSummary {
            skipped_due_to_cap: per_provider_skipped.get(&provider).copied().unwrap_or(0),
            provider,
            accepted,
        })
        .collect();
    let provider_summary_json = serde_json::to_string(&provider_summary).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode provider summary: {err}"),
        )
    })?;
    let filters_json = serde_json::json!({
        "provider": payload.provider,
        "region": payload.region,
        "limit": requested,
        "only_stale": only_stale,
        "min_score": min_score,
        "stale_after_seconds": stale_after_seconds,
        "task_timeout_seconds": task_timeout_seconds,
        "recently_used_within_seconds": recently_used_within_seconds,
        "failed_only": failed_only,
        "max_per_provider": max_per_provider,
        "mode": mode,
    })
    .to_string();
    sqlx::query(r#"INSERT INTO verify_batches (id, status, requested_count, accepted_count, skipped_count, stale_after_seconds, task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at)
                   VALUES (?, 'scheduled', ?, ?, ?, ?, ?, ?, ?, ?, ?)"#)
        .bind(&batch_id)
        .bind(requested)
        .bind(accepted)
        .bind(requested - accepted)
        .bind(stale_after_seconds)
        .bind(task_timeout_seconds)
        .bind(&provider_summary_json)
        .bind(&filters_json)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to persist verify batch: {err}")))?;
    tx.commit().await.map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to commit verify batch: {err}"),
        )
    })?;

    Ok((
        StatusCode::ACCEPTED,
        Json(ProxyVerifyBatchResponse {
            batch_id,
            created_at: now,
            requested,
            accepted,
            skipped: requested - accepted,
            stale_after_seconds,
            task_timeout_seconds,
            provider_summary,
            status: "scheduled".to_string(),
        }),
    ))
}

pub async fn verify_proxy(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxyVerifyResponse>, (StatusCode, String)> {
    Ok(Json(run_proxy_verify_probe(&state, &proxy_id).await?))
}

pub async fn create_behavior_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreateBehaviorProfileRequest>,
) -> Result<(StatusCode, Json<BehaviorProfileResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "behavior profile id and name are required".to_string()));
    }

    let now = now_ts_string();
    let status = normalize_status(payload.status, "active");
    let mouse_json = payload.mouse_json.as_ref().map(Value::to_string);
    let keyboard_json = payload.keyboard_json.as_ref().map(Value::to_string);
    let scroll_json = payload.scroll_json.as_ref().map(Value::to_string);
    let dwell_json = payload.dwell_json.as_ref().map(Value::to_string);
    let navigation_json = payload.navigation_json.as_ref().map(Value::to_string);
    let input_json = payload.input_json.as_ref().map(Value::to_string);
    let action_preference_json = payload.action_preference_json.as_ref().map(Value::to_string);
    let humanize_defaults_json = payload.humanize_defaults_json.as_ref().map(Value::to_string);
    let legacy_profile_json = json!({
        "description": payload.description.as_ref(),
        "mouse_json": payload.mouse_json.as_ref(),
        "keyboard_json": payload.keyboard_json.as_ref(),
        "scroll_json": payload.scroll_json.as_ref(),
        "dwell_json": payload.dwell_json.as_ref(),
        "navigation_json": payload.navigation_json.as_ref(),
        "input_json": payload.input_json.as_ref(),
        "action_preference_json": payload.action_preference_json.as_ref(),
        "humanize_defaults_json": payload.humanize_defaults_json.as_ref(),
        "status": status.clone(),
    })
    .to_string();
    sqlx::query(
        r#"INSERT INTO behavior_profiles (
               id, name, version, status, tags_json, profile_json, description,
               mouse_json, keyboard_json, scroll_json, dwell_json,
               navigation_json, input_json, action_preference_json, humanize_defaults_json,
               created_at, updated_at
           ) VALUES (?, ?, 1, ?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(&status)
    .bind(&legacy_profile_json)
    .bind(&payload.description)
    .bind(&mouse_json)
    .bind(&keyboard_json)
    .bind(&scroll_json)
    .bind(&dwell_json)
    .bind(&navigation_json)
    .bind(&input_json)
    .bind(&action_preference_json)
    .bind(&humanize_defaults_json)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create behavior profile: {err}")))?;

    Ok((
        StatusCode::CREATED,
        Json(BehaviorProfileResponse {
            id: payload.id,
            name: payload.name,
            description: payload.description,
            mouse_json: payload.mouse_json,
            keyboard_json: payload.keyboard_json,
            scroll_json: payload.scroll_json,
            dwell_json: payload.dwell_json,
            navigation_json: payload.navigation_json,
            input_json: payload.input_json,
            action_preference_json: payload.action_preference_json,
            humanize_defaults_json: payload.humanize_defaults_json,
            status,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_behavior_profiles(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<BehaviorProfileResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (
        String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>, String, String, String,
    )>(
        r#"SELECT
               id, name, description, mouse_json, keyboard_json, scroll_json, dwell_json,
               navigation_json, input_json, action_preference_json, humanize_defaults_json,
               status, created_at, updated_at
           FROM behavior_profiles
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list behavior profiles: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, description, mouse_json, keyboard_json, scroll_json, dwell_json, navigation_json, input_json, action_preference_json, humanize_defaults_json, status, created_at, updated_at)| {
                map_behavior_profile_row(
                    id,
                    name,
                    description,
                    mouse_json,
                    keyboard_json,
                    scroll_json,
                    dwell_json,
                    navigation_json,
                    input_json,
                    action_preference_json,
                    humanize_defaults_json,
                    status,
                    created_at,
                    updated_at,
                )
            })
            .collect(),
    ))
}

pub async fn get_behavior_profile(
    State(state): State<AppState>,
    Path(behavior_profile_id): Path<String>,
) -> Result<Json<BehaviorProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (
        String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>, Option<String>, String, String, String,
    )>(
        r#"SELECT
               id, name, description, mouse_json, keyboard_json, scroll_json, dwell_json,
               navigation_json, input_json, action_preference_json, humanize_defaults_json,
               status, created_at, updated_at
           FROM behavior_profiles
           WHERE id = ?"#,
    )
    .bind(&behavior_profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch behavior profile: {err}")))?;

    match row {
        Some((id, name, description, mouse_json, keyboard_json, scroll_json, dwell_json, navigation_json, input_json, action_preference_json, humanize_defaults_json, status, created_at, updated_at)) => Ok(Json(
            map_behavior_profile_row(
                id,
                name,
                description,
                mouse_json,
                keyboard_json,
                scroll_json,
                dwell_json,
                navigation_json,
                input_json,
                action_preference_json,
                humanize_defaults_json,
                status,
                created_at,
                updated_at,
            ),
        )),
        None => Err((StatusCode::NOT_FOUND, format!("behavior profile not found: {behavior_profile_id}"))),
    }
}

pub async fn create_persona_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreatePersonaProfileRequest>,
) -> Result<(StatusCode, Json<PersonaProfileResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty()
        || payload.store_id.trim().is_empty()
        || payload.platform_id.trim().is_empty()
        || payload.device_family.trim().is_empty()
        || payload.country_anchor.trim().is_empty()
        || payload.locale.trim().is_empty()
        || payload.timezone.trim().is_empty()
        || payload.fingerprint_profile_id.trim().is_empty()
        || payload.network_policy_id.trim().is_empty()
        || payload.continuity_policy_id.trim().is_empty()
    {
        return Err((StatusCode::BAD_REQUEST, "persona profile required fields are missing".to_string()));
    }
    if payload.device_family != "desktop" {
        return Err((StatusCode::BAD_REQUEST, "this phase only supports desktop personas".to_string()));
    }

    let now = now_ts_string();
    let status = normalize_status(payload.status, "active");
    sqlx::query(
        r#"INSERT INTO persona_profiles (
               id, store_id, platform_id, device_family, country_anchor, region_anchor, locale, timezone,
               fingerprint_profile_id, behavior_profile_id, network_policy_id, continuity_policy_id, credential_ref, status, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.store_id)
    .bind(&payload.platform_id)
    .bind(&payload.device_family)
    .bind(&payload.country_anchor)
    .bind(&payload.region_anchor)
    .bind(&payload.locale)
    .bind(&payload.timezone)
    .bind(&payload.fingerprint_profile_id)
    .bind(&payload.behavior_profile_id)
    .bind(&payload.network_policy_id)
    .bind(&payload.continuity_policy_id)
    .bind(&payload.credential_ref)
    .bind(&status)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create persona profile: {err}")))?;

    Ok((
        StatusCode::CREATED,
        Json(map_persona_profile_row(
            payload.id,
            payload.store_id,
            payload.platform_id,
            payload.device_family,
            payload.country_anchor,
            payload.region_anchor,
            payload.locale,
            payload.timezone,
            payload.fingerprint_profile_id,
            payload.behavior_profile_id,
            payload.network_policy_id,
            payload.continuity_policy_id,
            payload.credential_ref,
            status,
            now.clone(),
            now,
        )),
    ))
}

pub async fn list_persona_profiles(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<PersonaProfileResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (
        String, String, String, String, String, Option<String>, String, String, String, Option<String>, String, String, Option<String>, String, String, String,
    )>(
        r#"SELECT id, store_id, platform_id, device_family, country_anchor, region_anchor, locale, timezone,
                  fingerprint_profile_id, behavior_profile_id, network_policy_id, continuity_policy_id, credential_ref, status, created_at, updated_at
           FROM persona_profiles
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list persona profiles: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, store_id, platform_id, device_family, country_anchor, region_anchor, locale, timezone, fingerprint_profile_id, behavior_profile_id, network_policy_id, continuity_policy_id, credential_ref, status, created_at, updated_at)| {
                map_persona_profile_row(
                    id,
                    store_id,
                    platform_id,
                    device_family,
                    country_anchor,
                    region_anchor,
                    locale,
                    timezone,
                    fingerprint_profile_id,
                    behavior_profile_id,
                    network_policy_id,
                    continuity_policy_id,
                    credential_ref,
                    status,
                    created_at,
                    updated_at,
                )
            })
            .collect(),
    ))
}

pub async fn get_persona_profile(
    State(state): State<AppState>,
    Path(persona_id): Path<String>,
) -> Result<Json<PersonaProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (
        String, String, String, String, String, Option<String>, String, String, String, Option<String>, String, String, Option<String>, String, String, String,
    )>(
        r#"SELECT id, store_id, platform_id, device_family, country_anchor, region_anchor, locale, timezone,
                  fingerprint_profile_id, behavior_profile_id, network_policy_id, continuity_policy_id, credential_ref, status, created_at, updated_at
           FROM persona_profiles
           WHERE id = ?"#,
    )
    .bind(&persona_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch persona profile: {err}")))?;

    match row {
        Some((id, store_id, platform_id, device_family, country_anchor, region_anchor, locale, timezone, fingerprint_profile_id, behavior_profile_id, network_policy_id, continuity_policy_id, credential_ref, status, created_at, updated_at)) => Ok(Json(
            map_persona_profile_row(
                id,
                store_id,
                platform_id,
                device_family,
                country_anchor,
                region_anchor,
                locale,
                timezone,
                fingerprint_profile_id,
                behavior_profile_id,
                network_policy_id,
                continuity_policy_id,
                credential_ref,
                status,
                created_at,
                updated_at,
            ),
        )),
        None => Err((StatusCode::NOT_FOUND, format!("persona profile not found: {persona_id}"))),
    }
}

pub async fn create_network_policy(
    State(state): State<AppState>,
    Json(payload): Json<CreateNetworkPolicyRequest>,
) -> Result<(StatusCode, Json<NetworkPolicyResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.name.trim().is_empty() || payload.country_anchor.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "network policy required fields are missing".to_string()));
    }

    let now = now_ts_string();
    let status = normalize_status(payload.status, "active");
    sqlx::query(
        r#"INSERT INTO network_policies (
               id, name, country_anchor, region_anchor, allow_same_country_fallback, allow_same_region_fallback, provider_preference,
               allowed_regions_json, network_policy_json, status, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(&payload.country_anchor)
    .bind(&payload.region_anchor)
    .bind(if payload.allow_same_country_fallback { 1_i64 } else { 0_i64 })
    .bind(if payload.allow_same_region_fallback { 1_i64 } else { 0_i64 })
    .bind(&payload.provider_preference)
    .bind(payload.allowed_regions_json.as_ref().map(Value::to_string))
    .bind(payload.network_policy_json.to_string())
    .bind(&status)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create network policy: {err}")))?;

    Ok((
        StatusCode::CREATED,
        Json(NetworkPolicyResponse {
            id: payload.id,
            name: payload.name,
            country_anchor: payload.country_anchor,
            region_anchor: payload.region_anchor,
            allow_same_country_fallback: payload.allow_same_country_fallback,
            allow_same_region_fallback: payload.allow_same_region_fallback,
            provider_preference: payload.provider_preference,
            allowed_regions_json: payload.allowed_regions_json,
            network_policy_json: payload.network_policy_json,
            status,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_network_policies(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<NetworkPolicyResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (
        String, String, String, Option<String>, i64, i64, Option<String>, Option<String>, String, String, String, String,
    )>(
        r#"SELECT id, name, country_anchor, region_anchor, allow_same_country_fallback, allow_same_region_fallback, provider_preference,
                  allowed_regions_json, network_policy_json, status, created_at, updated_at
           FROM network_policies
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list network policies: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, country_anchor, region_anchor, allow_same_country_fallback, allow_same_region_fallback, provider_preference, allowed_regions_json, network_policy_json, status, created_at, updated_at)| {
                map_network_policy_row(
                    id,
                    name,
                    country_anchor,
                    region_anchor,
                    allow_same_country_fallback,
                    allow_same_region_fallback,
                    provider_preference,
                    allowed_regions_json,
                    network_policy_json,
                    status,
                    created_at,
                    updated_at,
                )
            })
            .collect(),
    ))
}

pub async fn get_network_policy(
    State(state): State<AppState>,
    Path(policy_id): Path<String>,
) -> Result<Json<NetworkPolicyResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (
        String, String, String, Option<String>, i64, i64, Option<String>, Option<String>, String, String, String, String,
    )>(
        r#"SELECT id, name, country_anchor, region_anchor, allow_same_country_fallback, allow_same_region_fallback, provider_preference,
                  allowed_regions_json, network_policy_json, status, created_at, updated_at
           FROM network_policies
           WHERE id = ?"#,
    )
    .bind(&policy_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch network policy: {err}")))?;

    match row {
        Some((id, name, country_anchor, region_anchor, allow_same_country_fallback, allow_same_region_fallback, provider_preference, allowed_regions_json, network_policy_json, status, created_at, updated_at)) => Ok(Json(
            map_network_policy_row(
                id,
                name,
                country_anchor,
                region_anchor,
                allow_same_country_fallback,
                allow_same_region_fallback,
                provider_preference,
                allowed_regions_json,
                network_policy_json,
                status,
                created_at,
                updated_at,
            ),
        )),
        None => Err((StatusCode::NOT_FOUND, format!("network policy not found: {policy_id}"))),
    }
}

pub async fn create_continuity_policy(
    State(state): State<AppState>,
    Json(payload): Json<CreateContinuityPolicyRequest>,
) -> Result<(StatusCode, Json<ContinuityPolicyResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.name.trim().is_empty() || payload.session_ttl_seconds <= 0 {
        return Err((StatusCode::BAD_REQUEST, "continuity policy required fields are invalid".to_string()));
    }

    let now = now_ts_string();
    let status = normalize_status(payload.status, "active");
    let site_group_mode = payload.site_group_mode.unwrap_or_else(|| "host".to_string());
    let heartbeat_interval_seconds = payload.heartbeat_interval_seconds.unwrap_or(21600).max(60);
    sqlx::query(
        r#"INSERT INTO continuity_policies (
               id, name, session_ttl_seconds, heartbeat_interval_seconds, site_group_mode, recovery_enabled,
               protect_on_login_loss, policy_json, status, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(payload.session_ttl_seconds)
    .bind(heartbeat_interval_seconds)
    .bind(&site_group_mode)
    .bind(if payload.recovery_enabled { 1_i64 } else { 0_i64 })
    .bind(if payload.protect_on_login_loss { 1_i64 } else { 0_i64 })
    .bind(payload.policy_json.as_ref().map(Value::to_string))
    .bind(&status)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create continuity policy: {err}")))?;

    Ok((
        StatusCode::CREATED,
        Json(ContinuityPolicyResponse {
            id: payload.id,
            name: payload.name,
            session_ttl_seconds: payload.session_ttl_seconds,
            heartbeat_interval_seconds,
            site_group_mode,
            recovery_enabled: payload.recovery_enabled,
            protect_on_login_loss: payload.protect_on_login_loss,
            policy_json: payload.policy_json,
            status,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_continuity_policies(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<ContinuityPolicyResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (
        String, String, i64, i64, String, i64, i64, Option<String>, String, String, String,
    )>(
        r#"SELECT id, name, session_ttl_seconds, heartbeat_interval_seconds, site_group_mode, recovery_enabled,
                  protect_on_login_loss, policy_json, status, created_at, updated_at
           FROM continuity_policies
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list continuity policies: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, name, session_ttl_seconds, heartbeat_interval_seconds, site_group_mode, recovery_enabled, protect_on_login_loss, policy_json, status, created_at, updated_at)| {
                map_continuity_policy_row(
                    id,
                    name,
                    session_ttl_seconds,
                    heartbeat_interval_seconds,
                    site_group_mode,
                    recovery_enabled,
                    protect_on_login_loss,
                    policy_json,
                    status,
                    created_at,
                    updated_at,
                )
            })
            .collect(),
    ))
}

pub async fn get_continuity_policy(
    State(state): State<AppState>,
    Path(policy_id): Path<String>,
) -> Result<Json<ContinuityPolicyResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (
        String, String, i64, i64, String, i64, i64, Option<String>, String, String, String,
    )>(
        r#"SELECT id, name, session_ttl_seconds, heartbeat_interval_seconds, site_group_mode, recovery_enabled,
                  protect_on_login_loss, policy_json, status, created_at, updated_at
           FROM continuity_policies
           WHERE id = ?"#,
    )
    .bind(&policy_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch continuity policy: {err}")))?;

    match row {
        Some((id, name, session_ttl_seconds, heartbeat_interval_seconds, site_group_mode, recovery_enabled, protect_on_login_loss, policy_json, status, created_at, updated_at)) => Ok(Json(
            map_continuity_policy_row(
                id,
                name,
                session_ttl_seconds,
                heartbeat_interval_seconds,
                site_group_mode,
                recovery_enabled,
                protect_on_login_loss,
                policy_json,
                status,
                created_at,
                updated_at,
            ),
        )),
        None => Err((StatusCode::NOT_FOUND, format!("continuity policy not found: {policy_id}"))),
    }
}

pub async fn create_platform_template(
    State(state): State<AppState>,
    Json(payload): Json<CreatePlatformTemplateRequest>,
) -> Result<(StatusCode, Json<PlatformTemplateResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.platform_id.trim().is_empty() || payload.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "platform template id, platform_id and name are required".to_string()));
    }

    let now = now_ts_string();
    let status = normalize_status(payload.status, "active");
    let readiness_level = payload.readiness_level.unwrap_or_else(|| "baseline".to_string());
    sqlx::query(
        r#"INSERT INTO platform_templates (
               id, platform_id, name, warm_paths_json, revisit_paths_json, stateful_paths_json,
               write_operation_paths_json, high_risk_paths_json, allowed_regions_json,
               preferred_locale, preferred_timezone, continuity_checks_json, identity_markers_json,
               login_loss_signals_json, recovery_steps_json, behavior_defaults_json, event_chain_templates_json, page_semantics_json,
               readiness_level, status, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.platform_id)
    .bind(&payload.name)
    .bind(payload.warm_paths_json.to_string())
    .bind(payload.revisit_paths_json.to_string())
    .bind(payload.stateful_paths_json.to_string())
    .bind(payload.write_operation_paths_json.to_string())
    .bind(payload.high_risk_paths_json.to_string())
    .bind(payload.allowed_regions_json.to_string())
    .bind(&payload.preferred_locale)
    .bind(&payload.preferred_timezone)
    .bind(payload.continuity_checks_json.as_ref().map(Value::to_string))
    .bind(payload.identity_markers_json.as_ref().map(Value::to_string))
    .bind(payload.login_loss_signals_json.as_ref().map(Value::to_string))
    .bind(payload.recovery_steps_json.as_ref().map(Value::to_string))
    .bind(payload.behavior_defaults_json.as_ref().map(Value::to_string))
    .bind(payload.event_chain_templates_json.as_ref().map(Value::to_string))
    .bind(payload.page_semantics_json.as_ref().map(Value::to_string))
    .bind(&readiness_level)
    .bind(&status)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create platform template: {err}")))?;

    Ok((
        StatusCode::CREATED,
        Json(PlatformTemplateResponse {
            id: payload.id,
            platform_id: payload.platform_id,
            name: payload.name,
            warm_paths_json: payload.warm_paths_json,
            revisit_paths_json: payload.revisit_paths_json,
            stateful_paths_json: payload.stateful_paths_json,
            write_operation_paths_json: payload.write_operation_paths_json,
            high_risk_paths_json: payload.high_risk_paths_json,
            allowed_regions_json: payload.allowed_regions_json,
            preferred_locale: payload.preferred_locale,
            preferred_timezone: payload.preferred_timezone,
            continuity_checks_json: payload.continuity_checks_json,
            identity_markers_json: payload.identity_markers_json,
            login_loss_signals_json: payload.login_loss_signals_json,
            recovery_steps_json: payload.recovery_steps_json,
            behavior_defaults_json: payload.behavior_defaults_json,
            event_chain_templates_json: payload.event_chain_templates_json,
            page_semantics_json: payload.page_semantics_json,
            readiness_level,
            status,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_platform_templates(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<PlatformTemplateResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, PlatformTemplateRow>(
        r#"SELECT id, platform_id, name, warm_paths_json, revisit_paths_json, stateful_paths_json,
                  write_operation_paths_json, high_risk_paths_json, allowed_regions_json,
                  preferred_locale, preferred_timezone, continuity_checks_json, identity_markers_json,
                  login_loss_signals_json, recovery_steps_json, behavior_defaults_json, event_chain_templates_json, page_semantics_json,
                  readiness_level, status, created_at, updated_at
           FROM platform_templates
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list platform templates: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|row| {
                map_platform_template_row(
                    row.id,
                    row.platform_id,
                    row.name,
                    row.warm_paths_json,
                    row.revisit_paths_json,
                    row.stateful_paths_json,
                    row.write_operation_paths_json,
                    row.high_risk_paths_json,
                    row.allowed_regions_json,
                    row.preferred_locale,
                    row.preferred_timezone,
                    row.continuity_checks_json,
                    row.identity_markers_json,
                    row.login_loss_signals_json,
                    row.recovery_steps_json,
                    row.behavior_defaults_json,
                    row.event_chain_templates_json,
                    row.page_semantics_json,
                    row.readiness_level,
                    row.status,
                    row.created_at,
                    row.updated_at,
                )
            })
            .collect(),
    ))
}

pub async fn get_platform_template(
    State(state): State<AppState>,
    Path(template_id): Path<String>,
) -> Result<Json<PlatformTemplateResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, PlatformTemplateRow>(
        r#"SELECT id, platform_id, name, warm_paths_json, revisit_paths_json, stateful_paths_json,
                  write_operation_paths_json, high_risk_paths_json, allowed_regions_json,
                  preferred_locale, preferred_timezone, continuity_checks_json, identity_markers_json,
                  login_loss_signals_json, recovery_steps_json, behavior_defaults_json, event_chain_templates_json, page_semantics_json,
                  readiness_level, status, created_at, updated_at
           FROM platform_templates
           WHERE id = ?"#,
    )
    .bind(&template_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch platform template: {err}")))?;

    match row {
        Some(row) => Ok(Json(
            map_platform_template_row(
                row.id,
                row.platform_id,
                row.name,
                row.warm_paths_json,
                row.revisit_paths_json,
                row.stateful_paths_json,
                row.write_operation_paths_json,
                row.high_risk_paths_json,
                row.allowed_regions_json,
                row.preferred_locale,
                row.preferred_timezone,
                row.continuity_checks_json,
                row.identity_markers_json,
                row.login_loss_signals_json,
                row.recovery_steps_json,
                row.behavior_defaults_json,
                row.event_chain_templates_json,
                row.page_semantics_json,
                row.readiness_level,
                row.status,
                row.created_at,
                row.updated_at,
            ),
        )),
        None => Err((StatusCode::NOT_FOUND, format!("platform template not found: {template_id}"))),
    }
}

pub async fn create_store_platform_override(
    State(state): State<AppState>,
    Json(payload): Json<CreateStorePlatformOverrideRequest>,
) -> Result<(StatusCode, Json<StorePlatformOverrideResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.store_id.trim().is_empty() || payload.platform_id.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "store platform override required fields are missing".to_string()));
    }

    let now = now_ts_string();
    let status = normalize_status(payload.status, "active");
    sqlx::query(
        r#"INSERT INTO store_platform_overrides (
               id, store_id, platform_id, admin_origin, entry_origin, entry_paths_json, warm_paths_json, revisit_paths_json, stateful_paths_json,
               high_risk_paths_json, recovery_steps_json, login_loss_signals_json, identity_markers_json,
               behavior_defaults_json, event_chain_templates_json, page_semantics_json,
               status, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.store_id)
    .bind(&payload.platform_id)
    .bind(&payload.admin_origin)
    .bind(&payload.entry_origin)
    .bind(payload.entry_paths_json.as_ref().map(Value::to_string))
    .bind(payload.warm_paths_json.as_ref().map(Value::to_string))
    .bind(payload.revisit_paths_json.as_ref().map(Value::to_string))
    .bind(payload.stateful_paths_json.as_ref().map(Value::to_string))
    .bind(payload.high_risk_paths_json.as_ref().map(Value::to_string))
    .bind(payload.recovery_steps_json.as_ref().map(Value::to_string))
    .bind(payload.login_loss_signals_json.as_ref().map(Value::to_string))
    .bind(payload.identity_markers_json.as_ref().map(Value::to_string))
    .bind(payload.behavior_defaults_json.as_ref().map(Value::to_string))
    .bind(payload.event_chain_templates_json.as_ref().map(Value::to_string))
    .bind(payload.page_semantics_json.as_ref().map(Value::to_string))
    .bind(&status)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create store platform override: {err}")))?;

    Ok((
        StatusCode::CREATED,
        Json(StorePlatformOverrideResponse {
            id: payload.id,
            store_id: payload.store_id,
            platform_id: payload.platform_id,
            admin_origin: payload.admin_origin,
            entry_origin: payload.entry_origin,
            entry_paths_json: payload.entry_paths_json,
            warm_paths_json: payload.warm_paths_json,
            revisit_paths_json: payload.revisit_paths_json,
            stateful_paths_json: payload.stateful_paths_json,
            high_risk_paths_json: payload.high_risk_paths_json,
            recovery_steps_json: payload.recovery_steps_json,
            login_loss_signals_json: payload.login_loss_signals_json,
            identity_markers_json: payload.identity_markers_json,
            behavior_defaults_json: payload.behavior_defaults_json,
            event_chain_templates_json: payload.event_chain_templates_json,
            page_semantics_json: payload.page_semantics_json,
            status,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_store_platform_overrides(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<StorePlatformOverrideResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, StorePlatformOverrideRow>(
        r#"SELECT id, store_id, platform_id, admin_origin, entry_origin, entry_paths_json, warm_paths_json, revisit_paths_json, stateful_paths_json,
                  high_risk_paths_json, recovery_steps_json, login_loss_signals_json, identity_markers_json,
                  behavior_defaults_json, event_chain_templates_json, page_semantics_json,
                  status, created_at, updated_at
           FROM store_platform_overrides
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list store platform overrides: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|row| {
                map_store_platform_override_row(
                    row.id,
                    row.store_id,
                    row.platform_id,
                    row.admin_origin,
                    row.entry_origin,
                    row.entry_paths_json,
                    row.warm_paths_json,
                    row.revisit_paths_json,
                    row.stateful_paths_json,
                    row.high_risk_paths_json,
                    row.recovery_steps_json,
                    row.login_loss_signals_json,
                    row.identity_markers_json,
                    row.behavior_defaults_json,
                    row.event_chain_templates_json,
                    row.page_semantics_json,
                    row.status,
                    row.created_at,
                    row.updated_at,
                )
            })
            .collect(),
    ))
}

pub async fn get_store_platform_override(
    State(state): State<AppState>,
    Path(override_id): Path<String>,
) -> Result<Json<StorePlatformOverrideResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, StorePlatformOverrideRow>(
        r#"SELECT id, store_id, platform_id, admin_origin, entry_origin, entry_paths_json, warm_paths_json, revisit_paths_json, stateful_paths_json,
                  high_risk_paths_json, recovery_steps_json, login_loss_signals_json, identity_markers_json,
                  behavior_defaults_json, event_chain_templates_json, page_semantics_json,
                  status, created_at, updated_at
           FROM store_platform_overrides
           WHERE id = ?"#,
    )
    .bind(&override_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch store platform override: {err}")))?;

    match row {
        Some(row) => Ok(Json(
            map_store_platform_override_row(
                row.id,
                row.store_id,
                row.platform_id,
                row.admin_origin,
                row.entry_origin,
                row.entry_paths_json,
                row.warm_paths_json,
                row.revisit_paths_json,
                row.stateful_paths_json,
                row.high_risk_paths_json,
                row.recovery_steps_json,
                row.login_loss_signals_json,
                row.identity_markers_json,
                row.behavior_defaults_json,
                row.event_chain_templates_json,
                row.page_semantics_json,
                row.status,
                row.created_at,
                row.updated_at,
            ),
        )),
        None => Err((StatusCode::NOT_FOUND, format!("store platform override not found: {override_id}"))),
    }
}

pub async fn list_manual_gates(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<ManualGateResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (
        String, String, Option<String>, Option<String>, Option<String>, String, Option<String>, String,
        String, String, Option<String>, String, String, Option<String>,
    )>(
        r#"SELECT id, task_id, persona_id, store_id, platform_id, requested_action_kind, requested_url,
                  reason_code, reason_summary, status, resolution_note, created_at, updated_at, resolved_at
           FROM manual_gate_requests
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list manual gates: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, task_id, persona_id, store_id, platform_id, requested_action_kind, requested_url, reason_code, reason_summary, status, resolution_note, created_at, updated_at, resolved_at)| {
                map_manual_gate_row(
                    id,
                    task_id,
                    persona_id,
                    store_id,
                    platform_id,
                    requested_action_kind,
                    requested_url,
                    reason_code,
                    reason_summary,
                    status,
                    resolution_note,
                    created_at,
                    updated_at,
                    resolved_at,
                )
            })
            .collect(),
    ))
}

pub async fn get_manual_gate(
    State(state): State<AppState>,
    Path(gate_id): Path<String>,
) -> Result<Json<ManualGateResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (
        String, String, Option<String>, Option<String>, Option<String>, String, Option<String>, String,
        String, String, Option<String>, String, String, Option<String>,
    )>(
        r#"SELECT id, task_id, persona_id, store_id, platform_id, requested_action_kind, requested_url,
                  reason_code, reason_summary, status, resolution_note, created_at, updated_at, resolved_at
           FROM manual_gate_requests
           WHERE id = ?"#,
    )
    .bind(&gate_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch manual gate: {err}")))?;

    match row {
        Some((id, task_id, persona_id, store_id, platform_id, requested_action_kind, requested_url, reason_code, reason_summary, status, resolution_note, created_at, updated_at, resolved_at)) => Ok(Json(
            map_manual_gate_row(
                id,
                task_id,
                persona_id,
                store_id,
                platform_id,
                requested_action_kind,
                requested_url,
                reason_code,
                reason_summary,
                status,
                resolution_note,
                created_at,
                updated_at,
                resolved_at,
            ),
        )),
        None => Err((StatusCode::NOT_FOUND, format!("manual gate not found: {gate_id}"))),
    }
}

pub async fn confirm_manual_gate(
    State(state): State<AppState>,
    Path(gate_id): Path<String>,
    Json(payload): Json<ManualGateActionRequest>,
) -> Result<Json<ManualGateResponse>, (StatusCode, String)> {
    let gate_row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, String)>(
        r#"SELECT task_id, persona_id, store_id, platform_id, status FROM manual_gate_requests WHERE id = ?"#,
    )
    .bind(&gate_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load manual gate state: {err}")))?;
    let Some((task_id, persona_id, store_id, platform_id, gate_status)) = gate_row else {
        return Err((StatusCode::NOT_FOUND, format!("manual gate not found: {gate_id}")));
    };
    if gate_status == "confirmed" {
        return get_manual_gate(State(state), Path(gate_id)).await;
    }
    if gate_status != "pending" {
        return Err((StatusCode::CONFLICT, format!("manual gate cannot be confirmed from status: {gate_status}")));
    }

    let now = now_ts_string();
    sqlx::query(
        r#"UPDATE manual_gate_requests
           SET status = 'confirmed', resolution_note = ?, updated_at = ?, resolved_at = ?
           WHERE id = ? AND status = 'pending'"#,
    )
    .bind(&payload.note)
    .bind(&now)
    .bind(&now)
    .bind(&gate_id)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to confirm manual gate: {err}")))?;

    sqlx::query(
        r#"UPDATE tasks
           SET status = ?, queued_at = ?, finished_at = NULL, error_message = NULL
           WHERE id = ? AND status = ?"#,
    )
    .bind(TASK_STATUS_QUEUED)
    .bind(&now)
    .bind(&task_id)
    .bind(TASK_STATUS_PENDING)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to re-queue confirmed manual gate task: {err}")))?;

    insert_task_log(
        &state,
        &task_id,
        None,
        "info",
        "manual gate confirmed; task queued for execution",
    )
    .await?;

    if let (Some(persona_id), Some(store_id), Some(platform_id)) = (persona_id.as_deref(), store_id.as_deref(), platform_id.as_deref()) {
        append_continuity_event(
            &state,
            Some(persona_id),
            Some(store_id),
            Some(platform_id),
            Some(&task_id),
            None,
            "manual_gate_confirmed",
            "info",
            Some(&json!({
                "manual_gate_request_id": gate_id.clone(),
                "note": payload.note,
            })),
        )
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to record manual_gate_confirmed event: {err}")))?;
    }

    get_manual_gate(State(state), Path(gate_id)).await
}

pub async fn reject_manual_gate(
    State(state): State<AppState>,
    Path(gate_id): Path<String>,
    Json(payload): Json<ManualGateActionRequest>,
) -> Result<Json<ManualGateResponse>, (StatusCode, String)> {
    let gate_row = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>, String)>(
        r#"SELECT task_id, persona_id, store_id, platform_id, status FROM manual_gate_requests WHERE id = ?"#,
    )
    .bind(&gate_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load manual gate state: {err}")))?;
    let Some((task_id, persona_id, store_id, platform_id, gate_status)) = gate_row else {
        return Err((StatusCode::NOT_FOUND, format!("manual gate not found: {gate_id}")));
    };
    if gate_status == "rejected" {
        return get_manual_gate(State(state), Path(gate_id)).await;
    }
    if gate_status != "pending" {
        return Err((StatusCode::CONFLICT, format!("manual gate cannot be rejected from status: {gate_status}")));
    }

    let now = now_ts_string();
    let rejection_message = payload.note.clone().unwrap_or_else(|| "manual gate rejected".to_string());
    let result_json = json!({
        "status": "cancelled",
        "error_kind": "manual_gate_rejected",
        "failure_scope": "manual_gate",
        "message": rejection_message,
        "manual_gate_request_id": gate_id.clone(),
    })
    .to_string();
    sqlx::query(
        r#"UPDATE manual_gate_requests
           SET status = 'rejected', resolution_note = ?, updated_at = ?, resolved_at = ?
           WHERE id = ? AND status = 'pending'"#,
    )
    .bind(&payload.note)
    .bind(&now)
    .bind(&now)
    .bind(&gate_id)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to reject manual gate: {err}")))?;

    sqlx::query(
        r#"UPDATE tasks
           SET status = ?, queued_at = NULL, finished_at = ?, result_json = COALESCE(result_json, ?), error_message = ?
           WHERE id = ? AND status = ?"#,
    )
    .bind(TASK_STATUS_CANCELLED)
    .bind(&now)
    .bind(&result_json)
    .bind(payload.note.clone().unwrap_or_else(|| "manual gate rejected".to_string()))
    .bind(&task_id)
    .bind(TASK_STATUS_PENDING)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to cancel rejected manual gate task: {err}")))?;

    insert_task_log(
        &state,
        &task_id,
        None,
        "warn",
        "manual gate rejected; task cancelled",
    )
    .await?;

    if let (Some(persona_id), Some(store_id), Some(platform_id)) = (persona_id.as_deref(), store_id.as_deref(), platform_id.as_deref()) {
        append_continuity_event(
            &state,
            Some(persona_id),
            Some(store_id),
            Some(platform_id),
            Some(&task_id),
            None,
            "manual_gate_rejected",
            "warning",
            Some(&json!({
                "manual_gate_request_id": gate_id.clone(),
                "note": payload.note,
            })),
        )
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to record manual_gate_rejected event: {err}")))?;
    }

    get_manual_gate(State(state), Path(gate_id)).await
}

pub async fn list_continuity_events(
    State(state): State<AppState>,
    Query(query): Query<ContinuityEventListQuery>,
) -> Result<Json<Vec<ContinuityEventResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let mut sql = String::from(
        "SELECT id, persona_id, store_id, platform_id, task_id, run_id, event_type, severity, event_json, created_at FROM continuity_events WHERE 1 = 1",
    );
    if query.persona_id.is_some() {
        sql.push_str(" AND persona_id = ?");
    }
    if query.platform_id.is_some() {
        sql.push_str(" AND platform_id = ?");
    }
    if query.store_id.is_some() {
        sql.push_str(" AND store_id = ?");
    }
    if query.event_type.is_some() {
        sql.push_str(" AND event_type = ?");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?");

    let mut stmt = sqlx::query_as::<_, (
        String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>,
        String, String, Option<String>, String,
    )>(&sql);
    if let Some(persona_id) = query.persona_id.as_deref() {
        stmt = stmt.bind(persona_id);
    }
    if let Some(platform_id) = query.platform_id.as_deref() {
        stmt = stmt.bind(platform_id);
    }
    if let Some(store_id) = query.store_id.as_deref() {
        stmt = stmt.bind(store_id);
    }
    if let Some(event_type) = query.event_type.as_deref() {
        stmt = stmt.bind(event_type);
    }
    let rows = stmt
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list continuity events: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, persona_id, store_id, platform_id, task_id, run_id, event_type, severity, event_json, created_at)| {
                map_continuity_event_row(
                    id,
                    persona_id,
                    store_id,
                    platform_id,
                    task_id,
                    run_id,
                    event_type,
                    severity,
                    event_json,
                    created_at,
                )
            })
            .collect(),
    ))
}

pub async fn list_persona_health_snapshots(
    State(state): State<AppState>,
    Query(query): Query<ContinuityEventListQuery>,
) -> Result<Json<Vec<PersonaHealthSnapshotResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let mut sql = String::from(
        "SELECT id, persona_id, store_id, platform_id, status, active_session_count, continuity_score, login_risk_count, last_event_type, last_task_at, snapshot_json, created_at FROM persona_health_snapshots WHERE 1 = 1",
    );
    if query.persona_id.is_some() {
        sql.push_str(" AND persona_id = ?");
    }
    if query.platform_id.is_some() {
        sql.push_str(" AND platform_id = ?");
    }
    if query.store_id.is_some() {
        sql.push_str(" AND store_id = ?");
    }
    sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?");

    let mut stmt = sqlx::query_as::<_, (
        String, String, String, String, String, i64, f64, i64, Option<String>, Option<String>, Option<String>, String,
    )>(&sql);
    if let Some(persona_id) = query.persona_id.as_deref() {
        stmt = stmt.bind(persona_id);
    }
    if let Some(platform_id) = query.platform_id.as_deref() {
        stmt = stmt.bind(platform_id);
    }
    if let Some(store_id) = query.store_id.as_deref() {
        stmt = stmt.bind(store_id);
    }
    let rows = stmt
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list persona health snapshots: {err}")))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, persona_id, store_id, platform_id, status, active_session_count, continuity_score, login_risk_count, last_event_type, last_task_at, snapshot_json, created_at)| {
                map_persona_health_snapshot_row(
                    id,
                    persona_id,
                    store_id,
                    platform_id,
                    status,
                    active_session_count,
                    continuity_score,
                    login_risk_count,
                    last_event_type,
                    last_task_at,
                    snapshot_json,
                    created_at,
                )
            })
            .collect(),
    ))
}
