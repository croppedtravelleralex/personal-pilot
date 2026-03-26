use axum::{extract::{Path, State}, http::StatusCode, response::Json};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::app::state::AppState;

use super::dto::{
    CancelTaskResponse, CreateTaskRequest, HealthResponse, LogResponse, RetryTaskResponse,
    RunResponse, StatusResponse, TaskResponse, TaskStatusCounts,
};

fn now_ts_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

async fn load_counts(state: &AppState) -> Result<TaskStatusCounts, (StatusCode, String)> {
    let total = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM tasks"#)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count tasks: {err}")))?;
    let queued = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM tasks WHERE status = 'queued'"#)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count queued tasks: {err}")))?;
    let running = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM tasks WHERE status = 'running'"#)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count running tasks: {err}")))?;
    let succeeded = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM tasks WHERE status = 'succeeded'"#)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count succeeded tasks: {err}")))?;
    let failed = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM tasks WHERE status = 'failed'"#)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count failed tasks: {err}")))?;
    let timeout = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM tasks WHERE status = 'timeout'"#)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count timeout tasks: {err}")))?;
    let cancelled = sqlx::query_scalar::<_, i64>(r#"SELECT COUNT(*) FROM tasks WHERE status = 'cancelled'"#)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to count cancelled tasks: {err}")))?;

    Ok(TaskStatusCounts { total, queued, running, succeeded, failed, timeout, cancelled })
}

pub async fn health(State(state): State<AppState>) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    let counts = load_counts(&state).await?;
    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        service: "AutoOpenBrowser".to_string(),
        queue_len: state.queue.len(),
        counts,
    }))
}

pub async fn status(State(state): State<AppState>) -> Result<Json<StatusResponse>, (StatusCode, String)> {
    let counts = load_counts(&state).await?;
    let rows = sqlx::query_as::<_, (String, String, String, i32)>(
        r#"SELECT id, kind, status, priority FROM tasks ORDER BY created_at DESC LIMIT 5"#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch latest tasks: {err}")))?;

    let latest_tasks = rows.into_iter().map(|(id, kind, status, priority)| TaskResponse { id, kind, status, priority }).collect();

    Ok(Json(StatusResponse {
        service: "AutoOpenBrowser".to_string(),
        queue_len: state.queue.len(),
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
    let input_json = serde_json::json!({ "url": payload.url, "script": payload.script }).to_string();
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
    .bind("queued")
    .bind(&input_json)
    .bind(priority)
    .bind(&created_at)
    .bind(&queued_at)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to insert task: {err}")))?;

    state.queue.push(task_id.clone());

    Ok((StatusCode::CREATED, Json(TaskResponse { id: task_id, kind: payload.kind, status: "queued".to_string(), priority })))
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResponse>, (StatusCode, String)> {
    if task_id.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "task id is required".to_string()));
    }

    let row = sqlx::query_as::<_, (String, String, String, i32)>(r#"SELECT id, kind, status, priority FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch task: {err}")))?;

    match row {
        Some((id, kind, status, priority)) => Ok(Json(TaskResponse { id, kind, status, priority })),
        None => Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}"))),
    }
}

pub async fn get_task_runs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Vec<RunResponse>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (String, String, String, i32, String, Option<String>, Option<String>, Option<String>)>(
        r#"SELECT id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message FROM runs WHERE task_id = ? ORDER BY attempt DESC"#,
    )
    .bind(&task_id)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch runs: {err}")))?;

    Ok(Json(rows.into_iter().map(|(id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message)| RunResponse {
        id, task_id, status, attempt, runner_kind, started_at, finished_at, error_message,
    }).collect()))
}

pub async fn get_task_logs(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<Vec<LogResponse>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, String, String)>(
        r#"SELECT id, task_id, run_id, level, message, created_at FROM logs WHERE task_id = ? ORDER BY created_at DESC"#,
    )
    .bind(&task_id)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch logs: {err}")))?;

    Ok(Json(rows.into_iter().map(|(id, task_id, run_id, level, message, created_at)| LogResponse {
        id, task_id, run_id, level, message, created_at,
    }).collect()))
}

pub async fn retry_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<RetryTaskResponse>, (StatusCode, String)> {
    let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to read task status: {err}")))?;

    let Some(status) = current_status else {
        return Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}")));
    };

    if status != "failed" && status != "timeout" {
        return Err((StatusCode::BAD_REQUEST, format!("task status does not allow retry: {status}")));
    }

    let queued_at = now_ts_string();
    sqlx::query(
        r#"UPDATE tasks SET status = ?, queued_at = ?, started_at = NULL, finished_at = NULL, result_json = NULL, error_message = NULL WHERE id = ?"#,
    )
    .bind("queued")
    .bind(&queued_at)
    .bind(&task_id)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to retry task: {err}")))?;

    state.queue.push(task_id.clone());

    Ok(Json(RetryTaskResponse { id: task_id, status: "queued".to_string(), message: "task re-queued for retry".to_string() }))
}

pub async fn cancel_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
) -> Result<Json<CancelTaskResponse>, (StatusCode, String)> {
    let current_status = sqlx::query_scalar::<_, String>(r#"SELECT status FROM tasks WHERE id = ?"#)
        .bind(&task_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to read task status: {err}")))?;

    let Some(status) = current_status else {
        return Err((StatusCode::NOT_FOUND, format!("task not found: {task_id}")));
    };

    if status != "queued" {
        return Err((StatusCode::BAD_REQUEST, format!("only queued tasks can be cancelled now, current status: {status}")));
    }

    let removed = state.queue.remove(&task_id);
    if !removed {
        return Err((StatusCode::CONFLICT, "task not found in memory queue; it may already be running".to_string()));
    }

    let finished_at = now_ts_string();
    sqlx::query(r#"UPDATE tasks SET status = ?, finished_at = ?, error_message = ? WHERE id = ?"#)
        .bind("cancelled")
        .bind(&finished_at)
        .bind("cancelled before execution")
        .bind(&task_id)
        .execute(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to cancel task: {err}")))?;

    Ok(Json(CancelTaskResponse { id: task_id, status: "cancelled".to_string(), message: "task removed from queue and cancelled".to_string() }))
}
