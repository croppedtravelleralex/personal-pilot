use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::{
    behavior::site_key_from_url,
    db::{init::DbPool, models::DashboardOnboardingDraftRecord},
    network_identity::proxy_health::proxy_health_stale_after_seconds_from_env,
};

use super::{
    authorize_admin, build_gateway_stats_snapshot, error_response, GatewayState,
    GatewayStatsSnapshot,
};

const DEFAULT_TASK_TIMEOUT_SECONDS: i64 = 30;
const VALIDATION_WAIT_TIMEOUT_SECONDS: u64 = 45;
const VALIDATION_POLL_INTERVAL_MS: u64 = 300;

#[derive(Debug, Clone, Serialize)]
struct NamedResourceOption {
    id: String,
    name: String,
    version: i64,
    status: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyOption {
    id: String,
    provider: Option<String>,
    region: Option<String>,
    status: String,
    score: f64,
    proxy_health_score: Option<f64>,
    proxy_health_grade: Option<String>,
    proxy_health_checked_at: Option<String>,
    trust_score_total: Option<i64>,
    source_label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SitePolicyOption {
    id: String,
    version: i64,
    site_key: String,
    behavior_profile_id: String,
    status: String,
    page_archetype: Option<String>,
    action_kind: Option<String>,
    has_form_contract: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardTaskListItem {
    id: String,
    kind: String,
    task_display_name: String,
    task_kind_display: String,
    status: String,
    display_status: String,
    form_action_mode: Option<String>,
    form_action_status: Option<String>,
    retry_count: Option<i64>,
    failure_signal: Option<String>,
    browser_failure_signal: Option<String>,
    started_at: Option<String>,
    finished_at: Option<String>,
    site_key: Option<String>,
    title: Option<String>,
    final_url: Option<String>,
    proxy_id: Option<String>,
    proxy_provider: Option<String>,
    proxy_region: Option<String>,
    proxy_health_score: Option<f64>,
    proxy_health_grade: Option<String>,
    trust_score_total: Option<i64>,
    session_persisted: bool,
    summary_raw: String,
    summary_zh: String,
    summary_compact_zh: String,
    summary_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyHealthGradeBucket {
    grade: String,
    label: String,
    tone: String,
    count: i64,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyHealthScoreBandBucket {
    key: String,
    label: String,
    tone: String,
    count: i64,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyHealthSourceComparisonRow {
    source_label: String,
    avg_score: Option<f64>,
    active_count: i64,
    checked_count: i64,
    stale_count: i64,
    low_quality_count: i64,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyHealthReasonBucket {
    key: String,
    label: String,
    tone: String,
    count: i64,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyHealthLowQualityRow {
    proxy_id: String,
    provider: Option<String>,
    region: Option<String>,
    source_label: Option<String>,
    proxy_health_score: Option<f64>,
    proxy_health_grade: Option<String>,
    proxy_health_checked_at: Option<String>,
    trust_score_total: Option<i64>,
    reason: String,
    last_probe_latency_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
struct ProxyHealthOverview {
    total_active: i64,
    checked_count: i64,
    unchecked_count: i64,
    stale_count: i64,
    avg_score: Option<f64>,
    healthy_count: i64,
    warning_count: i64,
    grade_distribution: Vec<ProxyHealthGradeBucket>,
    score_band_distribution: Vec<ProxyHealthScoreBandBucket>,
    source_comparison_rows: Vec<ProxyHealthSourceComparisonRow>,
    low_quality_reason_buckets: Vec<ProxyHealthReasonBucket>,
    low_quality_rows: Vec<ProxyHealthLowQualityRow>,
}

#[derive(Debug, Clone, Default)]
struct ProxyHealthSourceAccumulator {
    score_sum: f64,
    scored_count: i64,
    active_count: i64,
    checked_count: i64,
    stale_count: i64,
    low_quality_count: i64,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardDistributionBucket {
    key: String,
    label: String,
    tone: String,
    count: i64,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardContinuityEvent {
    event_type: String,
    severity: String,
    task_id: Option<String>,
    site_key: Option<String>,
    occurred_at: Option<String>,
    detail_short: String,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardSiteValidationRollup {
    draft_id: String,
    site_key: String,
    login_url: String,
    status: String,
    display_status: String,
    site_policy_id: Option<String>,
    site_policy_version: Option<i64>,
    site_contract_present: bool,
    shadow_task_id: Option<String>,
    active_success_task_id: Option<String>,
    active_failure_task_id: Option<String>,
    continuity_task_id: Option<String>,
    form_action_status: Option<String>,
    retry_count: Option<i64>,
    failure_signal: Option<String>,
    success_ready_selector_seen: bool,
    post_login_actions_executed: bool,
    session_persisted: bool,
    summary_raw: String,
    summary_zh: String,
    summary_compact_zh: String,
    summary_kind: String,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardBootstrapResponse {
    status: Option<Value>,
    status_error: Option<String>,
    runtime_mode: String,
    gateway_stats_snapshot: GatewayStatsSnapshot,
    ui_model: Value,
    behavior_profiles: Vec<NamedResourceOption>,
    identity_profiles: Vec<NamedResourceOption>,
    session_profiles: Vec<NamedResourceOption>,
    fingerprint_profiles: Vec<NamedResourceOption>,
    proxies: Vec<ProxyOption>,
    site_policies: Vec<SitePolicyOption>,
    overview_tasks: Vec<DashboardTaskListItem>,
    continuity_events: Vec<DashboardContinuityEvent>,
    site_validation_rollups: Vec<DashboardSiteValidationRollup>,
    drafts: Vec<DashboardOnboardingDraftResponse>,
}

#[derive(Debug, Clone, Serialize)]
struct PublicDashboardBootstrapResponse {
    status: Option<Value>,
    status_error: Option<String>,
    runtime_mode: String,
    gateway_stats_snapshot: GatewayStatsSnapshot,
    ui_model: Value,
    proxies: Vec<ProxyOption>,
    overview_tasks: Vec<DashboardTaskListItem>,
    continuity_events: Vec<DashboardContinuityEvent>,
    site_validation_rollups: Vec<DashboardSiteValidationRollup>,
    readonly: bool,
    public_preview: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DashboardOnboardingDraftResponse {
    id: String,
    share_token: String,
    share_url: String,
    share_expires_at: String,
    status: String,
    login_url: String,
    site_key: String,
    success_hint: Option<String>,
    behavior_profile_id: Option<String>,
    identity_profile_id: Option<String>,
    session_profile_id: Option<String>,
    fingerprint_profile_id: Option<String>,
    proxy_id: Option<String>,
    credential_mode: String,
    credential_ref: Option<String>,
    inferred_contract_json: Option<Value>,
    final_contract_json: Option<Value>,
    site_policy_id: Option<String>,
    site_policy_version: Option<i64>,
    shadow_task_id: Option<String>,
    active_success_task_id: Option<String>,
    active_failure_task_id: Option<String>,
    continuity_task_id: Option<String>,
    evidence_summary_json: Option<Value>,
    site_contract_present: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DraftListQuery {
    share_token: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct CreateOnboardingDraftRequest {
    login_url: String,
    success_hint: Option<String>,
    behavior_profile_id: Option<String>,
    identity_profile_id: Option<String>,
    session_profile_id: Option<String>,
    fingerprint_profile_id: Option<String>,
    proxy_id: Option<String>,
    credential_mode: Option<String>,
    credential_ref: Option<String>,
    final_contract_json: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct PatchOnboardingDraftRequest {
    login_url: Option<String>,
    success_hint: Option<String>,
    behavior_profile_id: Option<String>,
    identity_profile_id: Option<String>,
    session_profile_id: Option<String>,
    fingerprint_profile_id: Option<String>,
    proxy_id: Option<String>,
    credential_mode: Option<String>,
    credential_ref: Option<String>,
    final_contract_json: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct RunValidationRequest {
    scenario: Option<String>,
    inline_credentials: Option<InlineCredentials>,
}

#[derive(Debug, Clone, Deserialize)]
struct InlineCredentials {
    username: Option<String>,
    password: Option<String>,
}

pub fn build_admin_control_router() -> Router<GatewayState> {
    Router::new()
        .route("/admin/control/bootstrap", get(get_bootstrap))
        .route("/public/dashboard/bootstrap", get(get_public_bootstrap))
        .route(
            "/admin/control/onboarding-drafts",
            get(list_onboarding_drafts).post(create_onboarding_draft),
        )
        .route(
            "/admin/control/onboarding-drafts/:id",
            get(get_onboarding_draft).patch(patch_onboarding_draft),
        )
        .route(
            "/admin/control/onboarding-drafts/:id/discover",
            post(discover_onboarding_draft),
        )
        .route(
            "/admin/control/onboarding-drafts/:id/publish",
            post(publish_onboarding_draft),
        )
        .route(
            "/admin/control/onboarding-drafts/:id/run-validation",
            post(run_validation),
        )
}

async fn get_bootstrap(State(state): State<GatewayState>, headers: HeaderMap) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }

    match build_dashboard_bootstrap_response(&state).await {
        Ok(payload) => Json(payload).into_response(),
        Err(resp) => resp,
    }
}

async fn get_public_bootstrap(State(state): State<GatewayState>) -> Response {
    match build_public_dashboard_bootstrap_json(&state).await {
        Ok(payload) => Json(payload).into_response(),
        Err(resp) => resp,
    }
}

pub(crate) async fn build_public_dashboard_bootstrap_json(
    state: &GatewayState,
) -> Result<Value, Response> {
    let payload = build_dashboard_bootstrap_response(state).await?;
    let mut ui_model = payload.ui_model.clone();
    mark_ui_model_readonly(&mut ui_model);
    serde_json::to_value(PublicDashboardBootstrapResponse {
        status: payload.status,
        status_error: payload.status_error,
        runtime_mode: payload.runtime_mode,
        gateway_stats_snapshot: payload.gateway_stats_snapshot,
        ui_model,
        proxies: payload.proxies,
        overview_tasks: payload.overview_tasks,
        continuity_events: payload.continuity_events,
        site_validation_rollups: payload.site_validation_rollups,
        readonly: true,
        public_preview: true,
    })
    .map_err(|err| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "dashboard_preview_unavailable",
            &format!("failed to serialize public dashboard bootstrap: {err}"),
        )
    })
}

async fn build_dashboard_bootstrap_response(
    state: &GatewayState,
) -> Result<DashboardBootstrapResponse, Response> {
    let status_result = control_request_value(&state, Method::GET, "/status", None).await;
    let behavior_profiles = match load_active_named_resources(&state.db, "behavior_profiles").await
    {
        Ok(items) => items,
        Err(err) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load behavior profiles: {err}"),
            ))
        }
    };
    let identity_profiles = match load_active_named_resources(&state.db, "identity_profiles").await
    {
        Ok(items) => items,
        Err(err) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load identity profiles: {err}"),
            ))
        }
    };
    let session_profiles = match load_active_named_resources(&state.db, "session_profiles").await {
        Ok(items) => items,
        Err(err) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load session profiles: {err}"),
            ))
        }
    };
    let fingerprint_profiles =
        match load_active_named_resources(&state.db, "fingerprint_profiles").await {
            Ok(items) => items,
            Err(err) => {
                return Err(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "draft_query_failed",
                    &format!("failed to load fingerprint profiles: {err}"),
                ))
            }
        };
    let proxies = match load_active_proxies(&state.db).await {
        Ok(items) => items,
        Err(err) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load proxies: {err}"),
            ))
        }
    };
    let proxy_health_overview = match load_proxy_health_overview(&state.db).await {
        Ok(value) => value,
        Err(err) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load proxy health overview: {err}"),
            ))
        }
    };
    let site_policies = match load_active_auth_site_policies(&state.db).await {
        Ok(items) => items,
        Err(err) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load site policies: {err}"),
            ))
        }
    };
    let drafts = match list_draft_records(&state.db, 50, 0).await {
        Ok(items) => items
            .into_iter()
            .map(draft_record_to_response)
            .collect::<Vec<_>>(),
        Err(err) => {
            return Err(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load onboarding drafts: {err}"),
            ))
        }
    };

    let (status, status_error) = match status_result {
        Ok(value) => (Some(value), None),
        Err(message) => (None, Some(message)),
    };
    let gateway_stats_snapshot = build_gateway_stats_snapshot(&state);
    let overview_tasks = build_dashboard_overview_tasks(status.as_ref(), &proxies);
    let continuity_events = build_dashboard_continuity_events(status.as_ref(), &drafts);
    let site_validation_rollups = drafts
        .iter()
        .map(build_site_validation_rollup)
        .collect::<Vec<_>>();
    let ui_model = build_dashboard_ui_model(
        runtime_mode_label(&state.config.runtime_mode),
        status.as_ref(),
        status_error.as_deref(),
        &gateway_stats_snapshot,
        &overview_tasks,
        &continuity_events,
        &site_validation_rollups,
        &drafts,
        &site_policies,
        &proxy_health_overview,
    );

    Ok(DashboardBootstrapResponse {
        status,
        status_error,
        runtime_mode: state.config.runtime_mode.clone(),
        gateway_stats_snapshot,
        ui_model,
        behavior_profiles,
        identity_profiles,
        session_profiles,
        fingerprint_profiles,
        proxies,
        site_policies,
        overview_tasks,
        continuity_events,
        site_validation_rollups,
        drafts,
    })
}

async fn list_onboarding_drafts(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Query(query): Query<DraftListQuery>,
) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }

    if let Some(share_token) = normalize_optional_text(query.share_token) {
        match load_draft_by_share_token(&state.db, &share_token).await {
            Ok(Some(record)) => {
                if share_token_expired(&record.share_expires_at) {
                    return error_response(
                        StatusCode::GONE,
                        "share_token_expired",
                        "draft share token has expired",
                    );
                }
                return Json(vec![draft_record_to_response(record)]).into_response();
            }
            Ok(None) => {
                return error_response(
                    StatusCode::NOT_FOUND,
                    "draft_not_found",
                    "dashboard onboarding draft not found",
                )
            }
            Err(err) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "draft_query_failed",
                    &format!("failed to load onboarding draft: {err}"),
                )
            }
        }
    }

    match list_draft_records(
        &state.db,
        sanitize_limit(query.limit, 50, 200),
        sanitize_offset(query.offset),
    )
    .await
    {
        Ok(records) => Json(
            records
                .into_iter()
                .map(draft_record_to_response)
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "draft_query_failed",
            &format!("failed to list onboarding drafts: {err}"),
        ),
    }
}

