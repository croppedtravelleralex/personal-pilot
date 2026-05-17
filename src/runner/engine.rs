use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use reqwest::Url;
use serde_json::{json, Value};
use sqlx::Row;
use tokio::{sync::oneshot, task::JoinHandle, time::Duration};
use uuid::Uuid;

use crate::network_identity::fingerprint_consumption::{
    build_lightpanda_runtime_projection, fingerprint_perf_budget_tag_from_json,
    FINGERPRINT_CONSUMPTION_SOURCE_RUNTIME,
};
use crate::network_identity::proxy_growth::{
    assess_proxy_pool_health, evaluate_region_match, proxy_pool_growth_policy_from_env,
    ProxyPoolInventorySnapshot,
};
use crate::network_identity::proxy_selection::{
    apply_proxy_resolution_metadata, proxy_selection_base_where_sql,
    proxy_selection_order_by_trust_score_sql_with_tuning, resolved_proxy_json,
};
use crate::{
    api::dto::{
        CandidateRankPreviewItem, TrustScoreComponents, WinnerVsRunnerUpDiff,
        WinnerVsRunnerUpDirection, WinnerVsRunnerUpFactor,
    },
    api::handlers::apply_task_continuity_after_execution,
    app::state::AppState,
    behavior::{
        form::{
            build_form_action_summary_json, resolve_form_action_plan_for_task,
            FORM_ACTION_STATUS_BLOCKED, FORM_ACTION_STATUS_FAILED,
            FORM_ACTION_STATUS_NOT_REQUESTED, FORM_ACTION_STATUS_SHADOW_ONLY,
            FORM_ACTION_STATUS_SUCCEEDED,
        },
        should_store_raw_trace, BehaviorRuntimeExplain, BehaviorTraceSummary,
    },
    db::init::refresh_proxy_trust_views_for_scope,
    domain::{
        run::{
            RUN_STATUS_CANCELLED, RUN_STATUS_FAILED, RUN_STATUS_RUNNING, RUN_STATUS_SUCCEEDED,
            RUN_STATUS_TIMED_OUT,
        },
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING,
            TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
    network_identity::fingerprint_consistency::assess_fingerprint_profile_consistency,
    network_identity::fingerprint_policy::FingerprintPerfBudgetTag,
    runner::{
        runner_claim_retry_limit_from_env, runner_heartbeat_interval_seconds_from_env,
        RunnerBehaviorPlan, RunnerBehaviorProfile, RunnerExecutionIntent, RunnerExecutionResult,
        RunnerFingerprintProfile, RunnerFormActionPlan, RunnerOutcomeStatus, RunnerProxySelection,
        RunnerTask, TaskRunner,
    },
};

fn now_ts_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

fn fingerprint_perf_budget_tag_from_profile_json(
    profile_json: Option<&str>,
) -> FingerprintPerfBudgetTag {
    fingerprint_perf_budget_tag_from_json(profile_json)
}

fn medium_budget_limit(worker_count: usize) -> usize {
    std::env::var("PERSONA_PILOT_FP_MEDIUM_MAX_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or_else(|| worker_count.max(2))
}

fn heavy_budget_limit(worker_count: usize) -> usize {
    std::env::var("PERSONA_PILOT_FP_HEAVY_MAX_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or_else(|| worker_count.clamp(1, 2))
}

fn claim_candidate_scan_limit() -> i64 {
    std::env::var("PERSONA_PILOT_RUNNER_CLAIM_SCAN_LIMIT")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(16)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClaimBudgetSnapshot {
    running_medium: usize,
    running_heavy: usize,
    medium_limit: usize,
    heavy_limit: usize,
}

fn pick_claim_candidate_index(
    candidate_profile_jsons: &[Option<String>],
    budget: ClaimBudgetSnapshot,
) -> Option<usize> {
    candidate_profile_jsons.iter().position(|profile_json| {
        let tag = fingerprint_perf_budget_tag_from_profile_json(profile_json.as_deref());
        match tag {
            FingerprintPerfBudgetTag::Light => true,
            FingerprintPerfBudgetTag::Medium => budget.running_medium < budget.medium_limit,
            FingerprintPerfBudgetTag::Heavy => budget.running_heavy < budget.heavy_limit,
        }
    })
}

fn build_fingerprint_runtime_explain_json(
    task_payload: &Value,
    fingerprint_profile: Option<&RunnerFingerprintProfile>,
    selected_proxy: Option<&RunnerProxySelection>,
) -> Value {
    let target_region = task_payload
        .get("target_region")
        .and_then(|v| v.as_str())
        .or_else(|| task_payload.get("region").and_then(|v| v.as_str()))
        .or_else(|| {
            task_payload
                .get("network_policy_json")
                .and_then(|v| v.get("region"))
                .and_then(|v| v.as_str())
        });

    let consumption_explain = task_payload
        .get("fingerprint_runtime")
        .and_then(|v| v.get("consumption_explain"))
        .cloned()
        .or_else(|| {
            fingerprint_profile.map(|profile| {
                let projection = build_lightpanda_runtime_projection(
                    &profile.id,
                    profile.version,
                    &profile.profile_json,
                );
                json!({
                    "declared_fields": projection.consumption.declared_fields,
                    "resolved_fields": projection.consumption.resolved_fields,
                    "applied_fields": projection.consumption.applied_fields,
                    "ignored_fields": projection.consumption.ignored_fields,
                    "declared_count": projection.consumption.declared_count(),
                    "resolved_count": projection.consumption.resolved_count(),
                    "applied_count": projection.consumption.applied_count(),
                    "ignored_count": projection.consumption.ignored_count(),
                    "consumption_status": projection.consumption.consumption_status,
                    "consumption_version": projection.consumption.consumption_version,
                    "partial_support_warning": projection.consumption.partial_support_warning
                })
            })
        });
    let consumption_version = task_payload
        .get("fingerprint_runtime")
        .and_then(|v| v.get("consumption_version"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            consumption_explain
                .as_ref()
                .and_then(|v| v.get("consumption_version"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
    let consumption_source_of_truth = task_payload
        .get("fingerprint_runtime")
        .and_then(|v| v.get("consumption_source_of_truth"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            fingerprint_profile.map(|_| FINGERPRINT_CONSUMPTION_SOURCE_RUNTIME.to_string())
        });
    let consumption_status = task_payload
        .get("fingerprint_runtime")
        .and_then(|v| v.get("consumption_status"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            consumption_explain
                .as_ref()
                .and_then(|v| v.get("consumption_status"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
    let warning = task_payload
        .get("fingerprint_runtime")
        .and_then(|v| v.get("warning"))
        .cloned();

    let (budget_tag, consistency) = match fingerprint_profile {
        Some(profile) => {
            let budget_tag = match fingerprint_perf_budget_tag_from_profile_json(Some(
                &profile.profile_json.to_string(),
            )) {
                FingerprintPerfBudgetTag::Light => "light",
                FingerprintPerfBudgetTag::Medium => "medium",
                FingerprintPerfBudgetTag::Heavy => "heavy",
            };
            let consistency = assess_fingerprint_profile_consistency(
                target_region,
                selected_proxy.and_then(|p| p.region.as_deref()),
                None,
                &profile.profile_json,
            );
            (Some(budget_tag), Some(consistency))
        }
        None => (None, None),
    };

    json!({
        "fingerprint_budget_tag": budget_tag,
        "fingerprint_consistency": consistency,
        "consumption_source_of_truth": consumption_source_of_truth,
        "consumption_version": consumption_version,
        "consumption_status": consumption_status,
        "warning": warning,
        "consumption_explain": consumption_explain,
    })
}

async fn insert_log(
    state: &AppState,
    log_id: &str,
    task_id: &str,
    run_id: Option<&str>,
    level: &str,
    message: &str,
) -> Result<()> {
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
    .await?;
    Ok(())
}

fn extract_proxy_selection(payload: &Value) -> Option<RunnerProxySelection> {
    let policy = payload.get("network_policy_json")?;
    let proxy_obj = policy.get("resolved_proxy")?;
    Some(RunnerProxySelection {
        id: proxy_obj.get("id")?.as_str()?.to_string(),
        scheme: proxy_obj.get("scheme")?.as_str()?.to_string(),
        host: proxy_obj.get("host")?.as_str()?.to_string(),
        port: proxy_obj.get("port")?.as_i64()?,
        username: proxy_obj
            .get("username")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        password: proxy_obj
            .get("password")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        region: proxy_obj
            .get("region")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        country: proxy_obj
            .get("country")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        provider: proxy_obj
            .get("provider")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        score: proxy_obj
            .get("score")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0),
        resolution_status: policy
            .get("proxy_resolution_status")
            .and_then(|v| v.as_str())
            .unwrap_or("resolved")
            .to_string(),
        source_label: proxy_obj
            .get("source_label")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        source_tier: proxy_obj
            .get("source_tier")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        verification_path: proxy_obj
            .get("verification_path")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        last_verify_source: proxy_obj
            .get("last_verify_source")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        last_exit_country: proxy_obj
            .get("last_exit_country")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
        last_exit_region: proxy_obj
            .get("last_exit_region")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string()),
    })
}

fn is_browser_task_kind(kind: &str) -> bool {
    matches!(
        kind,
        "open_page" | "get_html" | "get_title" | "get_final_url" | "extract_text"
    )
}

fn browser_task_requires_proxy(kind: &str, payload: &Value) -> bool {
    if !is_browser_task_kind(kind) {
        return false;
    }
    payload
        .get("network_policy_json")
        .and_then(|value| value.get("require_proxy"))
        .and_then(|value| value.as_bool())
        .unwrap_or_else(|| {
            payload
                .get("network_policy_json")
                .and_then(|value| value.get("mode"))
                .and_then(|value| value.as_str())
                .map(|mode| !mode.eq_ignore_ascii_case("direct"))
                .unwrap_or(true)
        })
}

fn no_eligible_proxy_execution(
    task_kind: &str,
    payload: &Value,
    fingerprint_profile: Option<&RunnerFingerprintProfile>,
) -> RunnerExecutionResult {
    let message = "proxy required but no eligible active proxy matched".to_string();
    let fingerprint_json = fingerprint_profile.map(|profile| {
        json!({
            "id": profile.id,
            "version": profile.version,
            "profile": profile.profile_json,
        })
    });
    RunnerExecutionResult {
        status: RunnerOutcomeStatus::Failed,
        result_json: Some(json!({
            "status": "failed",
            "error_kind": "no_eligible_proxy",
            "failure_scope": "network_policy",
            "execution_stage": "selection",
            "browser_failure_signal": Value::Null,
            "message": message.clone(),
            "payload": payload.clone(),
            "fingerprint_profile": fingerprint_json,
            "proxy": Value::Null,
        })),
        error_message: Some(message.clone()),
        summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
            category: crate::runner::types::SummaryArtifactCategory::Summary,
            key: format!("{}.proxy_required_failure", task_kind),
            source: "selection.proxy_pool".to_string(),
            severity: crate::runner::types::SummaryArtifactSeverity::Error,
            title: "proxy selection failed".to_string(),
            summary: "proxy required but no eligible active proxy matched; replenish or candidate promotion is needed".to_string(),
        }],
        session_cookies: None,
        session_local_storage: None,
        session_session_storage: None,
    }
}

#[derive(Debug, Clone, Default)]
struct RunnerSessionContext {
    session_key: Option<String>,
    site_key: Option<String>,
    persona_id: Option<String>,
    fingerprint_profile_id: Option<String>,
    requested_region: Option<String>,
    requested_provider: Option<String>,
    restored_cookies: Option<Vec<Value>>,
    cookie_restore_count: i64,
    restored_local_storage: Option<Value>,
    restored_session_storage: Option<Value>,
    local_storage_restore_count: i64,
    session_storage_restore_count: i64,
    prior_proxy_id: Option<String>,
    identity_session_status: String,
    auto_session_enabled: bool,
}

fn task_site_key_from_payload(payload: &Value) -> Option<String> {
    let url = payload.get("url").and_then(|value| value.as_str())?;
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_ascii_lowercase()))
}

fn storage_entry_count(value: Option<&Value>) -> i64 {
    value
        .and_then(Value::as_object)
        .map(|items| i64::try_from(items.len()).unwrap_or(0))
        .unwrap_or(0)
}

fn decode_storage_json(raw: Option<String>) -> Option<Value> {
    let raw = raw?;
    let parsed = serde_json::from_str::<Value>(&raw).ok()?;
    if parsed.is_object() {
        Some(parsed)
    } else {
        None
    }
}

#[derive(Debug, Clone, Default)]
struct ProxySiteScoreSignal {
    site_success_bonus: i64,
    site_failure_penalty: i64,
}

#[derive(Debug, Clone)]
struct AutoSelectionCandidateRow {
    id: String,
    scheme: String,
    host: String,
    port: i64,
    username: Option<String>,
    password: Option<String>,
    region: Option<String>,
    country: Option<String>,
    provider: Option<String>,
    score: f64,
    last_used_at: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone)]
struct RankedAutoSelectionCandidate {
    row: AutoSelectionCandidateRow,
    trust_score_total: Option<i64>,
    trust_score_components: TrustScoreComponents,
}

fn auto_identity_session_key(kind: &str, payload: &Value) -> Option<String> {
    if !is_browser_task_kind(kind) {
        return None;
    }
    let fingerprint_profile_id = payload
        .get("fingerprint_profile_id")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let site_key = task_site_key_from_payload(payload)?;
    let requested_region = payload
        .get("network_policy_json")
        .and_then(|value| value.get("region"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("_any_region");
    let requested_provider = payload
        .get("network_policy_json")
        .and_then(|value| value.get("provider"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("_any_provider");
    Some(format!(
        "auto:{}:{}:{}:{}",
        fingerprint_profile_id, site_key, requested_region, requested_provider
    ))
}

fn decode_cookies_json(raw: Option<String>) -> Option<Vec<Value>> {
    let raw = raw?;
    let parsed = serde_json::from_str::<Value>(&raw).ok()?;
    parsed.as_array().cloned()
}

fn selection_reason_summary_for_mode(
    mode: &str,
    trust_score_total: Option<i64>,
    candidate_summary: Option<&str>,
) -> String {
    match (mode, trust_score_total, candidate_summary) {
        ("explicit", Some(score), _) => format!("explicit proxy_id selected active proxy directly; current trust score snapshot={score}"),
        ("sticky", Some(score), _) => format!("sticky session reused an active proxy binding; current trust score snapshot={score}"),
        ("auto", Some(score), Some(summary)) => format!("selected highest-ranked active proxy by trust score ordering; trust score total={score}; {summary}"),
        ("auto", None, Some(summary)) => format!("selected highest-ranked active proxy by trust score ordering; {summary}"),
        ("explicit", None, _) => "explicit proxy_id selected active proxy directly".to_string(),
        ("sticky", None, _) => "sticky session reused an active proxy binding".to_string(),
        ("auto", Some(score), None) => format!("selected highest-ranked active proxy by trust score ordering; trust score total={score}"),
        _ => "selected highest-ranked active proxy by trust score ordering".to_string(),
    }
}

fn selection_explain_json(
    selection_mode: &str,
    fallback_reason: Option<&str>,
    no_match_reason_code: Option<&str>,
    sticky_binding_age_seconds: Option<i64>,
    sticky_reuse_reason: Option<&str>,
    would_rank_position_if_auto: Option<i64>,
    soft_min_score: Option<f64>,
    soft_min_score_penalty_applied: Option<bool>,
    proxy_growth: Option<Value>,
    fingerprint_budget_tag: Option<&str>,
    fingerprint_budget_medium_limit: Option<usize>,
    fingerprint_budget_heavy_limit: Option<usize>,
) -> Value {
    let eligibility_gate = "active+cooldown+provider/region+min_score";
    json!({
        "selection_mode": selection_mode,
        "explicit_override": selection_mode == "explicit",
        "sticky_reused": selection_mode == "sticky",
        "sticky_binding_age_seconds": sticky_binding_age_seconds,
        "sticky_reuse_reason": sticky_reuse_reason,
        "would_rank_position_if_auto": would_rank_position_if_auto,
        "eligibility_gate": eligibility_gate,
        "soft_min_score": soft_min_score,
        "soft_min_score_penalty_applied": soft_min_score_penalty_applied,
        "fallback_reason": fallback_reason,
        "no_match_reason_code": no_match_reason_code,
        "proxy_growth": proxy_growth,
        "fingerprint_budget_tag": fingerprint_budget_tag,
        "fingerprint_budget_medium_limit": fingerprint_budget_medium_limit,
        "fingerprint_budget_heavy_limit": fingerprint_budget_heavy_limit,
    })
}

pub fn computed_trust_score_components(
    tuning: &crate::network_identity::proxy_selection::ProxySelectionTuning,
    score: f64,
    success_count: i64,
    failure_count: i64,
    last_verify_status: Option<&str>,
    last_verify_geo_match_ok: bool,
    last_region_match_ok: Option<bool>,
    last_smoke_upstream_ok: bool,
    last_verify_at: Option<i64>,
    last_verify_confidence: Option<f64>,
    last_verify_score_delta: Option<i64>,
    last_verify_source: Option<&str>,
    last_anonymity_level: Option<&str>,
    last_probe_latency_ms: Option<i64>,
    last_probe_error_category: Option<&str>,
    provider_risk_hit: bool,
    provider_region_cluster_hit: bool,
    now_ts: i64,
    soft_min_score: Option<f64>,
) -> TrustScoreComponents {
    let verify_ok_bonus = if last_verify_status == Some("ok") {
        tuning.verify_ok_bonus
    } else {
        0
    };
    let verify_geo_match_bonus = if last_verify_geo_match_ok {
        tuning.verify_geo_match_bonus
    } else {
        0
    };
    let geo_mismatch_penalty = if !last_verify_geo_match_ok {
        tuning.geo_mismatch_penalty
    } else {
        0
    };
    let region_mismatch_penalty = if last_region_match_ok == Some(false) {
        tuning.region_mismatch_penalty
    } else {
        0
    };
    let geo_risk_penalty = geo_mismatch_penalty + region_mismatch_penalty;
    let smoke_upstream_ok_bonus = if last_smoke_upstream_ok {
        tuning.smoke_upstream_ok_bonus
    } else {
        0
    };
    let raw_score_component = (score * tuning.raw_score_weight_tenths as f64).round() as i64;
    let missing_verify_penalty = if last_verify_at.is_none() {
        tuning.missing_verify_penalty
    } else {
        0
    };
    let stale_verify_penalty = if last_verify_at
        .map(|v| v <= now_ts - tuning.stale_after_seconds)
        .unwrap_or(false)
    {
        tuning.stale_verify_penalty
    } else {
        0
    };
    let verify_failed_heavy_penalty = if last_verify_status == Some("failed")
        && last_verify_at
            .map(|v| v >= now_ts - tuning.recent_failure_heavy_window_seconds)
            .unwrap_or(false)
    {
        tuning.verify_failed_heavy_penalty
    } else {
        0
    };
    let verify_failed_light_penalty = if last_verify_status == Some("failed")
        && verify_failed_heavy_penalty == 0
        && last_verify_at
            .map(|v| v >= now_ts - tuning.recent_failure_light_window_seconds)
            .unwrap_or(false)
    {
        tuning.verify_failed_light_penalty
    } else {
        0
    };
    let verify_failed_base_penalty = if last_verify_status == Some("failed") {
        tuning.verify_failed_base_penalty
    } else {
        0
    };
    let individual_history_penalty = if failure_count >= success_count + 3 {
        2
    } else if failure_count > success_count {
        1
    } else {
        0
    };
    let provider_risk_penalty = if provider_risk_hit {
        tuning.provider_failure_margin
    } else {
        0
    };
    let provider_region_cluster_penalty = if provider_region_cluster_hit {
        tuning.provider_region_failure_cluster_count
    } else {
        0
    };
    let verify_confidence_bonus = match last_verify_confidence {
        Some(v) if v >= 0.95 => 3,
        Some(v) if v >= 0.85 => 1,
        Some(v) if v > 0.0 && v < 0.60 => -2,
        _ => 0,
    };
    let verify_score_delta_bonus = match last_verify_score_delta {
        Some(v) if v >= 12 => 2,
        Some(v) if v >= 6 => 1,
        Some(v) if v <= -12 => -2,
        Some(v) if v <= -6 => -1,
        _ => 0,
    };
    let verify_source_bonus = match last_verify_source {
        Some("local_verify") => 2,
        Some("runner_verify") => 1,
        Some("imported_verify") | Some("manual_verify") | Some("backfill_verify") => -1,
        Some(_) => 0,
        None => 0,
    };
    let anonymity_bonus = match last_anonymity_level {
        Some("elite") => tuning.anonymity_elite_bonus,
        Some("anonymous") => -tuning.anonymity_anonymous_penalty,
        Some("transparent") => -tuning.anonymity_transparent_penalty,
        _ => 0,
    };
    let latency_penalty = match last_probe_latency_ms {
        Some(v) if v <= 800 => -tuning.low_latency_bonus,
        Some(v) if v <= 2000 => -tuning.medium_latency_bonus,
        Some(v) if v >= 8000 => tuning.very_high_latency_penalty,
        Some(v) if v >= 4000 => tuning.high_latency_penalty,
        _ => 0,
    };
    let exit_ip_not_public_penalty = if last_probe_error_category == Some("exit_ip_not_public") {
        tuning.exit_ip_not_public_penalty
    } else {
        0
    };
    let probe_error_penalty = match last_probe_error_category {
        Some("protocol_invalid") => tuning.probe_error_protocol_penalty,
        Some("upstream_missing") => tuning.probe_error_upstream_missing_penalty,
        Some("connect_failed") => tuning.probe_error_connect_failed_penalty,
        _ => 0,
    };
    let verify_risk_penalty = exit_ip_not_public_penalty + probe_error_penalty;
    let soft_min_score_penalty = if let Some(threshold) = soft_min_score {
        if score < threshold {
            tuning.soft_min_score_penalty
        } else {
            0
        }
    } else {
        0
    };

    TrustScoreComponents {
        verify_ok_bonus,
        verify_geo_match_bonus,
        site_success_bonus: 0,
        geo_mismatch_penalty,
        region_mismatch_penalty,
        geo_risk_penalty,
        smoke_upstream_ok_bonus,
        raw_score_component,
        missing_verify_penalty,
        stale_verify_penalty,
        verify_failed_heavy_penalty,
        verify_failed_light_penalty,
        verify_failed_base_penalty,
        individual_history_penalty,
        provider_risk_penalty,
        provider_region_cluster_penalty,
        verify_confidence_bonus,
        verify_score_delta_bonus,
        verify_source_bonus,
        anonymity_bonus,
        latency_penalty,
        exit_ip_not_public_penalty,
        probe_error_penalty,
        verify_risk_penalty,
        site_failure_penalty: 0,
        soft_min_score_penalty,
    }
}

fn component_value(components: &TrustScoreComponents, key: &str) -> i64 {
    match key {
        "verify_ok_bonus" => components.verify_ok_bonus,
        "verify_geo_match_bonus" => components.verify_geo_match_bonus,
        "site_success_bonus" => components.site_success_bonus,
        "geo_mismatch_penalty" => components.geo_mismatch_penalty,
        "region_mismatch_penalty" => components.region_mismatch_penalty,
        "geo_risk_penalty" => components.geo_risk_penalty,
        "smoke_upstream_ok_bonus" => components.smoke_upstream_ok_bonus,
        "raw_score_component" => components.raw_score_component,
        "missing_verify_penalty" => components.missing_verify_penalty,
        "stale_verify_penalty" => components.stale_verify_penalty,
        "verify_failed_heavy_penalty" => components.verify_failed_heavy_penalty,
        "verify_failed_light_penalty" => components.verify_failed_light_penalty,
        "verify_failed_base_penalty" => components.verify_failed_base_penalty,
        "individual_history_penalty" => components.individual_history_penalty,
        "provider_risk_penalty" => components.provider_risk_penalty,
        "provider_region_cluster_penalty" => components.provider_region_cluster_penalty,
        "verify_confidence_bonus" => components.verify_confidence_bonus,
        "verify_score_delta_bonus" => components.verify_score_delta_bonus,
        "verify_source_bonus" => components.verify_source_bonus,
        "anonymity_bonus" => components.anonymity_bonus,
        "latency_penalty" => components.latency_penalty,
        "exit_ip_not_public_penalty" => components.exit_ip_not_public_penalty,
        "probe_error_penalty" => components.probe_error_penalty,
        "verify_risk_penalty" => components.verify_risk_penalty,
        "site_failure_penalty" => components.site_failure_penalty,
        "soft_min_score_penalty" => components.soft_min_score_penalty,
        _ => 0,
    }
}

fn component_keys() -> [&'static str; 26] {
    [
        "verify_ok_bonus",
        "verify_geo_match_bonus",
        "site_success_bonus",
        "geo_mismatch_penalty",
        "region_mismatch_penalty",
        "geo_risk_penalty",
        "smoke_upstream_ok_bonus",
        "raw_score_component",
        "missing_verify_penalty",
        "stale_verify_penalty",
        "verify_failed_heavy_penalty",
        "verify_failed_light_penalty",
        "verify_failed_base_penalty",
        "individual_history_penalty",
        "provider_risk_penalty",
        "provider_region_cluster_penalty",
        "verify_confidence_bonus",
        "verify_score_delta_bonus",
        "verify_source_bonus",
        "anonymity_bonus",
        "latency_penalty",
        "exit_ip_not_public_penalty",
        "probe_error_penalty",
        "verify_risk_penalty",
        "site_failure_penalty",
        "soft_min_score_penalty",
    ]
}

fn positive_component_keys() -> [&'static str; 9] {
    [
        "verify_ok_bonus",
        "verify_geo_match_bonus",
        "site_success_bonus",
        "smoke_upstream_ok_bonus",
        "raw_score_component",
        "verify_confidence_bonus",
        "verify_score_delta_bonus",
        "verify_source_bonus",
        "anonymity_bonus",
    ]
}

fn empty_components() -> TrustScoreComponents {
    TrustScoreComponents {
        verify_ok_bonus: 0,
        verify_geo_match_bonus: 0,
        site_success_bonus: 0,
        geo_mismatch_penalty: 0,
        region_mismatch_penalty: 0,
        geo_risk_penalty: 0,
        smoke_upstream_ok_bonus: 0,
        raw_score_component: 0,
        missing_verify_penalty: 0,
        stale_verify_penalty: 0,
        verify_failed_heavy_penalty: 0,
        verify_failed_light_penalty: 0,
        verify_failed_base_penalty: 0,
        individual_history_penalty: 0,
        provider_risk_penalty: 0,
        provider_region_cluster_penalty: 0,
        verify_confidence_bonus: 0,
        verify_score_delta_bonus: 0,
        verify_source_bonus: 0,
        anonymity_bonus: 0,
        latency_penalty: 0,
        exit_ip_not_public_penalty: 0,
        probe_error_penalty: 0,
        verify_risk_penalty: 0,
        site_failure_penalty: 0,
        soft_min_score_penalty: 0,
    }
}

fn component_label(key: &str) -> &'static str {
    match key {
        "verify_ok_bonus" => "verify_ok",
        "verify_geo_match_bonus" => "geo_match",
        "site_success_bonus" => "site_success",
        "geo_mismatch_penalty" => "geo_mismatch",
        "region_mismatch_penalty" => "region_mismatch",
        "geo_risk_penalty" => "geo_risk",
        "smoke_upstream_ok_bonus" => "upstream_ok",
        "raw_score_component" => "raw_score",
        "missing_verify_penalty" => "missing_verify",
        "stale_verify_penalty" => "stale_verify",
        "verify_failed_heavy_penalty" => "verify_failed_heavy",
        "verify_failed_light_penalty" => "verify_failed_light",
        "verify_failed_base_penalty" => "verify_failed_base",
        "individual_history_penalty" => "history_risk",
        "provider_risk_penalty" => "provider_risk",
        "provider_region_cluster_penalty" => "provider_region_risk",
        "verify_confidence_bonus" => "verify_confidence",
        "verify_score_delta_bonus" => "verify_score_delta",
        "verify_source_bonus" => "verify_source",
        "anonymity_bonus" => "anonymity",
        "latency_penalty" => "probe_latency",
        "exit_ip_not_public_penalty" => "exit_ip_not_public",
        "probe_error_penalty" => "probe_error_category",
        "verify_risk_penalty" => "verify_risk",
        "site_failure_penalty" => "site_failure",
        "soft_min_score_penalty" => "soft_min_score",
        _ => "unknown",
    }
}

fn trust_score_total_from_components(components: &TrustScoreComponents) -> i64 {
    let mut total = 0;
    for key in positive_component_keys() {
        total += component_value(components, key);
    }
    total += components.latency_penalty;
    for key in [
        "geo_mismatch_penalty",
        "region_mismatch_penalty",
        "geo_risk_penalty",
        "missing_verify_penalty",
        "stale_verify_penalty",
        "verify_failed_heavy_penalty",
        "verify_failed_light_penalty",
        "verify_failed_base_penalty",
        "individual_history_penalty",
        "provider_risk_penalty",
        "provider_region_cluster_penalty",
        "exit_ip_not_public_penalty",
        "probe_error_penalty",
        "verify_risk_penalty",
        "site_failure_penalty",
        "soft_min_score_penalty",
    ] {
        total -= component_value(components, key);
    }
    total
}

pub fn summarize_component_advantages(current: &TrustScoreComponents) -> String {
    let mut positives = Vec::new();
    let mut penalties = Vec::new();
    let collapse_geo_risk = current.geo_risk_penalty > 0;
    let collapse_verify_risk = current.verify_risk_penalty > 0;
    for key in positive_component_keys() {
        let value = component_value(current, key);
        if value > 0 {
            positives.push(component_label(key));
        }
    }
    for key in [
        "geo_mismatch_penalty",
        "region_mismatch_penalty",
        "geo_risk_penalty",
        "missing_verify_penalty",
        "stale_verify_penalty",
        "verify_failed_heavy_penalty",
        "verify_failed_light_penalty",
        "verify_failed_base_penalty",
        "individual_history_penalty",
        "provider_risk_penalty",
        "provider_region_cluster_penalty",
        "exit_ip_not_public_penalty",
        "probe_error_penalty",
        "verify_risk_penalty",
        "site_failure_penalty",
        "soft_min_score_penalty",
    ] {
        if collapse_geo_risk && matches!(key, "geo_mismatch_penalty" | "region_mismatch_penalty") {
            continue;
        }
        if collapse_verify_risk
            && matches!(key, "exit_ip_not_public_penalty" | "probe_error_penalty")
        {
            continue;
        }
        let value = component_value(current, key);
        if value > 0 {
            penalties.push(component_label(key));
        }
    }
    let mut parts = Vec::new();
    if !positives.is_empty() {
        parts.push(format!("wins on {}", positives.join(", ")));
    }
    if !penalties.is_empty() {
        parts.push(format!("penalized by {}", penalties.join(", ")));
    }
    if parts.is_empty() {
        "neutral component mix".to_string()
    } else {
        parts.join("; ")
    }
}

pub fn structured_component_delta(
    current: &TrustScoreComponents,
    baseline: Option<&TrustScoreComponents>,
) -> WinnerVsRunnerUpDiff {
    let keys = component_keys();
    let positive = positive_component_keys();
    let collapse_geo_risk = baseline
        .map(|b| current.geo_risk_penalty > 0 || b.geo_risk_penalty > 0)
        .unwrap_or(current.geo_risk_penalty > 0);
    let collapse_verify_risk = baseline
        .map(|b| current.verify_risk_penalty > 0 || b.verify_risk_penalty > 0)
        .unwrap_or(current.verify_risk_penalty > 0);
    let filtered_keys: Vec<&'static str> = keys
        .into_iter()
        .filter(|key| {
            !(collapse_geo_risk
                && matches!(*key, "geo_mismatch_penalty" | "region_mismatch_penalty"))
                && !(collapse_verify_risk
                    && matches!(*key, "exit_ip_not_public_penalty" | "probe_error_penalty"))
        })
        .collect();
    let winner_total_score = trust_score_total_from_components(current);

    let Some(baseline) = baseline else {
        let mut factors: Vec<WinnerVsRunnerUpFactor> = filtered_keys
            .into_iter()
            .map(|key| {
                let value = component_value(current, key);
                WinnerVsRunnerUpFactor {
                    factor: key.to_string(),
                    label: component_label(key).to_string(),
                    winner_value: value,
                    runner_up_value: value,
                    delta: 0,
                    direction: WinnerVsRunnerUpDirection::Neutral,
                }
            })
            .collect();
        factors.sort_by_key(|item| std::cmp::Reverse(item.winner_value.abs()));
        factors.truncate(5);
        return WinnerVsRunnerUpDiff {
            winner_total_score,
            runner_up_total_score: winner_total_score,
            score_gap: 0,
            factors,
        };
    };

    let runner_up_total_score = trust_score_total_from_components(baseline);
    let mut factors = Vec::new();
    for key in filtered_keys {
        let cv = component_value(current, key);
        let bv = component_value(baseline, key);
        let direction = if positive.contains(&key) {
            if cv > bv {
                WinnerVsRunnerUpDirection::Winner
            } else if cv < bv {
                WinnerVsRunnerUpDirection::RunnerUp
            } else {
                WinnerVsRunnerUpDirection::Neutral
            }
        } else if cv < bv {
            WinnerVsRunnerUpDirection::Winner
        } else if cv > bv {
            WinnerVsRunnerUpDirection::RunnerUp
        } else {
            WinnerVsRunnerUpDirection::Neutral
        };
        let delta = if positive.contains(&key) {
            cv - bv
        } else {
            bv - cv
        };
        factors.push(WinnerVsRunnerUpFactor {
            factor: key.to_string(),
            label: component_label(key).to_string(),
            winner_value: cv,
            runner_up_value: bv,
            delta,
            direction,
        });
    }
    factors.sort_by_key(|v| std::cmp::Reverse(v.delta.abs()));
    factors.truncate(5);
    WinnerVsRunnerUpDiff {
        winner_total_score,
        runner_up_total_score,
        score_gap: winner_total_score - runner_up_total_score,
        factors,
    }
}

pub fn summarize_component_delta(
    current: &TrustScoreComponents,
    baseline: Option<&TrustScoreComponents>,
) -> String {
    let current_summary = summarize_component_advantages(current);
    let Some(baseline) = baseline else {
        return current_summary;
    };

    let collapse_geo_risk = current.geo_risk_penalty > 0 || baseline.geo_risk_penalty > 0;
    let collapse_verify_risk = current.verify_risk_penalty > 0 || baseline.verify_risk_penalty > 0;
    let mut better = Vec::new();
    let mut worse = Vec::new();
    for key in component_keys() {
        if collapse_geo_risk && matches!(key, "geo_mismatch_penalty" | "region_mismatch_penalty") {
            continue;
        }
        if collapse_verify_risk
            && matches!(key, "exit_ip_not_public_penalty" | "probe_error_penalty")
        {
            continue;
        }
        let cv = component_value(current, key);
        let bv = component_value(baseline, key);
        let label = component_label(key);
        if positive_component_keys().contains(&key) {
            if cv > bv {
                better.push(label);
            } else if cv < bv {
                worse.push(label);
            }
        } else if cv < bv {
            better.push(label);
        } else if cv > bv {
            worse.push(label);
        }
    }
    let mut parts = Vec::new();
    if !better.is_empty() {
        parts.push(format!("better on {}", better.join(", ")));
    }
    if !worse.is_empty() {
        parts.push(format!("worse on {}", worse.join(", ")));
    }
    if parts.is_empty() {
        current_summary
    } else {
        format!("{}; {}", current_summary, parts.join("; "))
    }
}

fn proxy_site_score_signal_from_stats(
    tuning: &crate::network_identity::proxy_selection::ProxySelectionTuning,
    success_count: i64,
    failure_count: i64,
    last_failure_scope: Option<&str>,
    last_browser_failure_signal: Option<&str>,
) -> ProxySiteScoreSignal {
    let site_success_bonus = if success_count >= 3 && failure_count == 0 {
        tuning.site_success_bonus
    } else if success_count > failure_count && success_count >= 1 {
        (tuning.site_success_bonus / 2).max(1)
    } else {
        0
    };
    let mut site_failure_penalty = if failure_count >= success_count + 3 {
        tuning.site_failure_penalty
    } else if failure_count > success_count {
        (tuning.site_failure_penalty / 2).max(1)
    } else {
        0
    };
    if failure_count > 0
        && last_failure_scope == Some("browser_execution")
        && last_browser_failure_signal.is_some()
    {
        site_failure_penalty += tuning.site_browser_failure_penalty;
    }
    ProxySiteScoreSignal {
        site_success_bonus,
        site_failure_penalty,
    }
}

async fn load_proxy_site_score_signal(
    state: &AppState,
    proxy_id: &str,
    site_key: Option<&str>,
) -> Result<ProxySiteScoreSignal> {
    let Some(site_key) = site_key else {
        return Ok(ProxySiteScoreSignal::default());
    };
    let row = sqlx::query_as::<_, (i64, i64, Option<String>, Option<String>)>(
        r#"SELECT success_count, failure_count, last_failure_scope, last_browser_failure_signal
           FROM proxy_site_stats
           WHERE proxy_id = ? AND site_key = ?
           LIMIT 1"#,
    )
    .bind(proxy_id)
    .bind(site_key)
    .fetch_optional(&state.db)
    .await?;
    let Some((success_count, failure_count, last_failure_scope, last_browser_failure_signal)) = row
    else {
        return Ok(ProxySiteScoreSignal::default());
    };
    Ok(proxy_site_score_signal_from_stats(
        &state.proxy_selection_tuning,
        success_count,
        failure_count,
        last_failure_scope.as_deref(),
        last_browser_failure_signal.as_deref(),
    ))
}

async fn fetch_ranked_auto_selection_candidates(
    state: &AppState,
    now: &str,
    provider: Option<&str>,
    region: Option<&str>,
    min_score: f64,
    soft_min_score: Option<f64>,
    site_key: Option<&str>,
    limit: i64,
) -> Result<Vec<RankedAutoSelectionCandidate>> {
    let query = format!(
        "SELECT id, scheme, host, port, username, password, region, country, provider, score, last_used_at, created_at
         FROM proxies
         {}
         ORDER BY {}
         LIMIT ?",
        proxy_selection_base_where_sql(),
        proxy_selection_order_by_trust_score_sql_with_tuning(&state.proxy_selection_tuning)
    );
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            f64,
            Option<String>,
            String,
        ),
    >(&query)
    .bind(now)
    .bind(provider)
    .bind(provider)
    .bind(region)
    .bind(region)
    .bind(min_score)
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let mut ranked = Vec::new();
    for (
        id,
        scheme,
        host,
        port,
        username,
        password,
        region,
        country,
        provider,
        score,
        last_used_at,
        created_at,
    ) in rows
    {
        let (trust_score_total, trust_score_components) =
            compute_proxy_selection_explain(state, &id, now, soft_min_score, site_key).await?;
        ranked.push(RankedAutoSelectionCandidate {
            row: AutoSelectionCandidateRow {
                id,
                scheme,
                host,
                port,
                username,
                password,
                region,
                country,
                provider,
                score,
                last_used_at,
                created_at,
            },
            trust_score_total,
            trust_score_components,
        });
    }
    ranked.sort_by(|left, right| {
        right
            .trust_score_total
            .unwrap_or(i64::MIN)
            .cmp(&left.trust_score_total.unwrap_or(i64::MIN))
            .then_with(|| {
                left.row
                    .last_used_at
                    .as_deref()
                    .unwrap_or("0")
                    .cmp(right.row.last_used_at.as_deref().unwrap_or("0"))
            })
            .then_with(|| left.row.created_at.cmp(&right.row.created_at))
    });
    Ok(ranked)
}

pub async fn compute_candidate_preview_with_reasons(
    state: &AppState,
    now: &str,
    provider: Option<&str>,
    region: Option<&str>,
    min_score: f64,
    soft_min_score: Option<f64>,
    site_key: Option<&str>,
) -> Result<Vec<CandidateRankPreviewItem>> {
    let ranked = fetch_ranked_auto_selection_candidates(
        state,
        now,
        provider,
        region,
        min_score,
        soft_min_score,
        site_key,
        20,
    )
    .await?;
    let baseline_components = ranked
        .get(1)
        .map(|item| item.trust_score_components.clone());
    let mut out = Vec::new();
    for (idx, candidate) in ranked.into_iter().take(3).enumerate() {
        let current_components = candidate.trust_score_components.clone();
        let summary = if idx == 0 {
            summarize_component_delta(&current_components, baseline_components.as_ref())
        } else {
            summarize_component_advantages(&current_components)
        };
        let diff = if idx == 0 {
            Some(structured_component_delta(
                &current_components,
                baseline_components.as_ref(),
            ))
        } else {
            None
        };
        out.push(CandidateRankPreviewItem {
            id: candidate.row.id,
            provider: candidate.row.provider,
            region: candidate.row.region,
            score: candidate.row.score,
            trust_score_total: candidate.trust_score_total.unwrap_or_default(),
            summary,
            winner_vs_runner_up_diff: diff,
        });
    }
    Ok(out)
}

async fn compute_proxy_selection_explain(
    state: &AppState,
    proxy_id: &str,
    now: &str,
    soft_min_score: Option<f64>,
    site_key: Option<&str>,
) -> Result<(Option<i64>, TrustScoreComponents)> {
    let provider_risk_query = "SELECT EXISTS(SELECT 1 FROM provider_risk_snapshots s JOIN proxies p ON p.provider = s.provider WHERE p.id = ? AND s.risk_hit != 0)";
    let provider_region_query = "SELECT EXISTS(SELECT 1 FROM provider_region_risk_snapshots s JOIN proxies p ON p.provider = s.provider AND p.region = s.region WHERE p.id = ? AND s.risk_hit != 0)";
    let trust_score_query = format!(
        "SELECT CAST(({}) AS INTEGER) FROM proxies WHERE id = ? LIMIT 1",
        crate::network_identity::proxy_selection::proxy_trust_score_sql_with_tuning(
            &state.proxy_selection_tuning,
        )
    );
    let row = sqlx::query(
        r#"SELECT score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, CAST(last_verify_at AS INTEGER) AS last_verify_at, last_verify_confidence, last_verify_score_delta, last_verify_source, last_anonymity_level, last_probe_latency_ms, last_probe_error_category, last_exit_country, last_exit_region, country, region FROM proxies WHERE id = ?"#
    )
    .bind(proxy_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else {
        return Ok((None, empty_components()));
    };
    let score: f64 = row.try_get("score")?;
    let success_count: i64 = row.try_get("success_count")?;
    let failure_count: i64 = row.try_get("failure_count")?;
    let last_verify_status: Option<String> = row.try_get("last_verify_status")?;
    let last_verify_geo_match_ok: Option<i64> = row.try_get("last_verify_geo_match_ok")?;
    let last_smoke_upstream_ok: Option<i64> = row.try_get("last_smoke_upstream_ok")?;
    let last_verify_at: Option<i64> = row.try_get("last_verify_at")?;
    let last_verify_confidence: Option<f64> = row.try_get("last_verify_confidence")?;
    let last_verify_score_delta: Option<i64> = row.try_get("last_verify_score_delta")?;
    let last_verify_source: Option<String> = row.try_get("last_verify_source")?;
    let last_anonymity_level: Option<String> = row.try_get("last_anonymity_level")?;
    let last_probe_latency_ms: Option<i64> = row.try_get("last_probe_latency_ms")?;
    let last_probe_error_category: Option<String> = row.try_get("last_probe_error_category")?;
    let last_exit_country: Option<String> = row.try_get("last_exit_country")?;
    let last_exit_region: Option<String> = row.try_get("last_exit_region")?;
    let proxy_country: Option<String> = row.try_get("country")?;
    let proxy_region: Option<String> = row.try_get("region")?;
    let base_trust_score_total = sqlx::query_scalar::<_, i64>(&trust_score_query)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(proxy_id)
        .fetch_optional(&state.db)
        .await?;
    let provider_risk_hit: i64 = sqlx::query_scalar(provider_risk_query)
        .bind(proxy_id)
        .fetch_one(&state.db)
        .await?;
    let provider_region_cluster_hit: i64 = sqlx::query_scalar(provider_region_query)
        .bind(proxy_id)
        .fetch_one(&state.db)
        .await?;
    let region_match_ok = match (last_exit_region.as_deref(), proxy_region.as_deref()) {
        (Some(actual), Some(expected)) => Some(actual.eq_ignore_ascii_case(expected)),
        _ => None,
    };
    let geo_match_ok = match (last_exit_country.as_deref(), proxy_country.as_deref()) {
        (Some(actual), Some(expected)) => actual.eq_ignore_ascii_case(expected),
        _ => last_verify_geo_match_ok.unwrap_or(0) != 0,
    };
    let mut components = computed_trust_score_components(
        &state.proxy_selection_tuning,
        score,
        success_count,
        failure_count,
        last_verify_status.as_deref(),
        geo_match_ok,
        region_match_ok,
        last_smoke_upstream_ok.unwrap_or(0) != 0,
        last_verify_at,
        last_verify_confidence,
        last_verify_score_delta,
        last_verify_source.as_deref(),
        last_anonymity_level.as_deref(),
        last_probe_latency_ms,
        last_probe_error_category.as_deref(),
        provider_risk_hit != 0,
        provider_region_cluster_hit != 0,
        now.parse::<i64>().unwrap_or_default(),
        soft_min_score,
    );
    let site_signal = load_proxy_site_score_signal(state, proxy_id, site_key).await?;
    components.site_success_bonus = site_signal.site_success_bonus;
    components.site_failure_penalty = site_signal.site_failure_penalty;
    let trust_score_total = base_trust_score_total
        .map(|value| value + components.site_success_bonus - components.site_failure_penalty);
    Ok((trust_score_total, components))
}

async fn auto_rank_position_for_proxy(
    state: &AppState,
    now: &str,
    proxy_id: &str,
    provider: Option<&str>,
    region: Option<&str>,
    min_score: f64,
    site_key: Option<&str>,
) -> Result<Option<i64>> {
    let ranked = fetch_ranked_auto_selection_candidates(
        state, now, provider, region, min_score, None, site_key, 20,
    )
    .await?;
    Ok(ranked
        .into_iter()
        .position(|candidate| candidate.row.id == proxy_id)
        .map(|idx| idx as i64 + 1))
}

// Selection boundary note:
// - eligibility gate: active / cooldown / provider-region filter / min_score
// - ranking score: trust_score_total + trust_score_components ordering within eligible candidates
// explicit and sticky are currently control-flow overrides around the ranking path, not score components.
async fn resolve_network_policy_for_task(
    state: &AppState,
    task_kind: &str,
    payload: &mut Value,
) -> Result<RunnerSessionContext> {
    let payload_snapshot = payload.clone();
    let Some(policy) = payload.get_mut("network_policy_json") else {
        return Ok(RunnerSessionContext {
            identity_session_status: "not_applicable".to_string(),
            ..RunnerSessionContext::default()
        });
    };
    let Some(policy_obj) = policy.as_object_mut() else {
        return Ok(RunnerSessionContext {
            identity_session_status: "not_applicable".to_string(),
            ..RunnerSessionContext::default()
        });
    };
    let mode = policy_obj
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("direct")
        .to_string();
    if mode == "direct" {
        policy_obj.insert("proxy_resolution_status".to_string(), json!("direct"));
        policy_obj.insert(
            "selection_reason_summary".to_string(),
            json!("direct mode bypasses proxy pool selection"),
        );
        policy_obj.insert(
            "identity_session_status".to_string(),
            json!("not_applicable"),
        );
        policy_obj.insert("cookie_restore_count".to_string(), json!(0));
        policy_obj.insert("local_storage_restore_count".to_string(), json!(0));
        policy_obj.insert("session_storage_restore_count".to_string(), json!(0));
        return Ok(RunnerSessionContext {
            identity_session_status: "not_applicable".to_string(),
            ..RunnerSessionContext::default()
        });
    }

    let now = now_ts_string();
    let explicit_sticky_session = policy_obj
        .get("sticky_session")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let auto_session_key = if explicit_sticky_session.is_none() {
        auto_identity_session_key(task_kind, &payload_snapshot)
    } else {
        None
    };
    let effective_session_key = explicit_sticky_session
        .clone()
        .or_else(|| auto_session_key.clone());
    let provider = policy_obj
        .get("provider")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let region = policy_obj
        .get("region")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let strict_region = policy_obj
        .get("strict_region")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let min_score = policy_obj
        .get("min_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let soft_min_score = policy_obj.get("soft_min_score").and_then(|v| v.as_f64());

    let mut session_context = RunnerSessionContext {
        session_key: effective_session_key.clone(),
        site_key: task_site_key_from_payload(&payload_snapshot),
        persona_id: payload_snapshot
            .get("persona_id")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        fingerprint_profile_id: payload_snapshot
            .get("fingerprint_profile_id")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        requested_region: region.clone(),
        requested_provider: provider.clone(),
        restored_cookies: None,
        cookie_restore_count: 0,
        restored_local_storage: None,
        restored_session_storage: None,
        local_storage_restore_count: 0,
        session_storage_restore_count: 0,
        prior_proxy_id: None,
        identity_session_status: if auto_session_key.is_some() {
            "auto_created".to_string()
        } else {
            "not_applicable".to_string()
        },
        auto_session_enabled: auto_session_key.is_some(),
    };
    if let Some(session_key) = effective_session_key.as_deref() {
        policy_obj.insert("identity_session_key".to_string(), json!(session_key));
        policy_obj.insert(
            "identity_session_mode".to_string(),
            json!(if auto_session_key.is_some() {
                "auto"
            } else {
                "manual_sticky"
            }),
        );
    } else {
        policy_obj.insert("identity_session_mode".to_string(), json!("not_applicable"));
    }

    let mut selection_mode = "auto";
    let mut fallback_reason: Option<&str> = None;
    let mut sticky_binding_created_at: Option<String> = None;
    let mut row = if let Some(proxy_id) = policy_obj.get("proxy_id").and_then(|v| v.as_str()) {
        selection_mode = "explicit";
        sqlx::query_as::<_, (String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, f64)>(
            r#"SELECT id, scheme, host, port, username, password, region, country, provider, score
               FROM proxies
               WHERE id = ?
                 AND status = 'active'
                 AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
               LIMIT 1"#,
        )
        .bind(proxy_id)
        .bind(&now)
        .fetch_optional(&state.db)
        .await?
    } else if let Some(ref session_key) = effective_session_key {
        selection_mode = "sticky";
        sqlx::query_as::<_, (String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, f64, String, Option<String>, Option<String>, Option<String>)>(
            r#"SELECT p.id, p.scheme, p.host, p.port, p.username, p.password, p.region, p.country, p.provider, p.score,
                      b.created_at, b.cookies_json, b.local_storage_json, b.session_storage_json
               FROM proxy_session_bindings b
               JOIN proxies p ON p.id = b.proxy_id
               WHERE b.session_key = ?
                 AND p.status = 'active'
                 AND (b.expires_at IS NULL OR CAST(b.expires_at AS INTEGER) > CAST(? AS INTEGER))
                 AND (p.cooldown_until IS NULL OR CAST(p.cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
                 AND (? IS NULL OR p.provider = ?)
                 AND p.score >= ?
               LIMIT 1"#,
        )
        .bind(session_key)
        .bind(&now)
        .bind(&now)
        .bind(provider.as_deref())
        .bind(provider.as_deref())
        .bind(min_score)
        .fetch_optional(&state.db)
        .await?
        .map(|(id, scheme, host, port, username, password, region, country, provider, score, created_at, cookies_json, local_storage_json, session_storage_json)| {
            sticky_binding_created_at = Some(created_at);
            session_context.prior_proxy_id = Some(id.clone());
            session_context.restored_cookies = decode_cookies_json(cookies_json);
            session_context.cookie_restore_count = session_context.restored_cookies.as_ref().map(|cookies| i64::try_from(cookies.len()).unwrap_or(0)).unwrap_or(0);
            session_context.restored_local_storage = decode_storage_json(local_storage_json);
            session_context.restored_session_storage = decode_storage_json(session_storage_json);
            session_context.local_storage_restore_count = storage_entry_count(session_context.restored_local_storage.as_ref());
            session_context.session_storage_restore_count = storage_entry_count(session_context.restored_session_storage.as_ref());
            (id, scheme, host, port, username, password, region, country, provider, score)
        })
    } else {
        None
    };

    if row.is_none() && selection_mode != "explicit" {
        fallback_reason = Some(match selection_mode {
            "sticky" if auto_session_key.is_some() => {
                "identity_session_binding_missing_or_ineligible_then_fallback_to_auto"
            }
            "sticky" => "sticky_binding_missing_or_ineligible_then_fallback_to_auto",
            _ => "auto_primary_path",
        });
        selection_mode = "auto";
        row = fetch_ranked_auto_selection_candidates(
            state,
            &now,
            provider.as_deref(),
            region.as_deref(),
            min_score,
            soft_min_score,
            session_context.site_key.as_deref(),
            20,
        )
        .await?
        .into_iter()
        .next()
        .map(|candidate| {
            (
                candidate.row.id,
                candidate.row.scheme,
                candidate.row.host,
                candidate.row.port,
                candidate.row.username,
                candidate.row.password,
                candidate.row.region,
                candidate.row.country,
                candidate.row.provider,
                candidate.row.score,
            )
        });
        if row.is_none() && region.is_some() && !strict_region {
            fallback_reason = Some("region_shortage_fallback_to_any_active");
            row = fetch_ranked_auto_selection_candidates(
                state,
                &now,
                provider.as_deref(),
                None,
                min_score,
                soft_min_score,
                session_context.site_key.as_deref(),
                20,
            )
            .await?
            .into_iter()
            .next()
            .map(|candidate| {
                (
                    candidate.row.id,
                    candidate.row.scheme,
                    candidate.row.host,
                    candidate.row.port,
                    candidate.row.username,
                    candidate.row.password,
                    candidate.row.region,
                    candidate.row.country,
                    candidate.row.provider,
                    candidate.row.score,
                )
            });
        }
    }

    if let Some((
        id,
        scheme,
        host,
        port,
        username,
        password,
        resolved_region,
        country,
        resolved_provider,
        score,
    )) = row
    {
        let (trust_score_total, trust_score_components) = compute_proxy_selection_explain(
            state,
            &id,
            &now,
            soft_min_score,
            session_context.site_key.as_deref(),
        )
        .await?;
        let preview_provider = resolved_provider.clone();
        let preview_region = resolved_region.clone();
        let rank_proxy_id = id.clone();
        let rank_provider = resolved_provider.clone();
        let rank_region = resolved_region.clone();
        let selected_proxy_snapshot = RunnerProxySelection {
            id: id.clone(),
            scheme: scheme.clone(),
            host: host.clone(),
            port,
            username: username.clone(),
            password: password.clone(),
            region: resolved_region.clone(),
            country: country.clone(),
            provider: resolved_provider.clone(),
            score,
            resolution_status: policy_obj
                .get("proxy_resolution_status")
                .and_then(|value| value.as_str())
                .unwrap_or("resolved")
                .to_string(),
            source_label: policy_obj
                .get("resolved_proxy")
                .and_then(|value| value.get("source_label"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            source_tier: policy_obj
                .get("resolved_proxy")
                .and_then(|value| value.get("source_tier"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            verification_path: policy_obj
                .get("resolved_proxy")
                .and_then(|value| value.get("verification_path"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            last_verify_source: policy_obj
                .get("resolved_proxy")
                .and_then(|value| value.get("last_verify_source"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            last_exit_country: policy_obj
                .get("resolved_proxy")
                .and_then(|value| value.get("last_exit_country"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
            last_exit_region: policy_obj
                .get("resolved_proxy")
                .and_then(|value| value.get("last_exit_region"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string()),
        };
        let mut resolved = resolved_proxy_json(
            id.clone(),
            scheme,
            host,
            port,
            username,
            password,
            resolved_region.clone(),
            country,
            resolved_provider.clone(),
            score,
        );
        if let Some(obj) = resolved.as_object_mut() {
            obj.insert(
                "trust_score_total".to_string(),
                trust_score_total.map_or(Value::Null, |v| json!(v)),
            );
            obj.insert(
                "trust_score_components".to_string(),
                json!(trust_score_components.clone()),
            );
        }
        apply_proxy_resolution_metadata(
            policy_obj,
            effective_session_key.as_deref(),
            Some(resolved),
        );
        let preview = if selection_mode == "auto" {
            Some(
                compute_candidate_preview_with_reasons(
                    state,
                    &now,
                    preview_provider.as_deref(),
                    preview_region.as_deref(),
                    min_score,
                    soft_min_score,
                    session_context.site_key.as_deref(),
                )
                .await?,
            )
        } else {
            None
        };
        let would_rank_position_if_auto =
            if selection_mode == "explicit" || selection_mode == "sticky" {
                auto_rank_position_for_proxy(
                    state,
                    &now,
                    &rank_proxy_id,
                    rank_provider.as_deref(),
                    rank_region.as_deref(),
                    min_score,
                    session_context.site_key.as_deref(),
                )
                .await?
            } else {
                None
            };
        let sticky_binding_age_seconds = if selection_mode == "sticky" {
            match sticky_binding_created_at.as_deref() {
                Some(created_at) => {
                    let now_i = now.parse::<i64>().unwrap_or(0);
                    let created_i = created_at.parse::<i64>().unwrap_or(now_i);
                    Some(now_i.saturating_sub(created_i))
                }
                None => effective_session_key.as_deref().map(|_| 0),
            }
        } else {
            None
        };
        let sticky_reuse_reason = if selection_mode == "sticky" {
            Some("sticky_binding_matched_and_candidate_still_eligible")
        } else {
            None
        };
        let sticky_binding_trust_score_total = if selection_mode == "sticky" {
            trust_score_total
        } else {
            None
        };
        let soft_min_score_penalty_applied = soft_min_score.map(|threshold| score < threshold);
        let candidate_summary = preview
            .as_ref()
            .and_then(|items| items.first())
            .map(|item| item.summary.as_str());
        let proxy_growth_for_selection = build_proxy_growth_explain_json(
            state,
            &payload_snapshot,
            Some(&selected_proxy_snapshot),
        )
        .await
        .ok();
        let requested_fp_budget = payload_snapshot
            .get("fingerprint_profile_json")
            .and_then(|v| v.as_str())
            .map(
                |v| match fingerprint_perf_budget_tag_from_profile_json(Some(v)) {
                    FingerprintPerfBudgetTag::Light => "light",
                    FingerprintPerfBudgetTag::Medium => "medium",
                    FingerprintPerfBudgetTag::Heavy => "heavy",
                },
            );
        let mut selection_summary = selection_reason_summary_for_mode(
            selection_mode,
            sticky_binding_trust_score_total.or(trust_score_total),
            candidate_summary,
        );
        if fallback_reason == Some("region_shortage_fallback_to_any_active") {
            selection_summary.push_str("; requested region had no eligible active proxy so selection fell back to another active region");
        }
        if session_context.auto_session_enabled {
            session_context.identity_session_status = if session_context.prior_proxy_id.as_deref()
                == Some(id.as_str())
            {
                "auto_reused".to_string()
            } else if session_context.prior_proxy_id.is_some() {
                match (
                    session_context.requested_region.as_deref(),
                    resolved_region.as_deref(),
                ) {
                    (Some(requested), Some(actual)) if !requested.eq_ignore_ascii_case(actual) => {
                        "auto_fallback_rebound".to_string()
                    }
                    (Some(_), None) => "auto_fallback_rebound".to_string(),
                    _ => "auto_rebound".to_string(),
                }
            } else {
                "auto_created".to_string()
            };
        }
        policy_obj.insert(
            "selection_reason_summary".to_string(),
            json!(selection_summary),
        );
        policy_obj.insert(
            "selection_explain".to_string(),
            selection_explain_json(
                selection_mode,
                fallback_reason,
                None,
                sticky_binding_age_seconds,
                sticky_reuse_reason,
                would_rank_position_if_auto,
                soft_min_score,
                soft_min_score_penalty_applied,
                proxy_growth_for_selection,
                requested_fp_budget,
                Some(medium_budget_limit(state.worker_count)),
                Some(heavy_budget_limit(state.worker_count)),
            ),
        );
        policy_obj.insert(
            "trust_score_components".to_string(),
            json!(trust_score_components),
        );
        if let Some(preview) = preview {
            policy_obj.insert("candidate_rank_preview".to_string(), json!(preview));
        }
        if let Some(score) = trust_score_total {
            policy_obj.insert("trust_score_total".to_string(), json!(score));
        }
    } else {
        apply_proxy_resolution_metadata(policy_obj, effective_session_key.as_deref(), None);
        let no_match_reason_code = if mode == "direct" {
            Some("direct_mode")
        } else if policy_obj
            .get("proxy_id")
            .and_then(|v| v.as_str())
            .is_some()
        {
            Some("explicit_proxy_missing_or_ineligible")
        } else if effective_session_key.is_some() && auto_session_key.is_some() {
            Some("identity_session_binding_missing_or_ineligible")
        } else if explicit_sticky_session.is_some() {
            Some("sticky_binding_missing_or_ineligible")
        } else if strict_region && region.is_some() {
            Some("no_match_after_strict_region_filter")
        } else if min_score > 0.0 {
            Some("no_match_after_min_score_filter")
        } else if provider.is_some() && region.is_some() {
            Some("no_match_after_provider_region_filters")
        } else if provider.is_some() {
            Some("no_match_after_provider_filter")
        } else if region.is_some() {
            Some("no_match_after_region_filter")
        } else {
            Some("no_eligible_active_proxy")
        };
        policy_obj.insert(
            "selection_reason_summary".to_string(),
            json!("no eligible active proxy matched the current policy filters"),
        );
        let proxy_growth_for_selection =
            build_proxy_growth_explain_json(state, &payload_snapshot, None)
                .await
                .ok();
        let requested_fp_budget = payload_snapshot
            .get("fingerprint_profile_json")
            .and_then(|v| v.as_str())
            .map(
                |v| match fingerprint_perf_budget_tag_from_profile_json(Some(v)) {
                    FingerprintPerfBudgetTag::Light => "light",
                    FingerprintPerfBudgetTag::Medium => "medium",
                    FingerprintPerfBudgetTag::Heavy => "heavy",
                },
            );
        policy_obj.insert(
            "selection_explain".to_string(),
            selection_explain_json(
                selection_mode,
                fallback_reason,
                no_match_reason_code,
                None,
                None,
                None,
                soft_min_score,
                None,
                proxy_growth_for_selection,
                requested_fp_budget,
                Some(medium_budget_limit(state.worker_count)),
                Some(heavy_budget_limit(state.worker_count)),
            ),
        );
    }
    policy_obj.insert(
        "identity_session_status".to_string(),
        json!(session_context.identity_session_status.clone()),
    );
    policy_obj.insert(
        "cookie_restore_count".to_string(),
        json!(session_context.cookie_restore_count),
    );
    policy_obj.insert(
        "local_storage_restore_count".to_string(),
        json!(session_context.local_storage_restore_count),
    );
    policy_obj.insert(
        "session_storage_restore_count".to_string(),
        json!(session_context.session_storage_restore_count),
    );
    Ok(session_context)
}

async fn upsert_proxy_session_binding(
    state: &AppState,
    session_context: &RunnerSessionContext,
    proxy: Option<&RunnerProxySelection>,
    execution_status: RunnerOutcomeStatus,
    session_cookies: Option<&[Value]>,
    session_local_storage: Option<&Value>,
    session_session_storage: Option<&Value>,
) -> Result<()> {
    let Some(session_key) = session_context.session_key.as_deref() else {
        return Ok(());
    };
    let Some(proxy) = proxy else {
        return Ok(());
    };

    let now = now_ts_string();
    let expires_at = (now.parse::<u64>().unwrap_or(0) + 86400).to_string();
    let cookies_json = session_cookies
        .map(|cookies| Value::Array(cookies.to_vec()))
        .map(|value| value.to_string());
    let cookie_updated_at = cookies_json.as_ref().map(|_| now.clone());
    let local_storage_json = session_local_storage
        .filter(|value| value.is_object())
        .map(Value::to_string);
    let session_storage_json = session_session_storage
        .filter(|value| value.is_object())
        .map(Value::to_string);
    let storage_updated_at = if local_storage_json.is_some() || session_storage_json.is_some() {
        Some(now.clone())
    } else {
        None
    };
    let last_success_at =
        matches!(execution_status, RunnerOutcomeStatus::Succeeded).then(|| now.clone());
    let last_failure_at = matches!(
        execution_status,
        RunnerOutcomeStatus::Failed
            | RunnerOutcomeStatus::TimedOut
            | RunnerOutcomeStatus::Cancelled
    )
    .then(|| now.clone());
    sqlx::query(
        r#"INSERT INTO proxy_session_bindings (
               session_key, proxy_id, provider, region, persona_id, fingerprint_profile_id, site_key,
               requested_region, requested_provider, cookies_json, cookie_updated_at,
               local_storage_json, session_storage_json, storage_updated_at,
               last_success_at, last_failure_at, last_used_at, expires_at, created_at, updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(session_key) DO UPDATE SET
             proxy_id = excluded.proxy_id,
             provider = excluded.provider,
             region = excluded.region,
             persona_id = excluded.persona_id,
             fingerprint_profile_id = excluded.fingerprint_profile_id,
             site_key = excluded.site_key,
             requested_region = excluded.requested_region,
             requested_provider = excluded.requested_provider,
             cookies_json = COALESCE(excluded.cookies_json, proxy_session_bindings.cookies_json),
             cookie_updated_at = COALESCE(excluded.cookie_updated_at, proxy_session_bindings.cookie_updated_at),
             local_storage_json = COALESCE(excluded.local_storage_json, proxy_session_bindings.local_storage_json),
             session_storage_json = COALESCE(excluded.session_storage_json, proxy_session_bindings.session_storage_json),
             storage_updated_at = COALESCE(excluded.storage_updated_at, proxy_session_bindings.storage_updated_at),
             last_success_at = COALESCE(excluded.last_success_at, proxy_session_bindings.last_success_at),
             last_failure_at = COALESCE(excluded.last_failure_at, proxy_session_bindings.last_failure_at),
             last_used_at = excluded.last_used_at,
             expires_at = excluded.expires_at,
             updated_at = excluded.updated_at"#,
    )
    .bind(session_key)
    .bind(&proxy.id)
    .bind(session_context.requested_provider.as_ref().or(proxy.provider.as_ref()))
    .bind(session_context.requested_region.as_ref().or(proxy.region.as_ref()))
    .bind(&session_context.persona_id)
    .bind(&session_context.fingerprint_profile_id)
    .bind(&session_context.site_key)
    .bind(&session_context.requested_region)
    .bind(&session_context.requested_provider)
    .bind(&cookies_json)
    .bind(&cookie_updated_at)
    .bind(&local_storage_json)
    .bind(&session_storage_json)
    .bind(&storage_updated_at)
    .bind(&last_success_at)
    .bind(&last_failure_at)
    .bind(&now)
    .bind(&expires_at)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await?;
    Ok(())
}

async fn build_proxy_growth_explain_json(
    state: &AppState,
    task_payload: &Value,
    selected_proxy: Option<&RunnerProxySelection>,
) -> Result<Value> {
    let target_region = task_payload
        .get("target_region")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            task_payload
                .get("region")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            task_payload
                .get("network_policy_json")
                .and_then(|v| v.get("region"))
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(ToOwned::to_owned)
        });

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM proxies")
        .fetch_one(&state.db)
        .await?;
    let available: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proxies WHERE status = 'active' AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(strftime('%s','now') AS INTEGER))",
    )
    .fetch_one(&state.db)
    .await?;

    let available_in_region: i64 = match target_region.as_deref() {
        Some(region) => sqlx::query_scalar(
            "SELECT COUNT(*) FROM proxies WHERE status = 'active' AND region = ? AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(strftime('%s','now') AS INTEGER))",
        )
        .bind(region)
        .fetch_one(&state.db)
        .await?,
        None => 0,
    };

    let inflight_tasks: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE status IN ('queued', 'running')")
            .fetch_one(&state.db)
            .await?;

    let snapshot = ProxyPoolInventorySnapshot {
        total,
        available,
        region: target_region.clone(),
        available_in_region,
        inflight_tasks,
    };
    let policy = proxy_pool_growth_policy_from_env();
    let health = assess_proxy_pool_health(&snapshot, &policy);
    let region_match = evaluate_region_match(
        target_region.as_deref(),
        selected_proxy.and_then(|proxy| proxy.region.as_deref()),
    );

    Ok(json!({
        "target_region": target_region,
        "selected_proxy_region": selected_proxy.and_then(|proxy| proxy.region.clone()),
        "inventory_snapshot": snapshot,
        "health_assessment": health,
        "region_match": region_match,
    }))
}

fn execution_failure_evidence(result_json: Option<&Value>) -> bool {
    let Some(parsed) = result_json else {
        return false;
    };
    let failure_scope = parsed.get("failure_scope").and_then(|v| v.as_str());
    let execution_stage = parsed.get("execution_stage").and_then(|v| v.as_str());
    matches!(
        (failure_scope, execution_stage),
        (Some("runner_process_exit"), Some(_))
            | (Some("browser_execution"), Some(_))
            | (Some("runner_timeout"), Some(_))
    )
}

fn execution_failure_metadata(result_json: Option<&Value>) -> (Option<String>, Option<String>) {
    let Some(parsed) = result_json else {
        return (None, None);
    };
    (
        parsed
            .get("failure_scope")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        parsed
            .get("browser_failure_signal")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
    )
}

pub async fn update_proxy_health_after_execution(
    state: &AppState,
    proxy: Option<&RunnerProxySelection>,
    execution_status: RunnerOutcomeStatus,
    result_json: Option<&Value>,
) -> Result<()> {
    let Some(proxy) = proxy else {
        return Ok(());
    };
    let now = now_ts_string();
    let has_failure_evidence = execution_failure_evidence(result_json);
    let (success_inc, failure_inc, cooldown_until, score_delta): (i64, i64, Option<String>, f64) =
        match execution_status {
            RunnerOutcomeStatus::Succeeded => (1, 0, None, 0.01_f64),
            RunnerOutcomeStatus::Cancelled => (0, 0, None, 0.0_f64),
            RunnerOutcomeStatus::Failed => {
                if has_failure_evidence {
                    (
                        0,
                        1,
                        Some((now.parse::<u64>().unwrap_or(0) + 60).to_string()),
                        -0.02_f64,
                    )
                } else {
                    (0, 0, None, 0.0_f64)
                }
            }
            RunnerOutcomeStatus::TimedOut => {
                if has_failure_evidence {
                    (
                        0,
                        1,
                        Some((now.parse::<u64>().unwrap_or(0) + 180).to_string()),
                        -0.03_f64,
                    )
                } else {
                    (0, 0, None, 0.0_f64)
                }
            }
        };
    sqlx::query(r#"UPDATE proxies SET success_count = success_count + ?, failure_count = failure_count + ?, last_used_at = ?, last_checked_at = ?, cooldown_until = ?, score = MAX(0.0, score + ?), updated_at = ? WHERE id = ?"#)
        .bind(success_inc).bind(failure_inc).bind(&now).bind(&now).bind(&cooldown_until)
        .bind(score_delta)
        .bind(&now).bind(&proxy.id)
        .execute(&state.db).await?;
    refresh_proxy_trust_views_for_scope(
        &state.db,
        &proxy.id,
        proxy.provider.as_deref(),
        proxy.region.as_deref(),
    )
    .await?;
    Ok(())
}

async fn update_proxy_site_stats_after_execution(
    state: &AppState,
    session_context: &RunnerSessionContext,
    proxy: Option<&RunnerProxySelection>,
    execution_status: RunnerOutcomeStatus,
    result_json: Option<&Value>,
) -> Result<()> {
    let Some(proxy) = proxy else {
        return Ok(());
    };
    let Some(site_key) = session_context.site_key.as_deref() else {
        return Ok(());
    };

    let now = now_ts_string();
    let success_increment = matches!(execution_status, RunnerOutcomeStatus::Succeeded) as i64;
    let failure_increment = matches!(
        execution_status,
        RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::TimedOut
    ) as i64;
    let (last_failure_scope, last_browser_failure_signal) = execution_failure_metadata(result_json);
    let last_success_at = (success_increment > 0).then(|| now.clone());
    let last_failure_at = (failure_increment > 0).then(|| now.clone());

    sqlx::query(
        r#"INSERT INTO proxy_site_stats (
               proxy_id, site_key, success_count, failure_count,
               last_success_at, last_failure_at, last_failure_scope,
               last_browser_failure_signal, updated_at
           )
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(proxy_id, site_key) DO UPDATE SET
             success_count = proxy_site_stats.success_count + excluded.success_count,
             failure_count = proxy_site_stats.failure_count + excluded.failure_count,
             last_success_at = COALESCE(excluded.last_success_at, proxy_site_stats.last_success_at),
             last_failure_at = COALESCE(excluded.last_failure_at, proxy_site_stats.last_failure_at),
             last_failure_scope = COALESCE(excluded.last_failure_scope, proxy_site_stats.last_failure_scope),
             last_browser_failure_signal = COALESCE(excluded.last_browser_failure_signal, proxy_site_stats.last_browser_failure_signal),
             updated_at = excluded.updated_at"#,
    )
    .bind(&proxy.id)
    .bind(site_key)
    .bind(success_increment)
    .bind(failure_increment)
    .bind(&last_success_at)
    .bind(&last_failure_at)
    .bind(&last_failure_scope)
    .bind(&last_browser_failure_signal)
    .bind(&now)
    .execute(&state.db)
    .await?;
    Ok(())
}

struct ClaimedTask {
    task_id: String,
    task_kind: String,
    input_json: String,
    execution_intent: Option<RunnerExecutionIntent>,
    fingerprint_profile: Option<RunnerFingerprintProfile>,
    requested_fingerprint_profile_id: Option<String>,
    requested_fingerprint_profile_version: Option<i64>,
    requested_behavior_profile_id: Option<String>,
    requested_behavior_profile_version: Option<i64>,
    behavior_profile: Option<RunnerBehaviorProfile>,
    behavior_plan: Option<RunnerBehaviorPlan>,
    form_action_plan: Option<RunnerFormActionPlan>,
    form_action_mode: Option<String>,
    form_action_summary_json: Option<Value>,
    behavior_runtime_explain: Option<BehaviorRuntimeExplain>,
    behavior_trace_summary: Option<BehaviorTraceSummary>,
    behavior_site_key: Option<String>,
    attempt: i64,
    run_id: String,
    started_at: String,
}

async fn claim_next_task<R>(
    state: &AppState,
    runner: &R,
    worker_label: &str,
) -> Result<Option<ClaimedTask>>
where
    R: TaskRunner + ?Sized,
{
    for _ in 0..runner_claim_retry_limit_from_env() {
        let started_at = now_ts_string();
        let run_id = format!("run-{}", Uuid::new_v4());

        let mut tx = state.db.begin().await?;
        let candidates = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Option<String>,
                Option<i64>,
                Option<String>,
            ),
        >(
            r#"
            SELECT id, kind, input_json, fingerprint_profile_id, fingerprint_profile_version,
                (
                    SELECT fp.profile_json
                    FROM fingerprint_profiles fp
                    WHERE fp.id = tasks.fingerprint_profile_id
                      AND fp.status = 'active'
                      AND fp.version = tasks.fingerprint_profile_version
                ) as profile_json
            FROM tasks
            WHERE status = ?
            ORDER BY priority DESC, COALESCE(queued_at, created_at) ASC, created_at ASC
            LIMIT ?
            "#,
        )
        .bind(TASK_STATUS_QUEUED)
        .bind(claim_candidate_scan_limit())
        .fetch_all(&mut *tx)
        .await?;

        if candidates.is_empty() {
            tx.rollback().await?;
            return Ok(None);
        }

        let running_profiles = sqlx::query_scalar::<_, Option<String>>(
            r#"
            SELECT (
                SELECT fp.profile_json
                FROM fingerprint_profiles fp
                WHERE fp.id = tasks.fingerprint_profile_id
                  AND fp.status = 'active'
                  AND fp.version = tasks.fingerprint_profile_version
            ) as profile_json
            FROM tasks
            WHERE status = ?
            "#,
        )
        .bind(TASK_STATUS_RUNNING)
        .fetch_all(&mut *tx)
        .await?;

        let running_medium = running_profiles
            .iter()
            .filter(|profile_json| {
                fingerprint_perf_budget_tag_from_profile_json(profile_json.as_deref())
                    == FingerprintPerfBudgetTag::Medium
            })
            .count();
        let running_heavy = running_profiles
            .iter()
            .filter(|profile_json| {
                fingerprint_perf_budget_tag_from_profile_json(profile_json.as_deref())
                    == FingerprintPerfBudgetTag::Heavy
            })
            .count();

        let medium_limit = medium_budget_limit(state.worker_count);
        let heavy_limit = heavy_budget_limit(state.worker_count);

        let candidate_profile_jsons = candidates
            .iter()
            .map(|candidate| candidate.5.clone())
            .collect::<Vec<_>>();
        let Some(selected_idx) = pick_claim_candidate_index(
            &candidate_profile_jsons,
            ClaimBudgetSnapshot {
                running_medium,
                running_heavy,
                medium_limit,
                heavy_limit,
            },
        ) else {
            tx.rollback().await?;
            return Ok(None);
        };

        let (
            selected_id,
            _selected_kind,
            _selected_input_json,
            _selected_fp_id,
            _selected_fp_version,
            _selected_profile_json,
        ) = candidates
            .into_iter()
            .nth(selected_idx)
            .expect("selected candidate by index");

        let claimed = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                Option<String>,
                Option<i64>,
                Option<String>,
                Option<String>,
                Option<i64>,
                Option<String>,
            ),
        >(
            r#"
            UPDATE tasks
            SET status = ?, started_at = ?, runner_id = ?, heartbeat_at = ?
            WHERE id = ?
              AND status = ?
            RETURNING id, kind, input_json, fingerprint_profile_id, fingerprint_profile_version,
                execution_intent_json, behavior_profile_id, behavior_profile_version,
                (
                    SELECT fp.profile_json
                    FROM fingerprint_profiles fp
                    WHERE fp.id = tasks.fingerprint_profile_id
                      AND fp.status = 'active'
                      AND fp.version = tasks.fingerprint_profile_version
                ) as profile_json
            "#,
        )
        .bind(TASK_STATUS_RUNNING)
        .bind(&started_at)
        .bind(worker_label)
        .bind(&started_at)
        .bind(&selected_id)
        .bind(TASK_STATUS_QUEUED)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((
            task_id,
            task_kind,
            input_json,
            fingerprint_profile_id,
            fingerprint_profile_version,
            execution_intent_json,
            behavior_profile_id,
            behavior_profile_version,
            fingerprint_profile_json,
        )) = claimed
        else {
            tx.rollback().await?;
            continue;
        };

        let requested_fingerprint_profile_id = fingerprint_profile_id.clone();
        let requested_fingerprint_profile_version = fingerprint_profile_version;
        let requested_behavior_profile_id = behavior_profile_id.clone();
        let requested_behavior_profile_version = behavior_profile_version;

        let attempt =
            sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM runs WHERE task_id = ?"#)
                .bind(&task_id)
                .fetch_one(&mut *tx)
                .await?
                + 1;

        sqlx::query(
            r#"
            INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message, result_json)
            VALUES (?, ?, ?, ?, ?, ?, NULL, NULL, NULL)
            "#,
        )
        .bind(&run_id)
        .bind(&task_id)
        .bind(RUN_STATUS_RUNNING)
        .bind(attempt)
        .bind(runner.name())
        .bind(&started_at)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let fingerprint_profile = match (
            fingerprint_profile_id,
            fingerprint_profile_version,
            fingerprint_profile_json,
        ) {
            (Some(id), Some(version), Some(profile_json)) => serde_json::from_str(&profile_json)
                .ok()
                .map(|profile_json| RunnerFingerprintProfile {
                    id,
                    version,
                    profile_json,
                }),
            _ => None,
        };
        let behavior_context =
            load_claimed_behavior_context(state, &task_id, execution_intent_json.as_deref())
                .await?;

        return Ok(Some(ClaimedTask {
            task_id,
            task_kind,
            input_json,
            execution_intent: behavior_context.execution_intent,
            fingerprint_profile,
            requested_fingerprint_profile_id,
            requested_fingerprint_profile_version,
            requested_behavior_profile_id,
            requested_behavior_profile_version,
            behavior_profile: behavior_context.behavior_profile,
            behavior_plan: behavior_context.behavior_plan,
            form_action_plan: behavior_context.form_action_plan,
            form_action_mode: behavior_context.form_action_mode,
            form_action_summary_json: behavior_context.form_action_summary_json,
            behavior_runtime_explain: behavior_context.behavior_runtime_explain,
            behavior_trace_summary: behavior_context.behavior_trace_summary,
            behavior_site_key: behavior_context.site_key,
            attempt,
            run_id,
            started_at,
        }));
    }

    Ok(None)
}

pub async fn reclaim_stale_running_tasks(
    state: &AppState,
    stale_after_seconds: u64,
) -> Result<u64> {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let threshold = now_secs.saturating_sub(stale_after_seconds);
    let queued_at = now_ts_string();

    let task_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM tasks
        WHERE status = ?
          AND started_at IS NOT NULL
          AND runner_id IS NOT NULL
          AND CAST(COALESCE(heartbeat_at, started_at) AS INTEGER) <= ?
        "#,
    )
    .bind(TASK_STATUS_RUNNING)
    .bind(threshold as i64)
    .fetch_all(&state.db)
    .await?;

    let mut reclaimed = 0_u64;
    for task_id in task_ids {
        let update = sqlx::query(
            r#"
            UPDATE tasks
            SET status = ?, queued_at = ?, started_at = NULL, finished_at = NULL, runner_id = NULL, heartbeat_at = NULL, error_message = NULL
            WHERE id = ? AND status = ? AND runner_id IS NOT NULL
            "#,
        )
        .bind(TASK_STATUS_QUEUED)
        .bind(&queued_at)
        .bind(&task_id)
        .bind(TASK_STATUS_RUNNING)
        .execute(&state.db)
        .await?;

        if update.rows_affected() == 0 {
            continue;
        }

        sqlx::query(
            r#"UPDATE runs SET status = ?, finished_at = ?, error_message = ? WHERE task_id = ? AND status = ?"#,
        )
        .bind(RUN_STATUS_FAILED)
        .bind(&queued_at)
        .bind("reclaimed after stale running timeout")
        .bind(&task_id)
        .bind(RUN_STATUS_RUNNING)
        .execute(&state.db)
        .await?;

        insert_log(
            state,
            &format!("log-{}", Uuid::new_v4()),
            &task_id,
            None,
            "warn",
            "stale running task reclaimed back to queued",
        )
        .await?;

        reclaimed += 1;
    }

    Ok(reclaimed)
}

fn spawn_task_heartbeat(
    state: AppState,
    task_id: String,
    worker_label: String,
) -> (oneshot::Sender<()>, JoinHandle<()>) {
    let heartbeat_interval_seconds = runner_heartbeat_interval_seconds_from_env();
    let (stop_tx, mut stop_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(Duration::from_secs(heartbeat_interval_seconds)) => {
                    let heartbeat_at = now_ts_string();
                    let _ = sqlx::query(
                        r#"UPDATE tasks SET heartbeat_at = ? WHERE id = ? AND status = ? AND runner_id = ?"#,
                    )
                    .bind(&heartbeat_at)
                    .bind(&task_id)
                    .bind(TASK_STATUS_RUNNING)
                    .bind(&worker_label)
                    .execute(&state.db)
                    .await;
                }
            }
        }
    });
    (stop_tx, handle)
}

#[derive(Debug, Clone, Default)]
struct ClaimedBehaviorContext {
    execution_intent: Option<RunnerExecutionIntent>,
    behavior_profile: Option<RunnerBehaviorProfile>,
    behavior_plan: Option<RunnerBehaviorPlan>,
    form_action_plan: Option<RunnerFormActionPlan>,
    form_action_mode: Option<String>,
    form_action_summary_json: Option<Value>,
    behavior_runtime_explain: Option<BehaviorRuntimeExplain>,
    behavior_trace_summary: Option<BehaviorTraceSummary>,
    site_key: Option<String>,
}

async fn load_claimed_behavior_context(
    state: &AppState,
    task_id: &str,
    execution_intent_json: Option<&str>,
) -> Result<ClaimedBehaviorContext> {
    let execution_intent = execution_intent_json
        .and_then(|raw| serde_json::from_str::<RunnerExecutionIntent>(raw).ok());
    let metadata_json = sqlx::query_scalar::<_, String>(
        r#"SELECT metadata_json
           FROM artifacts
           WHERE task_id = ? AND kind = 'behavior_plan'
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .bind(task_id)
    .fetch_optional(&state.db)
    .await?;
    let form_metadata_json = sqlx::query_scalar::<_, String>(
        r#"SELECT metadata_json
           FROM artifacts
           WHERE task_id = ? AND kind = 'form_action_plan'
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .bind(task_id)
    .fetch_optional(&state.db)
    .await?;
    let parsed = metadata_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    let form_parsed = form_metadata_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    Ok(ClaimedBehaviorContext {
        execution_intent,
        behavior_profile: parsed
            .as_ref()
            .and_then(|value| value.get("behavior_profile").cloned())
            .and_then(|value| serde_json::from_value(value).ok()),
        behavior_plan: parsed
            .as_ref()
            .and_then(|value| value.get("behavior_plan").cloned())
            .and_then(|value| serde_json::from_value(value).ok()),
        form_action_plan: form_parsed
            .as_ref()
            .and_then(|value| value.get("form_action_plan").cloned())
            .and_then(|value| serde_json::from_value(value).ok()),
        form_action_mode: form_parsed
            .as_ref()
            .and_then(|value| value.get("form_action_mode"))
            .and_then(Value::as_str)
            .map(str::to_string),
        form_action_summary_json: form_parsed
            .as_ref()
            .and_then(|value| value.get("form_action_summary_json").cloned()),
        behavior_runtime_explain: parsed
            .as_ref()
            .and_then(|value| value.get("behavior_runtime_explain").cloned())
            .and_then(|value| serde_json::from_value(value).ok()),
        behavior_trace_summary: parsed
            .as_ref()
            .and_then(|value| value.get("behavior_trace_summary").cloned())
            .and_then(|value| serde_json::from_value(value).ok()),
        site_key: parsed
            .as_ref()
            .and_then(|value| value.get("site_key"))
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

fn behavior_mode_from_payload(payload: &Value) -> Option<String> {
    payload
        .get("behavior_policy_json")
        .and_then(|value| value.get("mode"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
}

fn behavior_seed_from_payload(
    payload: &Value,
    behavior_plan: Option<&RunnerBehaviorPlan>,
) -> Option<String> {
    behavior_plan.map(|plan| plan.seed.clone()).or_else(|| {
        payload
            .get("behavior_policy_json")
            .and_then(|value| value.get("plan_seed"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
    })
}

fn planned_behavior_steps(behavior_plan: Option<&RunnerBehaviorPlan>) -> i64 {
    behavior_plan
        .and_then(|plan| plan.steps_json.as_array())
        .map(|steps| i64::try_from(steps.len()).unwrap_or(0))
        .unwrap_or(0)
}

fn extract_runner_behavior_runtime_explain(
    result_json: Option<&Value>,
) -> Option<BehaviorRuntimeExplain> {
    result_json
        .and_then(|value| value.get("behavior_runtime_explain").cloned())
        .and_then(|value| serde_json::from_value(value).ok())
}

fn extract_runner_behavior_trace_summary(
    result_json: Option<&Value>,
) -> Option<BehaviorTraceSummary> {
    result_json
        .and_then(|value| value.get("behavior_trace_summary").cloned())
        .and_then(|value| serde_json::from_value(value).ok())
}

fn extract_runner_behavior_trace_lines(result_json: Option<&Value>) -> Option<Vec<String>> {
    result_json
        .and_then(|value| value.get("behavior_trace_lines"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
}

fn extract_runner_form_action_status(result_json: Option<&Value>) -> Option<String> {
    result_json
        .and_then(|value| value.get("form_action_status"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_runner_form_action_mode(result_json: Option<&Value>) -> Option<String> {
    result_json
        .and_then(|value| value.get("form_action_mode"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_runner_form_action_retry_count(result_json: Option<&Value>) -> Option<i64> {
    result_json
        .and_then(|value| value.get("form_action_retry_count"))
        .and_then(Value::as_i64)
}

fn extract_runner_form_action_summary(result_json: Option<&Value>) -> Option<Value> {
    result_json.and_then(|value| value.get("form_action_summary_json").cloned())
}

fn form_action_preflight_failure(
    plan: &RunnerFormActionPlan,
    status: &str,
    message: String,
) -> RunnerExecutionResult {
    let summary = build_form_action_summary_json(
        plan,
        status,
        0,
        plan.blocked_reason.as_deref(),
        Some(message.as_str()),
    );
    RunnerExecutionResult {
        status: RunnerOutcomeStatus::Failed,
        result_json: Some(json!({
            "status": "failed",
            "message": message.clone(),
            "form_action_status": status,
            "form_action_mode": plan.mode,
            "form_action_retry_count": 0,
            "form_action_summary_json": summary,
        })),
        error_message: Some(message.clone()),
        summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
            category: crate::runner::types::SummaryArtifactCategory::Execution,
            key: "form_action.preflight".to_string(),
            source: "runner.form".to_string(),
            severity: crate::runner::types::SummaryArtifactSeverity::Error,
            title: "form action preflight failure".to_string(),
            summary: message,
        }],
        session_cookies: None,
        session_local_storage: None,
        session_session_storage: None,
    }
}

fn build_behavior_trace_summary_for_execution(
    mode: Option<&str>,
    behavior_plan: Option<&RunnerBehaviorPlan>,
    base_summary: Option<&BehaviorTraceSummary>,
    preserve_execution_counts: bool,
    execution_status: RunnerOutcomeStatus,
    session_persisted: bool,
) -> BehaviorTraceSummary {
    let mut summary = base_summary.cloned().unwrap_or(BehaviorTraceSummary {
        planned_steps: planned_behavior_steps(behavior_plan),
        executed_steps: 0,
        failed_steps: 0,
        aborted: false,
        abort_reason: None,
        session_persisted: false,
        raw_trace_persisted: false,
        total_added_latency_ms: 0,
    });

    if matches!(mode, Some("active")) && preserve_execution_counts {
    } else if matches!(mode, Some("active")) {
        summary.executed_steps = planned_behavior_steps(behavior_plan);
    } else {
        summary.executed_steps = 0;
    }
    summary.session_persisted = summary.session_persisted || session_persisted;

    match execution_status {
        RunnerOutcomeStatus::Succeeded => {}
        RunnerOutcomeStatus::Failed => {
            summary.failed_steps = summary.failed_steps.max(1);
        }
        RunnerOutcomeStatus::Cancelled => {
            summary.aborted = true;
            if summary.abort_reason.is_none() {
                summary.abort_reason = Some("cancelled".to_string());
            }
        }
        RunnerOutcomeStatus::TimedOut => {
            summary.aborted = true;
            if summary.abort_reason.is_none() {
                summary.abort_reason = Some("timeout".to_string());
            }
        }
    }

    summary
}

fn build_behavior_trace_lines(
    behavior_plan: Option<&RunnerBehaviorPlan>,
    mode: Option<&str>,
    summary: &BehaviorTraceSummary,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(plan) = behavior_plan {
        if let Some(steps) = plan.steps_json.as_array() {
            for (index, step) in steps.iter().enumerate() {
                let primitive = step
                    .get("primitive")
                    .and_then(|value| value.as_str())
                    .unwrap_or("unknown");
                let outcome = if !matches!(mode, Some("active")) {
                    "shadow_compiled"
                } else if i64::try_from(index).unwrap_or(0) < summary.executed_steps {
                    "executed"
                } else {
                    "skipped"
                };
                lines.push(
                    json!({
                        "step_index": index,
                        "primitive": primitive,
                        "outcome": outcome,
                    })
                    .to_string(),
                );
            }
        }
    }
    lines
}

async fn update_behavior_site_stats_after_execution(
    state: &AppState,
    behavior_profile_id: Option<&str>,
    site_key: Option<&str>,
    page_archetype: Option<&str>,
    execution_status: RunnerOutcomeStatus,
    trace_summary: &BehaviorTraceSummary,
    finished_at: &str,
) -> Result<()> {
    let (Some(behavior_profile_id), Some(site_key), Some(page_archetype)) =
        (behavior_profile_id, site_key, page_archetype)
    else {
        return Ok(());
    };

    let existing = sqlx::query_as::<_, (i64, i64, i64, i64, Option<i64>)>(
        r#"SELECT success_count, failure_count, timeout_count, abort_count, avg_added_latency_ms
           FROM behavior_site_stats
           WHERE behavior_profile_id = ? AND site_key = ? AND page_archetype = ?"#,
    )
    .bind(behavior_profile_id)
    .bind(site_key)
    .bind(page_archetype)
    .fetch_optional(&state.db)
    .await?;

    let (mut success_count, mut failure_count, mut timeout_count, mut abort_count, current_avg) =
        existing.unwrap_or((0, 0, 0, 0, None));
    match execution_status {
        RunnerOutcomeStatus::Succeeded => success_count += 1,
        RunnerOutcomeStatus::Failed => failure_count += 1,
        RunnerOutcomeStatus::TimedOut => timeout_count += 1,
        RunnerOutcomeStatus::Cancelled => abort_count += 1,
    }
    if trace_summary.aborted && !matches!(execution_status, RunnerOutcomeStatus::Cancelled) {
        abort_count += 1;
    }
    let total_runs = success_count + failure_count + timeout_count + abort_count;
    let updated_avg = if total_runs > 0 {
        let previous_total = current_avg.unwrap_or(0) * (total_runs - 1);
        Some((previous_total + trace_summary.total_added_latency_ms) / total_runs)
    } else {
        current_avg
    };

    sqlx::query(
        r#"INSERT INTO behavior_site_stats (
               behavior_profile_id, site_key, page_archetype, success_count, failure_count,
               timeout_count, abort_count, avg_added_latency_ms, last_success_at, last_failure_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(behavior_profile_id, site_key, page_archetype) DO UPDATE SET
             success_count = excluded.success_count,
             failure_count = excluded.failure_count,
             timeout_count = excluded.timeout_count,
             abort_count = excluded.abort_count,
             avg_added_latency_ms = excluded.avg_added_latency_ms,
             last_success_at = excluded.last_success_at,
             last_failure_at = excluded.last_failure_at,
             updated_at = excluded.updated_at"#,
    )
    .bind(behavior_profile_id)
    .bind(site_key)
    .bind(page_archetype)
    .bind(success_count)
    .bind(failure_count)
    .bind(timeout_count)
    .bind(abort_count)
    .bind(updated_avg)
    .bind(if matches!(execution_status, RunnerOutcomeStatus::Succeeded) {
        Some(finished_at)
    } else {
        None
    })
    .bind(if matches!(
        execution_status,
        RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::TimedOut | RunnerOutcomeStatus::Cancelled
    ) {
        Some(finished_at)
    } else {
        None
    })
    .bind(finished_at)
    .execute(&state.db)
    .await?;

    Ok(())
}

pub async fn run_one_task_with_runner<R>(
    state: &AppState,
    runner: &R,
    worker_label: &str,
) -> Result<bool>
where
    R: TaskRunner + ?Sized,
{
    let Some(claimed) = claim_next_task(state, runner, worker_label).await? else {
        return Ok(false);
    };

    let task_id = claimed.task_id;
    let task_kind = claimed.task_kind;
    let input_json = claimed.input_json;
    let attempt = claimed.attempt;
    let execution_intent = claimed.execution_intent;
    let fingerprint_profile = claimed.fingerprint_profile;
    let requested_fingerprint_profile_id = claimed.requested_fingerprint_profile_id;
    let requested_fingerprint_profile_version = claimed.requested_fingerprint_profile_version;
    let requested_behavior_profile_id = claimed.requested_behavior_profile_id;
    let requested_behavior_profile_version = claimed.requested_behavior_profile_version;
    let behavior_profile = claimed.behavior_profile;
    let behavior_plan = claimed.behavior_plan;
    let compiled_form_action_plan = claimed.form_action_plan;
    let compiled_form_action_mode = claimed.form_action_mode;
    let compiled_form_action_summary_json = claimed.form_action_summary_json;
    let behavior_runtime_explain = claimed.behavior_runtime_explain;
    let behavior_trace_summary = claimed.behavior_trace_summary;
    let behavior_site_key = claimed.behavior_site_key;
    let run_id = claimed.run_id;
    let _started_at = claimed.started_at;
    let (heartbeat_stop, heartbeat_handle) =
        spawn_task_heartbeat(state.clone(), task_id.clone(), worker_label.to_string());

    insert_log(
        state,
        &format!("log-{}", Uuid::new_v4()),
        &task_id,
        None,
        "info",
        &format!("task claimed from database queue by {}", worker_label),
    )
    .await?;

    insert_log(
        state,
        &format!("log-{}", Uuid::new_v4()),
        &task_id,
        Some(&run_id),
        "info",
        &format!(
            "{} runner started task execution, attempt={attempt}",
            runner.name()
        ),
    )
    .await?;

    match (
        &requested_fingerprint_profile_id,
        requested_fingerprint_profile_version,
        &fingerprint_profile,
    ) {
        (Some(profile_id), Some(version), Some(profile)) => {
            insert_log(
                state,
                &format!("log-{}", Uuid::new_v4()),
                &task_id,
                Some(&run_id),
                "info",
                &format!(
                    "fingerprint profile resolved for runner execution: requested_id={}, requested_version={}, resolved_id={}, resolved_version={}",
                    profile_id,
                    version,
                    profile.id,
                    profile.version
                ),
            )
            .await?;
        }
        (Some(profile_id), Some(version), None) => {
            insert_log(
                state,
                &format!("log-{}", Uuid::new_v4()),
                &task_id,
                Some(&run_id),
                "warn",
                &format!(
                    "fingerprint profile requested but not resolved at execution time; runner will continue without injected profile: requested_id={}, requested_version={}",
                    profile_id,
                    version,
                ),
            )
            .await?;
        }
        _ => {}
    }

    match (
        &requested_behavior_profile_id,
        requested_behavior_profile_version,
        &behavior_profile,
    ) {
        (Some(profile_id), Some(version), Some(profile)) => {
            insert_log(
                state,
                &format!("log-{}", Uuid::new_v4()),
                &task_id,
                Some(&run_id),
                "info",
                &format!(
                    "behavior profile resolved for runner execution: requested_id={}, requested_version={}, resolved_id={}, resolved_version={}",
                    profile_id, version, profile.id, profile.version
                ),
            )
            .await?;
        }
        (Some(profile_id), Some(version), None) => {
            insert_log(
                state,
                &format!("log-{}", Uuid::new_v4()),
                &task_id,
                Some(&run_id),
                "warn",
                &format!(
                    "behavior profile requested but not resolved at execution time: requested_id={}, requested_version={}",
                    profile_id, version
                ),
            )
            .await?;
        }
        _ => {}
    }

    let mut payload: Value = serde_json::from_str(&input_json).unwrap_or_else(|_| {
        json!({
            "raw_input_json": input_json,
        })
    });
    let session_context = resolve_network_policy_for_task(state, &task_kind, &mut payload).await?;
    let proxy = extract_proxy_selection(&payload);
    let proxy_required_for_browser = browser_task_requires_proxy(&task_kind, &payload);
    let timeout_seconds = payload
        .get("timeout_seconds")
        .and_then(|value| value.as_i64())
        .filter(|value| *value > 0);

    let payload_for_binding = payload.clone();
    let proxy_for_health = proxy.clone();
    let fingerprint_profile_for_explain = fingerprint_profile.clone();
    let execution_intent_for_runner = execution_intent.clone();
    let behavior_profile_for_runner = behavior_profile.clone();
    let behavior_plan_for_runner = behavior_plan.clone();
    let mut form_action_plan_for_runner = compiled_form_action_plan.clone();
    let preflight_execution = if let Some(plan) = compiled_form_action_plan.as_ref() {
        if plan.execution_mode == "active" && plan.blocked_reason.is_some() {
            Some(form_action_preflight_failure(
                plan,
                FORM_ACTION_STATUS_BLOCKED,
                plan.blocked_reason
                    .clone()
                    .unwrap_or_else(|| "form action is blocked".to_string()),
            ))
        } else if plan.execution_mode == "active" {
            match resolve_form_action_plan_for_task(
                state,
                &task_id,
                execution_intent.as_ref(),
                plan,
            )
            .await
            {
                Ok(resolved_plan) => {
                    form_action_plan_for_runner = Some(resolved_plan);
                    None
                }
                Err(err) => Some(form_action_preflight_failure(
                    plan,
                    FORM_ACTION_STATUS_FAILED,
                    err.to_string(),
                )),
            }
        } else {
            None
        }
    } else {
        None
    };
    let execution = if task_kind == "verify_proxy" {
        let proxy_id = payload
            .get("proxy_id")
            .and_then(|v| v.as_str())
            .or_else(|| {
                payload
                    .get("network_policy_json")
                    .and_then(|v| v.get("proxy_id"))
                    .and_then(|v| v.as_str())
            });
        match proxy_id {
            Some(proxy_id) => match crate::api::handlers::run_proxy_verify_probe(state, proxy_id).await {
                Ok(result) => RunnerExecutionResult {
                    status: if result.status == "ok" { RunnerOutcomeStatus::Succeeded } else { RunnerOutcomeStatus::Failed },
                    result_json: Some(serde_json::to_value(&result).unwrap_or_else(|_| json!({"proxy_id": proxy_id, "status": result.status}))),
                    error_message: (result.status != "ok").then_some(result.message.clone()),
                    summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
                        category: crate::runner::types::SummaryArtifactCategory::Summary,
                        key: "verify_proxy.execution".to_string(),
                        source: "verify_pipeline".to_string(),
                        severity: crate::runner::types::SummaryArtifactSeverity::Info,
                        title: "verify_proxy execution summary".to_string(),
                        summary: format!("kind=verify_proxy proxy_id={} status={} message={}", proxy_id, result.status, result.message),
                    }],
                    session_cookies: None,
                    session_local_storage: None,
                    session_session_storage: None,
                },
                Err((_status, message)) => RunnerExecutionResult {
                    status: RunnerOutcomeStatus::Failed,
                    result_json: Some(json!({"proxy_id": proxy_id, "status": "failed", "message": message.clone()})),
                    error_message: Some(message.clone()),
                    summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
                        category: crate::runner::types::SummaryArtifactCategory::Debug,
                        key: "verify_proxy.execution".to_string(),
                        source: "verify_pipeline".to_string(),
                        severity: crate::runner::types::SummaryArtifactSeverity::Error,
                        title: "verify_proxy execution summary".to_string(),
                        summary: format!("kind=verify_proxy proxy_id={} status=failed message={}", proxy_id, message),
                    }],
                    session_cookies: None,
                    session_local_storage: None,
                    session_session_storage: None,
                },
            },
            None => RunnerExecutionResult {
                status: RunnerOutcomeStatus::Failed,
                result_json: Some(json!({"status": "failed", "message": "verify_proxy task requires proxy_id"})),
                error_message: Some("verify_proxy task requires proxy_id".to_string()),
                summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
                    category: crate::runner::types::SummaryArtifactCategory::Debug,
                    key: "verify_proxy.execution".to_string(),
                    source: "verify_pipeline".to_string(),
                    severity: crate::runner::types::SummaryArtifactSeverity::Error,
                    title: "verify_proxy execution summary".to_string(),
                    summary: "kind=verify_proxy status=failed message=verify_proxy task requires proxy_id".to_string(),
                }],
                session_cookies: None,
                session_local_storage: None,
                session_session_storage: None,
            },
        }
    } else if let Some(preflight_execution) = preflight_execution {
        preflight_execution
    } else if proxy_required_for_browser && proxy.is_none() {
        no_eligible_proxy_execution(&task_kind, &payload, fingerprint_profile.as_ref())
    } else {
        runner
            .execute(RunnerTask {
                task_id: task_id.clone(),
                attempt,
                kind: task_kind.clone(),
                payload,
                timeout_seconds,
                execution_intent: execution_intent_for_runner,
                fingerprint_profile,
                behavior_profile: behavior_profile_for_runner,
                behavior_plan: behavior_plan_for_runner,
                form_action_plan: form_action_plan_for_runner,
                proxy,
                session_cookies: session_context.restored_cookies.clone(),
                session_local_storage: session_context.restored_local_storage.clone(),
                session_session_storage: session_context.restored_session_storage.clone(),
            })
            .await
    };

    let _ = heartbeat_stop.send(());
    let _ = heartbeat_handle.await;

    let cookie_persist_count = execution
        .session_cookies
        .as_ref()
        .map(|cookies| i64::try_from(cookies.len()).unwrap_or(0))
        .unwrap_or(0);
    let local_storage_persist_count = storage_entry_count(execution.session_local_storage.as_ref());
    let session_storage_persist_count =
        storage_entry_count(execution.session_session_storage.as_ref());
    upsert_proxy_session_binding(
        state,
        &session_context,
        proxy_for_health.as_ref(),
        execution.status,
        execution.session_cookies.as_deref(),
        execution.session_local_storage.as_ref(),
        execution.session_session_storage.as_ref(),
    )
    .await?;
    update_proxy_health_after_execution(
        state,
        proxy_for_health.as_ref(),
        execution.status,
        execution.result_json.as_ref(),
    )
    .await?;
    update_proxy_site_stats_after_execution(
        state,
        &session_context,
        proxy_for_health.as_ref(),
        execution.status,
        execution.result_json.as_ref(),
    )
    .await?;

    let finished_at = now_ts_string();
    let behavior_execution_mode = behavior_mode_from_payload(&payload_for_binding);
    let behavior_seed = behavior_seed_from_payload(&payload_for_binding, behavior_plan.as_ref());
    let page_archetype = behavior_plan
        .as_ref()
        .and_then(|plan| plan.page_archetype.clone())
        .or_else(|| {
            payload_for_binding
                .get("behavior_policy_json")
                .and_then(|value| value.get("page_archetype"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    let runner_behavior_runtime_explain =
        extract_runner_behavior_runtime_explain(execution.result_json.as_ref());
    let runner_behavior_trace_summary =
        extract_runner_behavior_trace_summary(execution.result_json.as_ref());
    let runner_behavior_trace_lines =
        extract_runner_behavior_trace_lines(execution.result_json.as_ref());
    let form_action_status = extract_runner_form_action_status(execution.result_json.as_ref())
        .unwrap_or_else(|| match compiled_form_action_plan.as_ref() {
            None => FORM_ACTION_STATUS_NOT_REQUESTED.to_string(),
            Some(plan) if plan.execution_mode != "active" => {
                FORM_ACTION_STATUS_SHADOW_ONLY.to_string()
            }
            Some(plan) if plan.blocked_reason.is_some() => FORM_ACTION_STATUS_BLOCKED.to_string(),
            Some(_) => FORM_ACTION_STATUS_FAILED.to_string(),
        });
    let form_action_mode = extract_runner_form_action_mode(execution.result_json.as_ref())
        .or(compiled_form_action_mode.clone())
        .or_else(|| {
            compiled_form_action_plan
                .as_ref()
                .map(|plan| plan.mode.clone())
        });
    let form_action_retry_count =
        extract_runner_form_action_retry_count(execution.result_json.as_ref()).unwrap_or(0);
    let form_action_summary_json =
        extract_runner_form_action_summary(execution.result_json.as_ref())
            .or(compiled_form_action_summary_json.clone())
            .or_else(|| {
                compiled_form_action_plan.as_ref().map(|plan| {
                    let fallback_status = match execution.status {
                        RunnerOutcomeStatus::Succeeded if plan.execution_mode != "active" => {
                            FORM_ACTION_STATUS_SHADOW_ONLY
                        }
                        RunnerOutcomeStatus::Succeeded => FORM_ACTION_STATUS_SUCCEEDED,
                        RunnerOutcomeStatus::Failed
                        | RunnerOutcomeStatus::Cancelled
                        | RunnerOutcomeStatus::TimedOut => FORM_ACTION_STATUS_FAILED,
                    };
                    build_form_action_summary_json(
                        plan,
                        if form_action_status.is_empty() {
                            fallback_status
                        } else {
                            form_action_status.as_str()
                        },
                        form_action_retry_count,
                        plan.blocked_reason.as_deref(),
                        execution.error_message.as_deref(),
                    )
                })
            });

    let (task_status, run_status, log_level, log_message) = match execution.status {
        RunnerOutcomeStatus::Succeeded => (
            TASK_STATUS_SUCCEEDED,
            RUN_STATUS_SUCCEEDED,
            "info",
            format!(
                "{} runner finished successfully, attempt={attempt}",
                runner.name()
            ),
        ),
        RunnerOutcomeStatus::Failed => (
            TASK_STATUS_FAILED,
            RUN_STATUS_FAILED,
            "error",
            format!(
                "{} runner finished with failure, attempt={attempt}",
                runner.name()
            ),
        ),
        RunnerOutcomeStatus::Cancelled => (
            TASK_STATUS_CANCELLED,
            RUN_STATUS_CANCELLED,
            "warn",
            format!(
                "{} runner finished with cancellation, attempt={attempt}",
                runner.name()
            ),
        ),
        RunnerOutcomeStatus::TimedOut => (
            TASK_STATUS_TIMED_OUT,
            RUN_STATUS_TIMED_OUT,
            "warn",
            format!(
                "{} runner finished with timeout, attempt={attempt}",
                runner.name()
            ),
        ),
    };
    let behavior_runtime_explain = runner_behavior_runtime_explain.or(behavior_runtime_explain);
    let mut behavior_trace_summary = build_behavior_trace_summary_for_execution(
        behavior_execution_mode.as_deref(),
        behavior_plan.as_ref(),
        runner_behavior_trace_summary
            .as_ref()
            .or(behavior_trace_summary.as_ref()),
        runner_behavior_trace_summary.is_some(),
        execution.status,
        cookie_persist_count > 0
            || local_storage_persist_count > 0
            || session_storage_persist_count > 0,
    );
    let raw_trace_should_store = should_store_raw_trace(
        behavior_execution_mode.as_deref(),
        Some(&behavior_trace_summary),
        task_status,
        behavior_seed.as_deref().unwrap_or(task_id.as_str()),
    );
    behavior_trace_summary.raw_trace_persisted = raw_trace_should_store;

    let proxy_growth_explain =
        build_proxy_growth_explain_json(state, &payload_for_binding, proxy_for_health.as_ref())
            .await
            .ok();
    let fingerprint_runtime_explain = build_fingerprint_runtime_explain_json(
        &payload_for_binding,
        fingerprint_profile_for_explain.as_ref(),
        proxy_for_health.as_ref(),
    );
    let mut result_json_value = execution.result_json;
    if let Some(value) = result_json_value.as_mut() {
        if let serde_json::Value::Object(ref mut obj) = value {
            let mut summaries = execution
                .summary_artifacts
                .iter()
                .map(|item| {
                    json!({
                        "category": format!("{:?}", item.category).to_lowercase(),
                        "key": item.key,
                        "source": item.source,
                        "severity": item.severity.as_str(),
                        "title": item.title,
                        "summary": item.summary,
                        "run_id": run_id,
                        "attempt": attempt,
                        "timestamp": finished_at,
                    })
                })
                .collect::<Vec<_>>();
            if let Some(proxy_growth_explain) = proxy_growth_explain.clone() {
                let target_region = proxy_growth_explain
                    .get("target_region")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none");
                let selected_proxy_region = proxy_growth_explain
                    .get("selected_proxy_region")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none");
                let available_ratio_percent = proxy_growth_explain
                    .get("health_assessment")
                    .and_then(|v| v.get("available_ratio_percent"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let require_replenish = proxy_growth_explain
                    .get("health_assessment")
                    .and_then(|v| v.get("require_replenish"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let region_match_reason = proxy_growth_explain
                    .get("region_match")
                    .and_then(|v| v.get("reason"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("none");
                summaries.push(json!({
                    "category": "selection",
                    "key": format!("{}.proxy_growth", task_kind),
                    "source": "selection.proxy_growth",
                    "severity": if require_replenish { "warn" } else { "info" },
                    "title": "proxy growth assessment",
                    "summary": if require_replenish {
                        format!(
                            "pool needs replenishment for this request; target region {} ; selected region {} ; availability {}% ; region fit {}",
                            target_region,
                            selected_proxy_region,
                            available_ratio_percent,
                            region_match_reason,
                        )
                    } else {
                        format!(
                            "pool is healthy for this request; target region {} ; selected region {} ; availability {}% ; region fit {}",
                            target_region,
                            selected_proxy_region,
                            available_ratio_percent,
                            region_match_reason,
                        )
                    },
                    "run_id": run_id,
                    "attempt": attempt,
                    "timestamp": finished_at,
                }));
                obj.insert("proxy_growth_explain".to_string(), proxy_growth_explain);
            }
            if fingerprint_runtime_explain
                .get("fingerprint_budget_tag")
                .and_then(|v| v.as_str())
                .is_some()
            {
                let budget = fingerprint_runtime_explain
                    .get("fingerprint_budget_tag")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none");
                let consistency = fingerprint_runtime_explain
                    .get("fingerprint_consistency")
                    .and_then(|v| v.get("overall_status"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("none");
                summaries.push(json!({
                    "category": "summary",
                    "key": format!("{}.fingerprint_runtime", task_kind),
                    "source": "runner.fingerprint",
                    "severity": if consistency == "mismatch" { "warning" } else { "info" },
                    "title": "fingerprint runtime assessment",
                    "summary": format!(
                        "fingerprint runtime used {} budget with {} consistency",
                        budget,
                        consistency,
                    ),
                    "run_id": run_id,
                    "attempt": attempt,
                    "timestamp": finished_at,
                }));
            }
            obj.insert(
                "fingerprint_runtime_explain".to_string(),
                fingerprint_runtime_explain.clone(),
            );
            obj.insert(
                "behavior_profile_id".to_string(),
                json!(behavior_profile
                    .as_ref()
                    .map(|profile| profile.id.clone())
                    .or(requested_behavior_profile_id.clone())),
            );
            obj.insert(
                "behavior_profile_version".to_string(),
                json!(behavior_profile
                    .as_ref()
                    .map(|profile| profile.version)
                    .or(requested_behavior_profile_version)),
            );
            obj.insert(
                "behavior_resolution_status".to_string(),
                json!(if behavior_profile.is_some() {
                    "resolved"
                } else if behavior_execution_mode.is_some() {
                    "disabled"
                } else {
                    "none"
                }),
            );
            obj.insert(
                "behavior_execution_mode".to_string(),
                json!(behavior_execution_mode.clone()),
            );
            obj.insert("page_archetype".to_string(), json!(page_archetype.clone()));
            obj.insert("behavior_seed".to_string(), json!(behavior_seed.clone()));
            obj.insert(
                "form_action_status".to_string(),
                json!(form_action_status.clone()),
            );
            obj.insert(
                "form_action_mode".to_string(),
                json!(form_action_mode.clone()),
            );
            obj.insert(
                "form_action_retry_count".to_string(),
                json!(form_action_retry_count),
            );
            obj.insert(
                "form_action_summary_json".to_string(),
                json!(form_action_summary_json.clone()),
            );
            obj.insert(
                "behavior_trace_summary".to_string(),
                json!(behavior_trace_summary.clone()),
            );
            if let Some(runtime_explain) = behavior_runtime_explain.clone() {
                summaries.push(json!({
                    "category": "summary",
                    "key": format!("{}.behavior_runtime", task_kind),
                    "source": "runner.behavior",
                    "severity": if behavior_execution_mode.as_deref() == Some("active") { "info" } else { "info" },
                    "title": "behavior runtime assessment",
                    "summary": format!(
                        "behavior mode={} page_archetype={} planned_steps={} executed_steps={} aborted={}",
                        behavior_execution_mode.as_deref().unwrap_or("disabled"),
                        page_archetype.as_deref().unwrap_or("generic"),
                        behavior_trace_summary.planned_steps,
                        behavior_trace_summary.executed_steps,
                        behavior_trace_summary.aborted,
                    ),
                    "run_id": run_id,
                    "attempt": attempt,
                    "timestamp": finished_at,
                }));
                obj.insert(
                    "behavior_runtime_explain".to_string(),
                    json!(runtime_explain),
                );
            }
            obj.insert(
                "identity_session_status".to_string(),
                json!(session_context.identity_session_status),
            );
            obj.insert(
                "cookie_restore_count".to_string(),
                json!(session_context.cookie_restore_count),
            );
            obj.insert(
                "cookie_persist_count".to_string(),
                json!(cookie_persist_count),
            );
            obj.insert(
                "local_storage_restore_count".to_string(),
                json!(session_context.local_storage_restore_count),
            );
            obj.insert(
                "local_storage_persist_count".to_string(),
                json!(local_storage_persist_count),
            );
            obj.insert(
                "session_storage_restore_count".to_string(),
                json!(session_context.session_storage_restore_count),
            );
            obj.insert(
                "session_storage_persist_count".to_string(),
                json!(session_storage_persist_count),
            );
            if let Some(network_policy) = obj
                .get_mut("payload")
                .and_then(|value| value.get_mut("network_policy_json"))
                .and_then(Value::as_object_mut)
            {
                network_policy.insert(
                    "identity_session_status".to_string(),
                    json!(session_context.identity_session_status),
                );
                network_policy.insert(
                    "cookie_restore_count".to_string(),
                    json!(session_context.cookie_restore_count),
                );
                network_policy.insert(
                    "cookie_persist_count".to_string(),
                    json!(cookie_persist_count),
                );
                network_policy.insert(
                    "local_storage_restore_count".to_string(),
                    json!(session_context.local_storage_restore_count),
                );
                network_policy.insert(
                    "local_storage_persist_count".to_string(),
                    json!(local_storage_persist_count),
                );
                network_policy.insert(
                    "session_storage_restore_count".to_string(),
                    json!(session_context.session_storage_restore_count),
                );
                network_policy.insert(
                    "session_storage_persist_count".to_string(),
                    json!(session_storage_persist_count),
                );
            }
            if session_context.auto_session_enabled {
                summaries.push(json!({
                    "category": "summary",
                    "key": format!("{}.identity_session", task_kind),
                    "source": "selection.identity_session",
                    "severity": "info",
                    "title": "identity session continuity",
                    "summary": format!(
                        "identity_session_status={} cookie_restore_count={} cookie_persist_count={} local_storage_restore_count={} local_storage_persist_count={} session_storage_restore_count={} session_storage_persist_count={}",
                        session_context.identity_session_status,
                        session_context.cookie_restore_count,
                        cookie_persist_count,
                        session_context.local_storage_restore_count,
                        local_storage_persist_count,
                        session_context.session_storage_restore_count,
                        session_storage_persist_count,
                    ),
                    "run_id": run_id,
                    "attempt": attempt,
                    "timestamp": finished_at,
                }));
            }
            obj.insert("summary_artifacts".to_string(), json!(summaries));
        }
    }
    if let Some(value) = result_json_value.as_mut() {
        apply_task_continuity_after_execution(
            state,
            &task_id,
            &run_id,
            &task_kind,
            task_status,
            &payload_for_binding,
            value,
        )
        .await?;
    }
    let result_json = result_json_value.as_ref().map(Value::to_string);
    let error_message = execution.error_message;
    let behavior_trace_lines = runner_behavior_trace_lines.unwrap_or_else(|| {
        build_behavior_trace_lines(
            behavior_plan.as_ref(),
            behavior_execution_mode.as_deref(),
            &behavior_trace_summary,
        )
    });

    if raw_trace_should_store {
        sqlx::query(
            r#"INSERT INTO artifacts (id, task_id, run_id, kind, storage_path, metadata_json, created_at)
               VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(format!("artifact-{}", Uuid::new_v4()))
        .bind(&task_id)
        .bind(&run_id)
        .bind("behavior_trace")
        .bind("db://behavior_trace.ndjson")
        .bind(json!({
            "lines": behavior_trace_lines,
            "behavior_execution_mode": behavior_execution_mode,
            "behavior_seed": behavior_seed,
        }).to_string())
        .bind(&finished_at)
        .execute(&state.db)
        .await?;
    }

    sqlx::query(
        r#"INSERT INTO artifacts (id, task_id, run_id, kind, storage_path, metadata_json, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(format!("artifact-{}", Uuid::new_v4()))
    .bind(&task_id)
    .bind(&run_id)
    .bind("session_state_before")
    .bind("db://session_state.before.json")
    .bind(json!({
        "cookies": session_context.restored_cookies,
        "local_storage": session_context.restored_local_storage,
        "session_storage": session_context.restored_session_storage,
    }).to_string())
    .bind(&finished_at)
    .execute(&state.db)
    .await?;

    sqlx::query(
        r#"INSERT INTO artifacts (id, task_id, run_id, kind, storage_path, metadata_json, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(format!("artifact-{}", Uuid::new_v4()))
    .bind(&task_id)
    .bind(&run_id)
    .bind("session_state_after")
    .bind("db://session_state.after.json")
    .bind(json!({
        "cookies": execution.session_cookies,
        "local_storage": execution.session_local_storage,
        "session_storage": execution.session_session_storage,
    }).to_string())
    .bind(&finished_at)
    .execute(&state.db)
    .await?;

    update_behavior_site_stats_after_execution(
        state,
        behavior_profile
            .as_ref()
            .map(|profile| profile.id.as_str())
            .or(requested_behavior_profile_id.as_deref()),
        behavior_site_key
            .as_deref()
            .or(session_context.site_key.as_deref()),
        page_archetype.as_deref(),
        execution.status,
        &behavior_trace_summary,
        &finished_at,
    )
    .await?;

    let run_update = sqlx::query(
        &format!(
            "UPDATE runs SET status = ?, finished_at = ?, error_message = ?, result_json = ? WHERE id = ? AND status = '{}'",
            RUN_STATUS_RUNNING,
        ),
    )
    .bind(run_status)
    .bind(&finished_at)
    .bind(&error_message)
    .bind(&result_json)
    .bind(&run_id)
    .execute(&state.db)
    .await?;

    if run_update.rows_affected() == 0 {
        insert_log(
            state,
            &format!("log-{}", Uuid::new_v4()),
            &task_id,
            Some(&run_id),
            "warn",
            &format!(
                "{} runner finished but run terminal overwrite skipped because run was no longer running, attempt={attempt}",
                runner.name()
            ),
        )
        .await?;
    }

    let current_task_status =
        sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_one(&state.db)
            .await?;

    if current_task_status != TASK_STATUS_CANCELLED {
        let task_update = sqlx::query(
            &format!(
                "UPDATE tasks SET status = ?, finished_at = ?, runner_id = NULL, heartbeat_at = NULL, result_json = ?, error_message = ? WHERE id = ? AND status = '{}'",
                TASK_STATUS_RUNNING,
            ),
        )
        .bind(task_status)
        .bind(&finished_at)
        .bind(&result_json)
        .bind(&error_message)
        .bind(&task_id)
        .execute(&state.db)
        .await?;

        if task_update.rows_affected() == 0 {
            let latest_task_status =
                sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
                    .bind(&task_id)
                    .fetch_one(&state.db)
                    .await?;

            if latest_task_status == TASK_STATUS_CANCELLED {
                sqlx::query("UPDATE tasks SET finished_at = ?, runner_id = NULL, heartbeat_at = NULL, result_json = COALESCE(?, result_json) WHERE id = ?")
                    .bind(&finished_at)
                    .bind(&result_json)
                    .bind(&task_id)
                    .execute(&state.db)
                    .await?;

                insert_log(
                    state,
                    &format!("log-{}", Uuid::new_v4()),
                    &task_id,
                    Some(&run_id),
                    "warn",
                    &format!(
                        "{} runner finished after cancel race; terminal task status overwrite skipped but result cleanup persisted, attempt={attempt}",
                        runner.name()
                    ),
                )
                .await?;
            } else {
                insert_log(
                    state,
                    &format!("log-{}", Uuid::new_v4()),
                    &task_id,
                    Some(&run_id),
                    "warn",
                    &format!(
                        "{} runner finished but task terminal overwrite skipped because task was no longer running, attempt={attempt}",
                        runner.name()
                    ),
                )
                .await?;
            }
        }
    } else {
        sqlx::query("UPDATE tasks SET finished_at = ?, runner_id = NULL, heartbeat_at = NULL, result_json = COALESCE(?, result_json) WHERE id = ?")
            .bind(&finished_at)
            .bind(&result_json)
            .bind(&task_id)
            .execute(&state.db)
            .await?;

        insert_log(
            state,
            &format!("log-{}", Uuid::new_v4()),
            &task_id,
            Some(&run_id),
            "warn",
            &format!(
                "{} runner finished after cancel; terminal task status overwrite skipped but result cleanup persisted, attempt={attempt}",
                runner.name()
            ),
        )
        .await?;
    }

    insert_log(
        state,
        &format!("log-{}", Uuid::new_v4()),
        &task_id,
        Some(&run_id),
        log_level,
        &log_message,
    )
    .await?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network_identity::proxy_selection::default_proxy_selection_tuning;

    fn components_json() -> TrustScoreComponents {
        TrustScoreComponents {
            verify_ok_bonus: 30,
            verify_geo_match_bonus: 20,
            site_success_bonus: 0,
            geo_mismatch_penalty: 0,
            region_mismatch_penalty: 0,
            geo_risk_penalty: 0,
            smoke_upstream_ok_bonus: 10,
            raw_score_component: 8,
            missing_verify_penalty: 0,
            stale_verify_penalty: 0,
            verify_failed_heavy_penalty: 0,
            verify_failed_light_penalty: 0,
            verify_failed_base_penalty: 0,
            individual_history_penalty: 0,
            provider_risk_penalty: 0,
            provider_region_cluster_penalty: 0,
            verify_confidence_bonus: 0,
            verify_score_delta_bonus: 0,
            verify_source_bonus: 0,
            anonymity_bonus: 0,
            latency_penalty: 0,
            exit_ip_not_public_penalty: 0,
            probe_error_penalty: 0,
            verify_risk_penalty: 0,
            site_failure_penalty: 0,
            soft_min_score_penalty: 0,
        }
    }

    fn penalty_components_json() -> TrustScoreComponents {
        TrustScoreComponents {
            verify_ok_bonus: 0,
            verify_geo_match_bonus: 0,
            site_success_bonus: 0,
            geo_mismatch_penalty: 8,
            region_mismatch_penalty: 4,
            geo_risk_penalty: 12,
            smoke_upstream_ok_bonus: 0,
            raw_score_component: 0,
            missing_verify_penalty: 12,
            stale_verify_penalty: 8,
            verify_failed_heavy_penalty: 30,
            verify_failed_light_penalty: 15,
            verify_failed_base_penalty: 10,
            individual_history_penalty: 2,
            provider_risk_penalty: 5,
            provider_region_cluster_penalty: 2,
            verify_confidence_bonus: 0,
            verify_score_delta_bonus: 0,
            verify_source_bonus: 0,
            anonymity_bonus: 0,
            latency_penalty: 0,
            exit_ip_not_public_penalty: 0,
            probe_error_penalty: 0,
            verify_risk_penalty: 0,
            site_failure_penalty: 0,
            soft_min_score_penalty: 0,
        }
    }

    #[test]
    fn pick_claim_candidate_skips_heavy_when_heavy_budget_is_full() {
        let heavy = Some(serde_json::json!({"canvas": {"mode": "noise"}}).to_string());
        let light = Some(serde_json::json!({"timezone": "Asia/Shanghai"}).to_string());
        let idx = pick_claim_candidate_index(
            &[heavy, light],
            ClaimBudgetSnapshot {
                running_medium: 0,
                running_heavy: 1,
                medium_limit: 2,
                heavy_limit: 1,
            },
        );
        assert_eq!(idx, Some(1));
    }

    #[test]
    fn pick_claim_candidate_accepts_heavy_when_capacity_exists() {
        let heavy = Some(serde_json::json!({"canvas": {"mode": "noise"}}).to_string());
        let light = Some(serde_json::json!({"timezone": "Asia/Shanghai"}).to_string());
        let idx = pick_claim_candidate_index(
            &[heavy, light],
            ClaimBudgetSnapshot {
                running_medium: 0,
                running_heavy: 0,
                medium_limit: 2,
                heavy_limit: 1,
            },
        );
        assert_eq!(idx, Some(0));
    }

    #[test]
    fn pick_claim_candidate_skips_medium_when_medium_budget_is_full_but_keeps_light_moving() {
        let medium = Some(serde_json::json!({"hardware_concurrency": 8}).to_string());
        let light = Some(serde_json::json!({"timezone": "Asia/Shanghai"}).to_string());
        let idx = pick_claim_candidate_index(
            &[medium, light],
            ClaimBudgetSnapshot {
                running_medium: 2,
                running_heavy: 0,
                medium_limit: 2,
                heavy_limit: 1,
            },
        );
        assert_eq!(idx, Some(1));
    }

    #[test]
    fn computed_trust_score_components_returns_typed_breakdown() {
        let tuning = default_proxy_selection_tuning();
        let components = computed_trust_score_components(
            &tuning,
            0.77,
            5,
            1,
            Some("ok"),
            true,
            Some(false),
            true,
            Some(9999999999),
            Some(0.98),
            Some(18),
            Some("local_verify"),
            Some("elite"),
            Some(650),
            Some("protocol_invalid"),
            true,
            true,
            1000,
            None,
        );
        assert_eq!(components.verify_ok_bonus, 30);
        assert_eq!(components.verify_geo_match_bonus, 20);
        assert_eq!(components.geo_mismatch_penalty, 0);
        assert_eq!(components.region_mismatch_penalty, 4);
        assert_eq!(components.geo_risk_penalty, 4);
        assert_eq!(components.smoke_upstream_ok_bonus, 10);
        assert_eq!(components.raw_score_component, 8);
        assert_eq!(components.provider_risk_penalty, 5);
        assert_eq!(components.provider_region_cluster_penalty, 2);
        assert_eq!(components.verify_confidence_bonus, 3);
        assert_eq!(components.verify_score_delta_bonus, 2);
        assert_eq!(components.verify_source_bonus, 2);
        let runner_components = computed_trust_score_components(
            &tuning,
            0.77,
            5,
            1,
            Some("ok"),
            true,
            Some(false),
            true,
            Some(9999999999),
            Some(0.98),
            Some(18),
            Some("runner_verify"),
            Some("elite"),
            Some(650),
            Some("protocol_invalid"),
            true,
            true,
            1000,
            None,
        );
        assert_eq!(runner_components.verify_source_bonus, 1);
        let imported_components = computed_trust_score_components(
            &tuning,
            0.77,
            5,
            1,
            Some("ok"),
            true,
            Some(false),
            true,
            Some(9999999999),
            Some(0.98),
            Some(18),
            Some("imported_verify"),
            Some("elite"),
            Some(650),
            Some("protocol_invalid"),
            true,
            true,
            1000,
            None,
        );
        assert_eq!(imported_components.verify_source_bonus, -1);
        assert_eq!(components.anonymity_bonus, 4);
        assert_eq!(components.latency_penalty, -2);
        assert_eq!(components.exit_ip_not_public_penalty, 0);
        assert_eq!(components.probe_error_penalty, 6);
        assert_eq!(components.verify_risk_penalty, 6);
        assert_eq!(components.missing_verify_penalty, 0);
        assert_eq!(components.site_success_bonus, 0);
        assert_eq!(components.site_failure_penalty, 0);
    }

    #[test]
    fn proxy_site_score_signal_rewards_clean_history_and_penalizes_browser_failures() {
        let tuning = default_proxy_selection_tuning();
        let success_signal = proxy_site_score_signal_from_stats(&tuning, 3, 0, None, None);
        assert_eq!(success_signal.site_success_bonus, 3);
        assert_eq!(success_signal.site_failure_penalty, 0);

        let failure_signal = proxy_site_score_signal_from_stats(
            &tuning,
            0,
            4,
            Some("browser_execution"),
            Some("browser_navigation_failure_signal"),
        );
        assert_eq!(failure_signal.site_success_bonus, 0);
        assert_eq!(failure_signal.site_failure_penalty, 8);
    }

    #[test]
    fn summarize_component_advantages_and_delta_expose_expected_language() {
        let summary = summarize_component_advantages(&components_json());
        assert!(summary.contains("wins on verify_ok, geo_match, upstream_ok, raw_score"));
        let penalty_summary = summarize_component_advantages(&penalty_components_json());
        assert!(penalty_summary.contains("penalized by geo_risk, missing_verify, stale_verify, verify_failed_heavy, verify_failed_light, verify_failed_base, history_risk, provider_risk, provider_region_risk"));
        assert!(!penalty_summary.contains("geo_mismatch"));
        assert!(!penalty_summary.contains("region_mismatch"));

        let delta = summarize_component_delta(&components_json(), Some(&penalty_components_json()));
        assert!(delta.contains("better on"));
        assert!(!delta.contains("worse on"));
        assert!(delta.contains("wins on verify_ok, geo_match, upstream_ok, raw_score"));
    }

    #[test]
    fn summarize_component_advantages_prefers_aggregate_risk_labels() {
        let mut penalty = penalty_components_json();
        penalty.exit_ip_not_public_penalty = 9;
        penalty.probe_error_penalty = 6;
        penalty.verify_risk_penalty = 15;

        let summary = summarize_component_advantages(&penalty);
        assert!(summary.contains("geo_risk"));
        assert!(summary.contains("verify_risk"));
        assert!(!summary.contains("geo_mismatch"));
        assert!(!summary.contains("region_mismatch"));
        assert!(!summary.contains("exit_ip_not_public"));
        assert!(!summary.contains("probe_error_category"));

        let delta = summarize_component_delta(&components_json(), Some(&penalty));
        assert!(delta.contains("geo_risk"));
        assert!(delta.contains("verify_risk"));
        assert!(!delta.contains("exit_ip_not_public"));
        assert!(!delta.contains("probe_error_category"));
    }

    #[test]
    fn structured_component_delta_orders_factors_by_magnitude_and_uses_expected_labels() {
        let current = components_json();
        let baseline = penalty_components_json();
        let delta = structured_component_delta(&current, Some(&baseline));
        assert_eq!(delta.winner_total_score, 68);
        assert_eq!(delta.runner_up_total_score, -108);
        assert_eq!(delta.score_gap, 176);
        let factors = &delta.factors;
        assert!(!factors.is_empty());
        assert!(factors.len() <= 5);
        let labels: Vec<&str> = factors.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.iter().any(|label| *label == "verify_ok"));
        assert!(labels.iter().any(|label| matches!(
            *label,
            "missing_verify"
                | "stale_verify"
                | "verify_failed_heavy"
                | "verify_failed_light"
                | "verify_failed_base"
                | "history_risk"
                | "provider_risk"
                | "provider_region_risk"
                | "geo_risk"
        )));
        assert!(!labels.iter().any(|label| matches!(
            *label,
            "geo_mismatch" | "region_mismatch" | "exit_ip_not_public" | "probe_error_category"
        )));
        let deltas: Vec<i64> = factors.iter().map(|item| item.delta.abs()).collect();
        assert!(deltas.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn structured_component_delta_without_baseline_returns_neutral_factor_bundle() {
        let current = components_json();
        let delta = structured_component_delta(&current, None);
        assert_eq!(delta.runner_up_total_score, delta.winner_total_score);
        assert_eq!(delta.score_gap, 0);
        let factors = &delta.factors;
        assert_eq!(factors.len(), 5);
        assert!(factors
            .iter()
            .all(|item| matches!(item.direction, WinnerVsRunnerUpDirection::Neutral)));
        assert!(factors.iter().all(|item| item.delta == 0));
    }
}
