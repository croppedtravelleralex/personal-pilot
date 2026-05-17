pub mod engine;
pub mod fake;
pub mod lightpanda;
pub mod types;

use std::sync::Arc;

use async_trait::async_trait;

use crate::app::state::AppState;
pub use types::{
    RunnerBehaviorPlan, RunnerBehaviorProfile, RunnerCancelResult, RunnerCapabilities,
    RunnerExecutionIntent, RunnerExecutionResult, RunnerFingerprintProfile, RunnerFormActionPlan,
    RunnerFormErrorSignals, RunnerFormFieldPlan, RunnerFormSubmitPlan, RunnerFormSuccessPlan,
    RunnerOutcomeStatus, RunnerProxySelection, RunnerTask,
};

#[derive(Debug, Clone, Copy)]
pub enum RunnerKind {
    Fake,
    Lightpanda,
}

impl RunnerKind {
    pub fn from_env() -> Self {
        match std::env::var("PERSONA_PILOT_RUNNER")
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
    std::env::var("PERSONA_PILOT_RUNNER_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(1)
}

pub fn runner_reclaim_seconds_from_env() -> Option<u64> {
    std::env::var("PERSONA_PILOT_RUNNER_RECLAIM_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
}

pub fn runner_heartbeat_interval_seconds_from_env() -> u64 {
    std::env::var("PERSONA_PILOT_RUNNER_HEARTBEAT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(5)
}

pub fn runner_claim_retry_limit_from_env() -> u32 {
    std::env::var("PERSONA_PILOT_RUNNER_CLAIM_RETRY_LIMIT")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(8)
}

pub fn runner_idle_backoff_min_ms_from_env() -> u64 {
    std::env::var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MIN_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(250)
}

pub fn runner_idle_backoff_max_ms_from_env() -> u64 {
    std::env::var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MAX_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3000)
        .max(runner_idle_backoff_min_ms_from_env())
}

pub fn next_runner_idle_backoff_ms(current_ms: u64) -> u64 {
    let min_ms = runner_idle_backoff_min_ms_from_env();
    let max_ms = runner_idle_backoff_max_ms_from_env();
    current_ms.saturating_mul(2).clamp(min_ms, max_ms)
}

pub fn runner_idle_backoff_jitter_ms_from_env() -> u64 {
    std::env::var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_JITTER_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(50)
}

pub fn runner_error_backoff_max_ms_from_env() -> u64 {
    std::env::var("PERSONA_PILOT_RUNNER_ERROR_BACKOFF_MAX_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(5000)
        .max(runner_idle_backoff_max_ms_from_env())
}

pub fn with_runner_backoff_jitter(base_ms: u64, worker_id: usize) -> u64 {
    let jitter = runner_idle_backoff_jitter_ms_from_env();
    if jitter == 0 {
        return base_ms;
    }
    base_ms.saturating_add((worker_id as u64 * 37) % (jitter + 1))
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

pub async fn spawn_runner_workers(
    state: AppState,
    runner: Arc<dyn TaskRunner>,
    worker_count: usize,
) {
    let worker_count = worker_count.max(1);
    let reclaim_after_seconds = runner_reclaim_seconds_from_env();

    for worker_id in 0..worker_count {
        let state = state.clone();
        let runner = runner.clone();
        tokio::spawn(async move {
            let worker_label = format!("{}-{}", runner.name(), worker_id);
            let min_idle_backoff_ms = runner_idle_backoff_min_ms_from_env();
            let mut idle_backoff_ms = min_idle_backoff_ms;
            loop {
                if let Some(reclaim_after_seconds) = reclaim_after_seconds {
                    if let Err(err) =
                        engine::reclaim_stale_running_tasks(&state, reclaim_after_seconds).await
                    {
                        eprintln!(
                            "runner reclaim error: worker_id={}, runner={}, error={}",
                            worker_id,
                            runner.name(),
                            err
                        );
                    }
                }
                match engine::run_one_task_with_runner(&state, runner.as_ref(), &worker_label).await
                {
                    Ok(true) => {
                        idle_backoff_ms = min_idle_backoff_ms;
                    }
                    Ok(false) => {
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            with_runner_backoff_jitter(idle_backoff_ms, worker_id),
                        ))
                        .await;
                        idle_backoff_ms = next_runner_idle_backoff_ms(idle_backoff_ms);
                    }
                    Err(err) => {
                        eprintln!(
                            "runner worker error: worker_id={}, runner={}, error={}",
                            worker_id,
                            runner.name(),
                            err
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            with_runner_backoff_jitter(
                                idle_backoff_ms.min(runner_error_backoff_max_ms_from_env()),
                                worker_id,
                            ),
                        ))
                        .await;
                        idle_backoff_ms = next_runner_idle_backoff_ms(idle_backoff_ms)
                            .min(runner_error_backoff_max_ms_from_env());
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_runner_idle_backoff_respects_bounds() {
        std::env::set_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MIN_MS", "200");
        std::env::set_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MAX_MS", "1000");
        std::env::set_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_JITTER_MS", "20");

        assert_eq!(next_runner_idle_backoff_ms(200), 400);
        assert_eq!(next_runner_idle_backoff_ms(400), 800);
        assert_eq!(next_runner_idle_backoff_ms(800), 1000);
        assert_eq!(next_runner_idle_backoff_ms(1000), 1000);
        assert_eq!(with_runner_backoff_jitter(200, 0), 200);
        assert!(with_runner_backoff_jitter(200, 1) >= 200);
        assert!(with_runner_backoff_jitter(200, 1) <= 220);

        std::env::remove_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MIN_MS");
        std::env::remove_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MAX_MS");
        std::env::remove_var("PERSONA_PILOT_RUNNER_IDLE_BACKOFF_JITTER_MS");
    }
}
