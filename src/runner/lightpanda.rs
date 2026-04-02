use async_trait::async_trait;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io,
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
    time::{timeout, Duration},
};

use crate::{
    domain::run::RUN_STATUS_TIMED_OUT,
    runner::{
        RunnerCancelResult, RunnerCapabilities, RunnerExecutionResult, RunnerOutcomeStatus,
        RunnerTask, TaskRunner,
    },
};

#[derive(Clone, Default)]
pub struct LightpandaRunner {
    running_tasks: Arc<Mutex<HashMap<String, u32>>>,
}

#[derive(Debug, Clone)]
struct LightpandaFingerprintRuntime {
    envs: Vec<(String, String)>,
    applied_fields: Vec<String>,
    ignored_fields: Vec<String>,
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

fn build_lightpanda_fingerprint_runtime(task: &RunnerTask) -> Option<LightpandaFingerprintRuntime> {
    let profile = task.fingerprint_profile.as_ref()?;
    let profile_obj = profile.profile_json.as_object()?;

    let field_map = [
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
        ("device_memory_gb", "LIGHTPANDA_FP_DEVICE_MEMORY_GB"),
    ];

    let mut envs = Vec::new();
    let mut applied_fields = Vec::new();
    let mut ignored_fields = Vec::new();

    envs.push(("LIGHTPANDA_FP_PROFILE_ID".to_string(), profile.id.clone()));
    envs.push(("LIGHTPANDA_FP_PROFILE_VERSION".to_string(), profile.version.to_string()));
    applied_fields.push("profile_id".to_string());
    applied_fields.push("profile_version".to_string());

    for (field, env_name) in field_map {
        match profile_obj.get(field) {
            Some(value) => match profile_value_as_env_string(value) {
                Some(value) => {
                    envs.push((env_name.to_string(), value));
                    applied_fields.push(field.to_string());
                }
                None => ignored_fields.push(field.to_string()),
            },
            None => {}
        }
    }

    for key in profile_obj.keys() {
        if !field_map.iter().any(|(field, _)| field == key) {
            ignored_fields.push(key.clone());
        }
    }

    Some(LightpandaFingerprintRuntime {
        envs,
        applied_fields,
        ignored_fields,
    })
}

fn result_payload(
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    failure_scope: Option<&str>,
    browser_failure_signal: Option<&str>,
    requested_action: &str,
    action: &str,
    task: &RunnerTask,
    url: Option<&str>,
    timeout_seconds: Option<u64>,
    bin: Option<&str>,
    exit_code: Option<i32>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    fingerprint_runtime: Option<&LightpandaFingerprintRuntime>,
    message: &str,
) -> Value {
    let fingerprint_profile = task.fingerprint_profile.as_ref().map(|profile| json!({
        "id": profile.id,
        "version": profile.version,
        "profile": profile.profile_json,
    }));
    let proxy_json = task.proxy.as_ref().map(|proxy| json!({
        "id": proxy.id,
        "scheme": proxy.scheme,
        "host": proxy.host,
        "port": proxy.port,
        "region": proxy.region,
        "country": proxy.country,
        "provider": proxy.provider,
        "score": proxy.score,
        "resolution_status": proxy.resolution_status,
    }));

    let html_preview = if action == "get_html" {
        stdout_preview.clone()
    } else {
        None
    };

    let fingerprint_runtime_json = fingerprint_runtime.map(|runtime| {
        let supported_field_count = runtime.applied_fields.iter().filter(|f| *f != "profile_id" && *f != "profile_version").count();
        let unsupported_field_count = runtime.ignored_fields.len();
        let consumption_status = if supported_field_count == 0 && unsupported_field_count == 0 {
            "metadata_only"
        } else if supported_field_count == 0 {
            "ignored_only"
        } else if unsupported_field_count == 0 {
            "fully_consumed"
        } else {
            "partially_consumed"
        };
        json!({
            "env_keys": runtime.envs.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>(),
            "applied_fields": runtime.applied_fields,
            "ignored_fields": runtime.ignored_fields,
            "applied_count": runtime.applied_fields.len(),
            "ignored_count": runtime.ignored_fields.len(),
            "supported_field_count": supported_field_count,
            "unsupported_field_count": unsupported_field_count,
            "consumption_status": consumption_status,
            "warning": (!runtime.ignored_fields.is_empty()).then_some("fingerprint profile contains fields that lightpanda does not currently consume"),
        })
    });

    json!({
        "runner": "lightpanda",
        "requested_action": requested_action,
        "action": action,
        "supported_actions": supported_actions(),
        "capability_stage": "minimal_real_execution_v1",
        "ok": ok,
        "status": status,
        "error_kind": error_kind,
        "failure_scope": failure_scope,
        "browser_failure_signal": browser_failure_signal,
        "task_id": task.task_id,
        "attempt": task.attempt,
        "kind": task.kind,
        "payload": task.payload,
        "url": url,
        "timeout_seconds": timeout_seconds,
        "fingerprint_profile": fingerprint_profile,
        "proxy": proxy_json,
        "fingerprint_runtime": fingerprint_runtime_json,
        "bin": bin,
        "exit_code": exit_code,
        "stdout_preview": stdout_preview,
        "stderr_preview": stderr_preview,
        "html_preview": html_preview,
        "content_kind": (action == "get_html").then_some("text/html"),
        "message": message,
    })
}

fn build_result(
    outcome: RunnerOutcomeStatus,
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    requested_action: &str,
    action: &str,
    task: &RunnerTask,
    url: Option<&str>,
    timeout_seconds: Option<u64>,
    bin: Option<&str>,
    exit_code: Option<i32>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    fingerprint_runtime: Option<&LightpandaFingerprintRuntime>,
    message: impl Into<String>,
) -> RunnerExecutionResult {
    let message = message.into();
    let is_error = matches!(outcome, RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::TimedOut);
    let browser_failure_signal = detect_browser_failure_signal(stderr_preview.as_deref(), stdout_preview.as_deref());
    let failure_scope = is_error.then_some(runner_failure_scope(
        error_kind,
        browser_failure_signal,
        matches!(outcome, RunnerOutcomeStatus::TimedOut),
    ));

    RunnerExecutionResult {
        status: outcome,
        result_json: Some(result_payload(
            ok,
            status,
            error_kind,
            failure_scope,
            browser_failure_signal,
            requested_action,
            action,
            task,
            url,
            timeout_seconds,
            bin,
            exit_code,
            stdout_preview,
            stderr_preview,
            fingerprint_runtime,
            &message,
        )),
        error_message: is_error.then_some(message.clone()),
        summary_artifacts: vec![crate::runner::types::RunnerSummaryArtifact {
            category: crate::runner::types::SummaryArtifactCategory::Execution,
            key: format!("{}.execution", task.kind),
            source: "runner.lightpanda".to_string(),
            severity: if is_error { crate::runner::types::SummaryArtifactSeverity::Error } else { crate::runner::types::SummaryArtifactSeverity::Info },
            title: execution_summary_title(action, status, error_kind),
            summary: format!(
                "{} failure_scope={} browser_failure_signal={}",
                execution_summary_text(action, task, status, error_kind, exit_code, timeout_seconds, &message),
                failure_scope.unwrap_or("none"),
                browser_failure_signal.unwrap_or("none"),
            ),
        }],
    }
}

fn invalid_input(task: &RunnerTask, requested_action: &str, action: &str, message: &str, url: Option<&str>) -> RunnerExecutionResult {
    build_result(
        RunnerOutcomeStatus::Failed,
        false,
        "failed",
        Some("invalid_input"),
        requested_action,
        action,
        task,
        url,
        None,
        None,
        None,
        None,
        None,
        None,
        message,
    )
}

fn extract_action(payload: &Value) -> String {
    payload
        .get("action")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("open_page")
        .to_string()
}

fn normalize_action(action: &str) -> Option<&'static str> {
    match action {
        "open_page" | "fetch" => Some("open_page"),
        "get_html" => Some("get_html"),
        _ => None,
    }
}

