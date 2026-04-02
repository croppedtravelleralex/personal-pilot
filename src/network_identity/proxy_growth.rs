use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyPoolGrowthPolicy {
    pub min_available_ratio_percent: i64,
    pub max_available_ratio_percent: i64,
    pub min_available_total: i64,
    pub min_available_per_region: i64,
    pub high_concurrency_threshold: i64,
    pub high_concurrency_min_available_total: i64,
}

impl Default for ProxyPoolGrowthPolicy {
    fn default() -> Self {
        Self {
            min_available_ratio_percent: 40,
            max_available_ratio_percent: 60,
            min_available_total: 20,
            min_available_per_region: 3,
            high_concurrency_threshold: 50,
            high_concurrency_min_available_total: 40,
        }
    }
}

pub fn default_proxy_pool_growth_policy() -> ProxyPoolGrowthPolicy {
    ProxyPoolGrowthPolicy::default()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyPoolInventorySnapshot {
    pub total: i64,
    pub available: i64,
    pub region: Option<String>,
    pub available_in_region: i64,
    pub inflight_tasks: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProxyPoolHealthAssessment {
    pub available_ratio_percent: i64,
    pub healthy_ratio_band: &'static str,
    pub below_min_ratio: bool,
    pub above_max_ratio: bool,
    pub below_min_total: bool,
    pub below_min_region: bool,
    pub require_replenish: bool,
    pub reasons: Vec<&'static str>,
}

pub fn assess_proxy_pool_health(
    snapshot: &ProxyPoolInventorySnapshot,
    policy: &ProxyPoolGrowthPolicy,
) -> ProxyPoolHealthAssessment {
    let available_ratio_percent = if snapshot.total <= 0 {
        0
    } else {
        (snapshot.available * 100) / snapshot.total
    };

    let required_min_total = if snapshot.inflight_tasks >= policy.high_concurrency_threshold {
        policy.high_concurrency_min_available_total.max(policy.min_available_total)
    } else {
        policy.min_available_total
    };

    let below_min_ratio = available_ratio_percent < policy.min_available_ratio_percent;
    let above_max_ratio = available_ratio_percent > policy.max_available_ratio_percent;
    let below_min_total = snapshot.available < required_min_total;
    let below_min_region = snapshot.region.is_some() && snapshot.available_in_region < policy.min_available_per_region;

    let healthy_ratio_band = if below_min_ratio {
        "below_min"
    } else if above_max_ratio {
        "above_max"
    } else {
        "within_band"
    };

    let mut reasons = Vec::new();
    if below_min_ratio {
        reasons.push("available_ratio_below_min");
    }
    if below_min_total {
        reasons.push("available_total_below_min");
    }
    if below_min_region {
        reasons.push("available_region_below_min");
    }

    let require_replenish = below_min_ratio || below_min_total || below_min_region;

    ProxyPoolHealthAssessment {
        available_ratio_percent,
        healthy_ratio_band,
        below_min_ratio,
        above_max_ratio,
        below_min_total,
        below_min_region,
        require_replenish,
        reasons,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegionMatchEvaluation {
    pub target_region: Option<String>,
    pub proxy_region: Option<String>,
    pub match_mode: &'static str,
    pub matches: bool,
    pub score: i64,
    pub reason: &'static str,
}

pub fn evaluate_region_match(target_region: Option<&str>, proxy_region: Option<&str>) -> RegionMatchEvaluation {
    match (target_region.map(str::trim).filter(|v| !v.is_empty()), proxy_region.map(str::trim).filter(|v| !v.is_empty())) {
        (Some(target), Some(proxy)) if target.eq_ignore_ascii_case(proxy) => RegionMatchEvaluation {
            target_region: Some(target.to_string()),
            proxy_region: Some(proxy.to_string()),
            match_mode: "region_preferred",
            matches: true,
            score: 100,
            reason: "exact_region_match",
        },
        (Some(target), Some(proxy)) => RegionMatchEvaluation {
            target_region: Some(target.to_string()),
            proxy_region: Some(proxy.to_string()),
            match_mode: "region_preferred",
            matches: false,
            score: 20,
            reason: "region_mismatch",
        },
        (Some(target), None) => RegionMatchEvaluation {
            target_region: Some(target.to_string()),
            proxy_region: None,
            match_mode: "region_preferred",
            matches: false,
            score: 0,
            reason: "proxy_region_missing",
        },
        (None, Some(proxy)) => RegionMatchEvaluation {
            target_region: None,
            proxy_region: Some(proxy.to_string()),
            match_mode: "no_target_region",
            matches: true,
            score: 60,
            reason: "target_region_not_requested",
        },
        (None, None) => RegionMatchEvaluation {
            target_region: None,
            proxy_region: None,
            match_mode: "no_target_region",
            matches: true,
            score: 50,
            reason: "no_region_constraint",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_assessment_requests_replenish_when_ratio_and_total_are_low() {
        let policy = default_proxy_pool_growth_policy();
        let snapshot = ProxyPoolInventorySnapshot {
            total: 100,
            available: 10,
            region: Some("us-east".to_string()),
            available_in_region: 1,
            inflight_tasks: 10,
        };

        let assessment = assess_proxy_pool_health(&snapshot, &policy);
        assert_eq!(assessment.available_ratio_percent, 10);
        assert!(assessment.below_min_ratio);
        assert!(assessment.below_min_total);
        assert!(assessment.below_min_region);
        assert!(assessment.require_replenish);
        assert!(assessment.reasons.contains(&"available_ratio_below_min"));
        assert!(assessment.reasons.contains(&"available_total_below_min"));
        assert!(assessment.reasons.contains(&"available_region_below_min"));
    }

    #[test]
    fn health_assessment_raises_min_total_under_high_concurrency() {
        let policy = default_proxy_pool_growth_policy();
        let snapshot = ProxyPoolInventorySnapshot {
            total: 100,
            available: 30,
            region: None,
            available_in_region: 0,
            inflight_tasks: 80,
        };

        let assessment = assess_proxy_pool_health(&snapshot, &policy);
        assert!(assessment.below_min_total);
        assert!(assessment.require_replenish);
        assert!(assessment.reasons.contains(&"available_total_below_min"));
    }

    #[test]
    fn health_assessment_accepts_inventory_within_band() {
        let policy = default_proxy_pool_growth_policy();
        let snapshot = ProxyPoolInventorySnapshot {
            total: 100,
            available: 45,
            region: Some("us-east".to_string()),
            available_in_region: 5,
            inflight_tasks: 10,
        };

        let assessment = assess_proxy_pool_health(&snapshot, &policy);
        assert_eq!(assessment.healthy_ratio_band, "within_band");
        assert!(!assessment.require_replenish);
        assert!(assessment.reasons.is_empty());
    }

    #[test]
    fn region_match_prefers_exact_region() {
        let evaluation = evaluate_region_match(Some("us-east"), Some("us-east"));
        assert!(evaluation.matches);
        assert_eq!(evaluation.score, 100);
        assert_eq!(evaluation.reason, "exact_region_match");
    }

    #[test]
    fn region_match_marks_mismatch_and_missing_region() {
        let mismatch = evaluate_region_match(Some("us-east"), Some("eu-west"));
        assert!(!mismatch.matches);
        assert_eq!(mismatch.reason, "region_mismatch");

        let missing = evaluate_region_match(Some("us-east"), None);
        assert!(!missing.matches);
        assert_eq!(missing.reason, "proxy_region_missing");
    }
}
