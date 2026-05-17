use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde_json::Value;

use crate::{app::state::AppState, behavior::normalize_resource_status};

use super::dto::{
    BehaviorProfileResponse, CreateBehaviorProfileRequest, CreateIdentityProfileRequest,
    CreateNetworkProfileRequest, CreateSessionProfileRequest, CreateSiteBehaviorPolicyRequest,
    IdentityProfileResponse, NetworkProfileResponse, PaginationQuery, SessionProfileResponse,
    SiteBehaviorPolicyResponse, UpdateBehaviorProfileRequest, UpdateIdentityProfileRequest,
    UpdateNetworkProfileRequest, UpdateSessionProfileRequest, UpdateSiteBehaviorPolicyRequest,
};

const PAGE_ARCHETYPES: &[&str] = &[
    "article",
    "search_results",
    "listing",
    "product",
    "form",
    "auth",
    "dashboard",
    "generic",
];

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

fn json_string_to_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::json!({}))
}

fn optional_json_string_to_value(raw: Option<&str>) -> Option<Value> {
    raw.map(json_string_to_value)
}

fn require_non_empty(value: &str, label: &str) -> Result<(), (StatusCode, String)> {
    if value.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, format!("{label} is required")));
    }
    Ok(())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalize_site_key(site_key: &str) -> Result<String, (StatusCode, String)> {
    require_non_empty(site_key, "site_key")?;
    Ok(site_key.trim().to_ascii_lowercase())
}

fn normalize_page_archetype_input(
    value: Option<&str>,
) -> Result<Option<String>, (StatusCode, String)> {
    match value.map(str::trim).filter(|item| !item.is_empty()) {
        Some(item) if PAGE_ARCHETYPES.contains(&item) => Ok(Some(item.to_string())),
        Some(item) => Err((
            StatusCode::BAD_REQUEST,
            format!(
                "page_archetype must be one of article|search_results|listing|product|form|auth|dashboard|generic, got {item}"
            ),
        )),
        None => Ok(None),
    }
}

async fn ensure_active_reference(
    state: &AppState,
    table: &str,
    resource_id: &str,
    label: &str,
) -> Result<(), (StatusCode, String)> {
    let sql = format!("SELECT id FROM {table} WHERE id = ? AND status = 'active'");
    let exists = sqlx::query_scalar::<_, String>(&sql)
        .bind(resource_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to validate {label}: {err}"),
            )
        })?;

    if exists.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{label} not found or inactive: {resource_id}"),
        ));
    }

    Ok(())
}

async fn ensure_active_reference_optional(
    state: &AppState,
    table: &str,
    resource_id: Option<&str>,
    label: &str,
) -> Result<(), (StatusCode, String)> {
    if let Some(resource_id) = resource_id {
        ensure_active_reference(state, table, resource_id, label).await?;
    }
    Ok(())
}

fn behavior_profile_response_from_tuple(
    row: (
        String,
        String,
        i64,
        String,
        Option<String>,
        String,
        String,
        String,
    ),
) -> BehaviorProfileResponse {
    let (id, name, version, status, tags_json, profile_json, created_at, updated_at) = row;
    BehaviorProfileResponse {
        id,
        name,
        version,
        status,
        tags_json,
        profile_json: json_string_to_value(&profile_json),
        created_at,
        updated_at,
    }
}

fn identity_profile_response_from_tuple(
    row: (
        String,
        String,
        i64,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        String,
        String,
    ),
) -> IdentityProfileResponse {
    let (
        id,
        name,
        version,
        status,
        fingerprint_profile_id,
        behavior_profile_id,
        network_profile_id,
        identity_json,
        secret_aliases_json,
        created_at,
        updated_at,
    ) = row;
    IdentityProfileResponse {
        id,
        name,
        version,
        status,
        fingerprint_profile_id,
        behavior_profile_id,
        network_profile_id,
        identity_json: json_string_to_value(&identity_json),
        secret_aliases_json: optional_json_string_to_value(secret_aliases_json.as_deref()),
        created_at,
        updated_at,
    }
}

