use std::path::Path;

use anyhow::Result;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Pool, Sqlite,
};
use tokio::fs;

use super::schema::ALL_SCHEMA_SQL;
use crate::behavior::{system_default_behavior_profile, RESOURCE_STATUS_ACTIVE};

pub type DbPool = Pool<Sqlite>;

fn perf_probe_enabled() -> bool {
    std::env::var("PP_PERF_PROBE")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

fn perf_probe_log(event: &str, fields: &[(&str, String)]) {
    if !perf_probe_enabled() {
        return;
    }
    let detail = fields
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join(" ");
    if detail.is_empty() {
        eprintln!("perf_probe event={}", event);
    } else {
        eprintln!("perf_probe event={} {}", event, detail);
    }
}

async fn ensure_column_exists(
    pool: &DbPool,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<()> {
    let pragma = format!("PRAGMA table_info({})", table);
    let rows = sqlx::query_as::<_, (i64, String, String, i64, Option<String>, i64)>(&pragma)
        .fetch_all(pool)
        .await?;
    let exists = rows.into_iter().any(|(_, name, _, _, _, _)| name == column);
    if !exists {
        sqlx::query(alter_sql).execute(pool).await?;
    }
    Ok(())
}

async fn ensure_sqlite_parent_dir(database_url: &str) -> Result<()> {
    if let Some(path_str) = database_url.strip_prefix("sqlite://") {
        let path = Path::new(path_str);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).await?;
            }
        }
    }
    Ok(())
}

pub async fn init_db(database_url: &str) -> Result<DbPool> {
    ensure_sqlite_parent_dir(database_url).await?;

    let options = database_url
        .parse::<SqliteConnectOptions>()?
        .create_if_missing(true)
        .busy_timeout(std::time::Duration::from_secs(5))
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    for stmt in ALL_SCHEMA_SQL {
        sqlx::query(stmt).execute(&pool).await?;
    }

    ensure_column_exists(
        &pool,
        "tasks",
        "runner_id",
        "ALTER TABLE tasks ADD COLUMN runner_id TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "heartbeat_at",
        "ALTER TABLE tasks ADD COLUMN heartbeat_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "behavior_policy_json",
        "ALTER TABLE tasks ADD COLUMN behavior_policy_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "form_input_redacted_json",
        "ALTER TABLE tasks ADD COLUMN form_input_redacted_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "execution_intent_json",
        "ALTER TABLE tasks ADD COLUMN execution_intent_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "fingerprint_profile_id",
        "ALTER TABLE tasks ADD COLUMN fingerprint_profile_id TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "fingerprint_profile_version",
        "ALTER TABLE tasks ADD COLUMN fingerprint_profile_version INTEGER",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "identity_profile_id",
        "ALTER TABLE tasks ADD COLUMN identity_profile_id TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "behavior_profile_id",
        "ALTER TABLE tasks ADD COLUMN behavior_profile_id TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "behavior_profile_version",
        "ALTER TABLE tasks ADD COLUMN behavior_profile_version INTEGER",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "network_profile_id",
        "ALTER TABLE tasks ADD COLUMN network_profile_id TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "tasks",
        "session_profile_id",
        "ALTER TABLE tasks ADD COLUMN session_profile_id TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "identity_profiles",
        "secret_aliases_json",
        "ALTER TABLE identity_profiles ADD COLUMN secret_aliases_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "site_behavior_policies",
        "version",
        "ALTER TABLE site_behavior_policies ADD COLUMN version INTEGER NOT NULL DEFAULT 1",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "runs",
        "result_json",
        "ALTER TABLE runs ADD COLUMN result_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "last_probe_latency_ms",
        "ALTER TABLE proxies ADD COLUMN last_probe_latency_ms INTEGER",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "last_probe_error",
        "ALTER TABLE proxies ADD COLUMN last_probe_error TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "last_probe_error_category",
        "ALTER TABLE proxies ADD COLUMN last_probe_error_category TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "last_verify_confidence",
        "ALTER TABLE proxies ADD COLUMN last_verify_confidence REAL",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "last_verify_score_delta",
        "ALTER TABLE proxies ADD COLUMN last_verify_score_delta INTEGER",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "last_verify_source",
        "ALTER TABLE proxies ADD COLUMN last_verify_source TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "cached_trust_score",
        "ALTER TABLE proxies ADD COLUMN cached_trust_score INTEGER",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "trust_score_cached_at",
        "ALTER TABLE proxies ADD COLUMN trust_score_cached_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "provider_risk_version_seen",
        "ALTER TABLE proxies ADD COLUMN provider_risk_version_seen INTEGER",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "source_label",
        "ALTER TABLE proxies ADD COLUMN source_label TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "proxy_health_score",
        "ALTER TABLE proxies ADD COLUMN proxy_health_score REAL",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "proxy_health_grade",
        "ALTER TABLE proxies ADD COLUMN proxy_health_grade TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "proxy_health_checked_at",
        "ALTER TABLE proxies ADD COLUMN proxy_health_checked_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "proxy_health_summary_json",
        "ALTER TABLE proxies ADD COLUMN proxy_health_summary_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "last_seen_at",
        "ALTER TABLE proxies ADD COLUMN last_seen_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxies",
        "promoted_at",
        "ALTER TABLE proxies ADD COLUMN promoted_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "fingerprint_profile_id",
        "ALTER TABLE proxy_session_bindings ADD COLUMN fingerprint_profile_id TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "site_key",
        "ALTER TABLE proxy_session_bindings ADD COLUMN site_key TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "requested_region",
        "ALTER TABLE proxy_session_bindings ADD COLUMN requested_region TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "requested_provider",
        "ALTER TABLE proxy_session_bindings ADD COLUMN requested_provider TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "cookies_json",
        "ALTER TABLE proxy_session_bindings ADD COLUMN cookies_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "cookie_updated_at",
        "ALTER TABLE proxy_session_bindings ADD COLUMN cookie_updated_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "local_storage_json",
        "ALTER TABLE proxy_session_bindings ADD COLUMN local_storage_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "session_storage_json",
        "ALTER TABLE proxy_session_bindings ADD COLUMN session_storage_json TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "storage_updated_at",
        "ALTER TABLE proxy_session_bindings ADD COLUMN storage_updated_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "last_success_at",
        "ALTER TABLE proxy_session_bindings ADD COLUMN last_success_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "proxy_session_bindings",
        "last_failure_at",
        "ALTER TABLE proxy_session_bindings ADD COLUMN last_failure_at TEXT",
    )
    .await?;
    ensure_column_exists(
        &pool,
        "provider_risk_snapshots",
        "version",
        "ALTER TABLE provider_risk_snapshots ADD COLUMN version INTEGER NOT NULL DEFAULT 1",
    )
    .await?;
    ensure_system_default_behavior_profile(&pool).await?;
    refresh_provider_risk_snapshots(&pool).await?;
    refresh_cached_trust_scores(&pool).await?;

    Ok(pool)
}

