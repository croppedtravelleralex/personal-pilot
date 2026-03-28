use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    app::state::AppState,
    domain::{
        run::{RUN_STATUS_RUNNING, RUN_STATUS_SUCCEEDED, RUN_STATUS_FAILED, RUN_STATUS_TIMED_OUT},
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING,
            TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
    runner::{RunnerOutcomeStatus, RunnerTask, TaskRunner},
};

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

    let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await?;

    let Some(current_status) = current_status else {
        return Ok(false);
    };

    if current_status != TASK_STATUS_QUEUED {
        insert_log(
            state,
            &format!("log-{}", Uuid::new_v4()),
            &task_id,
            None,
            "warn",
            &format!(
                "task popped from queue but skipped because current status is {}",
                current_status
            ),
        )
        .await?;
        return Ok(false);
    }

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
    .bind(RUN_STATUS_RUNNING)
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

    sqlx::query(&format!("UPDATE tasks SET status = ?, started_at = ? WHERE id = ? AND status = '{}'", TASK_STATUS_QUEUED))
        .bind(TASK_STATUS_RUNNING)
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
            TASK_STATUS_SUCCEEDED,
            RUN_STATUS_SUCCEEDED,
            "info",
            format!("{} runner finished successfully, attempt={attempt}", runner.name()),
        ),
        RunnerOutcomeStatus::Failed => (
            TASK_STATUS_FAILED,
            RUN_STATUS_FAILED,
            "error",
            format!("{} runner finished with failure, attempt={attempt}", runner.name()),
        ),
        RunnerOutcomeStatus::TimedOut => (
            TASK_STATUS_TIMED_OUT,
            RUN_STATUS_TIMED_OUT,
            "warn",
            format!("{} runner finished with timeout, attempt={attempt}", runner.name()),
        ),
    };

    let result_json = execution.result_json.map(|value| value.to_string());
    let error_message = execution.error_message;

    let run_update = sqlx::query(
        &format!(
            "UPDATE runs SET status = ?, finished_at = ?, error_message = ? WHERE id = ? AND status = '{}'",
            RUN_STATUS_RUNNING,
        ),
    )
    .bind(run_status)
    .bind(&finished_at)
    .bind(&error_message)
    .bind(&run_id)
    .execute(&state.db)
    .await?;

    if run_update.rows_affected() == 0 {
        insert_log(
            state,
            &format!("log-{}", Uuid::new_v4()),
            &task_id,
            Some(&run_id),
            "warn",
            &format!(
                "{} runner finished but run terminal overwrite skipped because run was no longer running, attempt={attempt}",
                runner.name()
            ),
        )
        .await?;
    }

    let current_task_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_one(&state.db)
        .await?;

    if current_task_status != TASK_STATUS_CANCELLED {
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
