use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::first_family::{detect_fingerprint_schema_kind, fingerprint_profile_field_value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintValidationIssue {
    pub level: String,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintValidationResult {
    pub ok: bool,
    pub issues: Vec<FingerprintValidationIssue>,
}

fn get_str(v: &Value, key: &str) -> Option<String> {
    fingerprint_profile_field_value(v, key)
        .and_then(|value| value.as_str().map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn get_i64(v: &Value, key: &str) -> Option<i64> {
    fingerprint_profile_field_value(v, key).and_then(|value| value.as_i64())
}

fn get_bool(v: &Value, key: &str) -> Option<bool> {
    fingerprint_profile_field_value(v, key).and_then(|value| value.as_bool())
}

fn push_issue(
    issues: &mut Vec<FingerprintValidationIssue>,
    level: &str,
    field: &str,
    message: &str,
) {
    issues.push(FingerprintValidationIssue {
        level: level.to_string(),
        field: field.to_string(),
        message: message.to_string(),
    });
}

pub fn validate_fingerprint_profile(profile: &Value) -> FingerprintValidationResult {
    let mut issues = Vec::new();
    let schema_kind = detect_fingerprint_schema_kind(profile);

    let timezone = get_str(profile, "timezone");
    let locale = get_str(profile, "locale");
    let accept_language = get_str(profile, "accept_language");
    let platform = get_str(profile, "platform");
    let viewport_width = get_i64(profile, "viewport_width");
    let viewport_height = get_i64(profile, "viewport_height");
    let screen_width = get_i64(profile, "screen_width");
    let screen_height = get_i64(profile, "screen_height");
    let available_width = get_i64(profile, "available_width");
    let available_height = get_i64(profile, "available_height");
    let device_memory_gb = get_i64(profile, "device_memory_gb");
    let hardware_concurrency = get_i64(profile, "hardware_concurrency");
    let gpu_vendor = get_str(profile, "gpu_vendor");
    let gpu_renderer = get_str(profile, "gpu_renderer");
    let webgl_vendor = get_str(profile, "webgl_vendor");
    let webgl_renderer = get_str(profile, "webgl_renderer");
    let touch_support = get_bool(profile, "touch_support");
    let max_touch_points = get_i64(profile, "max_touch_points");
    let sticky_session_ttl = get_i64(profile, "sticky_session_ttl");
    let rotation_policy = get_str(profile, "rotation_policy");

    if timezone.is_none() {
        push_issue(&mut issues, "warn", "timezone", "timezone is missing");
    }
    if locale.is_none() {
        push_issue(&mut issues, "warn", "locale", "locale is missing");
    }
    if accept_language.is_none() {
        push_issue(
            &mut issues,
            "warn",
            "accept_language",
            "accept_language is missing",
        );
    }
    if platform.is_none() {
        push_issue(&mut issues, "warn", "platform", "platform is missing");
    }

    if let (Some(vw), Some(sw)) = (viewport_width, screen_width) {
        if vw > sw {
            push_issue(
                &mut issues,
                "error",
                "viewport_width",
                "viewport_width cannot exceed screen_width",
            );
        }
    }
    if let (Some(vh), Some(sh)) = (viewport_height, screen_height) {
        if vh > sh {
            push_issue(
                &mut issues,
                "error",
                "viewport_height",
                "viewport_height cannot exceed screen_height",
            );
        }
    }
    if let (Some(aw), Some(sw)) = (available_width, screen_width) {
        if aw > sw {
            push_issue(
                &mut issues,
                "error",
                "available_width",
                "available_width cannot exceed screen_width",
            );
        }
    }
    if let (Some(ah), Some(sh)) = (available_height, screen_height) {
        if ah > sh {
            push_issue(
                &mut issues,
                "error",
                "available_height",
                "available_height cannot exceed screen_height",
            );
        }
    }

    if let Some(mem) = device_memory_gb {
        if !(1..=128).contains(&mem) {
            push_issue(
                &mut issues,
                "error",
                "device_memory_gb",
                "device_memory_gb is out of expected range",
            );
        }
    }

    if let Some(cpu) = hardware_concurrency {
        if !(1..=128).contains(&cpu) {
            push_issue(
                &mut issues,
                "error",
                "hardware_concurrency",
                "hardware_concurrency is out of expected range",
            );
        }
    }

    if let (Some(locale), Some(lang)) = (locale.as_deref(), accept_language.as_deref()) {
        let locale_prefix = locale
            .split(['-', '_'])
            .next()
            .unwrap_or(locale)
            .to_ascii_lowercase();
        let lang_prefix = lang
            .split([',', '-', '_'])
            .next()
            .unwrap_or(lang)
            .to_ascii_lowercase();
        if locale_prefix != lang_prefix {
            push_issue(
                &mut issues,
                "warn",
                "accept_language",
                "accept_language and locale look inconsistent",
            );
        }
    }

    if let (Some(gpu_vendor), Some(webgl_vendor)) = (gpu_vendor.as_deref(), webgl_vendor.as_deref())
    {
        if !gpu_vendor.eq_ignore_ascii_case(webgl_vendor) {
            push_issue(
                &mut issues,
                "error",
                "webgl_vendor",
                "webgl_vendor must align with gpu_vendor for stable rendering identity",
            );
        }
    }
    if let (Some(gpu_renderer), Some(webgl_renderer)) =
        (gpu_renderer.as_deref(), webgl_renderer.as_deref())
    {
        if !gpu_renderer.eq_ignore_ascii_case(webgl_renderer) {
            push_issue(
                &mut issues,
                "warn",
                "webgl_renderer",
                "webgl_renderer looks inconsistent with gpu_renderer",
            );
        }
    }
    if let (Some(false), Some(points)) = (touch_support, max_touch_points) {
        if points > 0 {
            push_issue(
                &mut issues,
                "error",
                "max_touch_points",
                "max_touch_points cannot be positive when touch_support is false",
            );
        }
    }
    if let Some(points) = max_touch_points {
        if !(0..=16).contains(&points) {
            push_issue(
                &mut issues,
                "error",
                "max_touch_points",
                "max_touch_points is out of expected range",
            );
        }
    }
    if let (Some(ttl), Some(rotation_policy)) = (sticky_session_ttl, rotation_policy.as_deref()) {
        if ttl > 0
            && matches!(
                rotation_policy.trim().to_ascii_lowercase().as_str(),
                "per_request" | "per-request" | "every_request" | "every-request"
            )
        {
            push_issue(
                &mut issues,
                "error",
                "rotation_policy",
                "sticky_session_ttl conflicts with per-request rotation_policy",
            );
        }
    }

    if schema_kind == "canonical_grouped" && profile.get("family_id").is_none() {
        push_issue(
            &mut issues,
            "warn",
            "family_id",
            "canonical_grouped profile should declare family_id explicitly",
        );
    }

    let ok = !issues.iter().any(|i| i.level == "error");
    FingerprintValidationResult { ok, issues }
}