async fn create_onboarding_draft(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(payload): Json<CreateOnboardingDraftRequest>,
) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }

    let login_url = match normalize_login_url(&payload.login_url) {
        Ok(value) => value,
        Err(resp) => return resp,
    };
    let site_key = match site_key_from_url(Some(login_url.as_str())) {
        Some(value) => value,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "invalid_login_url",
                "login_url must include a valid host",
            )
        }
    };
    let final_contract_json = match normalize_contract_input(payload.final_contract_json) {
        Ok(value) => value,
        Err(resp) => return resp,
    };
    let credential_mode = match normalize_credential_mode(payload.credential_mode) {
        Ok(value) => value,
        Err(resp) => return resp,
    };
    let credential_ref =
        match normalize_credential_ref(credential_mode.as_str(), payload.credential_ref) {
            Ok(value) => value,
            Err(resp) => return resp,
        };

    let created_at = now_ts_string();
    let mut record = DashboardOnboardingDraftRecord {
        id: format!("draft-{}", Uuid::new_v4().simple()),
        share_token: Uuid::new_v4().simple().to_string(),
        share_expires_at: future_ts_string(state.config.draft_share_ttl_seconds),
        status: "draft".to_string(),
        login_url,
        site_key,
        success_hint: normalize_optional_text(payload.success_hint),
        behavior_profile_id: payload.behavior_profile_id.or(unique_active_id(
            &state.db,
            "behavior_profiles",
        )
        .await
        .ok()
        .flatten()),
        identity_profile_id: payload.identity_profile_id.or(unique_active_id(
            &state.db,
            "identity_profiles",
        )
        .await
        .ok()
        .flatten()),
        session_profile_id: payload.session_profile_id.or(unique_active_id(
            &state.db,
            "session_profiles",
        )
        .await
        .ok()
        .flatten()),
        fingerprint_profile_id: payload.fingerprint_profile_id.or(unique_active_id(
            &state.db,
            "fingerprint_profiles",
        )
        .await
        .ok()
        .flatten()),
        proxy_id: payload
            .proxy_id
            .or(unique_active_proxy_id(&state.db).await.ok().flatten()),
        credential_mode,
        credential_ref,
        inferred_contract_json: None,
        final_contract_json: final_contract_json.as_ref().map(Value::to_string),
        site_policy_id: None,
        site_policy_version: None,
        shadow_task_id: None,
        active_success_task_id: None,
        active_failure_task_id: None,
        continuity_task_id: None,
        evidence_summary_json: None,
        created_at: created_at.clone(),
        updated_at: created_at,
    };

    if let Err(err) = insert_draft_record(&state.db, &record).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "draft_create_failed",
            &format!("failed to create onboarding draft: {err}"),
        );
    }

    refresh_draft_share_expiry(&mut record, state.config.draft_share_ttl_seconds);
    if let Err(err) = update_draft_record(&state.db, &record).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "draft_create_failed",
            &format!("failed to finalize onboarding draft: {err}"),
        );
    }

    (StatusCode::CREATED, Json(draft_record_to_response(record))).into_response()
}

async fn get_onboarding_draft(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }

    match load_draft_by_id(&state.db, &draft_id).await {
        Ok(Some(record)) => Json(draft_record_to_response(record)).into_response(),
        Ok(None) => error_response(
            StatusCode::NOT_FOUND,
            "draft_not_found",
            "dashboard onboarding draft not found",
        ),
        Err(err) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "draft_query_failed",
            &format!("failed to load onboarding draft: {err}"),
        ),
    }
}

async fn patch_onboarding_draft(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
    Json(payload): Json<PatchOnboardingDraftRequest>,
) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }

    let mut record = match load_draft_by_id(&state.db, &draft_id).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "draft_not_found",
                "dashboard onboarding draft not found",
            )
        }
        Err(err) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load onboarding draft: {err}"),
            )
        }
    };

    if let Some(login_url) = payload.login_url {
        let normalized = match normalize_login_url(&login_url) {
            Ok(value) => value,
            Err(resp) => return resp,
        };
        let site_key = match site_key_from_url(Some(normalized.as_str())) {
            Some(value) => value,
            None => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "invalid_login_url",
                    "login_url must include a valid host",
                )
            }
        };
        record.login_url = normalized;
        record.site_key = site_key;
        record.status = "draft".to_string();
    }
    if let Some(success_hint) = payload.success_hint {
        record.success_hint = normalize_optional_text(Some(success_hint));
    }
    if let Some(behavior_profile_id) = payload.behavior_profile_id {
        record.behavior_profile_id = normalize_optional_text(Some(behavior_profile_id));
        record.status = "draft".to_string();
    }
    if let Some(identity_profile_id) = payload.identity_profile_id {
        record.identity_profile_id = normalize_optional_text(Some(identity_profile_id));
    }
    if let Some(session_profile_id) = payload.session_profile_id {
        record.session_profile_id = normalize_optional_text(Some(session_profile_id));
    }
    if let Some(fingerprint_profile_id) = payload.fingerprint_profile_id {
        record.fingerprint_profile_id = normalize_optional_text(Some(fingerprint_profile_id));
    }
    if let Some(proxy_id) = payload.proxy_id {
        record.proxy_id = normalize_optional_text(Some(proxy_id));
    }
    if let Some(credential_mode) = payload.credential_mode {
        let normalized = match normalize_credential_mode(Some(credential_mode)) {
            Ok(value) => value,
            Err(resp) => return resp,
        };
        record.credential_mode = normalized;
        if record.credential_mode == "inline_once" {
            record.credential_ref = None;
        }
    }
    if let Some(credential_ref) = payload.credential_ref {
        let normalized =
            match normalize_credential_ref(record.credential_mode.as_str(), Some(credential_ref)) {
                Ok(value) => value,
                Err(resp) => return resp,
            };
        record.credential_ref = normalized;
    }
    if let Some(final_contract_json) = payload.final_contract_json {
        if final_contract_json.is_null() {
            record.final_contract_json = None;
        } else {
            let normalized = match normalize_contract_object(final_contract_json) {
                Ok(value) => value,
                Err(resp) => return resp,
            };
            record.final_contract_json = Some(normalized.to_string());
        }
        record.status = "draft".to_string();
    }

    refresh_draft_share_expiry(&mut record, state.config.draft_share_ttl_seconds);
    record.updated_at = now_ts_string();
    if let Err(err) = update_draft_record(&state.db, &record).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "draft_update_failed",
            &format!("failed to patch onboarding draft: {err}"),
        );
    }

    Json(draft_record_to_response(record)).into_response()
}

async fn discover_onboarding_draft(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }

    let mut record = match load_draft_by_id(&state.db, &draft_id).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "draft_not_found",
                "dashboard onboarding draft not found",
            )
        }
        Err(err) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load onboarding draft: {err}"),
            )
        }
    };

    let html = match fetch_login_page_html(&state, &record.login_url).await {
        Ok(value) => value,
        Err(resp) => return resp,
    };
    let inferred_contract = infer_form_contract_from_html(&html, record.success_hint.as_deref());
    record.inferred_contract_json = Some(inferred_contract.to_string());
    if record.final_contract_json.is_none() {
        record.final_contract_json = Some(inferred_contract.to_string());
    }
    record.status = "discovered".to_string();
    refresh_draft_share_expiry(&mut record, state.config.draft_share_ttl_seconds);
    record.updated_at = now_ts_string();
    if let Err(err) = update_draft_record(&state.db, &record).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "draft_update_failed",
            &format!("failed to update inferred contract: {err}"),
        );
    }

    Json(draft_record_to_response(record)).into_response()
}

async fn publish_onboarding_draft(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }

    let mut record = match load_draft_by_id(&state.db, &draft_id).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "draft_not_found",
                "dashboard onboarding draft not found",
            )
        }
        Err(err) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load onboarding draft: {err}"),
            )
        }
    };

    if let Err(resp) = ensure_site_policy_published(&state, &mut record).await {
        return resp;
    }
    refresh_draft_share_expiry(&mut record, state.config.draft_share_ttl_seconds);
    record.updated_at = now_ts_string();
    if let Err(err) = update_draft_record(&state.db, &record).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "draft_update_failed",
            &format!("failed to persist published draft: {err}"),
        );
    }

    Json(draft_record_to_response(record)).into_response()
}

async fn run_validation(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Path(draft_id): Path<String>,
    payload: Option<Json<RunValidationRequest>>,
) -> Response {
    if let Err(resp) = authorize_admin(&state, &headers) {
        return resp;
    }

    let request = payload.map(|item| item.0).unwrap_or_default();
    let scenario = match normalize_validation_scenario(request.scenario) {
        Ok(value) => value,
        Err(resp) => return resp,
    };

    let mut record = match load_draft_by_id(&state.db, &draft_id).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return error_response(
                StatusCode::NOT_FOUND,
                "draft_not_found",
                "dashboard onboarding draft not found",
            )
        }
        Err(err) => {
            return error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "draft_query_failed",
                &format!("failed to load onboarding draft: {err}"),
            )
        }
    };

    if let Err(resp) = ensure_site_policy_published(&state, &mut record).await {
        return resp;
    }

    let contract = match parsed_contract_object(record.final_contract_json.as_deref()) {
        Some(value) => value,
        None => {
            return error_response(
                StatusCode::BAD_REQUEST,
                "missing_contract",
                "final_contract_json is required before validation",
            )
        }
    };

    let shadow_payload = match build_task_payload(
        &record,
        &contract,
        "shadow",
        scenario.as_str(),
        request.inline_credentials.as_ref(),
        None,
    ) {
        Ok(value) => value,
        Err(resp) => return resp,
    };
    let shadow_task = match submit_task_and_wait(&state, shadow_payload).await {
        Ok(value) => value,
        Err(resp) => return resp,
    };
    record.shadow_task_id = json_string_field(&shadow_task, "id");

    let active_payload = match build_task_payload(
        &record,
        &contract,
        "active",
        scenario.as_str(),
        request.inline_credentials.as_ref(),
        None,
    ) {
        Ok(value) => value,
        Err(resp) => return resp,
    };
    let active_task = match submit_task_and_wait(&state, active_payload).await {
        Ok(value) => value,
        Err(resp) => return resp,
    };

    match scenario.as_str() {
        "business_failure" => {
            record.active_failure_task_id = json_string_field(&active_task, "id");
        }
        _ => {
            record.active_success_task_id = json_string_field(&active_task, "id");
        }
    }

    let mut continuity_task: Option<Value> = None;
    if scenario == "default" && is_successful_active_auth(&active_task) {
        let continuity_payload = build_continuity_task_payload(&record, &active_task);
        let task = match submit_task_and_wait(&state, continuity_payload).await {
            Ok(value) => value,
            Err(resp) => return resp,
        };
        record.continuity_task_id = json_string_field(&task, "id");
        continuity_task = Some(task);
    }

    let status_snapshot = match control_request_value(&state, Method::GET, "/status", None).await {
        Ok(value) => value,
        Err(message) => json!({ "auth_metrics_error": message }),
    };
    record.evidence_summary_json = Some(
        merge_evidence_summary(
            record.evidence_summary_json.as_deref(),
            &record,
            scenario.as_str(),
            &shadow_task,
            &active_task,
            continuity_task.as_ref(),
            &status_snapshot,
        )
        .to_string(),
    );
    record.status = if scenario == "default" && is_successful_active_auth(&active_task) {
        "validated".to_string()
    } else if scenario == "default" {
        "validation_failed".to_string()
    } else {
        "published".to_string()
    };
    refresh_draft_share_expiry(&mut record, state.config.draft_share_ttl_seconds);
    record.updated_at = now_ts_string();
    if let Err(err) = update_draft_record(&state.db, &record).await {
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "draft_update_failed",
            &format!("failed to persist validation evidence: {err}"),
        );
    }

    Json(draft_record_to_response(record)).into_response()
}

async fn ensure_site_policy_published(
    state: &GatewayState,
    record: &mut DashboardOnboardingDraftRecord,
) -> Result<(), Response> {
    let Some(behavior_profile_id) = normalize_optional_text(record.behavior_profile_id.clone())
    else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "missing_behavior_profile",
            "behavior_profile_id is required before publish",
        ));
    };
    let Some(mut contract) = parsed_contract_object(record.final_contract_json.as_deref()) else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "missing_contract",
            "final_contract_json is required before publish",
        ));
    };
    if contract
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("auth")
        != "auth"
    {
        contract.insert("mode".to_string(), json!("auth"));
    }
    let password_selector = contract_field_selector(&contract, "password");
    let submit_selector = contract_submit_selector(&contract);
    let ready_selector = contract
        .get("success")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("ready_selector"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    if password_selector.is_none() || submit_selector.is_none() || ready_selector.is_none() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "missing_publish_contract",
            "active auth publish requires explicit password selector, submit selector, and success.ready_selector",
        ));
    }

    let existing_policy = load_matching_site_policy(&state.db, record)
        .await
        .map_err(|err| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "site_policy_query_failed",
                &format!("failed to load matching site policy: {err}"),
            )
        })?;
    let policy_id = existing_policy
        .as_ref()
        .and_then(|item| item.0.clone())
        .or_else(|| record.site_policy_id.clone())
        .unwrap_or_else(|| {
            format!(
                "dashboard-site-policy-{}",
                slugify_site_key(&record.site_key)
            )
        });
    let payload = if existing_policy.is_some() {
        json!({
            "site_key": record.site_key,
            "page_archetype": "auth",
            "action_kind": "open_page",
            "behavior_profile_id": behavior_profile_id,
            "priority": 100,
            "required": false,
            "override_json": { "form_contract": contract },
            "status": "active"
        })
    } else {
        json!({
            "id": policy_id,
            "site_key": record.site_key,
            "page_archetype": "auth",
            "action_kind": "open_page",
            "behavior_profile_id": behavior_profile_id,
            "priority": 100,
            "required": false,
            "override_json": { "form_contract": contract },
            "status": "active"
        })
    };
    let path = if existing_policy.is_some() {
        format!("/site-behavior-policies/{}", policy_id)
    } else {
        "/site-behavior-policies".to_string()
    };
    let method = if existing_policy.is_some() {
        Method::PATCH
    } else {
        Method::POST
    };
    let response = control_request_value(state, method, &path, Some(payload))
        .await
        .map_err(|message| {
            error_response(
                StatusCode::BAD_GATEWAY,
                "site_policy_publish_failed",
                &message,
            )
        })?;
    record.site_policy_id = json_string_field(&response, "id");
    record.site_policy_version = json_i64_field(&response, "version");
    record.final_contract_json = Some(Value::Object(contract).to_string());
    record.status = "published".to_string();
    Ok(())
}

async fn submit_task_and_wait(state: &GatewayState, payload: Value) -> Result<Value, Response> {
    let created = control_request_value(state, Method::POST, "/tasks", Some(payload))
        .await
        .map_err(|message| {
            error_response(StatusCode::BAD_GATEWAY, "task_submit_failed", &message)
        })?;
    let Some(task_id) = json_string_field(&created, "id") else {
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            "task_submit_failed",
            "control-plane task response is missing id",
        ));
    };
    wait_for_task_terminal(state, &task_id).await
}

async fn wait_for_task_terminal(state: &GatewayState, task_id: &str) -> Result<Value, Response> {
    let deadline = std::time::Instant::now() + Duration::from_secs(VALIDATION_WAIT_TIMEOUT_SECONDS);
    loop {
        let task = control_request_value(state, Method::GET, &format!("/tasks/{task_id}"), None)
            .await
            .map_err(|message| {
                error_response(StatusCode::BAD_GATEWAY, "task_query_failed", &message)
            })?;
        let status = json_string_field(&task, "status").unwrap_or_else(|| "unknown".to_string());
        if is_terminal_task_status(status.as_str()) {
            return Ok(task);
        }
        if std::time::Instant::now() >= deadline {
            return Err(error_response(
                StatusCode::GATEWAY_TIMEOUT,
                "task_wait_timeout",
                "timed out waiting for validation task to finish",
            ));
        }
        tokio::time::sleep(Duration::from_millis(VALIDATION_POLL_INTERVAL_MS)).await;
    }
}

