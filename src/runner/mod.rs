pub mod engine;
pub mod fake;
pub mod lightpanda;
pub mod types;

use std::sync::Arc;

use async_trait::async_trait;

use crate::app::state::AppState;
pub use types::{
    RunnerCancelResult, RunnerCapabilities, RunnerExecutionResult, RunnerFingerprintProfile,
    RunnerOutcomeStatus, RunnerTask,
};

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

pub fn runner_concurrency_from_env() -> usize {
    std::env::var("AUTO_OPEN_BROWSER_RUNNER_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1)
}

pub fn runner_reclaim_seconds_from_env() -> Option<u64> {
    std::env::var("AUTO_OPEN_BROWSER_RUNNER_RECLAIM_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
}

pub fn runner_heartbeat_interval_seconds_from_env() -> u64 {
    std::env::var("AUTO_OPEN_BROWSER_RUNNER_HEARTBEAT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(5)
}

pub fn runner_claim_retry_limit_from_env() -> u32 {
    std::env::var("AUTO_OPEN_BROWSER_RUNNER_CLAIM_RETRY_LIMIT")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(8)
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

    async fn cancel_running(&self, task_id: &str) -> RunnerCancelResult {
        let _ = task_id;
        RunnerCancelResult {
            accepted: false,
            message: format!("runner {} does not support running cancel", self.name()),
        }
    }
}

pub async fn spawn_runner_workers(state: AppState, runner: Arc<dyn TaskRunner>, worker_count: usize) {
    let worker_count = worker_count.max(1);
    let reclaim_after_seconds = runner_reclaim_seconds_from_env();

    for worker_id in 0..worker_count {
        let state = state.clone();
        let runner = runner.clone();
        tokio::spawn(async move {
            let worker_label = format!("{}-{}", runner.name(), worker_id);
            loop {
                if let Some(reclaim_after_seconds) = reclaim_after_seconds {
                    if let Err(err) = engine::reclaim_stale_running_tasks(&state, reclaim_after_seconds).await {
                        eprintln!("runner reclaim error: worker_id={}, runner={}, error={}", worker_id, runner.name(), err);
                    }
                }
                if let Err(err) = engine::run_one_task_with_runner(&state, runner.as_ref(), &worker_label).await {
                    eprintln!(
                        "runner worker error: worker_id={}, runner={}, error={}",
                        worker_id,
                        runner.name(),
                        err
                    );
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }
        });
    }
}
