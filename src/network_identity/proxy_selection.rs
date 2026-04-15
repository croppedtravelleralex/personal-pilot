use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxySelectionTuning {
    pub stale_after_seconds: i64,
    pub recent_failure_heavy_window_seconds: i64,
    pub recent_failure_light_window_seconds: i64,
    pub provider_failure_margin: i64,
    pub provider_region_failure_cluster_window_seconds: i64,
    pub provider_region_failure_cluster_count: i64,
    pub raw_score_weight_tenths: i64,
    pub verify_ok_bonus: i64,
    pub verify_geo_match_bonus: i64,
    pub geo_mismatch_penalty: i64,
    pub region_mismatch_penalty: i64,
    pub smoke_upstream_ok_bonus: i64,
    pub verify_failed_heavy_penalty: i64,
    pub verify_failed_light_penalty: i64,
    pub verify_failed_base_penalty: i64,
    pub missing_verify_penalty: i64,
    pub stale_verify_penalty: i64,
    pub anonymity_elite_bonus: i64,
    pub anonymity_anonymous_penalty: i64,
    pub anonymity_transparent_penalty: i64,
    pub low_latency_bonus: i64,
    pub medium_latency_bonus: i64,
    pub high_latency_penalty: i64,
    pub very_high_latency_penalty: i64,
    pub exit_ip_not_public_penalty: i64,
    pub probe_error_protocol_penalty: i64,
    pub probe_error_upstream_missing_penalty: i64,
    pub probe_error_connect_failed_penalty: i64,
    pub site_success_bonus: i64,
    pub site_failure_penalty: i64,
    pub site_browser_failure_penalty: i64,
    pub soft_min_score_penalty: i64,
}

impl Default for ProxySelectionTuning {
    fn default() -> Self {
        Self {
            stale_after_seconds: 3600,
            recent_failure_heavy_window_seconds: 1800,
            recent_failure_light_window_seconds: 7200,
            provider_failure_margin: 5,
            provider_region_failure_cluster_window_seconds: 3600,
            provider_region_failure_cluster_count: 2,
            raw_score_weight_tenths: 10,
            verify_ok_bonus: 30,
            verify_geo_match_bonus: 20,
            geo_mismatch_penalty: 8,
            region_mismatch_penalty: 4,
            smoke_upstream_ok_bonus: 10,
            verify_failed_heavy_penalty: 30,
            verify_failed_light_penalty: 15,
            verify_failed_base_penalty: 10,
            missing_verify_penalty: 12,
            stale_verify_penalty: 8,
            anonymity_elite_bonus: 4,
            anonymity_anonymous_penalty: 2,
            anonymity_transparent_penalty: 6,
            low_latency_bonus: 2,
            medium_latency_bonus: 1,
            high_latency_penalty: 2,
            very_high_latency_penalty: 5,
            exit_ip_not_public_penalty: 8,
            probe_error_protocol_penalty: 6,
            probe_error_upstream_missing_penalty: 5,
            probe_error_connect_failed_penalty: 8,
            site_success_bonus: 3,
            site_failure_penalty: 6,
            site_browser_failure_penalty: 2,
            soft_min_score_penalty: 6,
        }
    }
}

pub fn default_proxy_selection_tuning() -> ProxySelectionTuning {
    ProxySelectionTuning::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProxySelectionTier {
    HardFilter,
    StrongPositiveSignal,
    RiskPenalty,
    ResourceBalancing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxySelectionRule {
    pub tier: ProxySelectionTier,
    pub name: &'static str,
    pub summary: &'static str,
}

pub fn current_proxy_selection_rules() -> Vec<ProxySelectionRule> {
    vec![
        ProxySelectionRule {
            tier: ProxySelectionTier::HardFilter,
            name: "active_and_usable",
            summary: "仅选择 active、未 cooldown、满足 provider/region/min_score 的代理；soft_min_score 仅影响排序惩罚",
        },
        ProxySelectionRule {
            tier: ProxySelectionTier::StrongPositiveSignal,
            name: "verified_and_geo_matched_first",
            summary: "优先 verify ok、geo match ok、smoke upstream ok 的代理",
        },
        ProxySelectionRule {
            tier: ProxySelectionTier::RiskPenalty,
            name: "failed_missing_or_stale_verify_penalty",
            summary: "recent verify failed、missing verify、stale verify 的代理后排",
        },
        ProxySelectionRule {
            tier: ProxySelectionTier::ResourceBalancing,
            name: "score_then_last_used_then_created",
            summary: "最后才按 score、last_used_at、created_at 做资源均衡",
        },
    ]
}

pub fn proxy_selection_base_where_sql() -> &'static str {
    r#"
               WHERE status = 'active'
                 AND (cooldown_until IS NULL OR CAST(cooldown_until AS INTEGER) <= CAST(? AS INTEGER))
                 AND (? IS NULL OR provider = ?)
                 AND (? IS NULL OR region = ?)
                 AND score >= ?
    "#
}

pub fn proxy_long_term_weight_sql() -> &'static str {
    r#"
                 CASE
                   WHEN failure_count >= success_count + 3 THEN 2
                   WHEN failure_count > success_count THEN 1
                   ELSE 0
                 END ASC,
    "#
}