fn build_task_payload(
    record: &DashboardOnboardingDraftRecord,
    contract: &Map<String, Value>,
    mode: &str,
    _scenario: &str,
    inline_credentials: Option<&InlineCredentials>,
    override_url: Option<&str>,
) -> Result<Value, Response> {
    let form_input = build_form_input(
        record,
        contract,
        record.credential_mode.as_str(),
        inline_credentials,
    )?;
    Ok(json!({
        "kind": "open_page",
        "url": override_url.unwrap_or(record.login_url.as_str()),
        "timeout_seconds": DEFAULT_TASK_TIMEOUT_SECONDS,
        "execution_intent": {
            "identity_profile_id": record.identity_profile_id,
            "fingerprint_profile_id": record.fingerprint_profile_id,
            "behavior_profile_id": record.behavior_profile_id,
            "session_profile_id": record.session_profile_id,
            "proxy_id": record.proxy_id
        },
        "behavior_policy_json": {
            "mode": mode,
            "page_archetype": "auth",
            "allow_site_overrides": true
        },
        "form_input": form_input
    }))
}

fn build_continuity_task_payload(
    record: &DashboardOnboardingDraftRecord,
    active_task: &Value,
) -> Value {
    json!({
        "kind": "open_page",
        "url": json_string_field(active_task, "final_url").unwrap_or_else(|| record.login_url.clone()),
        "timeout_seconds": DEFAULT_TASK_TIMEOUT_SECONDS,
        "execution_intent": {
            "identity_profile_id": record.identity_profile_id,
            "fingerprint_profile_id": record.fingerprint_profile_id,
            "behavior_profile_id": record.behavior_profile_id,
            "session_profile_id": record.session_profile_id,
            "proxy_id": record.proxy_id
        },
        "behavior_policy_json": {
            "mode": "active",
            "page_archetype": "dashboard",
            "allow_site_overrides": true
        }
    })
}

fn build_form_input(
    record: &DashboardOnboardingDraftRecord,
    contract: &Map<String, Value>,
    credential_mode: &str,
    inline_credentials: Option<&InlineCredentials>,
) -> Result<Value, Response> {
    let username_selector = contract_field_selector(contract, "username");
    let password_selector = contract_field_selector(contract, "password");
    let submit_selector = contract_submit_selector(contract);
    let success_obj = contract
        .get("success")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut fields = Vec::new();
    fields.push(build_auth_field_payload(
        "username",
        "username",
        username_selector,
        credential_mode,
        inline_credentials.and_then(|item| item.username.clone()),
        false,
    )?);
    fields.push(build_auth_field_payload(
        "password",
        "password",
        password_selector,
        credential_mode,
        inline_credentials.and_then(|item| item.password.clone()),
        true,
    )?);

    let mut form_input = json!({
        "mode": contract.get("mode").and_then(Value::as_str).unwrap_or("auth"),
        "form_selector": contract.get("primary_form_selector").and_then(Value::as_str),
        "fields": fields,
        "submit": {
            "selector": submit_selector,
            "trigger": "click"
        },
        "success": {
            "ready_selector": success_obj.get("ready_selector").and_then(Value::as_str),
            "url_patterns": success_obj.get("url_patterns").cloned().unwrap_or_else(|| json!([])),
            "title_contains": success_obj.get("title_contains").cloned().unwrap_or_else(|| json!([]))
        }
    });
    if let Some(obj) = form_input.as_object_mut() {
        if credential_mode == "alias" {
            let Some(secret_bundle_ref) = normalize_optional_text(record.credential_ref.clone())
            else {
                return Err(error_response(
                    StatusCode::BAD_REQUEST,
                    "missing_credential_ref",
                    "identity alias credential_ref is required for alias validation",
                ));
            };
            obj.insert("secret_bundle_ref".to_string(), json!(secret_bundle_ref));
        }
    }
    Ok(form_input)
}

fn build_auth_field_payload(
    key: &str,
    role: &str,
    selector: Option<String>,
    credential_mode: &str,
    inline_value: Option<String>,
    sensitive: bool,
) -> Result<Value, Response> {
    let mut field = json!({
        "key": key,
        "role": role,
        "selector": selector,
        "required": true,
        "sensitive": sensitive
    });
    let Some(obj) = field.as_object_mut() else {
        return Ok(field);
    };
    if credential_mode == "alias" {
        obj.insert("bundle_key".to_string(), json!(key));
    } else {
        let value = normalize_optional_text(inline_value).ok_or_else(|| {
            error_response(
                StatusCode::BAD_REQUEST,
                "missing_inline_credentials",
                "inline_once validation requires inline username and password values",
            )
        })?;
        obj.insert("value".to_string(), json!(value));
    }
    Ok(field)
}

fn merge_evidence_summary(
    raw_existing: Option<&str>,
    record: &DashboardOnboardingDraftRecord,
    scenario: &str,
    shadow_task: &Value,
    active_task: &Value,
    continuity_task: Option<&Value>,
    status_snapshot: &Value,
) -> Value {
    let mut summary = raw_existing
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    summary.insert("site_key".to_string(), json!(record.site_key));
    summary.insert("site_policy_id".to_string(), json!(record.site_policy_id));
    summary.insert(
        "site_policy_version".to_string(),
        json!(record.site_policy_version),
    );
    summary.insert(
        "site_contract_present".to_string(),
        json!(record.final_contract_json.is_some()),
    );
    summary.insert(
        "behavior_profile_id".to_string(),
        json!(record.behavior_profile_id),
    );
    summary.insert(
        "identity_profile_id".to_string(),
        json!(record.identity_profile_id),
    );
    summary.insert(
        "session_profile_id".to_string(),
        json!(record.session_profile_id),
    );
    summary.insert(
        "fingerprint_profile_id".to_string(),
        json!(record.fingerprint_profile_id),
    );
    summary.insert("proxy_id".to_string(), json!(record.proxy_id));
    summary.insert("shadow".to_string(), task_evidence(shadow_task));
    match scenario {
        "business_failure" => {
            summary.insert("active_failure".to_string(), task_evidence(active_task));
        }
        "retry_observation" => {
            summary.insert("retry_observation".to_string(), task_evidence(active_task));
        }
        _ => {
            summary.insert("active_success".to_string(), task_evidence(active_task));
        }
    }
    if let Some(task) = continuity_task {
        summary.insert("continuity".to_string(), task_evidence(task));
    }
    summary.insert(
        "auth_metrics_snapshot".to_string(),
        status_snapshot
            .get("auth_metrics")
            .cloned()
            .unwrap_or_else(|| Value::Null),
    );
    summary.insert("updated_at".to_string(), json!(now_ts_string()));
    Value::Object(summary)
}

#[derive(Debug, Clone)]
struct StandardizedTaskSummary {
    summary_raw: String,
    summary_zh: String,
    summary_compact_zh: String,
    summary_kind: String,
    task_kind_display: String,
}

