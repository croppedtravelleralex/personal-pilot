use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::fingerprint_policy::FingerprintPerfBudgetTag;
use super::first_family::{fingerprint_profile_field_value, first_family_declared_control_fields};

pub const FINGERPRINT_CONSUMPTION_VERSION: &str = "fingerprint_consumption_schema_v1";
pub const FINGERPRINT_CONSUMPTION_SOURCE_RUNTIME: &str = "runner_runtime";
pub const FINGERPRINT_CONSUMPTION_SOURCE_SHARED_SCHEMA: &str = "shared_schema_v1";

pub const DEVICE_MEMORY_ALIAS_FIELD: &str = "device_memory";
pub const DEVICE_MEMORY_CANONICAL_FIELD: &str = "device_memory_gb";

const LIGHTPANDA_SUPPORTED_FIELDS: [(&str, &str); 12] = [
    ("accept_language", "LIGHTPANDA_FP_ACCEPT_LANGUAGE"),
    ("timezone", "LIGHTPANDA_FP_TIMEZONE"),
    ("locale", "LIGHTPANDA_FP_LOCALE"),
    ("platform", "LIGHTPANDA_FP_PLATFORM"),
    ("user_agent", "LIGHTPANDA_FP_USER_AGENT"),
    ("viewport_width", "LIGHTPANDA_FP_VIEWPORT_WIDTH"),
    ("viewport_height", "LIGHTPANDA_FP_VIEWPORT_HEIGHT"),
    ("screen_width", "LIGHTPANDA_FP_SCREEN_WIDTH"),
    ("screen_height", "LIGHTPANDA_FP_SCREEN_HEIGHT"),
    ("device_pixel_ratio", "LIGHTPANDA_FP_DEVICE_PIXEL_RATIO"),
    ("hardware_concurrency", "LIGHTPANDA_FP_HARDWARE_CONCURRENCY"),
    (
        DEVICE_MEMORY_CANONICAL_FIELD,
        "LIGHTPANDA_FP_DEVICE_MEMORY_GB",
    ),
];

fn is_metadata_field(field: &str) -> bool {
    matches!(field, "id" | "name" | "version")
}

fn canonical_field_name(field: &str) -> &str {
    match field {
        DEVICE_MEMORY_ALIAS_FIELD => DEVICE_MEMORY_CANONICAL_FIELD,
        other => other,
    }
}

fn profile_value_as_env_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(v) => {
            let trimmed = v.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Bool(v) => Some(v.to_string()),
        Value::Number(v) => Some(v.to_string()),
        _ => None,
    }
}

fn insert_unique(target: &mut Vec<String>, value: &str) {
    if !target.iter().any(|item| item == value) {
        target.push(value.to_string());
    }
}