fn network_profile_response_from_tuple(
    row: (String, String, i64, String, String, String, String),
) -> NetworkProfileResponse {
    let (id, name, version, status, network_policy_json, created_at, updated_at) = row;
    NetworkProfileResponse {
        id,
        name,
        version,
        status,
        network_policy_json: json_string_to_value(&network_policy_json),
        created_at,
        updated_at,
    }
}

fn session_profile_response_from_tuple(
    row: (
        String,
        String,
        i64,
        String,
        String,
        Option<String>,
        String,
        String,
    ),
) -> SessionProfileResponse {
    let (id, name, version, status, continuity_mode, retention_policy_json, created_at, updated_at) =
        row;
    SessionProfileResponse {
        id,
        name,
        version,
        status,
        continuity_mode,
        retention_policy_json: optional_json_string_to_value(retention_policy_json.as_deref()),
        created_at,
        updated_at,
    }
}

fn site_behavior_policy_response_from_tuple(
    row: (
        String,
        i64,
        String,
        Option<String>,
        Option<String>,
        String,
        i64,
        bool,
        Option<String>,
        String,
        String,
        String,
    ),
) -> SiteBehaviorPolicyResponse {
    let (
        id,
        version,
        site_key,
        page_archetype,
        action_kind,
        behavior_profile_id,
        priority,
        required,
        override_json,
        status,
        created_at,
        updated_at,
    ) = row;
    SiteBehaviorPolicyResponse {
        id,
        version,
        site_key,
        page_archetype,
        action_kind,
        behavior_profile_id,
        priority,
        required,
        override_json: optional_json_string_to_value(override_json.as_deref()),
        status,
        created_at,
        updated_at,
    }
}

pub async fn create_behavior_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreateBehaviorProfileRequest>,
) -> Result<(StatusCode, Json<BehaviorProfileResponse>), (StatusCode, String)> {
    require_non_empty(&payload.id, "behavior profile id")?;
    require_non_empty(&payload.name, "behavior profile name")?;

    let now = now_ts_string();
    let status = normalize_resource_status(payload.status.as_deref())
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

    sqlx::query(
        r#"INSERT INTO behavior_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
           VALUES (?, ?, 1, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(&status)
    .bind(&payload.tags_json)
    .bind(payload.profile_json.to_string())
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create behavior profile: {err}"),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(BehaviorProfileResponse {
            id: payload.id,
            name: payload.name,
            version: 1,
            status,
            tags_json: payload.tags_json,
            profile_json: payload.profile_json,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_behavior_profiles(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<BehaviorProfileResponse>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            i64,
            String,
            Option<String>,
            String,
            String,
            String,
        ),
    >(
        r#"SELECT id, name, version, status, tags_json, profile_json, created_at, updated_at
           FROM behavior_profiles
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(sanitize_limit(query.limit, 20, 200))
    .bind(sanitize_offset(query.offset))
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to list behavior profiles: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(behavior_profile_response_from_tuple)
            .collect(),
    ))
}

pub async fn get_behavior_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<BehaviorProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            i64,
            String,
            Option<String>,
            String,
            String,
            String,
        ),
    >(
        r#"SELECT id, name, version, status, tags_json, profile_json, created_at, updated_at
           FROM behavior_profiles
           WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch behavior profile: {err}"),
        )
    })?;

    match row {
        Some(row) => Ok(Json(behavior_profile_response_from_tuple(row))),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("behavior profile not found: {profile_id}"),
        )),
    }
}

