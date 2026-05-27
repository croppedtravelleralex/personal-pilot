use async_trait::async_trait;
use serde_json::{json, Value};

use crate::{
    domain::run::RUN_STATUS_FAILED,
    runner::{
        RunnerCapabilities, RunnerExecutionResult, RunnerOutcomeStatus, RunnerTask, TaskRunner,
    },
};

const CAMOUFOX_RUNNER_MODE: &str = "camoufox_skeleton";
const ENABLED_ENV: &str = "PERSONA_PILOT_CAMOUFOX_ENABLED";
const CONFIG_ENV: &str = "PERSONA_PILOT_CAMOUFOX_CONFIG";

#[derive(Default)]
pub struct CamoufoxRunner;

fn env_enabled() -> bool {
    std::env::var(ENABLED_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn config_path() -> Option<String> {
    std::env::var(CONFIG_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn requested_action(task: &RunnerTask) -> String {
    task.payload
        .get("action")
        .and_then(|value| value.as_str())
        .unwrap_or(task.kind.as_str())
        .to_string()
}

fn result_payload(
    task: &RunnerTask,
    requested_action: &str,
    error_kind: &str,
    message: &str,
    config_path: Option<String>,
) -> Value {
    let url = task
        .payload
        .get("url")
        .and_then(|value| value.as_str())
        .map(str::to_owned);

    json!({
        "runner": "camoufox",
        "runner_mode": CAMOUFOX_RUNNER_MODE,
        "is_fake": false,
        "real_browser_execution": false,
        "browser_launch_attempted": false,
        "requested_action": requested_action,
        "action": requested_action,
        "supported_actions": ["open_page", "fetch", "get_html", "get_title", "get_final_url", "extract_text", "validation_probe"],
        "ok": false,
        "status": RUN_STATUS_FAILED,
        "error_kind": error_kind,
        "failure_scope": "runner_configuration",
        "browser_failure_signal": Value::Null,
        "execution_stage": "configuration",
        "task_id": task.task_id,
        "attempt": task.attempt,
        "kind": task.kind,
        "payload": task.payload,
        "url": url,
        "timeout_seconds": task.timeout_seconds.and_then(|value| u64::try_from(value).ok()),
        "config_env": CONFIG_ENV,
        "config_path": config_path,
        "enabled_env": ENABLED_ENV,
        "message": message,
    })
}

fn build_configuration_failure(
    task: &RunnerTask,
    requested_action: &str,
    error_kind: &'static str,
    message: impl Into<String>,
    config_path: Option<String>,
) -> RunnerExecutionResult {
    let message = message.into();

    RunnerExecutionResult {
        status: RunnerOutcomeStatus::Failed,
        result_json: Some(result_payload(
            task,
            requested_action,
            error_kind,
            &message,
            config_path,
        )),
        error_message: Some(message.clone()),
        summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
            category: crate::runner::types::SummaryArtifactCategory::Execution,
            key: format!("{}.execution", task.kind),
            source: "runner.camoufox".to_string(),
            severity: crate::runner::types::SummaryArtifactSeverity::Error,
            title: format!("{} camoufox runner skeleton summary", task.kind),
            summary: format!(
                "camoufox skeleton did not launch a browser; action={} error_kind={} message={}",
                requested_action, error_kind, message
            ),
        }],
        session_cookies: None,
        session_local_storage: None,
        session_session_storage: None,
    }
}

#[async_trait]
impl TaskRunner for CamoufoxRunner {
    fn name(&self) -> &'static str {
        "camoufox"
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_timeout: true,
            supports_cancel_running: false,
            supports_artifacts: false,
        }
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult {
        let requested_action = requested_action(&task);
        let config_path = config_path();

        if !env_enabled() {
            return build_configuration_failure(
                &task,
                requested_action.as_str(),
                "runner_disabled",
                format!(
                    "camoufox runner is disabled; set {ENABLED_ENV}=true after configuring {CONFIG_ENV}"
                ),
                config_path,
            );
        }

        if config_path.is_none() {
            return build_configuration_failure(
                &task,
                requested_action.as_str(),
                "runner_config_missing",
                format!("camoufox runner requires non-empty {CONFIG_ENV} before execution"),
                None,
            );
        }

        build_configuration_failure(
            &task,
            requested_action.as_str(),
            "runner_not_implemented",
            "camoufox runner skeleton is configured but browser launch is not implemented in this phase",
            config_path,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn task() -> RunnerTask {
        RunnerTask {
            task_id: "task-camoufox-disabled".to_string(),
            attempt: 1,
            kind: "open_page".to_string(),
            payload: json!({"url": "https://example.com"}),
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
        }
    }

    #[tokio::test]
    async fn execute_returns_disabled_configuration_failure_without_launching_browser() {
        std::env::remove_var(ENABLED_ENV);
        std::env::remove_var(CONFIG_ENV);

        let result = CamoufoxRunner.execute(task()).await;
        let json = result.result_json.expect("result json");

        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        assert_eq!(
            json.get("runner").and_then(|value| value.as_str()),
            Some("camoufox")
        );
        assert_eq!(
            json.get("error_kind").and_then(|value| value.as_str()),
            Some("runner_disabled")
        );
        assert_eq!(
            json.get("browser_launch_attempted")
                .and_then(|value| value.as_bool()),
            Some(false)
        );
        assert_eq!(result.summary_artifacts[0].source, "runner.camoufox");
    }
}
