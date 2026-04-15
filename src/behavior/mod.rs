pub mod form;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::Row;

use crate::{
    db::init::DbPool,
    runner::{RunnerBehaviorPlan, RunnerBehaviorProfile, RunnerExecutionIntent},
};

pub const RESOURCE_STATUS_DRAFT: &str = "draft";
pub const RESOURCE_STATUS_ACTIVE: &str = "active";
pub const RESOURCE_STATUS_DISABLED: &str = "disabled";

pub const BEHAVIOR_MODE_DISABLED: &str = "disabled";
pub const BEHAVIOR_MODE_SHADOW: &str = "shadow";
pub const BEHAVIOR_MODE_ACTIVE: &str = "active";
pub const BEHAVIOR_MODE_PROFILE_REQUIRED: &str = "profile_required";

pub const SYSTEM_DEFAULT_BEHAVIOR_PROFILE_ID: &str = "system-default-browser-v1";
pub const SYSTEM_DEFAULT_BEHAVIOR_PROFILE_VERSION: i64 = 1;

const DEFAULT_BROWSER_BUDGET_MAX_ADDED_LATENCY_MS: i64 = 12_000;
const DEFAULT_BROWSER_BUDGET_TIMEOUT_RESERVE_MS: i64 = 3_000;
const DEFAULT_BROWSER_BUDGET_MAX_STEP_COUNT: i64 = 24;

const PAGE_ARCHETYPES: &[&str] = &[
    "article",
    "search_results",
    "listing",
    "product",
    "form",
    "auth",
    "dashboard",
    "generic",
];