pub async fn patch_behavior_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
    Json(payload): Json<UpdateBehaviorProfileRequest>,
) -> Result<Json<BehaviorProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, i64, String, Option<String>, String, String)>(
        r#"SELECT name, version, status, tags_json, profile_json, created_at
           FROM behavior_profiles
           WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load behavior profile before patch: {err}"),
        )
    })?;
    let Some((
        current_name,
        current_version,
        current_status,
        current_tags_json,
        current_profile_json,
        created_at,
    )) = row
    else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("behavior profile not found: {profile_id}"),
        ));
    };

    let name = payload.name.unwrap_or(current_name);
    require_non_empty(&name, "behavior profile name")?;
    let status = normalize_resource_status(Some(
        payload.status.as_deref().unwrap_or(current_status.as_str()),
    ))
    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let tags_json = payload.tags_json.or(current_tags_json);
    let profile_json = payload
        .profile_json
        .unwrap_or_else(|| json_string_to_value(&current_profile_json));
    let version = current_version + 1;
    let updated_at = now_ts_string();

    sqlx::query(
        r#"UPDATE behavior_profiles
           SET name = ?, version = ?, status = ?, tags_json = ?, profile_json = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&name)
    .bind(version)
    .bind(&status)
    .bind(&tags_json)
    .bind(profile_json.to_string())
    .bind(&updated_at)
    .bind(&profile_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to patch behavior profile: {err}"),
        )
    })?;

    Ok(Json(BehaviorProfileResponse {
        id: profile_id,
        name,
        version,
        status,
        tags_json,
        profile_json,
        created_at,
        updated_at,
    }))
}

pub async fn delete_behavior_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<BehaviorProfileResponse>, (StatusCode, String)> {
    patch_behavior_profile(
        State(state),
        Path(profile_id),
        Json(UpdateBehaviorProfileRequest {
            name: None,
            status: Some("disabled".to_string()),
            tags_json: None,
            profile_json: None,
        }),
    )
    .await
}

pub async fn create_identity_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreateIdentityProfileRequest>,
) -> Result<(StatusCode, Json<IdentityProfileResponse>), (StatusCode, String)> {
    require_non_empty(&payload.id, "identity profile id")?;
    require_non_empty(&payload.name, "identity profile name")?;
    ensure_active_reference_optional(
        &state,
        "fingerprint_profiles",
        payload.fingerprint_profile_id.as_deref(),
        "fingerprint profile",
    )
    .await?;
    ensure_active_reference_optional(
        &state,
        "behavior_profiles",
        payload.behavior_profile_id.as_deref(),
        "behavior profile",
    )
    .await?;
    ensure_active_reference_optional(
        &state,
        "network_profiles",
        payload.network_profile_id.as_deref(),
        "network profile",
    )
    .await?;

    let now = now_ts_string();
    let status = normalize_resource_status(payload.status.as_deref())
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

    sqlx::query(
        r#"INSERT INTO identity_profiles (
               id, name, version, status, fingerprint_profile_id, behavior_profile_id,
               network_profile_id, identity_json, secret_aliases_json, created_at, updated_at
           ) VALUES (?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(&status)
    .bind(&payload.fingerprint_profile_id)
    .bind(&payload.behavior_profile_id)
    .bind(&payload.network_profile_id)
    .bind(payload.identity_json.to_string())
    .bind(
        payload
            .secret_aliases_json
            .as_ref()
            .map(serde_json::Value::to_string),
    )
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create identity profile: {err}"),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(IdentityProfileResponse {
            id: payload.id,
            name: payload.name,
            version: 1,
            status,
            fingerprint_profile_id: payload.fingerprint_profile_id,
            behavior_profile_id: payload.behavior_profile_id,
            network_profile_id: payload.network_profile_id,
            identity_json: payload.identity_json,
            secret_aliases_json: payload.secret_aliases_json,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_identity_profiles(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<IdentityProfileResponse>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            String,
            i64,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            String,
            String,
        ),
    >(
        r#"SELECT id, name, version, status, fingerprint_profile_id, behavior_profile_id,
                  network_profile_id, identity_json, secret_aliases_json, created_at, updated_at
           FROM identity_profiles
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(sanitize_limit(query.limit, 20, 200))
    .bind(sanitize_offset(query.offset))
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to list identity profiles: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(identity_profile_response_from_tuple)
            .collect(),
    ))
}

