use std::{
    fs,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

use crate::app::state::AppState;

static PROXY_RUNTIME_MODE_OVERRIDE: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn proxy_runtime_mode_override_cell() -> &'static Mutex<Option<String>> {
    PROXY_RUNTIME_MODE_OVERRIDE.get_or_init(|| Mutex::new(None))
}

pub fn set_proxy_runtime_mode_override(value: Option<&str>) -> Option<String> {
    let mut guard = proxy_runtime_mode_override_cell()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let previous = guard.clone();
    *guard = value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty());
    previous
}

pub fn proxy_runtime_mode_from_env() -> String {
    let override_value = proxy_runtime_mode_override_cell()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if let Some(value) = override_value {
        return normalize_proxy_runtime_mode(&value);
    }
    let env_value = std::env::var("PERSONA_PILOT_PROXY_MODE").unwrap_or_default();
    normalize_proxy_runtime_mode(&env_value)
}

fn normalize_proxy_runtime_mode(raw: &str) -> String {
    match raw.trim() {
        "prod_live" => "prod_live".to_string(),
        "demo_public" => "demo_public".to_string(),
        _ => "demo_public".to_string(),
    }
}

fn now_ts_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
        .to_string()
}

fn default_config_path() -> String {
    std::env::var("PERSONA_PILOT_PROXY_HARVEST_CONFIG")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "data/proxy_sources.json".to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHarvestRunSummary {
    pub id: String,
    pub source_label: Option<String>,
    pub source_kind: Option<String>,
    pub status: String,
    pub fetched_count: i64,
    pub accepted_count: i64,
    pub deduped_count: i64,
    pub rejected_count: i64,
    pub null_metadata_count: Option<i64>,
    pub active_count_snapshot: Option<i64>,
    pub candidate_count_snapshot: Option<i64>,
    pub candidate_rejected_count_snapshot: Option<i64>,
    pub source_promotion_rate_snapshot: Option<f64>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHarvestSourceSummary {
    pub source_label: String,
    pub source_kind: String,
    pub source_tier: Option<String>,
    pub for_demo: bool,
    pub for_prod: bool,
    pub validation_mode: Option<String>,
    pub declared_geo_quality: Option<String>,
    pub effective_geo_quality: Option<String>,
    pub geo_coverage_percent: f64,
    pub active_region_count: i64,
    pub active_country_count: i64,
    pub active_share_percent: f64,
    pub enabled: bool,
    pub health_score: f64,
    pub candidate_count: i64,
    pub active_count: i64,
    pub candidate_rejected_count: i64,
    pub null_provider_count: i64,
    pub null_region_count: i64,
    pub promotion_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyHarvestMetrics {
    pub source_count: i64,
    pub healthy_source_count: i64,
    pub due_source_count: i64,
    pub source_failures: i64,
    pub source_summaries: Vec<ProxyHarvestSourceSummary>,
    pub recent_harvest_runs: Vec<ProxyHarvestRunSummary>,
}

#[derive(Debug, Clone, Deserialize)]
struct ProxyHarvestSourceConfig {
    source_label: String,
    source_kind: String,
    #[serde(default = "default_source_tier")]
    source_tier: String,
    #[serde(default = "default_for_demo")]
    for_demo: bool,
    #[serde(default = "default_for_prod")]
    for_prod: bool,
    #[serde(default)]
    validation_mode: Option<String>,
    #[serde(default)]
    expected_geo_quality: Option<String>,
    #[serde(default)]
    cost_class: Option<String>,
    #[serde(default = "default_enabled")]
    enabled: bool,
    #[serde(default)]
    config_json: Value,
    #[serde(default)]
    interval_seconds: Option<i64>,
    #[serde(default)]
    base_proxy_score: Option<f64>,
}

fn default_enabled() -> bool {
    true
}

fn default_source_tier() -> String {
    "public".to_string()
}

fn default_for_demo() -> bool {
    true
}

fn default_for_prod() -> bool {
    false
}

#[derive(Debug, Clone)]
struct HarvestSourceRow {
    source_label: String,
    source_kind: String,
    config_json: Value,
    interval_seconds: i64,
    base_proxy_score: f64,
    consecutive_failures: i64,
}

#[derive(Debug, Clone, Default)]
struct HarvestCounters {
    fetched_count: i64,
    accepted_count: i64,
    deduped_count: i64,
    rejected_count: i64,
}

#[derive(Debug, Clone, Default)]
struct HarvestSourceSnapshot {
    total_count: i64,
    candidate_count: i64,
    active_count: i64,
    candidate_rejected_count: i64,
    null_provider_count: i64,
    null_region_count: i64,
    null_country_count: i64,
    connect_failed_count: i64,
    upstream_missing_count: i64,
}

#[derive(Debug, Clone)]
struct CandidateRecord {
    id: String,
    scheme: String,
    host: String,
    port: i64,
    username: Option<String>,
    password: Option<String>,
    provider: Option<String>,
    region: Option<String>,
    country: Option<String>,
    score: f64,
}

fn source_config_object<'a>(config_json: &'a Value) -> Option<&'a serde_json::Map<String, Value>> {
    config_json.as_object()
}

fn source_default_scheme(config: Option<&serde_json::Map<String, Value>>) -> String {
    config
        .and_then(|config| config.get("default_scheme"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("http")
        .to_string()
}

fn merge_default_fields(
    mut record: serde_json::Map<String, Value>,
    config: Option<&serde_json::Map<String, Value>>,
) -> serde_json::Map<String, Value> {
    if !record.contains_key("scheme") {
        record.insert("scheme".to_string(), json!(source_default_scheme(config)));
    }
    if let Some(default_fields) = config
        .and_then(|config| config.get("default_fields"))
        .and_then(Value::as_object)
    {
        for (key, value) in default_fields {
            let should_insert = record.get(key).map(Value::is_null).unwrap_or(true);
            if should_insert {
                record.insert(key.clone(), value.clone());
            }
        }
    }
    record
}

fn source_promotion_rate(snapshot: &HarvestSourceSnapshot) -> f64 {
    let decision_total = snapshot.active_count + snapshot.candidate_rejected_count;
    if decision_total <= 0 {
        0.0
    } else {
        snapshot.active_count as f64 / decision_total as f64
    }
}

fn source_null_metadata_ratio(snapshot: &HarvestSourceSnapshot) -> f64 {
    if snapshot.total_count <= 0 {
        return 0.0;
    }
    let blank_total = snapshot
        .null_provider_count
        .max(snapshot.null_region_count)
        .max(snapshot.null_country_count);
    blank_total as f64 / snapshot.total_count as f64
}

fn round_percentage(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn effective_geo_quality_label(
    declared_geo_quality: Option<&str>,
    externally_verified_active_count: i64,
    host_geo_inferred_active_count: i64,
) -> Option<String> {
    if externally_verified_active_count > 0 {
        Some("externally_verified".to_string())
    } else if host_geo_inferred_active_count > 0 {
        Some("host_geo_inferred".to_string())
    } else {
        declared_geo_quality
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }
}

fn parse_host_port(proxy_ref: &str) -> Result<(String, i64)> {
    let (host_raw, port_raw) = proxy_ref
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("proxy line missing host:port"))?;
    let host = host_raw.trim().trim_matches('[').trim_matches(']');
    if host.is_empty() {
        return Err(anyhow!("proxy line missing host"));
    }
    let port = port_raw
        .trim()
        .parse::<i64>()
        .map_err(|_| anyhow!("proxy line missing valid port"))?;
    Ok((host.to_string(), port))
}

fn parse_source_config(raw: &str) -> Result<Vec<ProxyHarvestSourceConfig>> {
    let parsed = serde_json::from_str::<Value>(raw)?;
    let items = match parsed {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| anyhow!("proxy source config object must contain items[]"))?,
        _ => {
            return Err(anyhow!(
                "proxy source config must be an array or object with items[]"
            ))
        }
    };
    items
        .into_iter()
        .map(serde_json::from_value::<ProxyHarvestSourceConfig>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

async fn sync_sources_from_config(state: &AppState) -> Result<()> {
    let path = default_config_path();
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    let configs = parse_source_config(&raw)?;
    let now = now_ts_string();
    for config in configs {
        let source_tier = config.source_tier.trim();
        let source_tier = if source_tier.is_empty() {
            "public".to_string()
        } else {
            source_tier.to_string()
        };
        let validation_mode = config
            .validation_mode
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let expected_geo_quality = config
            .expected_geo_quality
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let cost_class = config
            .cost_class
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        sqlx::query(
            r#"INSERT INTO proxy_harvest_sources (
                   source_label, source_kind, source_tier, for_demo, for_prod, validation_mode, expected_geo_quality, cost_class,
                   enabled, config_json, interval_seconds, base_proxy_score,
                   consecutive_failures, backoff_until, last_run_started_at, last_run_finished_at,
                   last_run_status, last_error, health_score, created_at, updated_at
               )
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, NULL, NULL, NULL, NULL, NULL, 100.0, ?, ?)
               ON CONFLICT(source_label) DO UPDATE SET
                   source_kind = excluded.source_kind,
                   source_tier = excluded.source_tier,
                   for_demo = excluded.for_demo,
                   for_prod = excluded.for_prod,
                   validation_mode = excluded.validation_mode,
                   expected_geo_quality = excluded.expected_geo_quality,
                   cost_class = excluded.cost_class,
                   enabled = excluded.enabled,
                   config_json = excluded.config_json,
                   interval_seconds = excluded.interval_seconds,
                   base_proxy_score = excluded.base_proxy_score,
                   updated_at = excluded.updated_at"#,
        )
        .bind(&config.source_label)
        .bind(&config.source_kind)
        .bind(&source_tier)
        .bind(if config.for_demo { 1_i64 } else { 0_i64 })
        .bind(if config.for_prod { 1_i64 } else { 0_i64 })
        .bind(validation_mode)
        .bind(expected_geo_quality)
        .bind(cost_class)
        .bind(if config.enabled { 1_i64 } else { 0_i64 })
        .bind(config.config_json.to_string())
        .bind(config.interval_seconds.unwrap_or(300).max(30))
        .bind(config.base_proxy_score.unwrap_or(1.0))
        .bind(&now)
        .bind(&now)
        .execute(&state.db)
        .await?;
    }
    Ok(())
}

async fn due_sources(state: &AppState) -> Result<Vec<HarvestSourceRow>> {
    let now = now_ts_string();
    let rows = sqlx::query(
        r#"SELECT source_label, source_kind, enabled, config_json, interval_seconds, base_proxy_score,
                  consecutive_failures, backoff_until, health_score
           FROM proxy_harvest_sources
           WHERE enabled = 1
             AND (backoff_until IS NULL OR CAST(backoff_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (
               last_run_started_at IS NULL
               OR CAST(last_run_started_at AS INTEGER) <= CAST(? AS INTEGER) - interval_seconds
             )
           ORDER BY health_score DESC, source_label ASC"#,
    )
    .bind(&now)
    .bind(&now)
    .fetch_all(&state.db)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(HarvestSourceRow {
                source_label: row.try_get("source_label")?,
                source_kind: row.try_get("source_kind")?,
                config_json: serde_json::from_str::<Value>(
                    &row.try_get::<String, _>("config_json")?,
                )
                .unwrap_or(Value::Object(Default::default())),
                interval_seconds: row.try_get("interval_seconds")?,
                base_proxy_score: row.try_get("base_proxy_score")?,
                consecutive_failures: row.try_get("consecutive_failures")?,
            })
        })
        .collect()
}

async fn load_source_lines(source: &HarvestSourceRow) -> Result<Vec<String>> {
    let config = source.config_json.as_object().cloned().unwrap_or_default();
    match source.source_kind.as_str() {
        "text_file" => {
            let path = config
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("text_file source requires config_json.path"))?;
            Ok(fs::read_to_string(path)?
                .lines()
                .map(|line| line.to_string())
                .collect())
        }
        "text_url" => {
            let url = config
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("text_url source requires config_json.url"))?;
            let body = reqwest::get(url).await?.text().await?;
            Ok(body.lines().map(|line| line.to_string()).collect())
        }
        "json_file" => {
            let path = config
                .get("path")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("json_file source requires config_json.path"))?;
            let body = fs::read_to_string(path)?;
            extract_json_source_items(&body)
        }
        "json_url" => {
            let url = config
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("json_url source requires config_json.url"))?;
            let body = reqwest::get(url).await?.text().await?;
            extract_json_source_items(&body)
        }
        other => Err(anyhow!("unsupported harvest source kind: {other}")),
    }
}

