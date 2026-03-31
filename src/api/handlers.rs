use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use std::{net::SocketAddr, time::{Instant, SystemTime, UNIX_EPOCH}};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::{
    network_identity::validator::validate_fingerprint_profile,
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
    CancelTaskResponse, CreateFingerprintProfileRequest, CreateProxyRequest, CreateTaskRequest,
    FingerprintMetricsResponse, FingerprintProfileResponse, HealthResponse, LogResponse,
    PaginationQuery, ProxyMetricsResponse, ProxyResponse, ProxySmokeResponse, ProxyVerifyBatchProviderSummary, ProxyVerifyBatchRequest, ProxyVerifyBatchResponse, ProxyVerifyResponse, RetryTaskResponse, VerifyBatchListQuery, VerifyBatchResponse, VerifyMetricsResponse,
    RunResponse, StatusResponse, TaskResponse, TaskStatusCounts, WorkerStatusResponse,
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

#[derive(sqlx::FromRow)]
struct ProxyRow {
    id: String,
    scheme: String,
    host: String,
    port: i64,
    username: Option<String>,
    region: Option<String>,
    country: Option<String>,
    provider: Option<String>,
    status: String,
    score: f64,
    success_count: i64,
    failure_count: i64,
    last_checked_at: Option<String>,
    last_used_at: Option<String>,
    cooldown_until: Option<String>,
    last_smoke_status: Option<String>,
    last_smoke_protocol_ok: Option<i64>,
    last_smoke_upstream_ok: Option<i64>,
    last_exit_ip: Option<String>,
    last_anonymity_level: Option<String>,
    last_smoke_at: Option<String>,
    last_verify_status: Option<String>,
    last_verify_geo_match_ok: Option<i64>,
    last_exit_country: Option<String>,
    last_exit_region: Option<String>,
    last_verify_at: Option<String>,
    created_at: String,
    updated_at: String,
}


pub async fn run_proxy_verify_probe(
    state: &AppState,
    proxy_id: &str,
) -> Result<ProxyVerifyResponse, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, i64, Option<String>)>(r#"SELECT host, port, country FROM proxies WHERE id = ?"#)
        .bind(proxy_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load proxy for verify: {err}")))?;

    let Some((host, port, expected_country)) = row else {
        return Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}")));
    };

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|err| (StatusCode::BAD_REQUEST, format!("proxy address is invalid for verify: {err}")))?;

    let started = Instant::now();
    let mut stream = tokio::time::timeout(std::time::Duration::from_secs(5), tokio::net::TcpStream::connect(addr))
        .await
        .ok()
        .and_then(|result| result.ok());
    let reachable = stream.is_some();
    let mut protocol_ok = false;
    let mut upstream_ok = false;
    let mut exit_ip: Option<String> = None;
    let mut exit_country: Option<String> = None;
    let mut exit_region: Option<String> = None;
    let mut geo_match_ok: Option<bool> = None;
    let mut anonymity_level: Option<String> = None;
    let mut verify_message = if reachable {
        "tcp connect succeeded but verify probe did not complete".to_string()
    } else {
        "proxy verify tcp connect failed".to_string()
    };

    if let Some(stream_ref) = stream.as_mut() {
        let probe = b"CONNECT verify.example:443 HTTP/1.1
Host: verify.example:443

";
        if tokio::time::timeout(std::time::Duration::from_secs(5), stream_ref.write_all(probe)).await.ok().is_some() {
            let mut buf = [0_u8; 1024];
            if let Ok(Ok(n)) = tokio::time::timeout(std::time::Duration::from_secs(5), stream_ref.read(&mut buf)).await {
                if n > 0 {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let text_lower = text.to_ascii_lowercase();
                    if text_lower.contains("http/1.1") || text_lower.contains("http/1.0") {
                        protocol_ok = true;
                        let has_via = text_lower.contains("via:");
                        let has_forwarded = text_lower.contains("forwarded:") || text_lower.contains("x-forwarded-for:");
                        anonymity_level = Some(if has_forwarded { "transparent".to_string() } else if has_via { "anonymous".to_string() } else { "elite".to_string() });
                        exit_ip = parse_probe_field(&text, "ip");
                        exit_country = parse_probe_field(&text, "country");
                        exit_region = parse_probe_field(&text, "region");
                        upstream_ok = exit_ip.is_some() || exit_country.is_some() || exit_region.is_some();
                        geo_match_ok = expected_country.as_ref().map(|expected| exit_country.as_ref().map(|actual| actual.eq_ignore_ascii_case(expected)).unwrap_or(false));
                        verify_message = format!("proxy verify completed ip={:?} country={:?} region={:?}", exit_ip, exit_country, exit_region);
                    } else {
                        verify_message = format!("proxy verify got non-http response: {text_lower}");
                    }
                }
            }
        }
    }

    let latency_ms = Some(started.elapsed().as_millis());
    let status = if reachable && protocol_ok && upstream_ok { "ok" } else { "failed" };
    let now = now_ts_string();
    sqlx::query(r#"UPDATE proxies SET last_checked_at = ?, last_verify_status = ?, last_verify_geo_match_ok = ?, last_exit_ip = ?, last_exit_country = ?, last_exit_region = ?, last_anonymity_level = ?, last_verify_at = ?, updated_at = ? WHERE id = ?"#)
        .bind(&now)
        .bind(status)
        .bind(geo_match_ok.map(|v| if v { 1_i64 } else { 0_i64 }))
        .bind(&exit_ip)
        .bind(&exit_country)
        .bind(&exit_region)
        .bind(&anonymity_level)
        .bind(&now)
        .bind(&now)
        .bind(proxy_id)
        .execute(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to persist proxy verify result: {err}")))?;

    Ok(ProxyVerifyResponse {
        id: proxy_id.to_string(),
        reachable,
        protocol_ok,
        upstream_ok,
        exit_ip,
        exit_country,
        exit_region,
        geo_match_ok,
        anonymity_level,
        latency_ms,
        status: status.to_string(),
        message: verify_message,
    })
}

fn parse_probe_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=");
    let idx = text.find(&needle)?;
    let value = text[idx + needle.len()..].lines().next()?.trim();
    if value.is_empty() { None } else { Some(value.to_string()) }
}

