use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintFieldPriorityLayer {
    L1,
    L2,
    L3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintPerfBudgetTag {
    Light,
    Medium,
    Heavy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FingerprintFieldPriorityRule {
    pub field: &'static str,
    pub layer: FingerprintFieldPriorityLayer,
    pub reason: &'static str,
}

pub fn fingerprint_field_priority_rules() -> Vec<FingerprintFieldPriorityRule> {
    vec![
        FingerprintFieldPriorityRule { field: "user_agent", layer: FingerprintFieldPriorityLayer::L1, reason: "must be truly consumed first" },
        FingerprintFieldPriorityRule { field: "accept_language", layer: FingerprintFieldPriorityLayer::L1, reason: "must be truly consumed first" },
        FingerprintFieldPriorityRule { field: "locale", layer: FingerprintFieldPriorityLayer::L1, reason: "must be truly consumed first" },
        FingerprintFieldPriorityRule { field: "timezone", layer: FingerprintFieldPriorityLayer::L1, reason: "must be truly consumed first" },
        FingerprintFieldPriorityRule { field: "viewport", layer: FingerprintFieldPriorityLayer::L1, reason: "must be truly consumed first" },
        FingerprintFieldPriorityRule { field: "platform", layer: FingerprintFieldPriorityLayer::L1, reason: "must be truly consumed first" },
        FingerprintFieldPriorityRule { field: "client_hints", layer: FingerprintFieldPriorityLayer::L2, reason: "high-value next layer" },
        FingerprintFieldPriorityRule { field: "hardware_concurrency", layer: FingerprintFieldPriorityLayer::L2, reason: "high-value next layer" },
        FingerprintFieldPriorityRule { field: "device_memory", layer: FingerprintFieldPriorityLayer::L2, reason: "high-value next layer" },
        FingerprintFieldPriorityRule { field: "color_scheme", layer: FingerprintFieldPriorityLayer::L2, reason: "high-value next layer" },
        FingerprintFieldPriorityRule { field: "canvas", layer: FingerprintFieldPriorityLayer::L3, reason: "advanced fingerprint layer" },
        FingerprintFieldPriorityRule { field: "webgl", layer: FingerprintFieldPriorityLayer::L3, reason: "advanced fingerprint layer" },
        FingerprintFieldPriorityRule { field: "audio", layer: FingerprintFieldPriorityLayer::L3, reason: "advanced fingerprint layer" },
        FingerprintFieldPriorityRule { field: "fonts", layer: FingerprintFieldPriorityLayer::L3, reason: "advanced fingerprint layer" },
        FingerprintFieldPriorityRule { field: "anti_detection_flags", layer: FingerprintFieldPriorityLayer::L3, reason: "advanced fingerprint layer" },
    ]
}

pub fn classify_fingerprint_field(field: &str) -> Option<FingerprintFieldPriorityLayer> {
    fingerprint_field_priority_rules()
        .into_iter()
        .find(|rule| rule.field == field)
        .map(|rule| rule.layer)
}

pub fn default_fingerprint_perf_budget_for_layer(layer: FingerprintFieldPriorityLayer) -> FingerprintPerfBudgetTag {
    match layer {
        FingerprintFieldPriorityLayer::L1 => FingerprintPerfBudgetTag::Light,
        FingerprintFieldPriorityLayer::L2 => FingerprintPerfBudgetTag::Medium,
        FingerprintFieldPriorityLayer::L3 => FingerprintPerfBudgetTag::Heavy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_rules_cover_expected_layers() {
        let rules = fingerprint_field_priority_rules();
        assert!(rules.iter().any(|r| r.field == "user_agent" && r.layer == FingerprintFieldPriorityLayer::L1));
        assert!(rules.iter().any(|r| r.field == "hardware_concurrency" && r.layer == FingerprintFieldPriorityLayer::L2));
        assert!(rules.iter().any(|r| r.field == "canvas" && r.layer == FingerprintFieldPriorityLayer::L3));
    }

    #[test]
    fn classify_field_returns_expected_layer() {
        assert_eq!(classify_fingerprint_field("timezone"), Some(FingerprintFieldPriorityLayer::L1));
        assert_eq!(classify_fingerprint_field("client_hints"), Some(FingerprintFieldPriorityLayer::L2));
        assert_eq!(classify_fingerprint_field("audio"), Some(FingerprintFieldPriorityLayer::L3));
        assert_eq!(classify_fingerprint_field("unknown_field"), None);
    }

    #[test]
    fn default_perf_budget_matches_layer_weight() {
        assert_eq!(default_fingerprint_perf_budget_for_layer(FingerprintFieldPriorityLayer::L1), FingerprintPerfBudgetTag::Light);
        assert_eq!(default_fingerprint_perf_budget_for_layer(FingerprintFieldPriorityLayer::L2), FingerprintPerfBudgetTag::Medium);
        assert_eq!(default_fingerprint_perf_budget_for_layer(FingerprintFieldPriorityLayer::L3), FingerprintPerfBudgetTag::Heavy);
    }
}