const SUPPORTED_PRIMITIVES: &[&str] = &[
    "idle",
    "wait_for_readiness",
    "wait_for_content_stable",
    "scroll_progressive",
    "scroll_to_ratio",
    "pause_on_content",
    "focus_element",
    "blur_element",
    "hover_candidate",
    "type_with_rhythm",
    "clear_with_corrections",
    "persist_session_state",
    "soft_abort_if_budget_exceeded",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorBudget {
    pub max_added_latency_ms: i64,
    pub timeout_reserve_ms: i64,
    pub max_step_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorRuntimeExplain {
    pub requested_behavior_profile_id: Option<String>,
    pub resolved_behavior_profile_id: Option<String>,
    pub resolved_version: Option<i64>,
    pub resolution_source: String,
    pub page_archetype: Option<String>,
    pub capability_status: String,
    pub applied_primitives: Vec<String>,
    pub ignored_primitives: Vec<String>,
    pub skipped_steps: Vec<String>,
    pub seed: Option<String>,
    pub budget: Option<BehaviorBudget>,
    pub total_added_latency_ms: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorTraceSummary {
    pub planned_steps: i64,
    pub executed_steps: i64,
    pub failed_steps: i64,
    pub aborted: bool,
    pub abort_reason: Option<String>,
    pub session_persisted: bool,
    pub raw_trace_persisted: bool,
    pub total_added_latency_ms: i64,
}

#[derive(Debug, Clone)]
pub struct BehaviorCompileResult {
    pub execution_intent: RunnerExecutionIntent,
    pub execution_intent_json: Value,
    pub behavior_policy_json: Value,
    pub behavior_profile_id: Option<String>,
    pub behavior_profile_version: Option<i64>,
    pub behavior_resolution_status: String,
    pub behavior_execution_mode: String,
    pub page_archetype: Option<String>,
    pub behavior_seed: Option<String>,
    pub behavior_profile: Option<RunnerBehaviorProfile>,
    pub behavior_plan: Option<RunnerBehaviorPlan>,
    pub behavior_runtime_explain: BehaviorRuntimeExplain,
    pub behavior_trace_summary: BehaviorTraceSummary,
    pub plan_artifact_metadata_json: Option<Value>,
    pub site_key: Option<String>,
}

#[derive(Debug, Clone)]
struct SiteBehaviorPolicyRecord {
    behavior_profile_id: String,
    override_json: Option<Value>,
}

#[derive(Debug, Clone)]
struct ResolvedBehaviorProfile {
    profile: RunnerBehaviorProfile,
    source: String,
}

pub fn is_browser_task_kind(kind: &str) -> bool {
    matches!(
        kind,
        "open_page" | "get_html" | "get_title" | "get_final_url" | "extract_text"
    )
}

pub fn normalize_resource_status(status: Option<&str>) -> Result<String> {
    match status.unwrap_or(RESOURCE_STATUS_DRAFT) {
        RESOURCE_STATUS_DRAFT | RESOURCE_STATUS_ACTIVE | RESOURCE_STATUS_DISABLED => {
            Ok(status.unwrap_or(RESOURCE_STATUS_DRAFT).to_string())
        }
        other => Err(anyhow!(
            "status must be one of draft|active|disabled, got {other}"
        )),
    }
}

pub fn site_key_from_url(url: Option<&str>) -> Option<String> {
    let url = url?;
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_ascii_lowercase()))
}

pub fn infer_page_archetype(task_kind: &str, url: Option<&str>) -> String {
    let lower_url = url.unwrap_or_default().to_ascii_lowercase();
    let parsed_path = Url::parse(url.unwrap_or_default())
        .ok()
        .map(|parsed| parsed.path().to_ascii_lowercase())
        .unwrap_or_default();

    if lower_url.contains("/login")
        || lower_url.contains("/signin")
        || lower_url.contains("/sign-in")
        || lower_url.contains("/auth")
    {
        return "auth".to_string();
    }

    if lower_url.contains("/dashboard")
        || lower_url.contains("/console")
        || lower_url.contains("/admin")
        || lower_url.contains("/workspace")
    {
        return "dashboard".to_string();
    }

    if lower_url.contains("/form")
        || lower_url.contains("/register")
        || lower_url.contains("/signup")
        || lower_url.contains("/checkout")
        || lower_url.contains("/contact")
    {
        return "form".to_string();
    }

    if lower_url.contains("/search")
        || lower_url.contains("?q=")
        || lower_url.contains("&q=")
        || lower_url.contains("query=")
    {
        return "search_results".to_string();
    }

    if lower_url.contains("/product") || lower_url.contains("/item") || lower_url.contains("/sku") {
        return "product".to_string();
    }

    if lower_url.contains("/listing")
        || lower_url.contains("/list")
        || lower_url.contains("/category")
        || lower_url.contains("/catalog")
    {
        return "listing".to_string();
    }

    if matches!(task_kind, "extract_text" | "get_html")
        || lower_url.contains("/article")
        || lower_url.contains("/blog")
        || lower_url.contains("/post")
        || lower_url.contains("/news")
        || parsed_path.ends_with(".html")
    {
        return "article".to_string();
    }

    "generic".to_string()
}

pub async fn compile_behavior_plan(
    db: &DbPool,
    task_kind: &str,
    url: Option<&str>,
    timeout_seconds: Option<i64>,
    mut execution_intent: RunnerExecutionIntent,
    behavior_policy_json: Option<Value>,
) -> Result<BehaviorCompileResult> {
    let policy_object = parse_optional_object(behavior_policy_json)?;

    let requested_behavior_profile_id = policy_object
        .get("behavior_profile_id")
        .and_then(Value::as_str)
        .map(|value| value.to_string())
        .or_else(|| execution_intent.behavior_profile_id.clone());

    let identity_defaults =
        load_identity_defaults(db, execution_intent.identity_profile_id.as_deref()).await?;
    if execution_intent.fingerprint_profile_id.is_none() {
        execution_intent.fingerprint_profile_id = identity_defaults.0.clone();
    }
    if execution_intent.behavior_profile_id.is_none() {
        execution_intent.behavior_profile_id = identity_defaults.1.clone();
    }
    if execution_intent.network_profile_id.is_none() {
        execution_intent.network_profile_id = identity_defaults.2.clone();
    }

    validate_session_profile(db, execution_intent.session_profile_id.as_deref()).await?;

    let site_key = site_key_from_url(url);
    let allow_site_overrides = policy_object
        .get("allow_site_overrides")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let requested_archetype =
        normalize_page_archetype(policy_object.get("page_archetype").and_then(Value::as_str))?;
    let inferred_archetype =
        requested_archetype.unwrap_or_else(|| infer_page_archetype(task_kind, url));

    let site_policy = if allow_site_overrides {
        load_site_behavior_policy(
            db,
            site_key.as_deref(),
            Some(inferred_archetype.as_str()),
            Some(task_kind),
        )
        .await?
    } else {
        None
    };
    let site_override = site_policy
        .as_ref()
        .and_then(|item| item.override_json.as_ref())
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();

    let resolved_behavior_profile = resolve_behavior_profile(
        db,
        requested_behavior_profile_id.as_deref(),
        execution_intent.behavior_profile_id.as_deref(),
        site_policy
            .as_ref()
            .map(|item| item.behavior_profile_id.as_str()),
        identity_defaults.1.as_deref(),
    )
    .await?;

    let requested_mode = normalize_behavior_mode(
        policy_object.get("mode").and_then(Value::as_str),
        site_override.get("mode").and_then(Value::as_str),
        task_kind,
    )?;

    let mut warnings = Vec::new();
    let actual_mode = materialize_behavior_mode(
        task_kind,
        requested_mode.as_str(),
        resolved_behavior_profile
            .as_ref()
            .map(|item| item.source.as_str()),
        &mut warnings,
    );

    let page_archetype = normalize_page_archetype(
        policy_object
            .get("page_archetype")
            .and_then(Value::as_str)
            .or_else(|| site_override.get("page_archetype").and_then(Value::as_str)),
    )?
    .unwrap_or(inferred_archetype);

    let budget = merge_budget(
        task_kind,
        timeout_seconds,
        policy_object.get("budget"),
        site_override.get("budget"),
        &mut warnings,
    );

    let allowed_primitives = resolve_allowed_primitives(
        page_archetype.as_str(),
        resolved_behavior_profile
            .as_ref()
            .map(|item| &item.profile.profile_json),
        policy_object.get("allowed_primitives"),
        site_override.get("allowed_primitives"),
        &mut warnings,
    );

    let seed = resolve_seed(
        task_kind,
        url,
        &page_archetype,
        resolved_behavior_profile
            .as_ref()
            .map(|item| item.profile.id.as_str()),
        policy_object.get("plan_seed").and_then(Value::as_str),
        site_override.get("plan_seed").and_then(Value::as_str),
    );

    let (behavior_plan, skipped_steps) = build_behavior_plan(
        actual_mode.as_str(),
        page_archetype.as_str(),
        &seed,
        &budget,
        &allowed_primitives,
    );
    warnings.extend(
        skipped_steps
            .iter()
            .map(|item| format!("plan step skipped: {item}")),
    );

    let behavior_profile_id = resolved_behavior_profile
        .as_ref()
        .map(|item| item.profile.id.clone());
    let behavior_profile_version = resolved_behavior_profile
        .as_ref()
        .map(|item| item.profile.version);
    let resolution_source = resolved_behavior_profile
        .as_ref()
        .map(|item| item.source.clone())
        .unwrap_or_else(|| "disabled".to_string());
    let estimated_added_latency_ms = if actual_mode == BEHAVIOR_MODE_ACTIVE {
        estimate_added_latency_ms(behavior_plan.as_ref())
    } else {
        0
    };
    let trace_summary = BehaviorTraceSummary {
        planned_steps: behavior_plan.as_ref().map(step_count).unwrap_or_default(),
        executed_steps: 0,
        failed_steps: 0,
        aborted: false,
        abort_reason: None,
        session_persisted: allowed_primitives
            .iter()
            .any(|item| item == "persist_session_state"),
        raw_trace_persisted: false,
        total_added_latency_ms: estimated_added_latency_ms,
    };
    let runtime_explain = BehaviorRuntimeExplain {
        requested_behavior_profile_id,
        resolved_behavior_profile_id: behavior_profile_id.clone(),
        resolved_version: behavior_profile_version,
        resolution_source: resolution_source.clone(),
        page_archetype: Some(page_archetype.clone()),
        capability_status: match actual_mode.as_str() {
            BEHAVIOR_MODE_ACTIVE => "compiled_active".to_string(),
            BEHAVIOR_MODE_SHADOW => "compiled_shadow".to_string(),
            _ => "disabled".to_string(),
        },
        applied_primitives: allowed_primitives.clone(),
        ignored_primitives: collect_ignored_primitives(
            policy_object.get("allowed_primitives"),
            site_override.get("allowed_primitives"),
        ),
        skipped_steps,
        seed: Some(seed.clone()),
        budget: Some(budget.clone()),
        total_added_latency_ms: estimated_added_latency_ms,
        warnings: warnings.clone(),
    };
    let normalized_policy_json = json!({
        "mode": actual_mode,
        "page_archetype": page_archetype,
        "allow_site_overrides": allow_site_overrides,
        "budget": budget,
        "plan_seed": seed,
        "allowed_primitives": allowed_primitives,
    });

    Ok(BehaviorCompileResult {
        execution_intent_json: serde_json::to_value(&execution_intent)
            .unwrap_or_else(|_| json!({})),
        plan_artifact_metadata_json: behavior_plan.as_ref().map(|plan| {
            json!({
                "behavior_plan": plan,
                "behavior_profile": resolved_behavior_profile.as_ref().map(|item| &item.profile),
                "behavior_runtime_explain": runtime_explain,
                "behavior_trace_summary": trace_summary,
                "site_key": site_key,
            })
        }),
        behavior_resolution_status: if actual_mode == BEHAVIOR_MODE_DISABLED {
            "disabled".to_string()
        } else {
            "resolved".to_string()
        },
        behavior_execution_mode: actual_mode,
        page_archetype: Some(page_archetype),
        behavior_seed: Some(seed),
        behavior_profile: resolved_behavior_profile
            .as_ref()
            .map(|item| item.profile.clone()),
        behavior_profile_id,
        behavior_profile_version,
        behavior_plan,
        behavior_runtime_explain: runtime_explain,
        behavior_trace_summary: trace_summary,
        behavior_policy_json: normalized_policy_json,
        execution_intent,
        site_key,
    })
}

pub fn should_store_raw_trace(
    behavior_execution_mode: Option<&str>,
    trace_summary: Option<&BehaviorTraceSummary>,
    task_status: &str,
    sample_seed: &str,
) -> bool {
    if !matches!(
        behavior_execution_mode,
        Some(BEHAVIOR_MODE_SHADOW | BEHAVIOR_MODE_ACTIVE)
    ) {
        return false;
    }

    if matches!(task_status, "failed" | "timed_out" | "cancelled") {
        return true;
    }

    if trace_summary
        .map(|summary| summary.aborted)
        .unwrap_or(false)
    {
        return true;
    }

    deterministic_bucket(sample_seed) == 0
}

fn parse_optional_object(value: Option<Value>) -> Result<Map<String, Value>> {
    match value {
        Some(Value::Object(obj)) => Ok(obj),
        Some(Value::Null) | None => Ok(Map::new()),
        Some(_) => Err(anyhow!("behavior_policy_json must be an object")),
    }
}

fn normalize_behavior_mode(
    requested_mode: Option<&str>,
    site_mode: Option<&str>,
    task_kind: &str,
) -> Result<String> {
    let fallback = if is_browser_task_kind(task_kind) {
        BEHAVIOR_MODE_SHADOW
    } else {
        BEHAVIOR_MODE_DISABLED
    };
    let mode = requested_mode.or(site_mode).unwrap_or(fallback);
    match mode {
        BEHAVIOR_MODE_DISABLED
        | BEHAVIOR_MODE_SHADOW
        | BEHAVIOR_MODE_ACTIVE
        | BEHAVIOR_MODE_PROFILE_REQUIRED => Ok(mode.to_string()),
        other => Err(anyhow!(
            "behavior mode must be one of shadow|active|disabled|profile_required, got {other}"
        )),
    }
}

fn materialize_behavior_mode(
    task_kind: &str,
    requested_mode: &str,
    resolution_source: Option<&str>,
    warnings: &mut Vec<String>,
) -> String {
    if !is_browser_task_kind(task_kind) {
        return BEHAVIOR_MODE_DISABLED.to_string();
    }

    match requested_mode {
        BEHAVIOR_MODE_PROFILE_REQUIRED => {
            if matches!(resolution_source, Some("system_default") | None) {
                warnings.push(
                    "profile_required requested without non-default behavior profile; mode downgraded to disabled"
                        .to_string(),
                );
                BEHAVIOR_MODE_DISABLED.to_string()
            } else {
                BEHAVIOR_MODE_SHADOW.to_string()
            }
        }
        other => other.to_string(),
    }
}

fn normalize_page_archetype(archetype: Option<&str>) -> Result<Option<String>> {
    match archetype {
        Some(value) if PAGE_ARCHETYPES.contains(&value) => Ok(Some(value.to_string())),
        Some(value) => Err(anyhow!(
            "page_archetype must be one of article|search_results|listing|product|form|auth|dashboard|generic, got {value}"
        )),
        None => Ok(None),
    }
}

fn merge_budget(
    task_kind: &str,
    timeout_seconds: Option<i64>,
    task_budget: Option<&Value>,
    site_budget: Option<&Value>,
    warnings: &mut Vec<String>,
) -> BehaviorBudget {
    let mut budget = if is_browser_task_kind(task_kind) {
        BehaviorBudget {
            max_added_latency_ms: DEFAULT_BROWSER_BUDGET_MAX_ADDED_LATENCY_MS,
            timeout_reserve_ms: DEFAULT_BROWSER_BUDGET_TIMEOUT_RESERVE_MS,
            max_step_count: DEFAULT_BROWSER_BUDGET_MAX_STEP_COUNT,
        }
    } else {
        BehaviorBudget {
            max_added_latency_ms: 0,
            timeout_reserve_ms: 0,
            max_step_count: 0,
        }
    };

    apply_budget_override(&mut budget, site_budget);
    apply_budget_override(&mut budget, task_budget);

    if let Some(timeout_ms) = timeout_seconds
        .and_then(|value| value.checked_mul(1_000))
        .filter(|value| *value > 0)
    {
        let available = timeout_ms.saturating_sub(budget.timeout_reserve_ms);
        if budget.max_added_latency_ms > available {
            warnings.push(format!(
                "behavior max_added_latency_ms clipped from {} to {} to respect timeout budget",
                budget.max_added_latency_ms, available
            ));
            budget.max_added_latency_ms = available.max(0);
        }
    }

    budget.max_added_latency_ms = budget.max_added_latency_ms.max(0);
    budget.timeout_reserve_ms = budget.timeout_reserve_ms.max(0);
    budget.max_step_count = budget.max_step_count.max(0);
    budget
}

fn apply_budget_override(target: &mut BehaviorBudget, override_value: Option<&Value>) {
    let Some(Value::Object(obj)) = override_value else {
        return;
    };

    if let Some(value) = obj.get("max_added_latency_ms").and_then(Value::as_i64) {
        target.max_added_latency_ms = value;
    }
    if let Some(value) = obj.get("timeout_reserve_ms").and_then(Value::as_i64) {
        target.timeout_reserve_ms = value;
    }
    if let Some(value) = obj.get("max_step_count").and_then(Value::as_i64) {
        target.max_step_count = value;
    }
}

fn resolve_allowed_primitives(
    page_archetype: &str,
    behavior_profile_json: Option<&Value>,
    task_allowed_primitives: Option<&Value>,
    site_allowed_primitives: Option<&Value>,
    warnings: &mut Vec<String>,
) -> Vec<String> {
    let mut requested = extract_string_array(
        behavior_profile_json.and_then(|value| value.get("allowed_primitives")),
    );
    if requested.is_empty() {
        requested = default_primitives_for_archetype(page_archetype)
            .into_iter()
            .map(|value| value.to_string())
            .collect();
    }
    if let Some(site_values) = extract_optional_string_array(site_allowed_primitives) {
        requested = site_values;
    }
    if let Some(task_values) = extract_optional_string_array(task_allowed_primitives) {
        requested = task_values;
    }

    let mut deduped = Vec::new();
    for item in requested {
        if SUPPORTED_PRIMITIVES.contains(&item.as_str()) {
            if !deduped.iter().any(|existing| existing == &item) {
                deduped.push(item);
            }
        } else {
            warnings.push(format!("unsupported primitive ignored: {item}"));
        }
    }

    deduped
}

fn collect_ignored_primitives(
    task_allowed_primitives: Option<&Value>,
    site_allowed_primitives: Option<&Value>,
) -> Vec<String> {
    let mut ignored = Vec::new();
    for source in [site_allowed_primitives, task_allowed_primitives] {
        for item in extract_string_array(source) {
            if !SUPPORTED_PRIMITIVES.contains(&item.as_str()) {
                ignored.push(item);
            }
        }
    }
    ignored
}

fn extract_optional_string_array(value: Option<&Value>) -> Option<Vec<String>> {
    match value {
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(|item| item.as_str().map(|value| value.to_string()))
                .collect(),
        ),
        _ => None,
    }
}