fn extract_json_source_items(raw: &str) -> Result<Vec<String>> {
    let parsed = serde_json::from_str::<Value>(raw)?;
    let items = match parsed {
        Value::Array(items) => items,
        Value::Object(map) => map
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| anyhow!("json source object must contain items[]"))?,
        _ => {
            return Err(anyhow!(
                "json source must be an array or object with items[]"
            ))
        }
    };
    Ok(items.into_iter().map(|item| item.to_string()).collect())
}

fn parse_proxy_line(
    raw: &str,
    default_score: f64,
    config: Option<&serde_json::Map<String, Value>>,
) -> Result<Option<CandidateRecord>> {
    let line = raw.trim();
    if line.is_empty() || line.starts_with('#') {
        return Ok(None);
    }
    let value = if line.starts_with('{') {
        serde_json::from_str::<Value>(line)?
    } else {
        let mut parts = line.split_whitespace();
        let proxy_ref = parts.next().ok_or_else(|| anyhow!("empty proxy line"))?;
        let mut obj = serde_json::Map::new();
        if let Ok(parsed) = Url::parse(proxy_ref) {
            obj.insert("scheme".to_string(), json!(parsed.scheme()));
            obj.insert(
                "host".to_string(),
                json!(parsed
                    .host_str()
                    .ok_or_else(|| anyhow!("proxy line missing host"))?),
            );
            obj.insert(
                "port".to_string(),
                json!(parsed
                    .port_or_known_default()
                    .ok_or_else(|| anyhow!("proxy line missing port"))?),
            );
            if !parsed.username().is_empty() {
                obj.insert("username".to_string(), json!(parsed.username()));
            }
            if let Some(password) = parsed.password() {
                obj.insert("password".to_string(), json!(password));
            }
        } else {
            let (host, port) = parse_host_port(proxy_ref)?;
            obj.insert("host".to_string(), json!(host));
            obj.insert("port".to_string(), json!(port));
        }
        for token in parts {
            if let Some((key, value)) = token.split_once('=') {
                obj.insert(key.to_string(), json!(value));
            }
        }
        Value::Object(obj)
    };
    let mut value = match value {
        Value::Object(record) => record,
        _ => return Err(anyhow!("proxy candidate must be an object")),
    };
    value = merge_default_fields(value, config);
    let host = value
        .get("host")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("candidate missing host"))?;
    let port = value
        .get("port")
        .and_then(Value::as_i64)
        .ok_or_else(|| anyhow!("candidate missing port"))?;
    let scheme = value
        .get("scheme")
        .and_then(Value::as_str)
        .unwrap_or("http")
        .to_string();
    let username = value
        .get("username")
        .and_then(Value::as_str)
        .map(|v| v.to_string());
    let provider = value
        .get("provider")
        .and_then(Value::as_str)
        .map(|v| v.to_string());
    let region = value
        .get("region")
        .and_then(Value::as_str)
        .map(|v| v.to_string());
    let id = format!("proxy-candidate-{}", Uuid::new_v4().simple());
    Ok(Some(CandidateRecord {
        id,
        scheme,
        host: host.to_string(),
        port,
        username,
        password: value
            .get("password")
            .and_then(Value::as_str)
            .map(|v| v.to_string()),
        provider,
        region,
        country: value
            .get("country")
            .and_then(Value::as_str)
            .map(|v| v.to_string()),
        score: value
            .get("score")
            .and_then(Value::as_f64)
            .unwrap_or(default_score),
    }))
}

