use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

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
            summary: "仅选择 active、未 cooldown、满足 provider/region/min_score 的代理",
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
                   WHEN CAST(last_verify_at AS INTEGER) <= CAST(? AS INTEGER) - 3600 THEN 1
                   ELSE 0
                 END ASC,
                 CASE
                   WHEN failure_count >= success_count + 3 THEN 2
                   WHEN failure_count > success_count THEN 1
                   ELSE 0
                 END ASC,
                 score DESC,
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
        assert_eq!(rules.len(), 4);
        assert!(rules.iter().any(|r| r.tier == ProxySelectionTier::HardFilter));
        assert!(rules.iter().any(|r| r.tier == ProxySelectionTier::StrongPositiveSignal));
        assert!(rules.iter().any(|r| r.tier == ProxySelectionTier::RiskPenalty));
        assert!(rules.iter().any(|r| r.tier == ProxySelectionTier::ResourceBalancing));
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
        assert!(recent_decay.contains("CAST(last_verify_at AS INTEGER) >= CAST(? AS INTEGER) - 1800"));
        assert!(provider_weight.contains("HAVING SUM(failure_count) >= SUM(success_count) + 5"));
        assert!(sql.contains("failure_count >= success_count + 3"));
        assert!(sql.contains("score DESC"));
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
        assert_eq!(proxy_resolution_status(Some("sess-1"), true), "resolved_sticky");
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
        assert_eq!(value.get("provider").and_then(|v| v.as_str()), Some("pool-a"));
    }
}