fn supported_actions() -> &'static [&'static str] {
    &["open_page", "fetch", "get_html"]
}

fn extract_url(payload: &Value) -> Option<String> {
    payload
        .get("url")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn looks_like_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

fn lightpanda_bin() -> String {
    std::env::var("LIGHTPANDA_BIN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "lightpanda".to_string())
}

fn truncate_output(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    let mut out: String = trimmed.chars().take(max_chars).collect();
    if trimmed.chars().count() > max_chars {
        out.push_str("...[truncated]");
    }
    out
}

fn preview_if_non_empty(raw: String, max_chars: usize) -> Option<String> {
    let preview = truncate_output(&raw, max_chars);
    (!preview.is_empty()).then_some(preview)
}

fn classify_spawn_error(err: &io::Error) -> &'static str {
    match err.kind() {
        io::ErrorKind::NotFound => "binary_not_found",
        io::ErrorKind::PermissionDenied => "spawn_permission_denied",
        _ => "spawn_failed",
    }
}

fn classify_exit_code(exit_code: Option<i32>) -> &'static str {
    match exit_code {
        Some(126) => "runner_invocation_not_executable",
        Some(127) => "runner_command_not_found",
        Some(code) if code >= 128 => "runner_terminated_by_signal",
        Some(_) => "runner_non_zero_exit",
        None => "runner_non_zero_exit",
    }
}