pub async fn get_identity_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<IdentityProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            i64,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            String,
            String,
        ),
    >(
        r#"SELECT id, name, version, status, fingerprint_profile_id, behavior_profile_id,
                  network_profile_id, identity_json, secret_aliases_json, created_at, updated_at
           FROM identity_profiles
           WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch identity profile: {err}"),
        )
    })?;

    match row {
        Some(row) => Ok(Json(identity_profile_response_from_tuple(row))),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("identity profile not found: {profile_id}"),
        )),
    }
}

pub async fn patch_identity_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
    Json(payload): Json<UpdateIdentityProfileRequest>,
) -> Result<Json<IdentityProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            i64,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
            String,
        ),
    >(
        r#"SELECT name, version, status, fingerprint_profile_id, behavior_profile_id,
                  network_profile_id, identity_json, secret_aliases_json, created_at
           FROM identity_profiles
           WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load identity profile before patch: {err}"),
        )
    })?;
    let Some((
        current_name,
        current_version,
        current_status,
        current_fingerprint_profile_id,
        current_behavior_profile_id,
        current_network_profile_id,
        current_identity_json,
        current_secret_aliases_json,
        created_at,
    )) = row
    else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("identity profile not found: {profile_id}"),
        ));
    };

    let name = payload.name.unwrap_or(current_name);
    require_non_empty(&name, "identity profile name")?;
    let status = normalize_resource_status(Some(
        payload.status.as_deref().unwrap_or(current_status.as_str()),
    ))
    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let fingerprint_profile_id = payload
        .fingerprint_profile_id
        .or(current_fingerprint_profile_id);
    let behavior_profile_id = payload.behavior_profile_id.or(current_behavior_profile_id);
    let network_profile_id = payload.network_profile_id.or(current_network_profile_id);
    ensure_active_reference_optional(
        &state,
        "fingerprint_profiles",
        fingerprint_profile_id.as_deref(),
        "fingerprint profile",
    )
    .await?;
    ensure_active_reference_optional(
        &state,
        "behavior_profiles",
        behavior_profile_id.as_deref(),
        "behavior profile",
    )
    .await?;
    ensure_active_reference_optional(
        &state,
        "network_profiles",
        network_profile_id.as_deref(),
        "network profile",
    )
    .await?;

    let identity_json = payload
        .identity_json
        .unwrap_or_else(|| json_string_to_value(&current_identity_json));
    let secret_aliases_json = payload
        .secret_aliases_json
        .or_else(|| optional_json_string_to_value(current_secret_aliases_json.as_deref()));
    let version = current_version + 1;
    let updated_at = now_ts_string();

    sqlx::query(
        r#"UPDATE identity_profiles
           SET name = ?, version = ?, status = ?, fingerprint_profile_id = ?,
               behavior_profile_id = ?, network_profile_id = ?, identity_json = ?, secret_aliases_json = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&name)
    .bind(version)
    .bind(&status)
    .bind(&fingerprint_profile_id)
    .bind(&behavior_profile_id)
    .bind(&network_profile_id)
    .bind(identity_json.to_string())
    .bind(secret_aliases_json.as_ref().map(serde_json::Value::to_string))
    .bind(&updated_at)
    .bind(&profile_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to patch identity profile: {err}"),
        )
    })?;

    Ok(Json(IdentityProfileResponse {
        id: profile_id,
        name,
        version,
        status,
        fingerprint_profile_id,
        behavior_profile_id,
        network_profile_id,
        identity_json,
        secret_aliases_json,
        created_at,
        updated_at,
    }))
}

