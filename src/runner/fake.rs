use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::{
    app::state::AppState,
    runner::{RunnerExecutionResult, RunnerOutcomeStatus, RunnerTask, TaskRunner},
};

pub struct FakeRunner;

fn now_ts_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

async fn insert_log(
    state: &AppState,
    log_id: &str,
    task_id: &str,
    run_id: Option<&str>,
    level: &str,
    message: &str,
) -> Result<()> {
    let created_at = now_ts_string();
    sqlx::query(
        r#"
        INSERT INTO logs (id, task_id, run_id, level, message, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(log_id)
    .bind(task_id)
    .bind(run_id)
    .bind(level)
    .bind(message)
    .bind(created_at)
    .execute(&state.db)
    .await?;
    Ok(())
}

#[async_trait]
impl TaskRunner for FakeRunner {
    fn name(&self) -> &'static str {
        "fake"
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult {
        let _ = task.timeout_seconds;
        sleep(Duration::from_millis(300)).await;

        match task.kind.as_str() {
            "fail" => RunnerExecutionResult {
                status: RunnerOutcomeStatus::Failed,
                result_json: None,
                error_message: Some("simulated failure by fake runner".to_string()),
            },
            "timeout" => RunnerExecutionResult {
                status: RunnerOutcomeStatus::TimedOut,
                result_json: None,
                error_message: Some("simulated timeout by fake runner".to_string()),
            },
            _ => RunnerExecutionResult {
                status: RunnerOutcomeStatus::Succeeded,
                result_json: Some(json!({
                    "runner": self.name(),
                    "message": "task completed by fake runner",
                    "task_id": task.task_id,
                    "attempt": task.attempt,
                    "payload": task.payload,
                })),
                error_message: None,
            },
        }
    }
}

pub async fn run_one_task_with_runner<R>(state: &AppState, runner: &R) -> Result<bool>
where
    R: TaskRunner + ?Sized,
{
    let Some(task_id) = state.queue.pop() else {
        return Ok(false);
    };

    let run_id = format!("run-{}", Uuid::new_v4());
    let started_at = now_ts_string();

    let task_kind = sqlx::query_scalar::<_, String>(
        r#"SELECT kind FROM tasks WHERE id = ?"#,
    )
    .bind(&task_id)
    .fetch_one(&state.db)
    .await?;

    let attempt = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM runs WHERE task_id = ?"#,
    )
    .bind(&task_id)
    .fetch_one(&state.db)
    .await? + 1;

    insert_log(
        state,
        &format!("log-{}", Uuid::new_v4()),
        &task_id,
        None,
        "info",
        "task popped from in-memory queue",
    )
    .await?;

    sqlx::query(
        r#"
        INSERT INTO runs (id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)
        VALUES (?, ?, ?, ?, ?, ?, NULL, NULL)
        "#,
    )
    .bind(&run_id)
    .bind(&task_id)
    .bind("running")
    .bind(attempt)
    .bind(runner.name())
    .bind(&started_at)
    .execute(&state.db)
    .await?;

    insert_log(
        state,
        &format!("log-{}", Uuid::new_v4()),
        &task_id,
        Some(&run_id),
        "info",
        &format!("{} runner started task execution, attempt={attempt}", runner.name()),
    )
    .await?;

    sqlx::query(
        r#"UPDATE tasks SET status = ?, started_at = ? WHERE id = ?"#,
    )
    .bind("running")
    .bind(&started_at)
    .bind(&task_id)
    .execute(&state.db)
    .await?;

    let execution = runner
        .execute(RunnerTask {
            task_id: task_id.clone(),
            attempt,
            kind: task_kind,
            payload: json!({}),
            timeout_seconds: None,
        })
        .await;

    let finished_at = now_ts_string();

    let (task_status, run_status, log_level, log_message) = match execution.status {
        RunnerOutcomeStatus::Succeeded => (
            "succeeded",
            "succeeded",
            "info",
            format!("{} runner finished successfully, attempt={attempt}", runner.name()),
        ),
        RunnerOutcomeStatus::Failed => (
            "failed",
            "failed",
            "error",
            format!("{} runner finished with failure, attempt={attempt}", runner.name()),
        ),
        RunnerOutcomeStatus::TimedOut => (
            "timeout",
            "timeout",
            "warn",
            format!("{} runner finished with timeout, attempt={attempt}", runner.name()),
        ),
    };

    let result_json = execution.result_json.map(|value| value.to_string());
    let error_message = execution.error_message;

    sqlx::query(
        r#"UPDATE runs SET status = ?, finished_at = ?, error_message = ? WHERE id = ?"#,
    )
    .bind(run_status)
    .bind(&finished_at)
    .bind(&error_message)
    .bind(&run_id)
    .execute(&state.db)
    .await?;

    sqlx::query(
        r#"UPDATE tasks SET status = ?, finished_at = ?, result_json = ?, error_message = ? WHERE id = ?"#,
    )
    .bind(task_status)
    .bind(&finished_at)
    .bind(&result_json)
    .bind(&error_message)
    .bind(&task_id)
    .execute(&state.db)
    .await?;

    insert_log(
        state,
        &format!("log-{}", Uuid::new_v4()),
        &task_id,
        Some(&run_id),
        log_level,
        &log_message,
    )
    .await?;

    Ok(true)
}