async fn load_source_snapshot(
    state: &AppState,
    source_label: &str,
) -> Result<HarvestSourceSnapshot> {
    let row = sqlx::query(
        r#"SELECT
               COUNT(*) AS total_count,
               COALESCE(SUM(CASE WHEN status = 'candidate' THEN 1 ELSE 0 END), 0) AS candidate_count,
               COALESCE(SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END), 0) AS active_count,
               COALESCE(SUM(CASE WHEN status = 'candidate_rejected' THEN 1 ELSE 0 END), 0) AS candidate_rejected_count,
               COALESCE(SUM(CASE WHEN provider IS NULL OR TRIM(provider) = '' THEN 1 ELSE 0 END), 0) AS null_provider_count,
               COALESCE(SUM(CASE WHEN region IS NULL OR TRIM(region) = '' THEN 1 ELSE 0 END), 0) AS null_region_count,
               COALESCE(SUM(CASE WHEN country IS NULL OR TRIM(country) = '' THEN 1 ELSE 0 END), 0) AS null_country_count,
               COALESCE(SUM(CASE WHEN status = 'candidate_rejected' AND last_probe_error_category = 'connect_failed' THEN 1 ELSE 0 END), 0) AS connect_failed_count,
               COALESCE(SUM(CASE WHEN status = 'candidate_rejected' AND last_probe_error_category = 'upstream_missing' THEN 1 ELSE 0 END), 0) AS upstream_missing_count
           FROM proxies
           WHERE source_label = ?"#,
    )
    .bind(source_label)
    .fetch_one(&state.db)
    .await?;

    Ok(HarvestSourceSnapshot {
        total_count: row.try_get("total_count")?,
        candidate_count: row.try_get("candidate_count")?,
        active_count: row.try_get("active_count")?,
        candidate_rejected_count: row.try_get("candidate_rejected_count")?,
        null_provider_count: row.try_get("null_provider_count")?,
        null_region_count: row.try_get("null_region_count")?,
        null_country_count: row.try_get("null_country_count")?,
        connect_failed_count: row.try_get("connect_failed_count")?,
        upstream_missing_count: row.try_get("upstream_missing_count")?,
    })
}

