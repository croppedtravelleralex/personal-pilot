use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};
use sqlx::Row;

use crate::{
    api::dto::{
        FormFieldInputRequest, FormInputRequest, FormSubmitInputRequest, FormSuccessInputRequest,
    },
    app::state::AppState,
    db::init::DbPool,
    runner::{
        RunnerExecutionIntent, RunnerFormActionPlan, RunnerFormErrorSignals, RunnerFormFieldPlan,
        RunnerFormSubmitPlan, RunnerFormSuccessPlan,
    },
};

use super::site_key_from_url;

pub const FORM_ACTION_STATUS_NOT_REQUESTED: &str = "not_requested";
pub const FORM_ACTION_STATUS_SHADOW_ONLY: &str = "shadow_only";
pub const FORM_ACTION_STATUS_BLOCKED: &str = "blocked";
pub const FORM_ACTION_STATUS_RUNNING: &str = "running";
pub const FORM_ACTION_STATUS_SUCCEEDED: &str = "succeeded";
pub const FORM_ACTION_STATUS_FAILED: &str = "failed";

#[derive(Debug, Clone)]
pub struct FormActionCompileResult {
    pub form_input_redacted_json: Option<Value>,
    pub form_action_plan: Option<RunnerFormActionPlan>,
    pub form_action_status: String,
    pub form_action_mode: Option<String>,
    pub form_action_summary_json: Option<Value>,
    pub artifact_metadata_json: Option<Value>,
    pub inline_secret_payload: Option<Value>,
}

#[derive(Debug, Clone)]
struct SiteFormContract {
    policy_id: String,
    policy_version: i64,
    contract_json: Map<String, Value>,
}