fn extract_string_array(value: Option<&Value>) -> Vec<String> {
    extract_optional_string_array(value).unwrap_or_default()
}

fn default_primitives_for_archetype(page_archetype: &str) -> Vec<&'static str> {
    match page_archetype {
        "article" => vec![
            "wait_for_readiness",
            "idle",
            "scroll_progressive",
            "pause_on_content",
            "wait_for_content_stable",
            "soft_abort_if_budget_exceeded",
        ],
        "search_results" | "listing" => vec![
            "wait_for_readiness",
            "idle",
            "scroll_progressive",
            "hover_candidate",
            "pause_on_content",
            "soft_abort_if_budget_exceeded",
        ],
        "product" => vec![
            "wait_for_readiness",
            "idle",
            "scroll_to_ratio",
            "hover_candidate",
            "pause_on_content",
            "soft_abort_if_budget_exceeded",
        ],
        "form" | "auth" => vec![
            "wait_for_readiness",
            "focus_element",
            "type_with_rhythm",
            "clear_with_corrections",
            "blur_element",
            "wait_for_content_stable",
            "persist_session_state",
            "soft_abort_if_budget_exceeded",
        ],
        "dashboard" => vec![
            "wait_for_readiness",
            "idle",
            "hover_candidate",
            "scroll_progressive",
            "persist_session_state",
            "soft_abort_if_budget_exceeded",
        ],
        _ => vec![
            "wait_for_readiness",
            "idle",
            "scroll_progressive",
            "pause_on_content",
            "soft_abort_if_budget_exceeded",
        ],
    }
}