fn compute_source_health_score(
    _source: &HarvestSourceRow,
    counters: &HarvestCounters,
    snapshot: &HarvestSourceSnapshot,
    status: &str,
    consecutive_failures: i64,
) -> f64 {
    let fetch_total = counters.accepted_count + counters.deduped_count;
    let accepted_ratio = if fetch_total <= 0 {
        0.0
    } else {
        counters.accepted_count as f64 / fetch_total as f64
    };
    let rejected_ratio = if counters.fetched_count <= 0 {
        0.0
    } else {
        counters.rejected_count as f64 / counters.fetched_count as f64
    };
    let null_ratio = source_null_metadata_ratio(snapshot);
    let connect_failed_ratio = if snapshot.candidate_rejected_count <= 0 {
        0.0
    } else {
        snapshot.connect_failed_count as f64 / snapshot.candidate_rejected_count as f64
    };
    let upstream_missing_ratio = if snapshot.candidate_rejected_count <= 0 {
        0.0
    } else {
        snapshot.upstream_missing_count as f64 / snapshot.candidate_rejected_count as f64
    };
    let promotion_rate = source_promotion_rate(snapshot);
    let mut score = 55.0
        + accepted_ratio * 12.0
        + promotion_rate * 28.0
        + (snapshot.active_count.min(10) as f64)
        - rejected_ratio * 16.0
        - null_ratio * 24.0
        - connect_failed_ratio * 10.0
        - upstream_missing_ratio * 6.0
        - (consecutive_failures as f64 * 8.0);
    if status == "failed" {
        score -= 10.0;
    } else if status == "partial" {
        score -= 4.0;
    }
    score.clamp(0.0, 100.0)
}

async fn upsert_candidate(
    state: &AppState,
    source: &HarvestSourceRow,
    record: CandidateRecord,
    counters: &mut HarvestCounters,
) -> Result<()> {
    let now = now_ts_string();
    let existing = {
        let exact = sqlx::query(
            r#"SELECT id, status
               FROM proxies
               WHERE scheme = ?
                 AND host = ?
                 AND port = ?
                 AND ((username IS NULL AND ? IS NULL) OR username = ?)
                 AND ((provider IS NULL AND ? IS NULL) OR provider = ?)
                 AND ((region IS NULL AND ? IS NULL) OR region = ?)
               ORDER BY
                 CASE status
                   WHEN 'active' THEN 0
                   WHEN 'candidate' THEN 1
                   WHEN 'candidate_rejected' THEN 2
                   ELSE 3
                 END ASC,
                 created_at ASC
               LIMIT 1"#,
        )
        .bind(&record.scheme)
        .bind(&record.host)
        .bind(record.port)
        .bind(&record.username)
        .bind(&record.username)
        .bind(&record.provider)
        .bind(&record.provider)
        .bind(&record.region)
        .bind(&record.region)
        .fetch_optional(&state.db)
        .await?;
        if exact.is_some() {
            exact
        } else {
            sqlx::query(
                r#"SELECT id, status
                   FROM proxies
                   WHERE scheme = ?
                     AND host = ?
                     AND port = ?
                     AND ((username IS NULL AND ? IS NULL) OR username = ?)
                     AND (
                       provider IS NULL OR TRIM(provider) = ''
                       OR (? IS NULL)
                       OR provider = ?
                     )
                     AND (
                       region IS NULL OR TRIM(region) = ''
                       OR (? IS NULL)
                       OR region = ?
                     )
                   ORDER BY
                     CASE
                       WHEN (provider IS NULL OR TRIM(provider) = '' OR region IS NULL OR TRIM(region) = '') THEN 0
                       ELSE 1
                     END ASC,
                     CASE status
                       WHEN 'active' THEN 0
                       WHEN 'candidate' THEN 1
                       WHEN 'candidate_rejected' THEN 2
                       ELSE 3
                     END ASC,
                     created_at ASC
                   LIMIT 1"#,
            )
            .bind(&record.scheme)
            .bind(&record.host)
            .bind(record.port)
            .bind(&record.username)
            .bind(&record.username)
            .bind(&record.provider)
            .bind(&record.provider)
            .bind(&record.region)
            .bind(&record.region)
            .fetch_optional(&state.db)
            .await?
        }
    };

    if let Some(existing) = existing {
        let existing_id: String = existing.try_get("id")?;
        sqlx::query(
            r#"UPDATE proxies
               SET last_seen_at = ?,
                   source_label = CASE
                     WHEN source_label IS NULL OR TRIM(source_label) = '' THEN ?
                     ELSE source_label
                   END,
                   score = MAX(score, ?),
                   provider = CASE
                     WHEN provider IS NULL OR TRIM(provider) = '' THEN ?
                     ELSE provider
                   END,
                   region = CASE
                     WHEN region IS NULL OR TRIM(region) = '' THEN ?
                     ELSE region
                   END,
                   country = CASE
                     WHEN country IS NULL OR TRIM(country) = '' THEN ?
                     ELSE country
                   END,
                   password = COALESCE(password, ?),
                   updated_at = ?
               WHERE id = ?"#,
        )
        .bind(&now)
        .bind(&source.source_label)
        .bind(record.score)
        .bind(&record.provider)
        .bind(&record.region)
        .bind(&record.country)
        .bind(&record.password)
        .bind(&now)
        .bind(&existing_id)
        .execute(&state.db)
        .await?;
        counters.deduped_count += 1;
        return Ok(());
    }

    sqlx::query(
        r#"INSERT INTO proxies (
               id, scheme, host, port, username, password, region, country, provider, status,
               score, success_count, failure_count, source_label, last_seen_at, created_at, updated_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'candidate', ?, 0, 0, ?, ?, ?, ?)"#,
    )
    .bind(&record.id)
    .bind(&record.scheme)
    .bind(&record.host)
    .bind(record.port)
    .bind(&record.username)
    .bind(&record.password)
    .bind(&record.region)
    .bind(&record.country)
    .bind(&record.provider)
    .bind(record.score)
    .bind(&source.source_label)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&state.db)
    .await?;
    counters.accepted_count += 1;
    Ok(())
}

