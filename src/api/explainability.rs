use serde_json::Value;
use std::time::Instant;

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

use super::dto::{
    CandidateRankPreviewItem, ConsumptionExplain, ExecutionIdentity, FingerprintRuntimeExplain,
    IdentityNetworkExplain, ProxySelectionExplain, SummaryArtifactResponse, TaskResponse,
    WinnerVsRunnerUpDiff,
};

fn parse_result_json(result_json: Option<&str>) -> Option<Value> {
    result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok())
}

fn parse_optional_json(raw: Option<&str>) -> Option<Value> {
    raw.and_then(|value| serde_json::from_str::<Value>(value).ok())
}

fn string_field_from_parsed(parsed: Option<&Value>, key: &str) -> Option<String> {
    parsed?
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn i64_field_from_parsed(parsed: Option<&Value>, key: &str) -> Option<i64> {
    parsed?.get(key).and_then(|value| value.as_i64())
}

fn behavior_runtime_explain_from_parsed(
    parsed: Option<&Value>,
) -> Option<crate::behavior::BehaviorRuntimeExplain> {
    parsed
        .and_then(|value| value.get("behavior_runtime_explain").cloned())
        .and_then(|value| serde_json::from_value(value).ok())
}

fn behavior_trace_summary_from_parsed(
    parsed: Option<&Value>,
) -> Option<crate::behavior::BehaviorTraceSummary> {
    parsed
        .and_then(|value| value.get("behavior_trace_summary").cloned())
        .and_then(|value| serde_json::from_value(value).ok())
}

fn fingerprint_resolution_status_from_parsed(
    fingerprint_profile_id: Option<&str>,
    fingerprint_profile_version: Option<i64>,
    parsed: Option<&Value>,
) -> Option<String> {
    let profile_id = fingerprint_profile_id?;
    let profile_version = fingerprint_profile_version?;

    if parsed
        .and_then(|json| json.get("fingerprint_profile"))
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        == Some(profile_id)
        && parsed
            .and_then(|json| json.get("fingerprint_profile"))
            .and_then(|value| value.get("version"))
            .and_then(|value| value.as_i64())
            == Some(profile_version)
    {
        return Some("resolved".to_string());
    }

    if parsed
        .and_then(|json| json.get("fingerprint_profile"))
        .map(|value| value.is_null())
        == Some(true)
    {
        return Some("downgraded".to_string());
    }

    Some("pending".to_string())
}

fn proxy_resolution_status_from_parsed(parsed: Option<&Value>) -> Option<String> {
    parsed?
        .get("proxy")
        .and_then(|value| value.get("resolution_status"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| {
            parsed
                .and_then(|value| value.get("payload"))
                .and_then(|value| value.get("network_policy_json"))
                .and_then(|value| value.get("proxy_resolution_status"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
}

fn proxy_identity_from_parsed(
    parsed: Option<&Value>,
) -> (Option<String>, Option<String>, Option<String>) {
    let proxy = parsed.and_then(|json| json.get("proxy"));
    (
        proxy
            .and_then(|value| value.get("id"))
            .and_then(|value| value.as_str())
            .map(|v| v.to_string()),
        proxy
            .and_then(|value| value.get("provider"))
            .and_then(|value| value.as_str())
            .map(|v| v.to_string()),
        proxy
            .and_then(|value| value.get("region"))
            .and_then(|value| value.as_str())
            .map(|v| v.to_string()),
    )
}

fn selection_reason_summary_from_parsed(parsed: Option<&Value>) -> Option<String> {
    parsed?
        .get("payload")
        .and_then(|value| value.get("network_policy_json"))
        .and_then(|value| value.get("selection_reason_summary"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn selection_explain_from_parsed(parsed: Option<&Value>) -> Option<ProxySelectionExplain> {
    parsed?
        .get("payload")
        .and_then(|value| value.get("network_policy_json"))
        .and_then(|value| value.get("selection_explain").cloned())
        .and_then(|value| serde_json::from_value::<ProxySelectionExplain>(value).ok())
}

fn fingerprint_runtime_explain_from_parsed(
    parsed: Option<&Value>,
) -> Option<FingerprintRuntimeExplain> {
    let parsed = parsed?;
    let runtime_explain = parsed.get("fingerprint_runtime_explain");
    let runtime = parsed.get("fingerprint_runtime");

    if let Some(value) = runtime_explain.cloned() {
        if let Ok(explain) = serde_json::from_value::<FingerprintRuntimeExplain>(value) {
            return Some(explain);
        }
    }

    let budget_tag = runtime_explain
        .and_then(|v| v.get("fingerprint_budget_tag"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let fingerprint_consistency = runtime_explain
        .and_then(|v| v.get("fingerprint_consistency"))
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok());
    let consumption_source_of_truth = runtime_explain
        .and_then(|v| v.get("consumption_source_of_truth"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            runtime
                .and_then(|v| v.get("consumption_source_of_truth"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
    let consumption_version = runtime_explain
        .and_then(|v| v.get("consumption_version"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            runtime
                .and_then(|v| v.get("consumption_version"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
    let consumption_status = runtime_explain
        .and_then(|v| v.get("consumption_status"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            runtime
                .and_then(|v| v.get("consumption_status"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });
    let warning = runtime_explain
        .and_then(|v| v.get("warning"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .or_else(|| {
            runtime
                .and_then(|v| v.get("warning"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        });

    let consumption_explain = runtime_explain
        .and_then(|v| v.get("consumption_explain").cloned())
        .or_else(|| runtime.and_then(|v| v.get("consumption_explain").cloned()))
        .and_then(|v| serde_json::from_value::<ConsumptionExplain>(v).ok());

    if budget_tag.is_none()
        && fingerprint_consistency.is_none()
        && consumption_source_of_truth.is_none()
        && consumption_version.is_none()
        && consumption_status.is_none()
        && warning.is_none()
        && consumption_explain.is_none()
    {
        return None;
    }

    Some(FingerprintRuntimeExplain {
        fingerprint_budget_tag: budget_tag,
        fingerprint_consistency,
        consumption_source_of_truth,
        consumption_version,
        consumption_status,
        warning,
        consumption_explain,
    })
}

fn trust_score_total_from_parsed(parsed: Option<&Value>) -> Option<i64> {
    parsed?
        .get("payload")
        .and_then(|value| value.get("network_policy_json"))
        .and_then(|value| value.get("trust_score_total"))
        .and_then(|value| value.as_i64())
}

fn identity_session_status_from_parsed(parsed: Option<&Value>) -> Option<String> {
    parsed?
        .get("identity_session_status")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| {
            parsed
                .and_then(|value| value.get("payload"))
                .and_then(|value| value.get("network_policy_json"))
                .and_then(|value| value.get("identity_session_status"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
}

fn count_from_parsed(parsed: Option<&Value>, key: &str) -> Option<i64> {
    parsed?
        .get(key)
        .and_then(|value| value.as_i64())
        .or_else(|| {
            parsed
                .and_then(|value| value.get("payload"))
                .and_then(|value| value.get("network_policy_json"))
                .and_then(|value| value.get(key))
                .and_then(|value| value.as_i64())
        })
}

fn candidate_rank_preview_from_parsed(parsed: Option<&Value>) -> Vec<CandidateRankPreviewItem> {
    parsed
        .and_then(|value| value.get("payload").cloned())
        .and_then(|value| value.get("network_policy_json").cloned())
        .and_then(|value| value.get("candidate_rank_preview").cloned())
        .and_then(|value| serde_json::from_value::<Vec<CandidateRankPreviewItem>>(value).ok())
        .unwrap_or_default()
}

pub fn content_string_field(parsed: Option<&Value>, key: &str) -> Option<String> {
    parsed?
        .get(key)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

pub fn content_i64_field(parsed: Option<&Value>, key: &str) -> Option<i64> {
    parsed?.get(key).and_then(|value| value.as_i64())
}

pub fn content_bool_field(parsed: Option<&Value>, key: &str) -> Option<bool> {
    parsed?.get(key).and_then(|value| value.as_bool())
}

#[derive(Debug, Clone)]
pub struct TaskExplainability {
    pub fingerprint_resolution_status: Option<String>,
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
    pub trust_score_total: Option<i64>,
    pub selection_reason_summary: Option<String>,
    pub selection_explain: Option<ProxySelectionExplain>,
    pub fingerprint_runtime_explain: Option<FingerprintRuntimeExplain>,
    pub execution_identity: ExecutionIdentity,
    pub identity_network_explain: Option<IdentityNetworkExplain>,
    pub winner_vs_runner_up_diff: Option<WinnerVsRunnerUpDiff>,
    pub failure_scope: Option<String>,
    pub browser_failure_signal: Option<String>,
    pub summary_artifacts: Vec<SummaryArtifactResponse>,
}

pub fn fingerprint_resolution_status(
    fingerprint_profile_id: Option<&str>,
    fingerprint_profile_version: Option<i64>,
    result_json: Option<&str>,
) -> Option<String> {
    let parsed = parse_result_json(result_json);
    fingerprint_resolution_status_from_parsed(
        fingerprint_profile_id,
        fingerprint_profile_version,
        parsed.as_ref(),
    )
}

pub fn proxy_resolution_status(result_json: Option<&str>) -> Option<String> {
    let parsed = parse_result_json(result_json);
    proxy_resolution_status_from_parsed(parsed.as_ref())
}

pub fn proxy_identity(
    result_json: Option<&str>,
) -> (Option<String>, Option<String>, Option<String>) {
    let parsed = parse_result_json(result_json);
    proxy_identity_from_parsed(parsed.as_ref())
}

pub fn selection_reason_summary(result_json: Option<&str>) -> Option<String> {
    let parsed = parse_result_json(result_json);
    selection_reason_summary_from_parsed(parsed.as_ref())
}

pub fn selection_explain(result_json: Option<&str>) -> Option<ProxySelectionExplain> {
    let parsed = parse_result_json(result_json);
    selection_explain_from_parsed(parsed.as_ref())
}

pub fn fingerprint_runtime_explain(result_json: Option<&str>) -> Option<FingerprintRuntimeExplain> {
    let parsed = parse_result_json(result_json);
    fingerprint_runtime_explain_from_parsed(parsed.as_ref())
}

pub fn trust_score_total(result_json: Option<&str>) -> Option<i64> {
    let parsed = parse_result_json(result_json);
    trust_score_total_from_parsed(parsed.as_ref())
}

pub fn candidate_rank_preview(result_json: Option<&str>) -> Vec<CandidateRankPreviewItem> {
    let parsed = parse_result_json(result_json);
    candidate_rank_preview_from_parsed(parsed.as_ref())
}

pub fn winner_vs_runner_up_diff(result_json: Option<&str>) -> Option<WinnerVsRunnerUpDiff> {
    candidate_rank_preview(result_json)
        .into_iter()
        .next()
        .and_then(|item| item.winner_vs_runner_up_diff)
}

fn normalize_summary_category(category: &str) -> String {
    match category {
        "execution" | "summary" | "result" | "debug" | "transient" => category.to_string(),
        _ => "summary".to_string(),
    }
}

fn normalize_summary_source(source: Option<&str>) -> String {
    match source.unwrap_or("runner.unknown") {
        "proxy_selection" => "selection.proxy".to_string(),
        "fake_runner" => "runner.fake".to_string(),
        "lightpanda_runner" => "runner.lightpanda".to_string(),
        other => other.to_string(),
    }
}

fn normalize_summary_severity(severity: Option<&str>) -> String {
    match severity.unwrap_or("info") {
        "error" => "error".to_string(),
        "warning" | "warn" => "warning".to_string(),
        "info" => "info".to_string(),
        _ => "info".to_string(),
    }
}

fn humanize_proxy_growth_reason(reason: &str) -> &'static str {
    match reason {
        "exact_region_match" => "exact region match",
        "region_mismatch" => "region mismatch",
        "proxy_region_missing" => "proxy region missing",
        "target_region_not_requested" => "target region not requested",
        "no_region_constraint" => "no region constraint",
        other => {
            let _ = other;
            "proxy growth signal"
        }
    }
}

fn humanize_selection_factor_label(label: &str) -> &'static str {
    match label {
        "verify_ok" => "verify ok",
        "geo_match" => "geo matched",
        "geo_mismatch" => "country mismatch risk",
        "region_mismatch" => "region mismatch risk",
        "geo_risk" => "geo risk",
        "upstream_ok" => "upstream reachable",
        "raw_score" => "base score",
        "missing_verify" => "missing verify",
        "stale_verify" => "stale verify",
        "verify_failed_heavy" => "recent verify heavy failure",
        "verify_failed_light" => "recent verify light failure",
        "verify_failed_base" => "historical verify failure",
        "history_risk" => "history risk",
        "provider_risk" => "provider risk",
        "provider_region_risk" => "provider-region risk",
        "anonymity" => "anonymity quality",
        "probe_latency" => "probe latency",
        "exit_ip_not_public" => "non-public exit ip risk",
        "probe_error_category" => "probe error risk",
        "verify_risk" => "verify risk",
        "soft_min_score" => "soft min-score penalty",
        _ => "selection factor",
    }
}

fn selection_decision_summary_artifact_from_parsed(
    parsed: Option<&Value>,
) -> Option<SummaryArtifactResponse> {
    let started = Instant::now();
    let diff = candidate_rank_preview_from_parsed(parsed)
        .into_iter()
        .next()
        .and_then(|item| item.winner_vs_runner_up_diff)?;
    let factor_summary = diff
        .factors
        .iter()
        .take(2)
        .map(|factor| {
            format!(
                "{}({:+})",
                humanize_selection_factor_label(&factor.label),
                factor.delta
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let summary = if factor_summary.is_empty() {
        format!(
            "this proxy stayed ahead by {} trust-score points",
            diff.score_gap
        )
    } else {
        format!(
            "this proxy stayed ahead by {} trust-score points; biggest score drivers: {}",
            diff.score_gap, factor_summary
        )
    };
    let artifact = SummaryArtifactResponse {
        category: "summary".to_string(),
        key: "proxy.selection.decision".to_string(),
        source: "selection.proxy".to_string(),
        severity: "info".to_string(),
        title: "proxy selection decision".to_string(),
        summary,
        task_id: None,
        task_kind: None,
        task_status: None,
        run_id: None,
        attempt: None,
        timestamp: None,
    };
    perf_probe_log(
        "selection_decision_summary_artifact",
        &[
            ("elapsed_ms", started.elapsed().as_millis().to_string()),
            ("score_gap", diff.score_gap.to_string()),
            ("factor_count", diff.factors.len().to_string()),
        ],
    );
    Some(artifact)
}

fn identity_network_summary_artifact_from_parsed(
    parsed: Option<&Value>,
) -> Option<SummaryArtifactResponse> {
    let proxy_resolution_status = proxy_resolution_status_from_parsed(parsed);
    let (proxy_id, proxy_provider, proxy_region) = proxy_identity_from_parsed(parsed);
    let selection_reason_summary = selection_reason_summary_from_parsed(parsed);
    let fingerprint_runtime_explain = fingerprint_runtime_explain_from_parsed(parsed);
    let mut parts = Vec::new();
    if let Some(provider) = proxy_provider.as_deref() {
        if let Some(region) = proxy_region.as_deref() {
            parts.push(format!("proxy {}@{}", provider, region));
        } else {
            parts.push(format!("proxy {}", provider));
        }
    } else if let Some(proxy_id) = proxy_id.as_deref() {
        parts.push(format!("proxy {}", proxy_id));
    }
    if let Some(status) = proxy_resolution_status.as_deref() {
        parts.push(format!("proxy resolution {}", status));
    }
    if let Some(tag) = fingerprint_runtime_explain
        .as_ref()
        .and_then(|v| v.fingerprint_budget_tag.as_deref())
    {
        parts.push(format!("fingerprint budget {}", tag));
    }
    if let Some(consumption) = fingerprint_runtime_explain
        .as_ref()
        .and_then(|v| v.consumption_explain.as_ref())
    {
        parts.push(format!(
            "fingerprint consumption {} (declared {}, applied {}, ignored {})",
            consumption.consumption_status,
            consumption.declared_count,
            consumption.applied_count,
            consumption.ignored_count,
        ));
    }
    if let Some(summary) = selection_reason_summary.as_deref() {
        let short = summary
            .split(';')
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(summary);
        parts.push(format!("selection summary {}", short));
    }
    if parts.is_empty() {
        return None;
    }
    Some(SummaryArtifactResponse {
        category: "summary".to_string(),
        key: "identity.network.summary".to_string(),
        source: "selection.identity_network".to_string(),
        severity: "info".to_string(),
        title: "identity and network summary".to_string(),
        summary: parts.join("; "),
        task_id: None,
        task_kind: None,
        task_status: None,
        run_id: None,
        attempt: None,
        timestamp: None,
    })
}

fn health_band_phrase(band: &str) -> &'static str {
    match band {
        "below_min" => "below target band",
        "above_max" => "above target band",
        "within_band" => "within target band",
        _ => "unknown band",
    }
}

fn proxy_growth_summary_artifact_from_parsed(
    parsed: Option<&Value>,
) -> Option<SummaryArtifactResponse> {
    let growth = selection_explain_from_parsed(parsed)?.proxy_growth?;
    let target_region = growth.target_region.as_deref().unwrap_or("none");
    let selected_proxy_region = growth.selected_proxy_region.as_deref().unwrap_or("none");
    let health = growth.health_assessment.as_ref();
    let available_ratio_percent = health.map(|v| v.available_ratio_percent).unwrap_or(0);
    let require_replenish = health.map(|v| v.require_replenish).unwrap_or(false);
    let health_band = health
        .map(|v| health_band_phrase(&v.healthy_ratio_band))
        .unwrap_or("unknown band");
    let region_match_reason = growth
        .region_match
        .as_ref()
        .map(|v| humanize_proxy_growth_reason(&v.reason))
        .unwrap_or("proxy growth signal");
    Some(SummaryArtifactResponse {
        category: "summary".to_string(),
        key: "proxy.selection.proxy_growth".to_string(),
        source: "selection.proxy_growth".to_string(),
        severity: if require_replenish {
            "warning".to_string()
        } else {
            "info".to_string()
        },
        title: "proxy growth assessment".to_string(),
        summary: if require_replenish {
            format!(
                "pool needs replenishment for this request; target region {} ; selected region {} ; availability {}% ({}) ; region fit {}",
                target_region,
                selected_proxy_region,
                available_ratio_percent,
                health_band,
                region_match_reason,
            )
        } else {
            format!(
                "pool is healthy for this request; target region {} ; selected region {} ; availability {}% ({}) ; region fit {}",
                target_region,
                selected_proxy_region,
                available_ratio_percent,
                health_band,
                region_match_reason,
            )
        },
        task_id: None,
        task_kind: None,
        task_status: None,
        run_id: None,
        attempt: None,
        timestamp: None,
    })
}

fn browser_result_summary_artifact_from_parsed(
    parsed: Option<&Value>,
) -> Option<SummaryArtifactResponse> {
    let parsed = parsed?;
    let action = parsed
        .get("action")
        .or_else(|| parsed.get("requested_action"))
        .and_then(|v| v.as_str())?;

    let title = parsed.get("title").and_then(|v| v.as_str());
    let final_url = parsed.get("final_url").and_then(|v| v.as_str());
    let content_kind = parsed.get("content_kind").and_then(|v| v.as_str());
    let content_preview = parsed.get("content_preview").and_then(|v| v.as_str());
    let content_length = parsed.get("content_length").and_then(|v| v.as_i64());
    let content_ready = parsed.get("content_ready").and_then(|v| v.as_bool());

    let mut parts = Vec::new();
    if let Some(title) = title {
        parts.push(format!("title {}", title));
    }
    if let Some(final_url) = final_url {
        parts.push(format!("final url {}", final_url));
    }
    if let Some(kind) = content_kind {
        parts.push(format!("content {}", kind));
    }
    if let Some(length) = content_length {
        parts.push(format!("content length {}", length));
    }
    if let Some(ready) = content_ready {
        parts.push(format!("content ready {}", ready));
    }
    if let Some(preview) = content_preview.filter(|v| !v.is_empty()) {
        let shortened = if preview.chars().count() > 80 {
            let prefix = preview.chars().take(80).collect::<String>();
            format!("{}…", prefix)
        } else {
            preview.to_string()
        };
        parts.push(format!("preview {}", shortened));
    }

    if parts.is_empty() {
        return None;
    }

    Some(SummaryArtifactResponse {
        category: "summary".to_string(),
        key: format!("browser.result.{}", action),
        source: "runner.browser_result".to_string(),
        severity: "info".to_string(),
        title: "browser result summary".to_string(),
        summary: format!("action {} ; {}", action, parts.join(" ; ")),
        task_id: None,
        task_kind: None,
        task_status: None,
        run_id: None,
        attempt: None,
        timestamp: None,
    })
}

fn browser_failure_summary_artifact_from_parsed(
    parsed: Option<&Value>,
) -> Option<SummaryArtifactResponse> {
    let failure_scope = parsed?.get("failure_scope")?.as_str()?;
    let browser_failure_signal = parsed
        .and_then(|value| value.get("browser_failure_signal"))
        .and_then(|value| value.as_str())
        .unwrap_or("none");
    let status = parsed
        .and_then(|value| value.get("status"))
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let error_kind = parsed
        .and_then(|value| value.get("error_kind"))
        .and_then(|value| value.as_str())
        .unwrap_or("none");
    let execution_stage = parsed
        .and_then(|value| value.get("execution_stage"))
        .and_then(|value| value.as_str())
        .unwrap_or("none");

    let severity = if failure_scope == "runner_timeout" || failure_scope == "browser_execution" {
        "error"
    } else {
        "warning"
    };

    Some(SummaryArtifactResponse {
        category: "execution".to_string(),
        key: format!("browser.failure.{}", failure_scope),
        source: "runner.browser_failure".to_string(),
        severity: severity.to_string(),
        title: "browser failure summary".to_string(),
        summary: format!(
            "failure_scope={} browser_failure_signal={} execution_stage={} status={} error_kind={}",
            failure_scope, browser_failure_signal, execution_stage, status, error_kind
        ),
        task_id: None,
        task_kind: None,
        task_status: None,
        run_id: None,
        attempt: None,
        timestamp: None,
    })
}

fn summary_artifacts_from_parsed(parsed: Option<&Value>) -> Vec<SummaryArtifactResponse> {
    let mut artifacts: Vec<SummaryArtifactResponse> = parsed
        .and_then(|value| value.get("summary_artifacts").cloned())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            Some(SummaryArtifactResponse {
                category: normalize_summary_category(item.get("category")?.as_str()?),
                key: item
                    .get("key")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        item.get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or("summary.unknown")
                    })
                    .to_string(),
                source: normalize_summary_source(item.get("source").and_then(|v| v.as_str())),
                severity: normalize_summary_severity(item.get("severity").and_then(|v| v.as_str())),
                title: item.get("title")?.as_str()?.to_string(),
                summary: item.get("summary")?.as_str()?.to_string(),
                task_id: None,
                task_kind: None,
                task_status: None,
                run_id: item
                    .get("run_id")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
                attempt: item
                    .get("attempt")
                    .and_then(|v| v.as_i64())
                    .and_then(|v| i32::try_from(v).ok()),
                timestamp: item
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string()),
            })
        })
        .collect();

    let has_selection_decision = artifacts
        .iter()
        .any(|item| item.title == "proxy selection decision");
    if !has_selection_decision {
        if let Some(artifact) = selection_decision_summary_artifact_from_parsed(parsed) {
            artifacts.push(artifact);
        }
    }

    let has_identity_network_summary = artifacts
        .iter()
        .any(|item| item.title == "identity and network summary");
    if !has_identity_network_summary {
        if let Some(artifact) = identity_network_summary_artifact_from_parsed(parsed) {
            artifacts.push(artifact);
        }
    }

    let has_proxy_growth_summary = artifacts
        .iter()
        .any(|item| item.title == "proxy growth assessment");
    if !has_proxy_growth_summary {
        if let Some(artifact) = proxy_growth_summary_artifact_from_parsed(parsed) {
            artifacts.push(artifact);
        }
    }

    let has_browser_result_summary = artifacts
        .iter()
        .any(|item| item.title == "browser result summary");
    if !has_browser_result_summary {
        if let Some(artifact) = browser_result_summary_artifact_from_parsed(parsed) {
            artifacts.push(artifact);
        }
    }

    let has_browser_failure_summary = artifacts
        .iter()
        .any(|item| item.title == "browser failure summary");
    if !has_browser_failure_summary {
        if let Some(artifact) = browser_failure_summary_artifact_from_parsed(parsed) {
            artifacts.push(artifact);
        }
    }

    artifacts
}

pub fn summary_artifacts(result_json: Option<&str>) -> Vec<SummaryArtifactResponse> {
    let parsed = parse_result_json(result_json);
    summary_artifacts_from_parsed(parsed.as_ref())
}

pub fn enrich_summary_artifacts(
    mut artifacts: Vec<SummaryArtifactResponse>,
    task_id: Option<&str>,
    task_kind: Option<&str>,
    task_status: Option<&str>,
    run_id: Option<&str>,
    attempt: Option<i32>,
    timestamp: Option<&str>,
) -> Vec<SummaryArtifactResponse> {
    for artifact in &mut artifacts {
        if artifact.task_id.is_none() {
            artifact.task_id = task_id.map(|v| v.to_string());
        }
        if artifact.task_kind.is_none() {
            artifact.task_kind = task_kind.map(|v| v.to_string());
        }
        if artifact.task_status.is_none() {
            artifact.task_status = task_status.map(|v| v.to_string());
        }
        if artifact.run_id.is_none() {
            artifact.run_id = run_id.map(|v| v.to_string());
        }
        if artifact.attempt.is_none() {
            artifact.attempt = attempt;
        }
        if artifact.timestamp.is_none() {
            artifact.timestamp = timestamp.map(|v| v.to_string());
        }
    }
    artifacts
}

fn summary_severity_rank(severity: &str) -> i32 {
    match severity {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    }
}

pub fn latest_execution_summaries(tasks: &[TaskResponse]) -> Vec<SummaryArtifactResponse> {
    let mut items = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (task_index, task) in tasks.iter().enumerate() {
        for mut artifact in task.summary_artifacts.iter().cloned() {
            artifact.task_id = Some(task.id.clone());
            artifact.task_kind = Some(task.kind.clone());
            artifact.task_status = Some(task.status.clone());
            let dedupe_key = format!("{}::{}", artifact.key, artifact.title);
            if seen.insert(dedupe_key) {
                items.push((task_index, artifact));
            }
        }
    }

    fn summary_priority(item: &SummaryArtifactResponse) -> i32 {
        match item.title.as_str() {
            "browser failure summary" => 0,
            "proxy selection decision" => 1,
            "identity and network summary" => 2,
            "browser result summary" => 3,
            "proxy growth assessment" => 4,
            _ => 5,
        }
    }

    items.sort_by_key(|(task_index, artifact)| {
        (
            summary_severity_rank(&artifact.severity),
            summary_priority(artifact),
            *task_index,
        )
    });
    items.truncate(5);
    items.into_iter().map(|(_, artifact)| artifact).collect()
}

pub fn browser_summary_from_task(
    task: &TaskResponse,
) -> Option<crate::api::dto::BrowserSummaryResponse> {
    if task.title.is_none()
        && task.final_url.is_none()
        && task.content_kind.is_none()
        && task.content_preview.is_none()
        && task.content_length.is_none()
        && task.content_ready.is_none()
    {
        return None;
    }

    Some(crate::api::dto::BrowserSummaryResponse {
        title: task.title.clone(),
        final_url: task.final_url.clone(),
        content_kind: task.content_kind.clone(),
        content_preview: task.content_preview.clone(),
        content_length: task.content_length,
        content_ready: task.content_ready,
    })
}

pub fn latest_browser_ready_tasks(tasks: &[TaskResponse], limit: usize) -> Vec<TaskResponse> {
    let mut items = tasks
        .iter()
        .filter(|task| browser_summary_from_task(task).is_some())
        .cloned()
        .collect::<Vec<_>>();

    items.sort_by(|a, b| {
        let a_ready = a.content_ready.unwrap_or(false);
        let b_ready = b.content_ready.unwrap_or(false);
        let a_readability = i32::from(a.title.as_ref().map(|v| !v.is_empty()).unwrap_or(false))
            + i32::from(
                a.content_preview
                    .as_ref()
                    .map(|v| !v.is_empty())
                    .unwrap_or(false),
            );
        let b_readability = i32::from(b.title.as_ref().map(|v| !v.is_empty()).unwrap_or(false))
            + i32::from(
                b.content_preview
                    .as_ref()
                    .map(|v| !v.is_empty())
                    .unwrap_or(false),
            );
        let a_freshness = a
            .finished_at
            .as_deref()
            .or(a.started_at.as_deref())
            .unwrap_or("");
        let b_freshness = b
            .finished_at
            .as_deref()
            .or(b.started_at.as_deref())
            .unwrap_or("");

        b_ready
            .cmp(&a_ready)
            .then_with(|| b_readability.cmp(&a_readability))
            .then_with(|| b_freshness.cmp(a_freshness))
    });

    items.truncate(limit);
    items
}

pub fn build_task_explainability(
    fingerprint_profile_id: Option<&str>,
    fingerprint_profile_version: Option<i64>,
    task_behavior_profile_id: Option<&str>,
    task_behavior_profile_version: Option<i64>,
    task_behavior_resolution_status: Option<&str>,
    task_behavior_policy_json: Option<&str>,
    result_json: Option<&str>,
    task_id: Option<&str>,
    task_kind: Option<&str>,
    task_status: Option<&str>,
    timestamp: Option<&str>,
) -> TaskExplainability {
    let parsed = parse_result_json(result_json);
    let parsed_ref = parsed.as_ref();
    let task_behavior_policy = parse_optional_json(task_behavior_policy_json);
    let task_behavior_policy_ref = task_behavior_policy.as_ref();
    let proxy_resolution_status = proxy_resolution_status_from_parsed(parsed_ref);
    let (proxy_id, proxy_provider, proxy_region) = proxy_identity_from_parsed(parsed_ref);
    let trust_score_total = trust_score_total_from_parsed(parsed_ref);
    let identity_session_status = identity_session_status_from_parsed(parsed_ref);
    let cookie_restore_count = count_from_parsed(parsed_ref, "cookie_restore_count");
    let cookie_persist_count = count_from_parsed(parsed_ref, "cookie_persist_count");
    let local_storage_restore_count = count_from_parsed(parsed_ref, "local_storage_restore_count");
    let local_storage_persist_count = count_from_parsed(parsed_ref, "local_storage_persist_count");
    let session_storage_restore_count =
        count_from_parsed(parsed_ref, "session_storage_restore_count");
    let session_storage_persist_count =
        count_from_parsed(parsed_ref, "session_storage_persist_count");
    let selection_reason_summary = selection_reason_summary_from_parsed(parsed_ref);
    let selection_explain = selection_explain_from_parsed(parsed_ref);
    let fingerprint_runtime_explain = fingerprint_runtime_explain_from_parsed(parsed_ref);
    let behavior_profile_id = string_field_from_parsed(parsed_ref, "behavior_profile_id")
        .or_else(|| task_behavior_profile_id.map(str::to_string));
    let behavior_profile_version = i64_field_from_parsed(parsed_ref, "behavior_profile_version")
        .or(task_behavior_profile_version);
    let behavior_resolution_status =
        string_field_from_parsed(parsed_ref, "behavior_resolution_status")
            .or_else(|| task_behavior_resolution_status.map(str::to_string))
            .or_else(|| behavior_profile_id.as_ref().map(|_| "pending".to_string()));
    let behavior_execution_mode = string_field_from_parsed(parsed_ref, "behavior_execution_mode")
        .or_else(|| {
            task_behavior_policy_ref
                .and_then(|value| value.get("mode"))
                .and_then(|value| value.as_str())
                .map(str::to_string)
        });
    let page_archetype = string_field_from_parsed(parsed_ref, "page_archetype").or_else(|| {
        task_behavior_policy_ref
            .and_then(|value| value.get("page_archetype"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
    });
    let behavior_seed = string_field_from_parsed(parsed_ref, "behavior_seed").or_else(|| {
        task_behavior_policy_ref
            .and_then(|value| value.get("plan_seed"))
            .and_then(|value| value.as_str())
            .map(str::to_string)
    });
    let behavior_runtime_explain = behavior_runtime_explain_from_parsed(parsed_ref);
    let behavior_trace_summary = behavior_trace_summary_from_parsed(parsed_ref);
    let execution_identity = ExecutionIdentity {
        fingerprint_profile_id: fingerprint_profile_id.map(str::to_string),
        fingerprint_profile_version,
        fingerprint_resolution_status: fingerprint_resolution_status_from_parsed(
            fingerprint_profile_id,
            fingerprint_profile_version,
            parsed_ref,
        ),
        fingerprint_runtime_explain: fingerprint_runtime_explain.clone(),
        behavior_profile_id: behavior_profile_id.clone(),
        behavior_profile_version,
        behavior_resolution_status: behavior_resolution_status.clone(),
        behavior_execution_mode: behavior_execution_mode.clone(),
        page_archetype: page_archetype.clone(),
        behavior_seed: behavior_seed.clone(),
        behavior_runtime_explain: behavior_runtime_explain.clone(),
        behavior_trace_summary: behavior_trace_summary.clone(),
        proxy_id: proxy_id.clone(),
        proxy_provider: proxy_provider.clone(),
        proxy_region: proxy_region.clone(),
        proxy_resolution_status: proxy_resolution_status.clone(),
        selection_reason_summary: selection_reason_summary.clone(),
        selection_explain: selection_explain.clone(),
        trust_score_total,
        identity_session_status: identity_session_status.clone(),
        cookie_restore_count,
        cookie_persist_count,
        local_storage_restore_count,
        local_storage_persist_count,
        session_storage_restore_count,
        session_storage_persist_count,
    };
    let identity_network_explain = Some(IdentityNetworkExplain {
        execution_identity: execution_identity.clone(),
        selection_explain: selection_explain.clone(),
        fingerprint_runtime_explain: fingerprint_runtime_explain.clone(),
        proxy_id: proxy_id.clone(),
        proxy_provider: proxy_provider.clone(),
        proxy_region: proxy_region.clone(),
        proxy_resolution_status: proxy_resolution_status.clone(),
        selection_reason_summary: selection_reason_summary.clone(),
        trust_score_total,
        identity_session_status,
        cookie_restore_count,
        cookie_persist_count,
        local_storage_restore_count,
        local_storage_persist_count,
        session_storage_restore_count,
        session_storage_persist_count,
    });
    let winner_vs_runner_up_diff = candidate_rank_preview_from_parsed(parsed_ref)
        .into_iter()
        .next()
        .and_then(|item| item.winner_vs_runner_up_diff);
    let summary_artifacts = enrich_summary_artifacts(
        summary_artifacts_from_parsed(parsed_ref),
        task_id,
        task_kind,
        task_status,
        None,
        None,
        timestamp,
    );
    let failure_scope = content_string_field(parsed_ref, "failure_scope");
    let browser_failure_signal = content_string_field(parsed_ref, "browser_failure_signal");

    TaskExplainability {
        fingerprint_resolution_status: execution_identity.fingerprint_resolution_status.clone(),
        behavior_profile_id,
        behavior_profile_version,
        behavior_resolution_status,
        behavior_execution_mode,
        page_archetype,
        behavior_seed,
        behavior_runtime_explain,
        behavior_trace_summary,
        proxy_id,
        proxy_provider,
        proxy_region,
        proxy_resolution_status,
        trust_score_total,
        selection_reason_summary,
        selection_explain,
        fingerprint_runtime_explain,
        execution_identity,
        identity_network_explain,
        winner_vs_runner_up_diff,
        failure_scope,
        browser_failure_signal,
        summary_artifacts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dto::{TaskResponse, WinnerVsRunnerUpDirection, WinnerVsRunnerUpFactor};
    use serde_json::json;

    fn sample_diff() -> WinnerVsRunnerUpDiff {
        WinnerVsRunnerUpDiff {
            winner_total_score: 90,
            runner_up_total_score: 70,
            score_gap: 20,
            factors: vec![
                WinnerVsRunnerUpFactor {
                    factor: "verify_ok_bonus".to_string(),
                    label: "verify_ok".to_string(),
                    winner_value: 30,
                    runner_up_value: 0,
                    delta: 30,
                    direction: WinnerVsRunnerUpDirection::Winner,
                },
                WinnerVsRunnerUpFactor {
                    factor: "provider_risk_penalty".to_string(),
                    label: "provider_risk".to_string(),
                    winner_value: 0,
                    runner_up_value: 5,
                    delta: 5,
                    direction: WinnerVsRunnerUpDirection::Winner,
                },
            ],
        }
    }

    fn sample_result_json_without_selection_artifact() -> String {
        json!({
            "fingerprint_profile": {"id": "fp-1", "version": 2},
            "proxy": {
                "id": "proxy-1",
                "provider": "pool-a",
                "region": "us-east",
                "resolution_status": "resolved"
            },
            "payload": {
                "network_policy_json": {
                    "selection_reason_summary": "winner has better trust score",
                    "trust_score_total": 90,
                    "selection_explain": {
                        "selection_mode": "auto",
                        "proxy_growth": {
                            "target_region": "us-east",
                            "selected_proxy_region": "us-east",
                            "inventory_snapshot": {
                                "total": 100,
                                "available": 45,
                                "region": "us-east",
                                "available_in_region": 5,
                                "inflight_tasks": 10
                            },
                            "health_assessment": {
                                "available_ratio_percent": 45,
                                "healthy_ratio_band": "within_band",
                                "below_min_ratio": false,
                                "above_max_ratio": false,
                                "below_min_total": false,
                                "below_min_region": false,
                                "require_replenish": false,
                                "reasons": []
                            },
                            "region_match": {
                                "target_region": "us-east",
                                "proxy_region": "us-east",
                                "match_mode": "region_preferred",
                                "matches": true,
                                "score": 100,
                                "reason": "exact_region_match"
                            }
                        }
                    },
                    "candidate_rank_preview": [{
                        "id": "proxy-1",
                        "provider": "pool-a",
                        "region": "us-east",
                        "score": 0.77,
                        "trust_score_total": 90,
                        "summary": "wins on verify_ok; penalized by provider_risk",
                        "winner_vs_runner_up_diff": sample_diff()
                    }]
                }
            },
            "fingerprint_runtime_explain": {
                "fingerprint_budget_tag": "medium",
                "fingerprint_consistency": {
                    "overall_status": "soft_match",
                    "checks": [{"name": "timezone_vs_region", "status": "soft_match", "reason": "timezone_matches_region_family"}]
                },
                "consumption_explain": {
                    "declared_fields": ["user_agent", "timezone", "platform"],
                    "resolved_fields": ["user_agent", "timezone"],
                    "applied_fields": ["user_agent", "timezone"],
                    "ignored_fields": ["platform"],
                    "declared_count": 3,
                    "resolved_count": 2,
                    "applied_count": 2,
                    "ignored_count": 1,
                    "consumption_status": "partially_consumed",
                    "partial_support_warning": "some declared fingerprint fields were not consumed by the current lightpanda runner"
                }
            },
            "action": "extract_text",
            "title": "Example title",
            "final_url": "https://example.com/final",
            "content_kind": "text/plain",
            "content_length": 24,
            "content_ready": true,
            "content_preview": "example preview text",
            "identity_session_status": "auto_reused",
            "cookie_restore_count": 3,
            "cookie_persist_count": 4,
            "local_storage_restore_count": 2,
            "local_storage_persist_count": 5,
            "session_storage_restore_count": 1,
            "session_storage_persist_count": 6,
            "failure_scope": "browser_execution",
            "browser_failure_signal": "browser_navigation_failure_signal",
            "summary_artifacts": [{
                "category": "weird",
                "title": "fake runner summary",
                "summary": "ran successfully",
                "source": "fake_runner",
                "severity": "notice",
                "attempt": 1,
                "timestamp": "123456"
            }]
        }).to_string()
    }

    #[test]
    fn summary_artifacts_normalize_fields_and_inject_selection_decision() {
        let raw = sample_result_json_without_selection_artifact();
        let artifacts = summary_artifacts(Some(&raw));
        assert_eq!(artifacts.len(), 6);

        let runner = artifacts
            .iter()
            .find(|a| a.title == "fake runner summary")
            .expect("runner artifact");
        assert_eq!(runner.category, "summary");
        assert_eq!(runner.source, "runner.fake");
        assert_eq!(runner.severity, "info");
        assert_eq!(runner.attempt, Some(1));
        assert_eq!(runner.timestamp.as_deref(), Some("123456"));
        assert_eq!(runner.key, "fake runner summary");

        let selection = artifacts
            .iter()
            .find(|a| a.title == "proxy selection decision")
            .expect("selection artifact");
        assert_eq!(selection.key, "proxy.selection.decision");
        assert_eq!(selection.source, "selection.proxy");
        assert_eq!(selection.severity, "info");
        assert!(selection.summary.contains("this proxy stayed ahead by"));
        assert!(selection.summary.contains("biggest score drivers"));

        let identity = artifacts
            .iter()
            .find(|a| a.title == "identity and network summary")
            .expect("identity artifact");
        assert_eq!(identity.key, "identity.network.summary");
        assert_eq!(identity.source, "selection.identity_network");
        assert_eq!(identity.severity, "info");
        assert!(identity.summary.contains("proxy pool-a@us-east"));
        assert!(identity.summary.contains("proxy resolution resolved"));
        assert!(identity.summary.contains("fingerprint budget medium"));
        assert!(identity.summary.contains(
            "fingerprint consumption partially_consumed (declared 3, applied 2, ignored 1)"
        ));
        assert!(identity.summary.contains("selection summary"));

        let growth = artifacts
            .iter()
            .find(|a| a.title == "proxy growth assessment")
            .expect("growth artifact");
        assert_eq!(growth.key, "proxy.selection.proxy_growth");
        assert_eq!(growth.source, "selection.proxy_growth");
        assert_eq!(growth.severity, "info");
        assert!(growth.summary.contains("pool is healthy for this request"));
        assert!(growth.summary.contains("target region us-east"));
        assert!(growth.summary.contains("region fit exact region match"));

        let browser = artifacts
            .iter()
            .find(|a| a.title == "browser result summary")
            .expect("browser artifact");
        assert_eq!(browser.key, "browser.result.extract_text");
        assert_eq!(browser.source, "runner.browser_result");
        assert_eq!(browser.severity, "info");
        assert!(browser.summary.contains("action extract_text"));
    }

    #[test]
    fn enrich_summary_artifacts_backfills_missing_context_only() {
        let artifacts = vec![
            SummaryArtifactResponse {
                category: "summary".to_string(),
                key: "a".to_string(),
                source: "runner.fake".to_string(),
                severity: "info".to_string(),
                title: "artifact a".to_string(),
                summary: "ok".to_string(),
                task_id: None,
                task_kind: None,
                task_status: None,
                run_id: None,
                attempt: None,
                timestamp: None,
            },
            SummaryArtifactResponse {
                category: "summary".to_string(),
                key: "b".to_string(),
                source: "runner.fake".to_string(),
                severity: "warning".to_string(),
                title: "artifact b".to_string(),
                summary: "warn".to_string(),
                task_id: Some("task-existing".to_string()),
                task_kind: Some("open_page".to_string()),
                task_status: Some("failed".to_string()),
                run_id: Some("run-existing".to_string()),
                attempt: Some(9),
                timestamp: Some("old-ts".to_string()),
            },
        ];
        let enriched = enrich_summary_artifacts(
            artifacts,
            Some("task-1"),
            Some("verify_proxy"),
            Some("succeeded"),
            Some("run-1"),
            Some(2),
            Some("new-ts"),
        );
        assert_eq!(enriched[0].task_id.as_deref(), Some("task-1"));
        assert_eq!(enriched[0].task_kind.as_deref(), Some("verify_proxy"));
        assert_eq!(enriched[0].task_status.as_deref(), Some("succeeded"));
        assert_eq!(enriched[0].run_id.as_deref(), Some("run-1"));
        assert_eq!(enriched[0].attempt, Some(2));
        assert_eq!(enriched[0].timestamp.as_deref(), Some("new-ts"));

        assert_eq!(enriched[1].task_id.as_deref(), Some("task-existing"));
        assert_eq!(enriched[1].run_id.as_deref(), Some("run-existing"));
        assert_eq!(enriched[1].attempt, Some(9));
        assert_eq!(enriched[1].timestamp.as_deref(), Some("old-ts"));
    }

    #[test]
    fn latest_execution_summaries_prioritize_errors_and_deduplicate() {
        let tasks = vec![
            TaskResponse {
                id: "task-1".to_string(),
                kind: "open_page".to_string(),
                status: "succeeded".to_string(),
                priority: 1,
                persona_id: None,
                platform_id: None,
                manual_gate_request_id: None,
                manual_gate_status: None,
                started_at: None,
                finished_at: None,
                summary_artifacts: vec![SummaryArtifactResponse {
                    category: "summary".to_string(),
                    key: "shared".to_string(),
                    source: "runner.fake".to_string(),
                    severity: "info".to_string(),
                    title: "duplicate title".to_string(),
                    summary: "first".to_string(),
                    task_id: None,
                    task_kind: None,
                    task_status: None,
                    run_id: None,
                    attempt: None,
                    timestamp: None,
                }],
                fingerprint_profile_id: None,
                fingerprint_profile_version: None,
                fingerprint_resolution_status: None,
                behavior_profile_id: None,
                behavior_profile_version: None,
                behavior_resolution_status: None,
                behavior_execution_mode: None,
                page_archetype: None,
                behavior_seed: None,
                behavior_runtime_explain: None,
                behavior_trace_summary: None,
                form_action_status: None,
                form_action_mode: None,
                form_action_retry_count: None,
                form_action_summary_json: None,
                proxy_id: None,
                proxy_provider: None,
                proxy_region: None,
                proxy_resolution_status: None,
                trust_score_total: None,
                selection_reason_summary: None,
                selection_explain: None,
                fingerprint_runtime_explain: None,
                execution_identity: Some(crate::api::dto::ExecutionIdentity {
                    fingerprint_profile_id: None,
                    fingerprint_profile_version: None,
                    fingerprint_resolution_status: None,
                    fingerprint_runtime_explain: None,
                    behavior_profile_id: None,
                    behavior_profile_version: None,
                    behavior_resolution_status: None,
                    behavior_execution_mode: None,
                    page_archetype: None,
                    behavior_seed: None,
                    behavior_runtime_explain: None,
                    behavior_trace_summary: None,
                    proxy_id: None,
                    proxy_provider: None,
                    proxy_region: None,
                    proxy_resolution_status: None,
                    selection_reason_summary: None,
                    selection_explain: None,
                    trust_score_total: None,
                    identity_session_status: None,
                    cookie_restore_count: None,
                    cookie_persist_count: None,
                    local_storage_restore_count: None,
                    local_storage_persist_count: None,
                    session_storage_restore_count: None,
                    session_storage_persist_count: None,
                }),
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
            },
            TaskResponse {
                id: "task-2".to_string(),
                kind: "verify_proxy".to_string(),
                status: "failed".to_string(),
                priority: 1,
                persona_id: None,
                platform_id: None,
                manual_gate_request_id: None,
                manual_gate_status: None,
                started_at: None,
                finished_at: None,
                summary_artifacts: vec![
                    SummaryArtifactResponse {
                        category: "execution".to_string(),
                        key: "verify_proxy.execution".to_string(),
                        source: "runner.fake".to_string(),
                        severity: "error".to_string(),
                        title: "verify failed".to_string(),
                        summary: "boom".to_string(),
                        task_id: None,
                        task_kind: None,
                        task_status: None,
                        run_id: None,
                        attempt: None,
                        timestamp: None,
                    },
                    SummaryArtifactResponse {
                        category: "summary".to_string(),
                        key: "shared".to_string(),
                        source: "runner.fake".to_string(),
                        severity: "warning".to_string(),
                        title: "duplicate title".to_string(),
                        summary: "second".to_string(),
                        task_id: None,
                        task_kind: None,
                        task_status: None,
                        run_id: None,
                        attempt: None,
                        timestamp: None,
                    },
                    SummaryArtifactResponse {
                        category: "execution".to_string(),
                        key: "browser.failure.browser_execution".to_string(),
                        source: "runner.browser_failure".to_string(),
                        severity: "error".to_string(),
                        title: "browser failure summary".to_string(),
                        summary: "failure_scope=browser_execution browser_failure_signal=browser_navigation_failure_signal".to_string(),
                        task_id: None,
                        task_kind: None,
                        task_status: None,
                        run_id: None,
                        attempt: None,
                        timestamp: None,
                    }
                ],
                fingerprint_profile_id: None,
                fingerprint_profile_version: None,
                fingerprint_resolution_status: None,
                behavior_profile_id: None,
                behavior_profile_version: None,
                behavior_resolution_status: None,
                behavior_execution_mode: None,
                page_archetype: None,
                behavior_seed: None,
                behavior_runtime_explain: None,
                behavior_trace_summary: None,
                form_action_status: None,
                form_action_mode: None,
                form_action_retry_count: None,
                form_action_summary_json: None,
                proxy_id: None,
                proxy_provider: None,
                proxy_region: None,
                proxy_resolution_status: None,
                trust_score_total: None,
                selection_reason_summary: None,
                selection_explain: None,
                fingerprint_runtime_explain: None,
                execution_identity: Some(crate::api::dto::ExecutionIdentity {
                    fingerprint_profile_id: None,
                    fingerprint_profile_version: None,
                    fingerprint_resolution_status: None,
                    fingerprint_runtime_explain: None,
                    behavior_profile_id: None,
                    behavior_profile_version: None,
                    behavior_resolution_status: None,
                    behavior_execution_mode: None,
                    page_archetype: None,
                    behavior_seed: None,
                    behavior_runtime_explain: None,
                    behavior_trace_summary: None,
                    proxy_id: None,
                    proxy_provider: None,
                    proxy_region: None,
                    proxy_resolution_status: None,
                    selection_reason_summary: None,
                    selection_explain: None,
                    trust_score_total: None,
                    identity_session_status: None,
                    cookie_restore_count: None,
                    cookie_persist_count: None,
                    local_storage_restore_count: None,
                    local_storage_persist_count: None,
                    session_storage_restore_count: None,
                    session_storage_persist_count: None,
                }),
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
            },
        ];

        let latest = latest_execution_summaries(&tasks);
        assert_eq!(latest.len(), 3);
        assert_eq!(latest[0].title, "browser failure summary");
        assert_eq!(latest[0].severity, "error");
        assert_eq!(latest[0].task_id.as_deref(), Some("task-2"));
        assert_eq!(latest[1].severity, "error");
        assert!(
            latest
                .iter()
                .filter(|item| item.title == "duplicate title")
                .count()
                == 1
        );
        assert!(latest
            .iter()
            .any(|item| item.task_id.as_deref() == Some("task-1")));
    }

    #[test]
    fn summary_artifacts_add_browser_failure_summary_when_failure_scope_present() {
        let raw = serde_json::json!({
            "status": "failed",
            "error_kind": "runner_non_zero_exit",
            "failure_scope": "browser_execution",
            "browser_failure_signal": "browser_navigation_failure_signal"
        })
        .to_string();
        let artifacts = summary_artifacts(Some(&raw));
        let browser_failure = artifacts
            .iter()
            .find(|item| item.title == "browser failure summary")
            .expect("browser failure summary artifact");
        assert_eq!(browser_failure.key, "browser.failure.browser_execution");
        assert_eq!(browser_failure.source, "runner.browser_failure");
        assert_eq!(browser_failure.severity, "error");
        assert!(browser_failure
            .summary
            .contains("browser_navigation_failure_signal"));
    }

    #[test]
    fn latest_browser_ready_tasks_prefers_browser_visible_rows() {
        let tasks = vec![
            TaskResponse {
                id: "task-browser-1".to_string(),
                kind: "get_title".to_string(),
                status: "succeeded".to_string(),
                priority: 1,
                persona_id: None,
                platform_id: None,
                manual_gate_request_id: None,
                manual_gate_status: None,
                started_at: None,
                finished_at: Some("3".to_string()),
                summary_artifacts: vec![],
                fingerprint_profile_id: None,
                fingerprint_profile_version: None,
                fingerprint_resolution_status: None,
                behavior_profile_id: None,
                behavior_profile_version: None,
                behavior_resolution_status: None,
                behavior_execution_mode: None,
                page_archetype: None,
                behavior_seed: None,
                behavior_runtime_explain: None,
                behavior_trace_summary: None,
                form_action_status: None,
                form_action_mode: None,
                form_action_retry_count: None,
                form_action_summary_json: None,
                proxy_id: None,
                proxy_provider: None,
                proxy_region: None,
                proxy_resolution_status: None,
                trust_score_total: None,
                selection_reason_summary: None,
                selection_explain: None,
                fingerprint_runtime_explain: None,
                execution_identity: Some(crate::api::dto::ExecutionIdentity {
                    fingerprint_profile_id: None,
                    fingerprint_profile_version: None,
                    fingerprint_resolution_status: None,
                    fingerprint_runtime_explain: None,
                    behavior_profile_id: None,
                    behavior_profile_version: None,
                    behavior_resolution_status: None,
                    behavior_execution_mode: None,
                    page_archetype: None,
                    behavior_seed: None,
                    behavior_runtime_explain: None,
                    behavior_trace_summary: None,
                    proxy_id: None,
                    proxy_provider: None,
                    proxy_region: None,
                    proxy_resolution_status: None,
                    selection_reason_summary: None,
                    selection_explain: None,
                    trust_score_total: None,
                    identity_session_status: None,
                    cookie_restore_count: None,
                    cookie_persist_count: None,
                    local_storage_restore_count: None,
                    local_storage_persist_count: None,
                    session_storage_restore_count: None,
                    session_storage_persist_count: None,
                }),
                identity_network_explain: None,
                winner_vs_runner_up_diff: None,
                failure_scope: None,
                browser_failure_signal: None,
                title: Some("Browser 1".to_string()),
                final_url: Some("https://example.com/1".to_string()),
                content_preview: None,
                content_length: None,
                content_truncated: None,
                content_kind: None,
                content_source_action: None,
                content_ready: None,
            },
            TaskResponse {
                id: "task-non-browser".to_string(),
                kind: "open_page".to_string(),
                status: "succeeded".to_string(),
                priority: 1,
                persona_id: None,
                platform_id: None,
                manual_gate_request_id: None,
                manual_gate_status: None,
                started_at: None,
                finished_at: None,
                summary_artifacts: vec![],
                fingerprint_profile_id: None,
                fingerprint_profile_version: None,
                fingerprint_resolution_status: None,
                behavior_profile_id: None,
                behavior_profile_version: None,
                behavior_resolution_status: None,
                behavior_execution_mode: None,
                page_archetype: None,
                behavior_seed: None,
                behavior_runtime_explain: None,
                behavior_trace_summary: None,
                form_action_status: None,
                form_action_mode: None,
                form_action_retry_count: None,
                form_action_summary_json: None,
                proxy_id: None,
                proxy_provider: None,
                proxy_region: None,
                proxy_resolution_status: None,
                trust_score_total: None,
                selection_reason_summary: None,
                selection_explain: None,
                fingerprint_runtime_explain: None,
                execution_identity: Some(crate::api::dto::ExecutionIdentity {
                    fingerprint_profile_id: None,
                    fingerprint_profile_version: None,
                    fingerprint_resolution_status: None,
                    fingerprint_runtime_explain: None,
                    behavior_profile_id: None,
                    behavior_profile_version: None,
                    behavior_resolution_status: None,
                    behavior_execution_mode: None,
                    page_archetype: None,
                    behavior_seed: None,
                    behavior_runtime_explain: None,
                    behavior_trace_summary: None,
                    proxy_id: None,
                    proxy_provider: None,
                    proxy_region: None,
                    proxy_resolution_status: None,
                    selection_reason_summary: None,
                    selection_explain: None,
                    trust_score_total: None,
                    identity_session_status: None,
                    cookie_restore_count: None,
                    cookie_persist_count: None,
                    local_storage_restore_count: None,
                    local_storage_persist_count: None,
                    session_storage_restore_count: None,
                    session_storage_persist_count: None,
                }),
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
            },
            TaskResponse {
                id: "task-browser-2".to_string(),
                kind: "extract_text".to_string(),
                status: "succeeded".to_string(),
                priority: 1,
                persona_id: None,
                platform_id: None,
                manual_gate_request_id: None,
                manual_gate_status: None,
                started_at: None,
                finished_at: Some("4".to_string()),
                summary_artifacts: vec![],
                fingerprint_profile_id: None,
                fingerprint_profile_version: None,
                fingerprint_resolution_status: None,
                behavior_profile_id: None,
                behavior_profile_version: None,
                behavior_resolution_status: None,
                behavior_execution_mode: None,
                page_archetype: None,
                behavior_seed: None,
                behavior_runtime_explain: None,
                behavior_trace_summary: None,
                form_action_status: None,
                form_action_mode: None,
                form_action_retry_count: None,
                form_action_summary_json: None,
                proxy_id: None,
                proxy_provider: None,
                proxy_region: None,
                proxy_resolution_status: None,
                trust_score_total: None,
                selection_reason_summary: None,
                selection_explain: None,
                fingerprint_runtime_explain: None,
                execution_identity: Some(crate::api::dto::ExecutionIdentity {
                    fingerprint_profile_id: None,
                    fingerprint_profile_version: None,
                    fingerprint_resolution_status: None,
                    fingerprint_runtime_explain: None,
                    behavior_profile_id: None,
                    behavior_profile_version: None,
                    behavior_resolution_status: None,
                    behavior_execution_mode: None,
                    page_archetype: None,
                    behavior_seed: None,
                    behavior_runtime_explain: None,
                    behavior_trace_summary: None,
                    proxy_id: None,
                    proxy_provider: None,
                    proxy_region: None,
                    proxy_resolution_status: None,
                    selection_reason_summary: None,
                    selection_explain: None,
                    trust_score_total: None,
                    identity_session_status: None,
                    cookie_restore_count: None,
                    cookie_persist_count: None,
                    local_storage_restore_count: None,
                    local_storage_persist_count: None,
                    session_storage_restore_count: None,
                    session_storage_persist_count: None,
                }),
                identity_network_explain: None,
                winner_vs_runner_up_diff: None,
                failure_scope: None,
                browser_failure_signal: None,
                title: None,
                final_url: Some("https://example.com/2".to_string()),
                content_preview: Some("preview".to_string()),
                content_length: Some(7),
                content_truncated: None,
                content_kind: Some("text/plain".to_string()),
                content_source_action: Some("extract_text".to_string()),
                content_ready: Some(true),
            },
        ];

        let latest = latest_browser_ready_tasks(&tasks, 3);
        assert_eq!(latest.len(), 2);
        assert_eq!(latest[0].id, "task-browser-2");
        assert_eq!(latest[1].id, "task-browser-1");
        let browser = browser_summary_from_task(&latest[0]).expect("browser summary");
        assert_eq!(browser.content_kind.as_deref(), Some("text/plain"));
    }

    #[test]
    fn build_task_explainability_assembles_expected_fields() {
        let raw = sample_result_json_without_selection_artifact();
        let explain = build_task_explainability(
            Some("fp-1"),
            Some(2),
            Some("beh-1"),
            Some(3),
            Some("resolved"),
            Some(r#"{"mode":"shadow","page_archetype":"article","plan_seed":"seed-1"}"#),
            Some(&raw),
            Some("task-1"),
            Some("open_page"),
            Some("succeeded"),
            Some("999999"),
        );
        assert_eq!(
            explain.fingerprint_resolution_status.as_deref(),
            Some("resolved")
        );
        assert_eq!(explain.proxy_id.as_deref(), Some("proxy-1"));
        assert_eq!(explain.proxy_provider.as_deref(), Some("pool-a"));
        assert_eq!(explain.proxy_region.as_deref(), Some("us-east"));
        assert_eq!(explain.proxy_resolution_status.as_deref(), Some("resolved"));
        assert_eq!(explain.trust_score_total, Some(90));
        assert_eq!(
            explain.selection_reason_summary.as_deref(),
            Some("winner has better trust score")
        );
        assert_eq!(
            explain
                .fingerprint_runtime_explain
                .as_ref()
                .and_then(|v| v.fingerprint_budget_tag.as_deref()),
            Some("medium")
        );
        assert!(explain
            .fingerprint_runtime_explain
            .as_ref()
            .and_then(|v| v.fingerprint_consistency.as_ref())
            .is_some());
        assert_eq!(
            explain
                .fingerprint_runtime_explain
                .as_ref()
                .and_then(|v| v.consumption_explain.as_ref())
                .map(|v| v.consumption_status.as_str()),
            Some("partially_consumed")
        );
        assert_eq!(
            explain
                .fingerprint_runtime_explain
                .as_ref()
                .and_then(|v| v.consumption_explain.as_ref())
                .map(|v| v.ignored_count),
            Some(1)
        );
        assert!(explain.identity_network_explain.is_some());
        assert_eq!(
            explain
                .identity_network_explain
                .as_ref()
                .and_then(|v| v.proxy_provider.as_deref()),
            Some("pool-a")
        );
        assert_eq!(
            explain
                .identity_network_explain
                .as_ref()
                .and_then(|v| v.fingerprint_runtime_explain.as_ref())
                .and_then(|v| v.fingerprint_budget_tag.as_deref()),
            Some("medium")
        );
        assert_eq!(
            explain
                .execution_identity
                .identity_session_status
                .as_deref(),
            Some("auto_reused")
        );
        assert_eq!(explain.execution_identity.cookie_restore_count, Some(3));
        assert_eq!(explain.execution_identity.cookie_persist_count, Some(4));
        assert_eq!(
            explain.execution_identity.local_storage_restore_count,
            Some(2)
        );
        assert_eq!(
            explain.execution_identity.local_storage_persist_count,
            Some(5)
        );
        assert_eq!(
            explain.execution_identity.session_storage_restore_count,
            Some(1)
        );
        assert_eq!(
            explain.execution_identity.session_storage_persist_count,
            Some(6)
        );
        assert_eq!(
            explain
                .identity_network_explain
                .as_ref()
                .and_then(|v| v.identity_session_status.as_deref()),
            Some("auto_reused")
        );
        assert_eq!(
            explain
                .identity_network_explain
                .as_ref()
                .and_then(|v| v.local_storage_restore_count),
            Some(2)
        );
        assert_eq!(
            explain
                .identity_network_explain
                .as_ref()
                .and_then(|v| v.local_storage_persist_count),
            Some(5)
        );
        assert_eq!(
            explain
                .identity_network_explain
                .as_ref()
                .and_then(|v| v.session_storage_restore_count),
            Some(1)
        );
        assert_eq!(
            explain
                .identity_network_explain
                .as_ref()
                .and_then(|v| v.session_storage_persist_count),
            Some(6)
        );
        assert!(explain.winner_vs_runner_up_diff.is_some());
        assert_eq!(explain.failure_scope.as_deref(), Some("browser_execution"));
        assert_eq!(
            explain.browser_failure_signal.as_deref(),
            Some("browser_navigation_failure_signal")
        );
        assert_eq!(explain.summary_artifacts.len(), 6);
        assert!(explain
            .summary_artifacts
            .iter()
            .all(|a| a.task_id.as_deref() == Some("task-1")));
        assert!(explain
            .summary_artifacts
            .iter()
            .all(|a| a.task_kind.as_deref() == Some("open_page")));
        assert!(explain
            .summary_artifacts
            .iter()
            .all(|a| a.task_status.as_deref() == Some("succeeded")));
        assert!(explain
            .summary_artifacts
            .iter()
            .all(|a| a.timestamp.as_deref().is_some()));
    }
}