fn map_proxy_row(row: ProxyRow) -> ProxyResponse {
    ProxyResponse {
        id: row.id,
        scheme: row.scheme,
        host: row.host,
        port: row.port,
        username: row.username,
        region: row.region,
        country: row.country,
        provider: row.provider,
        status: row.status,
        score: row.score,
        success_count: row.success_count,
        failure_count: row.failure_count,
        last_checked_at: row.last_checked_at,
        last_used_at: row.last_used_at,
        cooldown_until: row.cooldown_until,
        last_smoke_status: row.last_smoke_status,
        last_smoke_protocol_ok: row.last_smoke_protocol_ok.map(|v| v != 0),
        last_smoke_upstream_ok: row.last_smoke_upstream_ok.map(|v| v != 0),
        last_exit_ip: row.last_exit_ip,
        last_anonymity_level: row.last_anonymity_level,
        last_smoke_at: row.last_smoke_at,
        last_verify_status: row.last_verify_status,
        last_verify_geo_match_ok: row.last_verify_geo_match_ok.map(|v| v != 0),
        last_exit_country: row.last_exit_country,
        last_exit_region: row.last_exit_region,
        last_verify_at: row.last_verify_at,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn fingerprint_resolution_status(
    fingerprint_profile_id: Option<&str>,
    fingerprint_profile_version: Option<i64>,
    result_json: Option<&str>,
) -> Option<String> {
    let profile_id = fingerprint_profile_id?;
    let profile_version = fingerprint_profile_version?;

    let parsed = result_json
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());

    if parsed
        .as_ref()
        .and_then(|json| json.get("fingerprint_profile"))
        .and_then(|value| value.get("id"))
        .and_then(|value| value.as_str())
        == Some(profile_id)
        && parsed
            .as_ref()
            .and_then(|json| json.get("fingerprint_profile"))
            .and_then(|value| value.get("version"))
            .and_then(|value| value.as_i64())
            == Some(profile_version)
    {
        return Some("resolved".to_string());
    }

    if parsed
        .as_ref()
        .and_then(|json| json.get("fingerprint_profile"))
        .map(|value| value.is_null())
        == Some(true)
    {
        return Some("downgraded".to_string());
    }

    Some("pending".to_string())
}

fn proxy_resolution_status(result_json: Option<&str>) -> Option<String> {
    let parsed = result_json.and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())?;
    parsed.get("proxy")
        .and_then(|value| value.get("resolution_status"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| parsed.get("payload")
            .and_then(|value| value.get("network_policy_json"))
            .and_then(|value| value.get("proxy_resolution_status"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()))
}

fn proxy_identity(result_json: Option<&str>) -> (Option<String>, Option<String>, Option<String>) {
    let parsed = result_json.and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok());
    let proxy = parsed.as_ref().and_then(|json| json.get("proxy"));
    (
        proxy.and_then(|value| value.get("id")).and_then(|value| value.as_str()).map(|v| v.to_string()),
        proxy.and_then(|value| value.get("provider")).and_then(|value| value.as_str()).map(|v| v.to_string()),
        proxy.and_then(|value| value.get("region")).and_then(|value| value.as_str()).map(|v| v.to_string()),
    )
}

fn build_proxy_metrics(tasks: &[TaskResponse]) -> ProxyMetricsResponse {
    let mut metrics = ProxyMetricsResponse { direct: 0, resolved: 0, resolved_sticky: 0, unresolved: 0, none: 0 };
    for task in tasks {
        match task.proxy_resolution_status.as_deref() {
            Some("direct") => metrics.direct += 1,
            Some("resolved") => metrics.resolved += 1,
            Some("resolved_sticky") => metrics.resolved_sticky += 1,
            Some("unresolved") => metrics.unresolved += 1,
            _ => metrics.none += 1,
        }
    }
    metrics
}

fn build_fingerprint_metrics(tasks: &[TaskResponse]) -> FingerprintMetricsResponse {
    let mut metrics = FingerprintMetricsResponse {
        pending: 0,
        resolved: 0,
        downgraded: 0,
        none: 0,
    };

    for task in tasks {
        match task.fingerprint_resolution_status.as_deref() {
            Some("pending") => metrics.pending += 1,
            Some("resolved") => metrics.resolved += 1,
            Some("downgraded") => metrics.downgraded += 1,
            _ => metrics.none += 1,
        }
    }

    metrics
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
    let (total, queued, running, succeeded, failed, timed_out, cancelled): (i64, i64, i64, i64, i64, i64, i64) =
        sqlx::query_as(
            r#"SELECT
                   COUNT(*) AS total,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS queued,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS running,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS succeeded,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS failed,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS timed_out,
                   COALESCE(SUM(CASE WHEN status = ? THEN 1 ELSE 0 END), 0) AS cancelled
               FROM tasks"#,
        )
        .bind(TASK_STATUS_QUEUED)
        .bind(TASK_STATUS_RUNNING)
        .bind(TASK_STATUS_SUCCEEDED)
        .bind(TASK_STATUS_FAILED)
        .bind(TASK_STATUS_TIMED_OUT)
        .bind(TASK_STATUS_CANCELLED)
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate task counts: {err}")))?;

    Ok(TaskStatusCounts { total, queued, running, succeeded, failed, timed_out, cancelled })
}



async fn map_verify_batch_row(
    state: &AppState,
    id: String,
    status: String,
    requested_count: i64,
    accepted_count: i64,
    skipped_count: i64,
    stale_after_seconds: i64,
    task_timeout_seconds: i64,
    provider_summary_json: Option<String>,
    filters_json: Option<String>,
    created_at: String,
    updated_at: String,
) -> Result<VerifyBatchResponse, (StatusCode, String)> {
    let (queued_count, running_count, succeeded_count, failed_count): (i64, i64, i64, i64) = sqlx::query_as(
        r#"SELECT
               COALESCE(SUM(CASE WHEN status = 'queued' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN status = 'running' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN status = 'succeeded' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN status IN ('failed', 'timed_out', 'cancelled') THEN 1 ELSE 0 END), 0)
           FROM tasks
           WHERE kind = 'verify_proxy' AND json_extract(input_json, '$.verify_batch_id') = ?"#,
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate verify batch task counts: {err}")))?;

    let derived_status = if accepted_count == 0 {
        status.clone()
    } else if queued_count > 0 || running_count > 0 {
        "running".to_string()
    } else if succeeded_count + failed_count >= accepted_count {
        "completed".to_string()
    } else {
        status.clone()
    };

    Ok(VerifyBatchResponse {
        id,
        status: derived_status,
        requested_count,
        accepted_count,
        skipped_count,
        queued_count,
        running_count,
        succeeded_count,
        failed_count,
        stale_after_seconds,
        task_timeout_seconds,
        provider_summary_json: provider_summary_json.and_then(|v| serde_json::from_str(&v).ok()),
        filters_json: filters_json.and_then(|v| serde_json::from_str(&v).ok()),
        created_at,
        updated_at,
    })
}

async fn load_verify_metrics(state: &AppState) -> Result<VerifyMetricsResponse, (StatusCode, String)> {
    let (verified_ok, verified_failed, geo_match_ok, stale_or_missing_verify): (i64, i64, i64, i64) =
        sqlx::query_as(
            r#"SELECT
                   COALESCE(SUM(CASE WHEN last_verify_status = 'ok' THEN 1 ELSE 0 END), 0) AS verified_ok,
                   COALESCE(SUM(CASE WHEN last_verify_status = 'failed' THEN 1 ELSE 0 END), 0) AS verified_failed,
                   COALESCE(SUM(CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 1 ELSE 0 END), 0) AS geo_match_ok,
                   COALESCE(SUM(CASE WHEN last_verify_at IS NULL OR last_verify_status IS NULL THEN 1 ELSE 0 END), 0) AS stale_or_missing_verify
               FROM proxies"#,
        )
        .fetch_one(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to aggregate verify metrics: {err}")))?;

    Ok(VerifyMetricsResponse { verified_ok, verified_failed, geo_match_ok, stale_or_missing_verify })
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
    let rows = sqlx::query_as::<_, (String, String, String, i32, Option<String>, Option<i64>, Option<String>)>(
        r#"SELECT id, kind, status, priority, fingerprint_profile_id, fingerprint_profile_version, result_json FROM tasks ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#,
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

    let latest_tasks: Vec<TaskResponse> = rows
        .into_iter()
        .map(|(id, kind, status, priority, fingerprint_profile_id, fingerprint_profile_version, result_json)| {
            let proxy_resolution_status = proxy_resolution_status(result_json.as_deref());
            let (proxy_id, proxy_provider, proxy_region) = proxy_identity(result_json.as_deref());
            TaskResponse {
                fingerprint_resolution_status: fingerprint_resolution_status(
                    fingerprint_profile_id.as_deref(),
                    fingerprint_profile_version,
                    result_json.as_deref(),
                ),
                proxy_id,
                proxy_provider,
                proxy_region,
                proxy_resolution_status,
                id,
                kind,
                status,
                priority,
                fingerprint_profile_id,
                fingerprint_profile_version,
            }
        })
        .collect();

    let fingerprint_metrics = build_fingerprint_metrics(&latest_tasks);
    let proxy_metrics = build_proxy_metrics(&latest_tasks);
    let verify_metrics = load_verify_metrics(&state).await?;

    Ok(Json(StatusResponse {
        service: "AutoOpenBrowser".to_string(),
        queue_len: counts.queued as usize,
        counts,
        worker: WorkerStatusResponse {
            worker_count: state.worker_count,
            queue_mode: "db_first_with_memory_compat".to_string(),
            reclaim_after_seconds: crate::runner::runner_reclaim_seconds_from_env(),
            heartbeat_interval_seconds: crate::runner::runner_heartbeat_interval_seconds_from_env(),
            claim_retry_limit: crate::runner::runner_claim_retry_limit_from_env(),
            idle_backoff_min_ms: crate::runner::runner_idle_backoff_min_ms_from_env(),
            idle_backoff_max_ms: crate::runner::runner_idle_backoff_max_ms_from_env(),
        },
        fingerprint_metrics,
        proxy_metrics,
        verify_metrics,
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
    let profile_version = if let Some(profile_id) = payload.fingerprint_profile_id.as_deref() {
        let version = sqlx::query_scalar::<_, i64>(r#"SELECT version FROM fingerprint_profiles WHERE id = ? AND status = 'active'"#)
            .bind(profile_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to resolve fingerprint profile version: {err}")))?;
        if version.is_none() {
            return Err((StatusCode::BAD_REQUEST, "fingerprint profile not found or inactive".to_string()));
        }
        version
    } else {
        None
    };

    let network_policy_value = payload.network_policy_json.clone();
    let network_policy_json = network_policy_value.as_ref().map(|v| v.to_string());
    let input_json = serde_json::json!({
        "url": payload.url,
        "script": payload.script,
        "timeout_seconds": payload.timeout_seconds,
        "fingerprint_profile_id": payload.fingerprint_profile_id,
        "fingerprint_profile_version": profile_version,
        "proxy_id": payload.proxy_id,
        "network_policy_json": network_policy_value
    })
    .to_string();
    let created_at = now_ts_string();
    let queued_at = now_ts_string();

    sqlx::query(
        r#"
        INSERT INTO tasks (
            id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
            priority, created_at, queued_at, started_at, finished_at, fingerprint_profile_id,
            fingerprint_profile_version, result_json, error_message
        ) VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?, NULL, NULL, ?, ?, NULL, NULL)
        "#,
    )
    .bind(&task_id)
    .bind(&payload.kind)
    .bind(TASK_STATUS_QUEUED)
    .bind(&input_json)
    .bind(&network_policy_json)
    .bind(priority)
    .bind(&created_at)
    .bind(&queued_at)
    .bind(&payload.fingerprint_profile_id)
    .bind(profile_version)
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
            fingerprint_profile_id: payload.fingerprint_profile_id,
            fingerprint_profile_version: profile_version,
            fingerprint_resolution_status: profile_version.map(|_| "pending".to_string()),
            proxy_id: None,
            proxy_provider: None,
            proxy_region: None,
            proxy_resolution_status: payload.network_policy_json.as_ref().and_then(|v| v.get("mode")).and_then(|v| v.as_str()).map(|mode| if mode == "direct" { "direct".to_string() } else { "pending".to_string() }),
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

    let row = sqlx::query_as::<_, (String, String, String, i32, Option<String>, Option<i64>, Option<String>)>(
        r#"SELECT id, kind, status, priority, fingerprint_profile_id, fingerprint_profile_version, result_json FROM tasks WHERE id = ?"#,
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
        Some((id, kind, status, priority, fingerprint_profile_id, fingerprint_profile_version, result_json)) => {
            let proxy_resolution_status = proxy_resolution_status(result_json.as_deref());
            let (proxy_id, proxy_provider, proxy_region) = proxy_identity(result_json.as_deref());
            Ok(Json(TaskResponse {
                fingerprint_resolution_status: fingerprint_resolution_status(
                    fingerprint_profile_id.as_deref(),
                    fingerprint_profile_version,
                    result_json.as_deref(),
                ),
                proxy_id,
                proxy_provider,
                proxy_region,
                proxy_resolution_status,
                id,
                kind,
                status,
                priority,
                fingerprint_profile_id,
                fingerprint_profile_version,
            }))
        },
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


pub async fn create_fingerprint_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreateFingerprintProfileRequest>,
) -> Result<(StatusCode, Json<FingerprintProfileResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "fingerprint profile id and name are required".to_string()));
    }

    let validation = validate_fingerprint_profile(&payload.profile_json);
    let now = now_ts_string();
    let profile_json = payload.profile_json.to_string();

    sqlx::query(
        r#"
        INSERT INTO fingerprint_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
        VALUES (?, ?, 1, 'active', ?, ?, ?, ?)
        "#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(&payload.tags_json)
    .bind(&profile_json)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create fingerprint profile: {err}")))?;

    Ok((
        StatusCode::CREATED,
        Json(FingerprintProfileResponse {
            id: payload.id,
            name: payload.name,
            version: 1,
            status: "active".to_string(),
            tags_json: payload.tags_json,
            profile_json: payload.profile_json,
            validation_ok: validation.ok,
            validation_issues: validation.issues,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_fingerprint_profiles(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<FingerprintProfileResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, i64, String, Option<String>, String, String, String)>(
        r#"SELECT id, name, version, status, tags_json, profile_json, created_at, updated_at FROM fingerprint_profiles ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list fingerprint profiles: {err}")))?;

    let items = rows.into_iter().map(|(id, name, version, status, tags_json, profile_json, created_at, updated_at)| {
        let profile_json = serde_json::from_str(&profile_json).unwrap_or_else(|_| serde_json::json!({}));
        let validation = validate_fingerprint_profile(&profile_json);
        FingerprintProfileResponse { id, name, version, status, tags_json, profile_json, validation_ok: validation.ok, validation_issues: validation.issues, created_at, updated_at }
    }).collect();

    Ok(Json(items))
}

pub async fn get_fingerprint_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<FingerprintProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, String, i64, String, Option<String>, String, String, String)>(
        r#"SELECT id, name, version, status, tags_json, profile_json, created_at, updated_at FROM fingerprint_profiles WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch fingerprint profile: {err}")))?;

    match row {
        Some((id, name, version, status, tags_json, profile_json, created_at, updated_at)) => {
            let profile_json = serde_json::from_str(&profile_json).unwrap_or_else(|_| serde_json::json!({}));
            let validation = validate_fingerprint_profile(&profile_json);
            Ok(Json(FingerprintProfileResponse { id, name, version, status, tags_json, profile_json, validation_ok: validation.ok, validation_issues: validation.issues, created_at, updated_at }))
        }
        None => Err((StatusCode::NOT_FOUND, format!("fingerprint profile not found: {profile_id}"))),
    }
}


pub async fn create_proxy(
    State(state): State<AppState>,
    Json(payload): Json<CreateProxyRequest>,
) -> Result<(StatusCode, Json<ProxyResponse>), (StatusCode, String)> {
    if payload.id.trim().is_empty() || payload.scheme.trim().is_empty() || payload.host.trim().is_empty() || payload.port <= 0 {
        return Err((StatusCode::BAD_REQUEST, "proxy id/scheme/host/port are required".to_string()));
    }
    let now = now_ts_string();
    let status = payload.status.unwrap_or_else(|| "active".to_string());
    let score = payload.score.unwrap_or(1.0);
    sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, ?, ?)"#)
        .bind(&payload.id).bind(&payload.scheme).bind(&payload.host).bind(payload.port)
        .bind(&payload.username).bind(&payload.password).bind(&payload.region).bind(&payload.country).bind(&payload.provider)
        .bind(&status).bind(score).bind(&now).bind(&now)
        .execute(&state.db).await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to create proxy: {err}")))?;
    Ok((StatusCode::CREATED, Json(ProxyResponse {
        id: payload.id, scheme: payload.scheme, host: payload.host, port: payload.port, username: payload.username,
        region: payload.region, country: payload.country, provider: payload.provider, status, score, success_count: 0, failure_count: 0,
        last_checked_at: None, last_used_at: None, cooldown_until: None,
        last_smoke_status: None, last_smoke_protocol_ok: None, last_smoke_upstream_ok: None,
        last_exit_ip: None, last_anonymity_level: None, last_smoke_at: None,
        last_verify_status: None, last_verify_geo_match_ok: None, last_exit_country: None, last_exit_region: None, last_verify_at: None,
        created_at: now.clone(), updated_at: now,
    })))
}

pub async fn list_proxies(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<ProxyResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, ProxyRow>(r#"SELECT id, scheme, host, port, username, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at FROM proxies ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list proxies: {err}")))?;
    Ok(Json(rows.into_iter().map(map_proxy_row).collect()))
}

pub async fn get_proxy(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxyResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, ProxyRow>(r#"SELECT id, scheme, host, port, username, region, country, provider, status, score, success_count, failure_count, last_checked_at, last_used_at, cooldown_until, last_smoke_status, last_smoke_protocol_ok, last_smoke_upstream_ok, last_exit_ip, last_anonymity_level, last_smoke_at, last_verify_status, last_verify_geo_match_ok, last_exit_country, last_exit_region, last_verify_at, created_at, updated_at FROM proxies WHERE id = ?"#)
        .bind(&proxy_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch proxy: {err}")))?;
    match row {
        Some(row) => Ok(Json(map_proxy_row(row))),
        None => Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}"))),
    }
}


pub async fn smoke_test_proxy(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxySmokeResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, i64)>(r#"SELECT host, port FROM proxies WHERE id = ?"#)
        .bind(&proxy_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to load proxy for smoke test: {err}")))?;

    let Some((host, port)) = row else {
        return Err((StatusCode::NOT_FOUND, format!("proxy not found: {proxy_id}")));
    };

    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|err| (StatusCode::BAD_REQUEST, format!("proxy address is invalid for smoke test: {err}")))?;

    let started = Instant::now();
    let mut stream = tokio::time::timeout(std::time::Duration::from_secs(3), tokio::net::TcpStream::connect(addr))
        .await
        .ok()
        .and_then(|result| result.ok());
    let reachable = stream.is_some();
    let mut protocol_ok = false;
    let mut upstream_ok = false;
    let mut exit_ip: Option<String> = None;
    let mut anonymity_level: Option<String> = None;
    let mut smoke_message = if reachable {
        "tcp connect succeeded but proxy protocol not validated".to_string()
    } else {
        "tcp smoke test failed".to_string()
    };

    if let Some(stream_ref) = stream.as_mut() {
        let probe = b"CONNECT example.com:443 HTTP/1.1
Host: example.com:443

";
        if tokio::time::timeout(std::time::Duration::from_secs(3), stream_ref.write_all(probe)).await.ok().is_some() {
            let mut buf = [0_u8; 512];
            if let Ok(Ok(n)) = tokio::time::timeout(std::time::Duration::from_secs(3), stream_ref.read(&mut buf)).await {
                if n > 0 {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    let text_lower = text.to_ascii_lowercase();
                    if text_lower.contains("http/1.1") || text_lower.contains("http/1.0") {
                        protocol_ok = true;
                        let has_via = text_lower.contains("via:");
                        let has_forwarded = text_lower.contains("forwarded:") || text_lower.contains("x-forwarded-for:");
                        anonymity_level = Some(if has_forwarded { "transparent".to_string() } else if has_via { "anonymous".to_string() } else { "elite".to_string() });
                        if let Some(idx) = text.find("ip=") {
                            let ip = text[idx + 3..].lines().next().unwrap_or("").trim().to_string();
                            if !ip.is_empty() {
                                upstream_ok = true;
                                exit_ip = Some(ip.clone());
                                smoke_message = format!("http proxy smoke test got upstream ip={ip}");
                            }
                        }
                        if !upstream_ok {
                            smoke_message = "http connect smoke test received proxy response".to_string();
                        }
                    } else {
                        smoke_message = format!("tcp connect ok but proxy response was not http-like: {text_lower}");
                    }
                }
            }
        }
    }

    let latency_ms = Some(started.elapsed().as_millis());
    let now = now_ts_string();

    if reachable && protocol_ok {
        sqlx::query(r#"UPDATE proxies SET last_checked_at = ?, cooldown_until = NULL, last_smoke_status = ?, last_smoke_protocol_ok = ?, last_smoke_upstream_ok = ?, last_exit_ip = ?, last_anonymity_level = ?, last_smoke_at = ?, updated_at = ? WHERE id = ?"#)
            .bind(&now)
            .bind("ok")
            .bind(1_i64)
            .bind(if upstream_ok { 1_i64 } else { 0_i64 })
            .bind(&exit_ip)
            .bind(&anonymity_level)
            .bind(&now)
            .bind(&now)
            .bind(&proxy_id)
            .execute(&state.db)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update proxy after smoke success: {err}")))?;

        Ok(Json(ProxySmokeResponse {
            id: proxy_id,
            reachable: true,
            protocol_ok: true,
            upstream_ok,
            exit_ip,
            anonymity_level,
            latency_ms,
            status: "ok".to_string(),
            message: smoke_message,
        }))
    } else {
        let cooldown_until = (now.parse::<u64>().unwrap_or(0) + 60).to_string();
        sqlx::query(r#"UPDATE proxies SET failure_count = failure_count + 1, last_checked_at = ?, cooldown_until = ?, last_smoke_status = ?, last_smoke_protocol_ok = ?, last_smoke_upstream_ok = ?, last_exit_ip = ?, last_anonymity_level = ?, last_smoke_at = ?, updated_at = ? WHERE id = ?"#)
            .bind(&now)
            .bind(&cooldown_until)
            .bind("failed")
            .bind(if protocol_ok { 1_i64 } else { 0_i64 })
            .bind(if upstream_ok { 1_i64 } else { 0_i64 })
            .bind(&exit_ip)
            .bind(&anonymity_level)
            .bind(&now)
            .bind(&now)
            .bind(&proxy_id)
            .execute(&state.db)
            .await
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to update proxy after smoke failure: {err}")))?;

        Ok(Json(ProxySmokeResponse {
            id: proxy_id,
            reachable,
            protocol_ok,
            upstream_ok,
            exit_ip,
            anonymity_level,
            latency_ms,
            status: "failed".to_string(),
            message: smoke_message,
        }))
    }
}




pub async fn list_verify_batches(
    State(state): State<AppState>,
    Query(query): Query<VerifyBatchListQuery>,
) -> Result<Json<Vec<VerifyBatchResponse>>, (StatusCode, String)> {
    let limit = sanitize_limit(query.limit, 20, 200);
    let offset = sanitize_offset(query.offset);
    let rows = sqlx::query_as::<_, (String, String, i64, i64, i64, i64, i64, Option<String>, Option<String>, String, String)>(
        r#"SELECT id, status, requested_count, accepted_count, skipped_count, stale_after_seconds, task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at
           FROM verify_batches ORDER BY created_at DESC, id DESC LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to list verify batches: {err}")))?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(map_verify_batch_row(&state, row.0,row.1,row.2,row.3,row.4,row.5,row.6,row.7,row.8,row.9,row.10).await?);
    }
    Ok(Json(items))
}

pub async fn get_verify_batch(
    State(state): State<AppState>,
    Path(batch_id): Path<String>,
) -> Result<Json<VerifyBatchResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, String, i64, i64, i64, i64, i64, Option<String>, Option<String>, String, String)>(
        r#"SELECT id, status, requested_count, accepted_count, skipped_count, stale_after_seconds, task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at
           FROM verify_batches WHERE id = ?"#,
    )
    .bind(&batch_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to fetch verify batch: {err}")))?;

    match row {
        Some(row) => Ok(Json(map_verify_batch_row(&state, row.0,row.1,row.2,row.3,row.4,row.5,row.6,row.7,row.8,row.9,row.10).await?)),
        None => Err((StatusCode::NOT_FOUND, format!("verify batch not found: {batch_id}"))),
    }
}

pub async fn verify_batch_proxies(
    State(state): State<AppState>,
    Json(payload): Json<ProxyVerifyBatchRequest>,
) -> Result<(StatusCode, Json<ProxyVerifyBatchResponse>), (StatusCode, String)> {
    let requested = sanitize_limit(payload.limit, 20, 200);
    let min_score = payload.min_score.unwrap_or(0.0);
    let only_stale = payload.only_stale.unwrap_or(true);
    let stale_after_seconds = payload.stale_after_seconds.unwrap_or(3600).max(60);
    let task_timeout_seconds = payload.task_timeout_seconds.unwrap_or(5).max(1);
    let recently_used_within_seconds = payload.recently_used_within_seconds.unwrap_or(0).max(0);
    let failed_only = payload.failed_only.unwrap_or(false);
    let max_per_provider = payload.max_per_provider.unwrap_or(requested).max(1);
    let now = now_ts_string();
    let batch_id = format!("verify-batch-{}", Uuid::new_v4());
    let rows = sqlx::query_as::<_, (String, Option<String>)>(
        r#"SELECT id, provider FROM proxies
           WHERE status = 'active'
             AND (? IS NULL OR provider = ?)
             AND (? IS NULL OR region = ?)
             AND score >= ?
             AND (
               ? = 0
               OR last_verify_at IS NULL
               OR last_verify_status IS NULL
               OR last_verify_status != 'ok'
               OR CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - ?
             )
             AND (
               ? = 0
               OR CAST(COALESCE(last_used_at, '0') AS INTEGER) >= CAST(? AS INTEGER) - ?
             )
             AND (
               ? = 0
               OR last_verify_status = 'failed'
             )
           ORDER BY
             CASE WHEN last_verify_status = 'ok' THEN 1 ELSE 0 END ASC,
             COALESCE(last_verify_at, '0') ASC,
             score DESC,
             created_at ASC
           LIMIT ?"#,
    )
    .bind(&payload.provider)
    .bind(&payload.provider)
    .bind(&payload.region)
    .bind(&payload.region)
    .bind(min_score)
    .bind(if only_stale { 1_i64 } else { 0_i64 })
    .bind(&now)
    .bind(stale_after_seconds)
    .bind(if recently_used_within_seconds > 0 { 1_i64 } else { 0_i64 })
    .bind(&now)
    .bind(recently_used_within_seconds)
    .bind(if failed_only { 1_i64 } else { 0_i64 })
    .bind(requested.saturating_mul(4))
    .fetch_all(&state.db)
    .await
    .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to select proxies for verify batch: {err}")))?;

    let mut accepted = 0_i64;
    let mut per_provider_counts = std::collections::BTreeMap::<String, i64>::new();
    let mut per_provider_skipped = std::collections::BTreeMap::<String, i64>::new();
    for (proxy_id, provider) in &rows {
        if accepted >= requested {
            break;
        }
        let provider_key = provider.clone().unwrap_or_else(|| "__none__".to_string());
        let current = *per_provider_counts.get(&provider_key).unwrap_or(&0);
        if current >= max_per_provider {
            *per_provider_skipped.entry(provider_key).or_insert(0) += 1;
            continue;
        }
        let task_id = format!("task-{}", Uuid::new_v4());
        let created_at = now_ts_string();
        let input_json = serde_json::json!({
            "url": serde_json::Value::Null,
            "script": serde_json::Value::Null,
            "timeout_seconds": task_timeout_seconds,
            "fingerprint_profile_id": serde_json::Value::Null,
            "fingerprint_profile_version": serde_json::Value::Null,
            "proxy_id": proxy_id,
            "verify_batch_id": batch_id,
            "network_policy_json": serde_json::Value::Null,
        }).to_string();
        sqlx::query(
            r#"INSERT INTO tasks (
                id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
                priority, created_at, queued_at, started_at, finished_at, fingerprint_profile_id,
                fingerprint_profile_version, runner_id, heartbeat_at, result_json, error_message
            ) VALUES (?, 'verify_proxy', ?, ?, NULL, NULL, 0, ?, ?, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)"#,
        )
        .bind(&task_id)
        .bind(TASK_STATUS_QUEUED)
        .bind(&input_json)
        .bind(&created_at)
        .bind(&created_at)
        .execute(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to enqueue verify task: {err}")))?;
        accepted += 1;
        per_provider_counts.insert(provider_key, current + 1);
    }

    let provider_summary: Vec<ProxyVerifyBatchProviderSummary> = per_provider_counts
        .into_iter()
        .map(|(provider, accepted)| ProxyVerifyBatchProviderSummary {
            skipped_due_to_cap: per_provider_skipped.get(&provider).copied().unwrap_or(0),
            provider,
            accepted,
        })
        .collect();
    let provider_summary_json = serde_json::to_string(&provider_summary)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to encode provider summary: {err}")))?;
    let filters_json = serde_json::json!({
        "provider": payload.provider,
        "region": payload.region,
        "limit": requested,
        "only_stale": only_stale,
        "min_score": min_score,
        "stale_after_seconds": stale_after_seconds,
        "task_timeout_seconds": task_timeout_seconds,
        "recently_used_within_seconds": recently_used_within_seconds,
        "failed_only": failed_only,
        "max_per_provider": max_per_provider,
    }).to_string();
    sqlx::query(r#"INSERT INTO verify_batches (id, status, requested_count, accepted_count, skipped_count, stale_after_seconds, task_timeout_seconds, provider_summary_json, filters_json, created_at, updated_at)
                   VALUES (?, 'scheduled', ?, ?, ?, ?, ?, ?, ?, ?, ?)"#)
        .bind(&batch_id)
        .bind(requested)
        .bind(accepted)
        .bind(requested - accepted)
        .bind(stale_after_seconds)
        .bind(task_timeout_seconds)
        .bind(&provider_summary_json)
        .bind(&filters_json)
        .bind(&now)
        .bind(&now)
        .execute(&state.db)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("failed to persist verify batch: {err}")))?;

    Ok((
        StatusCode::ACCEPTED,
        Json(ProxyVerifyBatchResponse {
            batch_id,
            created_at: now,
            requested,
            accepted,
            skipped: requested - accepted,
            stale_after_seconds,
            task_timeout_seconds,
            provider_summary,
            status: "scheduled".to_string(),
        }),
    ))
}

pub async fn verify_proxy(
    State(state): State<AppState>,
    Path(proxy_id): Path<String>,
) -> Result<Json<ProxyVerifyResponse>, (StatusCode, String)> {
    Ok(Json(run_proxy_verify_probe(&state, &proxy_id).await?))
}

