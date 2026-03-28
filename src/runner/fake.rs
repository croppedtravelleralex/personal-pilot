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
        error_message: is_error.then_some(message),
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