fn resolve_seed(
    task_kind: &str,
    url: Option<&str>,
    page_archetype: &str,
    behavior_profile_id: Option<&str>,
    task_seed: Option<&str>,
    site_seed: Option<&str>,
) -> String {
    task_seed
        .or(site_seed)
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            let mut hasher = DefaultHasher::new();
            task_kind.hash(&mut hasher);
            url.unwrap_or_default().hash(&mut hasher);
            page_archetype.hash(&mut hasher);
            behavior_profile_id.unwrap_or_default().hash(&mut hasher);
            format!("seed-{:#x}", hasher.finish())
        })
}

fn deterministic_bucket(seed: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    hasher.finish() % 10
}

fn deterministic_i64(seed: &str, label: &str, min: i64, max: i64) -> i64 {
    if max <= min {
        return min;
    }
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    label.hash(&mut hasher);
    let bucket = hasher.finish();
    min + (bucket % (u64::try_from(max - min + 1).unwrap_or(1))) as i64
}

fn build_behavior_plan(
    behavior_mode: &str,
    page_archetype: &str,
    seed: &str,
    budget: &BehaviorBudget,
    allowed_primitives: &[String],
) -> (Option<RunnerBehaviorPlan>, Vec<String>) {
    if behavior_mode == BEHAVIOR_MODE_DISABLED {
        return (None, Vec::new());
    }

    let mut step_candidates = default_plan_steps(page_archetype, seed);
    step_candidates.retain(|step| {
        step.get("primitive")
            .and_then(Value::as_str)
            .map(|primitive| allowed_primitives.iter().any(|item| item == primitive))
            .unwrap_or(false)
    });

    let max_step_count = usize::try_from(budget.max_step_count.max(0)).unwrap_or_default();
    let mut skipped_steps = Vec::new();
    if max_step_count > 0 && step_candidates.len() > max_step_count {
        skipped_steps = step_candidates
            .iter()
            .skip(max_step_count)
            .filter_map(|step| {
                step.get("primitive")
                    .and_then(Value::as_str)
                    .map(|value| value.to_string())
            })
            .collect();
        step_candidates.truncate(max_step_count);
    }

    if allowed_primitives
        .iter()
        .any(|item| item == "soft_abort_if_budget_exceeded")
        && !step_candidates.iter().any(|step| {
            step.get("primitive").and_then(Value::as_str) == Some("soft_abort_if_budget_exceeded")
        })
    {
        step_candidates.push(json!({
            "phase": "guard",
            "primitive": "soft_abort_if_budget_exceeded",
            "budget_ms": budget.max_added_latency_ms,
        }));
    }

    (
        Some(RunnerBehaviorPlan {
            plan_version: 1,
            seed: seed.to_string(),
            page_archetype: Some(page_archetype.to_string()),
            budget_json: serde_json::to_value(budget).ok(),
            steps_json: Value::Array(step_candidates),
        }),
        skipped_steps,
    )
}

