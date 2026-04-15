use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;
use AutoOpenBrowser::{
    api::{
        handlers::{append_continuity_event, run_persona_heartbeat_tick},
        routes::build_router,
    },
    app::build_app_state,
    build_test_app,
    db::init::init_db,
    domain::task::{
        TASK_STATUS_CANCELLED, TASK_STATUS_FAILED, TASK_STATUS_QUEUED, TASK_STATUS_RUNNING,
        TASK_STATUS_SUCCEEDED, TASK_STATUS_TIMED_OUT,
    },
    runner::{
        fake::FakeRunner, spawn_runner_workers, RunnerExecutionResult, RunnerTask, TaskRunner,
    },
};

fn unique_db_url() -> String {
    format!(
        "sqlite:///tmp/auto_open_browser_continuity_test_{}.db",
        Uuid::new_v4()
    )
}

async fn json_response(app: &axum::Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(request)
        .await
        .expect("request should succeed");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body bytes");
    let json = serde_json::from_slice::<Value>(&body).unwrap_or_else(|err| {
        panic!(
            "json body: {err}; status={status}; raw={}",
            String::from_utf8_lossy(&body)
        )
    });
    (status, json)
}

async fn seed_persona_bundle(
    db: &AutoOpenBrowser::db::init::DbPool,
    persona_id: &str,
    store_id: &str,
    platform_id: &str,
) {
    sqlx::query(
        r#"INSERT INTO fingerprint_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
           VALUES ('fp-seed', 'Seed Profile', 1, 'active', NULL, '{"browser":{"name":"chrome"}}', '1', '1')"#,
    )
    .execute(db)
    .await
    .expect("insert fingerprint profile");

    sqlx::query(
        r#"INSERT INTO network_policies (
               id, name, country_anchor, region_anchor, allow_same_country_fallback,
               allow_same_region_fallback, provider_preference, allowed_regions_json,
               network_policy_json, status, created_at, updated_at
           ) VALUES (
               'np-seed', 'Seed Policy', 'CN', 'CN-31', 1, 0, NULL, '["CN-31"]',
               '{"mode":"required_proxy","region":"cn-shanghai"}', 'active', '1', '1'
           )"#,
    )
    .execute(db)
    .await
    .expect("insert network policy");

    sqlx::query(
        r#"INSERT INTO continuity_policies (
               id, name, session_ttl_seconds, heartbeat_interval_seconds, site_group_mode,
               recovery_enabled, protect_on_login_loss, policy_json, status, created_at, updated_at
           ) VALUES (
               'cp-seed', 'Seed Continuity', 86400, 60, 'host', 1, 1, '{}', 'active', '1', '1'
           )"#,
    )
    .execute(db)
    .await
    .expect("insert continuity policy");

    sqlx::query(
        r#"INSERT OR IGNORE INTO platform_templates (
               id, platform_id, name, warm_paths_json, revisit_paths_json, stateful_paths_json,
               write_operation_paths_json, high_risk_paths_json, allowed_regions_json,
               preferred_locale, preferred_timezone, continuity_checks_json, login_loss_signals_json,
               recovery_steps_json, readiness_level, status, created_at, updated_at
           ) VALUES (
               'tpl-seed', ?, 'Seed Template',
               '["/dashboard"]',
               '["/dashboard","/notes"]',
               '["/dashboard","/notes"]',
               '["/publish","/products"]',
               '["/security","/finance","/team","/permissions","/account","/inventory"]',
               '["CN-31"]',
               'zh-CN', 'Asia/Shanghai',
               '[\"login_state\",\"identity\",\"region\",\"dashboard\",\"notes\"]',
               '[\"login\"]',
               '[\"reload\",\"revisit\"]',
               'sample_ready', 'active', '1', '1'
           )"#,
    )
    .bind(platform_id)
    .execute(db)
    .await
    .expect("insert platform template");

    sqlx::query(
        r#"INSERT INTO persona_profiles (
               id, store_id, platform_id, device_family, country_anchor, region_anchor, locale, timezone,
               fingerprint_profile_id, network_policy_id, continuity_policy_id, credential_ref, status,
               created_at, updated_at
           ) VALUES (?, ?, ?, 'desktop', 'CN', 'CN-31', 'zh-CN', 'Asia/Shanghai',
                     'fp-seed', 'np-seed', 'cp-seed', NULL, 'active', '1', '1')"#,
    )
    .bind(persona_id)
    .bind(store_id)
    .bind(platform_id)
    .execute(db)
    .await
    .expect("insert persona profile");
}