pub async fn compile_form_action_plan(
    db: &DbPool,
    task_kind: &str,
    url: Option<&str>,
    behavior_execution_mode: &str,
    execution_intent: &RunnerExecutionIntent,
    form_input: Option<FormInputRequest>,
) -> Result<FormActionCompileResult> {
    let Some(form_input) = form_input else {
        return Ok(FormActionCompileResult {
            form_input_redacted_json: None,
            form_action_plan: None,
            form_action_status: FORM_ACTION_STATUS_NOT_REQUESTED.to_string(),
            form_action_mode: None,
            form_action_summary_json: None,
            artifact_metadata_json: None,
            inline_secret_payload: None,
        });
    };

    let normalized = normalize_form_input(&form_input)?;
    validate_secret_requirements(execution_intent, &normalized)?;
    let site_contract =
        load_site_form_contract(db, url, Some(normalized.mode.as_str()), Some(task_kind)).await?;

    let contract_obj = site_contract
        .as_ref()
        .map(|item| &item.contract_json)
        .cloned()
        .unwrap_or_default();
    let field_roles = contract_obj
        .get("field_roles")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let contract_success = contract_obj
        .get("success")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let contract_error_signals = contract_obj
        .get("error_signals")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let (form_selector_source, form_selector) = first_non_empty_with_source(vec![
        ("task".to_string(), normalized.form_selector.clone()),
        (
            "site_policy".to_string(),
            contract_obj
                .get("primary_form_selector")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
        ("heuristic".to_string(), Some("form".to_string())),
    ])
    .unwrap_or_else(|| ("none".to_string(), None));

    let mut inline_fields = Map::new();
    let mut fields = Vec::new();
    let mut has_password_field = false;
    for field in &normalized.fields {
        if field.role == "password" {
            has_password_field = true;
        }
        let contract_selector = selector_from_field_roles(&field_roles, field);
        let heuristic_selector = heuristic_selector_for_role(&field.role);
        let (selector_source, selector) = first_non_empty_with_source(vec![
            ("task".to_string(), field.selector.clone()),
            ("site_policy".to_string(), contract_selector),
            (
                "heuristic".to_string(),
                if matches!(field.role.as_str(), "username" | "remember_me") {
                    heuristic_selector
                } else {
                    None
                },
            ),
        ])
        .unwrap_or_else(|| ("none".to_string(), None));

        let (value_source, secret_ref, bundle_key, resolved_value) =
            if let Some(value) = &field.value {
                inline_fields.insert(field.key.clone(), value.clone());
                ("inline".to_string(), None, None, None)
            } else if let Some(secret_ref) = &field.secret_ref {
                (
                    "secret_ref".to_string(),
                    Some(secret_ref.clone()),
                    None,
                    None,
                )
            } else {
                (
                    "bundle_key".to_string(),
                    None,
                    field.bundle_key.clone(),
                    None,
                )
            };

        fields.push(RunnerFormFieldPlan {
            key: field.key.clone(),
            role: field.role.clone(),
            selector,
            selector_source,
            required: field.required,
            sensitive: field.sensitive,
            value_source,
            secret_ref,
            bundle_key,
            resolved_value,
            resolved: false,
        });
    }

    let (submit_selector_source, submit_selector) = first_non_empty_with_source(vec![
        (
            "task".to_string(),
            normalized
                .submit
                .as_ref()
                .and_then(|item| item.selector.clone()),
        ),
        (
            "site_policy".to_string(),
            contract_obj
                .get("submit")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("selector"))
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    field_roles
                        .get("submit")
                        .and_then(Value::as_object)
                        .and_then(|obj| obj.get("selector"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                }),
        ),
    ])
    .unwrap_or_else(|| ("none".to_string(), None));
    let submit_trigger = normalized
        .submit
        .as_ref()
        .map(|item| item.trigger.clone())
        .unwrap_or_else(|| "click".to_string());

    let (ready_selector_source, ready_selector) = first_non_empty_with_source(vec![
        (
            "task".to_string(),
            normalized
                .success
                .as_ref()
                .and_then(|item| item.ready_selector.clone()),
        ),
        (
            "site_policy".to_string(),
            contract_success
                .get("ready_selector")
                .and_then(Value::as_str)
                .map(str::to_string),
        ),
    ])
    .unwrap_or_else(|| ("none".to_string(), None));
    let url_patterns = normalized
        .success
        .as_ref()
        .map(|item| item.url_patterns.clone())
        .or_else(|| {
            contract_success
                .get("url_patterns")
                .and_then(Value::as_array)
                .map(|items| string_array(items))
        })
        .unwrap_or_default();
    let title_contains = normalized
        .success
        .as_ref()
        .map(|item| item.title_contains.clone())
        .or_else(|| {
            contract_success
                .get("title_contains")
                .and_then(Value::as_array)
                .map(|items| string_array(items))
        })
        .unwrap_or_default();
    let error_signals = RunnerFormErrorSignals {
        login_error: selector_list_from_object(&contract_error_signals, "login_error"),
        field_error: selector_list_from_object(&contract_error_signals, "field_error"),
        account_locked: selector_list_from_object(&contract_error_signals, "account_locked"),
    };

    let mut blocked_reasons = Vec::new();
    if behavior_execution_mode == "active" {
        if normalized.mode == "auth" && site_contract.is_none() {
            blocked_reasons
                .push("active auth requires an active whitelist site form contract".to_string());
        }
        if normalized.mode == "auth" && !has_password_field {
            blocked_reasons.push("auth form requires a password field".to_string());
        }
        if normalized.mode == "auth"
            && !fields.iter().any(|field| {
                field.role == "password"
                    && field.selector.is_some()
                    && field.selector_source != "heuristic"
            })
        {
            blocked_reasons.push(
                "active auth requires an explicit password selector from task or site policy"
                    .to_string(),
            );
        }
        if submit_selector.is_none() || submit_selector_source == "heuristic" {
            blocked_reasons
                .push("active form action requires an explicit submit selector".to_string());
        }
        if ready_selector.is_none() || ready_selector_source == "heuristic" {
            blocked_reasons
                .push("active form action requires an explicit success.ready_selector".to_string());
        }
        for field in &fields {
            if field.required && field.selector.is_none() {
                blocked_reasons.push(format!(
                    "required field '{}' is missing a selector",
                    field.key
                ));
            }
        }
    }

    let submit = submit_selector.map(|selector| RunnerFormSubmitPlan {
        selector,
        selector_source: submit_selector_source,
        trigger: submit_trigger,
    });
    let success = ready_selector.map(|ready_selector| RunnerFormSuccessPlan {
        ready_selector,
        ready_selector_source,
        url_patterns,
        title_contains,
    });

    let blocked_reason = (!blocked_reasons.is_empty()).then(|| blocked_reasons.join("; "));
    let form_action_plan = RunnerFormActionPlan {
        plan_version: 1,
        mode: normalized.mode.clone(),
        execution_mode: behavior_execution_mode.to_string(),
        site_policy_id: site_contract.as_ref().map(|item| item.policy_id.clone()),
        site_policy_version: site_contract.as_ref().map(|item| item.policy_version),
        form_selector,
        form_selector_source,
        secret_bundle_ref: normalized.secret_bundle_ref.clone(),
        fields,
        submit,
        success,
        error_signals: Some(error_signals),
        retry_limit: 1,
        blocked_reason: blocked_reason.clone(),
    };

    let form_action_status = if behavior_execution_mode != "active" {
        FORM_ACTION_STATUS_SHADOW_ONLY.to_string()
    } else if blocked_reason.is_some() {
        FORM_ACTION_STATUS_BLOCKED.to_string()
    } else {
        FORM_ACTION_STATUS_RUNNING.to_string()
    };
    let form_action_summary_json = Some(build_form_action_summary_json(
        &form_action_plan,
        form_action_status.as_str(),
        0,
        blocked_reason.as_deref(),
        None,
    ));
    let form_input_redacted_json = Some(redacted_form_input_json(&normalized));
    let artifact_metadata_json = Some(json!({
        "form_action_plan": form_action_plan,
        "form_action_status": form_action_status,
        "form_action_mode": normalized.mode,
        "form_action_summary_json": form_action_summary_json,
        "form_input_redacted_json": form_input_redacted_json,
    }));

    Ok(FormActionCompileResult {
        form_input_redacted_json,
        form_action_plan: Some(form_action_plan),
        form_action_status,
        form_action_mode: Some(normalized.mode),
        form_action_summary_json,
        artifact_metadata_json,
        inline_secret_payload: (!inline_fields.is_empty())
            .then(|| json!({ "fields": inline_fields })),
    })
}