fn default_plan_steps(page_archetype: &str, seed: &str) -> Vec<Value> {
    match page_archetype {
        "article" => vec![
            json!({"phase": "readiness", "primitive": "wait_for_readiness"}),
            json!({"phase": "settle", "primitive": "idle", "duration_ms": deterministic_i64(seed, "article:idle", 220, 900)}),
            json!({"phase": "scan", "primitive": "scroll_progressive", "segments": deterministic_i64(seed, "article:scroll", 2, 5)}),
            json!({"phase": "consume", "primitive": "pause_on_content", "duration_ms": deterministic_i64(seed, "article:pause", 400, 1600)}),
            json!({"phase": "consume", "primitive": "wait_for_content_stable", "stable_window_ms": deterministic_i64(seed, "article:stable", 500, 1400)}),
        ],
        "search_results" | "listing" => vec![
            json!({"phase": "readiness", "primitive": "wait_for_readiness"}),
            json!({"phase": "settle", "primitive": "idle", "duration_ms": deterministic_i64(seed, "listing:idle", 150, 500)}),
            json!({"phase": "scan", "primitive": "scroll_progressive", "segments": deterministic_i64(seed, "listing:scroll", 2, 4)}),
            json!({"phase": "scan", "primitive": "hover_candidate", "target": "ranked-visible-item"}),
            json!({"phase": "scan", "primitive": "pause_on_content", "duration_ms": deterministic_i64(seed, "listing:pause", 180, 600)}),
        ],
        "product" => vec![
            json!({"phase": "readiness", "primitive": "wait_for_readiness"}),
            json!({"phase": "settle", "primitive": "idle", "duration_ms": deterministic_i64(seed, "product:idle", 180, 700)}),
            json!({"phase": "scan", "primitive": "scroll_to_ratio", "ratio": deterministic_i64(seed, "product:ratio", 35, 88)}),
            json!({"phase": "scan", "primitive": "hover_candidate", "target": "primary-media"}),
            json!({"phase": "consume", "primitive": "pause_on_content", "duration_ms": deterministic_i64(seed, "product:pause", 220, 1000)}),
        ],
        "form" | "auth" => vec![
            json!({"phase": "readiness", "primitive": "wait_for_readiness"}),
            json!({"phase": "focus", "primitive": "focus_element", "target": "primary-form-field"}),
            json!({"phase": "input", "primitive": "type_with_rhythm", "profile": "human_rhythm"}),
            json!({"phase": "input", "primitive": "clear_with_corrections", "max_corrections": deterministic_i64(seed, "form:corrections", 1, 3)}),
            json!({"phase": "settle", "primitive": "blur_element", "target": "primary-form-field"}),
            json!({"phase": "settle", "primitive": "wait_for_content_stable", "stable_window_ms": deterministic_i64(seed, "form:stable", 350, 900)}),
            json!({"phase": "persist", "primitive": "persist_session_state"}),
        ],
        "dashboard" => vec![
            json!({"phase": "readiness", "primitive": "wait_for_readiness"}),
            json!({"phase": "settle", "primitive": "idle", "duration_ms": deterministic_i64(seed, "dashboard:idle", 200, 600)}),
            json!({"phase": "scan", "primitive": "hover_candidate", "target": "interactive-module"}),
            json!({"phase": "scan", "primitive": "scroll_progressive", "segments": deterministic_i64(seed, "dashboard:scroll", 1, 3)}),
            json!({"phase": "persist", "primitive": "persist_session_state"}),
        ],
        _ => vec![
            json!({"phase": "readiness", "primitive": "wait_for_readiness"}),
            json!({"phase": "settle", "primitive": "idle", "duration_ms": deterministic_i64(seed, "generic:idle", 180, 500)}),
            json!({"phase": "scan", "primitive": "scroll_progressive", "segments": deterministic_i64(seed, "generic:scroll", 1, 3)}),
            json!({"phase": "consume", "primitive": "pause_on_content", "duration_ms": deterministic_i64(seed, "generic:pause", 160, 480)}),
        ],
    }
}