async fn seed_active_proxy(
    db: &AutoOpenBrowser::db::init::DbPool,
    proxy_id: &str,
    provider: &str,
    region: &str,
    country: &str,
) {
    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status, score,
               success_count, failure_count, last_verify_status, last_verify_geo_match_ok,
               last_smoke_upstream_ok, last_exit_country, last_exit_region, last_verify_at,
               created_at, updated_at
           ) VALUES (
               ?, 'http', '127.0.0.1', 8080, NULL, NULL, ?, ?, ?, 'active', 0.95,
               0, 0, 'ok', 1, 1, ?, ?, '9999999999', '1', '1'
           )"#,
    )
    .bind(proxy_id)
    .bind(region)
    .bind(country)
    .bind(provider)
    .bind(country)
    .bind(region)
    .execute(db)
    .await
    .expect("insert active proxy");
}

async fn build_state_without_workers(database_url: &str) -> AutoOpenBrowser::app::state::AppState {
    let db = init_db(database_url).await.expect("init db");
    let runner: Arc<dyn TaskRunner> = Arc::new(FakeRunner);
    build_app_state(db, runner, None, 1)
}

async fn build_test_app_with_runner(
    database_url: &str,
    runner: Arc<dyn TaskRunner>,
) -> (AutoOpenBrowser::app::state::AppState, axum::Router) {
    let db = init_db(database_url).await.expect("init db");
    let state = build_app_state(db, runner.clone(), None, 1);
    spawn_runner_workers(state.clone(), runner, 1).await;
    let app = build_router(state.clone());
    (state, app)
}

