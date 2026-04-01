use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

use crate::{
    domain::run::{RUN_STATUS_FAILED, RUN_STATUS_SUCCEEDED, RUN_STATUS_TIMED_OUT},
    runner::{RunnerExecutionResult, RunnerOutcomeStatus, RunnerTask, TaskRunner},
};

pub struct FakeRunner;

fn result_payload(
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    task: &RunnerTask,
    message: &str,
) -> Value {
    let url = task
        .payload
        .get("url")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let timeout_seconds = task
        .timeout_seconds
        .and_then(|value| u64::try_from(value).ok());

    let fingerprint_profile = task.fingerprint_profile.as_ref().map(|profile| json!({
        "id": profile.id,
        "version": profile.version,
        "profile": profile.profile_json,
    }));
    let proxy = task.proxy.as_ref().map(|proxy| json!({
        "id": proxy.id,
        "scheme": proxy.scheme,
        "host": proxy.host,
        "port": proxy.port,
        "region": proxy.region,
        "country": proxy.country,
        "provider": proxy.provider,
        "score": proxy.score,
        "resolution_status": proxy.resolution_status,
    }));

    json!({
        "runner": "fake",
        "action": "simulate",
        "ok": ok,
        "status": status,
        "error_kind": error_kind,
        "task_id": task.task_id,
        "attempt": task.attempt,
        "kind": task.kind,
        "payload": task.payload,
        "url": url,
        "timeout_seconds": timeout_seconds,
        "fingerprint_profile": fingerprint_profile,
        "proxy": proxy,
        "bin": Value::Null,
        "exit_code": Value::Null,
        "stdout_preview": Value::Null,
        "stderr_preview": Value::Null,
        "message": message,
    })
}

fn build_result(
    outcome: RunnerOutcomeStatus,
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    task: &RunnerTask,
    message: impl Into<String>,
) -> RunnerExecutionResult {
    let message = message.into();
    let is_error = matches!(outcome, RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::TimedOut);

    RunnerExecutionResult {
        status: outcome,
        result_json: Some(result_payload(ok, status, error_kind, task, &message)),
        error_message: is_error.then_some(message.clone()),
        summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
            category: if is_error { crate::runner::types::SummaryArtifactCategory::Debug } else { crate::runner::types::SummaryArtifactCategory::Summary },
            title: "fake runner summary".to_string(),
            summary: message,
        }],
    }
}

#[async_trait]
impl TaskRunner for FakeRunner {
    fn name(&self) -> &'static str {
        "fake"
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult {
        sleep(Duration::from_millis(300)).await;

        match task.kind.as_str() {
            "fail" => build_result(
                RunnerOutcomeStatus::Failed,
                false,
                RUN_STATUS_FAILED,
                Some("simulated_failure"),
                &task,
                "simulated failure by fake runner",
            ),
            "timeout" => build_result(
                RunnerOutcomeStatus::TimedOut,
                false,
                RUN_STATUS_TIMED_OUT,
                Some("timeout"),
                &task,
                "simulated timeout by fake runner",
            ),
            _ => build_result(
                RunnerOutcomeStatus::Succeeded,
                true,
                RUN_STATUS_SUCCEEDED,
                None,
                &task,
                "task completed by fake runner",
            ),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn execute_success_exposes_aligned_result_fields() {
        let runner = FakeRunner;
        let task = RunnerTask {
            task_id: "task-fake-success".to_string(),
            attempt: 2,
            kind: "open_page".to_string(),
            payload: json!({"url": "https://example.com", "foo": "bar"}),
            timeout_seconds: Some(7),
            fingerprint_profile: None,
            proxy: None,
        };

        let result = runner.execute(task.clone()).await;
        let json = result.result_json.expect("result json");

        assert!(matches!(result.status, RunnerOutcomeStatus::Succeeded));
        assert_eq!(json.get("runner").and_then(|v| v.as_str()), Some("fake"));
        assert_eq!(json.get("action").and_then(|v| v.as_str()), Some("simulate"));
        assert_eq!(json.get("ok").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(json.get("status").and_then(|v| v.as_str()), Some(RUN_STATUS_SUCCEEDED));
        assert_eq!(json.get("task_id").and_then(|v| v.as_str()), Some("task-fake-success"));
        assert_eq!(json.get("attempt").and_then(|v| v.as_i64()), Some(2));
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("open_page"));
        assert_eq!(json.get("url").and_then(|v| v.as_str()), Some("https://example.com"));
        assert_eq!(json.get("timeout_seconds").and_then(|v| v.as_u64()), Some(7));
        assert_eq!(json.get("payload").and_then(|v| v.get("foo")).and_then(|v| v.as_str()), Some("bar"));
        assert!(json.get("bin").is_some());
        assert!(json.get("exit_code").is_some());
        assert!(json.get("stdout_preview").is_some());
        assert!(json.get("stderr_preview").is_some());
    }

    #[tokio::test]
    async fn execute_timeout_exposes_timed_out_status_and_error_kind() {
        let runner = FakeRunner;
        let task = RunnerTask {
            task_id: "task-fake-timeout".to_string(),
            attempt: 1,
            kind: "timeout".to_string(),
            payload: json!({"url": "https://example.com"}),
            timeout_seconds: Some(3),
            fingerprint_profile: None,
            proxy: None,
        };

        let result = runner.execute(task).await;
        let json = result.result_json.expect("result json");

        assert!(matches!(result.status, RunnerOutcomeStatus::TimedOut));
        assert_eq!(json.get("status").and_then(|v| v.as_str()), Some(RUN_STATUS_TIMED_OUT));
        assert_eq!(json.get("error_kind").and_then(|v| v.as_str()), Some("timeout"));
    }
}
