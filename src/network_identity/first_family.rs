use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FIRST_FAMILY_ID: &str = "win11_business_laptop";
pub const FIRST_FAMILY_VARIANT_DEFAULT: &str = "mainstream_ultrabook";

pub const BROWSER_FIELDS: [&str; 10] = [
    "browser_family",
    "browser_channel",
    "browser_major_version",
    "browser_minor_version",
    "user_agent",
    "ua_platform",
    "ua_brand_list",
    "ua_full_version_list",
    "ua_mobile",
    "ua_architecture",
];

pub const OS_FIELDS: [&str; 10] = [
    "os_name",
    "os_version",
    "os_build_number",
    "os_edition",
    "os_branch",
    "system_locale",
    "ui_language",
    "region_format",
    "timezone",
    "daylight_saving_rule",
];

pub const DISPLAY_FIELDS: [&str; 10] = [
    "screen_width",
    "screen_height",
    "available_width",
    "available_height",
    "viewport_width",
    "viewport_height",
    "device_pixel_ratio",
    "page_zoom",
    "color_depth",
    "multi_monitor_count",
];

pub const HARDWARE_FIELDS: [&str; 10] = [
    "cpu_architecture",
    "hardware_concurrency",
    "device_memory_gb",
    "cpu_class",
    "gpu_vendor",
    "gpu_renderer",
    "touch_support",
    "max_touch_points",
    "battery_presence",
    "power_plan",
];

pub const RENDERING_FIELDS: [&str; 10] = [
    "canvas_profile",
    "webgl_vendor",
    "webgl_renderer",
    "webgl_version",
    "audio_profile",
    "font_fingerprint_profile",
    "media_codec_profile",
    "image_decode_profile",
    "color_gamut_profile",
    "hdr_support_profile",
];

pub const LOCALE_FIELDS: [&str; 10] = [
    "locale",
    "accept_language",
    "keyboard_layout",
    "input_method",
    "text_direction",
    "date_format",
    "number_format",
    "first_day_of_week",
    "typing_latency_profile",
    "punctuation_profile",
];

pub const NETWORK_FIELDS: [&str; 10] = [
    "proxy_type",
    "proxy_provider",
    "proxy_host",
    "proxy_port",
    "proxy_auth_mode",
    "proxy_region",
    "exit_ip",
    "dns_mode",
    "sticky_session_ttl",
    "rotation_policy",
];

pub const BEHAVIOR_FIELDS: [&str; 10] = [
    "click_speed_profile",
    "scroll_speed_profile",
    "pointer_smoothing_profile",
    "dwell_time_profile",
    "tab_switch_cadence",
    "idle_timeout_profile",
    "session_length_bucket",
    "automation_policy",
    "extension_profile",
    "isolation_mode",
];

const RUNTIME_SUPPORTED_CONTROL_FIELDS: [&str; 11] = [
    "accept_language",
    "timezone",
    "locale",
    "user_agent",
    "viewport_width",
    "viewport_height",
    "screen_width",
    "screen_height",
    "device_pixel_ratio",
    "hardware_concurrency",
    "device_memory_gb",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirstFamilySectionSummary {
    pub name: String,
    pub declared_fields: Vec<String>,
    pub declared_count: usize,
}

pub fn first_family_sections() -> [(&'static str, &'static [&'static str]); 8] {
    [
        ("browser", &BROWSER_FIELDS),
        ("os", &OS_FIELDS),
        ("display", &DISPLAY_FIELDS),
        ("hardware", &HARDWARE_FIELDS),
        ("rendering", &RENDERING_FIELDS),
        ("locale", &LOCALE_FIELDS),
        ("network", &NETWORK_FIELDS),
        ("behavior", &BEHAVIOR_FIELDS),
    ]
}

pub fn runtime_supported_control_fields() -> &'static [&'static str] {
    &RUNTIME_SUPPORTED_CONTROL_FIELDS
}

fn value_is_present(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(value) => !value.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(items) => !items.is_empty(),
        _ => true,
    }
}

fn direct_profile_field_value(profile: &Value, field: &str) -> Option<Value> {
    profile.get(field).cloned().filter(value_is_present)
}

fn grouped_profile_field_value(profile: &Value, section: &str, field: &str) -> Option<Value> {
    profile
        .get("control")
        .and_then(|value| value.get(section))
        .and_then(|value| value.get(field))
        .cloned()
        .filter(value_is_present)
        .or_else(|| {
            profile
                .get(section)
                .and_then(|value| value.get(field))
                .cloned()
                .filter(value_is_present)
        })
}

fn derived_platform_value(profile: &Value) -> Option<Value> {
    direct_profile_field_value(profile, "platform")
        .or_else(|| grouped_profile_field_value(profile, "browser", "ua_platform"))
        .or_else(|| {
            fingerprint_profile_field_value(profile, "os_name").and_then(|value| {
                value.as_str().and_then(|os_name| {
                    let platform = match os_name.trim().to_ascii_lowercase().as_str() {
                        "windows" | "win11" | "win10" => Some("Win32"),
                        "macos" | "mac os" | "darwin" => Some("MacIntel"),
                        "linux" => Some("Linux x86_64"),
                        _ => None,
                    }?;
                    Some(Value::String(platform.to_string()))
                })
            })
        })
}

pub fn first_family_control_field_section(field: &str) -> Option<&'static str> {
    for (section, fields) in first_family_sections() {
        if fields.iter().any(|candidate| candidate == &field) {
            return Some(section);
        }
    }
    None
}