fn build_dashboard_ui_model(
    runtime_mode_label: &str,
    status: Option<&Value>,
    status_error: Option<&str>,
    gateway_stats_snapshot: &GatewayStatsSnapshot,
    overview_tasks: &[DashboardTaskListItem],
    continuity_events: &[DashboardContinuityEvent],
    site_validation_rollups: &[DashboardSiteValidationRollup],
    drafts: &[DashboardOnboardingDraftResponse],
    site_policies: &[SitePolicyOption],
    proxy_health_overview: &ProxyHealthOverview,
) -> Value {
    let counts = status
        .and_then(|value| value.get("counts"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let worker = status
        .and_then(|value| value.get("worker"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let verify = status
        .and_then(|value| value.get("verify_metrics"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let proxy_pool = status
        .and_then(|value| value.get("proxy_pool_status"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let session_metrics = status
        .and_then(|value| value.get("identity_session_metrics"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let auth_metrics = status
        .and_then(|value| value.get("auth_metrics"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let proxy_sources = status
        .and_then(|value| value.get("proxy_harvest_metrics"))
        .and_then(|value| value.get("source_summaries"))
        .and_then(Value::as_array)
        .map(|items| normalize_proxy_source_rows(items.as_slice()))
        .unwrap_or_default();

    let counts_total = counts.get("total").and_then(Value::as_i64).unwrap_or(0);
    let counts_succeeded = counts.get("succeeded").and_then(Value::as_i64).unwrap_or(0);
    let counts_failed = counts.get("failed").and_then(Value::as_i64).unwrap_or(0);
    let counts_timed_out = counts.get("timed_out").and_then(Value::as_i64).unwrap_or(0);
    let counts_cancelled = counts.get("cancelled").and_then(Value::as_i64).unwrap_or(0);
    let counts_running = counts.get("running").and_then(Value::as_i64).unwrap_or(0);
    let counts_queued = counts.get("queued").and_then(Value::as_i64).unwrap_or(0);
    let success_rate = if counts_total > 0 {
        (counts_succeeded as f64 / counts_total as f64) * 100.0
    } else {
        0.0
    };

    let reused_sessions = session_metrics
        .get("reused_sessions")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let created_sessions = session_metrics
        .get("created_sessions")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let reuse_denominator = reused_sessions + created_sessions;
    let reuse_rate = if reuse_denominator > 0 {
        (reused_sessions as f64 / reuse_denominator as f64) * 100.0
    } else {
        0.0
    };

    let alert_rows = overview_tasks
        .iter()
        .filter(|task| {
            matches!(
                task.display_status.as_str(),
                "failed" | "timed_out" | "blocked"
            )
        })
        .take(8)
        .cloned()
        .collect::<Vec<_>>();
    let blocked_count = overview_tasks
        .iter()
        .filter(|task| task.display_status == "blocked")
        .count() as i64
        + site_validation_rollups
            .iter()
            .filter(|rollup| rollup.display_status == "blocked")
            .count() as i64;
    let warning_continuity_count = continuity_events
        .iter()
        .filter(|event| event.severity == "warning")
        .count() as i64;

    let primary_cards = vec![
        json!({
            "id": "task_health",
            "title": "Task Health",
            "subtitle": "Task Health",
            "value": counts_total,
            "value_display": counts_total.to_string(),
            "tone": "info",
            "lines": [
                { "label": "Success Rate", "value": format!("{success_rate:.1}%") },
                { "label": "Running", "value": counts_running.to_string() },
                { "label": "Queued", "value": counts_queued.to_string() }
            ]
        }),
        json!({
            "id": "recent_alerts",
            "title": "Recent Alerts",
            "subtitle": "Recent Alerts",
            "value": counts_failed + counts_timed_out + blocked_count,
            "value_display": (counts_failed + counts_timed_out + blocked_count).to_string(),
            "tone": if counts_failed + counts_timed_out + blocked_count > 0 { "danger" } else { "ok" },
            "lines": [
                { "label": "Failed", "value": counts_failed.to_string() },
                { "label": "Timed Out", "value": counts_timed_out.to_string() },
                { "label": "Blocked", "value": blocked_count.to_string() }
            ]
        }),
        json!({
            "id": "proxy_health",
            "title": "Proxy Health",
            "subtitle": "Proxy Health",
            "value": proxy_health_overview
                .avg_score
                .map(|value| format!("{value:.0}"))
                .unwrap_or_else(|| "--".to_string()),
            "value_display": proxy_health_overview
                .avg_score
                .map(|value| format!("{value:.0}"))
                .unwrap_or_else(|| "--".to_string()),
            "tone": if proxy_health_overview.stale_count > 0 || proxy_health_overview.unchecked_count > 0 {
                "warn"
            } else {
                "accent"
            },
            "lines": [
                {
                    "label": "Active / Total",
                    "value": format!(
                        "{}/{}",
                        proxy_pool.get("active").and_then(Value::as_i64).unwrap_or(0),
                        proxy_pool.get("total").and_then(Value::as_i64).unwrap_or(0)
                    )
                },
                { "label": "Checked", "value": proxy_health_overview.checked_count.to_string() },
                {
                    "label": "Low / Stale",
                    "value": format!(
                        "{}/{}",
                        proxy_health_overview.low_quality_rows.len(),
                        proxy_health_overview.stale_count
                    )
                }
            ]
        }),
        json!({
            "id": "session_health",
            "title": "Session Health",
            "subtitle": "Session Health",
            "value": session_metrics.get("active_sessions").and_then(Value::as_i64).unwrap_or(0),
            "value_display": session_metrics.get("active_sessions").and_then(Value::as_i64).unwrap_or(0).to_string(),
            "tone": if warning_continuity_count > 0 { "warn" } else { "ok" },
            "lines": [
                { "label": "Reuse Rate", "value": format!("{reuse_rate:.1}%") },
                { "label": "Reused / Created", "value": format!("{}/{}", reused_sessions, created_sessions) },
                { "label": "Continuity Warnings", "value": warning_continuity_count.to_string() },
                {
                    "label": "Cookie Persist",
                    "value": session_metrics
                        .get("cookie_persist_count")
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                        .to_string()
                }
            ]
        }),
    ];

    let secondary_metrics = vec![
        json!({
            "label": "Worker Count",
            "value": worker.get("worker_count").and_then(Value::as_i64).unwrap_or(0),
            "value_display": worker.get("worker_count").and_then(Value::as_i64).unwrap_or(0).to_string()
        }),
        json!({
            "label": "Gateway Requests",
            "value": gateway_stats_snapshot.total_events,
            "value_display": gateway_stats_snapshot.total_events.to_string()
        }),
        json!({
            "label": "Verify OK / Failed",
            "value": format!(
                "{}/{}",
                verify.get("verified_ok").and_then(Value::as_i64).unwrap_or(0),
                verify.get("verified_failed").and_then(Value::as_i64).unwrap_or(0)
            ),
            "value_display": format!(
                "{}/{}",
                verify.get("verified_ok").and_then(Value::as_i64).unwrap_or(0),
                verify.get("verified_failed").and_then(Value::as_i64).unwrap_or(0)
            )
        }),
        json!({
            "label": "Onboarded Sites",
            "value": site_validation_rollups.len(),
            "value_display": site_validation_rollups.len().to_string()
        }),
    ];

    let notices = if let Some(message) = status_error {
        vec![json!({
            "kind": "warning",
            "title": "Control-plane status fetch failed",
            "detail": message
        })]
    } else {
        Vec::new()
    };

    let site_filter_values = unique_site_filter_values(overview_tasks, site_validation_rollups);
    let proxy_health_json =
        serde_json::to_value(proxy_health_overview).unwrap_or_else(|_| json!({}));
    let overview_task_rows =
        serde_json::to_value(overview_tasks).unwrap_or_else(|_| Value::Array(Vec::new()));
    let alert_rows_json =
        serde_json::to_value(alert_rows).unwrap_or_else(|_| Value::Array(Vec::new()));
    let continuity_rows =
        serde_json::to_value(continuity_events).unwrap_or_else(|_| Value::Array(Vec::new()));
    let site_rollup_rows =
        serde_json::to_value(site_validation_rollups).unwrap_or_else(|_| Value::Array(Vec::new()));
    let proxy_source_rows_json = Value::Array(proxy_sources);
    let task_status_distribution =
        serde_json::to_value(build_task_status_distribution(overview_tasks))
            .unwrap_or_else(|_| Value::Array(Vec::new()));
    let failure_reason_distribution =
        serde_json::to_value(build_failure_reason_distribution(overview_tasks))
            .unwrap_or_else(|_| Value::Array(Vec::new()));
    let task_kind_distribution = serde_json::to_value(build_task_kind_distribution(overview_tasks))
        .unwrap_or_else(|_| Value::Array(Vec::new()));
    let initial_selected_task_id = pick_initial_selected_task_id(overview_tasks);

    let site_rows = Value::Array(
        drafts
            .iter()
            .map(|draft| {
                let display_status = site_validation_rollups
                    .iter()
                    .find(|rollup| rollup.draft_id == draft.id)
                    .map(|rollup| rollup.display_status.clone())
                    .unwrap_or_else(|| normalize_task_terminal_status(&draft.status, None));
                json!({
                    "draft_id": draft.id,
                    "site_key": draft.site_key,
                    "login_url": draft.login_url,
                    "status": draft.status,
                    "display_status": display_status,
                    "site_policy_id": draft.site_policy_id,
                    "site_policy_version": draft.site_policy_version,
                    "site_contract_present": draft.site_contract_present,
                    "share_url": draft.share_url,
                    "updated_at": draft.updated_at
                })
            })
            .collect::<Vec<_>>(),
    );
    let evidence_rows = Value::Array(
        drafts
            .iter()
            .map(|draft| {
                let preferred = preferred_evidence_entry(draft.evidence_summary_json.as_ref());
                json!({
                    "draft_id": draft.id,
                    "site_key": draft.site_key,
                    "status": draft.status,
                    "display_status": site_validation_rollups
                        .iter()
                        .find(|rollup| rollup.draft_id == draft.id)
                        .map(|rollup| rollup.display_status.clone())
                        .unwrap_or_else(|| normalize_task_terminal_status(&draft.status, None)),
                    "site_policy_id": draft.site_policy_id,
                    "site_policy_version": draft.site_policy_version,
                    "failure_signal": preferred.and_then(|value| value.get("failure_signal")).cloned().unwrap_or(Value::Null),
                    "retry_count": preferred.and_then(|value| value.get("retry_count")).cloned().unwrap_or(Value::Null),
                    "summary_raw": preferred.and_then(|value| value.get("summary_raw")).cloned().unwrap_or(Value::String("no evidence".to_string())),
                    "summary_zh": preferred.and_then(|value| value.get("summary_zh")).cloned().unwrap_or(Value::String("No localized summary yet".to_string())),
                    "summary_compact_zh": preferred.and_then(|value| value.get("summary_compact_zh")).cloned().unwrap_or(Value::String("No summary".to_string())),
                    "evidence_json": draft.evidence_summary_json
                })
            })
            .collect::<Vec<_>>(),
    );

    json!({
        "shell": {
            "connection_status": if status.is_some() && status_error.is_none() { "online" } else { "degraded" },
            "status_title": "Behavior Realism Console",
            "status_subtitle": "Monitoring-first operator console",
            "runtime_mode_label": runtime_mode_label,
            "landing_page": "overview",
            "token_required": false,
            "partial_failure": status_error.is_some(),
            "notices": notices
        },
        "overview": {
            "primary_cards": primary_cards,
            "secondary_metrics": secondary_metrics,
            "proxy_health_charts": proxy_health_json,
            "task_rows": overview_task_rows,
            "alert_rows": alert_rows_json,
            "proxy_source_rows": proxy_source_rows_json,
            "continuity_rows": continuity_rows,
            "site_rollup_rows": site_rollup_rows,
            "auth_metrics": auth_metrics,
            "task_status_counts": {
                "total": counts_total,
                "succeeded": counts_succeeded,
                "failed": counts_failed,
                "timed_out": counts_timed_out,
                "cancelled": counts_cancelled,
                "running": counts_running,
                "queued": counts_queued,
                "blocked": blocked_count
            },
            "task_status_distribution": task_status_distribution,
            "failure_reason_distribution": failure_reason_distribution,
            "task_kind_distribution": task_kind_distribution,
            "initial_selected_task_id": initial_selected_task_id,
            "status_filter_options": [
                { "value": "all", "label": "All" },
                { "value": "failed", "label": "Failed" },
                { "value": "timed_out", "label": "Timed Out" },
                { "value": "blocked", "label": "Blocked" },
                { "value": "running", "label": "Running" },
                { "value": "succeeded", "label": "Succeeded" },
                { "value": "shadow_only", "label": "Shadow Only" }
            ],
            "site_filter_options": Value::Array(
                std::iter::once(json!({ "value": "all", "label": "All Sites" }))
                    .chain(site_filter_values.into_iter().map(|site| json!({ "value": site, "label": site })))
                    .collect::<Vec<_>>()
            ),
            "task_tab_options": [
                { "value": "all", "label": "All" },
                { "value": "failed", "label": "Failed" },
                { "value": "timed_out", "label": "Timed Out" },
                { "value": "running", "label": "Running" }
            ],
            "empty_state": {
                "visible": overview_tasks.is_empty(),
                "title": if overview_tasks.is_empty() { "No tasks yet" } else { "" },
                "detail": if overview_tasks.is_empty() { "New task data will appear here automatically." } else { "" }
            },
            "partial_failure": status_error.is_some(),
            "disabled_reason": status_error
        },
        "onboarding": {
            "description": {
                "title": "Authorized Site (Advanced)",
                "subtitle": "Authorized Site Onboarding",
                "when_to_use": "Use this page only for onboarding a new authorized site or rerunning shadow/active/continuity verification.",
                "when_not_to_use": "Do not use this page for daily monitoring. Overview is the default monitoring page.",
                "minimum_steps": "Login URL -> Discover -> Fill password/submit/ready selector -> Publish and validate",
                "steps": [
                    {
                        "title": "1. Basic Setup",
                        "detail": "Fill login URL and choose identity/session/behavior/fingerprint/proxy."
                    },
                    {
                        "title": "2. Discover and Review",
                        "detail": "Run discover and confirm password selector, submit selector, and success.ready_selector."
                    },
                    {
                        "title": "3. Publish and Validate",
                        "detail": "Publish site policy, then run shadow, active_success and continuity."
                    }
                ]
            },
            "site_rows": site_rows,
            "validation_rows": site_rollup_rows,
            "evidence_rows": evidence_rows
        },
        "display_meta": {
            "summary_mode_default": "zh",
            "last_good_snapshot_at": now_ts_string(),
            "partial_failure": status_error.is_some(),
            "data_sources": ["/status", "gateway_stats_snapshot", "dashboard_onboarding_drafts", "site_behavior_policies", "proxy_health_snapshots"],
            "site_policy_count": site_policies.len()
        }
    })
}

fn mark_ui_model_readonly(ui_model: &mut Value) {
    let Some(root) = ui_model.as_object_mut() else {
        return;
    };
    if let Some(shell) = root.get_mut("shell").and_then(Value::as_object_mut) {
        shell.insert("readonly".to_string(), Value::Bool(true));
        shell.insert("public_preview".to_string(), Value::Bool(true));
        shell.insert("token_required".to_string(), Value::Bool(false));
        let notices = shell
            .entry("notices".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(items) = notices.as_array_mut() {
            items.insert(
                0,
                json!({
                    "kind": "info",
                    "title": "只读预览",
                    "detail": "当前页面只展示监控数据，不提供站点接入、发布或验证操作。"
                }),
            );
        }
    }
    if let Some(display_meta) = root.get_mut("display_meta").and_then(Value::as_object_mut) {
        display_meta.insert("readonly".to_string(), Value::Bool(true));
        display_meta.insert("public_preview".to_string(), Value::Bool(true));
    }
    if let Some(onboarding) = root.get_mut("onboarding").and_then(Value::as_object_mut) {
        onboarding.insert("readonly".to_string(), Value::Bool(true));
    }
}

fn normalize_proxy_source_rows(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .map(|item| {
            json!({
                "source_label": item.get("source_label").and_then(Value::as_str).unwrap_or("unknown"),
                "source_kind": item.get("source_kind").and_then(Value::as_str).unwrap_or("unknown"),
                "enabled": item.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                "health_score": item.get("health_score").and_then(Value::as_f64).unwrap_or(0.0),
                "candidate_count": item.get("candidate_count").and_then(Value::as_i64).unwrap_or(0),
                "active_count": item.get("active_count").and_then(Value::as_i64).unwrap_or(0),
                "candidate_rejected_count": item.get("candidate_rejected_count").and_then(Value::as_i64).unwrap_or(0),
                "null_provider_count": item.get("null_provider_count").and_then(Value::as_i64).unwrap_or(0),
                "null_region_count": item.get("null_region_count").and_then(Value::as_i64).unwrap_or(0),
                "promotion_rate": item.get("promotion_rate").and_then(Value::as_f64).unwrap_or(0.0)
            })
        })
        .collect()
}

fn unique_site_filter_values(
    overview_tasks: &[DashboardTaskListItem],
    site_validation_rollups: &[DashboardSiteValidationRollup],
) -> Vec<String> {
    let mut values = overview_tasks
        .iter()
        .filter_map(|task| task.site_key.clone())
        .chain(
            site_validation_rollups
                .iter()
                .map(|item| item.site_key.clone()),
        )
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn pick_initial_selected_task_id(tasks: &[DashboardTaskListItem]) -> Option<String> {
    tasks
        .iter()
        .find(|task| task.display_status == "failed")
        .or_else(|| tasks.iter().find(|task| task.display_status == "running"))
        .or_else(|| tasks.first())
        .map(|task| task.id.clone())
}

fn build_task_status_distribution(
    tasks: &[DashboardTaskListItem],
) -> Vec<DashboardDistributionBucket> {
    let mut counts = BTreeMap::<String, i64>::new();
    for task in tasks {
        *counts.entry(task.display_status.clone()).or_insert(0) += 1;
    }

    [
        ("failed", "Failed", "failed"),
        ("timed_out", "Timed Out", "warning"),
        ("blocked", "Blocked", "warning"),
        ("running", "Running", "info"),
        ("queued", "Queued", "neutral"),
        ("succeeded", "Succeeded", "success"),
        ("shadow_only", "Shadow Only", "neutral"),
        ("not_requested", "Not Requested", "neutral"),
    ]
    .into_iter()
    .map(|(key, label, tone)| DashboardDistributionBucket {
        key: key.to_string(),
        label: label.to_string(),
        tone: tone.to_string(),
        count: counts.get(key).copied().unwrap_or(0),
    })
    .collect()
}

fn build_failure_reason_distribution(
    tasks: &[DashboardTaskListItem],
) -> Vec<DashboardDistributionBucket> {
    let mut buckets = BTreeMap::<String, DashboardDistributionBucket>::new();

    for task in tasks {
        let reason_meta = match task.display_status.as_str() {
            "blocked" => Some(("blocked", "Contract blocked".to_string(), "warning")),
            "timed_out" => Some(("timed_out", "Execution timed out".to_string(), "warning")),
            "failed" => {
                let signal = task
                    .failure_signal
                    .as_deref()
                    .or(task.browser_failure_signal.as_deref());
                match signal {
                    Some(signal) => Some((
                        signal,
                        humanize_failure_signal(signal).to_string(),
                        failure_bucket_tone(signal),
                    )),
                    None => Some(("unknown_failed", "Unknown failure".to_string(), "failed")),
                }
            }
            _ => None,
        };

        if let Some((key, label, tone)) = reason_meta {
            buckets
                .entry(key.to_string())
                .and_modify(|bucket| bucket.count += 1)
                .or_insert(DashboardDistributionBucket {
                    key: key.to_string(),
                    label,
                    tone: tone.to_string(),
                    count: 1,
                });
        }
    }

    let mut rows = buckets.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    rows.truncate(8);
    rows
}

fn failure_bucket_tone(signal: &str) -> &'static str {
    match signal {
        "login_error" | "field_error" | "account_locked" => "failed",
        "submit_no_effect" | "transient_dom_error" | "timeout_waiting_success" => "warning",
        "runner_timeout" | "navigation_failed" | "browser_launch_failed" => "warning",
        _ => "failed",
    }
}

fn build_task_kind_distribution(
    tasks: &[DashboardTaskListItem],
) -> Vec<DashboardDistributionBucket> {
    let mut buckets = BTreeMap::<String, DashboardDistributionBucket>::new();

    for task in tasks {
        let key = task.kind.clone();
        let label = if task.task_kind_display.trim().is_empty() {
            humanize_task_kind(task.kind.as_str()).to_string()
        } else {
            task.task_kind_display.clone()
        };
        let tone = match task.kind.as_str() {
            "verify_proxy" => "info",
            "open_page" | "browse_site" | "login" | "check_session" => "accent",
            "extract_content" | "extract_text" | "parse_api" | "scrape_list" => "neutral",
            _ => "neutral",
        };
        buckets
            .entry(key.clone())
            .and_modify(|bucket| bucket.count += 1)
            .or_insert(DashboardDistributionBucket {
                key,
                label,
                tone: tone.to_string(),
                count: 1,
            });
    }

    let mut rows = buckets.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });
    rows.truncate(8);
    rows
}

fn build_dashboard_overview_tasks(
    status: Option<&Value>,
    proxies: &[ProxyOption],
) -> Vec<DashboardTaskListItem> {
    status
        .and_then(|value| value.get("latest_tasks"))
        .and_then(Value::as_array)
        .map(|tasks| {
            tasks
                .iter()
                .take(10)
                .map(|task| {
                    let proxy = task_proxy_id(task)
                        .as_deref()
                        .and_then(|proxy_id| proxies.iter().find(|item| item.id == proxy_id));
                    dashboard_task_list_item_from_task(task, proxy)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn build_dashboard_continuity_events(
    status: Option<&Value>,
    drafts: &[DashboardOnboardingDraftResponse],
) -> Vec<DashboardContinuityEvent> {
    let mut events = status
        .and_then(|value| value.get("latest_tasks"))
        .and_then(Value::as_array)
        .map(|tasks| {
            tasks
                .iter()
                .filter(|task| task_has_continuity_signal(task))
                .take(5)
                .map(build_continuity_event_from_task)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !events.is_empty() {
        return events;
    }

    for draft in drafts {
        let Some(entry) = preferred_evidence_entry(draft.evidence_summary_json.as_ref()) else {
            continue;
        };
        let session_persisted = entry
            .get("session_persisted")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !session_persisted {
            continue;
        }
        events.push(DashboardContinuityEvent {
            event_type: "persist".to_string(),
            severity: "success".to_string(),
            task_id: entry
                .get("task_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            site_key: Some(draft.site_key.clone()),
            occurred_at: Some(draft.updated_at.clone()),
            detail_short: entry
                .get("summary_compact_zh")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| {
                    "Session persisted successfully from onboarding evidence".to_string()
                }),
        });
        if events.len() >= 5 {
            break;
        }
    }
    events
}

fn build_site_validation_rollup(
    draft: &DashboardOnboardingDraftResponse,
) -> DashboardSiteValidationRollup {
    let preferred = preferred_evidence_entry(draft.evidence_summary_json.as_ref());
    let form_action_status = preferred
        .and_then(|value| value.get("form_action_status"))
        .and_then(Value::as_str)
        .map(str::to_string);
    DashboardSiteValidationRollup {
        draft_id: draft.id.clone(),
        site_key: draft.site_key.clone(),
        login_url: draft.login_url.clone(),
        status: draft.status.clone(),
        display_status: normalize_task_display_status(&draft.status, form_action_status.as_deref()),
        site_policy_id: draft.site_policy_id.clone(),
        site_policy_version: draft.site_policy_version,
        site_contract_present: draft.site_contract_present,
        shadow_task_id: draft.shadow_task_id.clone(),
        active_success_task_id: draft.active_success_task_id.clone(),
        active_failure_task_id: draft.active_failure_task_id.clone(),
        continuity_task_id: draft.continuity_task_id.clone(),
        form_action_status,
        retry_count: preferred
            .and_then(|value| value.get("retry_count"))
            .and_then(Value::as_i64),
        failure_signal: preferred
            .and_then(|value| value.get("failure_signal"))
            .and_then(Value::as_str)
            .map(str::to_string),
        success_ready_selector_seen: preferred
            .and_then(|value| value.get("success_ready_selector_seen"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        post_login_actions_executed: preferred
            .and_then(|value| value.get("post_login_actions_executed"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        session_persisted: preferred
            .and_then(|value| value.get("session_persisted"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        summary_raw: preferred
            .and_then(|value| value.get("summary_raw"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "no validation evidence".to_string()),
        summary_zh: preferred
            .and_then(|value| value.get("summary_zh"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "No localized summary yet".to_string()),
        summary_compact_zh: preferred
            .and_then(|value| value.get("summary_compact_zh"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "No summary".to_string()),
        summary_kind: preferred
            .and_then(|value| value.get("summary_kind"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| "unknown".to_string()),
    }
}

fn preferred_evidence_entry(summary: Option<&Value>) -> Option<&Value> {
    let summary = summary?;
    summary
        .get("active_success")
        .or_else(|| summary.get("active_failure"))
        .or_else(|| summary.get("retry_observation"))
        .or_else(|| summary.get("continuity"))
        .or_else(|| summary.get("shadow"))
}

fn dashboard_task_list_item_from_task(
    task: &Value,
    proxy: Option<&ProxyOption>,
) -> DashboardTaskListItem {
    let standardized = standardize_task_summary_from_task(task);
    let status = json_string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    let kind = json_string_field(task, "kind").unwrap_or_else(|| "unknown".to_string());
    let form_action_mode = json_string_field(task, "form_action_mode");
    let form_action_status = json_string_field(task, "form_action_status");
    let failure_signal = task
        .get("form_action_summary_json")
        .and_then(|value| value.get("failure_signal"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let browser_failure_signal = json_string_field(task, "browser_failure_signal");
    let session_persisted = task
        .get("form_action_summary_json")
        .and_then(|value| value.get("session_persisted"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            json_nested_bool_field(task, &["behavior_trace_summary", "session_persisted"])
                .unwrap_or(false)
        });
    let site_key = site_key_from_task(task);
    let title = json_string_field(task, "title");
    let final_url = json_string_field(task, "final_url");
    let proxy_id = task_proxy_id(task).or_else(|| proxy.map(|item| item.id.clone()));
    let proxy_provider =
        task_proxy_provider(task).or_else(|| proxy.and_then(|item| item.provider.clone()));
    let proxy_region =
        task_proxy_region(task).or_else(|| proxy.and_then(|item| item.region.clone()));
    let proxy_health_score = proxy.and_then(|item| item.proxy_health_score);
    let proxy_health_grade = proxy.and_then(|item| item.proxy_health_grade.clone());
    let trust_score_total =
        task_trust_score_total(task).or_else(|| proxy.and_then(|item| item.trust_score_total));
    let task_display_name = build_task_display_name(
        &kind,
        task,
        site_key.as_deref(),
        title.as_deref(),
        final_url.as_deref(),
        proxy_provider.as_deref(),
        proxy_region.as_deref(),
        proxy_id.as_deref(),
    );
    DashboardTaskListItem {
        id: json_string_field(task, "id").unwrap_or_else(|| "-".to_string()),
        kind,
        task_display_name,
        task_kind_display: standardized.task_kind_display.clone(),
        status: status.clone(),
        display_status: normalize_task_display_status(&status, form_action_status.as_deref()),
        form_action_mode,
        form_action_status,
        retry_count: json_i64_field(task, "form_action_retry_count"),
        failure_signal,
        browser_failure_signal,
        started_at: json_string_field(task, "started_at"),
        finished_at: json_string_field(task, "finished_at"),
        site_key,
        title,
        final_url,
        proxy_id,
        proxy_provider,
        proxy_region,
        proxy_health_score,
        proxy_health_grade,
        trust_score_total,
        session_persisted,
        summary_raw: standardized.summary_raw,
        summary_zh: standardized.summary_zh,
        summary_compact_zh: standardized.summary_compact_zh,
        summary_kind: standardized.summary_kind,
    }
}

fn task_proxy_id(task: &Value) -> Option<String> {
    json_string_field(task, "proxy_id")
        .or_else(|| json_nested_string_field(task, &["execution_identity", "proxy_id"]))
        .or_else(|| json_nested_string_field(task, &["identity_network_explain", "proxy_id"]))
}

fn task_proxy_provider(task: &Value) -> Option<String> {
    json_string_field(task, "proxy_provider")
        .or_else(|| json_nested_string_field(task, &["execution_identity", "proxy_provider"]))
        .or_else(|| json_nested_string_field(task, &["identity_network_explain", "proxy_provider"]))
}

fn task_proxy_region(task: &Value) -> Option<String> {
    json_string_field(task, "proxy_region")
        .or_else(|| json_nested_string_field(task, &["execution_identity", "proxy_region"]))
        .or_else(|| json_nested_string_field(task, &["identity_network_explain", "proxy_region"]))
}

fn task_trust_score_total(task: &Value) -> Option<i64> {
    json_i64_field(task, "trust_score_total")
        .or_else(|| json_nested_i64_field(task, &["execution_identity", "trust_score_total"]))
        .or_else(|| json_nested_i64_field(task, &["identity_network_explain", "trust_score_total"]))
}

fn build_task_display_name(
    kind: &str,
    task: &Value,
    site_key: Option<&str>,
    title: Option<&str>,
    final_url: Option<&str>,
    proxy_provider: Option<&str>,
    proxy_region: Option<&str>,
    proxy_id: Option<&str>,
) -> String {
    let target = task_target_label(site_key, title, final_url);
    match kind {
        "verify_proxy" => {
            let label = proxy_label_from_parts(proxy_provider, proxy_region, proxy_id);
            if label.is_empty() {
                "Proxy Verify".to_string()
            } else {
                format!("Proxy Verify - {label}")
            }
        }
        "open_page" if task.get("form_action_mode").is_some() => {
            format!("Site Auth Check - {target}")
        }
        "login" | "register" | "check_session" => {
            format!("{} - {target}", humanize_task_kind(kind))
        }
        _ if !target.is_empty() => format!("{} - {target}", humanize_task_kind(kind)),
        _ => humanize_task_kind(kind).to_string(),
    }
}

fn task_target_label(
    site_key: Option<&str>,
    title: Option<&str>,
    final_url: Option<&str>,
) -> String {
    site_key
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            title
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            final_url
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown target".to_string())
}

fn task_has_continuity_signal(task: &Value) -> bool {
    let restore_total = session_count(task, "cookie_restore_count")
        + session_count(task, "local_storage_restore_count")
        + session_count(task, "session_storage_restore_count");
    let persist_total = session_count(task, "cookie_persist_count")
        + session_count(task, "local_storage_persist_count")
        + session_count(task, "session_storage_persist_count");
    restore_total > 0
        || persist_total > 0
        || json_nested_bool_field(task, &["behavior_trace_summary", "session_persisted"])
            .unwrap_or(false)
        || task
            .get("form_action_summary_json")
            .and_then(|value| value.get("session_persisted"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn build_continuity_event_from_task(task: &Value) -> DashboardContinuityEvent {
    let restore_total = session_count(task, "cookie_restore_count")
        + session_count(task, "local_storage_restore_count")
        + session_count(task, "session_storage_restore_count");
    let persist_total = session_count(task, "cookie_persist_count")
        + session_count(task, "local_storage_persist_count")
        + session_count(task, "session_storage_persist_count");
    let event_type = if restore_total > 0 && persist_total > 0 {
        "restore_and_persist"
    } else if restore_total > 0 {
        "restore"
    } else if persist_total > 0 {
        "persist"
    } else {
        "continuity"
    };
    let severity = match json_string_field(task, "status").as_deref() {
        Some("failed" | "timed_out" | "cancelled") => "warning",
        _ if persist_total > 0 => "success",
        _ => "info",
    };
    DashboardContinuityEvent {
        event_type: event_type.to_string(),
        severity: severity.to_string(),
        task_id: json_string_field(task, "id"),
        site_key: site_key_from_task(task),
        occurred_at: json_string_field(task, "finished_at")
            .or_else(|| json_string_field(task, "started_at")),
        detail_short: format!(
            "Cookie restore/persist: {}/{} | LocalStorage: {}/{} | SessionStorage: {}/{}",
            session_count(task, "cookie_restore_count"),
            session_count(task, "cookie_persist_count"),
            session_count(task, "local_storage_restore_count"),
            session_count(task, "local_storage_persist_count"),
            session_count(task, "session_storage_restore_count"),
            session_count(task, "session_storage_persist_count"),
        ),
    }
}

fn session_count(task: &Value, field: &str) -> i64 {
    json_nested_i64_field(task, &["execution_identity", field])
        .or_else(|| json_nested_i64_field(task, &["identity_network_explain", field]))
        .unwrap_or(0)
}

fn standardize_task_summary_from_task(task: &Value) -> StandardizedTaskSummary {
    let kind = json_string_field(task, "kind").unwrap_or_else(|| "unknown".to_string());
    let task_kind_display = humanize_task_kind(&kind).to_string();
    let (summary_raw, summary_kind) = primary_task_summary(task);
    let (summary_zh, summary_compact_zh) = translate_task_summary(task, &kind, &summary_raw);
    StandardizedTaskSummary {
        summary_raw,
        summary_zh,
        summary_compact_zh,
        summary_kind,
        task_kind_display,
    }
}

fn primary_task_summary(task: &Value) -> (String, String) {
    if let Some(artifact) = pick_primary_summary_artifact(task) {
        let raw = artifact
            .get("summary")
            .and_then(Value::as_str)
            .or_else(|| artifact.get("title").and_then(Value::as_str))
            .unwrap_or_default()
            .trim()
            .to_string();
        let kind = artifact
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or("summary.unknown")
            .to_string();
        if !raw.is_empty() {
            return (raw, kind);
        }
    }
    (
        build_summary_raw_fallback(task),
        json_string_field(task, "kind").unwrap_or_else(|| "unknown".to_string()),
    )
}

fn pick_primary_summary_artifact(task: &Value) -> Option<&Value> {
    let task_status = json_string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    task.get("summary_artifacts")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().max_by_key(|item| {
                let key = item.get("key").and_then(Value::as_str).unwrap_or_default();
                let title = item
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let severity = item
                    .get("severity")
                    .and_then(Value::as_str)
                    .unwrap_or("info");
                let base = match severity {
                    "error" => 40,
                    "warning" => 30,
                    _ => 10,
                };
                let status_bonus = if task_status != "succeeded"
                    && (key.contains("failure") || title.contains("failure"))
                {
                    100
                } else if key.starts_with("browser.result") {
                    80
                } else if key == "identity.network.summary" {
                    60
                } else if title.eq_ignore_ascii_case("identity session continuity") {
                    55
                } else {
                    0
                };
                base + status_bonus
            })
        })
}

fn build_summary_raw_fallback(task: &Value) -> String {
    let kind = json_string_field(task, "kind").unwrap_or_else(|| "unknown".to_string());
    let status = json_string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    let title = json_string_field(task, "title");
    let final_url = json_string_field(task, "final_url");
    let browser_failure_signal = json_string_field(task, "browser_failure_signal");
    let failure_signal = task
        .get("form_action_summary_json")
        .and_then(|value| value.get("failure_signal"))
        .and_then(Value::as_str)
        .map(str::to_string);

    if task.get("form_action_mode").is_some() {
        return format!(
            "form_action_status={} failure_signal={} retry_count={}",
            json_string_field(task, "form_action_status")
                .unwrap_or_else(|| "not_requested".to_string()),
            failure_signal.unwrap_or_else(|| "none".to_string()),
            json_i64_field(task, "form_action_retry_count").unwrap_or(0)
        );
    }

    match (title, final_url, browser_failure_signal) {
        (Some(title), Some(final_url), _) => {
            format!(
                "kind={} status={} title={} final_url={}",
                kind, status, title, final_url
            )
        }
        (Some(title), None, _) => format!("kind={} status={} title={}", kind, status, title),
        (None, Some(final_url), _) => {
            format!("kind={} status={} final_url={}", kind, status, final_url)
        }
        (_, _, Some(signal)) => {
            format!("kind={} status={} failure_signal={}", kind, status, signal)
        }
        _ => format!("kind={} status={}", kind, status),
    }
}

fn translate_task_summary(task: &Value, kind: &str, raw: &str) -> (String, String) {
    if task.get("form_action_mode").is_some() && is_form_summary_kind(kind) {
        return translate_form_task_summary(task);
    }
    match kind {
        "verify_proxy" => translate_verify_proxy_summary(task, raw),
        "extract_content" | "parse_api" => translate_data_task_summary(task, kind),
        "browse_site" | "open_page" | "screenshot" | "scroll_page" | "scrape_list" => {
            translate_browser_task_summary(task, kind)
        }
        "login" | "register" | "check_session" => translate_account_task_summary(task, kind),
        _ => translate_generic_task_summary(task, kind, raw),
    }
}

fn is_form_summary_kind(kind: &str) -> bool {
    matches!(
        kind,
        "open_page"
            | "browse_site"
            | "login"
            | "register"
            | "check_session"
            | "screenshot"
            | "scroll_page"
            | "scrape_list"
    )
}

fn translate_form_task_summary(task: &Value) -> (String, String) {
    let mode_label = match json_string_field(task, "form_action_mode").as_deref() {
        Some("form") => "Form",
        _ => "Auth",
    };
    let form_status = json_string_field(task, "form_action_status")
        .unwrap_or_else(|| "not_requested".to_string());
    let retry_count = json_i64_field(task, "form_action_retry_count").unwrap_or(0);
    let failure_signal = task
        .get("form_action_summary_json")
        .and_then(|value| value.get("failure_signal"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| json_string_field(task, "browser_failure_signal"));
    let session_persisted = task
        .get("form_action_summary_json")
        .and_then(|value| value.get("session_persisted"))
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            json_nested_bool_field(task, &["behavior_trace_summary", "session_persisted"])
                .unwrap_or(false)
        });

    match form_status.as_str() {
        "succeeded" => {
            let summary = if session_persisted {
                format!("{mode_label} flow succeeded; post-login actions completed and session persisted")
            } else {
                format!("{mode_label} flow succeeded; post-login actions completed")
            };
            (summary, format!("{mode_label} succeeded"))
        }
        "blocked" => (
            format!("{mode_label} blocked: required contract or ready selector missing"),
            format!("{mode_label} blocked"),
        ),
        "shadow_only" => (
            format!("{mode_label} shadow-only: no real submit executed"),
            format!("{mode_label} shadow-only"),
        ),
        "failed" => {
            let reason = failure_signal
                .as_deref()
                .map(humanize_failure_signal)
                .unwrap_or("unknown failure");
            if retry_count > 0 {
                (
                    format!("{mode_label} failed: {reason}; retried {retry_count} time(s)"),
                    format!("{mode_label} failed: {reason}"),
                )
            } else {
                (
                    format!("{mode_label} failed: {reason}"),
                    format!("{mode_label} failed: {reason}"),
                )
            }
        }
        "running" => (
            format!("{mode_label} flow is running"),
            format!("{mode_label} running"),
        ),
        _ => (
            format!("{mode_label} state unavailable"),
            format!("{mode_label} unavailable"),
        ),
    }
}

fn translate_verify_proxy_summary(task: &Value, raw: &str) -> (String, String) {
    let label = proxy_label(task);
    let status = json_string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    if status == "succeeded" {
        let detail = if label.is_empty() {
            "Proxy verification succeeded".to_string()
        } else {
            format!("Proxy verification succeeded: {label}")
        };
        return (detail.clone(), "Proxy ok".to_string());
    }
    let lower = raw.to_ascii_lowercase();
    let reason = if lower.contains("timeout") {
        "connection timeout"
    } else if lower.contains("refused")
        || lower.contains("connect failed")
        || lower.contains("connection")
    {
        "connection failed"
    } else if lower.contains("geo") || lower.contains("region") {
        "geo mismatch"
    } else if lower.contains("auth") {
        "proxy auth failed"
    } else {
        "verification failed"
    };
    if label.is_empty() {
        (
            format!("Proxy verification failed: {reason}"),
            format!("Proxy failed: {reason}"),
        )
    } else {
        (
            format!("Proxy verification failed for {label}: {reason}"),
            format!("Proxy failed: {reason}"),
        )
    }
}

fn translate_browser_task_summary(task: &Value, kind: &str) -> (String, String) {
    let action = humanize_task_kind(kind);
    let status = json_string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    let target = json_string_field(task, "title")
        .or_else(|| json_string_field(task, "final_url"))
        .or_else(|| site_key_from_task(task))
        .unwrap_or_else(|| "target page".to_string());
    match status.as_str() {
        "succeeded" => (
            format!("{action} succeeded: {target}"),
            format!("{action} succeeded"),
        ),
        "timed_out" => (
            format!("{action} timed out: {target}"),
            format!("{action} timed out"),
        ),
        "cancelled" => (
            format!("{action} cancelled: {target}"),
            format!("{action} cancelled"),
        ),
        "failed" => {
            let reason = json_string_field(task, "browser_failure_signal")
                .as_deref()
                .map(humanize_failure_signal)
                .unwrap_or("execution failed");
            (
                format!("{action} failed: {reason}"),
                format!("{action} failed"),
            )
        }
        "running" => (
            format!("{action} running: {target}"),
            format!("{action} running"),
        ),
        _ => (
            format!("{action} status: {}", humanize_status(status.as_str())),
            format!("{action} {}", humanize_status(status.as_str())),
        ),
    }
}

fn translate_data_task_summary(task: &Value, kind: &str) -> (String, String) {
    let action = humanize_task_kind(kind);
    let status = json_string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    if status == "succeeded" {
        let content_length = json_i64_field(task, "content_length").unwrap_or(0);
        if content_length > 0 {
            return (
                format!("{action} succeeded with content_length={content_length}"),
                format!("{action} succeeded"),
            );
        }
    }
    translate_browser_task_summary(task, kind)
}

fn translate_account_task_summary(task: &Value, kind: &str) -> (String, String) {
    let action = humanize_task_kind(kind);
    let status = json_string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    match status.as_str() {
        "succeeded" => (format!("{action} succeeded"), format!("{action} succeeded")),
        "failed" => {
            let reason = json_string_field(task, "browser_failure_signal")
                .as_deref()
                .map(humanize_failure_signal)
                .unwrap_or("unknown failure");
            (
                format!("{action} failed: {reason}"),
                format!("{action} failed"),
            )
        }
        _ => (
            format!("{action} status: {}", humanize_status(status.as_str())),
            format!("{action} {}", humanize_status(status.as_str())),
        ),
    }
}

fn translate_generic_task_summary(task: &Value, kind: &str, _raw: &str) -> (String, String) {
    let label = humanize_task_kind(kind);
    let status = json_string_field(task, "status").unwrap_or_else(|| "unknown".to_string());
    let site = site_key_from_task(task);
    let detail = site
        .map(|value| format!("{label} - {value}"))
        .unwrap_or_else(|| label.to_string());
    match status.as_str() {
        "succeeded" => (format!("{detail} succeeded"), format!("{label} succeeded")),
        "failed" => (format!("{detail} failed"), format!("{label} failed")),
        "timed_out" => (format!("{detail} timed out"), format!("{label} timed out")),
        "cancelled" => (format!("{detail} cancelled"), format!("{label} cancelled")),
        "running" => (format!("{detail} running"), format!("{label} running")),
        _ => (
            format!("{detail} {}", humanize_status(status.as_str())),
            format!("{label} {}", humanize_status(status.as_str())),
        ),
    }
}

fn proxy_label(task: &Value) -> String {
    proxy_label_from_parts(
        task_proxy_provider(task).as_deref(),
        task_proxy_region(task).as_deref(),
        task_proxy_id(task).as_deref(),
    )
}

fn proxy_label_from_parts(
    provider: Option<&str>,
    region: Option<&str>,
    proxy_id: Option<&str>,
) -> String {
    match (provider, region, proxy_id) {
        (Some(provider), Some(region), _) => format!("{provider}/{region}"),
        (Some(provider), None, _) => provider.to_string(),
        (None, Some(region), Some(proxy_id)) => format!("{proxy_id}/{region}"),
        (None, None, Some(proxy_id)) => proxy_id.to_string(),
        _ => String::new(),
    }
}

fn humanize_task_kind(kind: &str) -> &'static str {
    match kind {
        "verify_proxy" => "Proxy Verify",
        "browse_site" => "Browse Site",
        "open_page" => "Open Page",
        "screenshot" => "Screenshot",
        "scroll_page" => "Scroll Page",
        "extract_content" => "Extract Content",
        "extract_text" => "Extract Text",
        "scrape_list" => "Scrape List",
        "parse_api" => "Parse API",
        "get_title" => "Get Title",
        "get_html" => "Get HTML",
        "login" => "Login",
        "register" => "Register",
        "check_session" => "Check Session",
        _ => "Task",
    }
}

fn humanize_status(status: &str) -> &'static str {
    match status {
        "queued" => "Queued",
        "running" => "Running",
        "succeeded" => "Succeeded",
        "failed" => "Failed",
        "timed_out" => "Timed Out",
        "cancelled" => "Cancelled",
        "blocked" => "Blocked",
        "shadow_only" => "Shadow Only",
        "not_requested" => "Not Requested",
        _ => "Unknown",
    }
}

fn runtime_mode_label(mode: &str) -> &'static str {
    match mode {
        "prod_live" => "Prod",
        "dev" => "Dev",
        "demo" => "Demo",
        _ => "Unknown",
    }
}

fn humanize_failure_signal(signal: &str) -> &'static str {
    match signal {
        "login_error" => "invalid username or password",
        "field_error" => "field validation failed",
        "account_locked" => "account locked",
        "missing_required_field" => "missing required field",
        "submit_no_effect" => "submit had no effect",
        "transient_dom_error" => "transient DOM error",
        "timeout_waiting_success" => "timeout waiting success",
        "inline_secret_unavailable" => "inline secret unavailable",
        "runner_timeout" => "runner timeout",
        "browser_launch_failed" => "browser launch failed",
        "navigation_failed" => "navigation failed",
        _ => "unknown failure",
    }
}

fn normalize_task_display_status(status: &str, form_action_status: Option<&str>) -> String {
    match status {
        "queued" | "running" => status.to_string(),
        _ => normalize_task_terminal_status(status, form_action_status),
    }
}

fn normalize_task_terminal_status(status: &str, form_action_status: Option<&str>) -> String {
    match status {
        "succeeded" => match form_action_status {
            Some("shadow_only") => "shadow_only".to_string(),
            Some("blocked") => "blocked".to_string(),
            Some("failed") => "failed".to_string(),
            Some("succeeded") => "succeeded".to_string(),
            _ => "succeeded".to_string(),
        },
        "failed" => match form_action_status {
            Some("blocked") => "blocked".to_string(),
            Some("shadow_only") => "shadow_only".to_string(),
            Some("failed") => "failed".to_string(),
            _ => "failed".to_string(),
        },
        "timed_out" => "timed_out".to_string(),
        "cancelled" => "cancelled".to_string(),
        "blocked" => "blocked".to_string(),
        "shadow_only" => "shadow_only".to_string(),
        "not_requested" | "draft" | "discovered" | "published" => "not_requested".to_string(),
        _ => form_action_status
            .map(str::to_string)
            .unwrap_or_else(|| status.to_string()),
    }
}

fn site_key_from_task(task: &Value) -> Option<String> {
    json_string_field(task, "site_key")
        .or_else(|| json_nested_string_field(task, &["form_action_summary_json", "site_key"]))
        .or_else(|| {
            json_string_field(task, "final_url")
                .and_then(|url| site_key_from_url(Some(url.as_str())))
        })
        .or_else(|| json_nested_string_field(task, &["behavior_runtime_explain", "site_key"]))
        .or_else(|| {
            json_string_field(task, "title")
                .and_then(|value| site_key_from_url(Some(value.as_str())))
        })
}

fn task_evidence(task: &Value) -> Value {
    let standardized = standardize_task_summary_from_task(task);
    let form_summary = task
        .get("form_action_summary_json")
        .cloned()
        .unwrap_or_else(|| Value::Null);
    json!({
        "task_id": task.get("id"),
        "status": task.get("status"),
        "form_action_status": task.get("form_action_status"),
        "retry_count": task.get("form_action_retry_count").or_else(|| form_summary.get("retry_count")),
        "failure_signal": form_summary.get("failure_signal").cloned().unwrap_or_else(|| task.get("browser_failure_signal").cloned().unwrap_or(Value::Null)),
        "success_ready_selector_seen": form_summary.get("success_ready_selector_seen").cloned().unwrap_or(Value::Bool(false)),
        "post_login_actions_executed": form_summary.get("post_login_actions_executed").cloned().unwrap_or(Value::Bool(false)),
        "session_persisted": form_summary.get("session_persisted").cloned().or_else(|| task.get("behavior_trace_summary").and_then(|summary| summary.get("session_persisted")).cloned()).unwrap_or(Value::Bool(false)),
        "site_key": site_key_from_task(task),
        "task_kind_display": standardized.task_kind_display,
        "summary_kind": standardized.summary_kind,
        "summary_raw": standardized.summary_raw,
        "summary_zh": standardized.summary_zh,
        "summary_compact_zh": standardized.summary_compact_zh,
        "title": task.get("title"),
        "final_url": task.get("final_url")
    })
}

fn is_successful_active_auth(task: &Value) -> bool {
    json_string_field(task, "status").as_deref() == Some("succeeded")
        && json_string_field(task, "form_action_status").as_deref() == Some("succeeded")
}

async fn fetch_login_page_html(state: &GatewayState, login_url: &str) -> Result<String, Response> {
    let response = state
        .http_client
        .get(login_url)
        .send()
        .await
        .map_err(|err| {
            error_response(
                StatusCode::BAD_GATEWAY,
                "discover_fetch_failed",
                &format!("failed to fetch login_url for discover: {err}"),
            )
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(error_response(
            StatusCode::BAD_GATEWAY,
            "discover_fetch_failed",
            &format!("discover fetch returned HTTP {}", status.as_u16()),
        ));
    }
    response.text().await.map_err(|err| {
        error_response(
            StatusCode::BAD_GATEWAY,
            "discover_fetch_failed",
            &format!("failed to decode discovery html: {err}"),
        )
    })
}

fn infer_form_contract_from_html(html: &str, success_hint: Option<&str>) -> Value {
    let lower = html.to_ascii_lowercase();
    let forms = collect_tag_snippets(&lower, "form");
    let inputs = collect_tag_snippets(&lower, "input");
    let buttons = collect_tag_snippets(&lower, "button");
    let mut field_roles = Map::new();
    if let Some(selector) = infer_username_selector(&inputs) {
        field_roles.insert("username".to_string(), json!({ "selector": selector }));
    }
    if let Some(selector) = infer_password_selector(&inputs) {
        field_roles.insert("password".to_string(), json!({ "selector": selector }));
    }
    if let Some(selector) = infer_remember_selector(&inputs) {
        field_roles.insert("remember_me".to_string(), json!({ "selector": selector }));
    }
    if let Some(selector) = infer_submit_selector(&inputs, &buttons) {
        field_roles.insert("submit".to_string(), json!({ "selector": selector }));
    }

    json!({
        "mode": "auth",
        "primary_form_selector": infer_form_selector(&forms).unwrap_or_else(|| "form".to_string()),
        "field_roles": field_roles,
        "success": {
            "ready_selector": success_hint.filter(|hint| looks_like_selector(hint)).map(str::trim).filter(|hint| !hint.is_empty()),
            "url_patterns": [],
            "title_contains": []
        },
        "error_signals": {
            "login_error": infer_error_signal_selectors(&lower, &["login-error", "error-message", "alert-danger", "auth-error"]),
            "field_error": infer_error_signal_selectors(&lower, &["field-error", "invalid-feedback", "input-error", "aria-invalid=\"true\"", "aria-invalid='true'"]),
            "account_locked": infer_error_signal_selectors(&lower, &["account-locked", "locked-message", "account-status-locked"])
        }
    })
}

fn infer_form_selector(forms: &[String]) -> Option<String> {
    forms
        .iter()
        .find_map(|snippet| selector_from_snippet("form", snippet))
        .or_else(|| (!forms.is_empty()).then(|| "form".to_string()))
}

fn infer_username_selector(inputs: &[String]) -> Option<String> {
    inputs
        .iter()
        .max_by_key(|snippet| username_score(snippet))
        .and_then(|snippet| (username_score(snippet) > 0).then_some(snippet))
        .and_then(|snippet| selector_from_snippet("input", snippet))
}

fn infer_password_selector(inputs: &[String]) -> Option<String> {
    inputs
        .iter()
        .find(|snippet| {
            attribute_value(snippet, "type").as_deref() == Some("password")
                || snippet.contains("password")
        })
        .and_then(|snippet| selector_from_snippet("input", snippet))
}

fn infer_remember_selector(inputs: &[String]) -> Option<String> {
    inputs
        .iter()
        .find(|snippet| {
            attribute_value(snippet, "type").as_deref() == Some("checkbox")
                && contains_any(snippet, &["remember", "stay", "trusted"])
        })
        .and_then(|snippet| selector_from_snippet("input", snippet))
}

fn infer_submit_selector(inputs: &[String], buttons: &[String]) -> Option<String> {
    buttons
        .iter()
        .find(|snippet| {
            attribute_value(snippet, "type").as_deref() == Some("submit")
                || contains_any(snippet, &["login", "sign-in", "signin", "submit"])
        })
        .and_then(|snippet| selector_from_snippet("button", snippet))
        .or_else(|| {
            inputs
                .iter()
                .find(|snippet| {
                    matches!(
                        attribute_value(snippet, "type").as_deref(),
                        Some("submit" | "button")
                    ) && contains_any(snippet, &["login", "sign-in", "signin", "submit"])
                })
                .and_then(|snippet| selector_from_snippet("input", snippet))
        })
        .or_else(|| {
            buttons
                .iter()
                .find(|snippet| attribute_value(snippet, "type").as_deref() == Some("submit"))
                .map(|_| "button[type='submit']".to_string())
        })
        .or_else(|| {
            inputs
                .iter()
                .find(|snippet| attribute_value(snippet, "type").as_deref() == Some("submit"))
                .map(|_| "input[type='submit']".to_string())
        })
}

fn infer_error_signal_selectors(html: &str, markers: &[&str]) -> Vec<String> {
    markers
        .iter()
        .filter_map(|marker| {
            if !html.contains(marker) {
                return None;
            }
            if marker.starts_with("aria-invalid") {
                Some("[aria-invalid='true']".to_string())
            } else {
                Some(format!(".{}", marker.replace('\"', "").replace('\'', "")))
            }
        })
        .collect::<Vec<_>>()
}

fn collect_tag_snippets(html: &str, tag_name: &str) -> Vec<String> {
    let mut snippets = Vec::new();
    let needle = format!("<{}", tag_name);
    let mut start = 0usize;
    while let Some(index) = html[start..].find(needle.as_str()) {
        let absolute = start + index;
        if let Some(end) = html[absolute..].find('>') {
            snippets.push(html[absolute..absolute + end + 1].to_string());
            start = absolute + end + 1;
        } else {
            break;
        }
    }
    snippets
}

fn selector_from_snippet(tag_name: &str, snippet: &str) -> Option<String> {
    if let Some(id) = attribute_value(snippet, "id") {
        return Some(format!("{tag_name}#{}", id));
    }
    if let Some(name) = attribute_value(snippet, "name") {
        return Some(format!("{tag_name}[name='{}']", name.replace('\'', "\\'")));
    }
    if let Some(class_name) = attribute_value(snippet, "class")
        .and_then(|value| value.split_whitespace().next().map(str::to_string))
    {
        return Some(format!("{tag_name}.{}", class_name.replace('.', "")));
    }
    match tag_name {
        "button" if attribute_value(snippet, "type").as_deref() == Some("submit") => {
            Some("button[type='submit']".to_string())
        }
        "input" if attribute_value(snippet, "type").as_deref() == Some("password") => {
            Some("input[type='password']".to_string())
        }
        "input" if attribute_value(snippet, "type").as_deref() == Some("email") => {
            Some("input[type='email']".to_string())
        }
        _ => None,
    }
}

fn username_score(snippet: &str) -> i32 {
    if matches!(
        attribute_value(snippet, "type").as_deref(),
        Some("password" | "checkbox" | "hidden" | "submit")
    ) {
        return -100;
    }
    let mut score = 0;
    if attribute_value(snippet, "type").as_deref() == Some("email") {
        score += 8;
    }
    if attribute_value(snippet, "type").as_deref() == Some("text") {
        score += 4;
    }
    if contains_any(snippet, &["username", "email", "login", "identifier"]) {
        score += 6;
    }
    if attribute_value(snippet, "autocomplete")
        .as_deref()
        .is_some_and(|value| value.contains("username") || value.contains("email"))
    {
        score += 10;
    }
    score
}

fn attribute_value(snippet: &str, attribute: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{attribute}={quote}");
        if let Some(start) = snippet.find(needle.as_str()) {
            let value_start = start + needle.len();
            if let Some(end) = snippet[value_start..].find(quote) {
                let value = snippet[value_start..value_start + end].trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn looks_like_selector(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with('#')
        || trimmed.starts_with('.')
        || trimmed.starts_with('[')
        || trimmed.starts_with("div")
        || trimmed.starts_with("main")
        || trimmed.starts_with("nav")
        || trimmed.starts_with("section")
        || trimmed.contains('>')
        || trimmed.contains('[')
}

fn parsed_contract_object(raw: Option<&str>) -> Option<Map<String, Value>> {
    raw.and_then(|item| serde_json::from_str::<Value>(item).ok())
        .and_then(|value| value.as_object().cloned())
}

fn contract_field_selector(contract: &Map<String, Value>, field: &str) -> Option<String> {
    contract
        .get("field_roles")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get(field))
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("selector"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
}

fn contract_submit_selector(contract: &Map<String, Value>) -> Option<String> {
    contract
        .get("submit")
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("selector"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .or_else(|| contract_field_selector(contract, "submit"))
}

async fn control_request_value(
    state: &GatewayState,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let mut url = state
        .config
        .control_base_url
        .trim_end_matches('/')
        .to_string();
    url.push_str(path);
    let mut request = state.http_client.request(method, &url);
    if let Some(api_key) = state.config.control_api_key.as_deref() {
        request = request.header("x-api-key", api_key);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .map_err(|err| format!("failed to reach control-plane: {err}"))?;
    let status = response.status();
    let raw = response
        .text()
        .await
        .map_err(|err| format!("failed to read control-plane response: {err}"))?;
    if !status.is_success() {
        return Err(format!(
            "control-plane {} {} failed with HTTP {}: {}",
            state.config.control_base_url,
            path,
            status.as_u16(),
            raw
        ));
    }
    serde_json::from_str(&raw).map_err(|err| format!("control-plane returned invalid JSON: {err}"))
}

async fn load_active_named_resources(
    db: &DbPool,
    table: &str,
) -> Result<Vec<NamedResourceOption>, sqlx::Error> {
    let sql = format!(
        "SELECT id, name, version, status FROM {table} WHERE status = 'active' ORDER BY updated_at DESC, id DESC"
    );
    let rows = sqlx::query_as::<_, (String, String, i64, String)>(&sql)
        .fetch_all(db)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, version, status)| NamedResourceOption {
            id,
            name,
            version,
            status,
        })
        .collect())
}

async fn unique_active_id(db: &DbPool, table: &str) -> Result<Option<String>, sqlx::Error> {
    let sql = format!(
        "SELECT id FROM {table} WHERE status = 'active' ORDER BY updated_at DESC, id DESC LIMIT 2"
    );
    let ids = sqlx::query_scalar::<_, String>(&sql).fetch_all(db).await?;
    Ok((ids.len() == 1).then(|| ids[0].clone()))
}

async fn load_proxy_health_overview(db: &DbPool) -> Result<ProxyHealthOverview, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<f64>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<String>,
        ),
    >(
        r#"SELECT
               id,
               provider,
               region,
               source_label,
               proxy_health_score,
               proxy_health_grade,
               proxy_health_checked_at,
               cached_trust_score,
               last_probe_latency_ms,
               proxy_health_summary_json
           FROM proxies
           WHERE status = 'active'
           ORDER BY COALESCE(proxy_health_score, -1) ASC, updated_at DESC, id DESC"#,
    )
    .fetch_all(db)
    .await?;

    let stale_before = now_ts_i64() - proxy_health_stale_after_seconds_from_env();
    let total_active = rows.len() as i64;
    let checked_count = rows
        .iter()
        .filter(|(_, _, _, _, _, _, checked_at, _, _, _)| checked_at.is_some())
        .count() as i64;
    let unchecked_count = total_active - checked_count;
    let stale_count = rows
        .iter()
        .filter(|(_, _, _, _, _, _, checked_at, _, _, _)| {
            proxy_health_checked_stale(checked_at.as_deref(), stale_before)
        })
        .count() as i64;
    let scored_values = rows
        .iter()
        .filter_map(|(_, _, _, _, score, _, _, _, _, _)| *score)
        .collect::<Vec<_>>();
    let avg_score = if scored_values.is_empty() {
        None
    } else {
        Some(scored_values.iter().sum::<f64>() / scored_values.len() as f64)
    };
    let healthy_count = rows
        .iter()
        .filter(|(_, _, _, _, _, grade, _, _, _, _)| {
            matches!(grade.as_deref(), Some("A+") | Some("A") | Some("B+"))
        })
        .count() as i64;
    let warning_count = rows
        .iter()
        .filter(|(_, _, _, _, score, grade, checked_at, _, _, _)| {
            score.is_none()
                || proxy_health_checked_stale(checked_at.as_deref(), stale_before)
                || matches!(
                    grade.as_deref(),
                    Some("B") | Some("C+") | Some("C") | Some("D") | Some("F")
                )
        })
        .count() as i64;

    let mut grade_distribution = [
        ("A+", "A+ Excellent"),
        ("A", "A Healthy"),
        ("B+", "B+ Good"),
        ("B", "B Usable"),
        ("C+", "C+ Weak"),
        ("C", "C Risky"),
        ("D", "D High Risk"),
        ("F", "F Unusable"),
        ("unchecked", "Unchecked"),
    ]
    .into_iter()
    .map(|(grade, label)| ProxyHealthGradeBucket {
        grade: grade.to_string(),
        label: label.to_string(),
        tone: proxy_health_grade_tone(grade).to_string(),
        count: 0,
    })
    .collect::<Vec<_>>();
    let mut score_band_distribution = [
        ("90_plus", "90+", "success"),
        ("80_89", "80-89", "success"),
        ("70_79", "70-79", "info"),
        ("60_69", "60-69", "warning"),
        ("below_60", "<60", "failed"),
    ]
    .into_iter()
    .map(|(key, label, tone)| ProxyHealthScoreBandBucket {
        key: key.to_string(),
        label: label.to_string(),
        tone: tone.to_string(),
        count: 0,
    })
    .collect::<Vec<_>>();
    let mut source_comparison = BTreeMap::<String, ProxyHealthSourceAccumulator>::new();
    let mut reason_buckets = BTreeMap::<String, ProxyHealthReasonBucket>::new();
    let mut low_quality_rows = Vec::new();

    for (
        proxy_id,
        provider,
        region,
        source_label,
        proxy_health_score,
        proxy_health_grade,
        proxy_health_checked_at,
        trust_score_total,
        last_probe_latency_ms,
        proxy_health_summary_json,
    ) in rows.into_iter()
    {
        let bucket_key = proxy_health_grade.as_deref().unwrap_or("unchecked");
        if let Some(bucket) = grade_distribution
            .iter_mut()
            .find(|item| item.grade == bucket_key)
        {
            bucket.count += 1;
        }
        if let Some(score) = proxy_health_score {
            let score_band_key = proxy_health_score_band_key(score);
            if let Some(bucket) = score_band_distribution
                .iter_mut()
                .find(|item| item.key == score_band_key)
            {
                bucket.count += 1;
            }
        }

        let source_key = source_label
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "unknown-source".to_string());
        let source_stats = source_comparison.entry(source_key).or_default();
        source_stats.active_count += 1;
        if proxy_health_checked_at.is_some() {
            source_stats.checked_count += 1;
        }
        let is_stale = proxy_health_checked_stale(proxy_health_checked_at.as_deref(), stale_before);
        if is_stale {
            source_stats.stale_count += 1;
        }
        if let Some(score) = proxy_health_score {
            source_stats.score_sum += score;
            source_stats.scored_count += 1;
        }

        let reason = proxy_health_low_quality_reason(
            proxy_health_score,
            proxy_health_grade.as_deref(),
            proxy_health_checked_at.as_deref(),
            stale_before,
            proxy_health_summary_json.as_deref(),
        );
        if let Some(reason) = reason {
            source_stats.low_quality_count += 1;
            let (reason_key, reason_label, tone) = proxy_health_reason_bucket_meta(&reason);
            reason_buckets
                .entry(reason_key.to_string())
                .and_modify(|bucket| bucket.count += 1)
                .or_insert(ProxyHealthReasonBucket {
                    key: reason_key.to_string(),
                    label: reason_label.to_string(),
                    tone: tone.to_string(),
                    count: 1,
                });
            low_quality_rows.push(ProxyHealthLowQualityRow {
                proxy_id,
                provider,
                region,
                source_label,
                proxy_health_score,
                proxy_health_grade,
                proxy_health_checked_at,
                trust_score_total,
                reason,
                last_probe_latency_ms,
            });
        }
    }
    low_quality_rows.sort_by(|left, right| {
        proxy_health_low_quality_rank(left)
            .cmp(&proxy_health_low_quality_rank(right))
            .then_with(|| {
                left.proxy_health_score
                    .unwrap_or(-1.0)
                    .partial_cmp(&right.proxy_health_score.unwrap_or(-1.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    low_quality_rows.truncate(12);

    let mut source_comparison_rows = source_comparison
        .into_iter()
        .map(|(source_label, stats)| ProxyHealthSourceComparisonRow {
            source_label,
            avg_score: (stats.scored_count > 0)
                .then(|| stats.score_sum / stats.scored_count as f64),
            active_count: stats.active_count,
            checked_count: stats.checked_count,
            stale_count: stats.stale_count,
            low_quality_count: stats.low_quality_count,
        })
        .collect::<Vec<_>>();
    source_comparison_rows.sort_by(|left, right| {
        right.active_count.cmp(&left.active_count).then_with(|| {
            left.avg_score
                .unwrap_or(-1.0)
                .partial_cmp(&right.avg_score.unwrap_or(-1.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    let mut low_quality_reason_buckets = reason_buckets.into_values().collect::<Vec<_>>();
    low_quality_reason_buckets.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.label.cmp(&right.label))
    });

    Ok(ProxyHealthOverview {
        total_active,
        checked_count,
        unchecked_count,
        stale_count,
        avg_score,
        healthy_count,
        warning_count,
        grade_distribution,
        score_band_distribution,
        source_comparison_rows,
        low_quality_reason_buckets,
        low_quality_rows,
    })
}

fn proxy_health_score_band_key(score: f64) -> &'static str {
    if score >= 90.0 {
        "90_plus"
    } else if score >= 80.0 {
        "80_89"
    } else if score >= 70.0 {
        "70_79"
    } else if score >= 60.0 {
        "60_69"
    } else {
        "below_60"
    }
}

fn proxy_health_checked_stale(checked_at: Option<&str>, stale_before: i64) -> bool {
    checked_at
        .and_then(|value| value.parse::<i64>().ok())
        .map(|value| value <= stale_before)
        .unwrap_or(false)
}

fn proxy_health_probe_error(summary_json: Option<&str>) -> Option<String> {
    serde_json::from_str::<Value>(summary_json?)
        .ok()
        .and_then(|value| {
            value
                .get("probe")
                .and_then(|probe| probe.get("error"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn proxy_health_low_quality_reason(
    score: Option<f64>,
    grade: Option<&str>,
    checked_at: Option<&str>,
    stale_before: i64,
    summary_json: Option<&str>,
) -> Option<String> {
    if checked_at.is_none() {
        return Some("never checked".to_string());
    }
    if proxy_health_checked_stale(checked_at, stale_before) {
        return Some("health snapshot stale".to_string());
    }
    if let Some(error) = proxy_health_probe_error(summary_json) {
        return Some(format!("probe failed: {error}"));
    }
    match grade {
        Some("D") | Some("F") => Some("grade too low".to_string()),
        Some("C+") | Some("C") => Some("quality weak".to_string()),
        _ if score.unwrap_or(100.0) < 75.0 => Some("score below threshold".to_string()),
        _ => None,
    }
}

fn proxy_health_reason_bucket_meta(reason: &str) -> (&'static str, &'static str, &'static str) {
    if reason.contains("never checked") {
        ("unchecked", "Unchecked", "neutral")
    } else if reason.contains("stale") {
        ("stale", "Stale", "warning")
    } else if reason.contains("probe failed") {
        ("probe_error", "Probe Failed", "failed")
    } else if reason.contains("grade too low") {
        ("grade_low", "Grade Too Low", "failed")
    } else if reason.contains("quality weak") {
        ("quality_weak", "Quality Weak", "warning")
    } else if reason.contains("score below threshold") {
        ("score_low", "Score Low", "warning")
    } else {
        ("other", "Other", "neutral")
    }
}
fn proxy_health_grade_tone(grade: &str) -> &'static str {
    match grade {
        "A+" | "A" | "B+" => "ok",
        "B" | "C+" | "unchecked" => "warn",
        "C" | "D" | "F" => "danger",
        _ => "neutral",
    }
}

fn proxy_health_low_quality_rank(row: &ProxyHealthLowQualityRow) -> i32 {
    if row.proxy_health_checked_at.is_none() {
        0
    } else if row.reason.contains("probe failed") {
        1
    } else if matches!(row.proxy_health_grade.as_deref(), Some("F") | Some("D")) {
        2
    } else if row.reason.contains("quality weak") {
        3
    } else {
        4
    }
}

async fn load_active_proxies(db: &DbPool) -> Result<Vec<ProxyOption>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            Option<String>,
            Option<String>,
            String,
            f64,
            Option<f64>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
        ),
    >(
        r#"SELECT
               id,
               provider,
               region,
               status,
               score,
               proxy_health_score,
               proxy_health_grade,
               proxy_health_checked_at,
               cached_trust_score,
               source_label
           FROM proxies
           WHERE status = 'active'
           ORDER BY COALESCE(proxy_health_score, -1) DESC, score DESC, updated_at DESC, id DESC
           LIMIT 100"#,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                provider,
                region,
                status,
                score,
                proxy_health_score,
                proxy_health_grade,
                proxy_health_checked_at,
                trust_score_total,
                source_label,
            )| ProxyOption {
                id,
                provider,
                region,
                status,
                score,
                proxy_health_score,
                proxy_health_grade,
                proxy_health_checked_at,
                trust_score_total,
                source_label,
            },
        )
        .collect())
}

async fn unique_active_proxy_id(db: &DbPool) -> Result<Option<String>, sqlx::Error> {
    let ids = sqlx::query_scalar::<_, String>(
        r#"SELECT id FROM proxies WHERE status = 'active' ORDER BY score DESC, updated_at DESC, id DESC LIMIT 2"#,
    )
    .fetch_all(db)
    .await?;
    Ok((ids.len() == 1).then(|| ids[0].clone()))
}

async fn load_active_auth_site_policies(db: &DbPool) -> Result<Vec<SitePolicyOption>, sqlx::Error> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            i64,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"SELECT id, version, site_key, behavior_profile_id, status, page_archetype, action_kind, override_json
           FROM site_behavior_policies
           WHERE status = 'active'
             AND (page_archetype = 'auth' OR page_archetype IS NULL)
             AND (action_kind = 'open_page' OR action_kind IS NULL)
           ORDER BY priority DESC, updated_at DESC, id DESC
           LIMIT 50"#,
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                version,
                site_key,
                behavior_profile_id,
                status,
                page_archetype,
                action_kind,
                override_json,
            )| {
                let has_form_contract = override_json
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                    .and_then(|value| value.get("form_contract").cloned())
                    .and_then(|value| value.as_object().cloned())
                    .is_some();
                SitePolicyOption {
                    id,
                    version,
                    site_key,
                    behavior_profile_id,
                    status,
                    page_archetype,
                    action_kind,
                    has_form_contract,
                }
            },
        )
        .collect())
}

async fn load_matching_site_policy(
    db: &DbPool,
    record: &DashboardOnboardingDraftRecord,
) -> Result<Option<(Option<String>, Option<i64>)>, sqlx::Error> {
    if let Some(policy_id) = normalize_optional_text(record.site_policy_id.clone()) {
        let existing = sqlx::query_as::<_, (String, i64)>(
            "SELECT id, version FROM site_behavior_policies WHERE id = ?",
        )
        .bind(policy_id)
        .fetch_optional(db)
        .await?;
        if let Some((id, version)) = existing {
            return Ok(Some((Some(id), Some(version))));
        }
    }

    let existing = sqlx::query_as::<_, (String, i64)>(
        r#"SELECT id, version
           FROM site_behavior_policies
           WHERE site_key = ?
             AND (page_archetype = 'auth' OR page_archetype IS NULL)
             AND (action_kind = 'open_page' OR action_kind IS NULL)
           ORDER BY CASE WHEN status = 'active' THEN 1 ELSE 0 END DESC,
                    priority DESC,
                    updated_at DESC,
                    id DESC
           LIMIT 1"#,
    )
    .bind(&record.site_key)
    .fetch_optional(db)
    .await?;
    Ok(existing.map(|(id, version)| (Some(id), Some(version))))
}

async fn list_draft_records(
    db: &DbPool,
    limit: i64,
    offset: i64,
) -> Result<Vec<DashboardOnboardingDraftRecord>, sqlx::Error> {
    sqlx::query_as::<_, DashboardOnboardingDraftRecord>(
        r#"SELECT *
           FROM dashboard_onboarding_drafts
           ORDER BY updated_at DESC, created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
}

async fn load_draft_by_id(
    db: &DbPool,
    draft_id: &str,
) -> Result<Option<DashboardOnboardingDraftRecord>, sqlx::Error> {
    sqlx::query_as::<_, DashboardOnboardingDraftRecord>(
        "SELECT * FROM dashboard_onboarding_drafts WHERE id = ?",
    )
    .bind(draft_id)
    .fetch_optional(db)
    .await
}

async fn load_draft_by_share_token(
    db: &DbPool,
    share_token: &str,
) -> Result<Option<DashboardOnboardingDraftRecord>, sqlx::Error> {
    sqlx::query_as::<_, DashboardOnboardingDraftRecord>(
        "SELECT * FROM dashboard_onboarding_drafts WHERE share_token = ?",
    )
    .bind(share_token)
    .fetch_optional(db)
    .await
}

async fn insert_draft_record(
    db: &DbPool,
    record: &DashboardOnboardingDraftRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO dashboard_onboarding_drafts (
               id, share_token, share_expires_at, status, login_url, site_key, success_hint,
               behavior_profile_id, identity_profile_id, session_profile_id, fingerprint_profile_id, proxy_id,
               credential_mode, credential_ref, inferred_contract_json, final_contract_json,
               site_policy_id, site_policy_version, shadow_task_id, active_success_task_id, active_failure_task_id,
               continuity_task_id, evidence_summary_json, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&record.id)
    .bind(&record.share_token)
    .bind(&record.share_expires_at)
    .bind(&record.status)
    .bind(&record.login_url)
    .bind(&record.site_key)
    .bind(&record.success_hint)
    .bind(&record.behavior_profile_id)
    .bind(&record.identity_profile_id)
    .bind(&record.session_profile_id)
    .bind(&record.fingerprint_profile_id)
    .bind(&record.proxy_id)
    .bind(&record.credential_mode)
    .bind(&record.credential_ref)
    .bind(&record.inferred_contract_json)
    .bind(&record.final_contract_json)
    .bind(&record.site_policy_id)
    .bind(record.site_policy_version)
    .bind(&record.shadow_task_id)
    .bind(&record.active_success_task_id)
    .bind(&record.active_failure_task_id)
    .bind(&record.continuity_task_id)
    .bind(&record.evidence_summary_json)
    .bind(&record.created_at)
    .bind(&record.updated_at)
    .execute(db)
    .await?;
    Ok(())
}

async fn update_draft_record(
    db: &DbPool,
    record: &DashboardOnboardingDraftRecord,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE dashboard_onboarding_drafts
           SET share_token = ?, share_expires_at = ?, status = ?, login_url = ?, site_key = ?, success_hint = ?,
               behavior_profile_id = ?, identity_profile_id = ?, session_profile_id = ?, fingerprint_profile_id = ?,
               proxy_id = ?, credential_mode = ?, credential_ref = ?, inferred_contract_json = ?, final_contract_json = ?,
               site_policy_id = ?, site_policy_version = ?, shadow_task_id = ?, active_success_task_id = ?,
               active_failure_task_id = ?, continuity_task_id = ?, evidence_summary_json = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&record.share_token)
    .bind(&record.share_expires_at)
    .bind(&record.status)
    .bind(&record.login_url)
    .bind(&record.site_key)
    .bind(&record.success_hint)
    .bind(&record.behavior_profile_id)
    .bind(&record.identity_profile_id)
    .bind(&record.session_profile_id)
    .bind(&record.fingerprint_profile_id)
    .bind(&record.proxy_id)
    .bind(&record.credential_mode)
    .bind(&record.credential_ref)
    .bind(&record.inferred_contract_json)
    .bind(&record.final_contract_json)
    .bind(&record.site_policy_id)
    .bind(record.site_policy_version)
    .bind(&record.shadow_task_id)
    .bind(&record.active_success_task_id)
    .bind(&record.active_failure_task_id)
    .bind(&record.continuity_task_id)
    .bind(&record.evidence_summary_json)
    .bind(&record.updated_at)
    .bind(&record.id)
    .execute(db)
    .await?;
    Ok(())
}

fn draft_record_to_response(
    record: DashboardOnboardingDraftRecord,
) -> DashboardOnboardingDraftResponse {
    let inferred_contract_json = parse_optional_json(record.inferred_contract_json.as_deref());
    let final_contract_json = parse_optional_json(record.final_contract_json.as_deref());
    DashboardOnboardingDraftResponse {
        id: record.id,
        share_token: record.share_token.clone(),
        share_url: format!("/dashboard/?draft={}", record.share_token),
        share_expires_at: record.share_expires_at,
        status: record.status,
        login_url: record.login_url,
        site_key: record.site_key,
        success_hint: record.success_hint,
        behavior_profile_id: record.behavior_profile_id,
        identity_profile_id: record.identity_profile_id,
        session_profile_id: record.session_profile_id,
        fingerprint_profile_id: record.fingerprint_profile_id,
        proxy_id: record.proxy_id,
        credential_mode: record.credential_mode,
        credential_ref: record.credential_ref,
        inferred_contract_json,
        final_contract_json: final_contract_json.clone(),
        site_policy_id: record.site_policy_id,
        site_policy_version: record.site_policy_version,
        shadow_task_id: record.shadow_task_id,
        active_success_task_id: record.active_success_task_id,
        active_failure_task_id: record.active_failure_task_id,
        continuity_task_id: record.continuity_task_id,
        evidence_summary_json: parse_optional_json(record.evidence_summary_json.as_deref()),
        site_contract_present: final_contract_json
            .as_ref()
            .and_then(Value::as_object)
            .is_some(),
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

fn parse_optional_json(raw: Option<&str>) -> Option<Value> {
    raw.and_then(|value| serde_json::from_str::<Value>(value).ok())
}

fn normalize_login_url(login_url: &str) -> Result<String, Response> {
    let trimmed = login_url.trim();
    if trimmed.is_empty() {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_login_url",
            "login_url is required",
        ));
    }
    let parsed = Url::parse(trimmed).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "invalid_login_url",
            "login_url must be a valid http/https URL",
        )
    })?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_login_url",
            "login_url must use http or https",
        ));
    }
    Ok(trimmed.to_string())
}

fn normalize_credential_mode(value: Option<String>) -> Result<String, Response> {
    let normalized = value
        .unwrap_or_else(|| "alias".to_string())
        .trim()
        .to_string();
    match normalized.as_str() {
        "alias" | "inline_once" => Ok(normalized),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_credential_mode",
            "credential_mode must be alias or inline_once",
        )),
    }
}

fn normalize_credential_ref(mode: &str, value: Option<String>) -> Result<Option<String>, Response> {
    if mode == "inline_once" {
        return Ok(None);
    }
    let normalized = normalize_optional_text(value);
    if let Some(ref credential_ref) = normalized {
        if !credential_ref.starts_with("identity://") {
            return Err(error_response(
                StatusCode::BAD_REQUEST,
                "invalid_credential_ref",
                "credential_ref must use identity:// alias syntax",
            ));
        }
    }
    Ok(normalized)
}

fn normalize_contract_input(value: Option<Value>) -> Result<Option<Value>, Response> {
    match value {
        Some(value) if value.is_null() => Ok(None),
        Some(value) => normalize_contract_object(value).map(Some),
        None => Ok(None),
    }
}

fn normalize_contract_object(value: Value) -> Result<Value, Response> {
    match value {
        Value::Object(_) => Ok(value),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_contract",
            "final_contract_json must be a JSON object",
        )),
    }
}

fn normalize_validation_scenario(value: Option<String>) -> Result<String, Response> {
    let normalized = value
        .unwrap_or_else(|| "default".to_string())
        .trim()
        .to_string();
    match normalized.as_str() {
        "default" | "business_failure" | "retry_observation" => Ok(normalized),
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "invalid_validation_scenario",
            "scenario must be default, business_failure, or retry_observation",
        )),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn json_string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn json_nested_string_field(value: &Value, path: &[&str]) -> Option<String> {
    json_nested_value(value, path)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn json_i64_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn json_nested_value<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn json_nested_i64_field(value: &Value, path: &[&str]) -> Option<i64> {
    json_nested_value(value, path).and_then(Value::as_i64)
}

fn json_nested_bool_field(value: &Value, path: &[&str]) -> Option<bool> {
    json_nested_value(value, path).and_then(Value::as_bool)
}

fn share_token_expired(raw: &str) -> bool {
    raw.parse::<i64>().ok().unwrap_or(0) <= now_ts_i64()
}

fn now_ts_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn now_ts_string() -> String {
    now_ts_i64().to_string()
}

fn future_ts_string(ttl_seconds: u64) -> String {
    (now_ts_i64() + ttl_seconds as i64).to_string()
}

fn refresh_draft_share_expiry(record: &mut DashboardOnboardingDraftRecord, ttl_seconds: u64) {
    record.share_expires_at = future_ts_string(ttl_seconds);
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

fn slugify_site_key(site_key: &str) -> String {
    site_key
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn is_terminal_task_status(status: &str) -> bool {
    matches!(status, "succeeded" | "failed" | "timed_out" | "cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_like_hint_detection_is_strict_enough() {
        assert!(looks_like_selector("#app"));
        assert!(looks_like_selector("main.dashboard"));
        assert!(!looks_like_selector("dashboard loaded"));
    }

    #[test]
    fn infer_contract_prefills_required_selector_candidates() {
        let html = r#"
            <html>
              <body>
                <form id="login-form">
                  <input id="email" type="email" name="email" />
                  <input id="password" type="password" name="password" />
                  <input id="remember_me" type="checkbox" name="remember_me" />
                  <button id="submit-btn" type="submit">Sign in</button>
                  <div class="login-error"></div>
                </form>
              </body>
            </html>
        "#;
        let contract = infer_form_contract_from_html(html, Some("#dashboard"));
        assert_eq!(
            contract
                .get("field_roles")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("password"))
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("selector"))
                .and_then(Value::as_str),
            Some("input#password")
        );
        assert_eq!(
            contract
                .get("field_roles")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("submit"))
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("selector"))
                .and_then(Value::as_str),
            Some("button#submit-btn")
        );
        assert_eq!(
            contract
                .get("success")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("ready_selector"))
                .and_then(Value::as_str),
            Some("#dashboard")
        );
    }
    #[test]
    fn task_evidence_includes_dashboard_summary_fields() {
        let task = json!({
            "id": "task-1",
            "kind": "verify_proxy",
            "status": "failed",
            "proxy_provider": "demo-provider",
            "proxy_region": "us",
            "summary_artifacts": [{
                "key": "verify.proxy.failure",
                "title": "verify proxy failure",
                "summary": "validation timeout while connecting upstream",
                "severity": "error"
            }]
        });
        let evidence = task_evidence(&task);
        assert_eq!(
            evidence.get("task_kind_display").and_then(Value::as_str),
            Some("Proxy Verify")
        );
        assert_eq!(
            evidence.get("summary_kind").and_then(Value::as_str),
            Some("verify.proxy.failure")
        );
        assert!(evidence
            .get("summary_zh")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("Proxy verification failed"));
        assert!(evidence
            .get("summary_compact_zh")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("Proxy failed"));
    }

    #[test]
    fn verify_proxy_summary_does_not_fall_back_to_form_language() {
        let task = json!({
            "kind": "verify_proxy",
            "status": "failed",
            "form_action_mode": "auth",
            "summary_artifacts": [{
                "key": "verify.proxy.failure",
                "summary": "pool needs replenishment for this request"
            }]
        });
        let (detail, compact) = translate_task_summary(
            &task,
            "verify_proxy",
            "pool needs replenishment for this request",
        );
        assert!(detail.contains("Proxy verification failed"));
        assert!(compact.contains("Proxy failed"));
    }
}