pub fn provider_long_term_weight_sql() -> &'static str {
    r#"
                 CASE
                   WHEN provider IS NOT NULL AND provider IN (
                       SELECT provider
                       FROM proxies
                       WHERE provider IS NOT NULL
                       GROUP BY provider
                       HAVING SUM(failure_count) >= SUM(success_count) + 5
                   ) THEN 1
                   ELSE 0
                 END ASC,
    "#
}

pub fn proxy_recent_failure_decay_sql() -> &'static str {
    r#"
                 CASE
                   WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 1800 THEN 2
                   WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 7200 THEN 1
                   ELSE 0
                 END ASC,
    "#
}

pub fn provider_region_recent_failure_decay_sql() -> &'static str {
    r#"
                 CASE
                   WHEN provider IS NOT NULL AND region IS NOT NULL AND (provider, region) IN (
                       SELECT provider, region
                       FROM proxies
                       WHERE provider IS NOT NULL
                         AND region IS NOT NULL
                         AND last_verify_status = 'failed'
                         AND last_verify_at IS NOT NULL
                         AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 3600
                       GROUP BY provider, region
                       HAVING COUNT(*) >= 2
                   ) THEN 1
                   ELSE 0
                 END ASC,
    "#
}

pub fn proxy_selection_order_sql() -> &'static str {
    r#"
                 CASE WHEN last_verify_status = 'ok' THEN 0 ELSE 1 END ASC,
                 CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
                 CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 0 ELSE 1 END ASC,
                 CASE
                   WHEN last_verify_status = 'failed' THEN 3
                   WHEN last_verify_at IS NULL THEN 2
                   WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - {stale_after_seconds} THEN 1
                   ELSE 0
                 END ASC,
                 {provider_region_recent_failure_decay}
                 {recent_failure_decay}
                 {provider_long_term_weight}
                 {long_term_weight}
                 COALESCE(last_used_at, '0') ASC,
                 created_at ASC
    "#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_rules_expose_expected_tiers() {
        let rules = current_proxy_selection_rules();
        let tuning = default_proxy_selection_tuning();
        assert_eq!(tuning.stale_after_seconds, 3600);
        assert_eq!(tuning.provider_region_failure_cluster_count, 2);
        assert_eq!(tuning.raw_score_weight_tenths, 10);
        assert_eq!(tuning.verify_ok_bonus, 30);
        assert_eq!(tuning.geo_mismatch_penalty, 8);
        assert_eq!(tuning.region_mismatch_penalty, 4);
        assert_eq!(tuning.missing_verify_penalty, 12);
        assert_eq!(tuning.soft_min_score_penalty, 6);
        assert_eq!(tuning.anonymity_elite_bonus, 4);
        assert_eq!(tuning.very_high_latency_penalty, 5);
        assert_eq!(tuning.exit_ip_not_public_penalty, 8);
        assert_eq!(tuning.probe_error_protocol_penalty, 6);
        assert_eq!(tuning.probe_error_upstream_missing_penalty, 5);
        assert_eq!(tuning.probe_error_connect_failed_penalty, 8);
        let env_tuning = proxy_selection_tuning_from_env();
        assert!(env_tuning.stale_after_seconds > 0);
        assert_eq!(rules.len(), 4);
        assert!(rules
            .iter()
            .any(|r| r.tier == ProxySelectionTier::HardFilter));
        assert!(rules
            .iter()
            .any(|r| r.tier == ProxySelectionTier::StrongPositiveSignal));
        assert!(rules
            .iter()
            .any(|r| r.tier == ProxySelectionTier::RiskPenalty));
        assert!(rules
            .iter()
            .any(|r| r.tier == ProxySelectionTier::ResourceBalancing));
    }

    #[test]
    fn order_sql_contains_verify_risk_penalty_clauses() {
        let sql = proxy_selection_order_sql();
        let base = proxy_selection_base_where_sql();
        let long_term = proxy_long_term_weight_sql();
        assert!(long_term.contains("failure_count > success_count"));
        assert!(base.contains("WHERE status = 'active'"));
        assert!(sql.contains("last_verify_status = 'ok'"));
        assert!(sql.contains("last_verify_status = 'failed'"));
        assert!(sql.contains("last_verify_at IS NULL"));
        let provider_weight = provider_long_term_weight_sql();
        let provider_region_decay = provider_region_recent_failure_decay_sql();
        assert!(provider_region_decay.contains("HAVING COUNT(*) >= 2"));
        let recent_decay = proxy_recent_failure_decay_sql();
        assert!(
            recent_decay.contains("CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 1800")
        );
        assert!(provider_weight.contains("HAVING SUM(failure_count) >= SUM(success_count) + 5"));
        let tuned = proxy_selection_order_sql_with_tuning(&default_proxy_selection_tuning());
        assert!(tuned.contains("provider_risk_snapshots"));
        assert!(tuned.contains("provider_region_risk_snapshots"));
        let trust = proxy_trust_score_sql_with_tuning(&default_proxy_selection_tuning());
        assert!(trust.contains("last_verify_status = 'ok' THEN 30"));
        assert!(trust.contains("last_verify_at IS NULL THEN 12"));
        assert!(trust.contains("CAST(score * 10 AS INTEGER)"));
        let order_by =
            proxy_selection_order_by_trust_score_sql_with_tuning(&default_proxy_selection_tuning());
        assert!(!order_by.contains("score DESC, score DESC"));
        assert!(!order_by.contains("score DESC"));
        assert!(sql.contains("{provider_region_recent_failure_decay}"));
        assert!(sql.contains("{long_term_weight}"));
        assert!(!sql.contains("score DESC"));
    }

    #[test]
    fn trust_score_sql_uses_custom_tuning_weights() {
        let tuning = ProxySelectionTuning {
            verify_ok_bonus: 44,
            verify_geo_match_bonus: 21,
            smoke_upstream_ok_bonus: 13,
            verify_failed_heavy_penalty: 31,
            verify_failed_light_penalty: 17,
            verify_failed_base_penalty: 9,
            missing_verify_penalty: 19,
            stale_verify_penalty: 11,
            raw_score_weight_tenths: 7,
            ..default_proxy_selection_tuning()
        };
        let trust = proxy_trust_score_sql_with_tuning(&tuning);
        assert!(trust.contains("last_verify_status = 'ok' THEN 44"));
        assert!(trust.contains("COALESCE(last_verify_geo_match_ok, 0) != 0 THEN 21"));
        assert!(trust.contains("COALESCE(last_smoke_upstream_ok, 0) != 0 THEN 13"));
        assert!(trust.contains("THEN 31"));
        assert!(trust.contains("THEN 17"));
        assert!(trust.contains("last_verify_status = 'failed' THEN 9"));
        assert!(trust.contains("last_verify_at IS NULL THEN 19"));
        assert!(trust.contains("THEN 11"));
        assert!(trust.contains("CAST(score * 7 AS INTEGER)"));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProxyResolutionMode {
    Resolved,
    ResolvedSticky,
    Unresolved,
}

pub fn proxy_resolution_status(sticky_session: Option<&str>, resolved: bool) -> &'static str {
    match (sticky_session.is_some(), resolved) {
        (_, false) => "unresolved",
        (true, true) => "resolved_sticky",
        (false, true) => "resolved",
    }
}