fn step_count(plan: &RunnerBehaviorPlan) -> i64 {
    plan.steps_json
        .as_array()
        .map(|items| i64::try_from(items.len()).unwrap_or_default())
        .unwrap_or_default()
}

fn estimate_added_latency_ms(plan: Option<&RunnerBehaviorPlan>) -> i64 {
    let Some(plan) = plan else {
        return 0;
    };
    plan.steps_json
        .as_array()
        .map(|steps| {
            steps
                .iter()
                .map(|step| {
                    step.get("duration_ms")
                        .and_then(Value::as_i64)
                        .or_else(|| step.get("stable_window_ms").and_then(Value::as_i64))
                        .unwrap_or_else(|| match step.get("primitive").and_then(Value::as_str) {
                            Some("wait_for_readiness") => 300,
                            Some("scroll_progressive") => 700,
                            Some("scroll_to_ratio") => 500,
                            Some("hover_candidate") => 180,
                            Some("focus_element") => 120,
                            Some("blur_element") => 80,
                            Some("type_with_rhythm") => 900,
                            Some("clear_with_corrections") => 500,
                            Some("persist_session_state") => 120,
                            Some("soft_abort_if_budget_exceeded") => 0,
                            _ => 200,
                        })
                })
                .sum()
        })
        .unwrap_or_default()
}

async fn load_identity_defaults(
    db: &DbPool,
    identity_profile_id: Option<&str>,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let Some(identity_profile_id) = identity_profile_id else {
        return Ok((None, None, None));
    };

    let row = sqlx::query(
        r#"SELECT fingerprint_profile_id, behavior_profile_id, network_profile_id
           FROM identity_profiles
           WHERE id = ? AND status = 'active'"#,
    )
    .bind(identity_profile_id)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Err(anyhow!(
            "identity profile not found or inactive: {identity_profile_id}"
        ));
    };

    Ok((
        row.try_get("fingerprint_profile_id")?,
        row.try_get("behavior_profile_id")?,
        row.try_get("network_profile_id")?,
    ))
}

async fn validate_session_profile(db: &DbPool, session_profile_id: Option<&str>) -> Result<()> {
    let Some(session_profile_id) = session_profile_id else {
        return Ok(());
    };

    let exists = sqlx::query_scalar::<_, String>(
        r#"SELECT id FROM session_profiles WHERE id = ? AND status = 'active'"#,
    )
    .bind(session_profile_id)
    .fetch_optional(db)
    .await?;
    if exists.is_none() {
        return Err(anyhow!(
            "session profile not found or inactive: {session_profile_id}"
        ));
    }
    Ok(())
}

