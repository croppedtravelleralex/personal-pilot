use serde::{Deserialize, Serialize};
use serde_json::Value;

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

fn get_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|x| !x.is_empty())
}

fn get_i64(v: &Value, key: &str) -> Option<i64> {
    v.get(key).and_then(|x| x.as_i64())
}

pub fn validate_fingerprint_profile(profile: &Value) -> FingerprintValidationResult {
    let mut issues = Vec::new();

    let timezone = get_str(profile, "timezone");
    let locale = get_str(profile, "locale");
    let accept_language = get_str(profile, "accept_language");
    let platform = get_str(profile, "platform");
    let viewport_width = get_i64(profile, "viewport_width");
    let viewport_height = get_i64(profile, "viewport_height");
    let screen_width = get_i64(profile, "screen_width");
    let screen_height = get_i64(profile, "screen_height");
    let device_memory_gb = get_i64(profile, "device_memory_gb");
    let hardware_concurrency = get_i64(profile, "hardware_concurrency");

    if timezone.is_none() {
        issues.push(FingerprintValidationIssue {
            level: "warn".into(),
            field: "timezone".into(),
            message: "timezone is missing".into(),
        });
    }
    if locale.is_none() {
        issues.push(FingerprintValidationIssue {
            level: "warn".into(),
            field: "locale".into(),
            message: "locale is missing".into(),
        });
    }
    if accept_language.is_none() {
        issues.push(FingerprintValidationIssue {
            level: "warn".into(),
            field: "accept_language".into(),
            message: "accept_language is missing".into(),
        });
    }
    if platform.is_none() {
        issues.push(FingerprintValidationIssue {
            level: "warn".into(),
            field: "platform".into(),
            message: "platform is missing".into(),
        });
    }

    if let (Some(vw), Some(sw)) = (viewport_width, screen_width) {
        if vw > sw {
            issues.push(FingerprintValidationIssue {
                level: "error".into(),
                field: "viewport_width".into(),
                message: "viewport_width cannot exceed screen_width".into(),
            });
        }
    }
    if let (Some(vh), Some(sh)) = (viewport_height, screen_height) {
        if vh > sh {
            issues.push(FingerprintValidationIssue {
                level: "error".into(),
                field: "viewport_height".into(),
                message: "viewport_height cannot exceed screen_height".into(),
            });
        }
    }

    if let Some(mem) = device_memory_gb {
        if !(1..=128).contains(&mem) {
            issues.push(FingerprintValidationIssue {
                level: "error".into(),
                field: "device_memory_gb".into(),
                message: "device_memory_gb is out of expected range".into(),
            });
        }
    }

    if let Some(cpu) = hardware_concurrency {
        if !(1..=128).contains(&cpu) {
            issues.push(FingerprintValidationIssue {
                level: "error".into(),
                field: "hardware_concurrency".into(),
                message: "hardware_concurrency is out of expected range".into(),
            });
        }
    }

    if let (Some(locale), Some(lang)) = (locale, accept_language) {
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
            issues.push(FingerprintValidationIssue {
                level: "warn".into(),
                field: "accept_language".into(),
                message: "accept_language and locale look inconsistent".into(),
            });
        }
    }

    let ok = !issues.iter().any(|i| i.level == "error");
    FingerprintValidationResult { ok, issues }
}
