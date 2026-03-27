use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{app::state::AppState, runner::{RunnerOutcomeStatus, RunnerTask, TaskRunner}};

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

pub async fn run_one_task_with_runner<R>(state: &AppState, runner: &R) -> Result<bool>
where
    R: TaskRunner + ?Sized,
{
    let Some(task_id) = state.queue.pop() else {
        return Ok(false);
    };

    let run_id = format!("run-{}", Uuid::new_v4());
    let started_at = now_ts_string();

    let (task_kind, input_json) = sqlx::query_as::<_, (String, String)>(
        r#"SELECT kind, input_json FROM tasks WHERE id = ?"#,
    )
    .bind(&task_id)
    .fetch_one(&state.db)
    .await?;

    let attempt = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM runs WHERE task_id = ?"#)
        .bind(&task_id)
        .fetch_one(&state.db)
        .await?
        + 1;

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

    sqlx::query(r#"UPDATE tasks SET status = ?, started_at = ? WHERE id = ?"#)
        .bind("running")
        .bind(&started_at)
        .bind(&task_id)
        .execute(&state.db)
        .await?;

    let payload: Value = serde_json::from_str(&input_json).unwrap_or_else(|_| {
        json!({
            "raw_input_json": input_json,
        })
    });
    let timeout_seconds = payload
        .get("timeout_seconds")
        .and_then(|value| value.as_i64())
        .filter(|value| *value > 0);

    let execution = runner
        .execute(RunnerTask {
            task_id: task_id.clone(),
            attempt,
            kind: task_kind,
            payload,
            timeout_seconds,
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

    sqlx::query(r#"UPDATE runs SET status = ?, finished_at = ?, error_message = ? WHERE id = ?"#)
        .bind(run_status)
        .bind(&finished_at)
        .bind(&error_message)
        .bind(&run_id)
        .execute(&state.db)
        .await?;

    let current_task_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_one(&state.db)
        .await?;

    if current_task_status != "cancelled" {
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
    } else {
        insert_log(
            state,
            &format!("log-{}", Uuid::new_v4()),
            &task_id,
            Some(&run_id),
            "warn",
            &format!(
                "{} runner finished after cancel; terminal task overwrite skipped, attempt={attempt}",
                runner.name()
            ),
        )
        .await?;
    }

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
