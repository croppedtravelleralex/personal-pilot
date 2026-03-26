use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use crate::app::state::AppState;

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

pub async fn run_one_fake_task(state: &AppState) -> Result<bool> {
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
    .bind("fake")
    .bind(&started_at)
    .execute(&state.db)
    .await?;

    insert_log(
        state,
        &format!("log-{}", Uuid::new_v4()),
        &task_id,
        Some(&run_id),
        "info",
        &format!("fake runner started task execution, attempt={attempt}"),
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

    sleep(Duration::from_millis(300)).await;

    let finished_at = now_ts_string();

    let (task_status, run_status, error_message, result_json, log_level, log_message) = match task_kind.as_str() {
        "fail" => (
            "failed",
            "failed",
            Some("simulated failure by fake runner".to_string()),
            None,
            "error",
            format!("fake runner finished with simulated failure, attempt={attempt}"),
        ),
        "timeout" => (
            "timeout",
            "timeout",
            Some("simulated timeout by fake runner".to_string()),
            None,
            "warn",
            format!("fake runner finished with simulated timeout, attempt={attempt}"),
        ),
        _ => (
            "succeeded",
            "succeeded",
            None,
            Some(
                serde_json::json!({
                    "runner": "fake",
                    "message": "task completed by fake runner",
                    "run_id": run_id,
                    "attempt": attempt,
                })
                .to_string(),
            ),
            "info",
            format!("fake runner finished successfully, attempt={attempt}"),
        ),
    };

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

pub async fn spawn_fake_runner_loop(state: AppState) {
    tokio::spawn(async move {
        loop {
            let _ = run_one_fake_task(&state).await;
            sleep(Duration::from_millis(500)).await;
        }
    });
}
