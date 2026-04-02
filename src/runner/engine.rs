use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use tokio::{sync::oneshot, task::JoinHandle, time::Duration};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::network_identity::proxy_growth::{assess_proxy_pool_health, default_proxy_pool_growth_policy, evaluate_region_match, ProxyPoolInventorySnapshot};
use crate::network_identity::proxy_selection::{apply_proxy_resolution_metadata, proxy_selection_base_where_sql, proxy_selection_order_by_cached_trust_score_sql, proxy_selection_order_by_trust_score_sql_with_tuning, resolved_proxy_json};
use crate::{
    api::dto::{
        CandidateRankPreviewItem, TrustScoreComponents, WinnerVsRunnerUpDiff,
        WinnerVsRunnerUpDirection, WinnerVsRunnerUpFactor,
    },
    app::state::AppState,
    db::init::refresh_proxy_trust_views_for_scope,
    domain::{
        run::{RUN_STATUS_RUNNING, RUN_STATUS_SUCCEEDED, RUN_STATUS_FAILED, RUN_STATUS_TIMED_OUT},
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING,
            TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
    runner::{
        runner_claim_retry_limit_from_env, runner_heartbeat_interval_seconds_from_env,
        RunnerExecutionResult, RunnerFingerprintProfile, RunnerOutcomeStatus, RunnerProxySelection, RunnerTask,
        TaskRunner,
    },
};

fn now_ts_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
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
        username: proxy_obj.get("username").and_then(|v| v.as_str()).map(|v| v.to_string()),
        password: proxy_obj.get("password").and_then(|v| v.as_str()).map(|v| v.to_string()),
        region: proxy_obj.get("region").and_then(|v| v.as_str()).map(|v| v.to_string()),
        country: proxy_obj.get("country").and_then(|v| v.as_str()).map(|v| v.to_string()),
        provider: proxy_obj.get("provider").and_then(|v| v.as_str()).map(|v| v.to_string()),
        score: proxy_obj.get("score").and_then(|v| v.as_f64()).unwrap_or(1.0),
        resolution_status: policy.get("proxy_resolution_status").and_then(|v| v.as_str()).unwrap_or("resolved").to_string(),
    })
}

