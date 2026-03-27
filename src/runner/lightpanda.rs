use async_trait::async_trait;
use serde_json::json;

use crate::runner::{RunnerExecutionResult, RunnerOutcomeStatus, RunnerTask, TaskRunner};

pub struct LightpandaRunner;

#[async_trait]
impl TaskRunner for LightpandaRunner {
    fn name(&self) -> &'static str {
        "lightpanda"
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult {
        RunnerExecutionResult {
            status: RunnerOutcomeStatus::Failed,
            result_json: Some(json!({
                "runner": self.name(),
                "task_id": task.task_id,
                "message": "lightpanda runner adapter placeholder: real browser execution not implemented yet"
            })),
            error_message: Some("lightpanda runner is not implemented yet".to_string()),
        }
    }
}