fn execution_summary_title(action: &str, status: &str, error_kind: Option<&str>) -> String {
    match (status, error_kind) {
        ("succeeded", _) => format!("{action} execution succeeded"),
        ("timed_out", Some("timeout")) => format!("{action} execution timed out"),
        ("failed", Some(kind)) => format!("{action} execution failed ({kind})"),
        ("failed", None) => format!("{action} execution failed"),
        _ => format!("{action} execution {status}"),
    }
}

fn execution_summary_text(action: &str, task: &RunnerTask, status: &str, error_kind: Option<&str>, exit_code: Option<i32>, timeout_seconds: Option<u64>, message: &str) -> String {
    format!(
        "kind={} action={} status={} error_kind={} exit_code={:?} timeout_seconds={:?} message={}",
        task.kind,
        action,
        status,
        error_kind.unwrap_or("none"),
        exit_code,
        timeout_seconds,
        message,
    )
}

fn detect_browser_failure_signal(stderr_preview: Option<&str>, stdout_preview: Option<&str>) -> Option<&'static str> {
    let stderr = stderr_preview.unwrap_or("").to_ascii_lowercase();
    let stdout = stdout_preview.unwrap_or("").to_ascii_lowercase();
    let combined = format!("{}
{}", stderr, stdout);

    if combined.contains("timeout") || combined.contains("timed out") {
        Some("browser_timeout_signal")
    } else if combined.contains("navigation") && combined.contains("fail") {
        Some("browser_navigation_failure_signal")
    } else if combined.contains("dns") || combined.contains("name not resolved") {
        Some("browser_dns_failure_signal")
    } else if combined.contains("certificate") || combined.contains("tls") || combined.contains("ssl") {
        Some("browser_tls_failure_signal")
    } else {
        None
    }
}

fn runner_failure_scope(error_kind: Option<&str>, browser_failure_signal: Option<&str>, timed_out: bool) -> &'static str {
    if timed_out {
        "runner_timeout"
    } else if browser_failure_signal.is_some() {
        "browser_execution"
    } else {
        match error_kind {
            Some("binary_not_found") | Some("spawn_permission_denied") | Some("spawn_failed") => "runner_invocation",
            Some("process_wait_failed") => "runner_process_wait",
            Some("runner_command_not_found") | Some("runner_invocation_not_executable") | Some("runner_terminated_by_signal") | Some("runner_non_zero_exit") => "runner_process_exit",
            _ => "runner_execution",
        }
    }
}

async fn terminate_pid(pid: u32) -> Result<(), String> {
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .status()
        .await
        .map_err(|err| format!("failed to spawn kill command: {err}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("kill -TERM exited with status {:?}", status.code()))
    }
}

async fn read_stream_to_string<R>(reader: Option<R>) -> String
where
    R: tokio::io::AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return String::new();
    };

    let mut buf = Vec::new();
    match reader.read_to_end(&mut buf).await {
        Ok(_) => String::from_utf8_lossy(&buf).to_string(),
        Err(err) => format!("<failed to read stream: {err}>"),
    }
}

impl LightpandaRunner {
    fn register_child(&self, task_id: &str, child: &Child) {
        if let Some(pid) = child.id() {
            let mut guard = self
                .running_tasks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.insert(task_id.to_string(), pid);
        }
    }

    fn unregister_child(&self, task_id: &str) {
        let mut guard = self
            .running_tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.remove(task_id);
    }
}