pub fn apply_proxy_resolution_metadata(
    policy_obj: &mut Map<String, Value>,
    sticky_session: Option<&str>,
    resolved_proxy: Option<Value>,
) {
    let resolved = resolved_proxy.is_some();
    policy_obj.insert(
        "proxy_resolution_status".to_string(),
        json!(proxy_resolution_status(sticky_session, resolved)),
    );
    if let Some(proxy) = resolved_proxy {
        policy_obj.insert("resolved_proxy".to_string(), proxy);
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    #[test]
    fn proxy_resolution_status_matches_sticky_and_unresolved_modes() {
        assert_eq!(proxy_resolution_status(None, false), "unresolved");
        assert_eq!(proxy_resolution_status(None, true), "resolved");
        assert_eq!(
            proxy_resolution_status(Some("sess-1"), true),
            "resolved_sticky"
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub fn resolved_proxy_json(
    id: String,
    scheme: String,
    host: String,
    port: i64,
    username: Option<String>,
    password: Option<String>,
    region: Option<String>,
    country: Option<String>,
    provider: Option<String>,
    score: f64,
) -> Value {
    json!({
        "id": id,
        "scheme": scheme,
        "host": host,
        "port": port,
        "username": username,
        "password": password,
        "region": region,
        "country": country,
        "provider": provider,
        "score": score,
    })
}

#[cfg(test)]
mod json_tests {
    use super::*;

    #[test]
    fn resolved_proxy_json_contains_core_fields() {
        let value = resolved_proxy_json(
            "proxy-1".to_string(),
            "http".to_string(),
            "127.0.0.1".to_string(),
            8080,
            None,
            None,
            Some("us-east".to_string()),
            Some("US".to_string()),
            Some("pool-a".to_string()),
            0.9,
        );
        assert_eq!(value.get("id").and_then(|v| v.as_str()), Some("proxy-1"));
        assert_eq!(value.get("port").and_then(|v| v.as_i64()), Some(8080));
        assert_eq!(
            value.get("provider").and_then(|v| v.as_str()),
            Some("pool-a")
        );
    }
}

pub fn proxy_long_term_weight_sql_with_tuning(tuning: &ProxySelectionTuning) -> String {
    format!(
        "                 CASE
                   WHEN failure_count >= success_count + {margin} THEN 2
                   WHEN failure_count > success_count THEN 1
                   ELSE 0
                 END ASC,",
        margin = tuning.provider_failure_margin.saturating_sub(2).max(1)
    )
}

pub fn provider_long_term_weight_sql_with_tuning(_tuning: &ProxySelectionTuning) -> String {
    "                 CASE
                   WHEN provider IS NOT NULL AND EXISTS (
                       SELECT 1 FROM provider_risk_snapshots prs
                       WHERE prs.provider = proxies.provider
                         AND prs.risk_hit != 0
                   ) THEN 1
                   ELSE 0
                 END ASC,"
        .to_string()
}

pub fn proxy_recent_failure_decay_sql_with_tuning(tuning: &ProxySelectionTuning) -> String {
    format!(
        "                 CASE
                   WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - {heavy} THEN 2
                   WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - {light} THEN 1
                   ELSE 0
                 END ASC,",
        heavy = tuning.recent_failure_heavy_window_seconds,
        light = tuning.recent_failure_light_window_seconds
    )
}

pub fn provider_region_recent_failure_decay_sql_with_tuning(
    _tuning: &ProxySelectionTuning,
) -> String {
    "                 CASE
                   WHEN provider IS NOT NULL AND region IS NOT NULL AND EXISTS (
                       SELECT 1 FROM provider_region_risk_snapshots prrs
                       WHERE prrs.provider = proxies.provider
                         AND prrs.region = proxies.region
                         AND prrs.risk_hit != 0
                   ) THEN 1
                   ELSE 0
                 END ASC,"
        .to_string()
}

pub fn proxy_selection_order_sql_with_tuning(tuning: &ProxySelectionTuning) -> String {
    proxy_selection_order_sql()
        .replace(
            "{provider_region_recent_failure_decay}",
            &provider_region_recent_failure_decay_sql_with_tuning(tuning),
        )
        .replace(
            "{recent_failure_decay}",
            &proxy_recent_failure_decay_sql_with_tuning(tuning),
        )
        .replace(
            "{provider_long_term_weight}",
            &provider_long_term_weight_sql_with_tuning(tuning),
        )
        .replace(
            "{long_term_weight}",
            &proxy_long_term_weight_sql_with_tuning(tuning),
        )
        .replace(
            "{stale_after_seconds}",
            &tuning.stale_after_seconds.to_string(),
        )
}

pub fn proxy_selection_tuning_from_env() -> ProxySelectionTuning {
    let mut tuning = default_proxy_selection_tuning();
    if let Ok(value) = std::env::var("PP_PROXY_STALE_AFTER_SECONDS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.stale_after_seconds = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_RECENT_FAILURE_HEAVY_WINDOW_SECONDS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.recent_failure_heavy_window_seconds = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_RECENT_FAILURE_LIGHT_WINDOW_SECONDS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.recent_failure_light_window_seconds = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_PROVIDER_FAILURE_MARGIN") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.provider_failure_margin = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_PROVIDER_REGION_CLUSTER_WINDOW_SECONDS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.provider_region_failure_cluster_window_seconds = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_PROVIDER_REGION_CLUSTER_COUNT") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.provider_region_failure_cluster_count = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_RAW_SCORE_WEIGHT_TENTHS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.raw_score_weight_tenths = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_VERIFY_OK_BONUS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.verify_ok_bonus = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_VERIFY_GEO_MATCH_BONUS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.verify_geo_match_bonus = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_GEO_MISMATCH_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.geo_mismatch_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_REGION_MISMATCH_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.region_mismatch_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_SMOKE_UPSTREAM_OK_BONUS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.smoke_upstream_ok_bonus = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_VERIFY_FAILED_HEAVY_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.verify_failed_heavy_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_VERIFY_FAILED_LIGHT_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.verify_failed_light_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_VERIFY_FAILED_BASE_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.verify_failed_base_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_MISSING_VERIFY_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.missing_verify_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_STALE_VERIFY_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.stale_verify_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_ANONYMITY_ELITE_BONUS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.anonymity_elite_bonus = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_ANONYMITY_ANONYMOUS_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.anonymity_anonymous_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_ANONYMITY_TRANSPARENT_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.anonymity_transparent_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_LOW_LATENCY_BONUS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.low_latency_bonus = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_MEDIUM_LATENCY_BONUS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.medium_latency_bonus = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_HIGH_LATENCY_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.high_latency_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_VERY_HIGH_LATENCY_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.very_high_latency_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_EXIT_IP_NOT_PUBLIC_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.exit_ip_not_public_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_PROBE_ERROR_PROTOCOL_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.probe_error_protocol_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_PROBE_ERROR_UPSTREAM_MISSING_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.probe_error_upstream_missing_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_PROBE_ERROR_CONNECT_FAILED_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.probe_error_connect_failed_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_SITE_SUCCESS_BONUS") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.site_success_bonus = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_SITE_FAILURE_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.site_failure_penalty = parsed;
        }
    }
    if let Ok(value) = std::env::var("PP_PROXY_SITE_BROWSER_FAILURE_PENALTY") {
        if let Ok(parsed) = value.parse::<i64>() {
            tuning.site_browser_failure_penalty = parsed;
        }
    }
    tuning
}

pub fn proxy_trust_score_sql_with_tuning(tuning: &ProxySelectionTuning) -> String {
    let stale = tuning.stale_after_seconds;
    let heavy = tuning.recent_failure_heavy_window_seconds;
    let light = tuning.recent_failure_light_window_seconds;
    let provider_margin = tuning.provider_failure_margin;
    let raw_score_weight_tenths = tuning.raw_score_weight_tenths;
    let verify_ok_bonus = tuning.verify_ok_bonus;
    let verify_geo_match_bonus = tuning.verify_geo_match_bonus;
    let smoke_upstream_ok_bonus = tuning.smoke_upstream_ok_bonus;
    let verify_failed_heavy_penalty = tuning.verify_failed_heavy_penalty;
    let verify_failed_light_penalty = tuning.verify_failed_light_penalty;
    let verify_failed_base_penalty = tuning.verify_failed_base_penalty;
    let missing_verify_penalty = tuning.missing_verify_penalty;
    let stale_verify_penalty = tuning.stale_verify_penalty;
    let individual_margin = provider_margin.saturating_sub(2).max(1);
    format!(
        "(CASE WHEN last_verify_status = 'ok' THEN {verify_ok_bonus} ELSE 0 END) +
         (CASE WHEN COALESCE(last_verify_geo_match_ok, 0) != 0 THEN {verify_geo_match_bonus} ELSE 0 END) +
         (CASE WHEN COALESCE(last_smoke_upstream_ok, 0) != 0 THEN {smoke_upstream_ok_bonus} ELSE 0 END) -
         (CASE WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - {heavy} THEN {verify_failed_heavy_penalty}
               WHEN last_verify_status = 'failed' AND last_verify_at IS NOT NULL AND CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - {light} THEN {verify_failed_light_penalty}
               WHEN last_verify_status = 'failed' THEN {verify_failed_base_penalty}
               ELSE 0 END) -
         (CASE WHEN last_verify_at IS NULL THEN {missing_verify_penalty}
               WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - {stale} THEN {stale_verify_penalty}
               ELSE 0 END) -
         (CASE WHEN failure_count >= success_count + {individual_margin} THEN 18
               WHEN failure_count > success_count THEN 8
               ELSE 0 END) -
         (CASE WHEN provider IS NOT NULL AND EXISTS (
                    SELECT 1 FROM provider_risk_snapshots prs
                    WHERE prs.provider = proxies.provider
                      AND prs.risk_hit != 0
               ) THEN 10 ELSE 0 END) -
         (CASE WHEN provider IS NOT NULL AND region IS NOT NULL AND EXISTS (
                    SELECT 1 FROM provider_region_risk_snapshots prrs
                    WHERE prrs.provider = proxies.provider
                      AND prrs.region = proxies.region
                      AND prrs.risk_hit != 0
               ) THEN 12 ELSE 0 END) +
         CAST(score * {raw_score_weight_tenths} AS INTEGER)",
        stale = stale,
        heavy = heavy,
        light = light,
        individual_margin = individual_margin,
                raw_score_weight_tenths = raw_score_weight_tenths,
        verify_ok_bonus = verify_ok_bonus,
        verify_geo_match_bonus = verify_geo_match_bonus,
        smoke_upstream_ok_bonus = smoke_upstream_ok_bonus,
        verify_failed_heavy_penalty = verify_failed_heavy_penalty,
        verify_failed_light_penalty = verify_failed_light_penalty,
        verify_failed_base_penalty = verify_failed_base_penalty,
        missing_verify_penalty = missing_verify_penalty,
        stale_verify_penalty = stale_verify_penalty,
    )
}

pub fn proxy_selection_order_by_trust_score_sql_with_tuning(
    tuning: &ProxySelectionTuning,
) -> String {
    let trust = proxy_trust_score_sql_with_tuning(tuning);
    format!(
        "({trust}) DESC, COALESCE(last_used_at, '0') ASC, created_at ASC",
        trust = trust
    )
}

pub fn proxy_selection_order_by_cached_trust_score_sql() -> String {
    "COALESCE(cached_trust_score, -999999) DESC, COALESCE(last_used_at, '0') ASC, created_at ASC"
        .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProxyTrustScoreComponentWeights {
    pub verify_ok_bonus: i64,
    pub verify_geo_match_bonus: i64,
    pub site_success_bonus: i64,
    pub geo_mismatch_penalty: i64,
    pub region_mismatch_penalty: i64,
    pub smoke_upstream_ok_bonus: i64,
    pub verify_failed_heavy_penalty: i64,
    pub verify_failed_light_penalty: i64,
    pub verify_failed_base_penalty: i64,
    pub missing_verify_penalty: i64,
    pub stale_verify_penalty: i64,
    pub raw_score_weight_tenths: i64,
    pub provider_failure_margin: i64,
    pub provider_region_failure_cluster_count: i64,
    pub site_failure_penalty: i64,
    pub site_browser_failure_penalty: i64,
}

pub fn proxy_trust_score_component_weights(
    tuning: &ProxySelectionTuning,
) -> ProxyTrustScoreComponentWeights {
    ProxyTrustScoreComponentWeights {
        verify_ok_bonus: tuning.verify_ok_bonus,
        verify_geo_match_bonus: tuning.verify_geo_match_bonus,
        site_success_bonus: tuning.site_success_bonus,
        geo_mismatch_penalty: tuning.geo_mismatch_penalty,
        region_mismatch_penalty: tuning.region_mismatch_penalty,
        smoke_upstream_ok_bonus: tuning.smoke_upstream_ok_bonus,
        verify_failed_heavy_penalty: tuning.verify_failed_heavy_penalty,
        verify_failed_light_penalty: tuning.verify_failed_light_penalty,
        verify_failed_base_penalty: tuning.verify_failed_base_penalty,
        missing_verify_penalty: tuning.missing_verify_penalty,
        stale_verify_penalty: tuning.stale_verify_penalty,
        raw_score_weight_tenths: tuning.raw_score_weight_tenths,
        provider_failure_margin: tuning.provider_failure_margin,
        provider_region_failure_cluster_count: tuning.provider_region_failure_cluster_count,
        site_failure_penalty: tuning.site_failure_penalty,
        site_browser_failure_penalty: tuning.site_browser_failure_penalty,
    }
}