pub async fn delete_identity_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<IdentityProfileResponse>, (StatusCode, String)> {
    patch_identity_profile(
        State(state),
        Path(profile_id),
        Json(UpdateIdentityProfileRequest {
            name: None,
            status: Some("disabled".to_string()),
            fingerprint_profile_id: None,
            behavior_profile_id: None,
            network_profile_id: None,
            identity_json: None,
            secret_aliases_json: None,
        }),
    )
    .await
}

pub async fn create_network_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreateNetworkProfileRequest>,
) -> Result<(StatusCode, Json<NetworkProfileResponse>), (StatusCode, String)> {
    require_non_empty(&payload.id, "network profile id")?;
    require_non_empty(&payload.name, "network profile name")?;

    let now = now_ts_string();
    let status = normalize_resource_status(payload.status.as_deref())
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

    sqlx::query(
        r#"INSERT INTO network_profiles (id, name, version, status, network_policy_json, created_at, updated_at)
           VALUES (?, ?, 1, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(&status)
    .bind(payload.network_policy_json.to_string())
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create network profile: {err}"),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(NetworkProfileResponse {
            id: payload.id,
            name: payload.name,
            version: 1,
            status,
            network_policy_json: payload.network_policy_json,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_network_profiles(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<NetworkProfileResponse>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (String, String, i64, String, String, String, String)>(
        r#"SELECT id, name, version, status, network_policy_json, created_at, updated_at
           FROM network_profiles
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(sanitize_limit(query.limit, 20, 200))
    .bind(sanitize_offset(query.offset))
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to list network profiles: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(network_profile_response_from_tuple)
            .collect(),
    ))
}

pub async fn get_network_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<NetworkProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, String, i64, String, String, String, String)>(
        r#"SELECT id, name, version, status, network_policy_json, created_at, updated_at
           FROM network_profiles
           WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch network profile: {err}"),
        )
    })?;

    match row {
        Some(row) => Ok(Json(network_profile_response_from_tuple(row))),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("network profile not found: {profile_id}"),
        )),
    }
}

pub async fn patch_network_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
    Json(payload): Json<UpdateNetworkProfileRequest>,
) -> Result<Json<NetworkProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, i64, String, String, String)>(
        r#"SELECT name, version, status, network_policy_json, created_at
           FROM network_profiles
           WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load network profile before patch: {err}"),
        )
    })?;
    let Some((current_name, current_version, current_status, current_policy_json, created_at)) =
        row
    else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("network profile not found: {profile_id}"),
        ));
    };

    let name = payload.name.unwrap_or(current_name);
    require_non_empty(&name, "network profile name")?;
    let status = normalize_resource_status(Some(
        payload.status.as_deref().unwrap_or(current_status.as_str()),
    ))
    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let network_policy_json = payload
        .network_policy_json
        .unwrap_or_else(|| json_string_to_value(&current_policy_json));
    let version = current_version + 1;
    let updated_at = now_ts_string();

    sqlx::query(
        r#"UPDATE network_profiles
           SET name = ?, version = ?, status = ?, network_policy_json = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&name)
    .bind(version)
    .bind(&status)
    .bind(network_policy_json.to_string())
    .bind(&updated_at)
    .bind(&profile_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to patch network profile: {err}"),
        )
    })?;

    Ok(Json(NetworkProfileResponse {
        id: profile_id,
        name,
        version,
        status,
        network_policy_json,
        created_at,
        updated_at,
    }))
}

pub async fn delete_network_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<NetworkProfileResponse>, (StatusCode, String)> {
    patch_network_profile(
        State(state),
        Path(profile_id),
        Json(UpdateNetworkProfileRequest {
            name: None,
            status: Some("disabled".to_string()),
            network_policy_json: None,
        }),
    )
    .await
}