async fn wait_for_task_result_json(
    db: &AutoOpenBrowser::db::init::DbPool,
    task_id: &str,
) -> Value {
    for _ in 0..40 {
        let row = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT status, result_json FROM tasks WHERE id = ?",
        )
        .bind(task_id)
        .fetch_one(db)
        .await
        .expect("load task status");
        if !matches!(
            row.0.as_str(),
            TASK_STATUS_QUEUED | TASK_STATUS_RUNNING | "pending"
        ) {
            assert!(
                matches!(
                    row.0.as_str(),
                    TASK_STATUS_SUCCEEDED
                        | TASK_STATUS_FAILED
                        | TASK_STATUS_CANCELLED
                        | TASK_STATUS_TIMED_OUT
                ),
                "unexpected terminal task status: {}",
                row.0
            );
            return serde_json::from_str(
                row.1
                    .as_deref()
                    .expect("terminal task should have result json"),
            )
            .expect("parse task result json");
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("task did not reach terminal status in time: {task_id}");
}

async fn wait_for_continuity_event_json(
    db: &AutoOpenBrowser::db::init::DbPool,
    persona_id: &str,
    event_type: &str,
) -> Value {
    for _ in 0..40 {
        let event_json = sqlx::query_scalar::<_, Option<String>>(
            r#"SELECT event_json
               FROM continuity_events
               WHERE persona_id = ?
                 AND event_type = ?
               ORDER BY created_at DESC, id DESC
               LIMIT 1"#,
        )
        .bind(persona_id)
        .bind(event_type)
        .fetch_one(db)
        .await
        .expect("query continuity event");
        if let Some(raw) = event_json {
            return serde_json::from_str(&raw).expect("parse continuity event json");
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("continuity event not observed in time: {persona_id} {event_type}");
}

async fn wait_for_persona_health_snapshot_json(
    db: &AutoOpenBrowser::db::init::DbPool,
    persona_id: &str,
) -> Value {
    for _ in 0..40 {
        let snapshot_json = sqlx::query_scalar::<_, Option<String>>(
            r#"SELECT snapshot_json
               FROM persona_health_snapshots
               WHERE persona_id = ?
               ORDER BY created_at DESC, id DESC
               LIMIT 1"#,
        )
        .bind(persona_id)
        .fetch_one(db)
        .await
        .expect("query persona health snapshot");
        if let Some(raw) = snapshot_json {
            let parsed: Value = serde_json::from_str(&raw).expect("parse snapshot json");
            if parsed
                .get("last_continuity_check_results")
                .and_then(|value| value.get("matched_identity_marker"))
                .and_then(Value::as_str)
                .is_some()
            {
                return parsed;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("persona health snapshot did not expose matched identity marker in time: {persona_id}");
}

struct FixedProbeRunner {
    final_url: String,
    title: String,
    text_preview: String,
    html_preview: String,
}

#[async_trait]
impl TaskRunner for FixedProbeRunner {
    fn name(&self) -> &'static str {
        "fixed_probe"
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult {
        RunnerExecutionResult::success(Some(json!({
            "runner": "fixed_probe",
            "ok": true,
            "status": "succeeded",
            "requested_action": task.kind.clone(),
            "action": task.kind.clone(),
            "task_id": task.task_id.clone(),
            "attempt": task.attempt,
            "kind": task.kind,
            "url": task.payload.get("url").cloned().unwrap_or(Value::Null),
            "title": self.title.clone(),
            "final_url": self.final_url.clone(),
            "text_preview": self.text_preview.clone(),
            "content_preview": self.text_preview.clone(),
            "html_preview": self.html_preview.clone(),
            "message": "fixed sample-ready probe succeeded"
        })))
    }
}

#[tokio::test]
async fn manual_gate_uses_standard_permissions_team_category() {
    let db_url = unique_db_url();
    let (state, app) = build_test_app(&db_url).await.expect("build app");
    seed_persona_bundle(&state.db, "persona-gate", "store-gate", "xiaohongshu").await;

    let payload = json!({
        "kind": "open_page",
        "url": "https://seller.xiaohongshu.com/security/permissions",
        "timeout_seconds": 30,
        "persona_id": "persona-gate"
    });
    let (status, body) = json_response(
        &app,
        Request::builder()
            .method("POST")
            .uri("/tasks")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("request"),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        body.get("manual_gate_status").and_then(Value::as_str),
        Some("pending")
    );

    let requested_action_kind: String = sqlx::query_scalar(
        "SELECT requested_action_kind FROM manual_gate_requests WHERE persona_id = 'persona-gate' LIMIT 1",
    )
    .fetch_one(&state.db)
    .await
    .expect("load manual gate category");
    assert_eq!(requested_action_kind, "permissions_team");
}

#[tokio::test]
async fn login_risk_detected_freezes_persona_immediately() {
    let db_url = unique_db_url();
    let (state, _app) = build_test_app(&db_url).await.expect("build app");
    seed_persona_bundle(&state.db, "persona-frozen", "store-frozen", "xiaohongshu").await;

    let event_json = json!({
        "matched_signal": "login"
    });
    append_continuity_event(
        &state,
        Some("persona-frozen"),
        Some("store-frozen"),
        Some("xiaohongshu"),
        None,
        None,
        "login_risk_detected",
        "warning",
        Some(&event_json),
    )
    .await
    .expect("append continuity event");

    let status: String =
        sqlx::query_scalar("SELECT status FROM persona_profiles WHERE id = 'persona-frozen'")
            .fetch_one(&state.db)
            .await
            .expect("load persona status");
    assert_eq!(status, "frozen");
}

#[tokio::test]
async fn three_heartbeat_failures_degrade_and_scheduled_recovery_returns_active() {
    let db_url = unique_db_url();
    let (state, _app) = build_test_app(&db_url).await.expect("build app");
    seed_persona_bundle(&state.db, "persona-degraded", "store-degraded", "shopify").await;

    for _ in 0..3 {
        let event_json = json!({
            "reason": "heartbeat_target_origin_unresolved",
            "reason_bucket": "origin_unresolved"
        });
        append_continuity_event(
            &state,
            Some("persona-degraded"),
            Some("store-degraded"),
            Some("shopify"),
            None,
            None,
            "heartbeat_failed",
            "warning",
            Some(&event_json),
        )
        .await
        .expect("append heartbeat failed");
    }

    let degraded_status: String =
        sqlx::query_scalar("SELECT status FROM persona_profiles WHERE id = 'persona-degraded'")
            .fetch_one(&state.db)
            .await
            .expect("load degraded status");
    assert_eq!(degraded_status, "degraded");

    let scheduled_event_json = json!({
        "reason": "heartbeat_due",
        "reason_bucket": "scheduled_due"
    });
    append_continuity_event(
        &state,
        Some("persona-degraded"),
        Some("store-degraded"),
        Some("shopify"),
        None,
        None,
        "heartbeat_scheduled",
        "info",
        Some(&scheduled_event_json),
    )
    .await
    .expect("append heartbeat scheduled");

    let recovered_status: String =
        sqlx::query_scalar("SELECT status FROM persona_profiles WHERE id = 'persona-degraded'")
            .fetch_one(&state.db)
            .await
            .expect("load recovered status");
    assert_eq!(recovered_status, "active");
}

#[tokio::test]
async fn pending_manual_gate_persona_is_excluded_from_heartbeat_tick() {
    let db_url = unique_db_url();
    let (state, _app) = build_test_app(&db_url).await.expect("build app");
    seed_persona_bundle(
        &state.db,
        "persona-pending-gate",
        "store-pending-gate",
        "xiaohongshu",
    )
    .await;

    sqlx::query(
        r#"INSERT INTO tasks (
               id, kind, status, input_json, network_policy_json, fingerprint_profile_json,
               persona_id, platform_id, manual_gate_request_id, priority, created_at, queued_at,
               started_at, finished_at, fingerprint_profile_id, fingerprint_profile_version,
               result_json, error_message
           ) VALUES (
               'task-pending', 'open_page', 'pending', '{"url":"https://seller.xiaohongshu.com/security"}',
               NULL, NULL, 'persona-pending-gate', 'xiaohongshu', 'gate-pending', 0, '1', NULL,
               NULL, NULL, 'fp-seed', 1, NULL, NULL
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert pending task");

    sqlx::query(
        r#"INSERT INTO manual_gate_requests (
               id, task_id, persona_id, store_id, platform_id, requested_action_kind,
               requested_url, reason_code, reason_summary, status, resolution_note,
               created_at, updated_at, resolved_at
           ) VALUES (
               'gate-pending', 'task-pending', 'persona-pending-gate', 'store-pending-gate',
               'xiaohongshu', 'permissions_team', 'https://seller.xiaohongshu.com/security',
               'high_risk_path', 'pending manual gate', 'pending', NULL, '1', '1', NULL
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert pending manual gate");

    let response = run_persona_heartbeat_tick(&state)
        .await
        .expect("run heartbeat tick");
    assert_eq!(response.evaluated_count, 0);
    assert!(response.items.is_empty());
}

#[tokio::test]
async fn fresh_db_bootstraps_canonical_xiaohongshu_sample_ready_template() {
    let db_url = unique_db_url();
    let state = build_state_without_workers(&db_url).await;

    let row = sqlx::query_as::<_, (String, String, String, String)>(
        r#"SELECT platform_id, readiness_level, continuity_checks_json, revisit_paths_json
           FROM platform_templates
           WHERE id = 'tpl-canonical-xiaohongshu-sample-ready'"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load canonical xiaohongshu template");

    assert_eq!(row.0, "xiaohongshu");
    assert_eq!(row.1, "sample_ready");
    assert!(row.2.contains("login_state"));
    assert!(row.3.contains("/notes"));
}

#[tokio::test]
async fn sample_ready_xiaohongshu_heartbeat_uses_extract_text_and_round_robins() {
    let db_url = unique_db_url();
    let state = build_state_without_workers(&db_url).await;
    seed_persona_bundle(&state.db, "persona-rr", "store-rr", "xiaohongshu").await;

    let first = run_persona_heartbeat_tick(&state)
        .await
        .expect("first heartbeat tick");
    assert_eq!(first.scheduled_count, 1);
    let first_item = first.items.first().expect("first heartbeat item");
    assert_eq!(first_item.status, "scheduled");
    assert_eq!(
        first_item.target_url.as_deref(),
        Some("https://seller.xiaohongshu.com/dashboard")
    );
    let first_task_id = first_item.task_id.as_ref().expect("first task id");
    let first_kind: String = sqlx::query_scalar("SELECT kind FROM tasks WHERE id = ?")
        .bind(first_task_id)
        .fetch_one(&state.db)
        .await
        .expect("load first heartbeat task kind");
    assert_eq!(first_kind, "extract_text");

    sqlx::query(
        r#"UPDATE tasks
           SET status = 'succeeded', created_at = '1', queued_at = '1', started_at = '1', finished_at = '1'
           WHERE id = ?"#,
    )
    .bind(first_task_id)
    .execute(&state.db)
    .await
    .expect("age first heartbeat task");

    let second = run_persona_heartbeat_tick(&state)
        .await
        .expect("second heartbeat tick");
    assert_eq!(second.scheduled_count, 1);
    let second_item = second.items.first().expect("second heartbeat item");
    assert_eq!(
        second_item.target_url.as_deref(),
        Some("https://seller.xiaohongshu.com/notes")
    );
}

#[tokio::test]
async fn persona_health_snapshot_records_probe_summary_fields() {
    let db_url = unique_db_url();
    let (state, _app) = build_test_app(&db_url).await.expect("build app");
    seed_persona_bundle(
        &state.db,
        "persona-probe-snapshot",
        "store-probe-snapshot",
        "xiaohongshu",
    )
    .await;

    let event_json = json!({
        "probe_action": "extract_text",
        "probe_path": "/dashboard",
        "passed_checks": ["login_state", "identity", "region", "dashboard"],
        "failed_checks": [],
        "evidence_summary": "probe_action=extract_text probe_path=/dashboard passed=login_state,identity,region,dashboard failed=none"
    });
    append_continuity_event(
        &state,
        Some("persona-probe-snapshot"),
        Some("store-probe-snapshot"),
        Some("xiaohongshu"),
        None,
        None,
        "browser_action_succeeded",
        "info",
        Some(&event_json),
    )
    .await
    .expect("append browser action success");

    let snapshot_json: String = sqlx::query_scalar(
        r#"SELECT snapshot_json
           FROM persona_health_snapshots
           WHERE persona_id = 'persona-probe-snapshot'
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .fetch_one(&state.db)
    .await
    .expect("load persona health snapshot");
    let snapshot: Value = serde_json::from_str(&snapshot_json).expect("snapshot json");
    assert_eq!(
        snapshot.get("last_probe_action").and_then(Value::as_str),
        Some("extract_text")
    );
    assert_eq!(
        snapshot.get("last_probe_path").and_then(Value::as_str),
        Some("/dashboard")
    );
    assert!(
        snapshot
            .get("last_continuity_check_results")
            .and_then(|value| value.get("passed_checks"))
            .and_then(Value::as_array)
            .is_some()
    );
}

#[tokio::test]
async fn xiaohongshu_store_identity_markers_flow_into_probe_event_and_snapshot() {
    let db_url = unique_db_url();
    let runner: Arc<dyn TaskRunner> = Arc::new(FixedProbeRunner {
        final_url: "https://seller.xiaohongshu.com/dashboard".to_string(),
        title: "store-identity-marker 小红书商家后台".to_string(),
        text_preview: "store-identity-marker 创作中心 dashboard".to_string(),
        html_preview: "<main>store-identity-marker 创作中心 dashboard</main>".to_string(),
    });
    let (state, _app) = build_test_app_with_runner(&db_url, runner).await;
    seed_persona_bundle(
        &state.db,
        "persona-identity-marker",
        "store-identity-marker",
        "xiaohongshu",
    )
    .await;
    seed_active_proxy(
        &state.db,
        "proxy-xhs-identity-marker",
        "xhs-probe",
        "cn-shanghai",
        "CN",
    )
    .await;

    sqlx::query(
        r#"INSERT INTO store_platform_overrides (
               id, store_id, platform_id, identity_markers_json, status, created_at, updated_at
           ) VALUES (
               'override-identity-marker', 'store-identity-marker', 'xiaohongshu',
               '["store-identity-marker"]', 'active', '1', '1'
           )"#,
    )
    .execute(&state.db)
    .await
    .expect("insert store identity marker override");

    let heartbeat = run_persona_heartbeat_tick(&state)
        .await
        .expect("run heartbeat tick");
    assert_eq!(heartbeat.scheduled_count, 1);
    let heartbeat_item = heartbeat.items.first().expect("heartbeat item");
    let task_id = heartbeat_item.task_id.as_deref().expect("heartbeat task id");
    assert_eq!(
        heartbeat_item.target_url.as_deref(),
        Some("https://seller.xiaohongshu.com/dashboard")
    );

    let input_json_raw: String = sqlx::query_scalar("SELECT input_json FROM tasks WHERE id = ?")
        .bind(task_id)
        .fetch_one(&state.db)
        .await
        .expect("load heartbeat task input");
    let input_json: Value = serde_json::from_str(&input_json_raw).expect("parse task input json");
    assert_eq!(
        input_json
            .get("platform_template")
            .and_then(|value| value.get("identity_markers"))
            .and_then(Value::as_array)
            .and_then(|values| values.first())
            .and_then(Value::as_str),
        Some("store-identity-marker")
    );

    let task_result = wait_for_task_result_json(&state.db, task_id).await;
    assert_eq!(
        task_result.get("final_url").and_then(Value::as_str),
        Some("https://seller.xiaohongshu.com/dashboard"),
        "unexpected task result json: {task_result}"
    );
    assert_eq!(
        task_result.get("title").and_then(Value::as_str),
        Some("store-identity-marker 小红书商家后台"),
        "unexpected task result json: {task_result}"
    );
    let probe = task_result
        .get("continuity_check_result")
        .expect("continuity check result");
    assert_eq!(
        probe.get("matched_identity_marker").and_then(Value::as_str),
        Some("store-identity-marker"),
        "unexpected continuity probe result: {probe}"
    );
    assert!(
        probe.get("passed_checks")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("identity")))
    );
    assert!(
        probe.get("passed_checks")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("dashboard")))
    );
    assert!(
        probe.get("skipped_checks")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("notes")))
    );

    let success_event = wait_for_continuity_event_json(
        &state.db,
        "persona-identity-marker",
        "browser_action_succeeded",
    )
    .await;
    assert_eq!(
        success_event
            .get("matched_identity_marker")
            .and_then(Value::as_str),
        Some("store-identity-marker")
    );
    assert!(
        success_event
            .get("skipped_checks")
            .and_then(Value::as_array)
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("notes")))
    );

    let snapshot = wait_for_persona_health_snapshot_json(&state.db, "persona-identity-marker").await;
    assert_eq!(
        snapshot
            .get("last_continuity_check_results")
            .and_then(|value| value.get("matched_identity_marker"))
            .and_then(Value::as_str),
        Some("store-identity-marker")
    );
    assert!(
        snapshot
            .get("continuity_check_skipped_count_24h")
            .and_then(Value::as_i64)
            .is_some_and(|value| value >= 1)
    );
}