async fn load_site_behavior_policy(
    db: &DbPool,
    site_key: Option<&str>,
    page_archetype: Option<&str>,
    action_kind: Option<&str>,
) -> Result<Option<SiteBehaviorPolicyRecord>> {
    let Some(site_key) = site_key else {
        return Ok(None);
    };

    let row = sqlx::query(
        r#"SELECT behavior_profile_id, override_json
           FROM site_behavior_policies
           WHERE status = 'active'
             AND site_key = ?
             AND (? IS NULL OR page_archetype IS NULL OR page_archetype = ?)
             AND (? IS NULL OR action_kind IS NULL OR action_kind = ?)
           ORDER BY priority DESC, created_at DESC, id DESC
           LIMIT 1"#,
    )
    .bind(site_key)
    .bind(page_archetype)
    .bind(page_archetype)
    .bind(action_kind)
    .bind(action_kind)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    let override_json: Option<String> = row.try_get("override_json")?;
    Ok(Some(SiteBehaviorPolicyRecord {
        behavior_profile_id: row.try_get("behavior_profile_id")?,
        override_json: override_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok()),
    }))
}

async fn resolve_behavior_profile(
    db: &DbPool,
    explicit_policy_behavior_profile_id: Option<&str>,
    explicit_intent_behavior_profile_id: Option<&str>,
    site_behavior_profile_id: Option<&str>,
    identity_default_behavior_profile_id: Option<&str>,
) -> Result<Option<ResolvedBehaviorProfile>> {
    if let Some(profile_id) = explicit_policy_behavior_profile_id {
        return Ok(Some(
            load_behavior_profile(db, profile_id, "task_policy").await?,
        ));
    }
    if let Some(profile_id) = explicit_intent_behavior_profile_id {
        return Ok(Some(
            load_behavior_profile(db, profile_id, "execution_intent").await?,
        ));
    }
    if let Some(profile_id) = site_behavior_profile_id {
        return Ok(Some(
            load_behavior_profile(db, profile_id, "site_policy").await?,
        ));
    }
    if let Some(profile_id) = identity_default_behavior_profile_id {
        return Ok(Some(
            load_behavior_profile(db, profile_id, "identity_default").await?,
        ));
    }

    Ok(Some(ResolvedBehaviorProfile {
        profile: system_default_behavior_profile(),
        source: "system_default".to_string(),
    }))
}

async fn load_behavior_profile(
    db: &DbPool,
    profile_id: &str,
    source: &str,
) -> Result<ResolvedBehaviorProfile> {
    let row = sqlx::query(
        r#"SELECT id, version, profile_json
           FROM behavior_profiles
           WHERE id = ? AND status = 'active'"#,
    )
    .bind(profile_id)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Err(anyhow!(
            "behavior profile not found or inactive: {profile_id}"
        ));
    };

    let profile_json: String = row.try_get("profile_json")?;
    Ok(ResolvedBehaviorProfile {
        profile: RunnerBehaviorProfile {
            id: row.try_get("id")?,
            version: row.try_get("version")?,
            profile_json: serde_json::from_str::<Value>(&profile_json)
                .unwrap_or_else(|_| json!({})),
        },
        source: source.to_string(),
    })
}