pub async fn create_session_profile(
    State(state): State<AppState>,
    Json(payload): Json<CreateSessionProfileRequest>,
) -> Result<(StatusCode, Json<SessionProfileResponse>), (StatusCode, String)> {
    require_non_empty(&payload.id, "session profile id")?;
    require_non_empty(&payload.name, "session profile name")?;
    require_non_empty(&payload.continuity_mode, "continuity_mode")?;

    let now = now_ts_string();
    let status = normalize_resource_status(payload.status.as_deref())
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;

    sqlx::query(
        r#"INSERT INTO session_profiles (
               id, name, version, status, continuity_mode, retention_policy_json, created_at, updated_at
           ) VALUES (?, ?, 1, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&payload.name)
    .bind(&status)
    .bind(&payload.continuity_mode)
    .bind(payload.retention_policy_json.as_ref().map(Value::to_string))
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create session profile: {err}"),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(SessionProfileResponse {
            id: payload.id,
            name: payload.name,
            version: 1,
            status,
            continuity_mode: payload.continuity_mode,
            retention_policy_json: payload.retention_policy_json,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_session_profiles(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<SessionProfileResponse>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<
        _,
        (String, String, i64, String, String, Option<String>, String, String),
    >(
        r#"SELECT id, name, version, status, continuity_mode, retention_policy_json, created_at, updated_at
           FROM session_profiles
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(sanitize_limit(query.limit, 20, 200))
    .bind(sanitize_offset(query.offset))
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to list session profiles: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(session_profile_response_from_tuple)
            .collect(),
    ))
}

pub async fn get_session_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<SessionProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<
        _,
        (String, String, i64, String, String, Option<String>, String, String),
    >(
        r#"SELECT id, name, version, status, continuity_mode, retention_policy_json, created_at, updated_at
           FROM session_profiles
           WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch session profile: {err}"),
        )
    })?;

    match row {
        Some(row) => Ok(Json(session_profile_response_from_tuple(row))),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("session profile not found: {profile_id}"),
        )),
    }
}

pub async fn patch_session_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
    Json(payload): Json<UpdateSessionProfileRequest>,
) -> Result<Json<SessionProfileResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<_, (String, i64, String, String, Option<String>, String)>(
        r#"SELECT name, version, status, continuity_mode, retention_policy_json, created_at
           FROM session_profiles
           WHERE id = ?"#,
    )
    .bind(&profile_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load session profile before patch: {err}"),
        )
    })?;
    let Some((
        current_name,
        current_version,
        current_status,
        current_continuity_mode,
        current_retention_policy_json,
        created_at,
    )) = row
    else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("session profile not found: {profile_id}"),
        ));
    };

    let name = payload.name.unwrap_or(current_name);
    require_non_empty(&name, "session profile name")?;
    let status = normalize_resource_status(Some(
        payload.status.as_deref().unwrap_or(current_status.as_str()),
    ))
    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let continuity_mode = payload.continuity_mode.unwrap_or(current_continuity_mode);
    require_non_empty(&continuity_mode, "continuity_mode")?;
    let retention_policy_json = payload
        .retention_policy_json
        .or_else(|| optional_json_string_to_value(current_retention_policy_json.as_deref()));
    let version = current_version + 1;
    let updated_at = now_ts_string();

    sqlx::query(
        r#"UPDATE session_profiles
           SET name = ?, version = ?, status = ?, continuity_mode = ?, retention_policy_json = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(&name)
    .bind(version)
    .bind(&status)
    .bind(&continuity_mode)
    .bind(retention_policy_json.as_ref().map(Value::to_string))
    .bind(&updated_at)
    .bind(&profile_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to patch session profile: {err}"),
        )
    })?;

    Ok(Json(SessionProfileResponse {
        id: profile_id,
        name,
        version,
        status,
        continuity_mode,
        retention_policy_json,
        created_at,
        updated_at,
    }))
}