#[async_trait]
impl TaskRunner for LightpandaRunner {
    fn name(&self) -> &'static str {
        "lightpanda"
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_timeout: true,
            supports_cancel_running: true,
            supports_artifacts: false,
        }
    }

    async fn cancel_running(&self, task_id: &str) -> RunnerCancelResult {
        let pid = {
            let guard = self
                .running_tasks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.get(task_id).copied()
        };

        match pid {
            Some(pid) => match terminate_pid(pid).await {
                Ok(()) => {
                    self.unregister_child(task_id);
                    RunnerCancelResult {
                        accepted: true,
                        message: format!(
                            "lightpanda runner sent SIGTERM to running process for task_id={task_id}, pid={pid}"
                        ),
                    }
                }
                Err(err) => RunnerCancelResult {
                    accepted: false,
                    message: format!(
                        "lightpanda runner failed to terminate process for task_id={task_id}, pid={pid}: {err}"
                    ),
                },
            },
            None => RunnerCancelResult {
                accepted: false,
                message: format!(
                    "lightpanda runner has no registered running process for task_id={task_id}"
                ),
            },
        }
    }

    async fn execute(&self, task: RunnerTask) -> RunnerExecutionResult {
        let requested_action = extract_action(&task.payload);
        let action = match normalize_action(&requested_action) {
            Some(action) => action,
            None => {
                return invalid_input(
                    &task,
                    requested_action.as_str(),
                    requested_action.as_str(),
                    "lightpanda runner currently supports only action=open_page, action=get_html (fetch is accepted as an alias for open_page)",
                    extract_url(&task.payload).as_deref(),
                )
            }
        };

        let url = match extract_url(&task.payload) {
            Some(url) => url,
            None => {
                return invalid_input(
                    &task,
                    requested_action.as_str(),
                    action,
                    "lightpanda runner requires a non-empty url in task payload",
                    None,
                )
            }
        };

        if !looks_like_url(&url) {
            return invalid_input(
                &task,
                requested_action.as_str(),
                action,
                "lightpanda runner currently only accepts http:// or https:// urls",
                Some(&url),
            );
        }

        let timeout_seconds = task.timeout_seconds.unwrap_or(10).clamp(1, 120) as u64;
        let bin = lightpanda_bin();
        let fingerprint_runtime = build_lightpanda_fingerprint_runtime(&task);

        let mut cmd = Command::new(&bin);
        cmd.arg("fetch")
            .arg("--log-format")
            .arg("pretty")
            .arg("--log-level")
            .arg("info")
            .arg(&url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(runtime) = &fingerprint_runtime {
            for (key, value) in &runtime.envs {
                cmd.env(key, value);
            }
        }
        if let Some(proxy) = &task.proxy {
            cmd.env("LIGHTPANDA_PROXY_ID", &proxy.id)
                .env("LIGHTPANDA_PROXY_SCHEME", &proxy.scheme)
                .env("LIGHTPANDA_PROXY_HOST", &proxy.host)
                .env("LIGHTPANDA_PROXY_PORT", proxy.port.to_string());
            if let Some(username) = &proxy.username {
                cmd.env("LIGHTPANDA_PROXY_USERNAME", username);
            }
            if let Some(password) = &proxy.password {
                cmd.env("LIGHTPANDA_PROXY_PASSWORD", password);
            }
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                let message = format!("failed to spawn lightpanda binary: {err}");
                return build_result(
                    RunnerOutcomeStatus::Failed,
                    false,
                    "failed",
                    Some(classify_spawn_error(&err)),
                    requested_action.as_str(),
                    action,
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(&bin),
                    None,
                    None,
                    None,
                    fingerprint_runtime.as_ref(),
                    message,
                );
            }
        };

        self.register_child(&task.task_id, &child);

        let stdout_handle = tokio::spawn(read_stream_to_string(child.stdout.take()));
        let stderr_handle = tokio::spawn(read_stream_to_string(child.stderr.take()));

        let wait_result = timeout(Duration::from_secs(timeout_seconds), child.wait()).await;

        let (outcome, status_text, error_kind, message, exit_code) = match wait_result {
            Ok(waited) => match waited {
                Ok(status) if status.success() => (
                    RunnerOutcomeStatus::Succeeded,
                    "succeeded",
                    None,
                    "lightpanda fetch completed successfully".to_string(),
                    status.code(),
                ),
                Ok(status) => {
                    let exit_code = status.code();
                    (
                        RunnerOutcomeStatus::Failed,
                        "failed",
                        Some(classify_exit_code(exit_code)),
                        format!("lightpanda fetch exited with non-zero status (exit_code={exit_code:?})"),
                        exit_code,
                    )
                },
                Err(err) => (
                    RunnerOutcomeStatus::Failed,
                    "failed",
                    Some("process_wait_failed"),
                    format!("lightpanda process wait failed: {err}"),
                    None,
                ),
            },
            Err(_) => {
                let kill_note = match child.kill().await {
                    Ok(()) => "timeout reached; process killed".to_string(),
                    Err(err) => format!("timeout reached; failed to kill process cleanly: {err}"),
                };
                let _ = child.wait().await;
                (
                    RunnerOutcomeStatus::TimedOut,
                    RUN_STATUS_TIMED_OUT,
                    Some("timeout"),
                    format!("lightpanda fetch timed out after {timeout_seconds}s ({kill_note})"),
                    None,
                )
            }
        };

        self.unregister_child(&task.task_id);

        let stdout = stdout_handle.await.unwrap_or_else(|err| format!("<stdout join error: {err}>"));
        let stderr = stderr_handle.await.unwrap_or_else(|err| format!("<stderr join error: {err}>"));
        let stdout_preview = preview_if_non_empty(stdout, 4000);
        let stderr_preview = preview_if_non_empty(stderr, 2000);

        let message = if let Some(runtime) = fingerprint_runtime.as_ref() {
            let ignored_note = if runtime.ignored_fields.is_empty() {
                String::new()
            } else {
                format!(
                    "; warning=fingerprint profile contains fields that lightpanda does not currently consume"
                )
            };
            format!(
                "{}; fingerprint_runtime: applied_fields={:?}, ignored_fields={:?}, applied_count={}, ignored_count={}, consumption_status={}{}",
                message,
                runtime.applied_fields,
                runtime.ignored_fields,
                runtime.applied_fields.len(),
                runtime.ignored_fields.len(),
                if runtime.applied_fields.iter().filter(|f| *f != "profile_id" && *f != "profile_version").count() == 0 && runtime.ignored_fields.is_empty() {
                    "metadata_only"
                } else if runtime.applied_fields.iter().filter(|f| *f != "profile_id" && *f != "profile_version").count() == 0 {
                    "ignored_only"
                } else if runtime.ignored_fields.is_empty() {
                    "fully_consumed"
                } else {
                    "partially_consumed"
                },
                ignored_note,
            )
        } else if task.fingerprint_profile.is_some() {
            format!(
                "{}; fingerprint_runtime: profile present but no supported fields were mapped",
                message,
            )
        } else {
            message
        };

        build_result(
            outcome,
            matches!(outcome, RunnerOutcomeStatus::Succeeded),
            status_text,
            error_kind,
            requested_action.as_str(),
            action,
            &task,
            Some(&url),
            Some(timeout_seconds),
            Some(&bin),
            exit_code,
            stdout_preview,
            stderr_preview,
            fingerprint_runtime.as_ref(),
            message,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        path::PathBuf,
        sync::{Mutex, OnceLock},
        time::{SystemTime, UNIX_EPOCH},
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn unique_temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("lightpanda-test-{name}-{nanos}.sh"))
    }

    fn write_script(name: &str, body: &str) -> PathBuf {
        let path = unique_temp_path(name);
        fs::write(&path, format!("#!/bin/sh
{body}
")).expect("write test script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&path).expect("metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).expect("chmod");
        }
        path
    }

    async fn execute_with_bin(bin: &str, payload: Value, timeout_seconds: Option<i64>) -> RunnerExecutionResult {
        let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        std::env::set_var("LIGHTPANDA_BIN", bin);
        let runner = LightpandaRunner::default();
        let result = runner
            .execute(RunnerTask {
                task_id: "task-test".to_string(),
                attempt: 1,
                kind: "open_page".to_string(),
                payload,
                timeout_seconds,
                fingerprint_profile: None,
                proxy: None,
            })
            .await;
        std::env::remove_var("LIGHTPANDA_BIN");
        result
    }

    #[test]
    fn build_lightpanda_fingerprint_runtime_maps_supported_fields_to_envs() {
        let task = RunnerTask {
            task_id: "task-test".to_string(),
            attempt: 1,
            kind: "open_page".to_string(),
            payload: json!({"url": "https://example.com"}),
            timeout_seconds: Some(5),
            fingerprint_profile: Some(crate::runner::RunnerFingerprintProfile {
                id: "fp-desktop".to_string(),
                version: 3,
                profile_json: json!({
                    "accept_language": "en-US,en;q=0.9",
                    "timezone": "Asia/Shanghai",
                    "locale": "en-US",
                    "viewport_width": 1440,
                    "viewport_height": 900,
                    "platform": "MacIntel",
                    "unsupported_blob": {"x": 1}
                }),
            }),
            proxy: None,
        };

        let runtime = build_lightpanda_fingerprint_runtime(&task).expect("runtime");
        let env_map: std::collections::HashMap<_, _> = runtime.envs.iter().cloned().collect();

        assert_eq!(env_map.get("LIGHTPANDA_FP_PROFILE_ID").map(String::as_str), Some("fp-desktop"));
        assert_eq!(env_map.get("LIGHTPANDA_FP_PROFILE_VERSION").map(String::as_str), Some("3"));
        assert_eq!(env_map.get("LIGHTPANDA_FP_ACCEPT_LANGUAGE").map(String::as_str), Some("en-US,en;q=0.9"));
        assert_eq!(env_map.get("LIGHTPANDA_FP_TIMEZONE").map(String::as_str), Some("Asia/Shanghai"));
        assert_eq!(env_map.get("LIGHTPANDA_FP_VIEWPORT_WIDTH").map(String::as_str), Some("1440"));
        assert!(runtime.applied_fields.iter().any(|f| f == "platform"));
        assert!(runtime.ignored_fields.iter().any(|f| f == "unsupported_blob"));
    }

    #[test]
    fn build_lightpanda_fingerprint_runtime_marks_partial_consumption_when_fields_are_ignored() {
        let task = RunnerTask {
            task_id: "task-test".to_string(),
            attempt: 1,
            kind: "open_page".to_string(),
            payload: json!({"url": "https://example.com"}),
            timeout_seconds: Some(5),
            fingerprint_profile: Some(crate::runner::RunnerFingerprintProfile {
                id: "fp-partial".to_string(),
                version: 1,
                profile_json: json!({
                    "timezone": "Asia/Shanghai",
                    "unsupported_blob": {"x": 1}
                }),
            }),
            proxy: None,
        };

        let runtime = build_lightpanda_fingerprint_runtime(&task).expect("runtime");
        let runtime_json = serde_json::json!({
            "supported_field_count": runtime.applied_fields.iter().filter(|f| *f != "profile_id" && *f != "profile_version").count(),
            "unsupported_field_count": runtime.ignored_fields.len(),
            "consumption_status": if runtime.applied_fields.iter().filter(|f| *f != "profile_id" && *f != "profile_version").count() == 0 && runtime.ignored_fields.is_empty() {
                "metadata_only"
            } else if runtime.applied_fields.iter().filter(|f| *f != "profile_id" && *f != "profile_version").count() == 0 {
                "ignored_only"
            } else if runtime.ignored_fields.is_empty() {
                "fully_consumed"
            } else {
                "partially_consumed"
            }
        });
        assert_eq!(runtime_json.get("consumption_status").and_then(|v| v.as_str()), Some("partially_consumed"));
    }

    #[tokio::test]
    async fn execute_reports_fingerprint_runtime_when_profile_is_present() {
        let _guard = env_lock().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let script = write_script(
            "fingerprint-env",
            "echo \"$LIGHTPANDA_FP_PROFILE_ID|$LIGHTPANDA_FP_ACCEPT_LANGUAGE|$LIGHTPANDA_FP_TIMEZONE|$LIGHTPANDA_FP_VIEWPORT_WIDTH\"
exit 0",
        );
        std::env::set_var("LIGHTPANDA_BIN", script.to_str().unwrap());
        let runner = LightpandaRunner::default();
        let result = runner
            .execute(RunnerTask {
                task_id: "task-test".to_string(),
                attempt: 1,
                kind: "open_page".to_string(),
                payload: json!({"url": "https://example.com"}),
                timeout_seconds: Some(5),
                fingerprint_profile: Some(crate::runner::RunnerFingerprintProfile {
                    id: "fp-desktop".to_string(),
                    version: 3,
                    profile_json: json!({
                        "accept_language": "en-US,en;q=0.9",
                        "timezone": "Asia/Shanghai",
                        "viewport_width": 1440
                    }),
                }),
                proxy: None,
            })
            .await;
        std::env::remove_var("LIGHTPANDA_BIN");
        let _ = fs::remove_file(script);

        assert!(matches!(result.status, RunnerOutcomeStatus::Succeeded));
        let json = result.result_json.expect("result json");
        assert_eq!(
            json.get("fingerprint_runtime")
                .and_then(|v| v.get("env_keys"))
                .and_then(|v| v.as_array())
                .map(|v| !v.is_empty()),
            Some(true)
        );
        assert_eq!(
            json.get("fingerprint_runtime").and_then(|v| v.get("consumption_status")).and_then(|v| v.as_str()),
            Some("fully_consumed")
        );
        assert_eq!(
            json.get("fingerprint_runtime").and_then(|v| v.get("supported_field_count")).and_then(|v| v.as_u64()),
            Some(3)
        );
        assert_eq!(
            json.get("stdout_preview").and_then(|v| v.as_str()),
            Some("fp-desktop|en-US,en;q=0.9|Asia/Shanghai|1440")
        );
    }

    #[test]
    fn truncate_output_adds_marker() {
        let output = truncate_output("abcdef", 3);
        assert_eq!(output, "abc...[truncated]");
    }

    #[test]
    fn extract_action_defaults_to_open_page_and_accepts_explicit_action() {
        assert_eq!(extract_action(&json!({"url": "https://example.com"})), "open_page");
        assert_eq!(extract_action(&json!({"url": "https://example.com", "action": "fetch"})), "fetch");
        assert_eq!(normalize_action("open_page"), Some("open_page"));
        assert_eq!(normalize_action("fetch"), Some("open_page"));
        assert_eq!(normalize_action("get_html"), Some("get_html"));
        assert_eq!(normalize_action("screenshot"), None);
    }

    #[test]
    fn extract_url_trims_and_rejects_empty() {
        assert_eq!(extract_url(&json!({"url": "  https://example.com  "})), Some("https://example.com".to_string()));
        assert_eq!(extract_url(&json!({"url": "   "})), None);
    }

    #[tokio::test]
    async fn execute_accepts_fetch_action_alias() {
        let script = write_script(
            "fetch-alias",
            "echo ok
exit 0",
        );
        let result = execute_with_bin(script.to_str().unwrap(), json!({"url": "https://example.com", "action": "fetch"}), Some(5)).await;
        let _ = fs::remove_file(script);

        assert!(matches!(result.status, RunnerOutcomeStatus::Succeeded));
        let json = result.result_json.expect("result json");
        assert_eq!(json.get("requested_action").and_then(|v| v.as_str()), Some("fetch"));
        assert_eq!(json.get("action").and_then(|v| v.as_str()), Some("open_page"));
        assert_eq!(json.get("capability_stage").and_then(|v| v.as_str()), Some("minimal_real_execution_v1"));
        assert_eq!(json.get("supported_actions").and_then(|v| v.as_array()).map(|v| v.len()), Some(3));
    }

    #[tokio::test]
    async fn execute_supports_get_html_v1() {
        let script = write_script(
            "get-html",
            "echo '<html><body>ok</body></html>'
exit 0",
        );
        let result = execute_with_bin(script.to_str().unwrap(), json!({"url": "https://example.com", "action": "get_html"}), Some(5)).await;
        let _ = fs::remove_file(script);

        assert!(matches!(result.status, RunnerOutcomeStatus::Succeeded));
        let json = result.result_json.expect("result json");
        assert_eq!(json.get("requested_action").and_then(|v| v.as_str()), Some("get_html"));
        assert_eq!(json.get("action").and_then(|v| v.as_str()), Some("get_html"));
        assert_eq!(json.get("content_kind").and_then(|v| v.as_str()), Some("text/html"));
        assert_eq!(json.get("html_preview").and_then(|v| v.as_str()), Some("<html><body>ok</body></html>"));
    }

    #[tokio::test]
    async fn execute_rejects_unsupported_action() {
        let result = execute_with_bin("/bin/sh", json!({"url": "https://example.com", "action": "screenshot"}), Some(5)).await;
        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        let json = result.result_json.expect("result json");
        assert_eq!(json.get("error_kind").and_then(|v| v.as_str()), Some("invalid_input"));
        assert_eq!(json.get("requested_action").and_then(|v| v.as_str()), Some("screenshot"));
        assert_eq!(json.get("action").and_then(|v| v.as_str()), Some("screenshot"));
        assert!(json.get("message").and_then(|v| v.as_str()).unwrap_or("").contains("supports only action=open_page"));
    }

    #[tokio::test]
    async fn execute_rejects_invalid_url() {
        let result = execute_with_bin("/bin/sh", json!({"url": "example.com"}), Some(5)).await;
        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        assert_eq!(
            result.result_json.as_ref().and_then(|v| v.get("error_kind")).and_then(|v| v.as_str()),
            Some("invalid_input")
        );
    }

    #[tokio::test]
    async fn execute_classifies_missing_binary() {
        let result = execute_with_bin("/definitely/missing/lightpanda", json!({"url": "https://example.com"}), Some(5)).await;
        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        assert_eq!(
            result.result_json.as_ref().and_then(|v| v.get("error_kind")).and_then(|v| v.as_str()),
            Some("binary_not_found")
        );
    }

    #[tokio::test]
    async fn execute_reports_non_zero_exit() {
        let script = write_script(
            "non-zero",
            "echo bad-output
echo bad-error >&2
exit 7",
        );
        let result = execute_with_bin(script.to_str().unwrap(), json!({"url": "https://example.com"}), Some(5)).await;
        let _ = fs::remove_file(script);

        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        let json = result.result_json.expect("result json");
        assert_eq!(json.get("error_kind").and_then(|v| v.as_str()), Some("runner_non_zero_exit"));
        assert_eq!(json.get("task_id").and_then(|v| v.as_str()), Some("task-test"));
        assert_eq!(json.get("attempt").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("open_page"));
        assert_eq!(json.get("payload").and_then(|v| v.get("url")).and_then(|v| v.as_str()), Some("https://example.com"));
        assert_eq!(json.get("exit_code").and_then(|v| v.as_i64()), Some(7));
        assert_eq!(json.get("stdout_preview").and_then(|v| v.as_str()), Some("bad-output"));
        assert_eq!(json.get("stderr_preview").and_then(|v| v.as_str()), Some("bad-error"));
    }

    #[tokio::test]
    async fn execute_classifies_command_not_found_exit_code_127() {
        let script = write_script(
            "exit-127",
            "exit 127",
        );
        let result = execute_with_bin(script.to_str().unwrap(), json!({"url": "https://example.com"}), Some(5)).await;
        let _ = fs::remove_file(script);

        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        let json = result.result_json.expect("result json");
        assert_eq!(json.get("error_kind").and_then(|v| v.as_str()), Some("runner_command_not_found"));
        assert_eq!(json.get("exit_code").and_then(|v| v.as_i64()), Some(127));
    }

    #[tokio::test]
    async fn execute_detects_browser_failure_signal_from_stderr() {
        let script = write_script(
            "browser-nav-fail",
            "echo navigation failed >&2
exit 9",
        );
        let result = execute_with_bin(script.to_str().unwrap(), json!({"url": "https://example.com"}), Some(5)).await;
        let _ = fs::remove_file(script);

        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        let json = result.result_json.expect("result json");
        assert_eq!(json.get("browser_failure_signal").and_then(|v| v.as_str()), Some("browser_navigation_failure_signal"));
        assert_eq!(json.get("failure_scope").and_then(|v| v.as_str()), Some("browser_execution"));
    }

    #[tokio::test]
    async fn execute_classifies_not_executable_exit_code_126() {
        let script = write_script(
            "exit-126",
            "exit 126",
        );
        let result = execute_with_bin(script.to_str().unwrap(), json!({"url": "https://example.com"}), Some(5)).await;
        let _ = fs::remove_file(script);

        assert!(matches!(result.status, RunnerOutcomeStatus::Failed));
        let json = result.result_json.expect("result json");
        assert_eq!(json.get("error_kind").and_then(|v| v.as_str()), Some("runner_invocation_not_executable"));
        assert_eq!(json.get("exit_code").and_then(|v| v.as_i64()), Some(126));
    }

    #[tokio::test]
    async fn execute_kills_process_on_timeout() {
        let script = write_script(
            "timeout",
            "sleep 2
echo should-not-print",
        );
        let result = execute_with_bin(script.to_str().unwrap(), json!({"url": "https://example.com"}), Some(1)).await;
        let _ = fs::remove_file(script);

        assert!(matches!(result.status, RunnerOutcomeStatus::TimedOut));
        let json = result.result_json.expect("result json");
        assert_eq!(json.get("error_kind").and_then(|v| v.as_str()), Some("timeout"));
        assert_eq!(json.get("status").and_then(|v| v.as_str()), Some("timed_out"));
        assert_eq!(json.get("task_id").and_then(|v| v.as_str()), Some("task-test"));
        assert_eq!(json.get("attempt").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("open_page"));
        assert_eq!(json.get("payload").and_then(|v| v.get("url")).and_then(|v| v.as_str()), Some("https://example.com"));
    }
}