pub fn fingerprint_profile_field_value(profile: &Value, field: &str) -> Option<Value> {
    match field {
        "device_memory_gb" => direct_profile_field_value(profile, "device_memory_gb")
            .or_else(|| direct_profile_field_value(profile, "device_memory"))
            .or_else(|| grouped_profile_field_value(profile, "hardware", "device_memory_gb")),
        "accept_language" => direct_profile_field_value(profile, "accept_language")
            .or_else(|| {
                profile
                    .get("headers")
                    .and_then(|headers| headers.get("accept_language"))
                    .cloned()
                    .filter(value_is_present)
            })
            .or_else(|| grouped_profile_field_value(profile, "locale", "accept_language")),
        "platform" => derived_platform_value(profile),
        _ => direct_profile_field_value(profile, field).or_else(|| {
            first_family_control_field_section(field)
                .and_then(|section| grouped_profile_field_value(profile, section, field))
        }),
    }
}

pub fn detect_fingerprint_schema_kind(profile: &Value) -> &'static str {
    let Some(profile_obj) = profile.as_object() else {
        return "generic";
    };
    if profile_obj
        .get("control")
        .and_then(Value::as_object)
        .is_some()
    {
        return "canonical_grouped";
    }
    if first_family_sections().iter().any(|(section, _)| {
        profile_obj
            .get(*section)
            .and_then(Value::as_object)
            .is_some()
    }) {
        return "sectioned_legacy";
    }
    if !first_family_declared_control_fields(profile).is_empty()
        || profile_obj
            .get("headers")
            .and_then(Value::as_object)
            .and_then(|headers| headers.get("accept_language"))
            .is_some()
    {
        return "legacy_flat";
    }
    "generic"
}

pub fn inferred_family_id(profile: &Value) -> Option<String> {
    profile
        .get("family_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            profile
                .get("familyId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| {
            (detect_fingerprint_schema_kind(profile) != "generic")
                .then(|| FIRST_FAMILY_ID.to_string())
        })
}

pub fn inferred_family_variant(profile: &Value) -> Option<String> {
    profile
        .get("family_variant")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            profile
                .get("familyVariant")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .or_else(|| inferred_family_id(profile).map(|_| FIRST_FAMILY_VARIANT_DEFAULT.to_string()))
}

pub fn first_family_declared_control_fields(profile: &Value) -> Vec<String> {
    let mut fields = BTreeSet::new();
    for (_, section_fields) in first_family_sections() {
        for field in section_fields {
            if fingerprint_profile_field_value(profile, field).is_some() {
                fields.insert((*field).to_string());
            }
        }
    }
    fields.into_iter().collect()
}

pub fn first_family_section_summaries(profile: &Value) -> Vec<FirstFamilySectionSummary> {
    first_family_sections()
        .into_iter()
        .map(|(section, fields)| {
            let declared_fields = fields
                .iter()
                .filter_map(|field| {
                    fingerprint_profile_field_value(profile, field)
                        .is_some()
                        .then(|| (*field).to_string())
                })
                .collect::<Vec<_>>();
            FirstFamilySectionSummary {
                name: section.to_string(),
                declared_count: declared_fields.len(),
                declared_fields,
            }
        })
        .filter(|summary| summary.declared_count > 0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_family_catalog_covers_eighty_controls() {
        let total = first_family_sections()
            .into_iter()
            .map(|(_, fields)| fields.len())
            .sum::<usize>();
        assert_eq!(total, 80);
    }

    #[test]
    fn grouped_schema_fields_are_resolved() {
        let profile = serde_json::json!({
            "family_id": FIRST_FAMILY_ID,
            "control": {
                "browser": {
                    "user_agent": "Mozilla/5.0",
                    "ua_platform": "Win32"
                },
                "display": {
                    "screen_width": 1920,
                    "viewport_width": 1536
                }
            }
        });
        assert_eq!(
            fingerprint_profile_field_value(&profile, "user_agent")
                .and_then(|value| value.as_str().map(str::to_string)),
            Some("Mozilla/5.0".to_string())
        );
        assert_eq!(
            fingerprint_profile_field_value(&profile, "platform")
                .and_then(|value| value.as_str().map(str::to_string)),
            Some("Win32".to_string())
        );
        assert_eq!(
            detect_fingerprint_schema_kind(&profile),
            "canonical_grouped"
        );
    }

    #[test]
    fn section_summaries_track_declared_fields() {
        let profile = serde_json::json!({
            "timezone": "Asia/Shanghai",
            "locale": "zh-CN",
            "accept_language": "zh-CN,zh;q=0.9",
            "screen_width": 1920,
            "screen_height": 1080,
            "viewport_width": 1536,
            "viewport_height": 864
        });
        let summaries = first_family_section_summaries(&profile);
        assert!(summaries.iter().any(|summary| {
            summary.name == "os"
                && summary
                    .declared_fields
                    .iter()
                    .any(|field| field == "timezone")
        }));
        assert!(summaries.iter().any(|summary| {
            summary.name == "display"
                && summary
                    .declared_fields
                    .iter()
                    .any(|field| field == "screen_width")
        }));
    }
}
