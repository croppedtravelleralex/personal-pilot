use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use tokio::{sync::oneshot, task::JoinHandle, time::Duration};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::network_identity::proxy_selection::{apply_proxy_resolution_metadata, proxy_selection_base_where_sql, proxy_selection_order_by_cached_trust_score_sql, proxy_selection_order_by_trust_score_sql_with_tuning, proxy_trust_score_sql_with_tuning, resolved_proxy_json};
use crate::{
    app::state::AppState,
    db::init::{refresh_cached_trust_score_for_proxy, refresh_cached_trust_scores_for_provider, refresh_cached_trust_scores_for_provider_region, refresh_provider_risk_snapshot_for_provider, refresh_provider_region_risk_snapshot_for_pair},
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

async fn load_proxy_trust_score(state: &AppState, proxy_id: &str, now: &str) -> Result<Option<i64>> {
    let value = sqlx::query_scalar::<_, i64>("SELECT cached_trust_score FROM proxies WHERE id = ? LIMIT 1")
        .bind(proxy_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(value)
}

fn selection_reason_summary_for_mode(mode: &str, trust_score_total: Option<i64>) -> String {
    match (mode, trust_score_total) {
        ("explicit", Some(score)) => format!("explicit proxy_id selected active proxy directly; current trust score snapshot={score}"),
        ("sticky", Some(score)) => format!("sticky session reused an active proxy binding; current trust score snapshot={score}"),
        ("auto", Some(score)) => format!("selected highest-ranked active proxy by trust score ordering; trust_score_total={score}"),
        ("explicit", None) => "explicit proxy_id selected active proxy directly".to_string(),
        ("sticky", None) => "sticky session reused an active proxy binding".to_string(),
        _ => "selected highest-ranked active proxy by trust score ordering".to_string(),
    }
}

pub fn computed_trust_score_components(
    tuning: &crate::network_identity::proxy_selection::ProxySelectionTuning,
    score: f64,
    success_count: i64,
    failure_count: i64,
    last_verify_status: Option<&str>,
    last_verify_geo_match_ok: bool,
    last_smoke_upstream_ok: bool,
    last_verify_at: Option<i64>,
    provider_risk_hit: bool,
    provider_region_cluster_hit: bool,
    now_ts: i64,
) -> Value {
    let heavy_failed = matches!(last_verify_status, Some("failed"))
        && last_verify_at.map(|ts| ts >= now_ts - tuning.recent_failure_heavy_window_seconds).unwrap_or(false);
    let light_failed = matches!(last_verify_status, Some("failed"))
        && !heavy_failed
        && last_verify_at.map(|ts| ts >= now_ts - tuning.recent_failure_light_window_seconds).unwrap_or(false);
    let base_failed = matches!(last_verify_status, Some("failed")) && !heavy_failed && !light_failed;
    let missing_verify = last_verify_at.is_none();
    let stale_verify = last_verify_at.map(|ts| ts <= now_ts - tuning.stale_after_seconds).unwrap_or(false) && !missing_verify;
    let individual_penalty = if failure_count >= success_count + tuning.provider_failure_margin.saturating_sub(2).max(1) { 18 } else if failure_count > success_count { 8 } else { 0 };
    json!({
        "verify_ok_bonus": if matches!(last_verify_status, Some("ok")) { tuning.verify_ok_bonus } else { 0 },
        "verify_geo_match_bonus": if last_verify_geo_match_ok { tuning.verify_geo_match_bonus } else { 0 },
        "smoke_upstream_ok_bonus": if last_smoke_upstream_ok { tuning.smoke_upstream_ok_bonus } else { 0 },
        "verify_failed_heavy_penalty": if heavy_failed { tuning.verify_failed_heavy_penalty } else { 0 },
        "verify_failed_light_penalty": if light_failed { tuning.verify_failed_light_penalty } else { 0 },
        "verify_failed_base_penalty": if base_failed { tuning.verify_failed_base_penalty } else { 0 },
        "missing_verify_penalty": if missing_verify { tuning.missing_verify_penalty } else { 0 },
        "stale_verify_penalty": if stale_verify { tuning.stale_verify_penalty } else { 0 },
        "individual_history_penalty": individual_penalty,
        "provider_risk_penalty": if provider_risk_hit { 10 } else { 0 },
        "provider_region_cluster_penalty": if provider_region_cluster_hit { 12 } else { 0 },
        "raw_score_component": (score * tuning.raw_score_weight_tenths as f64).floor() as i64
    })
}

pub fn summarize_component_advantages(components: &Value) -> String {
    let obj = match components.as_object() {
        Some(v) => v,
        None => return "no component detail available".to_string(),
    };
    let mut wins: Vec<&str> = Vec::new();
    let mut losses: Vec<&str> = Vec::new();

    let get_i = |k: &str| obj.get(k).and_then(|v| v.as_i64()).unwrap_or(0);

    if get_i("verify_ok_bonus") > 0 { wins.push("verify_ok"); }
    if get_i("verify_geo_match_bonus") > 0 { wins.push("geo_match"); }
    if get_i("smoke_upstream_ok_bonus") > 0 { wins.push("upstream_ok"); }
    if get_i("raw_score_component") >= 8 { wins.push("raw_score"); }

    if get_i("missing_verify_penalty") > 0 { losses.push("missing_verify"); }
    if get_i("stale_verify_penalty") > 0 { losses.push("stale_verify"); }
    if get_i("verify_failed_heavy_penalty") > 0 || get_i("verify_failed_light_penalty") > 0 || get_i("verify_failed_base_penalty") > 0 { losses.push("verify_failure"); }
    if get_i("individual_history_penalty") > 0 { losses.push("history_risk"); }
    if get_i("provider_risk_penalty") > 0 { losses.push("provider_risk"); }
    if get_i("provider_region_cluster_penalty") > 0 { losses.push("provider_region_risk"); }

    match (wins.is_empty(), losses.is_empty()) {
        (false, false) => format!("wins on {}; penalized by {}", wins.join(", "), losses.join(", ")),
        (false, true) => format!("wins on {}", wins.join(", ")),
        (true, false) => format!("penalized by {}", losses.join(", ")),
        (true, true) => "mostly driven by neutral/default signals".to_string(),
    }
}

fn component_label(key: &str) -> &'static str {
    match key {
        "verify_ok_bonus" => "verify_ok",
        "verify_geo_match_bonus" => "geo_match",
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
        _ => "unknown",
    }
}

pub fn structured_component_delta(current: &Value, baseline: Option<&Value>) -> Value {
    let keys = [
        "verify_ok_bonus", "verify_geo_match_bonus", "smoke_upstream_ok_bonus", "raw_score_component",
        "missing_verify_penalty", "stale_verify_penalty", "verify_failed_heavy_penalty", "verify_failed_light_penalty",
        "verify_failed_base_penalty", "individual_history_penalty", "provider_risk_penalty", "provider_region_cluster_penalty"
    ];
    let positive = ["verify_ok_bonus", "verify_geo_match_bonus", "smoke_upstream_ok_bonus", "raw_score_component"];
    let winner_total_score = keys.iter().map(|key| current.get(*key).and_then(|v| v.as_i64()).unwrap_or(0)).sum::<i64>();
    let Some(baseline) = baseline else {
        let mut factors: Vec<Value> = keys.into_iter().map(|key| json!({
            "factor": key,
            "label": component_label(key),
            "winner_value": current.get(key).and_then(|v| v.as_i64()).unwrap_or(0),
            "runner_up_value": 0,
            "delta": current.get(key).and_then(|v| v.as_i64()).unwrap_or(0),
            "direction": "neutral",
        })).collect();
        factors.sort_by_key(|v| std::cmp::Reverse(v.get("delta").and_then(|v| v.as_i64()).unwrap_or(0).abs()));
        factors.truncate(5);
        return json!({
            "winner_total_score": winner_total_score,
            "runner_up_total_score": 0,
            "score_gap": winner_total_score,
            "factors": factors
        });
    };
    let c = match current.as_object() { Some(v) => v, None => return Value::Null };
    let b = match baseline.as_object() { Some(v) => v, None => return Value::Null };
    let runner_up_total_score = keys.iter().map(|key| baseline.get(*key).and_then(|v| v.as_i64()).unwrap_or(0)).sum::<i64>();
    let mut factors = Vec::new();
    for key in keys {
        let cv = c.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        let bv = b.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        let (delta, direction) = if positive.contains(&key) {
            let d = cv - bv;
            let dir = if d > 0 { "winner" } else if d < 0 { "runner_up" } else { "neutral" };
            (d, dir)
        } else {
            let d = bv - cv;
            let dir = if d > 0 { "winner" } else if d < 0 { "runner_up" } else { "neutral" };
            (d, dir)
        };
        factors.push(json!({
            "factor": key,
            "label": component_label(key),
            "winner_value": cv,
            "runner_up_value": bv,
            "delta": delta,
            "direction": direction,
        }));
    }
    factors.sort_by_key(|v| std::cmp::Reverse(v.get("delta").and_then(|v| v.as_i64()).unwrap_or(0).abs()));
    factors.truncate(5);
    json!({
        "winner_total_score": winner_total_score,
        "runner_up_total_score": runner_up_total_score,
        "score_gap": winner_total_score - runner_up_total_score,
        "factors": factors
    })
}

pub fn summarize_component_delta(current: &Value, baseline: Option<&Value>) -> String {
    let current_summary = summarize_component_advantages(current);
    let Some(baseline) = baseline else {
        return current_summary;
    };
    let c = current.as_object();
    let b = baseline.as_object();
    let (Some(c), Some(b)) = (c, b) else { return current_summary; };

    let mut better = Vec::new();
    let mut worse = Vec::new();
    for key in [
        "verify_ok_bonus", "verify_geo_match_bonus", "smoke_upstream_ok_bonus", "raw_score_component",
        "missing_verify_penalty", "stale_verify_penalty", "verify_failed_heavy_penalty", "verify_failed_light_penalty",
        "verify_failed_base_penalty", "individual_history_penalty", "provider_risk_penalty", "provider_region_cluster_penalty"
    ] {
        let cv = c.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        let bv = b.get(key).and_then(|v| v.as_i64()).unwrap_or(0);
        if ["verify_ok_bonus", "verify_geo_match_bonus", "smoke_upstream_ok_bonus", "raw_score_component"].contains(&key) {
            if cv > bv { better.push(key); }
            else if cv < bv { worse.push(key); }
        } else {
            if cv < bv { better.push(key); }
            else if cv > bv { worse.push(key); }
        }
    }
    let mut parts = Vec::new();
    if !better.is_empty() { parts.push(format!("better on {}", better.join(", "))); }
    if !worse.is_empty() { parts.push(format!("worse on {}", worse.join(", "))); }
    if parts.is_empty() { current_summary } else { format!("{}; {}", current_summary, parts.join("; ")) }
}

async fn compute_top_candidate_component_map(
    state: &AppState,
    now: &str,
    provider: Option<&str>,
    region: Option<&str>,
    min_score: f64,
) -> Result<std::collections::HashMap<String, Value>> {
    let query = format!(
        "SELECT id FROM proxies {} ORDER BY {} LIMIT 3",
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
        .fetch_all(&state.db).await?;
    let mut map = std::collections::HashMap::new();
    for id in ids {
        let (_, comp) = compute_proxy_selection_explain(state, &id, now).await?;
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
) -> Result<Vec<Value>> {
    let query = format!(
        "SELECT id, provider, region, score, COALESCE(cached_trust_score, 0) AS trust_score_total FROM proxies {} ORDER BY {} LIMIT 3",
        proxy_selection_base_where_sql(),
        proxy_selection_order_by_cached_trust_score_sql()
    );
    let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, f64, i64)>(&query)
        .bind(now)
        .bind(provider)
        .bind(provider)
        .bind(region)
        .bind(region)
        .bind(min_score)
        .fetch_all(&state.db).await?;
    let component_map = compute_top_candidate_component_map(state, now, provider, region, min_score).await?;
    let baseline = rows.get(1).and_then(|row| component_map.get(&row.0));
    let mut out = Vec::new();
    for (idx, (id, provider, region, score, trust_score_total)) in rows.into_iter().enumerate() {
        let comp = component_map.get(&id);
        let summary = if idx == 0 {
            summarize_component_delta(comp.unwrap_or(&Value::Null), baseline)
        } else {
            summarize_component_advantages(comp.unwrap_or(&Value::Null))
        };
        let diff = if idx == 0 {
            structured_component_delta(comp.unwrap_or(&Value::Null), baseline)
        } else {
            Value::Null
        };
        out.push(json!({
            "id": id,
            "provider": provider,
            "region": region,
            "score": score,
            "trust_score_total": trust_score_total,
            "summary": summary,
            "winner_vs_runner_up_diff": diff,
        }));
    }
    Ok(out)
}

async fn compute_proxy_selection_explain(
    state: &AppState,
    proxy_id: &str,
    now: &str,
) -> Result<(Option<i64>, Value)> {
    let provider_risk_query = "SELECT EXISTS(SELECT 1 FROM provider_risk_snapshots s JOIN proxies p ON p.provider = s.provider WHERE p.id = ? AND s.risk_hit != 0)";
    let provider_region_query = "SELECT EXISTS(SELECT 1 FROM provider_region_risk_snapshots s JOIN proxies p ON p.provider = s.provider AND p.region = s.region WHERE p.id = ? AND s.risk_hit != 0)";
    let row = sqlx::query_as::<_, (f64, i64, i64, Option<String>, Option<i64>, Option<i64>, Option<i64>)>(
        r#"SELECT score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, CAST(last_verify_at AS INTEGER) FROM proxies WHERE id = ?"#
    )
    .bind(proxy_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at)) = row else {
        return Ok((None, Value::Null));
    };
    let provider_risk_hit: i64 = sqlx::query_scalar(provider_risk_query).bind(proxy_id).fetch_one(&state.db).await?;
    let provider_region_cluster_hit: i64 = sqlx::query_scalar(provider_region_query).bind(proxy_id).fetch_one(&state.db).await?;
    let trust_score_total = load_proxy_trust_score(state, proxy_id, now).await?;
    let components = computed_trust_score_components(
        &state.proxy_selection_tuning,
        score,
        success_count,
        failure_count,
        last_verify_status.as_deref(),
        last_verify_geo_match_ok.unwrap_or(0) != 0,
        last_smoke_upstream_ok.unwrap_or(0) != 0,
        last_verify_at,
        provider_risk_hit != 0,
        provider_region_cluster_hit != 0,
        now.parse::<i64>().unwrap_or_default(),
    );
    Ok((trust_score_total, components))
}

async fn resolve_network_policy_for_task(state: &AppState, payload: &mut Value) -> Result<()> {
    let Some(policy) = payload.get_mut("network_policy_json") else { return Ok(()); };
    let Some(policy_obj) = policy.as_object_mut() else { return Ok(()); };
    let mode = policy_obj.get("mode").and_then(|v| v.as_str()).unwrap_or("direct");
    if mode == "direct" {
        policy_obj.insert("proxy_resolution_status".to_string(), json!("direct"));
        policy_obj.insert("selection_reason_summary".to_string(), json!("direct mode bypasses proxy pool selection"));
        return Ok(());
    }

    let now = now_ts_string();
    let sticky_session = policy_obj.get("sticky_session").and_then(|v| v.as_str()).map(|v| v.to_string());
    let provider = policy_obj.get("provider").and_then(|v| v.as_str());
    let region = policy_obj.get("region").and_then(|v| v.as_str());
    let min_score = policy_obj.get("min_score").and_then(|v| v.as_f64()).unwrap_or(0.0);

    let mut selection_mode = "auto";
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
        sqlx::query_as::<_, (String, String, String, i64, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, f64)>(
            r#"SELECT p.id, p.scheme, p.host, p.port, p.username, p.password, p.region, p.country, p.provider, p.score
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
        .bind(provider)
        .bind(provider)
        .bind(region)
        .bind(region)
        .bind(min_score)
        .fetch_optional(&state.db)
        .await?
    } else {
        None
    };

    if row.is_none() && selection_mode != "explicit" {
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
            .bind(provider)
            .bind(provider)
            .bind(region)
            .bind(region)
            .bind(min_score)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .bind(&now)
            .fetch_optional(&state.db)
            .await?;
    }

    if let Some((id, scheme, host, port, username, password, region, country, provider, score)) = row {
        let (trust_score_total, trust_score_components) = compute_proxy_selection_explain(state, &id, &now).await?;
        let preview_provider = provider.clone();
        let preview_region = region.clone();
        let mut resolved = resolved_proxy_json(id, scheme, host, port, username, password, region, country, provider, score);
        if let Some(obj) = resolved.as_object_mut() {
            obj.insert("trust_score_total".to_string(), trust_score_total.map_or(Value::Null, |v| json!(v)));
            obj.insert("trust_score_components".to_string(), trust_score_components.clone());
        }
        apply_proxy_resolution_metadata(policy_obj, sticky_session.as_deref(), Some(resolved));
        policy_obj.insert("selection_reason_summary".to_string(), json!(selection_reason_summary_for_mode(selection_mode, trust_score_total)));
        policy_obj.insert("trust_score_components".to_string(), trust_score_components);
        if selection_mode == "auto" {
            let preview = compute_candidate_preview_with_reasons(state, &now, preview_provider.as_deref(), preview_region.as_deref(), min_score).await?;
            policy_obj.insert("candidate_rank_preview".to_string(), json!(preview));
        }
        if let Some(score) = trust_score_total {
            policy_obj.insert("trust_score_total".to_string(), json!(score));
        }
    } else {
        apply_proxy_resolution_metadata(policy_obj, sticky_session.as_deref(), None);
        policy_obj.insert("selection_reason_summary".to_string(), json!("no eligible active proxy matched the current policy filters"));
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
    refresh_provider_risk_snapshot_for_provider(&state.db, proxy.provider.as_deref()).await?;
    refresh_provider_region_risk_snapshot_for_pair(&state.db, proxy.provider.as_deref(), proxy.region.as_deref()).await?;
    refresh_cached_trust_scores_for_provider(&state.db, proxy.provider.as_deref()).await?;
    refresh_cached_trust_scores_for_provider_region(&state.db, proxy.provider.as_deref(), proxy.region.as_deref()).await?;
    refresh_cached_trust_score_for_proxy(&state.db, &proxy.id).await?;
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
            INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)
            VALUES (?, ?, ?, ?, ?, ?, NULL, NULL)
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
                },
                Err((_status, message)) => RunnerExecutionResult {
                    status: RunnerOutcomeStatus::Failed,
                    result_json: Some(json!({"proxy_id": proxy_id, "status": "failed", "message": message})),
                    error_message: Some(message),
                },
            },
            None => RunnerExecutionResult {
                status: RunnerOutcomeStatus::Failed,
                result_json: Some(json!({"status": "failed", "message": "verify_proxy task requires proxy_id"})),
                error_message: Some("verify_proxy task requires proxy_id".to_string()),
            },
        }
    } else {
        runner
            .execute(RunnerTask {
                task_id: task_id.clone(),
                attempt,
                kind: task_kind,
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

    let result_json = execution.result_json.map(|value: serde_json::Value| value.to_string());
    let error_message = execution.error_message;

    let run_update = sqlx::query(
        &format!(
            "UPDATE runs SET status = ?, finished_at = ?, error_message = ? WHERE id = ? AND status = '{}'",
            RUN_STATUS_RUNNING,
        ),
    )
    .bind(run_status)
    .bind(&finished_at)
    .bind(&error_message)
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