pub async fn delete_session_profile(
    State(state): State<AppState>,
    Path(profile_id): Path<String>,
) -> Result<Json<SessionProfileResponse>, (StatusCode, String)> {
    patch_session_profile(
        State(state),
        Path(profile_id),
        Json(UpdateSessionProfileRequest {
            name: None,
            status: Some("disabled".to_string()),
            continuity_mode: None,
            retention_policy_json: None,
        }),
    )
    .await
}

pub async fn create_site_behavior_policy(
    State(state): State<AppState>,
    Json(payload): Json<CreateSiteBehaviorPolicyRequest>,
) -> Result<(StatusCode, Json<SiteBehaviorPolicyResponse>), (StatusCode, String)> {
    require_non_empty(&payload.id, "site behavior policy id")?;
    require_non_empty(&payload.behavior_profile_id, "behavior_profile_id")?;
    let site_key = normalize_site_key(&payload.site_key)?;
    let page_archetype = normalize_page_archetype_input(payload.page_archetype.as_deref())?;
    let action_kind = normalize_optional_text(payload.action_kind);
    ensure_active_reference(
        &state,
        "behavior_profiles",
        &payload.behavior_profile_id,
        "behavior profile",
    )
    .await?;

    let now = now_ts_string();
    let status = normalize_resource_status(payload.status.as_deref())
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let priority = payload.priority.unwrap_or(0);
    let required = payload.required.unwrap_or(false);

    sqlx::query(
        r#"INSERT INTO site_behavior_policies (
               id, version, site_key, page_archetype, action_kind, behavior_profile_id,
               priority, required, override_json, status, created_at, updated_at
           ) VALUES (?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&payload.id)
    .bind(&site_key)
    .bind(&page_archetype)
    .bind(&action_kind)
    .bind(&payload.behavior_profile_id)
    .bind(priority)
    .bind(required)
    .bind(payload.override_json.as_ref().map(Value::to_string))
    .bind(&status)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create site behavior policy: {err}"),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(SiteBehaviorPolicyResponse {
            id: payload.id,
            version: 1,
            site_key,
            page_archetype,
            action_kind,
            behavior_profile_id: payload.behavior_profile_id,
            priority,
            required,
            override_json: payload.override_json,
            status,
            created_at: now.clone(),
            updated_at: now,
        }),
    ))
}

pub async fn list_site_behavior_policies(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Result<Json<Vec<SiteBehaviorPolicyResponse>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<
        _,
        (
            String,
            i64,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            bool,
            Option<String>,
            String,
            String,
            String,
        ),
    >(
        r#"SELECT id, version, site_key, page_archetype, action_kind, behavior_profile_id,
                  priority, required, override_json, status, created_at, updated_at
           FROM site_behavior_policies
           ORDER BY priority DESC, created_at DESC, id DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(sanitize_limit(query.limit, 20, 200))
    .bind(sanitize_offset(query.offset))
    .fetch_all(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to list site behavior policies: {err}"),
        )
    })?;

    Ok(Json(
        rows.into_iter()
            .map(site_behavior_policy_response_from_tuple)
            .collect(),
    ))
}

pub async fn get_site_behavior_policy(
    State(state): State<AppState>,
    Path(policy_id): Path<String>,
) -> Result<Json<SiteBehaviorPolicyResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<
        _,
        (
            String,
            i64,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            bool,
            Option<String>,
            String,
            String,
            String,
        ),
    >(
        r#"SELECT id, version, site_key, page_archetype, action_kind, behavior_profile_id,
                  priority, required, override_json, status, created_at, updated_at
           FROM site_behavior_policies
           WHERE id = ?"#,
    )
    .bind(&policy_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to fetch site behavior policy: {err}"),
        )
    })?;

    match row {
        Some(row) => Ok(Json(site_behavior_policy_response_from_tuple(row))),
        None => Err((
            StatusCode::NOT_FOUND,
            format!("site behavior policy not found: {policy_id}"),
        )),
    }
}

