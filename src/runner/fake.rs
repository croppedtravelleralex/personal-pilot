use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::time::{sleep, Duration};

use crate::{
    domain::run::{RUN_STATUS_FAILED, RUN_STATUS_SUCCEEDED, RUN_STATUS_TIMED_OUT},
    runner::{RunnerExecutionResult, RunnerOutcomeStatus, RunnerTask, TaskRunner},
};

pub struct FakeRunner;

const FAKE_RUNNER_MODE: &str = "fake_stub";

fn simulated_action(task: &RunnerTask) -> &'static str {
    match task.kind.as_str() {
        "get_html" => "get_html",
        "get_title" => "get_title",
        "get_final_url" => "get_final_url",
        "extract_text" => "extract_text",
        _ => "open_page",
    }
}

fn simulated_content_fields(
    action: &str,
    url: Option<&str>,
) -> (Value, Value, Value, Value, Value, Value, Value) {
    match action {
        "get_html" => {
            let html = format!(
                "<html><body><main data-url=\"{}\">fake html content</main></body></html>",
                url.unwrap_or("https://example.com")
            );
            let len = html.chars().count() as u64;
            (
                json!(html.clone()),
                json!(len),
                json!(false),
                json!(html),
                json!(len),
                json!(false),
                json!("text/html"),
            )
        }
        "extract_text" => {
            let text = match url {
                Some(value) => format!("fake extracted text from {value}"),
                None => "fake extracted text".to_string(),
            };
            let len = text.chars().count() as u64;
            (
                Value::Null,
                Value::Null,
                Value::Null,
                json!(text.clone()),
                json!(len),
                json!(false),
                json!("text/plain"),
            )
        }
        _ => (
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null,
        ),
    }
}

fn failure_scope(outcome: RunnerOutcomeStatus) -> Option<&'static str> {
    match outcome {
        RunnerOutcomeStatus::Succeeded => None,
        RunnerOutcomeStatus::Failed => Some("runner_process_exit"),
        RunnerOutcomeStatus::Cancelled => Some("runner_cancelled"),
        RunnerOutcomeStatus::TimedOut => Some("runner_timeout"),
    }
}

fn execution_stage(outcome: RunnerOutcomeStatus, action: &str) -> Option<&'static str> {
    let is_content_action = matches!(action, "get_html" | "extract_text");
    match outcome {
        RunnerOutcomeStatus::Succeeded => Some(if is_content_action {
            "output_wait"
        } else {
            "action"
        }),
        RunnerOutcomeStatus::TimedOut => Some(if is_content_action {
            "output_wait"
        } else {
            "navigate"
        }),
        RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::Cancelled => Some("action"),
    }
}

