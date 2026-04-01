use std::path::Path;

use anyhow::Result;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Pool, Sqlite};
use tokio::fs;

use super::schema::ALL_SCHEMA_SQL;

pub type DbPool = Pool<Sqlite>;

async fn ensure_column_exists(pool: &DbPool, table: &str, column: &str, alter_sql: &str) -> Result<()> {
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
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    for stmt in ALL_SCHEMA_SQL {
        sqlx::query(stmt).execute(&pool).await?;
    }

    ensure_column_exists(&pool, "tasks", "runner_id", "ALTER TABLE tasks ADD COLUMN runner_id TEXT").await?;
    ensure_column_exists(&pool, "tasks", "heartbeat_at", "ALTER TABLE tasks ADD COLUMN heartbeat_at TEXT").await?;
    ensure_column_exists(&pool, "tasks", "fingerprint_profile_id", "ALTER TABLE tasks ADD COLUMN fingerprint_profile_id TEXT").await?;
    ensure_column_exists(&pool, "tasks", "fingerprint_profile_version", "ALTER TABLE tasks ADD COLUMN fingerprint_profile_version INTEGER").await?;
    ensure_column_exists(&pool, "runs", "result_json", "ALTER TABLE runs ADD COLUMN result_json TEXT").await?;
    ensure_column_exists(&pool, "proxies", "last_probe_latency_ms", "ALTER TABLE proxies ADD COLUMN last_probe_latency_ms INTEGER").await?;
    ensure_column_exists(&pool, "proxies", "last_probe_error", "ALTER TABLE proxies ADD COLUMN last_probe_error TEXT").await?;
    ensure_column_exists(&pool, "proxies", "last_probe_error_category", "ALTER TABLE proxies ADD COLUMN last_probe_error_category TEXT").await?;
    ensure_column_exists(&pool, "proxies", "last_verify_confidence", "ALTER TABLE proxies ADD COLUMN last_verify_confidence REAL").await?;
    ensure_column_exists(&pool, "proxies", "last_verify_score_delta", "ALTER TABLE proxies ADD COLUMN last_verify_score_delta INTEGER").await?;
    ensure_column_exists(&pool, "proxies", "last_verify_source", "ALTER TABLE proxies ADD COLUMN last_verify_source TEXT").await?;
    ensure_column_exists(&pool, "proxies", "cached_trust_score", "ALTER TABLE proxies ADD COLUMN cached_trust_score INTEGER").await?;
    ensure_column_exists(&pool, "proxies", "trust_score_cached_at", "ALTER TABLE proxies ADD COLUMN trust_score_cached_at TEXT").await?;
    refresh_provider_risk_snapshots(&pool).await?;
    refresh_cached_trust_scores(&pool).await?;

    Ok(pool)
}


pub async fn refresh_provider_risk_snapshots(pool: &DbPool) -> Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();

    sqlx::query("DELETE FROM provider_risk_snapshots").execute(pool).await?;
    sqlx::query(
        r#"INSERT INTO provider_risk_snapshots (provider, success_count, failure_count, risk_hit, updated_at)
           SELECT provider, SUM(success_count), SUM(failure_count),
                  CASE WHEN SUM(failure_count) >= SUM(success_count) + 5 THEN 1 ELSE 0 END,
                  ?
           FROM proxies
           WHERE provider IS NOT NULL
           GROUP BY provider"#,
    )
    .bind(&now)
    .execute(pool)
    .await?;

    sqlx::query("DELETE FROM provider_region_risk_snapshots").execute(pool).await?;
    sqlx::query(
        r#"INSERT INTO provider_region_risk_snapshots (provider, region, recent_failed_count, risk_hit, updated_at)
           SELECT provider, region, COUNT(*), CASE WHEN COUNT(*) >= 2 THEN 1 ELSE 0 END, ?
           FROM proxies
           WHERE provider IS NOT NULL
             AND region IS NOT NULL
             AND last_verify_status = 'failed'
             AND last_verify_at IS NOT NULL
             AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600
           GROUP BY provider, region"#,
    )
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}


pub async fn refresh_provider_risk_snapshot_for_provider(pool: &DbPool, provider: Option<&str>) -> Result<()> {
    let Some(provider) = provider else { return Ok(()); };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();

    sqlx::query("DELETE FROM provider_risk_snapshots WHERE provider = ?")
        .bind(provider)
        .execute(pool)
        .await?;
    sqlx::query(
        r#"INSERT INTO provider_risk_snapshots (provider, success_count, failure_count, risk_hit, updated_at)
           SELECT provider, SUM(success_count), SUM(failure_count),
                  CASE WHEN SUM(failure_count) >= SUM(success_count) + 5 THEN 1 ELSE 0 END,
                  ?
           FROM proxies
           WHERE provider = ?
           GROUP BY provider"#,
    )
    .bind(&now)
    .bind(provider)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn refresh_provider_region_risk_snapshot_for_pair(pool: &DbPool, provider: Option<&str>, region: Option<&str>) -> Result<()> {
    let (Some(provider), Some(region)) = (provider, region) else { return Ok(()); };
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
           SELECT provider, region, COUNT(*), CASE WHEN COUNT(*) >= 2 THEN 1 ELSE 0 END, ?
           FROM proxies
           WHERE provider = ?
             AND region = ?
             AND last_verify_status = 'failed'
             AND last_verify_at IS NOT NULL
             AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600
           GROUP BY provider, region"#,
    )
    .bind(&now)
    .bind(provider)
    .bind(region)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(())
}


