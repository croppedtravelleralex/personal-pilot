pub mod fake;
pub mod lightpanda;
pub mod types;

use async_trait::async_trait;

use crate::app::state::AppState;
pub use types::{RunnerCapabilities, RunnerExecutionResult, RunnerOutcomeStatus, RunnerTask};

#[derive(Debug, Clone, Copy)]
pub enum RunnerKind {
    Fake,
    Lightpanda,
}

impl RunnerKind {
    pub fn from_env() -> Self {
        match std::env::var("AUTO_OPEN_BROWSER_RUNNER")
            .ok()
            .unwrap_or_else(|| "fake".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "lightpanda" => RunnerKind::Lightpanda,
            _ => RunnerKind::Fake,
        }
    }
}

#[async_trait]
pub trait TaskRunner: Send + Sync {
    fn name(&self) -> &'static str;

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_timeout: true,
            supports_cancel_running: false,
            supports_artifacts: false,
        }
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult;
}

pub async fn spawn_runner_loop<R>(state: AppState, runner: R)
where
    R: TaskRunner + 'static,
{
    tokio::spawn(async move {
        loop {
            let _ = fake::run_one_task_with_runner(&state, &runner).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    });
}