async fn finalize_harvest_source_run(
    state: &AppState,
    source: &HarvestSourceRow,
    run_id: &str,
    started_at: &str,
    counters: &HarvestCounters,
    status: &str,
    last_error: Option<String>,
) -> Result<ProxyHarvestRunSummary> {
    let finished_at = now_ts_string();
    let failure = status == "failed";
    let consecutive_failures = if failure {
        source.consecutive_failures + 1
    } else {
        0
    };
    let backoff_until = if failure {
        let backoff = (source.interval_seconds * (1_i64 << source.consecutive_failures.min(4)))
            .clamp(source.interval_seconds, 3600);
        Some(
            (finished_at.parse::<i64>().unwrap_or(0) + backoff)
                .max(0)
                .to_string(),
        )
    } else {
        None
    };
    let snapshot = load_source_snapshot(state, &source.source_label).await?;
    let promotion_rate = source_promotion_rate(&snapshot);
    let null_metadata_count = snapshot
        .null_provider_count
        .max(snapshot.null_region_count)
        .max(snapshot.null_country_count);
    let health_score =
        compute_source_health_score(source, counters, &snapshot, status, consecutive_failures);
    let summary_json = json!({
        "source_label": source.source_label,
        "source_kind": source.source_kind,
        "fetched": counters.fetched_count,
        "accepted": counters.accepted_count,
        "deduped": counters.deduped_count,
        "rejected": counters.rejected_count,
        "null_metadata_count": null_metadata_count,
        "active_count_snapshot": snapshot.active_count,
        "candidate_count_snapshot": snapshot.candidate_count,
        "candidate_rejected_count_snapshot": snapshot.candidate_rejected_count,
        "source_promotion_rate_snapshot": promotion_rate,
        "error": last_error,
    });
    sqlx::query(
        r#"INSERT INTO proxy_harvest_runs (
               id, source_label, source_kind, fetched_count, accepted_count, deduped_count,
               rejected_count, status, summary_json, started_at, finished_at
           ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(run_id)
    .bind(&source.source_label)
    .bind(&source.source_kind)
    .bind(counters.fetched_count)
    .bind(counters.accepted_count)
    .bind(counters.deduped_count)
    .bind(counters.rejected_count)
    .bind(status)
    .bind(summary_json.to_string())
    .bind(started_at)
    .bind(&finished_at)
    .execute(&state.db)
    .await?;
    sqlx::query(
        r#"UPDATE proxy_harvest_sources
           SET consecutive_failures = ?, backoff_until = ?, last_run_started_at = ?, last_run_finished_at = ?,
               last_run_status = ?, last_error = ?, health_score = ?, updated_at = ?
           WHERE source_label = ?"#,
    )
    .bind(consecutive_failures)
    .bind(&backoff_until)
    .bind(started_at)
    .bind(&finished_at)
    .bind(status)
    .bind(last_error)
    .bind(health_score)
    .bind(&finished_at)
    .bind(&source.source_label)
    .execute(&state.db)
    .await?;
    Ok(ProxyHarvestRunSummary {
        id: run_id.to_string(),
        source_label: Some(source.source_label.clone()),
        source_kind: Some(source.source_kind.clone()),
        status: status.to_string(),
        fetched_count: counters.fetched_count,
        accepted_count: counters.accepted_count,
        deduped_count: counters.deduped_count,
        rejected_count: counters.rejected_count,
        null_metadata_count: Some(null_metadata_count),
        active_count_snapshot: Some(snapshot.active_count),
        candidate_count_snapshot: Some(snapshot.candidate_count),
        candidate_rejected_count_snapshot: Some(snapshot.candidate_rejected_count),
        source_promotion_rate_snapshot: Some(promotion_rate),
        started_at: started_at.to_string(),
        finished_at: Some(finished_at),
    })
}

pub async fn run_proxy_harvest_tick(state: &AppState) -> Result<Vec<ProxyHarvestRunSummary>> {
    sync_sources_from_config(state).await?;
    let mut summaries = Vec::new();
    for source in due_sources(state).await? {
        let run_id = format!("proxy-harvest-{}", Uuid::new_v4());
        let started_at = now_ts_string();
        let mut counters = HarvestCounters::default();
        let source_config = source_config_object(&source.config_json);
        let result = async {
            for line in load_source_lines(&source).await? {
                let parsed = parse_proxy_line(&line, source.base_proxy_score, source_config)?;
                let Some(record) = parsed else {
                    continue;
                };
                counters.fetched_count += 1;
                upsert_candidate(state, &source, record, &mut counters).await?;
            }
            Ok::<(), anyhow::Error>(())
        }
        .await;

        match result {
            Ok(()) => summaries.push(
                finalize_harvest_source_run(
                    state,
                    &source,
                    &run_id,
                    &started_at,
                    &counters,
                    "completed",
                    None,
                )
                .await?,
            ),
            Err(err) => summaries.push(
                finalize_harvest_source_run(
                    state,
                    &source,
                    &run_id,
                    &started_at,
                    &counters,
                    if counters.fetched_count > 0 {
                        "partial"
                    } else {
                        "failed"
                    },
                    Some(err.to_string()),
                )
                .await?,
            ),
        }
    }
    Ok(summaries)
}

pub async fn load_proxy_harvest_metrics(state: &AppState) -> Result<ProxyHarvestMetrics> {
    sync_sources_from_config(state).await?;
    let now = now_ts_string();
    let source_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM proxy_harvest_sources")
        .fetch_one(&state.db)
        .await?;
    let healthy_source_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proxy_harvest_sources WHERE enabled = 1 AND COALESCE(health_score, 0) >= 60",
    )
    .fetch_one(&state.db)
    .await?;
    let due_source_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM proxy_harvest_sources
           WHERE enabled = 1
             AND (backoff_until IS NULL OR CAST(backoff_until AS INTEGER) <= CAST(? AS INTEGER))
             AND (
               last_run_started_at IS NULL
               OR CAST(last_run_started_at AS INTEGER) <= CAST(? AS INTEGER) - interval_seconds
             )"#,
    )
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;
    let source_failures: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM proxy_harvest_sources WHERE enabled = 1 AND consecutive_failures > 0",
    )
    .fetch_one(&state.db)
    .await?;
    let source_rows = sqlx::query(
        r#"SELECT s.source_label, s.source_kind, s.source_tier, s.for_demo, s.for_prod,
                  s.validation_mode, s.expected_geo_quality, s.enabled, s.health_score,
                  COALESCE(SUM(CASE WHEN p.status = 'candidate' THEN 1 ELSE 0 END), 0) AS candidate_count,
                  COALESCE(SUM(CASE WHEN p.status = 'active' THEN 1 ELSE 0 END), 0) AS active_count,
                  COALESCE(SUM(CASE WHEN p.status = 'candidate_rejected' THEN 1 ELSE 0 END), 0) AS candidate_rejected_count,
                  COALESCE(SUM(CASE WHEN p.provider IS NULL OR TRIM(p.provider) = '' THEN 1 ELSE 0 END), 0) AS null_provider_count,
                  COALESCE(SUM(CASE WHEN p.region IS NULL OR TRIM(p.region) = '' THEN 1 ELSE 0 END), 0) AS null_region_count,
                  COUNT(DISTINCT CASE WHEN p.status = 'active' AND p.region IS NOT NULL AND TRIM(p.region) != '' THEN LOWER(TRIM(p.region)) END) AS active_region_count,
                  COUNT(DISTINCT CASE WHEN p.status = 'active' AND p.country IS NOT NULL AND TRIM(p.country) != '' THEN UPPER(TRIM(p.country)) END) AS active_country_count,
                  COALESCE(SUM(CASE
                    WHEN p.status = 'active'
                     AND p.last_verify_status = 'ok'
                     AND COALESCE(p.last_verify_source, '') IN ('local_verify', 'runner_verify', 'external_probe_v2')
                    THEN 1 ELSE 0 END), 0) AS externally_verified_active_count,
                  COALESCE(SUM(CASE
                    WHEN p.status = 'active'
                     AND p.last_verify_status = 'ok'
                     AND COALESCE(p.last_verify_source, '') LIKE '%geoip_host_enrich%'
                    THEN 1 ELSE 0 END), 0) AS host_geo_inferred_active_count
           FROM proxy_harvest_sources s
           LEFT JOIN proxies p ON p.source_label = s.source_label
           GROUP BY s.source_label, s.source_kind, s.source_tier, s.for_demo, s.for_prod,
                    s.validation_mode, s.expected_geo_quality, s.enabled, s.health_score
           ORDER BY s.health_score DESC, active_count DESC, candidate_count DESC, s.source_label ASC"#,
    )
    .fetch_all(&state.db)
    .await?;
    let total_active = source_rows
        .iter()
        .map(|row| row.try_get::<i64, _>("active_count").unwrap_or(0))
        .sum::<i64>();
    let source_summaries = source_rows
        .into_iter()
        .map(|row| {
            let active_count = row.try_get("active_count").unwrap_or(0);
            let candidate_rejected_count = row.try_get("candidate_rejected_count").unwrap_or(0);
            let decision_total = active_count + candidate_rejected_count;
            let declared_geo_quality = row
                .try_get::<Option<String>, _>("expected_geo_quality")
                .ok()
                .flatten()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            let externally_verified_active_count = row
                .try_get::<i64, _>("externally_verified_active_count")
                .unwrap_or(0);
            let host_geo_inferred_active_count = row
                .try_get::<i64, _>("host_geo_inferred_active_count")
                .unwrap_or(0);
            let geo_coverage_percent = if active_count <= 0 {
                0.0
            } else {
                round_percentage(
                    (externally_verified_active_count as f64 / active_count as f64) * 100.0,
                )
            };
            let active_share_percent = if total_active <= 0 {
                0.0
            } else {
                round_percentage((active_count as f64 / total_active as f64) * 100.0)
            };
            ProxyHarvestSourceSummary {
                source_label: row.try_get("source_label").unwrap_or_default(),
                source_kind: row.try_get("source_kind").unwrap_or_default(),
                source_tier: row
                    .try_get::<Option<String>, _>("source_tier")
                    .ok()
                    .flatten()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                for_demo: row.try_get::<i64, _>("for_demo").unwrap_or(1) != 0,
                for_prod: row.try_get::<i64, _>("for_prod").unwrap_or(0) != 0,
                validation_mode: row
                    .try_get::<Option<String>, _>("validation_mode")
                    .ok()
                    .flatten()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                declared_geo_quality: declared_geo_quality.clone(),
                effective_geo_quality: effective_geo_quality_label(
                    declared_geo_quality.as_deref(),
                    externally_verified_active_count,
                    host_geo_inferred_active_count,
                ),
                geo_coverage_percent,
                active_region_count: row.try_get("active_region_count").unwrap_or(0),
                active_country_count: row.try_get("active_country_count").unwrap_or(0),
                active_share_percent,
                enabled: row.try_get::<i64, _>("enabled").unwrap_or(0) != 0,
                health_score: row.try_get("health_score").unwrap_or(0.0),
                candidate_count: row.try_get("candidate_count").unwrap_or(0),
                active_count,
                candidate_rejected_count,
                null_provider_count: row.try_get("null_provider_count").unwrap_or(0),
                null_region_count: row.try_get("null_region_count").unwrap_or(0),
                promotion_rate: if decision_total <= 0 {
                    0.0
                } else {
                    active_count as f64 / decision_total as f64
                },
            }
        })
        .collect();
    let recent_rows = sqlx::query(
        r#"SELECT id, source_label, source_kind, status, fetched_count, accepted_count, deduped_count,
                  rejected_count, summary_json, started_at, finished_at
           FROM proxy_harvest_runs
           ORDER BY CAST(started_at AS INTEGER) DESC, id DESC
           LIMIT 5"#,
    )
    .fetch_all(&state.db)
    .await?;
    let recent_harvest_runs = recent_rows
        .into_iter()
        .map(|row| {
            let summary_json = row
                .try_get::<Option<String>, _>("summary_json")
                .ok()
                .flatten()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok());
            ProxyHarvestRunSummary {
                id: row.try_get("id").unwrap_or_default(),
                source_label: row.try_get("source_label").ok(),
                source_kind: row.try_get("source_kind").ok(),
                status: row
                    .try_get("status")
                    .unwrap_or_else(|_| "unknown".to_string()),
                fetched_count: row.try_get("fetched_count").unwrap_or(0),
                accepted_count: row.try_get("accepted_count").unwrap_or(0),
                deduped_count: row.try_get("deduped_count").unwrap_or(0),
                rejected_count: row.try_get("rejected_count").unwrap_or(0),
                null_metadata_count: summary_json
                    .as_ref()
                    .and_then(|value| value.get("null_metadata_count"))
                    .and_then(Value::as_i64),
                active_count_snapshot: summary_json
                    .as_ref()
                    .and_then(|value| value.get("active_count_snapshot"))
                    .and_then(Value::as_i64),
                candidate_count_snapshot: summary_json
                    .as_ref()
                    .and_then(|value| value.get("candidate_count_snapshot"))
                    .and_then(Value::as_i64),
                candidate_rejected_count_snapshot: summary_json
                    .as_ref()
                    .and_then(|value| value.get("candidate_rejected_count_snapshot"))
                    .and_then(Value::as_i64),
                source_promotion_rate_snapshot: summary_json
                    .as_ref()
                    .and_then(|value| value.get("source_promotion_rate_snapshot"))
                    .and_then(Value::as_f64),
                started_at: row.try_get("started_at").unwrap_or_default(),
                finished_at: row.try_get("finished_at").ok(),
            }
        })
        .collect();

    Ok(ProxyHarvestMetrics {
        source_count,
        healthy_source_count,
        due_source_count,
        source_failures,
        source_summaries,
        recent_harvest_runs,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        parse_proxy_line, upsert_candidate, CandidateRecord, HarvestCounters, HarvestSourceRow,
    };
    use crate::{app::build_app_state, db::init::init_db, runner::fake::FakeRunner};
    use serde_json::{json, Value};

    fn unique_db_url() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        format!("sqlite:///tmp/persona_pilot_proxy_harvest_test_{nanos}.db")
    }

    #[test]
    fn parse_proxy_line_supports_bare_host_port_with_defaults() {
        let config = json!({
            "default_scheme": "http",
            "default_fields": {
                "provider": "github_seed",
                "region": "global",
                "country": "unknown"
            }
        });
        let parsed = parse_proxy_line("141.98.153.86:80", 0.55, config.as_object())
            .expect("parse should succeed")
            .expect("candidate should exist");
        assert_eq!(parsed.scheme, "http");
        assert_eq!(parsed.host, "141.98.153.86");
        assert_eq!(parsed.port, 80);
        assert_eq!(parsed.provider.as_deref(), Some("github_seed"));
        assert_eq!(parsed.region.as_deref(), Some("global"));
        assert_eq!(parsed.country.as_deref(), Some("unknown"));
        assert!((parsed.score - 0.55).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_proxy_line_preserves_explicit_fields_over_defaults() {
        let config = json!({
            "default_scheme": "http",
            "default_fields": {
                "provider": "github_seed",
                "region": "global"
            }
        });
        let parsed = parse_proxy_line(
            r#"{"host":"1.2.3.4","port":8080,"scheme":"https","provider":"explicit","region":"us"}"#,
            1.0,
            config.as_object(),
        )
        .expect("parse should succeed")
        .expect("candidate should exist");
        assert_eq!(parsed.scheme, "https");
        assert_eq!(parsed.provider.as_deref(), Some("explicit"));
        assert_eq!(parsed.region.as_deref(), Some("us"));
    }

    #[test]
    fn parse_proxy_line_skips_comments() {
        let config = Value::Object(Default::default());
        let parsed =
            parse_proxy_line("# comment", 1.0, config.as_object()).expect("parse should not fail");
        assert!(parsed.is_none());
    }

    #[tokio::test]
    async fn upsert_candidate_backfills_blank_metadata_for_existing_candidate() {
        let db_url = unique_db_url();
        let db = init_db(&db_url).await.expect("init db");
        let state = build_app_state(db.clone(), std::sync::Arc::new(FakeRunner), None, 1);
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   success_count, failure_count, source_label, created_at, updated_at
               ) VALUES (
                   'proxy-existing-candidate', 'http', '1.2.3.4', 8080, NULL, NULL, NULL, NULL, NULL, 'candidate', 0.5,
                   0, 0, 'source-a', '1', '1'
               )"#,
        )
        .execute(&state.db)
        .await
        .expect("insert candidate");

        let source = HarvestSourceRow {
            source_label: "source-a".to_string(),
            source_kind: "text_url".to_string(),
            config_json: Value::Null,
            interval_seconds: 300,
            base_proxy_score: 0.5,
            consecutive_failures: 0,
        };
        let record = CandidateRecord {
            id: "proxy-candidate-updated".to_string(),
            scheme: "http".to_string(),
            host: "1.2.3.4".to_string(),
            port: 8080,
            username: None,
            password: None,
            provider: Some("provider-a".to_string()),
            region: Some("us-east".to_string()),
            country: Some("US".to_string()),
            score: 0.7,
        };

        let mut counters = HarvestCounters::default();
        upsert_candidate(&state, &source, record, &mut counters)
            .await
            .expect("upsert candidate");

        let stored: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            r#"SELECT provider, region, country FROM proxies WHERE id = 'proxy-existing-candidate'"#,
        )
        .fetch_one(&state.db)
        .await
        .expect("load candidate");
        assert_eq!(stored.0.as_deref(), Some("provider-a"));
        assert_eq!(stored.1.as_deref(), Some("us-east"));
        assert_eq!(stored.2.as_deref(), Some("US"));
        assert_eq!(counters.deduped_count, 1);
    }

    #[tokio::test]
    async fn upsert_candidate_backfills_blank_metadata_for_existing_active_proxy() {
        let db_url = unique_db_url();
        let db = init_db(&db_url).await.expect("init db");
        let state = build_app_state(db.clone(), std::sync::Arc::new(FakeRunner), None, 1);
        sqlx::query(
            r#"INSERT INTO proxies (
                   id, scheme, host, port, username, password, region, country, provider, status, score,
                   success_count, failure_count, source_label, created_at, updated_at
               ) VALUES (
                   'proxy-existing-active', 'http', '5.6.7.8', 8081, NULL, NULL, NULL, NULL, NULL, 'active', 0.9,
                   0, 0, 'source-a', '1', '1'
               )"#,
        )
        .execute(&state.db)
        .await
        .expect("insert active proxy");

        let source = HarvestSourceRow {
            source_label: "source-a".to_string(),
            source_kind: "text_url".to_string(),
            config_json: Value::Null,
            interval_seconds: 300,
            base_proxy_score: 0.5,
            consecutive_failures: 0,
        };
        let record = CandidateRecord {
            id: "proxy-candidate-updated".to_string(),
            scheme: "http".to_string(),
            host: "5.6.7.8".to_string(),
            port: 8081,
            username: None,
            password: None,
            provider: Some("provider-b".to_string()),
            region: Some("eu-west".to_string()),
            country: Some("GB".to_string()),
            score: 0.95,
        };

        let mut counters = HarvestCounters::default();
        upsert_candidate(&state, &source, record, &mut counters)
            .await
            .expect("upsert active proxy");

        let stored: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            r#"SELECT provider, region, country FROM proxies WHERE id = 'proxy-existing-active'"#,
        )
        .fetch_one(&state.db)
        .await
        .expect("load active proxy");
        assert_eq!(stored.0.as_deref(), Some("provider-b"));
        assert_eq!(stored.1.as_deref(), Some("eu-west"));
        assert_eq!(stored.2.as_deref(), Some("GB"));
        assert_eq!(counters.deduped_count, 1);
    }
}