async fn provider_risk_hit_for_provider(pool: &DbPool, provider: Option<&str>) -> Result<Option<i64>> {
    let Some(provider) = provider else { return Ok(None); };
    let hit = sqlx::query_scalar::<_, i64>("SELECT risk_hit FROM provider_risk_snapshots WHERE provider = ?")
        .bind(provider)
        .fetch_optional(pool)
        .await?;
    Ok(hit)
}

async fn provider_region_risk_hit_for_pair(pool: &DbPool, provider: Option<&str>, region: Option<&str>) -> Result<Option<i64>> {
    let (Some(provider), Some(region)) = (provider, region) else { return Ok(None); };
    let hit = sqlx::query_scalar::<_, i64>("SELECT risk_hit FROM provider_region_risk_snapshots WHERE provider = ? AND region = ?")
        .bind(provider)
        .bind(region)
        .fetch_optional(pool)
        .await?;
    Ok(hit)
}


pub async fn refresh_proxy_trust_views_for_scope(pool: &DbPool, proxy_id: &str, provider: Option<&str>, region: Option<&str>) -> Result<()> {
    let provider_risk_before = provider_risk_hit_for_provider(pool, provider).await?;
    let provider_region_risk_before = provider_region_risk_hit_for_pair(pool, provider, region).await?;

    refresh_provider_risk_snapshot_for_provider(pool, provider).await?;
    refresh_provider_region_risk_snapshot_for_pair(pool, provider, region).await?;

    let provider_risk_after = provider_risk_hit_for_provider(pool, provider).await?;
    let provider_region_risk_after = provider_region_risk_hit_for_pair(pool, provider, region).await?;

    if provider.is_none() {
        refresh_cached_trust_score_for_proxy(pool, proxy_id).await?;
    } else if provider_risk_before != provider_risk_after {
        refresh_cached_trust_scores_for_provider(pool, provider).await?;
    } else if provider_region_risk_before != provider_region_risk_after {
        refresh_cached_trust_scores_for_provider_region(pool, provider, region).await?;
    } else {
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
               trust_score_cached_at = ?"#.to_string();
    if let Some(where_clause) = where_clause {
        sql.push_str(" WHERE ");
        sql.push_str(where_clause);
    }
    sql
}

pub async fn refresh_cached_trust_scores(pool: &DbPool) -> Result<()> {
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
    Ok(())
}

pub async fn refresh_cached_trust_score_for_proxy(pool: &DbPool, proxy_id: &str) -> Result<()> {
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
    Ok(())
}


pub async fn refresh_cached_trust_scores_for_provider(pool: &DbPool, provider: Option<&str>) -> Result<()> {
    let Some(provider) = provider else { return Ok(()); };
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
    Ok(())
}

pub async fn refresh_cached_trust_scores_for_provider_region(pool: &DbPool, provider: Option<&str>, region: Option<&str>) -> Result<()> {
    let (Some(provider), Some(region)) = (provider, region) else { return Ok(()); };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        .to_string();
    sqlx::query(&cached_trust_score_update_sql(Some("provider = ? AND region = ?")))
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(provider)
        .bind(region)
        .execute(pool)
        .await?;
    Ok(())
}


#[cfg(test)]
mod scoped_refresh_tests {
    use super::*;

    fn unique_db_url() -> String {
        format!("sqlite:///tmp/auto_open_browser-db-init-test-{}.db", uuid::Uuid::new_v4())
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

        refresh_provider_risk_snapshots(&db).await.expect("refresh risk snapshots");
        refresh_cached_trust_scores(&db).await.expect("refresh all trust cache");
        let before_other: Option<String> = sqlx::query_scalar("SELECT trust_score_cached_at FROM proxies WHERE id = 'proxy-risk-same-2'")
            .fetch_one(&db)
            .await
            .expect("before ts");

        sqlx::query("UPDATE proxies SET score = 0.25, updated_at = '2' WHERE id = 'proxy-risk-same-1'")
            .execute(&db)
            .await
            .expect("update current proxy only");

        refresh_proxy_trust_views_for_scope(&db, "proxy-risk-same-1", Some("pool-same"), Some("us-east"))
            .await
            .expect("scoped refresh without risk flip");

        let after_other: Option<String> = sqlx::query_scalar("SELECT trust_score_cached_at FROM proxies WHERE id = 'proxy-risk-same-2'")
            .fetch_one(&db)
            .await
            .expect("after ts");
        assert_eq!(after_other, before_other);
    }

    #[tokio::test]
    async fn scoped_trust_refresh_helper_updates_provider_group_and_falls_back_for_providerless_proxy() {
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

        refresh_provider_risk_snapshots(&db).await.expect("refresh risk snapshots");
        refresh_proxy_trust_views_for_scope(&db, "proxy-scope-helper-1", Some("pool-helper"), Some("us-east")).await.expect("refresh helper provider scope");
        let helper_one: i64 = sqlx::query_scalar("SELECT COALESCE(cached_trust_score, 0) FROM proxies WHERE id = 'proxy-scope-helper-1'").fetch_one(&db).await.expect("cache 1");
        let helper_two: i64 = sqlx::query_scalar("SELECT COALESCE(cached_trust_score, 0) FROM proxies WHERE id = 'proxy-scope-helper-2'").fetch_one(&db).await.expect("cache 2");
        assert!(helper_two > helper_one);

        refresh_proxy_trust_views_for_scope(&db, "proxy-no-provider", None, Some("us-east")).await.expect("refresh helper providerless fallback");
        let providerless: i64 = sqlx::query_scalar("SELECT COALESCE(cached_trust_score, 0) FROM proxies WHERE id = 'proxy-no-provider'").fetch_one(&db).await.expect("providerless cache");
        assert!(providerless > 0);
    }
}