pub async fn resolve_form_action_plan_for_task(
    state: &AppState,
    task_id: &str,
    execution_intent: Option<&RunnerExecutionIntent>,
    plan: &RunnerFormActionPlan,
) -> Result<RunnerFormActionPlan> {
    let mut resolved_plan = plan.clone();
    let needs_inline = resolved_plan
        .fields
        .iter()
        .any(|field| field.value_source == "inline");
    let inline_payload = if needs_inline {
        let mut guard = state
            .inline_secret_vault
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.remove(task_id)
    } else {
        None
    };
    let inline_fields = inline_payload
        .as_ref()
        .and_then(|value| value.get("fields"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut bundle_cache: Option<Value> = None;
    for field in &mut resolved_plan.fields {
        let resolved_value = match field.value_source.as_str() {
            "inline" => inline_fields
                .get(&field.key)
                .cloned()
                .ok_or_else(|| anyhow!("inline_secret_unavailable for field '{}'", field.key))?,
            "secret_ref" => {
                resolve_secret_ref_value(
                    state,
                    execution_intent,
                    field
                        .secret_ref
                        .as_deref()
                        .ok_or_else(|| anyhow!("secret_ref missing for field '{}'", field.key))?,
                )
                .await?
            }
            "bundle_key" => {
                let bundle_ref = resolved_plan
                    .secret_bundle_ref
                    .as_deref()
                    .ok_or_else(|| anyhow!("secret_bundle_ref missing for bundle field"))?;
                if bundle_cache.is_none() {
                    bundle_cache =
                        Some(resolve_secret_ref_value(state, execution_intent, bundle_ref).await?);
                }
                bundle_cache
                    .as_ref()
                    .and_then(Value::as_object)
                    .and_then(|obj| {
                        field
                            .bundle_key
                            .as_ref()
                            .and_then(|key| obj.get(key))
                            .cloned()
                    })
                    .ok_or_else(|| {
                        anyhow!(
                            "bundle value not found for field '{}' key '{}'",
                            field.key,
                            field.bundle_key.clone().unwrap_or_default()
                        )
                    })?
            }
            other => return Err(anyhow!("unsupported form field value source '{}'", other)),
        };
        field.resolved_value = Some(resolved_value);
        field.resolved = true;
    }

    Ok(resolved_plan)
}

pub fn build_form_action_summary_json(
    plan: &RunnerFormActionPlan,
    status: &str,
    retry_count: i64,
    blocked_reason: Option<&str>,
    failure_signal: Option<&str>,
) -> Value {
    json!({
        "mode": plan.mode,
        "execution_mode": plan.execution_mode,
        "status": status,
        "retry_count": retry_count,
        "site_policy_id": plan.site_policy_id,
        "site_policy_version": plan.site_policy_version,
        "site_contract_present": plan.site_policy_id.is_some(),
        "blocked_reason": blocked_reason.or(plan.blocked_reason.as_deref()),
        "failure_signal": failure_signal,
        "success_ready_selector_seen": false,
        "post_login_actions_executed": false,
        "session_persisted": false,
        "form_selector_source": plan.form_selector_source,
        "secret_bundle_ref_present": plan.secret_bundle_ref.is_some(),
        "fields": plan.fields.iter().map(|field| json!({
            "key": field.key,
            "role": field.role,
            "selector_source": field.selector_source,
            "required": field.required,
            "sensitive": field.sensitive,
            "value_source": field.value_source,
            "resolved": field.resolved,
        })).collect::<Vec<_>>(),
        "success_ready_selector_present": plan.success.as_ref().map(|item| !item.ready_selector.is_empty()).unwrap_or(false),
        "error_signal_groups": plan.error_signals.as_ref().map(|item| {
            json!({
                "login_error": !item.login_error.is_empty(),
                "field_error": !item.field_error.is_empty(),
                "account_locked": !item.account_locked.is_empty(),
            })
        }).unwrap_or(Value::Null),
    })
}

fn normalize_form_input(form_input: &FormInputRequest) -> Result<NormalizedFormInput> {
    let mode = normalize_mode(form_input.mode.as_str())?;
    if form_input.fields.is_empty() {
        return Err(anyhow!("form_input.fields must not be empty"));
    }

    let mut fields = Vec::new();
    for field in &form_input.fields {
        fields.push(normalize_form_field(field)?);
    }

    let submit = form_input
        .submit
        .as_ref()
        .map(normalize_submit_input)
        .transpose()?;
    let success = form_input
        .success
        .as_ref()
        .map(normalize_success_input)
        .transpose()?;
    let secret_bundle_ref = normalize_optional_text(form_input.secret_bundle_ref.clone());
    if let Some(secret_bundle_ref) = &secret_bundle_ref {
        validate_secret_ref(secret_bundle_ref)?;
    }

    Ok(NormalizedFormInput {
        mode,
        form_selector: normalize_optional_text(form_input.form_selector.clone()),
        secret_bundle_ref,
        fields,
        submit,
        success,
    })
}

fn normalize_form_field(field: &FormFieldInputRequest) -> Result<NormalizedFormFieldInput> {
    let key = field.key.trim().to_string();
    if key.is_empty() {
        return Err(anyhow!("form_input.fields[*].key is required"));
    }
    let role = normalize_role(field.role.as_str())?;
    let value_sources = usize::from(field.value.is_some())
        + usize::from(field.secret_ref.is_some())
        + usize::from(field.bundle_key.is_some());
    if value_sources != 1 {
        return Err(anyhow!(
            "form_input.fields[*].value, secret_ref, bundle_key must contain exactly one source"
        ));
    }
    if let Some(secret_ref) = &field.secret_ref {
        validate_secret_ref(secret_ref)?;
    }

    Ok(NormalizedFormFieldInput {
        key,
        role,
        selector: normalize_optional_text(field.selector.clone()),
        required: field.required.unwrap_or(false),
        sensitive: field.sensitive.unwrap_or(false),
        value: field.value.clone(),
        secret_ref: normalize_optional_text(field.secret_ref.clone()),
        bundle_key: normalize_optional_text(field.bundle_key.clone()),
    })
}

fn normalize_submit_input(input: &FormSubmitInputRequest) -> Result<NormalizedFormSubmitInput> {
    let trigger = input
        .trigger
        .clone()
        .unwrap_or_else(|| "click".to_string())
        .trim()
        .to_string();
    if trigger != "click" {
        return Err(anyhow!("form_input.submit.trigger must be click"));
    }
    Ok(NormalizedFormSubmitInput {
        selector: normalize_optional_text(input.selector.clone()),
        trigger,
    })
}

fn normalize_success_input(input: &FormSuccessInputRequest) -> Result<NormalizedFormSuccessInput> {
    Ok(NormalizedFormSuccessInput {
        ready_selector: normalize_optional_text(input.ready_selector.clone()),
        url_patterns: input
            .url_patterns
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
        title_contains: input
            .title_contains
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect(),
    })
}

fn validate_secret_requirements(
    execution_intent: &RunnerExecutionIntent,
    form_input: &NormalizedFormInput,
) -> Result<()> {
    let requires_identity = form_input.secret_bundle_ref.is_some()
        || form_input
            .fields
            .iter()
            .any(|field| field.secret_ref.is_some());
    if requires_identity && execution_intent.identity_profile_id.is_none() {
        return Err(anyhow!(
            "identity_profile_id is required when using identity:// secret refs"
        ));
    }
    Ok(())
}

async fn load_site_form_contract(
    db: &DbPool,
    url: Option<&str>,
    page_archetype: Option<&str>,
    action_kind: Option<&str>,
) -> Result<Option<SiteFormContract>> {
    let site_key = site_key_from_url(url);
    let Some(site_key) = site_key else {
        return Ok(None);
    };

    let row = sqlx::query(
        r#"SELECT id, version, override_json
           FROM site_behavior_policies
           WHERE status = 'active'
             AND site_key = ?
             AND (? IS NULL OR page_archetype IS NULL OR page_archetype = ?)
             AND (? IS NULL OR action_kind IS NULL OR action_kind = ?)
           ORDER BY priority DESC, created_at DESC, id DESC
           LIMIT 1"#,
    )
    .bind(site_key)
    .bind(page_archetype)
    .bind(page_archetype)
    .bind(action_kind)
    .bind(action_kind)
    .fetch_optional(db)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };
    let policy_id: String = row.try_get("id")?;
    let policy_version: i64 = row.try_get("version")?;
    let override_json: Option<String> = row.try_get("override_json")?;
    let contract_json = override_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|value| value.get("form_contract").cloned())
        .and_then(|value| value.as_object().cloned());
    Ok(contract_json.map(|contract_json| SiteFormContract {
        policy_id,
        policy_version,
        contract_json,
    }))
}