pub async fn patch_site_behavior_policy(
    State(state): State<AppState>,
    Path(policy_id): Path<String>,
    Json(payload): Json<UpdateSiteBehaviorPolicyRequest>,
) -> Result<Json<SiteBehaviorPolicyResponse>, (StatusCode, String)> {
    let row = sqlx::query_as::<
        _,
        (
            i64,
            String,
            Option<String>,
            Option<String>,
            String,
            i64,
            bool,
            Option<String>,
            String,
            String,
        ),
    >(
        r#"SELECT version, site_key, page_archetype, action_kind, behavior_profile_id,
                  priority, required, override_json, status, created_at
           FROM site_behavior_policies
           WHERE id = ?"#,
    )
    .bind(&policy_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to load site behavior policy before patch: {err}"),
        )
    })?;
    let Some((
        current_version,
        current_site_key,
        current_page_archetype,
        current_action_kind,
        current_behavior_profile_id,
        current_priority,
        current_required,
        current_override_json,
        current_status,
        created_at,
    )) = row
    else {
        return Err((
            StatusCode::NOT_FOUND,
            format!("site behavior policy not found: {policy_id}"),
        ));
    };

    let site_key = match payload.site_key {
        Some(site_key) => normalize_site_key(&site_key)?,
        None => current_site_key,
    };
    let page_archetype = match payload.page_archetype {
        Some(page_archetype) => normalize_page_archetype_input(Some(&page_archetype))?,
        None => current_page_archetype,
    };
    let action_kind = match payload.action_kind {
        Some(action_kind) => normalize_optional_text(Some(action_kind)),
        None => current_action_kind,
    };
    let behavior_profile_id = payload
        .behavior_profile_id
        .unwrap_or(current_behavior_profile_id);
    require_non_empty(&behavior_profile_id, "behavior_profile_id")?;
    ensure_active_reference(
        &state,
        "behavior_profiles",
        &behavior_profile_id,
        "behavior profile",
    )
    .await?;
    let priority = payload.priority.unwrap_or(current_priority);
    let required = payload.required.unwrap_or(current_required);
    let override_json = payload
        .override_json
        .or_else(|| optional_json_string_to_value(current_override_json.as_deref()));
    let status = normalize_resource_status(Some(
        payload.status.as_deref().unwrap_or(current_status.as_str()),
    ))
    .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let version = current_version + 1;
    let updated_at = now_ts_string();

    sqlx::query(
        r#"UPDATE site_behavior_policies
           SET version = ?, site_key = ?, page_archetype = ?, action_kind = ?, behavior_profile_id = ?,
               priority = ?, required = ?, override_json = ?, status = ?, updated_at = ?
           WHERE id = ?"#,
    )
    .bind(version)
    .bind(&site_key)
    .bind(&page_archetype)
    .bind(&action_kind)
    .bind(&behavior_profile_id)
    .bind(priority)
    .bind(required)
    .bind(override_json.as_ref().map(Value::to_string))
    .bind(&status)
    .bind(&updated_at)
    .bind(&policy_id)
    .execute(&state.db)
    .await
    .map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to patch site behavior policy: {err}"),
        )
    })?;

    Ok(Json(SiteBehaviorPolicyResponse {
        id: policy_id,
        version,
        site_key,
        page_archetype,
        action_kind,
        behavior_profile_id,
        priority,
        required,
        override_json,
        status,
        created_at,
        updated_at,
    }))
}

pub async fn delete_site_behavior_policy(
    State(state): State<AppState>,
    Path(policy_id): Path<String>,
) -> Result<Json<SiteBehaviorPolicyResponse>, (StatusCode, String)> {
    patch_site_behavior_policy(
        State(state),
        Path(policy_id),
        Json(UpdateSiteBehaviorPolicyRequest {
            site_key: None,
            page_archetype: None,
            action_kind: None,
            behavior_profile_id: None,
            priority: None,
            required: None,
            override_json: None,
            status: Some("disabled".to_string()),
        }),
    )
    .await
}
