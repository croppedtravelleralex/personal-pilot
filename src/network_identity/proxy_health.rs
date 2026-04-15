use std::{env, time::Duration};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::FromRow;
use tokio::{
    net::TcpStream,
    time::{sleep, timeout, Instant},
};
use uuid::Uuid;

use crate::{app::state::AppState, db::init::DbPool};

const DEFAULT_PROXY_HEALTH_INTERVAL_SECONDS: u64 = 60 * 60;
const DEFAULT_PROXY_HEALTH_STALE_AFTER_SECONDS: i64 = 2 * 60 * 60;
const DEFAULT_PROXY_HEALTH_BATCH_LIMIT: i64 = 12;
const DEFAULT_PROXY_HEALTH_INTER_PROXY_DELAY_MS: u64 = 2_500;
const DEFAULT_PROXY_HEALTH_PROBE_TIMEOUT_SECONDS: u64 = 5;

const FULL_PROFILE_WEIGHTS: [(f64, &str); 6] = [
    (0.16, "identity_score"),
    (0.20, "privacy_score"),
    (0.24, "fraud_score"),
    (0.10, "mail_reputation_score"),
    (0.14, "network_quality_score"),
    (0.16, "site_access_score"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHealthComponentScores {
    pub identity_score: f64,
    pub privacy_score: f64,
    pub fraud_score: f64,
    pub mail_reputation_score: f64,
    pub network_quality_score: f64,
    pub site_access_score: f64,
    pub browser_privacy_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHealthComputation {
    pub proxy_id: String,
    pub overall_score: f64,
    pub grade: String,
    pub grade_summary: String,
    pub tone: String,
    pub checked_at: String,
    pub components: ProxyHealthComponentScores,
    pub probe_ok: bool,
    pub probe_latency_ms: Option<i64>,
    pub probe_error: Option<String>,
    pub summary_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHealthTickResult {
    pub proxy_id: String,
    pub grade: String,
    pub overall_score: f64,
    pub probe_ok: bool,
}

#[derive(Debug, Clone, FromRow)]
struct ProxyHealthTarget {
    id: String,
    scheme: String,
    host: String,
    port: i64,
    region: Option<String>,
    country: Option<String>,
    provider: Option<String>,
    status: String,
    success_count: i64,
    failure_count: i64,
    last_smoke_protocol_ok: Option<i64>,
    last_smoke_upstream_ok: Option<i64>,
    last_anonymity_level: Option<String>,
    last_verify_status: Option<String>,
    last_verify_geo_match_ok: Option<i64>,
    last_exit_country: Option<String>,
    last_exit_region: Option<String>,
    last_probe_latency_ms: Option<i64>,
    last_probe_error: Option<String>,
    last_probe_error_category: Option<String>,
    cached_trust_score: Option<i64>,
    source_label: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct ProxySiteAggregate {
    success_total: i64,
    failure_total: i64,
    site_count: i64,
}

#[derive(Debug, Clone, FromRow)]
struct SnapshotHistoryRow {
    probe_ok: i64,
}

pub fn proxy_health_tick_interval_seconds_from_env() -> u64 {
    env::var("AOB_PROXY_HEALTH_TICK_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_PROXY_HEALTH_INTERVAL_SECONDS)
}

pub fn proxy_health_stale_after_seconds_from_env() -> i64 {
    env::var("AOB_PROXY_HEALTH_STALE_AFTER_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_PROXY_HEALTH_STALE_AFTER_SECONDS)
}

pub fn proxy_health_batch_limit_from_env() -> i64 {
    env::var("AOB_PROXY_HEALTH_BATCH_LIMIT")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_PROXY_HEALTH_BATCH_LIMIT)
}

pub fn proxy_health_inter_proxy_delay_ms_from_env() -> u64 {
    env::var("AOB_PROXY_HEALTH_INTER_PROXY_DELAY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(DEFAULT_PROXY_HEALTH_INTER_PROXY_DELAY_MS)
}

pub async fn run_proxy_health_tick(state: &AppState) -> Result<Vec<ProxyHealthTickResult>> {
    let stale_after = proxy_health_stale_after_seconds_from_env();
    let due_ids =
        select_due_proxy_ids(&state.db, stale_after, proxy_health_batch_limit_from_env()).await?;
    let total_due = due_ids.len();
    let mut results = Vec::new();
    for (index, proxy_id) in due_ids.into_iter().enumerate() {
        let computation = refresh_proxy_health_for_proxy(&state.db, &proxy_id).await?;
        results.push(ProxyHealthTickResult {
            proxy_id: computation.proxy_id.clone(),
            grade: computation.grade.clone(),
            overall_score: computation.overall_score,
            probe_ok: computation.probe_ok,
        });
        if index + 1 < total_due {
            sleep(Duration::from_millis(
                proxy_health_inter_proxy_delay_ms_from_env(),
            ))
            .await;
        }
    }
    Ok(results)
}

pub async fn refresh_proxy_health_for_proxy(
    db: &DbPool,
    proxy_id: &str,
) -> Result<ProxyHealthComputation> {
    let target = load_proxy_health_target(db, proxy_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("proxy not found: {proxy_id}"))?;
    let site_aggregate = load_proxy_site_aggregate(db, proxy_id).await?;
    let history = load_recent_snapshot_history(db, proxy_id).await?;
    let probe = tcp_probe(&target.host, target.port as u16).await;
    let checked_at = now_ts_string();

    let components = ProxyHealthComponentScores {
        identity_score: score_identity(&target),
        privacy_score: score_privacy(&target, probe.error.as_deref()),
        fraud_score: score_fraud(&target, &site_aggregate),
        mail_reputation_score: score_mail_reputation(&target),
        network_quality_score: score_network_quality(&target, &site_aggregate, &history, &probe),
        site_access_score: score_site_access(&target, &site_aggregate),
        browser_privacy_score: None,
    };
    let overall_score = compose_overall_score(&components);
    let (grade, grade_summary, tone) = grade_for_score(overall_score);
    let summary_json = json!({
        "scheme": target.scheme,
        "provider": target.provider,
        "region": target.region,
        "country": target.country,
        "source_label": target.source_label,
        "components": {
            "identity_score": components.identity_score,
            "privacy_score": components.privacy_score,
            "fraud_score": components.fraud_score,
            "mail_reputation_score": components.mail_reputation_score,
            "network_quality_score": components.network_quality_score,
            "site_access_score": components.site_access_score,
            "browser_privacy_score": components.browser_privacy_score,
        },
        "probe": {
            "ok": probe.ok,
            "latency_ms": probe.latency_ms,
            "error": probe.error,
        },
        "site_aggregate": {
            "success_total": site_aggregate.success_total,
            "failure_total": site_aggregate.failure_total,
            "site_count": site_aggregate.site_count,
        },
        "grade_summary": grade_summary,
        "tone": tone,
        "checked_at": checked_at,
    });

    let snapshot_id = format!("phs-{}", Uuid::new_v4().simple());
    sqlx::query(
        r#"INSERT INTO proxy_health_snapshots (
               id, proxy_id, overall_score, grade,
               identity_score, privacy_score, fraud_score, mail_reputation_score,
               network_quality_score, site_access_score, browser_privacy_score,
               probe_ok, probe_latency_ms, error, summary_json, created_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&snapshot_id)
    .bind(&target.id)
    .bind(overall_score)
    .bind(grade)
    .bind(components.identity_score)
    .bind(components.privacy_score)
    .bind(components.fraud_score)
    .bind(components.mail_reputation_score)
    .bind(components.network_quality_score)
    .bind(components.site_access_score)
    .bind(components.browser_privacy_score)
    .bind(probe.ok as i64)
    .bind(probe.latency_ms)
    .bind(&probe.error)
    .bind(summary_json.to_string())
    .bind(&checked_at)
    .execute(db)
    .await?;

    sqlx::query(
        r#"UPDATE proxies
           SET proxy_health_score = ?,
               proxy_health_grade = ?,
               proxy_health_checked_at = ?,
               proxy_health_summary_json = ?,
               updated_at = updated_at
           WHERE id = ?"#,
    )
    .bind(overall_score)
    .bind(grade)
    .bind(&checked_at)
    .bind(summary_json.to_string())
    .bind(&target.id)
    .execute(db)
    .await?;

    Ok(ProxyHealthComputation {
        proxy_id: target.id,
        overall_score,
        grade: grade.to_string(),
        grade_summary: grade_summary.to_string(),
        tone: tone.to_string(),
        checked_at,
        components,
        probe_ok: probe.ok,
        probe_latency_ms: probe.latency_ms,
        probe_error: probe.error,
        summary_json,
    })
}

async fn select_due_proxy_ids(
    db: &DbPool,
    stale_after_seconds: i64,
    limit: i64,
) -> Result<Vec<String>> {
    let stale_before = now_ts_i64() - stale_after_seconds;
    let rows = sqlx::query_scalar::<_, String>(
        r#"SELECT id
           FROM proxies
           WHERE status = 'active'
           ORDER BY
             CASE
               WHEN proxy_health_checked_at IS NULL THEN 0
               WHEN CAST(proxy_health_checked_at AS INTEGER) <= ? THEN 1
               ELSE 2
             END ASC,
             CASE WHEN last_used_at IS NULL THEN 1 ELSE 0 END ASC,
             COALESCE(last_used_at, '0') DESC,
             updated_at DESC,
             id DESC
           LIMIT ?"#,
    )
    .bind(stale_before.to_string())
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

async fn load_proxy_health_target(
    db: &DbPool,
    proxy_id: &str,
) -> Result<Option<ProxyHealthTarget>> {
    let row = sqlx::query_as::<_, ProxyHealthTarget>(
        r#"SELECT
               id, scheme, host, port, region, country, provider, status,
               success_count, failure_count, last_smoke_protocol_ok, last_smoke_upstream_ok,
               last_anonymity_level, last_verify_status, last_verify_geo_match_ok,
               last_exit_country, last_exit_region, last_probe_latency_ms, last_probe_error,
               last_probe_error_category, cached_trust_score, source_label
           FROM proxies
           WHERE id = ?"#,
    )
    .bind(proxy_id)
    .fetch_optional(db)
    .await?;
    Ok(row)
}

async fn load_proxy_site_aggregate(db: &DbPool, proxy_id: &str) -> Result<ProxySiteAggregate> {
    let row = sqlx::query_as::<_, ProxySiteAggregate>(
        r#"SELECT
               COALESCE(SUM(success_count), 0) AS success_total,
               COALESCE(SUM(failure_count), 0) AS failure_total,
               COUNT(DISTINCT site_key) AS site_count
           FROM proxy_site_stats
           WHERE proxy_id = ?"#,
    )
    .bind(proxy_id)
    .fetch_one(db)
    .await?;
    Ok(row)
}

async fn load_recent_snapshot_history(
    db: &DbPool,
    proxy_id: &str,
) -> Result<Vec<SnapshotHistoryRow>> {
    let rows = sqlx::query_as::<_, SnapshotHistoryRow>(
        r#"SELECT probe_ok
           FROM proxy_health_snapshots
           WHERE proxy_id = ?
           ORDER BY created_at DESC
           LIMIT 5"#,
    )
    .bind(proxy_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

#[derive(Debug, Clone)]
struct ProbeResult {
    ok: bool,
    latency_ms: Option<i64>,
    error: Option<String>,
}

async fn tcp_probe(host: &str, port: u16) -> ProbeResult {
    let start = Instant::now();
    let connect = timeout(
        Duration::from_secs(DEFAULT_PROXY_HEALTH_PROBE_TIMEOUT_SECONDS),
        TcpStream::connect((host, port)),
    )
    .await;
    match connect {
        Ok(Ok(stream)) => {
            drop(stream);
            ProbeResult {
                ok: true,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error: None,
            }
        }
        Ok(Err(err)) => ProbeResult {
            ok: false,
            latency_ms: None,
            error: Some(err.to_string()),
        },
        Err(_) => ProbeResult {
            ok: false,
            latency_ms: None,
            error: Some("probe_timeout".to_string()),
        },
    }
}

fn score_identity(target: &ProxyHealthTarget) -> f64 {
    average(
        &[
            score_bool(
                target.region.as_deref().is_some() || target.country.as_deref().is_some(),
                90.0,
                45.0,
            ),
            score_optional_bool(
                target.last_verify_geo_match_ok.map(|value| value != 0),
                100.0,
                35.0,
            ),
            score_bool(
                target.last_exit_country.as_deref().is_some()
                    || target.last_exit_region.as_deref().is_some(),
                82.0,
                55.0,
            ),
            score_bool(
                target.provider.as_deref().is_some() || target.source_label.as_deref().is_some(),
                85.0,
                60.0,
            ),
        ],
        60.0,
    )
}

fn score_privacy(target: &ProxyHealthTarget, probe_error: Option<&str>) -> f64 {
    let anonymity_score = match target
        .last_anonymity_level
        .as_deref()
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("elite") | Some("high_anonymous") => Some(96.0),
        Some("anonymous") => Some(84.0),
        Some("transparent") => Some(28.0),
        Some(_) => Some(68.0),
        None => None,
    };
    let probe_error_score = match probe_error
        .or(target.last_probe_error_category.as_deref())
        .map(|value| value.to_ascii_lowercase())
        .as_deref()
    {
        Some("probe_timeout") => Some(32.0),
        Some("connect_failed") => Some(26.0),
        Some("protocol_error") => Some(22.0),
        Some("upstream_missing") => Some(38.0),
        Some(_) => Some(48.0),
        None => Some(86.0),
    };
    average(
        &[
            anonymity_score,
            score_optional_bool(
                target.last_smoke_protocol_ok.map(|value| value != 0),
                92.0,
                36.0,
            ),
            score_optional_bool(
                target.last_smoke_upstream_ok.map(|value| value != 0),
                88.0,
                40.0,
            ),
            probe_error_score,
        ],
        70.0,
    )
}

fn score_fraud(target: &ProxyHealthTarget, site: &ProxySiteAggregate) -> f64 {
    let base_ratio = success_ratio_score(target.success_count, target.failure_count, 68.0);
    let site_ratio = success_ratio_score(site.success_total, site.failure_total, 70.0);
    average(
        &[
            target
                .cached_trust_score
                .map(|value| clamp_score(value as f64))
                .or(Some(62.0)),
            Some(base_ratio),
            Some(site_ratio),
            match target.last_verify_status.as_deref() {
                Some("ok") => Some(92.0),
                Some("failed") => Some(28.0),
                Some(_) => Some(58.0),
                None => None,
            },
        ],
        65.0,
    )
}

fn score_mail_reputation(target: &ProxyHealthTarget) -> f64 {
    let baseline = if matches!(target.last_verify_status.as_deref(), Some("ok")) {
        64.0
    } else if matches!(target.last_verify_status.as_deref(), Some("failed")) {
        44.0
    } else {
        60.0
    };
    clamp_score(baseline)
}

fn score_network_quality(
    target: &ProxyHealthTarget,
    site: &ProxySiteAggregate,
    history: &[SnapshotHistoryRow],
    probe: &ProbeResult,
) -> f64 {
    let latency = probe.latency_ms.or(target.last_probe_latency_ms);
    let latency_score = latency.map(latency_to_score);
    let history_stability = if history.is_empty() {
        None
    } else {
        let ok_count = history.iter().filter(|row| row.probe_ok != 0).count() as f64;
        Some((ok_count / history.len() as f64) * 100.0)
    };
    let operational_stability = Some(success_ratio_score(
        target.success_count + site.success_total,
        target.failure_count + site.failure_total,
        66.0,
    ));
    average(
        &[
            latency_score,
            history_stability.or(operational_stability),
            score_optional_bool(
                target.last_smoke_protocol_ok.map(|value| value != 0),
                96.0,
                34.0,
            ),
            score_bool(
                site.site_count > 0,
                84.0,
                if probe.ok { 72.0 } else { 42.0 },
            ),
        ],
        65.0,
    )
}

fn score_site_access(target: &ProxyHealthTarget, site: &ProxySiteAggregate) -> f64 {
    let site_ratio = success_ratio_score(site.success_total, site.failure_total, 60.0);
    average(
        &[
            Some(site_ratio),
            Some(match site.site_count {
                count if count >= 3 => 92.0,
                2 => 84.0,
                1 => 72.0,
                _ => 55.0,
            }),
            Some(match target.status.as_str() {
                "active" => 82.0,
                "candidate" => 64.0,
                _ => 42.0,
            }),
        ],
        60.0,
    )
}

fn compose_overall_score(components: &ProxyHealthComponentScores) -> f64 {
    let values = [
        ("identity_score", components.identity_score),
        ("privacy_score", components.privacy_score),
        ("fraud_score", components.fraud_score),
        ("mail_reputation_score", components.mail_reputation_score),
        ("network_quality_score", components.network_quality_score),
        ("site_access_score", components.site_access_score),
    ];
    let total = FULL_PROFILE_WEIGHTS
        .iter()
        .map(|(weight, key)| {
            let value = values
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| *value)
                .unwrap_or(60.0);
            value * weight
        })
        .sum::<f64>();
    clamp_score((total * 10.0).round() / 10.0)
}

fn grade_for_score(score: f64) -> (&'static str, &'static str, &'static str) {
    if score >= 90.0 {
        ("A+", "极其稳定，适合长期白名单使用", "ok")
    } else if score >= 80.0 {
        ("A", "整体健康，适合稳定任务", "ok")
    } else if score >= 70.0 {
        ("B+", "质量较好，可作为主力候选", "info")
    } else if score >= 60.0 {
        ("B", "中等偏稳，需要继续观察", "info")
    } else if score >= 50.0 {
        ("C+", "存在明显风险，建议谨慎使用", "warn")
    } else if score >= 40.0 {
        ("C", "风险偏高，应降低优先级", "warn")
    } else if score >= 30.0 {
        ("D", "高风险，建议替换", "danger")
    } else {
        ("F", "极高风险，基本不可用", "danger")
    }
}

fn score_optional_bool(value: Option<bool>, success: f64, failure: f64) -> Option<f64> {
    value.map(|flag| if flag { success } else { failure })
}

fn score_bool(value: bool, success: f64, failure: f64) -> Option<f64> {
    Some(if value { success } else { failure })
}

fn average(values: &[Option<f64>], fallback: f64) -> f64 {
    let usable = values
        .iter()
        .flatten()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if usable.is_empty() {
        return fallback;
    }
    clamp_score((usable.iter().sum::<f64>() / usable.len() as f64 * 10.0).round() / 10.0)
}

fn success_ratio_score(success: i64, failure: i64, fallback: f64) -> f64 {
    let total = success + failure;
    if total <= 0 {
        return fallback;
    }
    clamp_score((success as f64 / total as f64) * 100.0)
}

fn latency_to_score(latency_ms: i64) -> f64 {
    clamp_score(100.0 - (latency_ms as f64 / 10.0).min(100.0))
}

fn clamp_score(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn now_ts_i64() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn now_ts_string() -> String {
    now_ts_i64().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::build_app_state, db::init::init_db, runner::fake::FakeRunner};
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::net::TcpListener;

    #[test]
    fn score_grade_thresholds_match_expected_buckets() {
        assert_eq!(grade_for_score(91.0).0, "A+");
        assert_eq!(grade_for_score(84.0).0, "A");
        assert_eq!(grade_for_score(73.0).0, "B+");
        assert_eq!(grade_for_score(61.0).0, "B");
        assert_eq!(grade_for_score(54.0).0, "C+");
        assert_eq!(grade_for_score(42.0).0, "C");
        assert_eq!(grade_for_score(31.0).0, "D");
        assert_eq!(grade_for_score(10.0).0, "F");
    }

    #[test]
    fn compose_overall_score_uses_full_profile_weights() {
        let components = ProxyHealthComponentScores {
            identity_score: 80.0,
            privacy_score: 70.0,
            fraud_score: 60.0,
            mail_reputation_score: 50.0,
            network_quality_score: 90.0,
            site_access_score: 75.0,
            browser_privacy_score: None,
        };
        assert_eq!(compose_overall_score(&components), 69.1);
    }

    #[tokio::test]
    async fn proxy_health_tick_writes_snapshot_and_denormalized_fields() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("proxy-health.db");
        let db = init_db(&format!("sqlite://{}", db_path.to_string_lossy()))
            .await
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, region, country, provider, status, score,
                   success_count, failure_count, last_smoke_protocol_ok, last_smoke_upstream_ok,
                   last_anonymity_level, last_verify_status, last_verify_geo_match_ok,
                   last_exit_country, last_exit_region, cached_trust_score, source_label,
                   created_at, updated_at
               ) VALUES (
                   'proxy-health-1', 'http', '127.0.0.1', ?, 'us-east', 'US', 'demo', 'active', 1.0,
                   10, 1, 1, 1,
                   'elite', 'ok', 1,
                   'US', 'Virginia', 88, 'demo-source',
                   '1', '1'
               )"#,
        )
        .bind(i64::from(port))
        .execute(&db)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO proxy_site_stats (
                   proxy_id, site_key, success_count, failure_count, updated_at
               ) VALUES ('proxy-health-1', 'example.com', 4, 1, '1')"#,
        )
        .execute(&db)
        .await
        .unwrap();

        let state = build_app_state(db.clone(), Arc::new(FakeRunner), None, 1);
        let results = run_proxy_health_tick(&state).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].proxy_id, "proxy-health-1");
        let stored = sqlx::query_as::<_, (Option<f64>, Option<String>, Option<String>)>(
            r#"SELECT proxy_health_score, proxy_health_grade, proxy_health_checked_at
               FROM proxies WHERE id = 'proxy-health-1'"#,
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert!(stored.0.unwrap_or_default() > 70.0);
        assert!(matches!(
            stored.1.as_deref(),
            Some("A+") | Some("A") | Some("B+")
        ));
        assert!(stored.2.is_some());
        let snapshot_count = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM proxy_health_snapshots WHERE proxy_id = 'proxy-health-1'"#,
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(snapshot_count, 1);
    }
}