fn result_payload(
    outcome: RunnerOutcomeStatus,
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
    let action = simulated_action(task);

    let fingerprint_profile = task.fingerprint_profile.as_ref().map(|profile| {
        json!({
            "id": profile.id,
            "version": profile.version,
            "profile": profile.profile_json,
        })
    });
    let proxy = task.proxy.as_ref().map(|proxy| {
        json!({
            "id": proxy.id,
            "scheme": proxy.scheme,
            "host": proxy.host,
            "port": proxy.port,
            "region": proxy.region,
            "country": proxy.country,
            "provider": proxy.provider,
            "score": proxy.score,
            "resolution_status": proxy.resolution_status,
        })
    });
    let title = url.as_ref().map(|value| format!("Fake title for {value}"));
    let final_url = url.as_ref().map(|value| format!("{value}#final"));
    let (
        html_preview,
        html_length,
        html_truncated,
        content_preview,
        content_length,
        content_truncated,
        content_kind,
    ) = simulated_content_fields(action, url.as_deref());
    let content_source_action = match action {
        "get_html" | "extract_text" => json!(action),
        _ => Value::Null,
    };
    let content_ready = match action {
        "get_html" | "extract_text" => json!(true),
        _ => Value::Null,
    };

    json!({
        "runner": "fake",
        "runner_mode": FAKE_RUNNER_MODE,
        "is_fake": true,
        "real_browser_execution": false,
        "requested_action": action,
        "action": action,
        "supported_actions": ["open_page", "fetch", "get_html", "get_title", "get_final_url", "extract_text"],
        "ok": ok,
        "status": status,
        "error_kind": error_kind,
        "failure_scope": failure_scope(outcome),
        "browser_failure_signal": Value::Null,
        "execution_stage": execution_stage(outcome, action),
        "task_id": task.task_id,
        "attempt": task.attempt,
        "kind": task.kind,
        "payload": task.payload,
        "url": url,
        "timeout_seconds": timeout_seconds,
        "fingerprint_profile": fingerprint_profile,
        "proxy": proxy,
        "title": title,
        "final_url": final_url,
        "html_preview": html_preview,
        "html_length": html_length,
        "html_truncated": html_truncated,
        "text_preview": if action == "extract_text" { content_preview.clone() } else { Value::Null },
        "text_length": if action == "extract_text" { content_length.clone() } else { Value::Null },
        "text_truncated": if action == "extract_text" { content_truncated.clone() } else { Value::Null },
        "content_preview": content_preview,
        "content_length": content_length,
        "content_truncated": content_truncated,
        "content_encoding": match action {
            "get_html" => json!("html"),
            "extract_text" => json!("plain"),
            _ => Value::Null,
        },
        "content_source_action": content_source_action,
        "content_ready": content_ready,
        "content_kind": content_kind,
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
    let is_error = matches!(
        outcome,
        RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::TimedOut
    );

    RunnerExecutionResult {
        status: outcome,
        result_json: Some(result_payload(
            outcome, ok, status, error_kind, task, &message,
        )),
        error_message: is_error.then_some(message.clone()),
        summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
            category: crate::runner::types::SummaryArtifactCategory::Execution,
            key: format!("{}.execution", task.kind),
            source: "runner.fake".to_string(),
            severity: if is_error {
                crate::runner::types::SummaryArtifactSeverity::Error
            } else {
                crate::runner::types::SummaryArtifactSeverity::Info
            },
            title: format!("{} fake runner stub summary", task.kind),
            summary: format!(
                "fake/stub result only; kind={} action={} status={} message={}",
                task.kind,
                simulated_action(task),
                status,
                message
            ),
        }],
        session_cookies: None,
        session_local_storage: None,
        session_session_storage: None,
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
                "fake runner stub simulated failure",
            ),
            "timeout" => build_result(
                RunnerOutcomeStatus::TimedOut,
                false,
                RUN_STATUS_TIMED_OUT,
                Some("timeout"),
                &task,
                "fake runner stub simulated timeout",
            ),
            _ => build_result(
                RunnerOutcomeStatus::Succeeded,
                true,
                RUN_STATUS_SUCCEEDED,
                None,
                &task,
                "fake runner stub completed task; no real browser session was used",
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
            execution_intent: None,
            fingerprint_profile: None,
            behavior_profile: None,
            behavior_plan: None,
            form_action_plan: None,
            proxy: None,
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        };

        let result = runner.execute(task.clone()).await;
        let json = result.result_json.expect("result json");

        assert!(matches!(result.status, RunnerOutcomeStatus::Succeeded));
        assert_eq!(json.get("runner").and_then(|v| v.as_str()), Some("fake"));
        assert_eq!(
            json.get("runner_mode").and_then(|v| v.as_str()),
            Some(FAKE_RUNNER_MODE)
        );
        assert_eq!(json.get("is_fake").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            json.get("real_browser_execution").and_then(|v| v.as_bool()),
            Some(false)
        );
        assert_eq!(
            json.get("requested_action").and_then(|v| v.as_str()),
            Some("open_page")
        );
        assert_eq!(
            json.get("action").and_then(|v| v.as_str()),
            Some("open_page")
        );
        assert_eq!(json.get("ok").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            json.get("status").and_then(|v| v.as_str()),
            Some(RUN_STATUS_SUCCEEDED)
        );
        assert_eq!(json.get("failure_scope").and_then(|v| v.as_str()), None);
        assert_eq!(json.get("browser_failure_signal"), Some(&Value::Null));
        assert_eq!(
            json.get("execution_stage").and_then(|v| v.as_str()),
            Some("action")
        );
        assert_eq!(
            json.get("task_id").and_then(|v| v.as_str()),
            Some("task-fake-success")
        );
        assert_eq!(json.get("attempt").and_then(|v| v.as_i64()), Some(2));
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("open_page"));
        assert_eq!(
            json.get("url").and_then(|v| v.as_str()),
            Some("https://example.com")
        );
        assert_eq!(
            json.get("timeout_seconds").and_then(|v| v.as_u64()),
            Some(7)
        );
        assert_eq!(
            json.get("payload")
                .and_then(|v| v.get("foo"))
                .and_then(|v| v.as_str()),
            Some("bar")
        );
        assert!(json
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("no real browser session"));
        assert_eq!(result.summary_artifacts.len(), 1);
        assert_eq!(result.summary_artifacts[0].source, "runner.fake");
        assert!(result.summary_artifacts[0]
            .summary
            .contains("fake/stub result only"));
        assert!(json.get("bin").is_some());
        assert!(json.get("exit_code").is_some());
        assert!(json.get("stdout_preview").is_some());
        assert!(json.get("stderr_preview").is_some());
    }

    #[tokio::test]
    async fn execute_extract_text_exposes_content_contract() {
        let runner = FakeRunner;
        let task = RunnerTask {
            task_id: "task-fake-extract-text".to_string(),
            attempt: 1,
            kind: "extract_text".to_string(),
            payload: json!({"url": "https://example.com/article"}),
            timeout_seconds: Some(5),
            execution_intent: None,
            fingerprint_profile: None,
            behavior_profile: None,
            behavior_plan: None,
            form_action_plan: None,
            proxy: None,
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        };

        let result = runner.execute(task).await;
        let json = result.result_json.expect("result json");

        assert!(matches!(result.status, RunnerOutcomeStatus::Succeeded));
        assert_eq!(
            json.get("requested_action").and_then(|v| v.as_str()),
            Some("extract_text")
        );
        assert_eq!(
            json.get("action").and_then(|v| v.as_str()),
            Some("extract_text")
        );
        assert_eq!(
            json.get("execution_stage").and_then(|v| v.as_str()),
            Some("output_wait")
        );
        assert_eq!(
            json.get("content_kind").and_then(|v| v.as_str()),
            Some("text/plain")
        );
        assert_eq!(
            json.get("content_source_action").and_then(|v| v.as_str()),
            Some("extract_text")
        );
        assert_eq!(
            json.get("content_ready").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert!(json
            .get("content_preview")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("fake extracted text"));
        assert!(
            json.get("text_length")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                > 0
        );
    }

    #[tokio::test]
    async fn execute_title_and_final_url_expose_browser_metadata() {
        let runner = FakeRunner;
        let task = RunnerTask {
            task_id: "task-fake-title".to_string(),
            attempt: 1,
            kind: "get_title".to_string(),
            payload: json!({"url": "https://example.com/page"}),
            timeout_seconds: Some(5),
            execution_intent: None,
            fingerprint_profile: None,
            behavior_profile: None,
            behavior_plan: None,
            form_action_plan: None,
            proxy: None,
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        };

        let json = runner.execute(task).await.result_json.expect("result json");
        assert_eq!(
            json.get("title").and_then(|v| v.as_str()),
            Some("Fake title for https://example.com/page")
        );
        assert_eq!(
            json.get("final_url").and_then(|v| v.as_str()),
            Some("https://example.com/page#final")
        );
    }

    #[tokio::test]
    async fn execute_fail_exposes_failure_scope_and_stage() {
        let runner = FakeRunner;
        let task = RunnerTask {
            task_id: "task-fake-fail".to_string(),
            attempt: 1,
            kind: "fail".to_string(),
            payload: json!({"url": "https://example.com"}),
            timeout_seconds: Some(3),
            execution_intent: None,
            fingerprint_profile: None,
            behavior_profile: None,
            behavior_plan: None,
            form_action_plan: None,
            proxy: None,
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        };

        let result = runner.execute(task).await;
        let json = result.result_json.expect("result json");

        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        assert_eq!(
            json.get("status").and_then(|v| v.as_str()),
            Some(RUN_STATUS_FAILED)
        );
        assert_eq!(
            json.get("error_kind").and_then(|v| v.as_str()),
            Some("simulated_failure")
        );
        assert_eq!(
            json.get("failure_scope").and_then(|v| v.as_str()),
            Some("runner_process_exit")
        );
        assert_eq!(json.get("browser_failure_signal"), Some(&Value::Null));
        assert_eq!(
            json.get("execution_stage").and_then(|v| v.as_str()),
            Some("action")
        );
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
            execution_intent: None,
            fingerprint_profile: None,
            behavior_profile: None,
            behavior_plan: None,
            form_action_plan: None,
            proxy: None,
            session_cookies: None,
            session_local_storage: None,
            session_session_storage: None,
        };

        let result = runner.execute(task).await;
        let json = result.result_json.expect("result json");

        assert!(matches!(result.status, RunnerOutcomeStatus::TimedOut));
        assert_eq!(
            json.get("status").and_then(|v| v.as_str()),
            Some(RUN_STATUS_TIMED_OUT)
        );
        assert_eq!(
            json.get("error_kind").and_then(|v| v.as_str()),
            Some("timeout")
        );
        assert_eq!(
            json.get("failure_scope").and_then(|v| v.as_str()),
            Some("runner_timeout")
        );
        assert_eq!(json.get("browser_failure_signal"), Some(&Value::Null));
        assert_eq!(
            json.get("execution_stage").and_then(|v| v.as_str()),
            Some("navigate")
        );
    }
}
