use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::{
    app::state::AppState,
    domain::{
        run::{RUN_STATUS_CANCELLED, RUN_STATUS_RUNNING},
        task::{
            TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING,
            TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
        },
    },
};

use super::dto::{
    CancelTaskResponse, CreateTaskRequest, HealthResponse, LogResponse, PaginationQuery,
    RetryTaskResponse, RunResponse, StatusResponse, TaskResponse, TaskStatusCounts,
};

fn now_ts_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

fn sanitize_limit(limit: Option<i64>, default_value: i64, max_value: i64) -> i64 {
    match limit {
        Some(value) if value > 0 => value.min(max_value),
        _ => default_value,
    }
}

fn sanitize_offset(offset: Option<i64>) -> i64 {
    match offset {
        Some(value) if value > 0 => value,
        _ => 0,
    }
}

async fn insert_task_log(
    state: &AppState,
    task_id: &str,
    run_id: Option<&str>,
    level: &str,
    message: &str,
) -> Result<(), (StatusCode, String)> {
    let log_id = format!("log-{}", Uuid::new_v4());
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
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to insert cancel log: {err}"),
        )
    })?;
    Ok(())
}

async fn load_counts(state: &AppState) -> Result<TaskStatusCounts, (StatusCode, String)> {
    let total = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM tasks"#)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count tasks: {err}")))?;
    let queued = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM tasks WHERE status = '{}'", TASK_STATUS_QUEUED))
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count queued tasks: {err}")))?;
    let running = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM tasks WHERE status = '{}'", TASK_STATUS_RUNNING))
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count running tasks: {err}")))?;
    let succeeded = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM tasks WHERE status = '{}'", TASK_STATUS_SUCCEEDED))
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count succeeded tasks: {err}")))?;
    let failed = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM tasks WHERE status = '{}'", TASK_STATUS_FAILED))
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count failed tasks: {err}")))?;
    let timed_out = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM tasks WHERE status = '{}'", TASK_STATUS_TIMED_OUT))
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count timed_out tasks: {err}")))?;
    let cancelled = sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM tasks WHERE status = '{}'", TASK_STATUS_CANCELLED))
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count cancelled tasks: {err}")))?;

    Ok(TaskStatusCounts {
        total,
        queued,
        running,
        succeeded,
        failed,
        timed_out,
        cancelled,
    })
}

pub async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    let counts = load_counts(&state).await?;
    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        service: "AutoOpenBrowser".to_string(),
        queue_len: counts.queued as usize,
        counts,
    }))
}

pub async fn status(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    let counts = load_counts(&state).await?;
    let limit = sanitize_limit(query.limit, 5, 100);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, String, i32)>(
        r#"SELECT id, kind, status, priority FROM tasks ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch latest tasks: {err}"),
        )
    })?;

    let latest_tasks = rows
        .into_iter()
        .map(|(id, kind, status, priority)| TaskResponse {
            id,
            kind,
            status,
            priority,
        })
        .collect();

    Ok(Json(StatusResponse {
        service: "AutoOpenBrowser".to_string(),
        queue_len: counts.queued as usize,
        counts,
        latest_tasks,
    }))
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), (StatusCode, String)> {
    if payload.kind.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "kind is required".to_string()));
    }

    let task_id = format!("task-{}", Uuid::new_v4());
    let priority = payload.priority.unwrap_or(0);
    let input_json = serde_json::json!({
        "url": payload.url,
        "script": payload.script,
        "timeout_seconds": payload.timeout_seconds
    })
    .to_string();
    let created_at = now_ts_string();
    let queued_at = now_ts_string();

    sqlx::query(
        r#"
        INSERT INTO tasks (
            id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
            priority, created_at, queued_at, started_at, finished_at, result_json, error_message
        ) VALUES (?, ?, ?, ?, NULL, NULL, ?, ?, ?, NULL, NULL, NULL, NULL)
        "#,
    )
    .bind(&task_id)
    .bind(&payload.kind)
    .bind(TASK_STATUS_QUEUED)
    .bind(&input_json)
    .bind(priority)
    .bind(&created_at)
    .bind(&queued_at)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to insert task: {err}"),
        )
    })?;


    Ok((
        StatusCode::CREATED,
        Json(TaskResponse {
            id: task_id,
            kind: payload.kind,
            status: TASK_STATUS_QUEUED.to_string(),
            priority,
        }),
    ))
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResponse>, (StatusCode, String)> {
    if task_id.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "task id is required".to_string()));
    }

    let row = sqlx::query_as::<_, (String, String, String, i32)>(
        r#"SELECT id, kind, status, priority FROM tasks WHERE id = ?"#,
    )
    .bind(&task_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch task: {err}"),
        )
    })?;

    match row {
        Some((id, kind, status, priority)) => Ok(Json(TaskResponse {
            id,
            kind,
            status,
            priority,
        })),
        None => Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}"))),
    }
}

pub async fn get_task_runs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<RunResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, String, i32, String, Option<String>, Option<String>, Option<String>)>(
        r#"SELECT id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message FROM runs WHERE task_id = ? ORDER BY attempt DESC LIMIT ? OFFSET ?"#,
    )
    .bind(&task_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch runs: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)| RunResponse {
                    id,
                    task_id,
                    status,
                    attempt,
                    runner_kind,
                    started_at,
                    finished_at,
                    error_message,
                },
            )
            .collect(),
    ))
}