pub fn system_default_behavior_profile() -> RunnerBehaviorProfile {
    RunnerBehaviorProfile {
        id: SYSTEM_DEFAULT_BEHAVIOR_PROFILE_ID.to_string(),
        version: SYSTEM_DEFAULT_BEHAVIOR_PROFILE_VERSION,
        profile_json: json!({
            "persona_class": "system_default_browser",
            "tempo_model": {
                "idle_min_ms": 180,
                "idle_max_ms": 900
            },
            "reading_model": {
                "pause_on_content": true
            },
            "scroll_model": {
                "style": "progressive"
            },
            "typing_model": {
                "style": "rhythm"
            },
            "allowed_primitives": SUPPORTED_PRIMITIVES,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use crate::db::init::{init_db, DbPool};

    fn unique_db_url() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("sqlite:///tmp/persona_pilot_behavior_test_{nanos}.db")
    }

    async fn seed_behavior_profile(db: &DbPool, profile_id: &str, allowed_primitives: &[&str]) {
        sqlx::query(
            r#"INSERT INTO behavior_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
               VALUES (?, ?, 1, 'active', NULL, ?, '1', '1')"#,
        )
        .bind(profile_id)
        .bind(format!("Behavior {profile_id}"))
        .bind(
            json!({
                "allowed_primitives": allowed_primitives,
            })
            .to_string(),
        )
        .execute(db)
        .await
        .expect("insert behavior profile");
    }

    async fn seed_site_behavior_policy(
        db: &DbPool,
        policy_id: &str,
        behavior_profile_id: &str,
        override_json: Value,
    ) {
        sqlx::query(
            r#"INSERT INTO site_behavior_policies (
                   id, version, site_key, page_archetype, action_kind, behavior_profile_id,
                   priority, required, override_json, status, created_at, updated_at
               ) VALUES (?, 1, 'example.com', 'article', 'open_page', ?, 10, 0, ?, 'active', '1', '1')"#,
        )
        .bind(policy_id)
        .bind(behavior_profile_id)
        .bind(override_json.to_string())
        .execute(db)
        .await
        .expect("insert site behavior policy");
    }

    #[tokio::test]
    async fn compile_behavior_plan_is_stable_for_same_explicit_seed() {
        let db = init_db(&unique_db_url()).await.expect("init db");
        seed_behavior_profile(
            &db,
            "beh-stable",
            &[
                "wait_for_readiness",
                "idle",
                "scroll_progressive",
                "soft_abort_if_budget_exceeded",
            ],
        )
        .await;

        let execution_intent = RunnerExecutionIntent {
            behavior_profile_id: Some("beh-stable".to_string()),
            ..RunnerExecutionIntent::default()
        };

        let first = compile_behavior_plan(
            &db,
            "open_page",
            Some("https://example.com/article/stable.html"),
            Some(12),
            execution_intent.clone(),
            Some(json!({
                "mode": "active",
                "page_archetype": "article",
                "plan_seed": "seed-stable-001"
            })),
        )
        .await
        .expect("compile first plan");
        let second = compile_behavior_plan(
            &db,
            "open_page",
            Some("https://example.com/article/stable.html"),
            Some(12),
            execution_intent,
            Some(json!({
                "mode": "active",
                "page_archetype": "article",
                "plan_seed": "seed-stable-001"
            })),
        )
        .await
        .expect("compile second plan");

        assert_eq!(first.behavior_seed, Some("seed-stable-001".to_string()));
        assert_eq!(first.behavior_seed, second.behavior_seed);
        assert_eq!(
            first
                .behavior_plan
                .as_ref()
                .expect("first behavior plan")
                .steps_json,
            second
                .behavior_plan
                .as_ref()
                .expect("second behavior plan")
                .steps_json
        );
        assert_eq!(
            first.behavior_runtime_explain.applied_primitives,
            second.behavior_runtime_explain.applied_primitives
        );
    }

    #[tokio::test]
    async fn compile_behavior_plan_applies_site_override_when_enabled() {
        let db = init_db(&unique_db_url()).await.expect("init db");
        seed_behavior_profile(
            &db,
            "beh-site-override",
            &[
                "wait_for_readiness",
                "idle",
                "scroll_progressive",
                "pause_on_content",
                "soft_abort_if_budget_exceeded",
            ],
        )
        .await;
        seed_site_behavior_policy(
            &db,
            "site-policy-article",
            "beh-site-override",
            json!({
                "allowed_primitives": ["idle", "soft_abort_if_budget_exceeded"]
            }),
        )
        .await;

        let with_override = compile_behavior_plan(
            &db,
            "open_page",
            Some("https://example.com/article/with-policy.html"),
            Some(10),
            RunnerExecutionIntent::default(),
            Some(json!({
                "mode": "active",
                "page_archetype": "article"
            })),
        )
        .await
        .expect("compile plan with site override");
        let without_override = compile_behavior_plan(
            &db,
            "open_page",
            Some("https://example.com/article/with-policy.html"),
            Some(10),
            RunnerExecutionIntent::default(),
            Some(json!({
                "mode": "active",
                "page_archetype": "article",
                "allow_site_overrides": false
            })),
        )
        .await
        .expect("compile plan without site override");

        let with_override_primitives = with_override
            .behavior_plan
            .as_ref()
            .and_then(|plan| plan.steps_json.as_array())
            .expect("steps with override")
            .iter()
            .filter_map(|step| step.get("primitive").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let without_override_primitives = without_override
            .behavior_plan
            .as_ref()
            .and_then(|plan| plan.steps_json.as_array())
            .expect("steps without override")
            .iter()
            .filter_map(|step| step.get("primitive").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert!(with_override_primitives
            .iter()
            .all(|item| matches!(*item, "idle" | "soft_abort_if_budget_exceeded")));
        assert!(without_override_primitives.len() > with_override_primitives.len());
        assert_eq!(
            with_override.behavior_profile_id.as_deref(),
            Some("beh-site-override")
        );
        assert_eq!(
            without_override.behavior_profile_id.as_deref(),
            Some(SYSTEM_DEFAULT_BEHAVIOR_PROFILE_ID)
        );
    }

    #[tokio::test]
    async fn compile_behavior_plan_clips_budget_to_timeout_window() {
        let db = init_db(&unique_db_url()).await.expect("init db");

        let compiled = compile_behavior_plan(
            &db,
            "open_page",
            Some("https://example.com/article/budget.html"),
            Some(2),
            RunnerExecutionIntent::default(),
            Some(json!({
                "mode": "active",
                "budget": {
                    "max_added_latency_ms": 10000,
                    "timeout_reserve_ms": 1500,
                    "max_step_count": 24
                }
            })),
        )
        .await
        .expect("compile clipped budget");

        let budget = compiled
            .behavior_runtime_explain
            .budget
            .expect("behavior budget");
        assert_eq!(budget.max_added_latency_ms, 500);
        assert_eq!(budget.timeout_reserve_ms, 1500);
        assert!(compiled
            .behavior_runtime_explain
            .warnings
            .iter()
            .any(|warning| warning.contains("clipped")));
    }

    #[tokio::test]
    async fn profile_required_without_non_default_profile_downgrades_to_disabled() {
        let db = init_db(&unique_db_url()).await.expect("init db");

        let compiled = compile_behavior_plan(
            &db,
            "open_page",
            Some("https://example.com/login"),
            Some(10),
            RunnerExecutionIntent::default(),
            Some(json!({
                "mode": "profile_required"
            })),
        )
        .await
        .expect("compile profile required");

        assert_eq!(compiled.behavior_execution_mode, BEHAVIOR_MODE_DISABLED);
        assert_eq!(compiled.behavior_resolution_status, "disabled");
        assert_eq!(
            compiled.behavior_profile_id.as_deref(),
            Some(SYSTEM_DEFAULT_BEHAVIOR_PROFILE_ID)
        );
        assert!(compiled.behavior_plan.is_none());
        assert!(compiled
            .behavior_runtime_explain
            .warnings
            .iter()
            .any(|warning| warning.contains("profile_required requested")));
    }
}