async fn load_proxy_trust_score(state: &AppState, proxy_id: &str, _now: &str) -> Result<Option<i64>> {
    let value = sqlx::query_scalar::<_, i64>("SELECT cached_trust_score FROM proxies WHERE id = ? LIMIT 1")
        .bind(proxy_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(value)
}

fn selection_reason_summary_for_mode(mode: &str, trust_score_total: Option<i64>, candidate_summary: Option<&str>) -> String {
    match (mode, trust_score_total, candidate_summary) {
        ("explicit", Some(score), _) => format!("explicit proxy_id selected active proxy directly; current trust score snapshot={score}"),
        ("sticky", Some(score), _) => format!("sticky session reused an active proxy binding; current trust score snapshot={score}"),
        ("auto", Some(score), Some(summary)) => format!("selected highest-ranked active proxy by trust score ordering; trust_score_total={score}; {summary}"),
        ("auto", None, Some(summary)) => format!("selected highest-ranked active proxy by trust score ordering; {summary}"),
        ("explicit", None, _) => "explicit proxy_id selected active proxy directly".to_string(),
        ("sticky", None, _) => "sticky session reused an active proxy binding".to_string(),
        ("auto", Some(score), None) => format!("selected highest-ranked active proxy by trust score ordering; trust_score_total={score}"),
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
    last_anonymity_level: Option<&str>,
    last_probe_latency_ms: Option<i64>,
    last_probe_error_category: Option<&str>,
    provider_risk_hit: bool,
    provider_region_cluster_hit: bool,
    now_ts: i64,
    soft_min_score: Option<f64>,
) -> TrustScoreComponents {
    let verify_ok_bonus = if last_verify_status == Some("ok") { tuning.verify_ok_bonus } else { 0 };
    let verify_geo_match_bonus = if last_verify_geo_match_ok { tuning.verify_geo_match_bonus } else { 0 };
    let geo_mismatch_penalty = if !last_verify_geo_match_ok { tuning.geo_mismatch_penalty } else { 0 };
    let region_mismatch_penalty = if last_region_match_ok == Some(false) { tuning.region_mismatch_penalty } else { 0 };
    let smoke_upstream_ok_bonus = if last_smoke_upstream_ok { tuning.smoke_upstream_ok_bonus } else { 0 };
    let raw_score_component = (score * tuning.raw_score_weight_tenths as f64).round() as i64;
    let missing_verify_penalty = if last_verify_at.is_none() { tuning.missing_verify_penalty } else { 0 };
    let stale_verify_penalty = if last_verify_at.map(|v| v <= now_ts - tuning.stale_after_seconds).unwrap_or(false) { tuning.stale_verify_penalty } else { 0 };
    let verify_failed_heavy_penalty = if last_verify_status == Some("failed") && last_verify_at.map(|v| v >= now_ts - tuning.recent_failure_heavy_window_seconds).unwrap_or(false) { tuning.verify_failed_heavy_penalty } else { 0 };
    let verify_failed_light_penalty = if last_verify_status == Some("failed") && verify_failed_heavy_penalty == 0 && last_verify_at.map(|v| v >= now_ts - tuning.recent_failure_light_window_seconds).unwrap_or(false) { tuning.verify_failed_light_penalty } else { 0 };
    let verify_failed_base_penalty = if last_verify_status == Some("failed") { tuning.verify_failed_base_penalty } else { 0 };
    let individual_history_penalty = if failure_count >= success_count + 3 { 2 } else if failure_count > success_count { 1 } else { 0 };
    let provider_risk_penalty = if provider_risk_hit { tuning.provider_failure_margin } else { 0 };
    let provider_region_cluster_penalty = if provider_region_cluster_hit { tuning.provider_region_failure_cluster_count } else { 0 };
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
    let soft_min_score_penalty = if let Some(threshold) = soft_min_score {
        if score < threshold { tuning.soft_min_score_penalty } else { 0 }
    } else {
        0
    };

    TrustScoreComponents {
        verify_ok_bonus,
        verify_geo_match_bonus,
        geo_mismatch_penalty,
        region_mismatch_penalty,
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
        anonymity_bonus,
        latency_penalty,
        exit_ip_not_public_penalty,
        probe_error_penalty,
        soft_min_score_penalty,
    }
}

fn component_value(components: &TrustScoreComponents, key: &str) -> i64 {
    match key {
        "verify_ok_bonus" => components.verify_ok_bonus,
        "verify_geo_match_bonus" => components.verify_geo_match_bonus,
        "geo_mismatch_penalty" => components.geo_mismatch_penalty,
        "region_mismatch_penalty" => components.region_mismatch_penalty,
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
        "anonymity_bonus" => components.anonymity_bonus,
        "latency_penalty" => components.latency_penalty,
        "exit_ip_not_public_penalty" => components.exit_ip_not_public_penalty,
        "probe_error_penalty" => components.probe_error_penalty,
        "soft_min_score_penalty" => components.soft_min_score_penalty,
        _ => 0,
    }
}

fn component_keys() -> [&'static str; 18] {
    [
        "verify_ok_bonus",
        "verify_geo_match_bonus",
        "geo_mismatch_penalty",
        "region_mismatch_penalty",
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
        "anonymity_bonus",
        "latency_penalty",
        "exit_ip_not_public_penalty",
        "probe_error_penalty",
    ]
}

fn positive_component_keys() -> [&'static str; 5] {
    [
        "verify_ok_bonus",
        "verify_geo_match_bonus",
        "smoke_upstream_ok_bonus",
        "raw_score_component",
        "anonymity_bonus",
    ]
}

fn empty_components() -> TrustScoreComponents {
    TrustScoreComponents {
        verify_ok_bonus: 0,
        verify_geo_match_bonus: 0,
        geo_mismatch_penalty: 0,
        region_mismatch_penalty: 0,
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
        anonymity_bonus: 0,
        latency_penalty: 0,
        exit_ip_not_public_penalty: 0,
        probe_error_penalty: 0,
        soft_min_score_penalty: 0,
    }
}

fn component_label(key: &str) -> &'static str {
    match key {
        "verify_ok_bonus" => "verify_ok",
        "verify_geo_match_bonus" => "geo_match",
        "geo_mismatch_penalty" => "geo_mismatch",
        "region_mismatch_penalty" => "region_mismatch",
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
        "anonymity_bonus" => "anonymity",
        "latency_penalty" => "probe_latency",
        "exit_ip_not_public_penalty" => "exit_ip_not_public",
        "probe_error_penalty" => "probe_error_category",
        "soft_min_score_penalty" => "soft_min_score",
        _ => "unknown",
    }
}

pub fn summarize_component_advantages(current: &TrustScoreComponents) -> String {
    let mut positives = Vec::new();
    let mut penalties = Vec::new();
    for key in positive_component_keys() {
        let value = component_value(current, key);
        if value > 0 {
            positives.push(component_label(key));
        }
    }
    for key in [
        "missing_verify_penalty",
        "stale_verify_penalty",
        "verify_failed_heavy_penalty",
        "verify_failed_light_penalty",
        "verify_failed_base_penalty",
        "individual_history_penalty",
        "provider_risk_penalty",
        "provider_region_cluster_penalty",
    ] {
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

pub fn structured_component_delta(current: &TrustScoreComponents, baseline: Option<&TrustScoreComponents>) -> WinnerVsRunnerUpDiff {
    let keys = component_keys();
    let positive = positive_component_keys();
    let winner_total_score = keys.iter().map(|key| component_value(current, key)).sum::<i64>();

    let Some(baseline) = baseline else {
        let mut factors: Vec<WinnerVsRunnerUpFactor> = keys
            .into_iter()
            .map(|key| {
                let value = component_value(current, key);
                WinnerVsRunnerUpFactor {
                    factor: key.to_string(),
                    label: component_label(key).to_string(),
                    winner_value: value,
                    runner_up_value: 0,
                    delta: value,
                    direction: WinnerVsRunnerUpDirection::Neutral,
                }
            })
            .collect();
        factors.sort_by_key(|v| std::cmp::Reverse(v.delta.abs()));
        factors.truncate(5);
        return WinnerVsRunnerUpDiff {
            winner_total_score,
            runner_up_total_score: 0,
            score_gap: winner_total_score,
            factors,
        };
    };

    let runner_up_total_score = keys.iter().map(|key| component_value(baseline, key)).sum::<i64>();
    let mut factors = Vec::new();
    for key in keys {
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
        let delta = if positive.contains(&key) { cv - bv } else { bv - cv };
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

pub fn summarize_component_delta(current: &TrustScoreComponents, baseline: Option<&TrustScoreComponents>) -> String {
    let current_summary = summarize_component_advantages(current);
    let Some(baseline) = baseline else {
        return current_summary;
    };

    let mut better = Vec::new();
    let mut worse = Vec::new();
    for key in component_keys() {
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

async fn compute_top_candidate_component_map(
    state: &AppState,
    now: &str,
    provider: Option<&str>,
    region: Option<&str>,
    min_score: f64,
    soft_min_score: Option<f64>,
) -> Result<std::collections::HashMap<String, TrustScoreComponents>> {
    let query = format!(
        "SELECT id FROM proxies {} ORDER BY {} LIMIT 3",
        proxy_selection_base_where_sql(),
        proxy_selection_order_by_cached_trust_score_sql()
    );
    let ids = sqlx::query_scalar::<_, String>(&query)
        .bind(now)
         .bind(provider.as_deref())
        .bind(provider.as_deref())
        .bind(region.as_deref())
        .bind(region.as_deref())
        .bind(min_score)
        .fetch_all(&state.db).await?;
    let mut map = std::collections::HashMap::new();
    for id in ids {
        let (_, comp) = compute_proxy_selection_explain(state, &id, now, soft_min_score).await?;
        map.insert(id, comp);
    }
    Ok(map)
}

pub async fn compute_candidate_preview_with_reasons(
    state: &AppState,
    now: &str,
    provider: Option<&str>,
    region: Option<&str>,
    min_score: f64,
    soft_min_score: Option<f64>,
) -> Result<Vec<CandidateRankPreviewItem>> {
    let query = format!(
        "SELECT id, provider, region, score, COALESCE(cached_trust_score, 0) AS trust_score_total FROM proxies {} ORDER BY {} LIMIT 3",
        proxy_selection_base_where_sql(),
        proxy_selection_order_by_cached_trust_score_sql()
    );
    let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, f64, i64)>(&query)
        .bind(now)
         .bind(provider.as_deref())
        .bind(provider.as_deref())
        .bind(region.as_deref())
        .bind(region.as_deref())
        .bind(min_score)
        .fetch_all(&state.db).await?;
    let component_map = compute_top_candidate_component_map(state, now, provider, region, min_score, soft_min_score).await?;
    let baseline = rows.get(1).and_then(|row| component_map.get(&row.0));
    let mut out = Vec::new();
    for (idx, (id, provider, region, score, trust_score_total)) in rows.into_iter().enumerate() {
        let comp = component_map.get(&id);
        let current_components = comp.cloned().unwrap_or_else(empty_components);
        let summary = if idx == 0 {
            summarize_component_delta(&current_components, baseline)
        } else {
            summarize_component_advantages(&current_components)
        };
        let diff = if idx == 0 {
            Some(structured_component_delta(&current_components, baseline))
        } else {
            None
        };
        out.push(CandidateRankPreviewItem {
            id,
            provider,
            region,
            score,
            trust_score_total,
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
) -> Result<(Option<i64>, TrustScoreComponents)> {
    let provider_risk_query = "SELECT EXISTS(SELECT 1 FROM provider_risk_snapshots s JOIN proxies p ON p.provider = s.provider WHERE p.id = ? AND s.risk_hit != 0)";
    let provider_region_query = "SELECT EXISTS(SELECT 1 FROM provider_region_risk_snapshots s JOIN proxies p ON p.provider = s.provider AND p.region = s.region WHERE p.id = ? AND s.risk_hit != 0)";
    let row = sqlx::query_as::<_, (f64, i64, i64, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
        r#"SELECT score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, CAST(last_verify_at AS INTEGER), last_anonymity_level, last_probe_latency_ms, last_probe_error_category, last_exit_country, last_exit_region, country, region FROM proxies WHERE id = ?"#
    )
    .bind(proxy_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, last_anonymity_level, last_probe_latency_ms, last_probe_error_category, last_exit_country, last_exit_region, proxy_country, proxy_region)) = row else {
        return Ok((None, empty_components()));
    };
    let provider_risk_hit: i64 = sqlx::query_scalar(provider_risk_query).bind(proxy_id).fetch_one(&state.db).await?;
    let provider_region_cluster_hit: i64 = sqlx::query_scalar(provider_region_query).bind(proxy_id).fetch_one(&state.db).await?;
    let trust_score_total = load_proxy_trust_score(state, proxy_id, now).await?;
    let region_match_ok = match (last_exit_region.as_deref(), proxy_region.as_deref()) {
        (Some(actual), Some(expected)) => Some(actual.eq_ignore_ascii_case(expected)),
        _ => None,
    };
    let geo_match_ok = match (last_exit_country.as_deref(), proxy_country.as_deref()) {
        (Some(actual), Some(expected)) => actual.eq_ignore_ascii_case(expected),
        _ => last_verify_geo_match_ok.unwrap_or(0) != 0,
    };
    let components = computed_trust_score_components(
        &state.proxy_selection_tuning,
        score,
        success_count,
        failure_count,
        last_verify_status.as_deref(),
        geo_match_ok,
        region_match_ok,
        last_smoke_upstream_ok.unwrap_or(0) != 0,
        last_verify_at,
        last_anonymity_level.as_deref(),
        last_probe_latency_ms,
        last_probe_error_category.as_deref(),
        provider_risk_hit != 0,
        provider_region_cluster_hit != 0,
        now.parse::<i64>().unwrap_or_default(),
        soft_min_score,
    );
    Ok((trust_score_total, components))
}

async fn auto_rank_position_for_proxy(
    state: &AppState,
    now: &str,
    proxy_id: &str,
    provider: Option<&str>,
    region: Option<&str>,
    min_score: f64,
) -> Result<Option<i64>> {
    let query = format!(
        "SELECT id FROM proxies {} ORDER BY {} LIMIT 50",
        proxy_selection_base_where_sql(),
        proxy_selection_order_by_cached_trust_score_sql()
    );
    let ids = sqlx::query_scalar::<_, String>(&query)
        .bind(now)
        .bind(provider)
        .bind(provider)
        .bind(region)
        .bind(region)
        .bind(min_score)
        .fetch_all(&state.db)
        .await?;
    Ok(ids.into_iter().position(|id| id == proxy_id).map(|idx| idx as i64 + 1))
}

// Selection boundary note:
// - eligibility gate: active / cooldown / provider-region filter / min_score
// - ranking score: trust_score_total + trust_score_components ordering within eligible candidates
// explicit and sticky are currently control-flow overrides around the ranking path, not score components.
async fn resolve_network_policy_for_task(state: &AppState, payload: &mut Value) -> Result<()> {
    let Some(policy) = payload.get_mut("network_policy_json") else { return Ok(()); };
    let Some(policy_obj) = policy.as_object_mut() else { return Ok(()); };
    let mode = policy_obj.get("mode").and_then(|v| v.as_str()).unwrap_or("direct").to_string();
    if mode == "direct" {
        policy_obj.insert("proxy_resolution_status".to_string(), json!("direct"));
        policy_obj.insert("selection_reason_summary".to_string(), json!("direct mode bypasses proxy pool selection"));
        return Ok(());
    }

    let now = now_ts_string();
    let sticky_session = policy_obj.get("sticky_session").and_then(|v| v.as_str()).map(|v| v.to_string());
    let provider = policy_obj.get("provider").and_then(|v| v.as_str()).map(|v| v.to_string());
    let region = policy_obj.get("region").and_then(|v| v.as_str()).map(|v| v.to_string());
    let min_score = policy_obj.get("min_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let soft_min_score = policy_obj.get("soft_min_score").and_then(|v| v.as_f64());

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
    } else if let Some(ref sticky_session) = sticky_session {
        selection_mode = "sticky";
        sqlx::query_as::<_, (String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, f64, String)>(
            r#"SELECT p.id, p.scheme, p.host, p.port, p.username, p.password, p.region, p.country, p.provider, p.score, b.created_at
               FROM proxy_session_bindings b
               JOIN proxies p ON p.id = b.proxy_id
               WHERE b.session_key = ?
                 AND p.status = 'active'
                 AND (b.expires_at IS NULL OR CAST(b.expires_at AS INTEGER) > CAST(? AS INTEGER))
                 AND (p.cooldown_until IS NULL OR CAST(p.cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
                 AND (? IS NULL OR p.provider = ?)
                 AND (? IS NULL OR p.region = ?)
                 AND p.score >= ?
               LIMIT 1"#,
        )
        .bind(sticky_session)
        .bind(&now)
        .bind(&now)
         .bind(provider.as_deref())
        .bind(provider.as_deref())
        .bind(region.as_deref())
        .bind(region.as_deref())
        .bind(min_score)
        .fetch_optional(&state.db)
        .await?
        .map(|(id, scheme, host, port, username, password, region, country, provider, score, created_at)| {
            sticky_binding_created_at = Some(created_at);
            (id, scheme, host, port, username, password, region, country, provider, score)
        })
    } else {
        None
    };

    if row.is_none() && selection_mode != "explicit" {
        fallback_reason = Some(match selection_mode {
            "sticky" => "sticky_binding_missing_or_ineligible_then_fallback_to_auto",
            _ => "auto_primary_path",
        });
        selection_mode = "auto";
        let query = format!(
            "SELECT id, scheme, host, port, username, password, region, country, provider, score
             FROM proxies
             {}
             ORDER BY {}
             LIMIT 1",
            proxy_selection_base_where_sql(),
            proxy_selection_order_by_trust_score_sql_with_tuning(&state.proxy_selection_tuning)
        );
        row = sqlx::query_as::<_, (String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, f64)>(&query)
            .bind(&now)
             .bind(provider.as_deref())
            .bind(provider.as_deref())
            .bind(region.as_deref())
            .bind(region.as_deref())
            .bind(min_score)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .fetch_optional(&state.db)
            .await?;
    }

    if let Some((id, scheme, host, port, username, password, region, country, provider, score)) = row {
        let (trust_score_total, trust_score_components) = compute_proxy_selection_explain(state, &id, &now, soft_min_score).await?;
        let preview_provider = provider.clone();
        let preview_region = region.clone();
        let rank_proxy_id = id.clone();
        let rank_provider = provider.clone();
        let rank_region = region.clone();
        let mut resolved = resolved_proxy_json(id, scheme, host, port, username, password, region, country, provider, score);
        if let Some(obj) = resolved.as_object_mut() {
            obj.insert("trust_score_total".to_string(), trust_score_total.map_or(Value::Null, |v| json!(v)));
            obj.insert("trust_score_components".to_string(), json!(trust_score_components.clone()));
        }
        apply_proxy_resolution_metadata(policy_obj, sticky_session.as_deref(), Some(resolved));
        let preview = if selection_mode == "auto" {
            Some(compute_candidate_preview_with_reasons(state, &now, preview_provider.as_deref(), preview_region.as_deref(), min_score, soft_min_score).await?)
        } else {
            None
        };
        let would_rank_position_if_auto = if selection_mode == "explicit" || selection_mode == "sticky" {
            auto_rank_position_for_proxy(state, &now, &rank_proxy_id, rank_provider.as_deref(), rank_region.as_deref(), min_score).await?
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
                None => sticky_session.as_deref().map(|_| 0),
            }
        } else {
            None
        };
        let sticky_reuse_reason = if selection_mode == "sticky" {
            Some("sticky_binding_matched_and_candidate_still_eligible")
        } else {
            None
        };
        let soft_min_score_penalty_applied = soft_min_score.map(|threshold| score < threshold);
        let candidate_summary = preview
            .as_ref()
            .and_then(|items| items.first())
            .map(|item| item.summary.as_str());
        policy_obj.insert("selection_reason_summary".to_string(), json!(selection_reason_summary_for_mode(selection_mode, trust_score_total, candidate_summary)));
        policy_obj.insert("selection_explain".to_string(), selection_explain_json(selection_mode, fallback_reason, None, sticky_binding_age_seconds, sticky_reuse_reason, would_rank_position_if_auto, soft_min_score, soft_min_score_penalty_applied));
        policy_obj.insert("trust_score_components".to_string(), json!(trust_score_components));
        if let Some(preview) = preview {
            policy_obj.insert("candidate_rank_preview".to_string(), json!(preview));
        }
        if let Some(score) = trust_score_total {
            policy_obj.insert("trust_score_total".to_string(), json!(score));
        }
    } else {
        apply_proxy_resolution_metadata(policy_obj, sticky_session.as_deref(), None);
        let no_match_reason_code = if mode == "direct" {
            Some("direct_mode")
        } else if policy_obj.get("proxy_id").and_then(|v| v.as_str()).is_some() {
            Some("explicit_proxy_missing_or_ineligible")
        } else if sticky_session.is_some() {
            Some("sticky_binding_missing_or_ineligible")
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
        policy_obj.insert("selection_reason_summary".to_string(), json!("no eligible active proxy matched the current policy filters"));
        policy_obj.insert("selection_explain".to_string(), selection_explain_json(selection_mode, fallback_reason, no_match_reason_code, None, None, None, soft_min_score, None));
    }
    Ok(())
}

async fn upsert_proxy_session_binding(
    state: &AppState,
    payload: &Value,
    proxy: Option<&RunnerProxySelection>,
) -> Result<()> {
    let Some(sticky_session) = payload
        .get("network_policy_json")
        .and_then(|v| v.get("sticky_session"))
        .and_then(|v| v.as_str())
    else {
        return Ok(());
    };
    let Some(proxy) = proxy else {
        return Ok(());
    };

    let provider = payload
        .get("network_policy_json")
        .and_then(|v| v.get("provider"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .or_else(|| proxy.provider.clone());
    let region = payload
        .get("network_policy_json")
        .and_then(|v| v.get("region"))
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .or_else(|| proxy.region.clone());

    let now = now_ts_string();
    let expires_at = (now.parse::<u64>().unwrap_or(0) + 86400).to_string();
    sqlx::query(
        r#"INSERT INTO proxy_session_bindings (session_key, proxy_id, provider, region, last_used_at, expires_at, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(session_key) DO UPDATE SET
             proxy_id = excluded.proxy_id,
             provider = excluded.provider,
             region = excluded.region,
             last_used_at = excluded.last_used_at,
             expires_at = excluded.expires_at,
             updated_at = excluded.updated_at"#,
    )
    .bind(sticky_session)
    .bind(&proxy.id)
    .bind(&provider)
    .bind(&region)
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

    let inflight_tasks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks WHERE status IN ('queued', 'running')")
        .fetch_one(&state.db)
        .await?;

    let snapshot = ProxyPoolInventorySnapshot {
        total,
        available,
        region: target_region.clone(),
        available_in_region,
        inflight_tasks,
    };
    let policy = default_proxy_pool_growth_policy();
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

async fn update_proxy_health_after_execution(state: &AppState, proxy: Option<&RunnerProxySelection>, execution_status: RunnerOutcomeStatus) -> Result<()> {
    let Some(proxy) = proxy else { return Ok(()); };
    let now = now_ts_string();
    let (success_inc, failure_inc, cooldown_until): (i64, i64, Option<String>) = match execution_status {
        RunnerOutcomeStatus::Succeeded => (1, 0, None),
        RunnerOutcomeStatus::Failed => (0, 1, Some((now.parse::<u64>().unwrap_or(0) + 60).to_string())),
        RunnerOutcomeStatus::TimedOut => (0, 1, Some((now.parse::<u64>().unwrap_or(0) + 180).to_string())),
    };
    sqlx::query(r#"UPDATE proxies SET success_count = success_count + ?, failure_count = failure_count + ?, last_used_at = ?, last_checked_at = ?, cooldown_until = ?, score = MAX(0.0, score + ?), updated_at = ? WHERE id = ?"#)
        .bind(success_inc).bind(failure_inc).bind(&now).bind(&now).bind(&cooldown_until)
        .bind(match execution_status { RunnerOutcomeStatus::Succeeded => 0.01_f64, RunnerOutcomeStatus::Failed => -0.02_f64, RunnerOutcomeStatus::TimedOut => -0.03_f64 })
        .bind(&now).bind(&proxy.id)
        .execute(&state.db).await?;
    refresh_proxy_trust_views_for_scope(&state.db, &proxy.id, proxy.provider.as_deref(), proxy.region.as_deref()).await?;
    Ok(())
}

struct ClaimedTask {
    task_id: String,
    task_kind: String,
    input_json: String,
    fingerprint_profile: Option<RunnerFingerprintProfile>,
    requested_fingerprint_profile_id: Option<String>,
    requested_fingerprint_profile_version: Option<i64>,
    attempt: i64,
    run_id: String,
    started_at: String,
}

async fn claim_next_task<R>(state: &AppState, runner: &R, worker_label: &str) -> Result<Option<ClaimedTask>>
where
    R: TaskRunner + ?Sized,
{
    for _ in 0..runner_claim_retry_limit_from_env() {
        let started_at = now_ts_string();
        let run_id = format!("run-{}", Uuid::new_v4());

        let mut tx = state.db.begin().await?;
        let claimed = sqlx::query_as::<_, (String, String, String, Option<String>, Option<i64>, Option<String>)>(
            r#"
            WITH next_task AS (
                SELECT id
                FROM tasks
                WHERE status = ?
                ORDER BY priority DESC, COALESCE(queued_at, created_at) ASC, created_at ASC
                LIMIT 1
            )
            UPDATE tasks
            SET status = ?, started_at = ?, runner_id = ?, heartbeat_at = ?
            WHERE id = (SELECT id FROM next_task)
              AND status = ?
            RETURNING id, kind, input_json, fingerprint_profile_id, fingerprint_profile_version,
                (
                    SELECT fp.profile_json
                    FROM fingerprint_profiles fp
                    WHERE fp.id = tasks.fingerprint_profile_id
                      AND fp.status = 'active'
                      AND fp.version = tasks.fingerprint_profile_version
                ) as profile_json
            "#,
        )
        .bind(TASK_STATUS_QUEUED)
        .bind(TASK_STATUS_RUNNING)
        .bind(&started_at)
        .bind(worker_label)
        .bind(&started_at)
        .bind(TASK_STATUS_QUEUED)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((task_id, task_kind, input_json, fingerprint_profile_id, fingerprint_profile_version, fingerprint_profile_json)) = claimed else {
            tx.rollback().await?;
            return Ok(None);
        };

        let requested_fingerprint_profile_id = fingerprint_profile_id.clone();
        let requested_fingerprint_profile_version = fingerprint_profile_version;

        let attempt = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM runs WHERE task_id = ?"#)
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

        let fingerprint_profile = match (fingerprint_profile_id, fingerprint_profile_version, fingerprint_profile_json) {
            (Some(id), Some(version), Some(profile_json)) => serde_json::from_str(&profile_json)
                .ok()
                .map(|profile_json| RunnerFingerprintProfile { id, version, profile_json }),
            _ => None,
        };

        return Ok(Some(ClaimedTask {
            task_id,
            task_kind,
            input_json,
            fingerprint_profile,
            requested_fingerprint_profile_id,
            requested_fingerprint_profile_version,
            attempt,
            run_id,
            started_at,
        }));
    }

    Ok(None)
}

pub async fn reclaim_stale_running_tasks(state: &AppState, stale_after_seconds: u64) -> Result<u64> {
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

fn spawn_task_heartbeat(state: AppState, task_id: String, worker_label: String) -> (oneshot::Sender<()>, JoinHandle<()>) {
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

pub async fn run_one_task_with_runner<R>(state: &AppState, runner: &R, worker_label: &str) -> Result<bool>
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
    let fingerprint_profile = claimed.fingerprint_profile;
    let requested_fingerprint_profile_id = claimed.requested_fingerprint_profile_id;
    let requested_fingerprint_profile_version = claimed.requested_fingerprint_profile_version;
    let run_id = claimed.run_id;
    let _started_at = claimed.started_at;
    let (heartbeat_stop, heartbeat_handle) = spawn_task_heartbeat(state.clone(), task_id.clone(), worker_label.to_string());

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
        &format!("{} runner started task execution, attempt={attempt}", runner.name()),
    )
    .await?;

    match (&requested_fingerprint_profile_id, requested_fingerprint_profile_version, &fingerprint_profile) {
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

    let mut payload: Value = serde_json::from_str(&input_json).unwrap_or_else(|_| {
        json!({
            "raw_input_json": input_json,
        })
    });
    resolve_network_policy_for_task(state, &mut payload).await?;
    let proxy = extract_proxy_selection(&payload);
    let timeout_seconds = payload
        .get("timeout_seconds")
        .and_then(|value| value.as_i64())
        .filter(|value| *value > 0);

    let payload_for_binding = payload.clone();
    let proxy_for_health = proxy.clone();
    let execution = if task_kind == "verify_proxy" {
        let proxy_id = payload
            .get("proxy_id")
            .and_then(|v| v.as_str())
            .or_else(|| payload.get("network_policy_json").and_then(|v| v.get("proxy_id")).and_then(|v| v.as_str()));
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
            },
        }
    } else {
        runner
            .execute(RunnerTask {
                task_id: task_id.clone(),
                attempt,
                kind: task_kind.clone(),
                payload,
                timeout_seconds,
                fingerprint_profile,
                proxy,
            })
            .await
    };

    let _ = heartbeat_stop.send(());
    let _ = heartbeat_handle.await;

    upsert_proxy_session_binding(state, &payload_for_binding, proxy_for_health.as_ref()).await?;
    update_proxy_health_after_execution(state, proxy_for_health.as_ref(), execution.status).await?;

    let finished_at = now_ts_string();

    let (task_status, run_status, log_level, log_message) = match execution.status {
        RunnerOutcomeStatus::Succeeded => (
            TASK_STATUS_SUCCEEDED,
            RUN_STATUS_SUCCEEDED,
            "info",
            format!("{} runner finished successfully, attempt={attempt}", runner.name()),
        ),
        RunnerOutcomeStatus::Failed => (
            TASK_STATUS_FAILED,
            RUN_STATUS_FAILED,
            "error",
            format!("{} runner finished with failure, attempt={attempt}", runner.name()),
        ),
        RunnerOutcomeStatus::TimedOut => (
            TASK_STATUS_TIMED_OUT,
            RUN_STATUS_TIMED_OUT,
            "warn",
            format!("{} runner finished with timeout, attempt={attempt}", runner.name()),
        ),
    };

    let proxy_growth_explain = build_proxy_growth_explain_json(state, &payload_for_binding, proxy_for_health.as_ref()).await.ok();
    let result_json = execution.result_json.map(|mut value: serde_json::Value| {
        if let serde_json::Value::Object(ref mut obj) = value {
            let mut summaries = execution
                .summary_artifacts
                .iter()
                .map(|item| json!({
                    "category": format!("{:?}", item.category).to_lowercase(),
                    "key": item.key,
                    "source": item.source,
                    "severity": item.severity.as_str(),
                    "title": item.title,
                    "summary": item.summary,
                    "run_id": run_id,
                    "attempt": attempt,
                    "timestamp": finished_at,
                }))
                .collect::<Vec<_>>();
            if let Some(proxy_growth_explain) = proxy_growth_explain.clone() {
                summaries.push(json!({
                    "category": "selection",
                    "key": format!("{}.proxy_growth", task_kind),
                    "source": "selection.proxy_growth",
                    "severity": if proxy_growth_explain.get("health_assessment").and_then(|v| v.get("require_replenish")).and_then(|v| v.as_bool()) == Some(true) { "warn" } else { "info" },
                    "title": "proxy growth assessment",
                    "summary": format!(
                        "target_region={} selected_proxy_region={} available_ratio_percent={} require_replenish={} region_match_reason={}",
                        proxy_growth_explain.get("target_region").and_then(|v| v.as_str()).unwrap_or("none"),
                        proxy_growth_explain.get("selected_proxy_region").and_then(|v| v.as_str()).unwrap_or("none"),
                        proxy_growth_explain.get("health_assessment").and_then(|v| v.get("available_ratio_percent")).and_then(|v| v.as_i64()).unwrap_or(0),
                        proxy_growth_explain.get("health_assessment").and_then(|v| v.get("require_replenish")).and_then(|v| v.as_bool()).unwrap_or(false),
                        proxy_growth_explain.get("region_match").and_then(|v| v.get("reason")).and_then(|v| v.as_str()).unwrap_or("none"),
                    ),
                    "run_id": run_id,
                    "attempt": attempt,
                    "timestamp": finished_at,
                }));
                obj.insert("proxy_growth_explain".to_string(), proxy_growth_explain);
            }
            obj.insert("summary_artifacts".to_string(), json!(summaries));
        }
        value.to_string()
    });
    let error_message = execution.error_message;

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

    let current_task_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
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
    } else {
        insert_log(
            state,
            &format!("log-{}", Uuid::new_v4()),
            &task_id,
            Some(&run_id),
            "warn",
            &format!(
                "{} runner finished after cancel; terminal task overwrite skipped, attempt={attempt}",
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
            geo_mismatch_penalty: 0,
            region_mismatch_penalty: 0,
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
            anonymity_bonus: 0,
            latency_penalty: 0,
            exit_ip_not_public_penalty: 0,
            probe_error_penalty: 0,
            soft_min_score_penalty: 0,
        }
    }

    fn penalty_components_json() -> TrustScoreComponents {
        TrustScoreComponents {
            verify_ok_bonus: 0,
            verify_geo_match_bonus: 0,
            geo_mismatch_penalty: 8,
            region_mismatch_penalty: 4,
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
            anonymity_bonus: 0,
            latency_penalty: 0,
            exit_ip_not_public_penalty: 0,
            probe_error_penalty: 0,
            soft_min_score_penalty: 0,
        }
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
        assert_eq!(components.smoke_upstream_ok_bonus, 10);
        assert_eq!(components.raw_score_component, 8);
        assert_eq!(components.provider_risk_penalty, 5);
        assert_eq!(components.provider_region_cluster_penalty, 2);
        assert_eq!(components.anonymity_bonus, 4);
        assert_eq!(components.latency_penalty, -2);
        assert_eq!(components.exit_ip_not_public_penalty, 0);
        assert_eq!(components.probe_error_penalty, 6);
        assert_eq!(components.missing_verify_penalty, 0);
    }

    #[test]
    fn summarize_component_advantages_and_delta_expose_expected_language() {
        let summary = summarize_component_advantages(&components_json());
        assert!(summary.contains("wins on verify_ok, geo_match, upstream_ok, raw_score"));
        let penalty_summary = summarize_component_advantages(&penalty_components_json());
        assert!(penalty_summary.contains("penalized by missing_verify, stale_verify, verify_failed_heavy, verify_failed_light, verify_failed_base, history_risk, provider_risk, provider_region_risk"));

        let delta = summarize_component_delta(&components_json(), Some(&penalty_components_json()));
        assert!(delta.contains("better on"));
        assert!(!delta.contains("worse on"));
        assert!(delta.contains("wins on verify_ok, geo_match, upstream_ok, raw_score"));
    }

    #[test]
    fn structured_component_delta_orders_factors_by_magnitude_and_uses_expected_labels() {
        let current = components_json();
        let baseline = penalty_components_json();
        let delta = structured_component_delta(&current, Some(&baseline));
        assert_eq!(delta.winner_total_score, 68);
        assert_eq!(delta.runner_up_total_score, 96);
        assert_eq!(delta.score_gap, -28);
        let factors = &delta.factors;
        assert!(!factors.is_empty());
        assert!(factors.len() <= 5);
        let labels: Vec<&str> = factors.iter().map(|item| item.label.as_str()).collect();
        assert!(labels.iter().any(|label| *label == "verify_ok"));
        assert!(labels.iter().any(|label| matches!(*label, "missing_verify" | "stale_verify" | "verify_failed_heavy" | "verify_failed_light" | "verify_failed_base" | "history_risk" | "provider_risk" | "provider_region_risk")));
        let deltas: Vec<i64> = factors.iter().map(|item| item.delta.abs()).collect();
        assert!(deltas.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn structured_component_delta_without_baseline_returns_neutral_factor_bundle() {
        let current = components_json();
        let delta = structured_component_delta(&current, None);
        assert_eq!(delta.runner_up_total_score, 0);
        assert_eq!(delta.score_gap, 68);
        let factors = &delta.factors;
        assert_eq!(factors.len(), 5);
        assert!(factors.iter().all(|item| matches!(item.direction, WinnerVsRunnerUpDirection::Neutral)));
    }
}
