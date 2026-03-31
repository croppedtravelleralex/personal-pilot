use serde::{Deserialize, Serialize};

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
        assert!(base.contains("WHERE status = 'active'"));
        assert!(sql.contains("last_verify_status = 'ok'"));
        assert!(sql.contains("last_verify_status = 'failed'"));
        assert!(sql.contains("last_verify_at IS NULL"));
        assert!(sql.contains("score DESC"));
    }
}