fn canonical_field_value(profile: &Value, field: &str) -> Option<Value> {
    match field {
        DEVICE_MEMORY_CANONICAL_FIELD => {
            fingerprint_profile_field_value(profile, "device_memory_gb")
        }
        "accept_language" => fingerprint_profile_field_value(profile, "accept_language"),
        "platform" => fingerprint_profile_field_value(profile, "platform"),
        other => fingerprint_profile_field_value(profile, other),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FingerprintConsumptionSnapshot {
    pub declared_fields: Vec<String>,
    pub resolved_fields: Vec<String>,
    pub applied_fields: Vec<String>,
    pub ignored_fields: Vec<String>,
    pub consumption_status: String,
    pub consumption_version: String,
    pub partial_support_warning: Option<String>,
}

impl FingerprintConsumptionSnapshot {
    pub fn declared_count(&self) -> usize {
        self.declared_fields.len()
    }

    pub fn resolved_count(&self) -> usize {
        self.resolved_fields.len()
    }

    pub fn applied_count(&self) -> usize {
        self.applied_fields.len()
    }

    pub fn ignored_count(&self) -> usize {
        self.ignored_fields.len()
    }
}

#[derive(Debug, Clone)]
pub struct FingerprintRuntimeProjection {
    pub envs: Vec<(String, String)>,
    pub env_keys: Vec<String>,
    pub consumption: FingerprintConsumptionSnapshot,
}

pub fn canonicalize_fingerprint_profile(profile: &Value) -> Value {
    let Some(profile_obj) = profile.as_object() else {
        return profile.clone();
    };
    let mut canonical = profile_obj.clone();
    for field in LIGHTPANDA_SUPPORTED_FIELDS.map(|(field, _)| field) {
        if !canonical.contains_key(field) {
            if let Some(value) = canonical_field_value(profile, field) {
                canonical.insert(field.to_string(), value);
            }
        }
    }
    if !canonical.contains_key("platform") {
        if let Some(value) = canonical_field_value(profile, "platform") {
            canonical.insert("platform".to_string(), value);
        }
    }
    Value::Object(canonical)
}

pub fn fingerprint_declared_fields(profile: &Value) -> Vec<String> {
    let Some(profile_obj) = profile.as_object() else {
        return Vec::new();
    };
    let mut fields = BTreeSet::new();
    for (key, _value) in profile_obj {
        if is_metadata_field(key) {
            continue;
        }
        if key == "headers" {
            if canonical_field_value(profile, "accept_language").is_some() {
                fields.insert("accept_language".to_string());
            }
            continue;
        }
        // Treat unknown object-shaped fields as declared input so consumption explainability
        // does not silently drift from partially_consumed to fully_consumed.
        fields.insert(canonical_field_name(key).to_string());
    }
    for field in first_family_declared_control_fields(profile) {
        fields.insert(field);
    }
    for field in LIGHTPANDA_SUPPORTED_FIELDS.map(|(field, _)| field) {
        if canonical_field_value(profile, field).is_some() {
            fields.insert(field.to_string());
        }
    }
    fields.into_iter().collect()
}

pub fn fingerprint_value_as_string(profile: &Value, field: &str) -> Option<String> {
    canonical_field_value(profile, field)
        .as_ref()
        .and_then(profile_value_as_env_string)
}

pub fn build_lightpanda_runtime_projection(
    profile_id: &str,
    profile_version: i64,
    profile: &Value,
) -> FingerprintRuntimeProjection {
    let canonical_profile = canonicalize_fingerprint_profile(profile);
    let declared_fields = fingerprint_declared_fields(&canonical_profile);
    let mut resolved_fields = Vec::new();
    let mut applied_fields = Vec::new();
    let mut envs = Vec::new();
    let mut env_keys = Vec::new();

    envs.push((
        "LIGHTPANDA_FP_PROFILE_ID".to_string(),
        profile_id.to_string(),
    ));
    envs.push((
        "LIGHTPANDA_FP_PROFILE_VERSION".to_string(),
        profile_version.to_string(),
    ));
    env_keys.push("LIGHTPANDA_FP_PROFILE_ID".to_string());
    env_keys.push("LIGHTPANDA_FP_PROFILE_VERSION".to_string());

    for (field, env_key) in LIGHTPANDA_SUPPORTED_FIELDS {
        if let Some(value) = canonical_field_value(&canonical_profile, field) {
            insert_unique(&mut resolved_fields, field);
            if let Some(value) = profile_value_as_env_string(&value) {
                envs.push((env_key.to_string(), value));
                env_keys.push(env_key.to_string());
                insert_unique(&mut applied_fields, field);
            }
        }
    }

    let ignored_fields = declared_fields
        .iter()
        .filter(|field| !applied_fields.iter().any(|applied| applied == *field))
        .cloned()
        .collect::<Vec<_>>();
    let consumption_status =
        fingerprint_consumption_status_from_counts(applied_fields.len(), ignored_fields.len())
            .to_string();
    let partial_support_warning = (!ignored_fields.is_empty()).then_some(
        "some declared fingerprint fields were not consumed by the current lightpanda runner"
            .to_string(),
    );
    FingerprintRuntimeProjection {
        envs,
        env_keys,
        consumption: FingerprintConsumptionSnapshot {
            declared_fields,
            resolved_fields,
            applied_fields,
            ignored_fields,
            consumption_status,
            consumption_version: FINGERPRINT_CONSUMPTION_VERSION.to_string(),
            partial_support_warning,
        },
    }
}

pub fn fingerprint_consumption_status_from_counts(
    applied_field_count: usize,
    ignored_field_count: usize,
) -> &'static str {
    if applied_field_count == 0 && ignored_field_count == 0 {
        "metadata_only"
    } else if applied_field_count == 0 {
        "ignored_only"
    } else if ignored_field_count == 0 {
        "fully_consumed"
    } else {
        "partially_consumed"
    }
}

pub fn fingerprint_perf_budget_tag_for_value(profile: &Value) -> FingerprintPerfBudgetTag {
    let declared_fields = fingerprint_declared_fields(profile);
    let has = |field: &str| declared_fields.iter().any(|item| item == field);
    if has("canvas")
        || has("canvas_profile")
        || has("webgl")
        || has("webgl_vendor")
        || has("webgl_renderer")
        || has("audio")
        || has("audio_profile")
        || has("fonts")
        || has("font_fingerprint_profile")
        || has("anti_detection_flags")
    {
        FingerprintPerfBudgetTag::Heavy
    } else if has("client_hints")
        || has("hardware_concurrency")
        || has(DEVICE_MEMORY_CANONICAL_FIELD)
        || has("color_scheme")
    {
        FingerprintPerfBudgetTag::Medium
    } else {
        FingerprintPerfBudgetTag::Light
    }
}

pub fn fingerprint_perf_budget_tag_from_json(
    profile_json: Option<&str>,
) -> FingerprintPerfBudgetTag {
    let Some(profile_json) = profile_json else {
        return FingerprintPerfBudgetTag::Light;
    };
    let Ok(profile) = serde_json::from_str::<Value>(profile_json) else {
        return FingerprintPerfBudgetTag::Light;
    };
    fingerprint_perf_budget_tag_for_value(&profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_device_memory_alias() {
        let profile = serde_json::json!({
            "timezone": "Asia/Shanghai",
            "device_memory": 8
        });
        let canonical = canonicalize_fingerprint_profile(&profile);
        assert_eq!(
            canonical
                .get(DEVICE_MEMORY_CANONICAL_FIELD)
                .and_then(Value::as_i64),
            Some(8)
        );
        let declared_fields = fingerprint_declared_fields(&canonical);
        assert!(declared_fields
            .iter()
            .any(|field| field == DEVICE_MEMORY_CANONICAL_FIELD));
        assert!(!declared_fields
            .iter()
            .any(|field| field == DEVICE_MEMORY_ALIAS_FIELD));
    }

    #[test]
    fn resolves_accept_language_from_headers_alias() {
        let profile = serde_json::json!({
            "headers": {
                "accept_language": "en-US,en;q=0.9"
            }
        });
        let projection = build_lightpanda_runtime_projection("fp-1", 1, &profile);
        assert!(projection
            .consumption
            .applied_fields
            .iter()
            .any(|field| field == "accept_language"));
        assert!(projection.envs.iter().any(
            |(key, value)| key == "LIGHTPANDA_FP_ACCEPT_LANGUAGE" && value == "en-US,en;q=0.9"
        ));
    }

    #[test]
    fn budget_classifier_uses_canonical_device_memory_field() {
        let profile = serde_json::json!({
            "device_memory": 8
        });
        assert_eq!(
            fingerprint_perf_budget_tag_for_value(&profile),
            FingerprintPerfBudgetTag::Medium
        );
    }

    #[test]
    fn unknown_object_fields_stay_visible_in_consumption_explainability() {
        let profile = serde_json::json!({
            "timezone": "Asia/Shanghai",
            "unsupported_blob": {"k": "v"}
        });
        let projection = build_lightpanda_runtime_projection("fp-unknown-object", 1, &profile);
        assert_eq!(
            projection.consumption.consumption_status,
            "partially_consumed"
        );
        assert!(projection
            .consumption
            .ignored_fields
            .iter()
            .any(|field| field == "unsupported_blob"));
        assert_eq!(projection.consumption.ignored_count(), 1);
    }

    #[test]
    fn supported_fields_only_remain_fully_consumed() {
        let profile = serde_json::json!({
            "timezone": "Asia/Shanghai",
            "locale": "zh-CN"
        });
        let projection = build_lightpanda_runtime_projection("fp-supported-only", 1, &profile);
        assert_eq!(projection.consumption.consumption_status, "fully_consumed");
        assert_eq!(projection.consumption.ignored_count(), 0);
    }
}
