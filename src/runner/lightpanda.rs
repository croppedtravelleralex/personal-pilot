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

fn result_payload(
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    task: &RunnerTask,
    url: Option<&str>,
    timeout_seconds: Option<u64>,
    bin: Option<&str>,
    exit_code: Option<i32>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    message: &str,
) -> Value {
    json!({
        "runner": "lightpanda",
        "action": "open_page",
        "ok": ok,
        "status": status,
        "error_kind": error_kind,
        "task_id": task.task_id,
        "attempt": task.attempt,
        "kind": task.kind,
        "payload": task.payload,
        "url": url,
        "timeout_seconds": timeout_seconds,
        "bin": bin,
        "exit_code": exit_code,
        "stdout_preview": stdout_preview,
        "stderr_preview": stderr_preview,
        "message": message,
    })
}

fn build_result(
    outcome: RunnerOutcomeStatus,
    ok: bool,
    status: &str,
    error_kind: Option<&str>,
    task: &RunnerTask,
    url: Option<&str>,
    timeout_seconds: Option<u64>,
    bin: Option<&str>,
    exit_code: Option<i32>,
    stdout_preview: Option<String>,
    stderr_preview: Option<String>,
    message: impl Into<String>,
) -> RunnerExecutionResult {
    let message = message.into();
    let is_error = matches!(outcome, RunnerOutcomeStatus::Failed | RunnerOutcomeStatus::TimedOut);

    RunnerExecutionResult {
        status: outcome,
        result_json: Some(result_payload(
            ok,
            status,
            error_kind,
            task,
            url,
            timeout_seconds,
            bin,
            exit_code,
            stdout_preview,
            stderr_preview,
            &message,
        )),
        error_message: is_error.then_some(message),
    }
}

fn invalid_input(task: &RunnerTask, message: &str, url: Option<&str>) -> RunnerExecutionResult {
    build_result(
        RunnerOutcomeStatus::Failed,
        false,
        "failed",
        Some("invalid_input"),
        task,
        url,
        None,
        None,
        None,
        None,
        None,
        message,
    )
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
                .expect("lightpanda running_tasks poisoned");
            guard.insert(task_id.to_string(), pid);
        }
    }

    fn unregister_child(&self, task_id: &str) {
        let mut guard = self
            .running_tasks
            .lock()
            .expect("lightpanda running_tasks poisoned");
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
                .expect("lightpanda running_tasks poisoned");
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
        let url = match extract_url(&task.payload) {
            Some(url) => url,
            None => {
                return invalid_input(
                    &task,
                    "lightpanda runner requires a non-empty url in task payload",
                    None,
                )
            }
        };

        if !looks_like_url(&url) {
            return invalid_input(
                &task,
                "lightpanda runner currently only accepts http:// or https:// urls",
                Some(&url),
            );
        }

        let timeout_seconds = task.timeout_seconds.unwrap_or(10).clamp(1, 120) as u64;
        let bin = lightpanda_bin();

        let mut cmd = Command::new(&bin);
        cmd.arg("fetch")
            .arg("--log-format")
            .arg("pretty")
            .arg("--log-level")
            .arg("info")
            .arg(&url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(err) => {
                let message = format!("failed to spawn lightpanda binary: {err}");
                return build_result(
                    RunnerOutcomeStatus::Failed,
                    false,
                    "failed",
                    Some(classify_spawn_error(&err)),
                    &task,
                    Some(&url),
                    Some(timeout_seconds),
                    Some(&bin),
                    None,
                    None,
                    None,
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
                    &task,
                    "lightpanda fetch completed successfully".to_string(),
                    status.code(),
                ),
                Ok(status) => (
                    RunnerOutcomeStatus::Failed,
                    "failed",
                    Some("non_zero_exit"),
                    &task,
                    "lightpanda fetch exited with non-zero status".to_string(),
                    status.code(),
                ),
                Err(err) => (
                    RunnerOutcomeStatus::Failed,
                    "failed",
                    Some("process_wait_failed"),
                    &task,
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
                    &task,
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

        build_result(
            outcome,
            matches!(outcome, RunnerOutcomeStatus::Succeeded),
            status_text,
            error_kind,
            &task,
            Some(&url),
            Some(timeout_seconds),
            Some(&bin),
            exit_code,
            stdout_preview,
            stderr_preview,
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
        let _guard = env_lock().lock().expect("env lock poisoned");
        std::env::set_var("LIGHTPANDA_BIN", bin);
        let runner = LightpandaRunner::default();
        let result = runner
            .execute(RunnerTask {
                task_id: "task-test".to_string(),
                attempt: 1,
                kind: "open_page".to_string(),
                payload,
                timeout_seconds,
            })
            .await;
        std::env::remove_var("LIGHTPANDA_BIN");
        result
    }

    #[test]
    fn truncate_output_adds_marker() {
        let output = truncate_output("abcdef", 3);
        assert_eq!(output, "abc...[truncated]");
    }

    #[test]
    fn extract_url_trims_and_rejects_empty() {
        assert_eq!(extract_url(&json!({"url": "  https://example.com  "})), Some("https://example.com".to_string()));
        assert_eq!(extract_url(&json!({"url": "   "})), None);
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
        assert_eq!(json.get("error_kind").and_then(|v| v.as_str()), Some("non_zero_exit"));
        assert_eq!(json.get("task_id").and_then(|v| v.as_str()), Some("task-test"));
        assert_eq!(json.get("attempt").and_then(|v| v.as_i64()), Some(1));
        assert_eq!(json.get("kind").and_then(|v| v.as_str()), Some("open_page"));
        assert_eq!(json.get("payload").and_then(|v| v.get("url")).and_then(|v| v.as_str()), Some("https://example.com"));
        assert_eq!(json.get("exit_code").and_then(|v| v.as_i64()), Some(7));
        assert_eq!(json.get("stdout_preview").and_then(|v| v.as_str()), Some("bad-output"));
        assert_eq!(json.get("stderr_preview").and_then(|v| v.as_str()), Some("bad-error"));
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
