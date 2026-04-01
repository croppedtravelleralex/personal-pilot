use serde_json::Value;

use super::dto::{CandidateRankPreviewItem, SummaryArtifactResponse, TaskResponse, WinnerVsRunnerUpDiff};

#[derive(Debug, Clone)]
pub struct TaskExplainability {
    pub fingerprint_resolution_status: Option<String>,
    pub proxy_id: Option<String>,
    pub proxy_provider: Option<String>,
    pub proxy_region: Option<String>,
    pub proxy_resolution_status: Option<String>,
    pub trust_score_total: Option<i64>,
    pub selection_reason_summary: Option<String>,
    pub winner_vs_runner_up_diff: Option<WinnerVsRunnerUpDiff>,
    pub summary_artifacts: Vec<SummaryArtifactResponse>,
}

pub fn fingerprint_resolution_status(
    fingerprint_profile_id: Option<&str>,
    fingerprint_profile_version: Option<i64>,
    result_json: Option<&str>,
) -> Option<String> {
    let profile_id = fingerprint_profile_id?;
    let profile_version = fingerprint_profile_version?;

    let parsed = result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok());

    if parsed
        .as_ref()
        .and_then(|json| json.get("fingerprint_profile"))
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        == Some(profile_id)
        && parsed
            .as_ref()
            .and_then(|json| json.get("fingerprint_profile"))
            .and_then(|value| value.get("version"))
            .and_then(|value| value.as_i64())
            == Some(profile_version)
    {
        return Some("resolved".to_string());
    }

    if parsed
        .as_ref()
        .and_then(|json| json.get("fingerprint_profile"))
        .map(|value| value.is_null())
        == Some(true)
    {
        return Some("downgraded".to_string());
    }

    Some("pending".to_string())
}

pub fn proxy_resolution_status(result_json: Option<&str>) -> Option<String> {
    let parsed = result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok())?;
    parsed
        .get("proxy")
        .and_then(|value| value.get("resolution_status"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| {
            parsed
                .get("payload")
                .and_then(|value| value.get("network_policy_json"))
                .and_then(|value| value.get("proxy_resolution_status"))
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
}

pub fn proxy_identity(result_json: Option<&str>) -> (Option<String>, Option<String>, Option<String>) {
    let parsed = result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    let proxy = parsed.as_ref().and_then(|json| json.get("proxy"));
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

pub fn selection_reason_summary(result_json: Option<&str>) -> Option<String> {
    let parsed = result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok())?;
    parsed
        .get("payload")
        .and_then(|value| value.get("network_policy_json"))
        .and_then(|value| value.get("selection_reason_summary"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

pub fn trust_score_total(result_json: Option<&str>) -> Option<i64> {
    let parsed = result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok())?;
    parsed
        .get("payload")
        .and_then(|value| value.get("network_policy_json"))
        .and_then(|value| value.get("trust_score_total"))
        .and_then(|value| value.as_i64())
}

pub fn candidate_rank_preview(result_json: Option<&str>) -> Vec<CandidateRankPreviewItem> {
    let parsed = result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok());
    parsed
        .and_then(|value| value.get("payload").cloned())
        .and_then(|value| value.get("network_policy_json").cloned())
        .and_then(|value| value.get("candidate_rank_preview").cloned())
        .and_then(|value| serde_json::from_value::<Vec<CandidateRankPreviewItem>>(value).ok())
        .unwrap_or_default()
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
        "error" | "warning" | "info" => severity.unwrap_or("info").to_string(),
        _ => "info".to_string(),
    }
}

fn selection_decision_summary_artifact(result_json: Option<&str>) -> Option<SummaryArtifactResponse> {
    let diff = winner_vs_runner_up_diff(result_json)?;
    let factor_summary = diff
        .factors
        .iter()
        .take(2)
        .map(|factor| format!("{}({:+})", factor.label, factor.delta))
        .collect::<Vec<_>>()
        .join(", ");
    let summary = if factor_summary.is_empty() {
        format!("winner beat runner-up by {} trust-score points", diff.score_gap)
    } else {
        format!(
            "winner beat runner-up by {} trust-score points; top factors: {}",
            diff.score_gap, factor_summary
        )
    };
    Some(SummaryArtifactResponse {
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
    })
}

pub fn summary_artifacts(result_json: Option<&str>) -> Vec<SummaryArtifactResponse> {
    let parsed = result_json.and_then(|raw| serde_json::from_str::<Value>(raw).ok());
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
        if let Some(artifact) = selection_decision_summary_artifact(result_json) {
            artifacts.push(artifact);
        }
    }

    artifacts
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
    let proxy_resolution_status = proxy_resolution_status(result_json);
    let (proxy_id, proxy_provider, proxy_region) = proxy_identity(result_json);
    let trust_score_total = trust_score_total(result_json);
    let selection_reason_summary = selection_reason_summary(result_json);
    let winner_vs_runner_up_diff = winner_vs_runner_up_diff(result_json);
    let summary_artifacts = enrich_summary_artifacts(
        summary_artifacts(result_json),
        task_id,
        task_kind,
        task_status,
        None,
        None,
        timestamp,
    );

    TaskExplainability {
        fingerprint_resolution_status: fingerprint_resolution_status(
            fingerprint_profile_id,
            fingerprint_profile_version,
            result_json,
        ),
        proxy_id,
        proxy_provider,
        proxy_region,
        proxy_resolution_status,
        trust_score_total,
        selection_reason_summary,
        winner_vs_runner_up_diff,
        summary_artifacts,
    }
}