pub async fn get_task_logs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<LogResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 50, 500);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        r#"SELECT id, task_id, run_id, level, message, created_at FROM logs WHERE task_id = ? ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#,
    )
    .bind(&task_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch logs: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, task_id, run_id, level, message, created_at)| LogResponse {
                id,
                task_id,
                run_id,
                level,
                message,
                created_at,
            })
            .collect(),
    ))
}

pub async fn retry_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<RetryTaskResponse>, (StatusCode, String)> {
    let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read task status before retry: {err}"),
            )
        })?;

    let Some(status) = current_status else {
        return Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}")));
    };

    if status == TASK_STATUS_QUEUED {
        return Ok(Json(RetryTaskResponse {
            id: task_id,
            status: TASK_STATUS_QUEUED.to_string(),
            message: "task already queued; retry treated as idempotent".to_string(),
        }));
    }

    if status != TASK_STATUS_FAILED && status != TASK_STATUS_TIMED_OUT {
        return Err((
            StatusCode::CONFLICT,
            format!("task status does not allow retry now: {status}"),
        ));
    }

    let queued_at = now_ts_string();
    let retry_sql = format!(
        "UPDATE tasks SET status = ?, queued_at = ?, started_at = NULL, finished_at = NULL, runner_id = NULL, heartbeat_at = NULL, result_json = NULL, error_message = NULL WHERE id = ? AND status IN ('{}', '{}')",
        TASK_STATUS_FAILED, TASK_STATUS_TIMED_OUT,
    );
    let result = sqlx::query(&retry_sql)
        .bind(TASK_STATUS_QUEUED)
        .bind(&queued_at)
        .bind(&task_id)
        .execute(&state.db)
        .await;

    let result = match result {
        Ok(result) => result,
        Err(err) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to retry task: {err}"),
            ));
        }
    };

    if result.rows_affected() == 0 {

        let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
            .bind(&task_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to read task status after retry conflict: {err}"),
                )
            })?;

        let Some(status) = current_status else {
            return Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}")));
        };

        if status == TASK_STATUS_QUEUED {
            return Ok(Json(RetryTaskResponse {
                id: task_id,
                status: TASK_STATUS_QUEUED.to_string(),
                message: "task already queued after retry race; treated as idempotent".to_string(),
            }));
        }

        return Err((
            StatusCode::CONFLICT,
            format!("task status does not allow retry now: {status}"),
        ));
    }

    let message = "task re-queued for retry".to_string();

    Ok(Json(RetryTaskResponse {
        id: task_id,
        status: TASK_STATUS_QUEUED.to_string(),
        message,
    }))
}

pub async fn cancel_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<CancelTaskResponse>, (StatusCode, String)> {
    let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to read task status: {err}"),
            )
        })?;

    let Some(status) = current_status else {
        return Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}")));
    };

    if status == TASK_STATUS_QUEUED {
        let finished_at = now_ts_string();
        sqlx::query(r#"UPDATE tasks SET status = ?, finished_at = ?, runner_id = NULL, heartbeat_at = NULL, error_message = ? WHERE id = ?"#)
            .bind(TASK_STATUS_CANCELLED)
            .bind(&finished_at)
            .bind("task cancelled while queued")
            .bind(&task_id)
            .execute(&state.db)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to cancel task: {err}"),
                )
            })?;

        insert_task_log(
            &state,
            &task_id,
            None,
            "warn",
            "task cancelled while queued",
        )
        .await?;

        return Ok(Json(CancelTaskResponse {
            id: task_id,
            status: TASK_STATUS_CANCELLED.to_string(),
            message: "task cancelled while queued".to_string(),
        }));
    }

    if status == TASK_STATUS_RUNNING {
        let cancel = state.runner.cancel_running(&task_id).await;
        if !cancel.accepted {
            return Err((StatusCode::CONFLICT, cancel.message));
        }

        let finished_at = now_ts_string();
        sqlx::query(r#"UPDATE tasks SET status = ?, finished_at = ?, runner_id = NULL, heartbeat_at = NULL, error_message = ? WHERE id = ?"#)
            .bind(TASK_STATUS_CANCELLED)
            .bind(&finished_at)
            .bind("task cancelled while running")
            .bind(&task_id)
            .execute(&state.db)
            .await
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to mark running task as cancelled: {err}"),
                )
            })?;

        let running_run_id = sqlx::query_scalar::<_, String>(
            &format!(
                "SELECT id FROM runs WHERE task_id = ? AND status = '{}' ORDER BY attempt DESC LIMIT 1",
                RUN_STATUS_RUNNING,
            ),
        )
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to fetch running run for cancel: {err}"),
            )
        })?;

        if let Some(run_id) = running_run_id.as_deref() {
            sqlx::query(r#"UPDATE runs SET status = ?, finished_at = ?, error_message = ? WHERE id = ?"#)
                .bind(RUN_STATUS_CANCELLED)
                .bind(&finished_at)
                .bind("task cancelled while running")
                .bind(run_id)
                .execute(&state.db)
                .await
                .map_err(|err| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("failed to mark latest run as cancelled: {err}"),
                    )
                })?;

            insert_task_log(
                &state,
                &task_id,
                Some(run_id),
                "warn",
                &format!("task cancelled while running; {}", cancel.message),
            )
            .await?;
        } else {
            insert_task_log(
                &state,
                &task_id,
                None,
                "warn",
                &format!("task cancelled while running; {}", cancel.message),
            )
            .await?;
        }

        return Ok(Json(CancelTaskResponse {
            id: task_id,
            status: TASK_STATUS_CANCELLED.to_string(),
            message: cancel.message,
        }));
    }

    Err((
        StatusCode::BAD_REQUEST,
        format!("task status does not allow cancel: {status}"),
    ))
}
