use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::first_family::{
    detect_fingerprint_schema_kind, fingerprint_profile_field_value, inferred_family_id,
    inferred_family_variant,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsistencyStatus {
    ExactMatch,
    SoftMatch,
    Mismatch,
    MissingContext,
    SuspiciousCombination,
}

impl Default for ConsistencyStatus {
    fn default() -> Self {
        Self::MissingContext
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsistencyCheckItem {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: ConsistencyStatus,
    #[serde(default)]
    pub edge_type: String,
    #[serde(default)]
    pub fields: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FingerprintConsistencyAssessment {
    #[serde(default)]
    pub overall_status: ConsistencyStatus,
    #[serde(default)]
    pub coherence_score: i64,
    #[serde(default)]
    pub risk_reasons: Vec<String>,
    #[serde(default)]
    pub hard_failure_count: usize,
    #[serde(default)]
    pub soft_warning_count: usize,
    #[serde(default)]
    pub family_id: Option<String>,
    #[serde(default)]
    pub family_variant: Option<String>,
    #[serde(default)]
    pub schema_kind: String,
    #[serde(default)]
    pub checks: Vec<ConsistencyCheckItem>,
}

fn norm(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_ascii_lowercase())
}

fn locale_prefix(v: Option<&str>) -> Option<String> {
    norm(v).map(|s| s.split(['-', '_']).next().unwrap_or("").to_string())
}

fn string_value(profile: &Value, field: &str) -> Option<String> {
    fingerprint_profile_field_value(profile, field)
        .and_then(|value| value.as_str().map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn i64_value(profile: &Value, field: &str) -> Option<i64> {
    fingerprint_profile_field_value(profile, field).and_then(|value| value.as_i64())
}

fn bool_value(profile: &Value, field: &str) -> Option<bool> {
    fingerprint_profile_field_value(profile, field).and_then(|value| value.as_bool())
}

fn timezone_matches_region(timezone: Option<&str>, region: Option<&str>) -> Option<bool> {
    let timezone = norm(timezone)?;
    let region = norm(region)?;

    let ok = match region.as_str() {
        "us-east" | "us" | "virginia" | "new-york" => {
            timezone.contains("new_york") || timezone.contains("detroit")
        }
        "us-west" | "california" | "oregon" => timezone.contains("los_angeles"),
        "eu-west" | "london" | "uk" => timezone.contains("london"),
        "de" | "germany" | "berlin" => timezone.contains("berlin"),
        "jp" | "japan" | "tokyo" => timezone.contains("tokyo"),
        "cn" | "china" | "asia/shanghai" | "shanghai" => timezone.contains("shanghai"),
        _ => return None,
    };
    Some(ok)
}

fn push_check(
    checks: &mut Vec<ConsistencyCheckItem>,
    name: &str,
    status: ConsistencyStatus,
    edge_type: &str,
    fields: &[&str],
    reason: &str,
) {
    checks.push(ConsistencyCheckItem {
        name: name.to_string(),
        status,
        edge_type: edge_type.to_string(),
        fields: fields.iter().map(|field| (*field).to_string()).collect(),
        reason: reason.to_string(),
    });
}

fn summarize_checks(
    profile: &Value,
    checks: Vec<ConsistencyCheckItem>,
) -> FingerprintConsistencyAssessment {
    let hard_failure_count = checks
        .iter()
        .filter(|check| {
            check.edge_type == "hard" && matches!(check.status, ConsistencyStatus::Mismatch)
        })
        .count();
    let soft_warning_count = checks
        .iter()
        .filter(|check| check.status == ConsistencyStatus::SuspiciousCombination)
        .count();
    let mut coherence_score = 100_i64;
    for check in &checks {
        coherence_score -= match check.status {
            ConsistencyStatus::Mismatch => {
                if check.edge_type == "hard" {
                    30
                } else {
                    20
                }
            }
            ConsistencyStatus::SuspiciousCombination => 10,
            _ => 0,
        };
    }
    coherence_score = coherence_score.clamp(0, 100);
    let risk_reasons = checks
        .iter()
        .filter(|check| {
            matches!(
                check.status,
                ConsistencyStatus::Mismatch | ConsistencyStatus::SuspiciousCombination
            )
        })
        .map(|check| check.reason.clone())
        .collect::<Vec<_>>();
    let overall_status = if checks
        .iter()
        .any(|check| check.status == ConsistencyStatus::Mismatch)
    {
        ConsistencyStatus::Mismatch
    } else if checks
        .iter()
        .any(|check| check.status == ConsistencyStatus::SuspiciousCombination)
    {
        ConsistencyStatus::SuspiciousCombination
    } else if checks.iter().all(|check| {
        matches!(
            check.status,
            ConsistencyStatus::ExactMatch | ConsistencyStatus::SoftMatch
        )
    }) {
        ConsistencyStatus::ExactMatch
    } else if checks.iter().any(|check| {
        matches!(
            check.status,
            ConsistencyStatus::ExactMatch | ConsistencyStatus::SoftMatch
        )
    }) {
        ConsistencyStatus::ExactMatch
    } else {
        ConsistencyStatus::MissingContext
    };

    FingerprintConsistencyAssessment {
        overall_status,
        coherence_score,
        risk_reasons,
        hard_failure_count,
        soft_warning_count,
        family_id: inferred_family_id(profile),
        family_variant: inferred_family_variant(profile),
        schema_kind: detect_fingerprint_schema_kind(profile).to_string(),
        checks,
    }
}

pub fn assess_fingerprint_profile_consistency(
    target_region: Option<&str>,
    proxy_region: Option<&str>,
    exit_region: Option<&str>,
    profile: &Value,
) -> FingerprintConsistencyAssessment {
    let mut checks = Vec::new();
    let target = norm(target_region);
    let proxy = norm(proxy_region);
    let exit = norm(exit_region);
    let timezone = string_value(profile, "timezone");
    let locale = string_value(profile, "locale");
    let accept_language = string_value(profile, "accept_language");
    let locale_p = locale_prefix(locale.as_deref());
    let lang_p = locale_prefix(accept_language.as_deref());

    match (target.as_deref(), proxy.as_deref()) {
        (Some(target), Some(proxy)) if target == proxy => push_check(
            &mut checks,
            "target_region_vs_proxy_region",
            ConsistencyStatus::ExactMatch,
            "hard",
            &["target_region", "proxy_region"],
            "target_and_proxy_region_match",
        ),
        (Some(_), Some(_)) => push_check(
            &mut checks,
            "target_region_vs_proxy_region",
            ConsistencyStatus::Mismatch,
            "hard",
            &["target_region", "proxy_region"],
            "target_and_proxy_region_mismatch",
        ),
        (Some(_), None) => push_check(
            &mut checks,
            "target_region_vs_proxy_region",
            ConsistencyStatus::MissingContext,
            "hard",
            &["target_region", "proxy_region"],
            "proxy_region_missing",
        ),
        _ => push_check(
            &mut checks,
            "target_region_vs_proxy_region",
            ConsistencyStatus::MissingContext,
            "hard",
            &["target_region", "proxy_region"],
            "target_region_not_requested",
        ),
    }

    match (proxy.as_deref(), exit.as_deref()) {
        (Some(proxy), Some(exit)) if proxy == exit => push_check(
            &mut checks,
            "proxy_region_vs_exit_region",
            ConsistencyStatus::ExactMatch,
            "hard",
            &["proxy_region", "exit_region"],
            "proxy_and_exit_region_match",
        ),
        (Some(_), Some(_)) => push_check(
            &mut checks,
            "proxy_region_vs_exit_region",
            ConsistencyStatus::Mismatch,
            "hard",
            &["proxy_region", "exit_region"],
            "proxy_and_exit_region_mismatch",
        ),
        (Some(_), None) => push_check(
            &mut checks,
            "proxy_region_vs_exit_region",
            ConsistencyStatus::MissingContext,
            "hard",
            &["proxy_region", "exit_region"],
            "exit_region_missing",
        ),
        _ => push_check(
            &mut checks,
            "proxy_region_vs_exit_region",
            ConsistencyStatus::MissingContext,
            "hard",
            &["proxy_region", "exit_region"],
            "proxy_region_missing",
        ),
    }

    match timezone_matches_region(timezone.as_deref(), target.as_deref().or(proxy.as_deref())) {
        Some(true) => push_check(
            &mut checks,
            "timezone_vs_region",
            ConsistencyStatus::SoftMatch,
            "soft",
            &["timezone", "proxy_region"],
            "timezone_matches_region_family",
        ),
        Some(false) => push_check(
            &mut checks,
            "timezone_vs_region",
            ConsistencyStatus::SuspiciousCombination,
            "soft",
            &["timezone", "proxy_region"],
            "timezone_looks_inconsistent_with_region",
        ),
        None => push_check(
            &mut checks,
            "timezone_vs_region",
            ConsistencyStatus::MissingContext,
            "soft",
            &["timezone", "proxy_region"],
            "timezone_or_region_missing_or_unknown_mapping",
        ),
    }

    match (locale_p.as_deref(), lang_p.as_deref()) {
        (Some(locale), Some(language)) if locale == language => push_check(
            &mut checks,
            "locale_vs_accept_language",
            ConsistencyStatus::ExactMatch,
            "hard",
            &["locale", "accept_language"],
            "locale_and_accept_language_match",
        ),
        (Some(_), Some(_)) => push_check(
            &mut checks,
            "locale_vs_accept_language",
            ConsistencyStatus::Mismatch,
            "hard",
            &["locale", "accept_language"],
            "locale_and_accept_language_mismatch",
        ),
        _ => push_check(
            &mut checks,
            "locale_vs_accept_language",
            ConsistencyStatus::MissingContext,
            "hard",
            &["locale", "accept_language"],
            "locale_or_accept_language_missing",
        ),
    }

    match (
        i64_value(profile, "screen_width"),
        i64_value(profile, "screen_height"),
        i64_value(profile, "viewport_width"),
        i64_value(profile, "viewport_height"),
    ) {
        (Some(screen_width), Some(screen_height), Some(viewport_width), Some(viewport_height))
            if viewport_width <= screen_width && viewport_height <= screen_height =>
        {
            push_check(
                &mut checks,
                "screen_vs_viewport",
                ConsistencyStatus::ExactMatch,
                "hard",
                &[
                    "screen_width",
                    "screen_height",
                    "viewport_width",
                    "viewport_height",
                ],
                "viewport_is_bounded_by_screen",
            )
        }
        (Some(_), Some(_), Some(_), Some(_)) => push_check(
            &mut checks,
            "screen_vs_viewport",
            ConsistencyStatus::Mismatch,
            "hard",
            &[
                "screen_width",
                "screen_height",
                "viewport_width",
                "viewport_height",
            ],
            "viewport_exceeds_screen_bounds",
        ),
        _ => push_check(
            &mut checks,
            "screen_vs_viewport",
            ConsistencyStatus::MissingContext,
            "hard",
            &[
                "screen_width",
                "screen_height",
                "viewport_width",
                "viewport_height",
            ],
            "screen_or_viewport_missing",
        ),
    }

    match (
        string_value(profile, "gpu_vendor"),
        string_value(profile, "webgl_vendor"),
        string_value(profile, "gpu_renderer"),
        string_value(profile, "webgl_renderer"),
    ) {
        (Some(gpu_vendor), Some(webgl_vendor), Some(gpu_renderer), Some(webgl_renderer))
            if gpu_vendor.eq_ignore_ascii_case(&webgl_vendor)
                && gpu_renderer.eq_ignore_ascii_case(&webgl_renderer) =>
        {
            push_check(
                &mut checks,
                "gpu_vs_webgl",
                ConsistencyStatus::ExactMatch,
                "hard",
                &[
                    "gpu_vendor",
                    "webgl_vendor",
                    "gpu_renderer",
                    "webgl_renderer",
                ],
                "gpu_and_webgl_stack_match",
            )
        }
        (Some(_), Some(_), Some(_), Some(_)) => push_check(
            &mut checks,
            "gpu_vs_webgl",
            ConsistencyStatus::Mismatch,
            "hard",
            &[
                "gpu_vendor",
                "webgl_vendor",
                "gpu_renderer",
                "webgl_renderer",
            ],
            "gpu_and_webgl_stack_mismatch",
        ),
        _ => push_check(
            &mut checks,
            "gpu_vs_webgl",
            ConsistencyStatus::MissingContext,
            "hard",
            &[
                "gpu_vendor",
                "webgl_vendor",
                "gpu_renderer",
                "webgl_renderer",
            ],
            "gpu_or_webgl_identity_missing",
        ),
    }

    match (
        bool_value(profile, "touch_support"),
        i64_value(profile, "max_touch_points"),
    ) {
        (Some(true), Some(points)) if points > 0 => push_check(
            &mut checks,
            "touch_vs_max_touch_points",
            ConsistencyStatus::ExactMatch,
            "hard",
            &["touch_support", "max_touch_points"],
            "touch_support_matches_touch_points",
        ),
        (Some(false), Some(0)) => push_check(
            &mut checks,
            "touch_vs_max_touch_points",
            ConsistencyStatus::ExactMatch,
            "hard",
            &["touch_support", "max_touch_points"],
            "touch_support_matches_touch_points",
        ),
        (Some(_), Some(_)) => push_check(
            &mut checks,
            "touch_vs_max_touch_points",
            ConsistencyStatus::Mismatch,
            "hard",
            &["touch_support", "max_touch_points"],
            "touch_support_conflicts_with_touch_points",
        ),
        _ => push_check(
            &mut checks,
            "touch_vs_max_touch_points",
            ConsistencyStatus::MissingContext,
            "hard",
            &["touch_support", "max_touch_points"],
            "touch_support_or_touch_points_missing",
        ),
    }

    match (
        i64_value(profile, "sticky_session_ttl"),
        string_value(profile, "rotation_policy"),
    ) {
        (Some(ttl), Some(policy))
            if ttl > 0
                && matches!(
                    policy.trim().to_ascii_lowercase().as_str(),
                    "sticky"
                        | "sticky_session"
                        | "sticky-session"
                        | "bounded_rotation"
                        | "bounded-rotation"
                ) =>
        {
            push_check(
                &mut checks,
                "sticky_session_vs_rotation_policy",
                ConsistencyStatus::ExactMatch,
                "hard",
                &["sticky_session_ttl", "rotation_policy"],
                "sticky_session_and_rotation_policy_align",
            )
        }
        (Some(ttl), Some(policy))
            if ttl <= 0
                && matches!(
                    policy.trim().to_ascii_lowercase().as_str(),
                    "per_request" | "per-request" | "every_request" | "every-request"
                ) =>
        {
            push_check(
                &mut checks,
                "sticky_session_vs_rotation_policy",
                ConsistencyStatus::ExactMatch,
                "hard",
                &["sticky_session_ttl", "rotation_policy"],
                "non_sticky_session_and_rotation_policy_align",
            )
        }
        (Some(_), Some(_)) => push_check(
            &mut checks,
            "sticky_session_vs_rotation_policy",
            ConsistencyStatus::Mismatch,
            "hard",
            &["sticky_session_ttl", "rotation_policy"],
            "sticky_session_and_rotation_policy_conflict",
        ),
        _ => push_check(
            &mut checks,
            "sticky_session_vs_rotation_policy",
            ConsistencyStatus::MissingContext,
            "hard",
            &["sticky_session_ttl", "rotation_policy"],
            "sticky_session_or_rotation_policy_missing",
        ),
    }

    match (
        i64_value(profile, "hardware_concurrency"),
        i64_value(profile, "device_memory_gb"),
        string_value(profile, "cpu_class"),
        string_value(profile, "power_plan"),
    ) {
        (Some(cpu), Some(memory), Some(cpu_class), Some(power_plan))
            if cpu >= 8
                && memory >= 16
                && matches!(
                    cpu_class.trim().to_ascii_lowercase().as_str(),
                    "high" | "workstation" | "performance"
                )
                && matches!(
                    power_plan.trim().to_ascii_lowercase().as_str(),
                    "balanced" | "performance" | "plugged_in" | "plugged-in"
                ) =>
        {
            push_check(
                &mut checks,
                "hardware_vs_power_plan",
                ConsistencyStatus::SoftMatch,
                "soft",
                &[
                    "hardware_concurrency",
                    "device_memory_gb",
                    "cpu_class",
                    "power_plan",
                ],
                "hardware_tier_matches_power_plan_family",
            )
        }
        (Some(cpu), Some(memory), Some(cpu_class), Some(power_plan))
            if cpu <= 4
                && memory <= 8
                && matches!(
                    cpu_class.trim().to_ascii_lowercase().as_str(),
                    "entry" | "light" | "mobile"
                )
                && matches!(
                    power_plan.trim().to_ascii_lowercase().as_str(),
                    "balanced" | "battery_saver" | "battery-saver"
                ) =>
        {
            push_check(
                &mut checks,
                "hardware_vs_power_plan",
                ConsistencyStatus::SoftMatch,
                "soft",
                &[
                    "hardware_concurrency",
                    "device_memory_gb",
                    "cpu_class",
                    "power_plan",
                ],
                "hardware_tier_matches_power_plan_family",
            )
        }
        (Some(_), Some(_), Some(_), Some(_)) => push_check(
            &mut checks,
            "hardware_vs_power_plan",
            ConsistencyStatus::SuspiciousCombination,
            "soft",
            &[
                "hardware_concurrency",
                "device_memory_gb",
                "cpu_class",
                "power_plan",
            ],
            "hardware_tier_looks_inconsistent_with_power_plan",
        ),
        _ => push_check(
            &mut checks,
            "hardware_vs_power_plan",
            ConsistencyStatus::MissingContext,
            "soft",
            &[
                "hardware_concurrency",
                "device_memory_gb",
                "cpu_class",
                "power_plan",
            ],
            "hardware_tier_or_power_plan_missing",
        ),
    }

    match (
        bool_value(profile, "battery_presence"),
        string_value(profile, "session_length_bucket"),
    ) {
        (Some(true), Some(bucket))
            if matches!(
                bucket.trim().to_ascii_lowercase().as_str(),
                "short" | "medium" | "standard"
            ) =>
        {
            push_check(
                &mut checks,
                "battery_vs_session_length",
                ConsistencyStatus::SoftMatch,
                "soft",
                &["battery_presence", "session_length_bucket"],
                "battery_presence_matches_session_length",
            )
        }
        (Some(false), Some(bucket))
            if matches!(
                bucket.trim().to_ascii_lowercase().as_str(),
                "medium" | "long" | "extended"
            ) =>
        {
            push_check(
                &mut checks,
                "battery_vs_session_length",
                ConsistencyStatus::SoftMatch,
                "soft",
                &["battery_presence", "session_length_bucket"],
                "battery_presence_matches_session_length",
            )
        }
        (Some(_), Some(_)) => push_check(
            &mut checks,
            "battery_vs_session_length",
            ConsistencyStatus::SuspiciousCombination,
            "soft",
            &["battery_presence", "session_length_bucket"],
            "battery_presence_looks_inconsistent_with_session_length",
        ),
        _ => push_check(
            &mut checks,
            "battery_vs_session_length",
            ConsistencyStatus::MissingContext,
            "soft",
            &["battery_presence", "session_length_bucket"],
            "battery_presence_or_session_length_missing",
        ),
    }

    match (
        string_value(profile, "automation_policy"),
        string_value(profile, "idle_timeout_profile"),
        string_value(profile, "tab_switch_cadence"),
    ) {
        (Some(policy), Some(idle), Some(cadence))
            if matches!(
                policy.trim().to_ascii_lowercase().as_str(),
                "human_like" | "human-like" | "assisted"
            ) && !matches!(idle.trim().to_ascii_lowercase().as_str(), "none" | "zero")
                && !matches!(
                    cadence.trim().to_ascii_lowercase().as_str(),
                    "instant" | "robotic"
                ) =>
        {
            push_check(
                &mut checks,
                "automation_policy_vs_behavior_cadence",
                ConsistencyStatus::SoftMatch,
                "derived",
                &[
                    "automation_policy",
                    "idle_timeout_profile",
                    "tab_switch_cadence",
                ],
                "automation_policy_matches_behavior_cadence",
            )
        }
        (Some(_), Some(_), Some(_)) => push_check(
            &mut checks,
            "automation_policy_vs_behavior_cadence",
            ConsistencyStatus::SuspiciousCombination,
            "derived",
            &[
                "automation_policy",
                "idle_timeout_profile",
                "tab_switch_cadence",
            ],
            "automation_policy_looks_inconsistent_with_behavior_cadence",
        ),
        _ => push_check(
            &mut checks,
            "automation_policy_vs_behavior_cadence",
            ConsistencyStatus::MissingContext,
            "derived",
            &[
                "automation_policy",
                "idle_timeout_profile",
                "tab_switch_cadence",
            ],
            "automation_policy_or_behavior_cadence_missing",
        ),
    }

    summarize_checks(profile, checks)
}

pub fn assess_fingerprint_proxy_region_consistency(
    target_region: Option<&str>,
    proxy_region: Option<&str>,
    exit_region: Option<&str>,
    timezone: Option<&str>,
    locale: Option<&str>,
    accept_language: Option<&str>,
) -> FingerprintConsistencyAssessment {
    let profile = serde_json::json!({
        "timezone": timezone,
        "locale": locale,
        "accept_language": accept_language,
    });
    assess_fingerprint_profile_consistency(target_region, proxy_region, exit_region, &profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assessment_reports_exact_and_soft_matches() {
        let profile = serde_json::json!({
            "timezone": "America/New_York",
            "locale": "en-US",
            "accept_language": "en-US,en;q=0.9",
            "screen_width": 1920,
            "screen_height": 1080,
            "viewport_width": 1536,
            "viewport_height": 864,
            "gpu_vendor": "Intel",
            "webgl_vendor": "Intel",
            "gpu_renderer": "Intel Iris Xe",
            "webgl_renderer": "Intel Iris Xe",
            "touch_support": false,
            "max_touch_points": 0,
            "sticky_session_ttl": 1800,
            "rotation_policy": "sticky",
        });
        let assessment = assess_fingerprint_profile_consistency(
            Some("us-east"),
            Some("us-east"),
            Some("us-east"),
            &profile,
        );
        assert_eq!(assessment.overall_status, ConsistencyStatus::ExactMatch);
        assert!(assessment.coherence_score >= 90);
        assert!(assessment
            .checks
            .iter()
            .any(|check| check.name == "timezone_vs_region"
                && check.status == ConsistencyStatus::SoftMatch));
    }

    #[test]
    fn assessment_reports_hard_mismatch_for_structural_conflicts() {
        let profile = serde_json::json!({
            "locale": "zh-CN",
            "accept_language": "en-US,en;q=0.9",
            "screen_width": 1280,
            "screen_height": 720,
            "viewport_width": 1400,
            "viewport_height": 900,
            "touch_support": false,
            "max_touch_points": 5,
        });
        let assessment = assess_fingerprint_profile_consistency(
            Some("us-east"),
            Some("eu-west"),
            Some("eu-west"),
            &profile,
        );
        assert_eq!(assessment.overall_status, ConsistencyStatus::Mismatch);
        assert!(assessment.coherence_score < 50);
        assert!(assessment
            .checks
            .iter()
            .any(|check| check.name == "screen_vs_viewport"
                && check.status == ConsistencyStatus::Mismatch));
    }

    #[test]
    fn grouped_profiles_are_supported() {
        let profile = serde_json::json!({
            "family_id": "win11_business_laptop",
            "control": {
                "os": {
                    "timezone": "Asia/Shanghai"
                },
                "locale": {
                    "locale": "zh-CN",
                    "accept_language": "zh-CN,zh;q=0.9"
                },
                "display": {
                    "screen_width": 1920,
                    "screen_height": 1080,
                    "viewport_width": 1536,
                    "viewport_height": 864
                }
            }
        });
        let assessment =
            assess_fingerprint_profile_consistency(Some("cn"), Some("cn"), Some("cn"), &profile);
        assert_eq!(assessment.schema_kind, "canonical_grouped");
        assert!(assessment.family_id.as_deref() == Some("win11_business_laptop"));
    }
}
