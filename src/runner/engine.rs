use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use tokio::{sync::oneshot, task::JoinHandle, time::Duration};
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
    runner::{
        runner_claim_retry_limit_from_env, runner_heartbeat_interval_seconds_from_env,
        RunnerFingerprintProfile, RunnerOutcomeStatus, RunnerTask, TaskRunner,
    },
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

struct ClaimedTask {
    task_id: String,
    task_kind: String,
    input_json: String,
    fingerprint_profile: Option<RunnerFingerprintProfile>,
    requested_fingerprint_profile_id: Option<String>,
    requested_fingerprint_profile_version: Option<i64>,
    attempt: i64,
    run_id: String,
    started_at: String,
}

async fn claim_next_task<R>(state: &AppState, runner: &R, worker_label: &str) -> Result<Option<ClaimedTask>>
where
    R: TaskRunner + ?Sized,
{
    for _ in 0..runner_claim_retry_limit_from_env() {
        let started_at = now_ts_string();
        let run_id = format!("run-{}", Uuid::new_v4());

        let mut tx = state.db.begin().await?;
        let claimed = sqlx::query_as::<_, (String, String, String, Option<String>, Option<i64>, Option<String>)>(
            r#"
            WITH next_task AS (
                SELECT id
                FROM tasks
                WHERE status = ?
                ORDER BY priority DESC, COALESCE(queued_at, created_at) ASC, created_at ASC
                LIMIT 1
            )
            UPDATE tasks
            SET status = ?, started_at = ?, runner_id = ?, heartbeat_at = ?
            WHERE id = (SELECT id FROM next_task)
              AND status = ?
            RETURNING id, kind, input_json, fingerprint_profile_id, fingerprint_profile_version,
                (
                    SELECT fp.profile_json
                    FROM fingerprint_profiles fp
                    WHERE fp.id = tasks.fingerprint_profile_id
                      AND fp.status = 'active'
                      AND fp.version = tasks.fingerprint_profile_version
                ) as profile_json
            "#,
        )
        .bind(TASK_STATUS_QUEUED)
        .bind(TASK_STATUS_RUNNING)
        .bind(&started_at)
        .bind(worker_label)
        .bind(&started_at)
        .bind(TASK_STATUS_QUEUED)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((task_id, task_kind, input_json, fingerprint_profile_id, fingerprint_profile_version, fingerprint_profile_json)) = claimed else {
            tx.rollback().await?;
            return Ok(None);
        };

        let requested_fingerprint_profile_id = fingerprint_profile_id.clone();
        let requested_fingerprint_profile_version = fingerprint_profile_version;

        let attempt = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM runs WHERE task_id = ?"#)
            .bind(&task_id)
            .fetch_one(&mut *tx)
            .await?
            + 1;

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
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let fingerprint_profile = match (fingerprint_profile_id, fingerprint_profile_version, fingerprint_profile_json) {
            (Some(id), Some(version), Some(profile_json)) => serde_json::from_str(&profile_json)
                .ok()
                .map(|profile_json| RunnerFingerprintProfile { id, version, profile_json }),
            _ => None,
        };

        return Ok(Some(ClaimedTask {
            task_id,
            task_kind,
            input_json,
            fingerprint_profile,
            requested_fingerprint_profile_id,
            requested_fingerprint_profile_version,
            attempt,
            run_id,
            started_at,
        }));
    }

    Ok(None)
}

pub async fn reclaim_stale_running_tasks(state: &AppState, stale_after_seconds: u64) -> Result<u64> {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let threshold = now_secs.saturating_sub(stale_after_seconds);
    let queued_at = now_ts_string();

    let task_ids = sqlx::query_scalar::<_, String>(
        r#"
        SELECT id
        FROM tasks
        WHERE status = ?
          AND started_at IS NOT NULL
          AND CAST(COALESCE(heartbeat_at, started_at) AS INTEGER) <= ?
        "#,
    )
    .bind(TASK_STATUS_RUNNING)
    .bind(threshold as i64)
    .fetch_all(&state.db)
    .await?;

    let mut reclaimed = 0_u64;
    for task_id in task_ids {
        let update = sqlx::query(
            r#"
            UPDATE tasks
            SET status = ?, queued_at = ?, started_at = NULL, finished_at = NULL, runner_id = NULL, heartbeat_at = NULL, error_message = NULL
            WHERE id = ? AND status = ?
            "#,
        )
        .bind(TASK_STATUS_QUEUED)
        .bind(&queued_at)
        .bind(&task_id)
        .bind(TASK_STATUS_RUNNING)
        .execute(&state.db)
        .await?;

        if update.rows_affected() == 0 {
            continue;
        }

        sqlx::query(
            r#"UPDATE runs SET status = ?, finished_at = ?, error_message = ? WHERE task_id = ? AND status = ?"#,
        )
        .bind(RUN_STATUS_FAILED)
        .bind(&queued_at)
        .bind("reclaimed after stale running timeout")
        .bind(&task_id)
        .bind(RUN_STATUS_RUNNING)
        .execute(&state.db)
        .await?;

        insert_log(
            state,
            &format!("log-{}", Uuid::new_v4()),
            &task_id,
            None,
            "warn",
            "stale running task reclaimed back to queued",
        )
        .await?;

        reclaimed += 1;
    }

    Ok(reclaimed)
}