async fn ensure_system_default_behavior_profile(pool: &DbPool) -> Result<()> {
    let profile = system_default_behavior_profile();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();

    sqlx::query(
        r#"INSERT INTO behavior_profiles (id, name, version, status, tags_json, profile_json, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)
           ON CONFLICT(id) DO UPDATE SET
             version = excluded.version,
             status = excluded.status,
             tags_json = excluded.tags_json,
             profile_json = excluded.profile_json,
             updated_at = excluded.updated_at"#,
    )
    .bind(&profile.id)
    .bind("System Default Browser V1")
    .bind(profile.version)
    .bind(RESOURCE_STATUS_ACTIVE)
    .bind(Some(r#"["system","default","browser"]"#.to_string()))
    .bind(profile.profile_json.to_string())
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn refresh_provider_risk_snapshots(pool: &DbPool) -> Result<()> {
    let started = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();

    sqlx::query("DELETE FROM provider_risk_snapshots")
        .execute(pool)
        .await?;
    sqlx::query(
        r#"INSERT INTO provider_risk_snapshots (provider, success_count, failure_count, risk_hit, version, updated_at)
           SELECT provider, SUM(success_count), SUM(failure_count),
                  CASE
                    WHEN SUM(failure_count) >= SUM(success_count) + 5 THEN 1
                    WHEN SUM(CASE WHEN last_probe_error_category = 'exit_ip_not_public' THEN 1 ELSE 0 END) >= 2 THEN 1
                    ELSE 0
                  END,
                  1,
                  ?
           FROM proxies
           WHERE provider IS NOT NULL
           GROUP BY provider"#,
    )
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM provider_region_risk_snapshots")
        .execute(pool)
        .await?;
    sqlx::query(
        r#"INSERT INTO provider_region_risk_snapshots (provider, region, recent_failed_count, risk_hit, updated_at)
           SELECT provider, region,
                  SUM(CASE WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600 THEN 1 ELSE 0 END),
                  CASE
                    WHEN SUM(CASE WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600 THEN 1 ELSE 0 END) >= 2 THEN 1
                    WHEN SUM(CASE WHEN last_exit_region IS NOT NULL AND region IS NOT NULL AND LOWER(last_exit_region) != LOWER(region) THEN 1 ELSE 0 END) >= 2 THEN 1
                    ELSE 0
                  END,
                  ?
           FROM proxies
           WHERE provider IS NOT NULL
             AND region IS NOT NULL
           GROUP BY provider, region"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    perf_probe_log(
        "refresh_provider_risk_snapshots",
        &[
            ("scope", "all".to_string()),
            ("elapsed_ms", started.elapsed().as_millis().to_string()),
        ],
    );

    Ok(())
}

pub async fn refresh_provider_risk_snapshot_for_provider(
    pool: &DbPool,
    provider: Option<&str>,
) -> Result<()> {
    let Some(provider) = provider else {
        return Ok(());
    };
    let started = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();

    let previous = sqlx::query_as::<_, (i64, i64)>(
        "SELECT risk_hit, version FROM provider_risk_snapshots WHERE provider = ?",
    )
    .bind(provider)
    .fetch_optional(pool)
    .await?;
    let aggregate = sqlx::query_as::<_, (i64, i64, i64)>(
        r#"SELECT COALESCE(SUM(success_count), 0), COALESCE(SUM(failure_count), 0),
                  CASE
                    WHEN COALESCE(SUM(failure_count), 0) >= COALESCE(SUM(success_count), 0) + 5 THEN 1
                    WHEN COALESCE(SUM(CASE WHEN last_probe_error_category = 'exit_ip_not_public' THEN 1 ELSE 0 END), 0) >= 2 THEN 1
                    ELSE 0
                  END
           FROM proxies
           WHERE provider = ?"#,
    )
    .bind(provider)
    .fetch_one(pool)
    .await?;
    let (success_count, failure_count, risk_hit) = aggregate;
    let version = match previous {
        Some((old_hit, old_version)) if old_hit != risk_hit => old_version + 1,
        Some((_, old_version)) => old_version,
        None => 1,
    };

    sqlx::query(
        r#"INSERT INTO provider_risk_snapshots (provider, success_count, failure_count, risk_hit, version, updated_at)
           VALUES (?, ?, ?, ?, ?, ?)
           ON CONFLICT(provider) DO UPDATE SET
             success_count = excluded.success_count,
             failure_count = excluded.failure_count,
             risk_hit = excluded.risk_hit,
             version = excluded.version,
             updated_at = excluded.updated_at"#,
    )
    .bind(provider)
    .bind(success_count)
    .bind(failure_count)
    .bind(risk_hit)
    .bind(version)
    .bind(&now)
    .execute(pool)
    .await?;
    perf_probe_log(
        "refresh_provider_risk_snapshot",
        &[
            ("scope", "provider".to_string()),
            ("provider", provider.to_string()),
            ("risk_hit", risk_hit.to_string()),
            ("version", version.to_string()),
            ("elapsed_ms", started.elapsed().as_millis().to_string()),
        ],
    );
    Ok(())
}

pub async fn refresh_provider_region_risk_snapshot_for_pair(
    pool: &DbPool,
    provider: Option<&str>,
    region: Option<&str>,
) -> Result<()> {
    let (Some(provider), Some(region)) = (provider, region) else {
        return Ok(());
    };
    let started = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();

    sqlx::query("DELETE FROM provider_region_risk_snapshots WHERE provider = ? AND region = ?")
        .bind(provider)
        .bind(region)
        .execute(pool)
        .await?;
    sqlx::query(
        r#"INSERT INTO provider_region_risk_snapshots (provider, region, recent_failed_count, risk_hit, updated_at)
           SELECT provider, region,
                  SUM(CASE WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600 THEN 1 ELSE 0 END),
                  CASE
                    WHEN SUM(CASE WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600 THEN 1 ELSE 0 END) >= 2 THEN 1
                    WHEN SUM(CASE WHEN last_exit_region IS NOT NULL AND region IS NOT NULL AND LOWER(last_exit_region) != LOWER(region) THEN 1 ELSE 0 END) >= 2 THEN 1
                    ELSE 0
                  END,
                  ?
           FROM proxies
           WHERE provider = ?
             AND region = ?
           GROUP BY provider, region"#,
    )
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(provider)
    .bind(region)
    .execute(pool)
    .await?;
    perf_probe_log(
        "refresh_provider_region_risk_snapshot",
        &[
            ("scope", "provider_region".to_string()),
            ("provider", provider.to_string()),
            ("region", region.to_string()),
            ("elapsed_ms", started.elapsed().as_millis().to_string()),
        ],
    );
    Ok(())
}

async fn provider_risk_hit_for_provider(
    pool: &DbPool,
    provider: Option<&str>,
) -> Result<Option<i64>> {
    let Some(provider) = provider else {
        return Ok(None);
    };
    let hit = sqlx::query_scalar::<_, i64>(
        "SELECT risk_hit FROM provider_risk_snapshots WHERE provider = ?",
    )
    .bind(provider)
    .fetch_optional(pool)
    .await?;
    Ok(hit)
}

async fn provider_region_risk_hit_for_pair(
    pool: &DbPool,
    provider: Option<&str>,
    region: Option<&str>,
) -> Result<Option<i64>> {
    let (Some(provider), Some(region)) = (provider, region) else {
        return Ok(None);
    };
    let hit = sqlx::query_scalar::<_, i64>(
        "SELECT risk_hit FROM provider_region_risk_snapshots WHERE provider = ? AND region = ?",
    )
    .bind(provider)
    .bind(region)
    .fetch_optional(pool)
    .await?;
    Ok(hit)
}

pub async fn provider_risk_version_state_for_proxy(
    pool: &DbPool,
    proxy_id: &str,
) -> Result<(Option<i64>, Option<i64>, String)> {
    let row = sqlx::query_as::<_, (Option<String>, Option<i64>)>(
        "SELECT provider, provider_risk_version_seen FROM proxies WHERE id = ?",
    )
    .bind(proxy_id)
    .fetch_optional(pool)
    .await?;
    let Some((provider, seen_version)) = row else {
        return Ok((None, None, "not_applicable".to_string()));
    };
    let Some(provider) = provider else {
        return Ok((None, seen_version, "not_applicable".to_string()));
    };
    let current_version = sqlx::query_scalar::<_, i64>(
        "SELECT version FROM provider_risk_snapshots WHERE provider = ?",
    )
    .bind(provider)
    .fetch_optional(pool)
    .await?;
    let status = match (current_version, seen_version) {
        (Some(current), Some(seen)) if current == seen => "aligned",
        (Some(_), Some(_)) => "stale",
        (Some(_), None) => "stale",
        _ => "not_applicable",
    }
    .to_string();
    Ok((current_version, seen_version, status))
}

pub async fn refresh_proxy_trust_views_for_scope(
    pool: &DbPool,
    proxy_id: &str,
    provider: Option<&str>,
    region: Option<&str>,
) -> Result<()> {
    let provider_risk_before = provider_risk_hit_for_provider(pool, provider).await?;
    let provider_region_risk_before =
        provider_region_risk_hit_for_pair(pool, provider, region).await?;

    refresh_provider_risk_snapshot_for_provider(pool, provider).await?;
    refresh_provider_region_risk_snapshot_for_pair(pool, provider, region).await?;

    let provider_risk_after = provider_risk_hit_for_provider(pool, provider).await?;
    let provider_region_risk_after =
        provider_region_risk_hit_for_pair(pool, provider, region).await?;

    if provider.is_none() {
        perf_probe_log(
            "refresh_proxy_trust_views_for_scope",
            &[
                ("branch", "proxy_only_providerless".to_string()),
                ("proxy_id", proxy_id.to_string()),
            ],
        );
        refresh_cached_trust_score_for_proxy(pool, proxy_id).await?;
    } else if provider_risk_before != provider_risk_after {
        perf_probe_log(
            "refresh_proxy_trust_views_for_scope",
            &[
                ("branch", "provider_scope_flip".to_string()),
                ("mode", "lazy_current_proxy".to_string()),
                ("proxy_id", proxy_id.to_string()),
                ("provider", provider.unwrap_or_default().to_string()),
            ],
        );
        refresh_cached_trust_score_for_proxy(pool, proxy_id).await?;
    } else if provider_region_risk_before != provider_region_risk_after {
        perf_probe_log(
            "refresh_proxy_trust_views_for_scope",
            &[
                ("branch", "provider_region_scope_flip".to_string()),
                ("proxy_id", proxy_id.to_string()),
                ("provider", provider.unwrap_or_default().to_string()),
                ("region", region.unwrap_or_default().to_string()),
            ],
        );
        refresh_cached_trust_scores_for_provider_region(pool, provider, region).await?;
    } else {
        perf_probe_log(
            "refresh_proxy_trust_views_for_scope",
            &[
                ("branch", "proxy_only_no_flip".to_string()),
                ("proxy_id", proxy_id.to_string()),
                ("provider", provider.unwrap_or_default().to_string()),
                ("region", region.unwrap_or_default().to_string()),
            ],
        );
        refresh_cached_trust_score_for_proxy(pool, proxy_id).await?;
    }

    Ok(())
}

fn cached_trust_score_update_sql(where_clause: Option<&str>) -> String {
    let mut sql = r#"UPDATE proxies
           SET cached_trust_score =
                (CASE WHEN last_verify_status = 'ok' THEN 30 ELSE 0 END) +
                (CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 20 ELSE 0 END) +
                (CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 10 ELSE 0 END) -
                (CASE WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 1800 THEN 25
                      WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 7200 THEN 12
                      WHEN last_verify_status = 'failed' THEN 6
                      ELSE 0 END) -
                (CASE WHEN last_verify_at IS NULL THEN 12
                      WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 86400 THEN 8
                      ELSE 0 END) -
                (CASE WHEN failure_count >= success_count + 3 THEN 18
                      WHEN failure_count > success_count THEN 8
                      ELSE 0 END) -
                (CASE WHEN provider IS NOT NULL AND EXISTS (SELECT 1 FROM provider_risk_snapshots prs WHERE prs.provider = proxies.provider AND prs.risk_hit != 0) THEN 10 ELSE 0 END) -
                (CASE WHEN provider IS NOT NULL AND region IS NOT NULL AND EXISTS (SELECT 1 FROM provider_region_risk_snapshots prrs WHERE prrs.provider = proxies.provider AND prrs.region = proxies.region AND prrs.risk_hit != 0) THEN 12 ELSE 0 END) +
                CAST(score * 10 AS INTEGER),
               trust_score_cached_at = ?,
               provider_risk_version_seen = CASE
                   WHEN provider IS NOT NULL THEN (SELECT prs.version FROM provider_risk_snapshots prs WHERE prs.provider = proxies.provider)
                   ELSE NULL
               END"#.to_string();
    if let Some(where_clause) = where_clause {
        sql.push_str(" WHERE ");
        sql.push_str(where_clause);
    }
    sql
}

pub async fn refresh_cached_trust_scores(pool: &DbPool) -> Result<()> {
    let started = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();
    sqlx::query(&cached_trust_score_update_sql(None))
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
    perf_probe_log(
        "refresh_cached_trust_scores",
        &[
            ("scope", "all".to_string()),
            ("elapsed_ms", started.elapsed().as_millis().to_string()),
        ],
    );
    Ok(())
}

pub async fn refresh_cached_trust_score_for_proxy(pool: &DbPool, proxy_id: &str) -> Result<()> {
    let started = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();
    sqlx::query(&cached_trust_score_update_sql(Some("id = ?")))
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(proxy_id)
        .execute(pool)
        .await?;
    perf_probe_log(
        "refresh_cached_trust_scores",
        &[
            ("scope", "proxy".to_string()),
            ("proxy_id", proxy_id.to_string()),
            ("elapsed_ms", started.elapsed().as_millis().to_string()),
        ],
    );
    Ok(())
}

pub async fn refresh_cached_trust_scores_for_provider(
    pool: &DbPool,
    provider: Option<&str>,
) -> Result<()> {
    let Some(provider) = provider else {
        return Ok(());
    };
    let started = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();
    sqlx::query(&cached_trust_score_update_sql(Some("provider = ?")))
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(provider)
        .execute(pool)
        .await?;
    perf_probe_log(
        "refresh_cached_trust_scores",
        &[
            ("scope", "provider".to_string()),
            ("provider", provider.to_string()),
            ("elapsed_ms", started.elapsed().as_millis().to_string()),
        ],
    );
    Ok(())
}

pub async fn refresh_cached_trust_scores_for_provider_region(
    pool: &DbPool,
    provider: Option<&str>,
    region: Option<&str>,
) -> Result<()> {
    let (Some(provider), Some(region)) = (provider, region) else {
        return Ok(());
    };
    let started = std::time::Instant::now();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();
    sqlx::query(&cached_trust_score_update_sql(Some(
        "provider = ? AND region = ?",
    )))
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(provider)
    .bind(region)
    .execute(pool)
    .await?;
    perf_probe_log(
        "refresh_cached_trust_scores",
        &[
            ("scope", "provider_region".to_string()),
            ("provider", provider.to_string()),
            ("region", region.to_string()),
            ("elapsed_ms", started.elapsed().as_millis().to_string()),
        ],
    );
    Ok(())
}

#[cfg(test)]
mod scoped_refresh_tests {
    use super::*;

    fn unique_db_url() -> String {
        format!(
            "sqlite:///tmp/persona_pilot-db-init-test-{}.db",
            uuid::Uuid::new_v4()
        )
    }

    #[tokio::test]
    async fn scoped_trust_refresh_helper_limits_cache_refresh_when_risk_flags_do_not_change() {
        let db_url = unique_db_url();
        let db = init_db(&db_url).await.expect("init db");

        sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                      VALUES
                      ('proxy-risk-same-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-same', 'active', 0.2, 0, 0, NULL, 0, 0, NULL, '1', '1'),
                      ('proxy-risk-same-2', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-west', 'US', 'pool-same', 'active', 0.9, 5, 0, 'ok', 1, 1, '9999999999', '1', '1')"#)
            .execute(&db)
            .await
            .expect("insert proxies");

        refresh_provider_risk_snapshots(&db)
            .await
            .expect("refresh risk snapshots");
        refresh_cached_trust_scores(&db)
            .await
            .expect("refresh all trust cache");
        let before_other: Option<String> = sqlx::query_scalar(
            "SELECT trust_score_cached_at FROM proxies WHERE id = 'proxy-risk-same-2'",
        )
        .fetch_one(&db)
        .await
        .expect("before ts");

        sqlx::query(
            "UPDATE proxies SET score = 0.25, updated_at = '2' WHERE id = 'proxy-risk-same-1'",
        )
        .execute(&db)
        .await
        .expect("update current proxy only");

        refresh_proxy_trust_views_for_scope(
            &db,
            "proxy-risk-same-1",
            Some("pool-same"),
            Some("us-east"),
        )
        .await
        .expect("scoped refresh without risk flip");

        let after_other: Option<String> = sqlx::query_scalar(
            "SELECT trust_score_cached_at FROM proxies WHERE id = 'proxy-risk-same-2'",
        )
        .fetch_one(&db)
        .await
        .expect("after ts");
        assert_eq!(after_other, before_other);
    }

    #[tokio::test]
    async fn scoped_trust_refresh_helper_refreshes_current_proxy_on_provider_flip_and_falls_back_for_providerless_proxy(
    ) {
        let db_url = unique_db_url();
        let db = init_db(&db_url).await.expect("init db");

        sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, username, password, region, country, provider, status, score, success_count, failure_count, last_verify_status, last_verify_geo_match_ok, last_smoke_upstream_ok, last_verify_at, created_at, updated_at)
                      VALUES
                      ('proxy-scope-helper-1', 'http', '127.0.0.1', 8080, NULL, NULL, 'us-east', 'US', 'pool-helper', 'active', 0.3, 0, 0, NULL, 0, 0, NULL, '1', '1'),
                      ('proxy-scope-helper-2', 'http', '127.0.0.2', 8081, NULL, NULL, 'us-west', 'US', 'pool-helper', 'active', 0.3, 5, 0, 'ok', 1, 1, '9999999999', '1', '1'),
                      ('proxy-no-provider', 'http', '127.0.0.3', 8082, NULL, NULL, 'us-east', 'US', NULL, 'active', 0.9, 5, 0, 'ok', 1, 1, '9999999999', '1', '1')"#)
            .execute(&db)
            .await
            .expect("insert proxies");

        refresh_provider_risk_snapshots(&db)
            .await
            .expect("refresh risk snapshots");
        refresh_cached_trust_scores(&db)
            .await
            .expect("refresh initial caches");
        let helper_two_before: Option<String> = sqlx::query_scalar(
            "SELECT trust_score_cached_at FROM proxies WHERE id = 'proxy-scope-helper-2'",
        )
        .fetch_one(&db)
        .await
        .expect("helper2 before ts");
        let helper_two_seen_before: Option<i64> = sqlx::query_scalar(
            "SELECT provider_risk_version_seen FROM proxies WHERE id = 'proxy-scope-helper-2'",
        )
        .fetch_one(&db)
        .await
        .expect("helper2 before seen");

        sqlx::query("UPDATE proxies SET failure_count = 6, updated_at = '2' WHERE id = 'proxy-scope-helper-1'")
            .execute(&db)
            .await
            .expect("make provider risk flip");

        refresh_proxy_trust_views_for_scope(
            &db,
            "proxy-scope-helper-1",
            Some("pool-helper"),
            Some("us-east"),
        )
        .await
        .expect("refresh helper provider scope");
        let helper_one_after: Option<String> = sqlx::query_scalar(
            "SELECT trust_score_cached_at FROM proxies WHERE id = 'proxy-scope-helper-1'",
        )
        .fetch_one(&db)
        .await
        .expect("helper1 after ts");
        let helper_one_seen: Option<i64> = sqlx::query_scalar(
            "SELECT provider_risk_version_seen FROM proxies WHERE id = 'proxy-scope-helper-1'",
        )
        .fetch_one(&db)
        .await
        .expect("helper1 seen");
        let provider_version: i64 = sqlx::query_scalar(
            "SELECT version FROM provider_risk_snapshots WHERE provider = 'pool-helper'",
        )
        .fetch_one(&db)
        .await
        .expect("provider version");
        let helper_two_after: Option<String> = sqlx::query_scalar(
            "SELECT trust_score_cached_at FROM proxies WHERE id = 'proxy-scope-helper-2'",
        )
        .fetch_one(&db)
        .await
        .expect("helper2 after ts");
        let helper_two_seen_after: Option<i64> = sqlx::query_scalar(
            "SELECT provider_risk_version_seen FROM proxies WHERE id = 'proxy-scope-helper-2'",
        )
        .fetch_one(&db)
        .await
        .expect("helper2 after seen");
        assert!(helper_one_after.is_some());
        assert_eq!(helper_one_seen, Some(provider_version));
        assert_eq!(helper_two_after, helper_two_before);
        assert_eq!(helper_two_seen_after, helper_two_seen_before);

        refresh_proxy_trust_views_for_scope(&db, "proxy-no-provider", None, Some("us-east"))
            .await
            .expect("refresh helper providerless fallback");
        let providerless: i64 = sqlx::query_scalar(
            "SELECT COALESCE(cached_trust_score, 0) FROM proxies WHERE id = 'proxy-no-provider'",
        )
        .fetch_one(&db)
        .await
        .expect("providerless cache");
        assert!(providerless > 0);
    }

    #[tokio::test]
    async fn provider_risk_snapshot_version_increments_when_risk_hit_flips() {
        let db_url = unique_db_url();
        let db = init_db(&db_url).await.expect("init db");
        sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, provider, region, country, status, score, success_count, failure_count, created_at, updated_at)
                      VALUES
                      ('proxy-version-1', 'http', '127.0.0.1', 8080, 'pool-version', 'us-east', 'US', 'active', 0.6, 3, 0, '1', '1'),
                      ('proxy-version-2', 'http', '127.0.0.2', 8081, 'pool-version', 'us-west', 'US', 'active', 0.6, 3, 0, '1', '1')"#)
            .execute(&db)
            .await
            .expect("insert proxies");
        refresh_provider_risk_snapshots(&db)
            .await
            .expect("refresh snapshots");
        let before: i64 = sqlx::query_scalar(
            "SELECT version FROM provider_risk_snapshots WHERE provider = 'pool-version'",
        )
        .fetch_one(&db)
        .await
        .expect("version before");

        sqlx::query(
            "UPDATE proxies SET failure_count = 12, updated_at = '2' WHERE id = 'proxy-version-1'",
        )
        .execute(&db)
        .await
        .expect("flip risk");
        refresh_provider_risk_snapshot_for_provider(&db, Some("pool-version"))
            .await
            .expect("refresh provider scoped");

        let after: i64 = sqlx::query_scalar(
            "SELECT version FROM provider_risk_snapshots WHERE provider = 'pool-version'",
        )
        .fetch_one(&db)
        .await
        .expect("version after");
        let hit: i64 = sqlx::query_scalar(
            "SELECT risk_hit FROM provider_risk_snapshots WHERE provider = 'pool-version'",
        )
        .fetch_one(&db)
        .await
        .expect("risk hit after");
        assert_eq!(after, before + 1);
        assert_eq!(hit, 1);
    }

    #[tokio::test]
    async fn provider_risk_version_state_reports_aligned_stale_and_not_applicable() {
        let db_url = unique_db_url();
        let db = init_db(&db_url).await.expect("init db");
        sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, provider, region, country, status, score, success_count, failure_count, created_at, updated_at)
                      VALUES
                      ('proxy-version-state-a', 'http', '127.0.0.1', 8080, 'pool-state', 'us-east', 'US', 'active', 0.6, 3, 0, '1', '1'),
                      ('proxy-version-state-b', 'http', '127.0.0.2', 8081, 'pool-state', 'us-west', 'US', 'active', 0.6, 3, 0, '1', '1'),
                      ('proxy-version-state-none', 'http', '127.0.0.3', 8082, NULL, 'us-west', 'US', 'active', 0.6, 3, 0, '1', '1')"#)
            .execute(&db)
            .await
            .expect("insert proxies");
        refresh_provider_risk_snapshots(&db)
            .await
            .expect("refresh snapshots");
        refresh_cached_trust_scores(&db)
            .await
            .expect("refresh caches");

        let aligned = provider_risk_version_state_for_proxy(&db, "proxy-version-state-a")
            .await
            .expect("aligned state");
        assert_eq!(aligned.2, "aligned");

        sqlx::query("UPDATE provider_risk_snapshots SET version = version + 1 WHERE provider = 'pool-state'")
            .execute(&db)
            .await
            .expect("bump version");
        let stale = provider_risk_version_state_for_proxy(&db, "proxy-version-state-a")
            .await
            .expect("stale state");
        assert_eq!(stale.2, "stale");

        let na = provider_risk_version_state_for_proxy(&db, "proxy-version-state-none")
            .await
            .expect("na state");
        assert_eq!(na.2, "not_applicable");
    }

    #[tokio::test]
    async fn provider_risk_snapshot_hits_on_exit_ip_not_public_cluster() {
        let db_url = unique_db_url();
        let db = init_db(&db_url).await.expect("init db");
        sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, provider, region, country, status, score, success_count, failure_count, last_probe_error_category, created_at, updated_at)
                      VALUES
                      ('proxy-provider-exit-1', 'http', '127.0.0.1', 8080, 'pool-exit', 'us-east', 'US', 'active', 0.6, 1, 0, 'exit_ip_not_public', '1', '1'),
                      ('proxy-provider-exit-2', 'http', '127.0.0.2', 8081, 'pool-exit', 'us-west', 'US', 'active', 0.6, 1, 0, 'exit_ip_not_public', '1', '1')"#)
            .execute(&db)
            .await
            .expect("insert proxies");
        refresh_provider_risk_snapshots(&db)
            .await
            .expect("refresh snapshots");
        let hit: i64 = sqlx::query_scalar(
            "SELECT risk_hit FROM provider_risk_snapshots WHERE provider = 'pool-exit'",
        )
        .fetch_one(&db)
        .await
        .expect("provider risk hit");
        assert_eq!(hit, 1);
    }

    #[tokio::test]
    async fn provider_region_risk_snapshot_hits_on_region_mismatch_cluster() {
        let db_url = unique_db_url();
        let db = init_db(&db_url).await.expect("init db");
        sqlx::query(r#"INSERT INTO proxies (id, scheme, host, port, provider, region, country, status, score, success_count, failure_count, last_exit_region, created_at, updated_at)
                      VALUES
                      ('proxy-region-mismatch-1', 'http', '127.0.0.1', 8080, 'pool-region', 'us-east', 'US', 'active', 0.6, 1, 0, 'Virginia', '1', '1'),
                      ('proxy-region-mismatch-2', 'http', '127.0.0.2', 8081, 'pool-region', 'us-east', 'US', 'active', 0.6, 1, 0, 'Ohio', '1', '1')"#)
            .execute(&db)
            .await
            .expect("insert proxies");
        refresh_provider_risk_snapshots(&db)
            .await
            .expect("refresh snapshots");
        let hit: i64 = sqlx::query_scalar("SELECT risk_hit FROM provider_region_risk_snapshots WHERE provider = 'pool-region' AND region = 'us-east'")
            .fetch_one(&db)
            .await
            .expect("provider region risk hit");
        assert_eq!(hit, 1);
    }
}
