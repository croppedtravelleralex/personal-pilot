use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsistencyStatus {
    ExactMatch,
    SoftMatch,
    Mismatch,
    MissingContext,
    SuspiciousCombination,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsistencyCheckItem {
    pub name: String,
    pub status: ConsistencyStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FingerprintConsistencyAssessment {
    pub overall_status: ConsistencyStatus,
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

pub fn assess_fingerprint_proxy_region_consistency(
    target_region: Option<&str>,
    proxy_region: Option<&str>,
    exit_region: Option<&str>,
    timezone: Option<&str>,
    locale: Option<&str>,
    accept_language: Option<&str>,
) -> FingerprintConsistencyAssessment {
    let mut checks = Vec::new();

    let target = norm(target_region);
    let proxy = norm(proxy_region);
    let exit = norm(exit_region);
    let locale_p = locale_prefix(locale);
    let lang_p = locale_prefix(accept_language);

    let target_proxy = match (target.as_deref(), proxy.as_deref()) {
        (Some(t), Some(p)) if t == p => ConsistencyCheckItem {
            name: "target_region_vs_proxy_region".to_string(),
            status: ConsistencyStatus::ExactMatch,
            reason: "target_and_proxy_region_match".to_string(),
        },
        (Some(_), Some(_)) => ConsistencyCheckItem {
            name: "target_region_vs_proxy_region".to_string(),
            status: ConsistencyStatus::Mismatch,
            reason: "target_and_proxy_region_mismatch".to_string(),
        },
        (Some(_), None) => ConsistencyCheckItem {
            name: "target_region_vs_proxy_region".to_string(),
            status: ConsistencyStatus::MissingContext,
            reason: "proxy_region_missing".to_string(),
        },
        _ => ConsistencyCheckItem {
            name: "target_region_vs_proxy_region".to_string(),
            status: ConsistencyStatus::MissingContext,
            reason: "target_region_not_requested".to_string(),
        },
    };
    checks.push(target_proxy);

    let proxy_exit = match (proxy.as_deref(), exit.as_deref()) {
        (Some(p), Some(e)) if p == e => ConsistencyCheckItem {
            name: "proxy_region_vs_exit_region".to_string(),
            status: ConsistencyStatus::ExactMatch,
            reason: "proxy_and_exit_region_match".to_string(),
        },
        (Some(_), Some(_)) => ConsistencyCheckItem {
            name: "proxy_region_vs_exit_region".to_string(),
            status: ConsistencyStatus::Mismatch,
            reason: "proxy_and_exit_region_mismatch".to_string(),
        },
        (Some(_), None) => ConsistencyCheckItem {
            name: "proxy_region_vs_exit_region".to_string(),
            status: ConsistencyStatus::MissingContext,
            reason: "exit_region_missing".to_string(),
        },
        _ => ConsistencyCheckItem {
            name: "proxy_region_vs_exit_region".to_string(),
            status: ConsistencyStatus::MissingContext,
            reason: "proxy_region_missing".to_string(),
        },
    };
    checks.push(proxy_exit);

    let timezone_check =
        match timezone_matches_region(timezone, target.as_deref().or(proxy.as_deref())) {
            Some(true) => ConsistencyCheckItem {
                name: "timezone_vs_region".to_string(),
                status: ConsistencyStatus::SoftMatch,
                reason: "timezone_matches_region_family".to_string(),
            },
            Some(false) => ConsistencyCheckItem {
                name: "timezone_vs_region".to_string(),
                status: ConsistencyStatus::SuspiciousCombination,
                reason: "timezone_looks_inconsistent_with_region".to_string(),
            },
            None => ConsistencyCheckItem {
                name: "timezone_vs_region".to_string(),
                status: ConsistencyStatus::MissingContext,
                reason: "timezone_or_region_missing_or_unknown_mapping".to_string(),
            },
        };
    checks.push(timezone_check);

    let locale_lang = match (locale_p.as_deref(), lang_p.as_deref()) {
        (Some(l), Some(a)) if l == a => ConsistencyCheckItem {
            name: "locale_vs_accept_language".to_string(),
            status: ConsistencyStatus::ExactMatch,
            reason: "locale_and_accept_language_match".to_string(),
        },
        (Some(_), Some(_)) => ConsistencyCheckItem {
            name: "locale_vs_accept_language".to_string(),
            status: ConsistencyStatus::SuspiciousCombination,
            reason: "locale_and_accept_language_mismatch".to_string(),
        },
        _ => ConsistencyCheckItem {
            name: "locale_vs_accept_language".to_string(),
            status: ConsistencyStatus::MissingContext,
            reason: "locale_or_accept_language_missing".to_string(),
        },
    };
    checks.push(locale_lang);

    let overall_status = if checks
        .iter()
        .any(|c| c.status == ConsistencyStatus::Mismatch)
    {
        ConsistencyStatus::Mismatch
    } else if checks
        .iter()
        .any(|c| c.status == ConsistencyStatus::SuspiciousCombination)
    {
        ConsistencyStatus::SuspiciousCombination
    } else if checks.iter().all(|c| {
        c.status == ConsistencyStatus::ExactMatch || c.status == ConsistencyStatus::SoftMatch
    }) {
        ConsistencyStatus::ExactMatch
    } else {
        ConsistencyStatus::MissingContext
    };

    FingerprintConsistencyAssessment {
        overall_status,
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assessment_reports_exact_and_soft_matches() {
        let assessment = assess_fingerprint_proxy_region_consistency(
            Some("us-east"),
            Some("us-east"),
            Some("us-east"),
            Some("America/New_York"),
            Some("en-US"),
            Some("en-US,en;q=0.9"),
        );
        assert_eq!(assessment.overall_status, ConsistencyStatus::ExactMatch);
        assert!(assessment
            .checks
            .iter()
            .any(|c| c.name == "timezone_vs_region" && c.status == ConsistencyStatus::SoftMatch));
    }

    #[test]
    fn assessment_reports_mismatch_when_target_and_proxy_diverge() {
        let assessment = assess_fingerprint_proxy_region_consistency(
            Some("us-east"),
            Some("eu-west"),
            Some("eu-west"),
            Some("Europe/London"),
            Some("en-GB"),
            Some("en-GB,en;q=0.9"),
        );
        assert_eq!(assessment.overall_status, ConsistencyStatus::Mismatch);
        assert!(assessment
            .checks
            .iter()
            .any(|c| c.name == "target_region_vs_proxy_region"
                && c.status == ConsistencyStatus::Mismatch));
    }

    #[test]
    fn assessment_reports_suspicious_combination_for_timezone_or_locale_conflicts() {
        let assessment = assess_fingerprint_proxy_region_consistency(
            Some("us-east"),
            Some("us-east"),
            Some("us-east"),
            Some("Asia/Shanghai"),
            Some("zh-CN"),
            Some("en-US,en;q=0.9"),
        );
        assert_eq!(
            assessment.overall_status,
            ConsistencyStatus::SuspiciousCombination
        );
        assert!(assessment
            .checks
            .iter()
            .any(|c| c.name == "timezone_vs_region"
                && c.status == ConsistencyStatus::SuspiciousCombination));
        assert!(assessment
            .checks
            .iter()
            .any(|c| c.name == "locale_vs_accept_language"
                && c.status == ConsistencyStatus::SuspiciousCombination));
    }
}
