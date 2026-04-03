use serde_json::Value;
use std::time::Instant;

fn perf_probe_enabled() -> bool {
    std::env::var("AOB_PERF_PROBE")
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

use super::dto::{CandidateRankPreviewItem, FingerprintRuntimeExplain, IdentityNetworkExplain, ProxySelectionExplain, SummaryArtifactResponse, TaskResponse, WinnerVsRunnerUpDiff};

fn parse_result_json(result_json: Option<&str>) -> Option<Value> {
    result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok())
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

fn proxy_identity_from_parsed(parsed: Option<&Value>) -> (Option<String>, Option<String>, Option<String>) {
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

fn fingerprint_runtime_explain_from_parsed(parsed: Option<&Value>) -> Option<FingerprintRuntimeExplain> {
    parsed?
        .get("fingerprint_runtime_explain")
        .cloned()
        .and_then(|value| serde_json::from_value::<FingerprintRuntimeExplain>(value).ok())
}

fn trust_score_total_from_parsed(parsed: Option<&Value>) -> Option<i64> {
    parsed?
        .get("payload")
        .and_then(|value| value.get("network_policy_json"))
        .and_then(|value| value.get("trust_score_total"))
        .and_then(|value| value.as_i64())
}

fn candidate_rank_preview_from_parsed(parsed: Option<&Value>) -> Vec<CandidateRankPreviewItem> {
    parsed
        .and_then(|value| value.get("payload").cloned())
        .and_then(|value| value.get("network_policy_json").cloned())
        .and_then(|value| value.get("candidate_rank_preview").cloned())
        .and_then(|value| serde_json::from_value::<Vec<CandidateRankPreviewItem>>(value).ok())
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
pub struct TaskExplainability {
    pub fingerprint_resolution_status: Option<String>,
    pub proxy_id: Option<String>,
    pub proxy_provider: Option<String>,
    pub proxy_region: Option<String>,
    pub proxy_resolution_status: Option<String>,
    pub trust_score_total: Option<i64>,
    pub selection_reason_summary: Option<String>,
    pub selection_explain: Option<ProxySelectionExplain>,
    pub fingerprint_runtime_explain: Option<FingerprintRuntimeExplain>,
    pub identity_network_explain: Option<IdentityNetworkExplain>,
    pub winner_vs_runner_up_diff: Option<WinnerVsRunnerUpDiff>,
    pub summary_artifacts: Vec<SummaryArtifactResponse>,
}

pub fn fingerprint_resolution_status(
    fingerprint_profile_id: Option<&str>,
    fingerprint_profile_version: Option<i64>,
    result_json: Option<&str>,
) -> Option<String> {
    let parsed = parse_result_json(result_json);
    fingerprint_resolution_status_from_parsed(fingerprint_profile_id, fingerprint_profile_version, parsed.as_ref())
}

pub fn proxy_resolution_status(result_json: Option<&str>) -> Option<String> {
    let parsed = parse_result_json(result_json);
    proxy_resolution_status_from_parsed(parsed.as_ref())
}

pub fn proxy_identity(result_json: Option<&str>) -> (Option<String>, Option<String>, Option<String>) {
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
        "soft_min_score" => "soft min-score penalty",
        _ => "selection factor",
    }
}

fn selection_decision_summary_artifact_from_parsed(parsed: Option<&Value>) -> Option<SummaryArtifactResponse> {
    let started = Instant::now();
    let diff = candidate_rank_preview_from_parsed(parsed)
        .into_iter()
        .next()
        .and_then(|item| item.winner_vs_runner_up_diff)?;
    let factor_summary = diff
        .factors
        .iter()
        .take(2)
        .map(|factor| format!("{}({:+})", humanize_selection_factor_label(&factor.label), factor.delta))
        .collect::<Vec<_>>()
        .join(", ");
    let summary = if factor_summary.is_empty() {
        format!("selected this proxy by a {}-point trust-score margin", diff.score_gap)
    } else {
        format!(
            "selected this proxy by a {}-point trust-score margin; biggest reasons: {}",
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
        &[("elapsed_ms", started.elapsed().as_millis().to_string()), ("score_gap", diff.score_gap.to_string()), ("factor_count", diff.factors.len().to_string())],
    );
    Some(artifact)
}

fn identity_network_summary_artifact_from_parsed(parsed: Option<&Value>) -> Option<SummaryArtifactResponse> {
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
        parts.push(format!("resolution {}", status));
    }
    if let Some(tag) = fingerprint_runtime_explain
        .as_ref()
        .and_then(|v| v.fingerprint_budget_tag.as_deref())
    {
        parts.push(format!("fingerprint budget {}", tag));
    }
    if let Some(summary) = selection_reason_summary.as_deref() {
        parts.push(summary.to_string());
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

fn proxy_growth_summary_artifact_from_parsed(parsed: Option<&Value>) -> Option<SummaryArtifactResponse> {
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
        severity: if require_replenish { "warning".to_string() } else { "info".to_string() },
        title: "proxy growth assessment".to_string(),
        summary: if require_replenish {
            format!(
                "proxy pool is below target for this request; target region {} ; selected region {} ; availability {}% ({}) ; region signal {}",
                target_region,
                selected_proxy_region,
                available_ratio_percent,
                health_band,
                region_match_reason,
            )
        } else {
            format!(
                "proxy pool looks healthy for this request; target region {} ; selected region {} ; availability {}% ({}) ; region signal {}",
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
                    .unwrap_or_else(|| item.get("title").and_then(|v| v.as_str()).unwrap_or("summary.unknown"))
                    .to_string(),
                source: normalize_summary_source(item.get("source").and_then(|v| v.as_str())),
                severity: normalize_summary_severity(item.get("severity").and_then(|v| v.as_str())),
                title: item.get("title")?.as_str()?.to_string(),
                summary: item.get("summary")?.as_str()?.to_string(),
                task_id: None,
                task_kind: None,
                task_status: None,
                run_id: item.get("run_id").and_then(|v| v.as_str()).map(|v| v.to_string()),
                attempt: item
                    .get("attempt")
                    .and_then(|v| v.as_i64())
                    .and_then(|v| i32::try_from(v).ok()),
                timestamp: item.get("timestamp").and_then(|v| v.as_str()).map(|v| v.to_string()),
            })
        })
        .collect();

    let has_selection_decision = artifacts.iter().any(|item| item.title == "proxy selection decision");
    if !has_selection_decision {
        if let Some(artifact) = selection_decision_summary_artifact_from_parsed(parsed) {
            artifacts.push(artifact);
        }
    }

    let has_identity_network_summary = artifacts.iter().any(|item| item.title == "identity and network summary");
    if !has_identity_network_summary {
        if let Some(artifact) = identity_network_summary_artifact_from_parsed(parsed) {
            artifacts.push(artifact);
        }
    }

    let has_proxy_growth_summary = artifacts.iter().any(|item| item.title == "proxy growth assessment");
    if !has_proxy_growth_summary {
        if let Some(artifact) = proxy_growth_summary_artifact_from_parsed(parsed) {
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
            let dedupe_key = format!(
                "{}::{}::{}",
                artifact.task_id.clone().unwrap_or_default(),
                artifact.key,
                artifact.title
            );
            if seen.insert(dedupe_key) {
                items.push((task_index, artifact));
            }
        }
    }

    items.sort_by_key(|(task_index, artifact)| (summary_severity_rank(&artifact.severity), *task_index));
    items.truncate(5);
    items.into_iter().map(|(_, artifact)| artifact).collect()
}

pub fn build_task_explainability(
    fingerprint_profile_id: Option<&str>,
    fingerprint_profile_version: Option<i64>,
    result_json: Option<&str>,
    task_id: Option<&str>,
    task_kind: Option<&str>,
    task_status: Option<&str>,
    timestamp: Option<&str>,
) -> TaskExplainability {
    let parsed = parse_result_json(result_json);
    let parsed_ref = parsed.as_ref();
    let proxy_resolution_status = proxy_resolution_status_from_parsed(parsed_ref);
    let (proxy_id, proxy_provider, proxy_region) = proxy_identity_from_parsed(parsed_ref);
    let trust_score_total = trust_score_total_from_parsed(parsed_ref);
    let selection_reason_summary = selection_reason_summary_from_parsed(parsed_ref);
    let selection_explain = selection_explain_from_parsed(parsed_ref);
    let fingerprint_runtime_explain = fingerprint_runtime_explain_from_parsed(parsed_ref);
    let identity_network_explain = Some(IdentityNetworkExplain {
        selection_explain: selection_explain.clone(),
        fingerprint_runtime_explain: fingerprint_runtime_explain.clone(),
        proxy_id: proxy_id.clone(),
        proxy_provider: proxy_provider.clone(),
        proxy_region: proxy_region.clone(),
        proxy_resolution_status: proxy_resolution_status.clone(),
        selection_reason_summary: selection_reason_summary.clone(),
        trust_score_total,
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

    TaskExplainability {
        fingerprint_resolution_status: fingerprint_resolution_status_from_parsed(
            fingerprint_profile_id,
            fingerprint_profile_version,
            parsed_ref,
        ),
        proxy_id,
        proxy_provider,
        proxy_region,
        proxy_resolution_status,
        trust_score_total,
        selection_reason_summary,
        selection_explain,
        fingerprint_runtime_explain,
        identity_network_explain,
        winner_vs_runner_up_diff,
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
                }
            },
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
        assert_eq!(artifacts.len(), 4);

        let runner = artifacts.iter().find(|a| a.title == "fake runner summary").expect("runner artifact");
        assert_eq!(runner.category, "summary");
        assert_eq!(runner.source, "runner.fake");
        assert_eq!(runner.severity, "info");
        assert_eq!(runner.attempt, Some(1));
        assert_eq!(runner.timestamp.as_deref(), Some("123456"));
        assert_eq!(runner.key, "fake runner summary");

        let selection = artifacts.iter().find(|a| a.title == "proxy selection decision").expect("selection artifact");
        assert_eq!(selection.key, "proxy.selection.decision");
        assert_eq!(selection.source, "selection.proxy");
        assert_eq!(selection.severity, "info");
        assert!(selection.summary.contains("selected this proxy by a"));
        assert!(selection.summary.contains("biggest reasons"));

        let identity = artifacts.iter().find(|a| a.title == "identity and network summary").expect("identity artifact");
        assert_eq!(identity.key, "identity.network.summary");
        assert_eq!(identity.source, "selection.identity_network");
        assert_eq!(identity.severity, "info");
        assert!(identity.summary.contains("proxy pool-a@us-east"));
        assert!(identity.summary.contains("resolution resolved"));
        assert!(identity.summary.contains("fingerprint budget medium"));

        let growth = artifacts.iter().find(|a| a.title == "proxy growth assessment").expect("growth artifact");
        assert_eq!(growth.key, "proxy.selection.proxy_growth");
        assert_eq!(growth.source, "selection.proxy_growth");
        assert_eq!(growth.severity, "info");
        assert!(growth.summary.contains("proxy pool looks healthy for this request"));
        assert!(growth.summary.contains("target region us-east"));
        assert!(growth.summary.contains("region signal exact region match"));
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
                proxy_id: None,
                proxy_provider: None,
                proxy_region: None,
                proxy_resolution_status: None,
                trust_score_total: None,
                selection_reason_summary: None,
                selection_explain: None,
                fingerprint_runtime_explain: None,
                identity_network_explain: None,
                winner_vs_runner_up_diff: None,
            },
            TaskResponse {
                id: "task-2".to_string(),
                kind: "verify_proxy".to_string(),
                status: "failed".to_string(),
                priority: 1,
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
                    }
                ],
                fingerprint_profile_id: None,
                fingerprint_profile_version: None,
                fingerprint_resolution_status: None,
                proxy_id: None,
                proxy_provider: None,
                proxy_region: None,
                proxy_resolution_status: None,
                trust_score_total: None,
                selection_reason_summary: None,
                selection_explain: None,
                fingerprint_runtime_explain: None,
                identity_network_explain: None,
                winner_vs_runner_up_diff: None,
            },
        ];

        let latest = latest_execution_summaries(&tasks);
        assert_eq!(latest.len(), 3);
        assert_eq!(latest[0].severity, "error");
        assert_eq!(latest[0].task_id.as_deref(), Some("task-2"));
        assert!(latest.iter().filter(|item| item.title == "duplicate title").count() == 2);
        assert!(latest.iter().any(|item| item.task_id.as_deref() == Some("task-1")));
    }

    #[test]
    fn build_task_explainability_assembles_expected_fields() {
        let raw = sample_result_json_without_selection_artifact();
        let explain = build_task_explainability(
            Some("fp-1"),
            Some(2),
            Some(&raw),
            Some("task-1"),
            Some("open_page"),
            Some("succeeded"),
            Some("999999"),
        );
        assert_eq!(explain.fingerprint_resolution_status.as_deref(), Some("resolved"));
        assert_eq!(explain.proxy_id.as_deref(), Some("proxy-1"));
        assert_eq!(explain.proxy_provider.as_deref(), Some("pool-a"));
        assert_eq!(explain.proxy_region.as_deref(), Some("us-east"));
        assert_eq!(explain.proxy_resolution_status.as_deref(), Some("resolved"));
        assert_eq!(explain.trust_score_total, Some(90));
        assert_eq!(explain.selection_reason_summary.as_deref(), Some("winner has better trust score"));
        assert_eq!(explain.fingerprint_runtime_explain.as_ref().and_then(|v| v.fingerprint_budget_tag.as_deref()), Some("medium"));
        assert!(explain.fingerprint_runtime_explain.as_ref().and_then(|v| v.fingerprint_consistency.as_ref()).is_some());
        assert!(explain.identity_network_explain.is_some());
        assert_eq!(explain.identity_network_explain.as_ref().and_then(|v| v.proxy_provider.as_deref()), Some("pool-a"));
        assert_eq!(explain.identity_network_explain.as_ref().and_then(|v| v.fingerprint_runtime_explain.as_ref()).and_then(|v| v.fingerprint_budget_tag.as_deref()), Some("medium"));
        assert!(explain.winner_vs_runner_up_diff.is_some());
        assert_eq!(explain.summary_artifacts.len(), 4);
        assert!(explain.summary_artifacts.iter().all(|a| a.task_id.as_deref() == Some("task-1")));
        assert!(explain.summary_artifacts.iter().all(|a| a.task_kind.as_deref() == Some("open_page")));
        assert!(explain.summary_artifacts.iter().all(|a| a.task_status.as_deref() == Some("succeeded")));
        assert!(explain.summary_artifacts.iter().all(|a| a.timestamp.as_deref().is_some()));
    }
}