fn selector_from_field_roles(
    field_roles: &Map<String, Value>,
    field: &NormalizedFormFieldInput,
) -> Option<String> {
    field_roles
        .get(field.role.as_str())
        .and_then(Value::as_object)
        .and_then(|obj| obj.get("selector"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            field_roles
                .get(field.key.as_str())
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("selector"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn selector_list_from_object(obj: &Map<String, Value>, key: &str) -> Vec<String> {
    obj.get(key)
        .and_then(Value::as_array)
        .map(|items| string_array(items))
        .unwrap_or_default()
}

fn heuristic_selector_for_role(role: &str) -> Option<String> {
    match role {
        "username" => Some(
            "input[name='username'], input[name='email'], input[type='email'], input[id*='user' i], input[id*='email' i]"
                .to_string(),
        ),
        "remember_me" => Some(
            "input[type='checkbox'][name*='remember' i], input[type='checkbox'][id*='remember' i]"
                .to_string(),
        ),
        _ => None,
    }
}

fn resolve_secret_alias(alias: &str) -> Result<&str> {
    alias
        .strip_prefix("identity://")
        .filter(|item| !item.trim().is_empty())
        .ok_or_else(|| anyhow!("secret ref must use identity://<alias> syntax"))
}

async fn resolve_secret_ref_value(
    state: &AppState,
    execution_intent: Option<&RunnerExecutionIntent>,
    secret_ref: &str,
) -> Result<Value> {
    let identity_profile_id = execution_intent
        .and_then(|intent| intent.identity_profile_id.as_deref())
        .ok_or_else(|| anyhow!("identity_profile_id is required to resolve secret refs"))?;
    let alias = resolve_secret_alias(secret_ref)?;
    let secret_aliases_json: Option<String> = sqlx::query_scalar(
        r#"SELECT secret_aliases_json FROM identity_profiles WHERE id = ? AND status = 'active'"#,
    )
    .bind(identity_profile_id)
    .fetch_optional(&state.db)
    .await?;
    let aliases = secret_aliases_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|value| value.as_object().cloned())
        .ok_or_else(|| anyhow!("identity profile has no secret_aliases_json"))?;
    let alias_config = aliases
        .get(alias)
        .cloned()
        .ok_or_else(|| anyhow!("secret alias not found: {}", alias))?;
    resolve_alias_config_value(&alias_config)
}

fn resolve_alias_config_value(alias_config: &Value) -> Result<Value> {
    match alias_config {
        Value::String(env_name) => read_env_secret(env_name, false),
        Value::Object(obj) => {
            let resolver = obj
                .get("resolver")
                .or_else(|| obj.get("kind"))
                .or_else(|| obj.get("source"))
                .and_then(Value::as_str)
                .unwrap_or("env");
            let env_name = obj
                .get("env")
                .and_then(Value::as_str)
                .ok_or_else(|| anyhow!("secret alias config is missing env"))?;
            match resolver {
                "env" => read_env_secret(env_name, false),
                "env_json" => read_env_secret(env_name, true),
                other => Err(anyhow!("unsupported secret alias resolver '{}'", other)),
            }
        }
        _ => Err(anyhow!("invalid secret alias config")),
    }
}

fn read_env_secret(env_name: &str, parse_json: bool) -> Result<Value> {
    let raw = std::env::var(env_name).map_err(|_| anyhow!("secret env not found: {}", env_name))?;
    if parse_json {
        serde_json::from_str::<Value>(&raw)
            .map_err(|err| anyhow!("failed to parse env_json secret '{}': {}", env_name, err))
    } else {
        Ok(Value::String(raw))
    }
}

fn redacted_form_input_json(input: &NormalizedFormInput) -> Value {
    json!({
        "mode": input.mode,
        "form_selector": input.form_selector,
        "secret_bundle_ref": input.secret_bundle_ref,
        "fields": input.fields.iter().map(|field| json!({
            "key": field.key,
            "role": field.role,
            "selector": field.selector,
            "required": field.required,
            "sensitive": field.sensitive,
            "value_source": if field.value.is_some() { "inline" } else if field.secret_ref.is_some() { "secret_ref" } else { "bundle_key" },
            "secret_ref": field.secret_ref,
            "bundle_key": field.bundle_key,
            "resolved": false,
        })).collect::<Vec<_>>(),
        "submit": input.submit.as_ref().map(|submit| json!({
            "selector": submit.selector,
            "trigger": submit.trigger,
        })),
        "success": input.success.as_ref().map(|success| json!({
            "ready_selector": success.ready_selector,
            "url_patterns": success.url_patterns,
            "title_contains": success.title_contains,
        })),
    })
}

fn normalize_mode(mode: &str) -> Result<String> {
    match mode.trim() {
        "auth" | "form" => Ok(mode.trim().to_string()),
        other => Err(anyhow!("form_input.mode must be auth|form, got {}", other)),
    }
}

fn normalize_role(role: &str) -> Result<String> {
    match role.trim() {
        "username" | "password" | "remember_me" | "custom" => Ok(role.trim().to_string()),
        other => Err(anyhow!(
            "form_input.fields[*].role must be username|password|remember_me|custom, got {}",
            other
        )),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn validate_secret_ref(secret_ref: &str) -> Result<()> {
    let _ = resolve_secret_alias(secret_ref)?;
    Ok(())
}

fn string_array(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| item.as_str().map(|value| value.trim().to_string()))
        .filter(|item| !item.is_empty())
        .collect()
}

fn first_non_empty_with_source(
    candidates: Vec<(String, Option<String>)>,
) -> Option<(String, Option<String>)> {
    candidates
        .into_iter()
        .find(|(_, value)| value.as_ref().is_some_and(|item| !item.trim().is_empty()))
}

#[derive(Debug, Clone)]
struct NormalizedFormInput {
    mode: String,
    form_selector: Option<String>,
    secret_bundle_ref: Option<String>,
    fields: Vec<NormalizedFormFieldInput>,
    submit: Option<NormalizedFormSubmitInput>,
    success: Option<NormalizedFormSuccessInput>,
}

#[derive(Debug, Clone)]
struct NormalizedFormFieldInput {
    key: String,
    role: String,
    selector: Option<String>,
    required: bool,
    sensitive: bool,
    value: Option<Value>,
    secret_ref: Option<String>,
    bundle_key: Option<String>,
}

#[derive(Debug, Clone)]
struct NormalizedFormSubmitInput {
    selector: Option<String>,
    trigger: String,
}

#[derive(Debug, Clone)]
struct NormalizedFormSuccessInput {
    ready_selector: Option<String>,
    url_patterns: Vec<String>,
    title_contains: Vec<String>,
}