fn spawn_task_heartbeat(state: AppState, task_id: String, worker_label: String) -> (oneshot::Sender<()>, JoinHandle<()>) {
    let heartbeat_interval_seconds = runner_heartbeat_interval_seconds_from_env();
    let (stop_tx, mut stop_rx) = oneshot::channel();
    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_rx => break,
                _ = tokio::time::sleep(Duration::from_secs(heartbeat_interval_seconds)) => {
                    let heartbeat_at = now_ts_string();
                    let _ = sqlx::query(
                        r#"UPDATE tasks SET heartbeat_at = ? WHERE id = ? AND status = ? AND runner_id = ?"#,
                    )
                    .bind(&heartbeat_at)
                    .bind(&task_id)
                    .bind(TASK_STATUS_RUNNING)
                    .bind(&worker_label)
                    .execute(&state.db)
                    .await;
                }
            }
        }
    });
    (stop_tx, handle)
}

pub async fn run_one_task_with_runner<R>(state: &AppState, runner: &R, worker_label: &str) -> Result<bool>
where
    R: TaskRunner + ?Sized,
{
    let Some(claimed) = claim_next_task(state, runner, worker_label).await? else {
        return Ok(false);
    };

    let task_id = claimed.task_id;
    let task_kind = claimed.task_kind;
    let input_json = claimed.input_json;
    let attempt = claimed.attempt;
    let fingerprint_profile = claimed.fingerprint_profile;
    let requested_fingerprint_profile_id = claimed.requested_fingerprint_profile_id;
    let requested_fingerprint_profile_version = claimed.requested_fingerprint_profile_version;
    let run_id = claimed.run_id;
    let _started_at = claimed.started_at;
    let (heartbeat_stop, heartbeat_handle) = spawn_task_heartbeat(state.clone(), task_id.clone(), worker_label.to_string());

    insert_log(
        state,
        &format!("log-{}", Uuid::new_v4()),
        &task_id,
        None,
        "info",
        &format!("task claimed from database queue by {}", worker_label),
    )
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

    match (&requested_fingerprint_profile_id, requested_fingerprint_profile_version, &fingerprint_profile) {
        (Some(profile_id), Some(version), Some(profile)) => {
            insert_log(
                state,
                &format!("log-{}", Uuid::new_v4()),
                &task_id,
                Some(&run_id),
                "info",
                &format!(
                    "fingerprint profile resolved for runner execution: requested_id={}, requested_version={}, resolved_id={}, resolved_version={}",
                    profile_id,
                    version,
                    profile.id,
                    profile.version
                ),
            )
            .await?;
        }
        (Some(profile_id), Some(version), None) => {
            insert_log(
                state,
                &format!("log-{}", Uuid::new_v4()),
                &task_id,
                Some(&run_id),
                "warn",
                &format!(
                    "fingerprint profile requested but not resolved at execution time; runner will continue without injected profile: requested_id={}, requested_version={}",
                    profile_id,
                    version,
                ),
            )
            .await?;
        }
        _ => {}
    }

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
            fingerprint_profile,
        })
        .await;

    let _ = heartbeat_stop.send(());
    let _ = heartbeat_handle.await;

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
        let task_update = sqlx::query(
            &format!(
                "UPDATE tasks SET status = ?, finished_at = ?, runner_id = NULL, heartbeat_at = NULL, result_json = ?, error_message = ? WHERE id = ? AND status = '{}'",
                TASK_STATUS_RUNNING,
            ),
        )
        .bind(task_status)
        .bind(&finished_at)
        .bind(&result_json)
        .bind(&error_message)
        .bind(&task_id)
        .execute(&state.db)
        .await?;

        if task_update.rows_affected() == 0 {
            insert_log(
                state,
                &format!("log-{}", Uuid::new_v4()),
                &task_id,
                Some(&run_id),
                "warn",
                &format!(
                    "{} runner finished but task terminal overwrite skipped because task was no longer running, attempt={attempt}",
                    runner.name()
                ),
            )
            .await?;
        }
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
