use std::{
    collections::HashSet,
    env,
    fs::{self, OpenOptions},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use persona_pilot::desktop::{
    apply_desktop_browser_environment_policy, apply_desktop_local_api_settings,
    apply_desktop_runtime_settings, change_desktop_proxy_ip, check_desktop_profile_proxies,
    compile_desktop_template_run, confirm_desktop_manual_gate, create_desktop_profile,
    delete_desktop_template, export_desktop_session_bundle, launch_desktop_template_run, load_desktop_logs,
    load_desktop_profile_detail, load_desktop_profile_page, load_desktop_proxy_health,
    load_desktop_proxy_page, load_desktop_proxy_usage, load_desktop_status, load_desktop_tasks,
    load_desktop_template_metadata_page, open_desktop_profiles,
    read_desktop_browser_environment_policy, read_desktop_import_export_skeleton,
    read_desktop_local_api_snapshot, read_desktop_local_asset_workspace, read_desktop_run_detail,
    read_desktop_settings, reject_desktop_manual_gate, resolve_desktop_local_asset_entry_path,
    restore_desktop_browser_environment_policy_defaults, restore_desktop_local_api_defaults,
    restore_desktop_runtime_settings_defaults, retry_desktop_task, run_desktop_proxy_batch_check,
    save_desktop_template, start_desktop_profiles, stop_desktop_profiles, sync_desktop_profiles,
    update_desktop_profile, update_desktop_template, DesktopAppendBehaviorRecordingStepRequest,
    DesktopBrowserEnvironmentPolicyDraft, DesktopBrowserEnvironmentPolicyMutationResult,
    DesktopBrowserEnvironmentPolicySnapshot, DesktopCompileTemplateRunRequest,
    DesktopCompileTemplateRunResult, DesktopCreateProfileInput, DesktopImportExportSkeleton,
    DesktopLaunchTemplateRunRequest, DesktopLaunchTemplateRunResult, DesktopLocalApiMutationResult,
    DesktopLocalApiSettingsDraft, DesktopLocalApiSnapshot, DesktopLocalAssetWorkspaceSnapshot,
    DesktopLogPage, DesktopLogQuery, DesktopManualGateActionRequest,
    DesktopProfileBatchActionRequest, DesktopProfileBatchActionResult, DesktopProfileDetail,
    DesktopProfileMutationResult, DesktopProfilePage, DesktopProfilePageQuery,
    DesktopProxyBatchCheckRequest, DesktopProxyBatchCheckResponse, DesktopProxyChangeIpRequest,
    DesktopProxyChangeIpResult, DesktopProxyHealth, DesktopProxyPage, DesktopProxyPageQuery,
    DesktopProxyUsageItem, DesktopReadRunDetailQuery, DesktopRecorderSnapshot,
    DesktopRecorderSnapshotQuery, DesktopRunDetail, DesktopRuntimeSettingsDraft,
    DesktopSessionBundleExport, DesktopSessionBundleExportRequest, DesktopSettingsMutationResult, DesktopSettingsSnapshot, DesktopStartBehaviorRecordingRequest,
    DesktopStatusSnapshot, DesktopStopBehaviorRecordingRequest, DesktopSyncLayoutState,
    DesktopSyncLayoutUpdate, DesktopSyncWindowBounds, DesktopSyncWindowState,
    DesktopSynchronizerActionResult, DesktopSynchronizerSnapshot, DesktopTaskPage,
    DesktopTaskQuery, DesktopTaskWriteResult, DesktopTemplateDeleteInput,
    DesktopTemplateMetadataPage, DesktopTemplateMetadataPageQuery, DesktopTemplateMutationResult,
    DesktopTemplateUpsertInput, DesktopUpdateProfileInput,
};
use persona_pilot::runner::{RunnerOutcomeStatus, RunnerTask};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::State;

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM, RECT},
    Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    },
    UI::WindowsAndMessaging::{
        BringWindowToTop, EnumWindows, GetForegroundWindow, GetSystemMetrics, GetWindowRect,
        GetWindowTextLengthW, GetWindowTextW, IsIconic, IsWindow, IsWindowVisible,
        SetForegroundWindow, SetWindowPos, ShowWindow, SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE,
        SWP_NOZORDER, SW_RESTORE,
    },
};

use crate::state::{DesktopState, ManagedRuntimeProcess};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const LOCAL_RUNTIME_HEALTH_URL: &str = "http://127.0.0.1:3000/health";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopValidationSignal {
    pub id: String,
    pub category: String,
    pub layer: String,
    pub status: String,
    pub label: String,
    pub summary: String,
    pub detail: Option<String>,
    pub duration_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopValidationReport {
    pub report_id: String,
    pub generated_at: String,
    pub profile_id: Option<String>,
    pub collector_version: String,
    pub categories: Vec<String>,
    pub signals: Vec<DesktopValidationSignal>,
    pub report_path: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopValidationReportSummary {
    pub report_id: String,
    pub generated_at: String,
    pub profile_id: Option<String>,
    pub collector_version: String,
    pub categories: Vec<String>,
    pub signal_count: usize,
    pub failed_count: usize,
    pub warning_count: usize,
    pub report_path: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopValidationProfileExport {
    pub export_id: String,
    pub profile_id: Option<String>,
    pub exported_at: String,
    pub report_count: usize,
    pub export_path: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopValidationProfileExportPayload {
    export_id: String,
    profile_id: Option<String>,
    exported_at: String,
    reports: Vec<DesktopValidationReport>,
}

fn validation_report_dir(state: &DesktopState) -> Result<PathBuf, String> {
    let snapshot = read_desktop_settings(Some(&state.database_url));
    let report_dir = PathBuf::from(snapshot.reports_dir).join("validation");
    fs::create_dir_all(&report_dir).map_err(|error| {
        format!(
            "Failed to prepare validation report directory {}: {error}",
            report_dir.display()
        )
    })?;
    Ok(report_dir)
}

fn validation_report_summary(report: &DesktopValidationReport) -> DesktopValidationReportSummary {
    let failed_count = report
        .signals
        .iter()
        .filter(|signal| signal.status == "failed")
        .count();
    let warning_count = report
        .signals
        .iter()
        .filter(|signal| signal.status == "warning")
        .count();
    DesktopValidationReportSummary {
        report_id: report.report_id.clone(),
        generated_at: report.generated_at.clone(),
        profile_id: report.profile_id.clone(),
        collector_version: report.collector_version.clone(),
        categories: report.categories.clone(),
        signal_count: report.signals.len(),
        failed_count,
        warning_count,
        report_path: report.report_path.clone(),
        summary: report.summary.clone(),
    }
}

fn read_validation_reports_from_dir(report_dir: &Path) -> Result<Vec<DesktopValidationReport>, String> {
    let mut reports = Vec::new();
    let entries = fs::read_dir(report_dir).map_err(|error| {
        format!(
            "Failed to read validation report directory {}: {error}",
            report_dir.display()
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to read validation report entry: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("profile-evidence-export-"))
        {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|error| {
            format!("Failed to read validation report {}: {error}", path.display())
        })?;
        let mut report: DesktopValidationReport = serde_json::from_str(&raw).map_err(|error| {
            format!("Failed to parse validation report {}: {error}", path.display())
        })?;
        if report.report_path.trim().is_empty() {
            report.report_path = path.to_string_lossy().to_string();
        }
        reports.push(report);
    }

    reports.sort_by(|left, right| right.generated_at.cmp(&left.generated_at));
    Ok(reports)
}

fn validation_report_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("validation-report-{millis}")
}

fn validation_signal(
    id: &str,
    category: &str,
    status: &str,
    label: &str,
    summary: String,
    detail: Option<String>,
    duration_ms: Option<u128>,
) -> DesktopValidationSignal {
    DesktopValidationSignal {
        id: id.to_string(),
        category: category.to_string(),
        layer: "observed".to_string(),
        status: status.to_string(),
        label: label.to_string(),
        summary,
        detail,
        duration_ms,
    }
}

fn collect_dns_signal() -> DesktopValidationSignal {
    let started = Instant::now();
    match ("example.com", 443).to_socket_addrs() {
        Ok(addrs) => {
            let addresses: Vec<String> = addrs.map(|addr| addr.ip().to_string()).collect();
            let unique_count = addresses.iter().collect::<HashSet<_>>().len();
            validation_signal(
                "dns-example-com",
                "dns",
                if unique_count > 0 { "succeeded" } else { "failed" },
                "example.com DNS resolution",
                if unique_count > 0 {
                    format!("Resolved example.com to {unique_count} unique address(es).")
                } else {
                    "Resolver returned no usable addresses for example.com.".to_string()
                },
                Some(addresses.join(", ")),
                Some(started.elapsed().as_millis()),
            )
        }
        Err(error) => validation_signal(
            "dns-example-com",
            "dns",
            "failed",
            "example.com DNS resolution",
            format!("DNS resolution failed: {error}"),
            None,
            Some(started.elapsed().as_millis()),
        ),
    }
}

fn collect_webrtc_signal() -> DesktopValidationSignal {
    let started = Instant::now();
    validation_signal(
        "webrtc-native-contract",
        "webrtc",
        "warning",
        "WebRTC native collector contract",
        "Native WebRTC leak probing requires a browser runtime probe; this report records the missing observed collector boundary.".to_string(),
        Some("next: run browser-scoped local/public IP and media device exposure probe".to_string()),
        Some(started.elapsed().as_millis()),
    )
}

fn collect_leak_signal() -> DesktopValidationSignal {
    let started = Instant::now();
    validation_signal(
        "leak-report-persistence",
        "leak",
        "warning",
        "Cross-profile leak collector contract",
        "Report persistence is available, but storage/cookie cross-profile leak probing still needs a browser-scoped collector.".to_string(),
        Some("next: compare profile storage scopes, cookies, localStorage, sessionStorage, and identity material.".to_string()),
        Some(started.elapsed().as_millis()),
    )
}

fn normalize_browser_validation_signal(
    mut signal: DesktopValidationSignal,
) -> DesktopValidationSignal {
    let valid_category = matches!(
        signal.category.as_str(),
        "detector" | "leak" | "dns" | "webrtc" | "canvas" | "audio" | "worker" | "transport"
    );
    if !valid_category {
        signal.category = "detector".to_string();
        signal.status = "warning".to_string();
        signal.summary = format!(
            "Browser signal category was not recognized; original signal id={} was normalized.",
            signal.id
        );
    }

    if !matches!(
        signal.status.as_str(),
        "succeeded" | "warning" | "failed"
    ) {
        signal.status = "warning".to_string();
    }

    signal.layer = "observed".to_string();
    signal.id = format!("browser-{}", signal.id.trim().replace(char::is_whitespace, "-"));
    signal.label = format!("{}", signal.label.trim());
    if signal.label.is_empty() {
        signal.label = "Browser validation signal".to_string();
    }
    if signal.summary.trim().is_empty() {
        signal.summary = "Browser validation signal did not include a summary.".to_string();
    }
    signal
}

fn validation_signal_from_value(value: Value) -> Option<DesktopValidationSignal> {
    serde_json::from_value::<DesktopValidationSignal>(value).ok()
}

async fn collect_profile_browser_runtime_signals(
    state: &DesktopState,
) -> Vec<DesktopValidationSignal> {
    let started = Instant::now();
    let task = RunnerTask {
        task_id: format!("validation-profile-runtime-{}", validation_report_id()),
        attempt: 1,
        kind: "validation_probe".to_string(),
        payload: json!({
            "action": "validation_probe",
            "url": "https://example.com/",
            "collector": "validation-profile-browser-runtime-v1"
        }),
        timeout_seconds: Some(15),
        execution_intent: None,
        fingerprint_profile: None,
        behavior_profile: None,
        behavior_plan: None,
        form_action_plan: None,
        proxy: None,
        session_cookies: None,
        session_local_storage: None,
        session_session_storage: None,
    };

    let result = state.runner.execute(task).await;
    let mut signals: Vec<DesktopValidationSignal> = result
        .result_json
        .as_ref()
        .and_then(|value| value.get("validation_signals"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .cloned()
                .filter_map(validation_signal_from_value)
                .map(normalize_browser_validation_signal)
                .collect()
        })
        .unwrap_or_default();

    if signals.is_empty() || !matches!(result.status, RunnerOutcomeStatus::Succeeded) {
        let status = if matches!(result.status, RunnerOutcomeStatus::Succeeded) {
            "warning"
        } else {
            "failed"
        };
        let detail = result
            .error_message
            .or_else(|| result.result_json.map(|value| value.to_string()))
            .unwrap_or_else(|| "runner returned no validation signal detail".to_string());
        signals.push(validation_signal(
            "profile-browser-runtime-runner",
            "detector",
            status,
            "Profile browser runtime collector",
            "Profile browser runtime validation probe did not return complete evidence signals."
                .to_string(),
            Some(format!(
                "scope=profile-browser-runtime; runner={}; detail={detail}",
                state.runner.name()
            )),
            Some(started.elapsed().as_millis()),
        ));
    }

    signals
}

async fn collect_transport_signal() -> DesktopValidationSignal {
    let started = Instant::now();
    let client = match Client::builder().timeout(Duration::from_secs(8)).build() {
        Ok(client) => client,
        Err(error) => {
            return validation_signal(
                "transport-example-com",
                "transport",
                "failed",
                "HTTPS HEAD probe",
                format!("Transport client could not be created: {error}"),
                None,
                Some(started.elapsed().as_millis()),
            );
        }
    };

    match client.head("https://example.com/").send().await {
        Ok(response) => {
            let status = response.status();
            validation_signal(
                "transport-example-com",
                "transport",
                if status.is_success() || status.is_redirection() {
                    "succeeded"
                } else {
                    "warning"
                },
                "HTTPS HEAD probe",
                format!("HTTPS probe returned HTTP {status}."),
                response.url().host_str().map(|host| format!("host={host}")),
                Some(started.elapsed().as_millis()),
            )
        }
        Err(error) => validation_signal(
            "transport-example-com",
            "transport",
            "failed",
            "HTTPS HEAD probe",
            format!("HTTPS probe failed: {error}"),
            None,
            Some(started.elapsed().as_millis()),
        ),
    }
}

async fn build_validation_report(
    state: &DesktopState,
    browser_signals: Vec<DesktopValidationSignal>,
) -> Result<DesktopValidationReport, String> {
    let snapshot = read_desktop_settings(Some(&state.database_url));
    let report_id = validation_report_id();
    let generated_at = now_ts_string();
    let mut signals = vec![collect_dns_signal()];
    signals.push(collect_transport_signal().await);
    signals.push(collect_webrtc_signal());
    signals.push(collect_leak_signal());
    signals.extend(collect_profile_browser_runtime_signals(state).await);
    signals.extend(
        browser_signals
            .into_iter()
            .map(normalize_browser_validation_signal),
    );

    let succeeded = signals
        .iter()
        .filter(|signal| signal.status == "succeeded")
        .count();
    let failed = signals
        .iter()
        .filter(|signal| signal.status == "failed")
        .count();
    let signal_count = signals.len();
    let report_dir = PathBuf::from(snapshot.reports_dir).join("validation");
    fs::create_dir_all(&report_dir).map_err(|error| {
        format!(
            "Failed to prepare validation report directory {}: {error}",
            report_dir.display()
        )
    })?;
    let report_path = report_dir.join(format!("{report_id}.json"));

    let report = DesktopValidationReport {
        report_id,
        generated_at,
        profile_id: None,
        collector_version: "validation-observed-v1".to_string(),
        categories: vec![
            "dns".to_string(),
            "transport".to_string(),
            "webrtc".to_string(),
            "leak".to_string(),
            "canvas".to_string(),
            "audio".to_string(),
        ],
        signals,
        report_path: report_path.to_string_lossy().to_string(),
        summary: format!("{signal_count} observed signal(s), {succeeded} succeeded, {failed} failed."),
    };

    let payload = serde_json::to_string_pretty(&report)
        .map_err(|error| format!("Failed to serialize validation report: {error}"))?;
    fs::write(&report_path, payload).map_err(|error| {
        format!(
            "Failed to write validation report {}: {error}",
            report_path.display()
        )
    })?;

    Ok(report)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSynchronizerBroadcastRequest {
    pub plan_id: String,
    pub target_window_ids: Option<Vec<String>>,
    pub controller_window_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopRuntimeStatus {
    pub status: String,
    pub running: bool,
    pub managed: bool,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub health_url: String,
    pub api_reachable: bool,
    pub binary_path: Option<String>,
    pub log_dir: Option<String>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub last_exit_code: Option<i32>,
}

fn normalize_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}

fn now_ts_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalApiCancelTaskResponse {
    id: String,
    status: String,
    message: String,
}

fn local_api_auth_key() -> Option<String> {
    ["x-api-key", "X_API_KEY", "PERSONA_PILOT_API_KEY"]
        .iter()
        .filter_map(|name| env::var(name).ok())
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

fn local_api_requires_auth(snapshot: &DesktopLocalApiSnapshot) -> bool {
    snapshot.require_local_token || snapshot.auth_mode == "loopback_token"
}

async fn cancel_task_via_local_api(
    snapshot: &DesktopLocalApiSnapshot,
    task_id: &str,
) -> Result<LocalApiCancelTaskResponse, String> {
    let api_key = local_api_auth_key();
    if local_api_requires_auth(snapshot) && api_key.is_none() {
        return Err(
            "local API requires x-api-key, but no usable x-api-key environment variable was found"
                .to_string(),
        );
    }

    let client = Client::new();
    let url = format!(
        "{}/tasks/{}/cancel",
        snapshot.base_url.trim_end_matches('/'),
        task_id
    );
    let mut request = client.post(url);
    if let Some(api_key) = api_key {
        request = request.header("x-api-key", api_key);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("failed to call local API cancel endpoint: {error}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("failed to read local API cancel response: {error}"))?;

    if !status.is_success() {
        return Err(format!(
            "local API cancel request failed with HTTP {}: {}",
            status.as_u16(),
            body
        ));
    }

    serde_json::from_str::<LocalApiCancelTaskResponse>(&body)
        .map_err(|error| format!("failed to parse local API cancel response: {error}; body={body}"))
}

fn is_api_reachable() -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

fn resolve_directory_target(
    snapshot: &DesktopSettingsSnapshot,
    target: &str,
) -> Result<PathBuf, String> {
    let path = match target {
        "projectRoot" => PathBuf::from(&snapshot.project_root),
        "dataDir" => PathBuf::from(&snapshot.data_dir),
        "reportsDir" => PathBuf::from(&snapshot.reports_dir),
        "logsDir" => PathBuf::from(&snapshot.logs_dir),
        "packagedDataDir" => PathBuf::from(&snapshot.packaged_data_dir),
        "packagedReportsDir" => PathBuf::from(&snapshot.packaged_reports_dir),
        "packagedLogsDir" => PathBuf::from(&snapshot.packaged_logs_dir),
        _ => return Err(format!("Unsupported directory target: {target}")),
    };

    Ok(path)
}

fn resolve_runtime_binary(project_root: &Path) -> Result<PathBuf, String> {
    let release_binary = project_root
        .join("target")
        .join("release")
        .join("PersonaPilot.exe");
    if release_binary.exists() {
        return Ok(release_binary);
    }

    let debug_binary = project_root
        .join("target")
        .join("debug")
        .join("PersonaPilot.exe");
    if debug_binary.exists() {
        return Ok(debug_binary);
    }

    Err(format!(
        "Local runtime binary not found. Build PersonaPilot first at {} or {}.",
        release_binary.display(),
        debug_binary.display()
    ))
}

fn runtime_log_paths(snapshot: &DesktopSettingsSnapshot) -> (PathBuf, PathBuf, PathBuf) {
    let log_dir = PathBuf::from(&snapshot.logs_dir).join("runtime");
    let stdout_path = log_dir.join("persona-runtime.stdout.log");
    let stderr_path = log_dir.join("persona-runtime.stderr.log");
    (log_dir, stdout_path, stderr_path)
}

fn open_path_in_explorer(path: &Path, select_file: bool) -> Result<(), String> {
    let mut command = Command::new("explorer.exe");

    if select_file && path.exists() {
        command.arg(format!("/select,{}", path.display()));
    } else {
        let open_target = if select_file {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        command.arg(open_target);
    }

    command
        .spawn()
        .map_err(|error| format!("Failed to open {}: {error}", path.display()))?;

    Ok(())
}

fn status_from_managed_process(
    process: &ManagedRuntimeProcess,
    api_reachable: bool,
    last_exit_code: Option<i32>,
) -> DesktopRuntimeStatus {
    DesktopRuntimeStatus {
        status: "managed_running".to_string(),
        running: true,
        managed: true,
        pid: Some(process.pid),
        started_at: Some(process.started_at.clone()),
        health_url: LOCAL_RUNTIME_HEALTH_URL.to_string(),
        api_reachable,
        binary_path: Some(process.binary_path.clone()),
        log_dir: Some(process.log_dir.clone()),
        stdout_path: Some(process.stdout_path.clone()),
        stderr_path: Some(process.stderr_path.clone()),
        last_exit_code,
    }
}

fn build_runtime_status(state: &DesktopState) -> Result<DesktopRuntimeStatus, String> {
    let api_reachable = is_api_reachable();
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "Failed to lock local runtime state".to_string())?;
    let last_exit_code = runtime.last_exit_code;

    if let Some(process) = runtime.managed_process.as_mut() {
        match process.child.try_wait() {
            Ok(None) => {
                return Ok(status_from_managed_process(
                    process,
                    api_reachable,
                    last_exit_code,
                ));
            }
            Ok(Some(status)) => {
                runtime.last_exit_code = status.code();
                runtime.managed_process = None;
            }
            Err(error) => {
                return Err(format!("Failed to inspect local runtime process: {error}"));
            }
        }
    }

    let status = if api_reachable {
        "external_running"
    } else if runtime.last_exit_code.is_some() {
        "managed_stopped"
    } else {
        "stopped"
    };

    Ok(DesktopRuntimeStatus {
        status: status.to_string(),
        running: api_reachable,
        managed: false,
        pid: None,
        started_at: None,
        health_url: LOCAL_RUNTIME_HEALTH_URL.to_string(),
        api_reachable,
        binary_path: None,
        log_dir: None,
        stdout_path: None,
        stderr_path: None,
        last_exit_code: runtime.last_exit_code,
    })
}

fn placeholder_recorder_snapshot(query: DesktopRecorderSnapshotQuery) -> DesktopRecorderSnapshot {
    let now = now_ts_string();
    DesktopRecorderSnapshot {
        session_id: query
            .session_id
            .unwrap_or_else(|| "recorder-idle".to_string()),
        status: "idle".to_string(),
        profile_id: query.profile_id,
        platform_id: query.platform_id,
        template_id: query.template_id,
        current_tab_id: Some("tab-home".to_string()),
        current_url: Some("about:blank".to_string()),
        is_dirty: false,
        can_undo: false,
        can_redo: false,
        step_count: 0,
        sensitive_step_count: 0,
        variable_count: 0,
        started_at: None,
        stopped_at: None,
        updated_at: now.clone(),
        tabs: vec![persona_pilot::desktop::DesktopRecorderTabSnapshot {
            tab_id: "tab-home".to_string(),
            title: Some("Recorder Idle".to_string()),
            url: Some("about:blank".to_string()),
            active: true,
        }],
        steps: Vec::new(),
    }
}

fn sync_action_result(
    action: &str,
    snapshot: DesktopSynchronizerSnapshot,
    message: &str,
) -> DesktopSynchronizerActionResult {
    DesktopSynchronizerActionResult {
        action: action.to_string(),
        updated_at: snapshot.updated_at.clone(),
        snapshot,
        message: message.to_string(),
    }
}

#[allow(dead_code)]
fn desktop_command_not_ready(contract_name: &str) -> String {
    format!("desktop_command_not_ready: {contract_name} native contract is not implemented yet.")
}

fn create_recorder_step_tab_snapshot(
    tab_id: &str,
    active: bool,
) -> persona_pilot::desktop::DesktopRecorderTabSnapshot {
    persona_pilot::desktop::DesktopRecorderTabSnapshot {
        tab_id: tab_id.to_string(),
        title: Some(tab_id.to_string()),
        url: None,
        active,
    }
}

fn create_recording_snapshot(
    request: &DesktopAppendBehaviorRecordingStepRequest,
    session_id: String,
    now: &str,
) -> DesktopRecorderSnapshot {
    let tab_id = request
        .tab_id
        .clone()
        .unwrap_or_else(|| "tab-active".to_string());

    DesktopRecorderSnapshot {
        session_id,
        status: "recording".to_string(),
        profile_id: request.profile_id.clone(),
        platform_id: request.platform_id.clone(),
        template_id: request.template_id.clone(),
        current_tab_id: Some(tab_id.clone()),
        current_url: request.url.clone(),
        is_dirty: false,
        can_undo: false,
        can_redo: false,
        step_count: 0,
        sensitive_step_count: 0,
        variable_count: 0,
        started_at: Some(now.to_string()),
        stopped_at: None,
        updated_at: now.to_string(),
        tabs: vec![create_recorder_step_tab_snapshot(&tab_id, true)],
        steps: Vec::new(),
    }
}

fn count_recorder_variables(steps: &[persona_pilot::desktop::DesktopRecorderStep]) -> i64 {
    let mut keys = HashSet::new();

    for step in steps {
        if let Some(key) = step.input_key.as_ref() {
            if !key.trim().is_empty() {
                keys.insert(key.clone());
            }
        }
    }

    keys.len() as i64
}

fn upsert_recorder_tab(
    tabs: &mut Vec<persona_pilot::desktop::DesktopRecorderTabSnapshot>,
    tab_id: &str,
    url: Option<String>,
) {
    let mut found = false;

    for tab in tabs.iter_mut() {
        let is_target = tab.tab_id == tab_id;
        tab.active = is_target;
        if is_target {
            tab.title = Some(tab_id.to_string());
            if url.is_some() {
                tab.url = url.clone();
            }
            found = true;
        }
    }

    if !found {
        tabs.push(persona_pilot::desktop::DesktopRecorderTabSnapshot {
            tab_id: tab_id.to_string(),
            title: Some(tab_id.to_string()),
            url,
            active: true,
        });
    }
}

#[cfg(target_os = "windows")]
fn read_window_title(hwnd: HWND) -> Option<String> {
    unsafe {
        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return None;
        }

        let mut buffer = vec![0u16; length as usize + 1];
        let written = GetWindowTextW(hwnd, &mut buffer);
        if written <= 0 {
            return None;
        }

        let title = String::from_utf16_lossy(&buffer[..written as usize]);
        let trimmed = title.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }
}

#[cfg(target_os = "windows")]
fn window_bounds(hwnd: HWND) -> Option<DesktopSyncWindowBounds> {
    unsafe {
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return None;
        }

        Some(DesktopSyncWindowBounds {
            x: rect.left as i64,
            y: rect.top as i64,
            width: width as i64,
            height: height as i64,
        })
    }
}

#[cfg(target_os = "windows")]
struct SyncEnumContext {
    windows: Vec<DesktopSyncWindowState>,
    focused_handle: isize,
    main_window_id: Option<String>,
    now: String,
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_sync_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let context = &mut *(lparam.0 as *mut SyncEnumContext);

    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }

    let title = match read_window_title(hwnd) {
        Some(title) => title,
        None => return BOOL(1),
    };

    let bounds = match window_bounds(hwnd) {
        Some(bounds) => Some(bounds),
        None => return BOOL(1),
    };

    let window_id = (hwnd.0 as isize).to_string();
    let is_focused = context.focused_handle == hwnd.0 as isize;
    let is_minimized = IsIconic(hwnd).as_bool();
    let status = if is_focused {
        "focused"
    } else if is_minimized {
        "minimized"
    } else {
        "ready"
    };

    context.windows.push(DesktopSyncWindowState {
        window_id: window_id.clone(),
        native_handle: Some(window_id.clone()),
        title: Some(title),
        status: status.to_string(),
        order_index: context.windows.len() as i64,
        is_main_window: context.main_window_id.as_deref() == Some(window_id.as_str()),
        is_focused,
        is_minimized,
        is_visible: true,
        profile_id: None,
        profile_label: None,
        store_id: None,
        platform_id: None,
        last_seen_at: Some(context.now.clone()),
        last_action_at: None,
        bounds,
    });

    BOOL(1)
}

#[cfg(target_os = "windows")]
fn capture_live_synchronizer_snapshot(
    previous: &DesktopSynchronizerSnapshot,
) -> Result<DesktopSynchronizerSnapshot, String> {
    let now = now_ts_string();
    let focused_handle = unsafe { GetForegroundWindow().0 as isize };
    let mut context = SyncEnumContext {
        windows: Vec::new(),
        focused_handle,
        main_window_id: previous.layout.main_window_id.clone(),
        now: now.clone(),
    };

    unsafe {
        let _ = EnumWindows(
            Some(enum_sync_windows),
            LPARAM((&mut context as *mut SyncEnumContext) as isize),
        );
    }

    if context.windows.is_empty() {
        return Err(
            "No visible desktop windows were detected for synchronizer snapshot.".to_string(),
        );
    }

    let focused_window_id = context
        .windows
        .iter()
        .find(|window| window.is_focused)
        .map(|window| window.window_id.clone());
    let main_window_id = previous
        .layout
        .main_window_id
        .as_ref()
        .filter(|main_id| {
            context
                .windows
                .iter()
                .any(|window| &window.window_id == *main_id)
        })
        .cloned();

    for window in &mut context.windows {
        window.is_main_window = main_window_id
            .as_ref()
            .map(|main_id| &window.window_id == main_id)
            .unwrap_or(false);
    }

    Ok(DesktopSynchronizerSnapshot {
        windows: context.windows,
        layout: DesktopSyncLayoutState {
            main_window_id,
            updated_at: now.clone(),
            ..previous.layout.clone()
        },
        focused_window_id,
        updated_at: now,
    })
}

#[cfg(not(target_os = "windows"))]
fn capture_live_synchronizer_snapshot(
    _previous: &DesktopSynchronizerSnapshot,
) -> Result<DesktopSynchronizerSnapshot, String> {
    Err(desktop_command_not_ready("readSynchronizerSnapshot"))
}

#[cfg(target_os = "windows")]
fn focus_window(window_id: &str) -> Result<(), String> {
    let handle = window_id
        .parse::<isize>()
        .map_err(|_| format!("invalid native sync window id: {window_id}"))?;
    let hwnd = HWND(handle as *mut _);

    unsafe {
        if !IsWindow(hwnd).as_bool() {
            return Err(format!("sync window not found: {window_id}"));
        }

        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        let _ = BringWindowToTop(hwnd);
        let focused = SetForegroundWindow(hwnd).as_bool() || GetForegroundWindow() == hwnd;

        if !focused {
            return Err(format!("Failed to focus sync window {window_id}."));
        }
    }

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn focus_window(_window_id: &str) -> Result<(), String> {
    Err(desktop_command_not_ready("focusSyncWindow"))
}

fn normalize_sync_layout_mode(mode: &str) -> Result<String, String> {
    let normalized = mode.trim().to_lowercase();
    if normalized.is_empty() {
        return Err("layout mode cannot be empty".to_string());
    }

    match normalized.as_str() {
    "grid" | "tiled" | "overlap" | "cascade" | "focus_stack" | "uniform_size" => Ok(normalized),
    _ => Err(format!(
      "unsupported sync layout mode: {mode}. expected one of: grid, tiled, overlap, cascade, focus_stack, uniform_size"
    )),
  }
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct SyncWindowPlacement {
    window_id: String,
    hwnd: HWND,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[cfg(target_os = "windows")]
fn sync_window_hwnd(window: &DesktopSyncWindowState) -> Result<HWND, String> {
    let handle_id = window
        .native_handle
        .as_deref()
        .unwrap_or(window.window_id.as_str());
    let handle = handle_id
        .parse::<isize>()
        .map_err(|_| format!("invalid native sync window id: {}", window.window_id))?;
    let hwnd = HWND(handle as *mut _);

    unsafe {
        if !IsWindow(hwnd).as_bool() {
            return Err(format!("sync window not found: {}", window.window_id));
        }
        if !IsWindowVisible(hwnd).as_bool() {
            return Err(format!("sync window is not visible: {}", window.window_id));
        }
    }

    Ok(hwnd)
}

#[cfg(target_os = "windows")]
fn live_layout_windows(snapshot: &DesktopSynchronizerSnapshot) -> Vec<&DesktopSyncWindowState> {
    snapshot
        .windows
        .iter()
        .filter(|window| window.is_visible && !window.is_minimized)
        .collect()
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy)]
struct WorkArea {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[cfg(target_os = "windows")]
fn primary_work_area() -> Result<WorkArea, String> {
    unsafe {
        let monitor = MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTONEAREST);
        if !monitor.is_invalid() {
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let width = info.rcWork.right - info.rcWork.left;
                let height = info.rcWork.bottom - info.rcWork.top;
                if width > 0 && height > 0 {
                    return Ok(WorkArea {
                        x: info.rcWork.left,
                        y: info.rcWork.top,
                        width,
                        height,
                    });
                }
            }
        }

        let width = GetSystemMetrics(SM_CXSCREEN);
        let height = GetSystemMetrics(SM_CYSCREEN);
        if width <= 0 || height <= 0 {
            return Err("Unable to read primary screen size for native sync layout.".to_string());
        }
        Ok(WorkArea {
            x: 0,
            y: 0,
            width,
            height,
        })
    }
}

#[cfg(target_os = "windows")]
fn ceil_div(left: i64, right: i64) -> i64 {
    (left + right - 1) / right
}

#[cfg(target_os = "windows")]
fn clamp_layout_size(value: i64) -> i32 {
    value.max(1).min(i32::MAX as i64) as i32
}

#[cfg(target_os = "windows")]
fn offset_layout_position(origin: i32, offset: i64) -> i32 {
    (origin as i64 + offset)
        .max(i32::MIN as i64)
        .min(i32::MAX as i64) as i32
}

#[cfg(target_os = "windows")]
fn tiled_window_placements(
    snapshot: &DesktopSynchronizerSnapshot,
    windows: &[&DesktopSyncWindowState],
) -> Result<Vec<SyncWindowPlacement>, String> {
    let work_area = primary_work_area()?;
    let screen_width = work_area.width as i64;
    let screen_height = work_area.height as i64;
    let count = windows.len() as i64;
    let gap = snapshot.layout.gap_px.max(0);
    let columns = snapshot
        .layout
        .columns
        .filter(|columns| *columns > 0)
        .unwrap_or_else(|| (count as f64).sqrt().ceil() as i64)
        .min(count)
        .max(1);
    let rows = snapshot
        .layout
        .rows
        .filter(|rows| *rows > 0)
        .unwrap_or_else(|| ceil_div(count, columns))
        .max(1);
    let cell_width = ((screen_width - gap * (columns + 1)) / columns).max(1);
    let cell_height = ((screen_height - gap * (rows + 1)) / rows).max(1);
    let mut placements = Vec::with_capacity(windows.len());

    for (index, window) in windows.iter().enumerate() {
        let index = index as i64;
        let column = index % columns;
        let row = index / columns;
        placements.push(SyncWindowPlacement {
            window_id: window.window_id.clone(),
            hwnd: sync_window_hwnd(window)?,
            x: offset_layout_position(work_area.x, gap + column * (cell_width + gap)),
            y: offset_layout_position(work_area.y, gap + row * (cell_height + gap)),
            width: clamp_layout_size(cell_width),
            height: clamp_layout_size(cell_height),
        });
    }

    Ok(placements)
}

#[cfg(target_os = "windows")]
fn cascade_window_placements(
    snapshot: &DesktopSynchronizerSnapshot,
    windows: &[&DesktopSyncWindowState],
) -> Result<Vec<SyncWindowPlacement>, String> {
    let work_area = primary_work_area()?;
    let screen_width = work_area.width as i64;
    let screen_height = work_area.height as i64;
    let gap = snapshot.layout.gap_px.max(0);
    let offset_x = snapshot.layout.overlap_offset_x.unwrap_or(40).max(0);
    let offset_y = snapshot.layout.overlap_offset_y.unwrap_or(40).max(0);
    let width = snapshot
        .layout
        .uniform_width
        .filter(|width| *width > 0)
        .unwrap_or_else(|| (screen_width * 2 / 3).max(1));
    let height = snapshot
        .layout
        .uniform_height
        .filter(|height| *height > 0)
        .unwrap_or_else(|| (screen_height * 2 / 3).max(1));
    let mut placements = Vec::with_capacity(windows.len());

    for (index, window) in windows.iter().enumerate() {
        let index = index as i64;
        placements.push(SyncWindowPlacement {
            window_id: window.window_id.clone(),
            hwnd: sync_window_hwnd(window)?,
            x: offset_layout_position(work_area.x, gap + index * offset_x),
            y: offset_layout_position(work_area.y, gap + index * offset_y),
            width: clamp_layout_size(width),
            height: clamp_layout_size(height),
        });
    }

    Ok(placements)
}

#[cfg(target_os = "windows")]
fn focus_stack_window_placements(
    snapshot: &DesktopSynchronizerSnapshot,
    windows: &[&DesktopSyncWindowState],
) -> Result<Vec<SyncWindowPlacement>, String> {
    let work_area = primary_work_area()?;
    let screen_width = work_area.width as i64;
    let screen_height = work_area.height as i64;
    let focus_id = snapshot
        .focused_window_id
        .as_ref()
        .or(snapshot.layout.main_window_id.as_ref());
    let mut ordered = windows.to_vec();
    if let Some(focus_id) = focus_id {
        ordered.sort_by_key(|window| if &window.window_id == focus_id { 0 } else { 1 });
    }

    let gap = snapshot.layout.gap_px.max(0);
    let stack_width = (screen_width / 4).max(240);
    let focus_width = (screen_width - stack_width - gap * 3).max(1);
    let focus_height = (screen_height - gap * 2).max(1);
    let stack_height = ((screen_height - gap * (ordered.len() as i64 + 1))
        / (ordered.len() as i64).max(1))
    .max(120);
    let mut placements = Vec::with_capacity(ordered.len());

    for (index, window) in ordered.iter().enumerate() {
        let is_focus = index == 0;
        placements.push(SyncWindowPlacement {
            window_id: window.window_id.clone(),
            hwnd: sync_window_hwnd(window)?,
            x: if is_focus {
                offset_layout_position(work_area.x, gap)
            } else {
                offset_layout_position(work_area.x, gap * 2 + focus_width)
            },
            y: if is_focus {
                offset_layout_position(work_area.y, gap)
            } else {
                offset_layout_position(work_area.y, gap + (index as i64 - 1) * (stack_height + gap))
            },
            width: if is_focus {
                clamp_layout_size(focus_width)
            } else {
                clamp_layout_size(stack_width)
            },
            height: if is_focus {
                clamp_layout_size(focus_height)
            } else {
                clamp_layout_size(stack_height)
            },
        });
    }

    Ok(placements)
}

#[cfg(target_os = "windows")]
fn uniform_size_window_placements(
    snapshot: &DesktopSynchronizerSnapshot,
    windows: &[&DesktopSyncWindowState],
) -> Result<Vec<SyncWindowPlacement>, String> {
    let width = snapshot
        .layout
        .uniform_width
        .filter(|width| *width > 0)
        .ok_or_else(|| {
            "uniform_size layout requires uniformWidth for native physical layout.".to_string()
        })?;
    let height = snapshot
        .layout
        .uniform_height
        .filter(|height| *height > 0)
        .ok_or_else(|| {
            "uniform_size layout requires uniformHeight for native physical layout.".to_string()
        })?;
    let mut placements = Vec::with_capacity(windows.len());

    for window in windows {
        let bounds = window.bounds.as_ref().ok_or_else(|| {
            format!(
                "sync window {} has no captured bounds for uniform_size native layout.",
                window.window_id
            )
        })?;
        placements.push(SyncWindowPlacement {
            window_id: window.window_id.clone(),
            hwnd: sync_window_hwnd(window)?,
            x: bounds.x.max(0).min(i32::MAX as i64) as i32,
            y: bounds.y.max(0).min(i32::MAX as i64) as i32,
            width: clamp_layout_size(width),
            height: clamp_layout_size(height),
        });
    }

    Ok(placements)
}

#[cfg(target_os = "windows")]
fn native_layout_placements(
    snapshot: &DesktopSynchronizerSnapshot,
) -> Result<Vec<SyncWindowPlacement>, String> {
    let windows = live_layout_windows(snapshot);
    if windows.is_empty() {
        return Err(
            "No visible, non-minimized sync windows are available for native layout.".to_string(),
        );
    }

    match snapshot.layout.mode.as_str() {
    "grid" | "tiled" => tiled_window_placements(snapshot, &windows),
    "overlap" | "cascade" => cascade_window_placements(snapshot, &windows),
    "focus_stack" => focus_stack_window_placements(snapshot, &windows),
    "uniform_size" => uniform_size_window_placements(snapshot, &windows),
    mode => Err(format!(
      "unsupported native sync layout mode: {mode}. expected one of: tiled, cascade, focus_stack, grid, overlap, uniform_size"
    )),
  }
}

#[cfg(target_os = "windows")]
fn apply_native_window_layout(snapshot: &DesktopSynchronizerSnapshot) -> Result<usize, String> {
    let placements = native_layout_placements(snapshot)?;

    for placement in &placements {
        unsafe {
            // SAFETY: placement.hwnd was parsed from a captured native handle and revalidated with IsWindow/IsWindowVisible before this call.
            SetWindowPos(
                placement.hwnd,
                HWND(std::ptr::null_mut()),
                placement.x,
                placement.y,
                placement.width,
                placement.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .map_err(|error| {
                format!(
                    "Failed to apply native physical layout to sync window {}: {error}",
                    placement.window_id
                )
            })?;
        }
    }

    Ok(placements.len())
}

#[cfg(target_os = "windows")]
fn broadcast_native_placement(
    controller_hwnd: HWND,
    target_hwnds: &[(String, HWND)],
    plan_id: &str,
    work_area: &WorkArea,
) -> Result<Vec<SyncWindowPlacement>, String> {
    let gap = 8i64;
    let target_count = target_hwnds.len() as i64;
    let screen_width = work_area.width as i64;
    let screen_height = work_area.height as i64;

    if target_hwnds.is_empty() {
        return Ok(vec![]);
    }

    match plan_id {
        "nav-mirror" => {
            let mut placements = Vec::with_capacity(target_hwnds.len() + 1);

            let controller_width = (screen_width * 3 / 5).max(640);
            let controller_height = (screen_height - gap * 2).max(480);
            placements.push(SyncWindowPlacement {
                window_id: "controller".to_string(),
                hwnd: controller_hwnd,
                x: offset_layout_position(work_area.x, gap),
                y: offset_layout_position(work_area.y, gap),
                width: clamp_layout_size(controller_width),
                height: clamp_layout_size(controller_height),
            });

            if target_count > 0 {
                let stack_width = (screen_width - controller_width - gap * 3).max(240);
                let stack_height =
                    ((screen_height - gap * (target_count + 1)) / target_count).max(120);
                for (index, (window_id, hwnd)) in target_hwnds.iter().enumerate() {
                    let index = index as i64;
                    placements.push(SyncWindowPlacement {
                        window_id: window_id.clone(),
                        hwnd: *hwnd,
                        x: offset_layout_position(work_area.x, gap * 2 + controller_width),
                        y: offset_layout_position(work_area.y, gap + index * (stack_height + gap)),
                        width: clamp_layout_size(stack_width),
                        height: clamp_layout_size(stack_height),
                    });
                }
            }

            Ok(placements)
        }
        "layout-regroup" => {
            let columns = (target_count as f64).sqrt().ceil() as i64;
            let rows = ceil_div(target_count, columns);
            let cell_width = ((screen_width - gap * (columns + 1)) / columns).max(240);
            let cell_height = ((screen_height - gap * (rows + 1)) / rows).max(180);
            let mut placements = Vec::with_capacity(target_hwnds.len() + 1);

            placements.push(SyncWindowPlacement {
                window_id: "controller".to_string(),
                hwnd: controller_hwnd,
                x: offset_layout_position(work_area.x, gap),
                y: offset_layout_position(work_area.y, gap),
                width: clamp_layout_size(cell_width),
                height: clamp_layout_size(cell_height),
            });

            for (index, (window_id, hwnd)) in target_hwnds.iter().enumerate() {
                let global_index = (index + 1) as i64;
                let column = global_index % columns;
                let row = global_index / columns;
                placements.push(SyncWindowPlacement {
                    window_id: window_id.clone(),
                    hwnd: *hwnd,
                    x: offset_layout_position(work_area.x, gap + column * (cell_width + gap)),
                    y: offset_layout_position(work_area.y, gap + row * (cell_height + gap)),
                    width: clamp_layout_size(cell_width),
                    height: clamp_layout_size(cell_height),
                });
            }

            Ok(placements)
        }
        "scroll-checkpoint" => {
            let offset_x = 48i64;
            let offset_y = 36i64;
            let check_width = (screen_width * 4 / 5).max(640);
            let check_height = (screen_height * 4 / 5).max(480);
            let mut placements = Vec::with_capacity(target_hwnds.len() + 1);

            placements.push(SyncWindowPlacement {
                window_id: "controller".to_string(),
                hwnd: controller_hwnd,
                x: offset_layout_position(work_area.x, gap),
                y: offset_layout_position(work_area.y, gap),
                width: clamp_layout_size(check_width),
                height: clamp_layout_size(check_height),
            });

            for (index, (window_id, hwnd)) in target_hwnds.iter().enumerate() {
                let index = index as i64 + 1;
                placements.push(SyncWindowPlacement {
                    window_id: window_id.clone(),
                    hwnd: *hwnd,
                    x: offset_layout_position(work_area.x, gap + index * offset_x),
                    y: offset_layout_position(work_area.y, gap + index * offset_y),
                    width: clamp_layout_size(check_width),
                    height: clamp_layout_size(check_height),
                });
            }

            Ok(placements)
        }
        "input-burst" => Ok(vec![SyncWindowPlacement {
            window_id: "controller".to_string(),
            hwnd: controller_hwnd,
            x: work_area.x,
            y: work_area.y,
            width: work_area.width,
            height: work_area.height,
        }]),
        _ => Err(format!(
            "unsupported broadcast plan for native physical placement: {plan_id}"
        )),
    }
}

#[cfg(target_os = "windows")]
fn apply_broadcast_native_layout(
    plan_id: &str,
    controller_hwnd: HWND,
    target_hwnds: &[(String, HWND)],
) -> Result<usize, String> {
    let work_area = primary_work_area()?;
    let placements =
        broadcast_native_placement(controller_hwnd, target_hwnds, plan_id, &work_area)?;

    for placement in &placements {
        unsafe {
            SetWindowPos(
                placement.hwnd,
                HWND(std::ptr::null_mut()),
                placement.x,
                placement.y,
                placement.width,
                placement.height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .map_err(|error| {
                format!(
                    "Failed to apply native broadcast placement to window {}: {error}",
                    placement.window_id
                )
            })?;
        }
    }

    Ok(placements.len())
}

#[cfg(not(target_os = "windows"))]
fn apply_native_window_layout(_snapshot: &DesktopSynchronizerSnapshot) -> Result<usize, String> {
    Err("Native physical sync window layout is not supported on this platform.".to_string())
}

fn apply_sync_layout_update(
    snapshot: &mut DesktopSynchronizerSnapshot,
    update: DesktopSyncLayoutUpdate,
) -> Result<bool, String> {
    let mut changed = false;

    if let Some(mode) = update.mode {
        let normalized_mode = normalize_sync_layout_mode(&mode)?;
        if snapshot.layout.mode != normalized_mode {
            snapshot.layout.mode = normalized_mode;
            changed = true;
        }
    }

    if let Some(columns) = update.columns {
        if columns <= 0 {
            return Err("layout columns must be greater than 0".to_string());
        }
        if snapshot.layout.columns != Some(columns) {
            snapshot.layout.columns = Some(columns);
            changed = true;
        }
    }

    if let Some(rows) = update.rows {
        if rows <= 0 {
            return Err("layout rows must be greater than 0".to_string());
        }
        if snapshot.layout.rows != Some(rows) {
            snapshot.layout.rows = Some(rows);
            changed = true;
        }
    }

    if let Some(gap_px) = update.gap_px {
        if gap_px < 0 {
            return Err("layout gapPx must be greater than or equal to 0".to_string());
        }
        if snapshot.layout.gap_px != gap_px {
            snapshot.layout.gap_px = gap_px;
            changed = true;
        }
    }

    if let Some(overlap_offset_x) = update.overlap_offset_x {
        if snapshot.layout.overlap_offset_x != Some(overlap_offset_x) {
            snapshot.layout.overlap_offset_x = Some(overlap_offset_x);
            changed = true;
        }
    }

    if let Some(overlap_offset_y) = update.overlap_offset_y {
        if snapshot.layout.overlap_offset_y != Some(overlap_offset_y) {
            snapshot.layout.overlap_offset_y = Some(overlap_offset_y);
            changed = true;
        }
    }

    if let Some(uniform_width) = update.uniform_width {
        if uniform_width <= 0 {
            return Err("layout uniformWidth must be greater than 0".to_string());
        }
        if snapshot.layout.uniform_width != Some(uniform_width) {
            snapshot.layout.uniform_width = Some(uniform_width);
            changed = true;
        }
    }

    if let Some(uniform_height) = update.uniform_height {
        if uniform_height <= 0 {
            return Err("layout uniformHeight must be greater than 0".to_string());
        }
        if snapshot.layout.uniform_height != Some(uniform_height) {
            snapshot.layout.uniform_height = Some(uniform_height);
            changed = true;
        }
    }

    if let Some(sync_scroll) = update.sync_scroll {
        if snapshot.layout.sync_scroll != sync_scroll {
            snapshot.layout.sync_scroll = sync_scroll;
            changed = true;
        }
    }

    if let Some(sync_navigation) = update.sync_navigation {
        if snapshot.layout.sync_navigation != sync_navigation {
            snapshot.layout.sync_navigation = sync_navigation;
            changed = true;
        }
    }

    if let Some(sync_input) = update.sync_input {
        if snapshot.layout.sync_input != sync_input {
            snapshot.layout.sync_input = sync_input;
            changed = true;
        }
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sync_snapshot() -> DesktopSynchronizerSnapshot {
        DesktopSynchronizerSnapshot {
            windows: Vec::new(),
            layout: DesktopSyncLayoutState {
                mode: "grid".to_string(),
                main_window_id: None,
                columns: Some(2),
                rows: Some(2),
                gap_px: 16,
                overlap_offset_x: Some(42),
                overlap_offset_y: Some(34),
                uniform_width: Some(960),
                uniform_height: Some(720),
                sync_scroll: true,
                sync_navigation: true,
                sync_input: false,
                updated_at: "2026-05-18T00:00:00.000Z".to_string(),
            },
            focused_window_id: None,
            updated_at: "2026-05-18T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn normalizes_supported_sync_layout_modes() {
        assert_eq!(normalize_sync_layout_mode(" Grid ").unwrap(), "grid");
        assert_eq!(
            normalize_sync_layout_mode("FOCUS_STACK").unwrap(),
            "focus_stack"
        );
        assert_eq!(
            normalize_sync_layout_mode("uniform_size").unwrap(),
            "uniform_size"
        );
    }

    #[test]
    fn rejects_unsupported_sync_layout_modes() {
        assert_eq!(
            normalize_sync_layout_mode("   ").unwrap_err(),
            "layout mode cannot be empty"
        );
        assert!(normalize_sync_layout_mode("fullscreen")
            .unwrap_err()
            .contains("unsupported sync layout mode"));
    }

    #[test]
    fn rejects_invalid_sync_layout_update_dimensions() {
        let mut snapshot = sync_snapshot();

        assert_eq!(
            apply_sync_layout_update(
                &mut snapshot,
                DesktopSyncLayoutUpdate {
                    columns: Some(0),
                    ..Default::default()
                },
            )
            .unwrap_err(),
            "layout columns must be greater than 0"
        );
        assert_eq!(
            apply_sync_layout_update(
                &mut snapshot,
                DesktopSyncLayoutUpdate {
                    rows: Some(0),
                    ..Default::default()
                },
            )
            .unwrap_err(),
            "layout rows must be greater than 0"
        );
        assert_eq!(
            apply_sync_layout_update(
                &mut snapshot,
                DesktopSyncLayoutUpdate {
                    gap_px: Some(-1),
                    ..Default::default()
                },
            )
            .unwrap_err(),
            "layout gapPx must be greater than or equal to 0"
        );
        assert_eq!(
            apply_sync_layout_update(
                &mut snapshot,
                DesktopSyncLayoutUpdate {
                    uniform_width: Some(0),
                    ..Default::default()
                },
            )
            .unwrap_err(),
            "layout uniformWidth must be greater than 0"
        );
        assert_eq!(
            apply_sync_layout_update(
                &mut snapshot,
                DesktopSyncLayoutUpdate {
                    uniform_height: Some(0),
                    ..Default::default()
                },
            )
            .unwrap_err(),
            "layout uniformHeight must be greater than 0"
        );
    }

    #[test]
    fn applies_valid_sync_layout_update() {
        let mut snapshot = sync_snapshot();

        let changed = apply_sync_layout_update(
            &mut snapshot,
            DesktopSyncLayoutUpdate {
                mode: Some(" Cascade ".to_string()),
                columns: Some(3),
                rows: Some(2),
                gap_px: Some(20),
                sync_input: Some(true),
                ..Default::default()
            },
        )
        .unwrap();

        assert!(changed);
        assert_eq!(snapshot.layout.mode, "cascade");
        assert_eq!(snapshot.layout.columns, Some(3));
        assert_eq!(snapshot.layout.rows, Some(2));
        assert_eq!(snapshot.layout.gap_px, 20);
        assert!(snapshot.layout.sync_input);
    }
}

#[tauri::command]
pub fn list_validation_reports(
    state: State<'_, DesktopState>,
) -> Result<Vec<DesktopValidationReportSummary>, String> {
    let report_dir = validation_report_dir(&state)?;
    let reports = read_validation_reports_from_dir(&report_dir)?;
    Ok(reports.iter().map(validation_report_summary).collect())
}

#[tauri::command]
pub fn export_validation_profile_evidence(
    state: State<'_, DesktopState>,
    profile_id: Option<String>,
) -> Result<DesktopValidationProfileExport, String> {
    let report_dir = validation_report_dir(&state)?;
    let reports = read_validation_reports_from_dir(&report_dir)?;
    let export_id = format!("profile-evidence-export-{}", validation_report_id());
    let exported_at = now_ts_string();
    let safe_profile = profile_id
        .as_deref()
        .map(|value| {
            value
                .chars()
                .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "all-profiles".to_string());
    let export_path = report_dir.join(format!("profile-evidence-export-{safe_profile}-{exported_at}.json"));
    let report_count = reports.len();
    let payload = DesktopValidationProfileExportPayload {
        export_id: export_id.clone(),
        profile_id: profile_id.clone(),
        exported_at: exported_at.clone(),
        reports,
    };
    let raw = serde_json::to_string_pretty(&payload)
        .map_err(|error| format!("Failed to serialize profile evidence export: {error}"))?;
    fs::write(&export_path, raw).map_err(|error| {
        format!(
            "Failed to write profile evidence export {}: {error}",
            export_path.display()
        )
    })?;

    Ok(DesktopValidationProfileExport {
        export_id,
        profile_id,
        exported_at,
        report_count,
        export_path: export_path.to_string_lossy().to_string(),
        summary: format!("Exported {report_count} validation report(s) for evidence review."),
    })
}

#[tauri::command]
pub async fn collect_validation_report(
    state: State<'_, DesktopState>,
    browser_signals: Option<Vec<DesktopValidationSignal>>,
) -> Result<DesktopValidationReport, String> {
    build_validation_report(&state, browser_signals.unwrap_or_default()).await
}

#[tauri::command]
pub async fn get_app_status(
    state: State<'_, DesktopState>,
) -> Result<DesktopStatusSnapshot, String> {
    load_desktop_status(&state.db, Some(&state.database_url))
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn list_task_page(
    state: State<'_, DesktopState>,
    query: DesktopTaskQuery,
) -> Result<DesktopTaskPage, String> {
    load_desktop_tasks(&state.db, query)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn list_log_page(
    state: State<'_, DesktopState>,
    query: DesktopLogQuery,
) -> Result<DesktopLogPage, String> {
    load_desktop_logs(&state.db, query)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub fn read_local_settings(
    state: State<'_, DesktopState>,
) -> Result<DesktopSettingsSnapshot, String> {
    Ok(read_desktop_settings(Some(&state.database_url)))
}

#[tauri::command]
pub fn apply_runtime_settings(
    state: State<'_, DesktopState>,
    draft: DesktopRuntimeSettingsDraft,
) -> Result<DesktopSettingsMutationResult, String> {
    apply_desktop_runtime_settings(&state.database_url, draft).map_err(normalize_error)
}

#[tauri::command]
pub fn restore_runtime_settings_defaults(
    state: State<'_, DesktopState>,
) -> Result<DesktopSettingsMutationResult, String> {
    restore_desktop_runtime_settings_defaults(&state.database_url).map_err(normalize_error)
}

#[tauri::command]
pub fn read_local_api_snapshot(
    state: State<'_, DesktopState>,
) -> Result<DesktopLocalApiSnapshot, String> {
    Ok(read_desktop_local_api_snapshot(Some(&state.database_url)))
}

#[tauri::command]
pub fn apply_local_api_settings(
    state: State<'_, DesktopState>,
    draft: DesktopLocalApiSettingsDraft,
) -> Result<DesktopLocalApiMutationResult, String> {
    apply_desktop_local_api_settings(&state.database_url, draft).map_err(normalize_error)
}

#[tauri::command]
pub fn restore_local_api_defaults(
    state: State<'_, DesktopState>,
) -> Result<DesktopLocalApiMutationResult, String> {
    restore_desktop_local_api_defaults(&state.database_url).map_err(normalize_error)
}

#[tauri::command]
pub fn read_browser_environment_policy(
    state: State<'_, DesktopState>,
) -> Result<DesktopBrowserEnvironmentPolicySnapshot, String> {
    Ok(read_desktop_browser_environment_policy(Some(
        &state.database_url,
    )))
}

#[tauri::command]
pub fn apply_browser_environment_policy(
    state: State<'_, DesktopState>,
    draft: DesktopBrowserEnvironmentPolicyDraft,
) -> Result<DesktopBrowserEnvironmentPolicyMutationResult, String> {
    apply_desktop_browser_environment_policy(&state.database_url, draft).map_err(normalize_error)
}

#[tauri::command]
pub fn restore_browser_environment_policy_defaults(
    state: State<'_, DesktopState>,
) -> Result<DesktopBrowserEnvironmentPolicyMutationResult, String> {
    restore_desktop_browser_environment_policy_defaults(&state.database_url)
        .map_err(normalize_error)
}

#[tauri::command]
pub fn read_local_asset_workspace(
    state: State<'_, DesktopState>,
) -> Result<DesktopLocalAssetWorkspaceSnapshot, String> {
    Ok(read_desktop_local_asset_workspace(Some(
        &state.database_url,
    )))
}

#[tauri::command]
pub fn read_import_export_skeleton(
    state: State<'_, DesktopState>,
) -> Result<DesktopImportExportSkeleton, String> {
    Ok(read_desktop_import_export_skeleton(Some(
        &state.database_url,
    )))
}

#[tauri::command]
pub async fn export_session_bundle(
    state: State<'_, DesktopState>,
    request: DesktopSessionBundleExportRequest,
) -> Result<DesktopSessionBundleExport, String> {
    export_desktop_session_bundle(&state.db, &state.database_url, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub fn open_local_asset_entry(
    state: State<'_, DesktopState>,
    entry_id: String,
) -> Result<(), String> {
    let (path, select_file) =
        resolve_desktop_local_asset_entry_path(&state.database_url, &entry_id)
            .map_err(normalize_error)?;

    if select_file {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to prepare asset parent {}: {error}",
                    parent.display()
                )
            })?;
        }
    } else {
        fs::create_dir_all(&path).map_err(|error| {
            format!(
                "Failed to prepare asset directory {}: {error}",
                path.display()
            )
        })?;
    }

    open_path_in_explorer(&path, select_file)
}

#[tauri::command]
pub fn open_local_directory(state: State<'_, DesktopState>, target: String) -> Result<(), String> {
    let snapshot = read_desktop_settings(Some(&state.database_url));
    let path = resolve_directory_target(&snapshot, &target)?;

    fs::create_dir_all(&path)
        .map_err(|error| format!("Failed to prepare directory {}: {error}", path.display()))?;

    Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map_err(|error| format!("Failed to open directory {}: {error}", path.display()))?;

    Ok(())
}

#[tauri::command]
pub fn read_local_runtime_status(
    state: State<'_, DesktopState>,
) -> Result<DesktopRuntimeStatus, String> {
    build_runtime_status(&state)
}

#[tauri::command]
pub fn start_local_runtime(state: State<'_, DesktopState>) -> Result<DesktopRuntimeStatus, String> {
    let snapshot = read_desktop_settings(Some(&state.database_url));
    let project_root = PathBuf::from(&snapshot.project_root);
    let binary_path = resolve_runtime_binary(&project_root)?;
    let (log_dir, stdout_path, stderr_path) = runtime_log_paths(&snapshot);

    {
        let mut runtime = state
            .runtime
            .lock()
            .map_err(|_| "Failed to lock local runtime state".to_string())?;
        let last_exit_code = runtime.last_exit_code;

        if let Some(process) = runtime.managed_process.as_mut() {
            match process.child.try_wait() {
                Ok(None) => {
                    return Ok(status_from_managed_process(
                        process,
                        is_api_reachable(),
                        last_exit_code,
                    ));
                }
                Ok(Some(status)) => {
                    runtime.last_exit_code = status.code();
                    runtime.managed_process = None;
                }
                Err(error) => {
                    return Err(format!("Failed to inspect local runtime process: {error}"));
                }
            }
        }
    }

    if is_api_reachable() {
        return Err(
      "A local runtime is already reachable at http://127.0.0.1:3000. Stop the external process first or refresh the status.".to_string(),
    );
    }

    fs::create_dir_all(&log_dir).map_err(|error| {
        format!(
            "Failed to create runtime log directory {}: {error}",
            log_dir.display()
        )
    })?;

    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&stdout_path)
        .map_err(|error| {
            format!(
                "Failed to open runtime stdout log {}: {error}",
                stdout_path.display()
            )
        })?;
    let stderr_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&stderr_path)
        .map_err(|error| {
            format!(
                "Failed to open runtime stderr log {}: {error}",
                stderr_path.display()
            )
        })?;

    let mut command = Command::new(&binary_path);
    command
        .current_dir(&project_root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .env("PERSONA_PILOT_DATABASE_URL", &snapshot.database_url)
        .env("PERSONA_PILOT_RUNNER", &snapshot.runner_kind)
        .env(
            "PERSONA_PILOT_RUNNER_CONCURRENCY",
            snapshot.worker_count.to_string(),
        )
        .env(
            "PERSONA_PILOT_RUNNER_HEARTBEAT_SECONDS",
            snapshot.heartbeat_interval_seconds.to_string(),
        )
        .env(
            "PERSONA_PILOT_RUNNER_CLAIM_RETRY_LIMIT",
            snapshot.claim_retry_limit.to_string(),
        )
        .env(
            "PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MIN_MS",
            snapshot.idle_backoff_min_ms.to_string(),
        )
        .env(
            "PERSONA_PILOT_RUNNER_IDLE_BACKOFF_MAX_MS",
            snapshot.idle_backoff_max_ms.to_string(),
        )
        .creation_flags(CREATE_NO_WINDOW);

    if let Some(reclaim_after_seconds) = snapshot.reclaim_after_seconds {
        command.env(
            "PERSONA_PILOT_RUNNER_RECLAIM_SECONDS",
            reclaim_after_seconds.to_string(),
        );
    } else {
        command.env_remove("PERSONA_PILOT_RUNNER_RECLAIM_SECONDS");
    }

    let child = command.spawn().map_err(|error| {
        format!(
            "Failed to start local runtime {}: {error}",
            binary_path.display()
        )
    })?;
    let pid = child.id();

    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "Failed to lock local runtime state".to_string())?;
    runtime.last_exit_code = None;
    runtime.managed_process = Some(ManagedRuntimeProcess {
        child,
        pid,
        started_at: now_ts_string(),
        binary_path: binary_path.to_string_lossy().to_string(),
        log_dir: log_dir.to_string_lossy().to_string(),
        stdout_path: stdout_path.to_string_lossy().to_string(),
        stderr_path: stderr_path.to_string_lossy().to_string(),
    });

    drop(runtime);
    build_runtime_status(&state)
}

#[tauri::command]
pub fn stop_local_runtime(state: State<'_, DesktopState>) -> Result<DesktopRuntimeStatus, String> {
    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "Failed to lock local runtime state".to_string())?;

    let Some(mut process) = runtime.managed_process.take() else {
        if is_api_reachable() {
            return Err(
        "A local runtime is reachable, but it was not started by the desktop shell in this session.".to_string(),
      );
        }
        return Ok(DesktopRuntimeStatus {
            status: "stopped".to_string(),
            running: false,
            managed: false,
            pid: None,
            started_at: None,
            health_url: LOCAL_RUNTIME_HEALTH_URL.to_string(),
            api_reachable: false,
            binary_path: None,
            log_dir: None,
            stdout_path: None,
            stderr_path: None,
            last_exit_code: runtime.last_exit_code,
        });
    };

    match process.child.try_wait() {
        Ok(Some(status)) => {
            runtime.last_exit_code = status.code();
            drop(runtime);
            return build_runtime_status(&state);
        }
        Ok(None) => {}
        Err(error) => {
            return Err(format!("Failed to inspect local runtime process: {error}"));
        }
    }

    process.child.kill().map_err(|error| {
        format!(
            "Failed to stop local runtime process {}: {error}",
            process.pid
        )
    })?;
    let status = process.child.wait().map_err(|error| {
        format!(
            "Failed to wait for local runtime process {}: {error}",
            process.pid
        )
    })?;
    runtime.last_exit_code = status.code();

    drop(runtime);
    build_runtime_status(&state)
}

#[tauri::command]
pub async fn list_profile_page(
    state: State<'_, DesktopState>,
    query: DesktopProfilePageQuery,
) -> Result<DesktopProfilePage, String> {
    load_desktop_profile_page(&state.db, query)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn read_profile_detail(
    state: State<'_, DesktopState>,
    profile_id: String,
) -> Result<DesktopProfileDetail, String> {
    load_desktop_profile_detail(&state.db, &profile_id)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn create_profile(
    state: State<'_, DesktopState>,
    input: DesktopCreateProfileInput,
) -> Result<DesktopProfileMutationResult, String> {
    create_desktop_profile(&state.db, input)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn update_profile(
    state: State<'_, DesktopState>,
    input: DesktopUpdateProfileInput,
) -> Result<DesktopProfileMutationResult, String> {
    update_desktop_profile(&state.db, input)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn start_profiles(
    state: State<'_, DesktopState>,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult, String> {
    start_desktop_profiles(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn stop_profiles(
    state: State<'_, DesktopState>,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult, String> {
    stop_desktop_profiles(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn open_profiles(
    state: State<'_, DesktopState>,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult, String> {
    open_desktop_profiles(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn check_profile_proxies(
    state: State<'_, DesktopState>,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult, String> {
    check_desktop_profile_proxies(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn sync_profiles(
    state: State<'_, DesktopState>,
    request: DesktopProfileBatchActionRequest,
) -> Result<DesktopProfileBatchActionResult, String> {
    sync_desktop_profiles(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn list_proxy_page(
    state: State<'_, DesktopState>,
    query: DesktopProxyPageQuery,
) -> Result<DesktopProxyPage, String> {
    load_desktop_proxy_page(&state.db, query)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn read_proxy_health(
    state: State<'_, DesktopState>,
    proxy_id: String,
) -> Result<DesktopProxyHealth, String> {
    load_desktop_proxy_health(&state.db, &proxy_id)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn read_proxy_usage(
    state: State<'_, DesktopState>,
    proxy_id: String,
) -> Result<Vec<DesktopProxyUsageItem>, String> {
    load_desktop_proxy_usage(&state.db, &proxy_id)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn check_proxy_batch(
    state: State<'_, DesktopState>,
    request: DesktopProxyBatchCheckRequest,
) -> Result<DesktopProxyBatchCheckResponse, String> {
    run_desktop_proxy_batch_check(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn change_proxy_ip(
    state: State<'_, DesktopState>,
    request: DesktopProxyChangeIpRequest,
) -> Result<DesktopProxyChangeIpResult, String> {
    change_desktop_proxy_ip(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn list_template_metadata_page(
    state: State<'_, DesktopState>,
    query: DesktopTemplateMetadataPageQuery,
) -> Result<DesktopTemplateMetadataPage, String> {
    load_desktop_template_metadata_page(&state.db, query)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn save_template(
    state: State<'_, DesktopState>,
    input: DesktopTemplateUpsertInput,
) -> Result<DesktopTemplateMutationResult, String> {
    save_desktop_template(&state.db, input)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn update_template(
    state: State<'_, DesktopState>,
    input: DesktopTemplateUpsertInput,
) -> Result<DesktopTemplateMutationResult, String> {
    update_desktop_template(&state.db, input)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn delete_template(
    state: State<'_, DesktopState>,
    input: DesktopTemplateDeleteInput,
) -> Result<DesktopTemplateMutationResult, String> {
    delete_desktop_template(&state.db, input)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn compile_template_run(
    state: State<'_, DesktopState>,
    request: DesktopCompileTemplateRunRequest,
) -> Result<DesktopCompileTemplateRunResult, String> {
    compile_desktop_template_run(&state.db, &state.database_url, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn launch_template_run(
    state: State<'_, DesktopState>,
    request: DesktopLaunchTemplateRunRequest,
) -> Result<DesktopLaunchTemplateRunResult, String> {
    launch_desktop_template_run(&state.db, &state.database_url, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn read_run_detail(
    state: State<'_, DesktopState>,
    query: DesktopReadRunDetailQuery,
) -> Result<DesktopRunDetail, String> {
    read_desktop_run_detail(&state.db, query)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn retry_task(
    state: State<'_, DesktopState>,
    task_id: String,
) -> Result<DesktopTaskWriteResult, String> {
    retry_desktop_task(&state.db, &task_id)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn cancel_task(
    state: State<'_, DesktopState>,
    task_id: String,
) -> Result<DesktopTaskWriteResult, String> {
    let snapshot = read_desktop_local_api_snapshot(Some(&state.database_url));
    let task_id = task_id.trim().to_string();
    let cancel_result = cancel_task_via_local_api(&snapshot, &task_id).await?;

    Ok(DesktopTaskWriteResult {
        task_id: cancel_result.id,
        status: cancel_result.status,
        message: cancel_result.message,
        updated_at: now_ts_string(),
        run_id: None,
        manual_gate_request_id: None,
    })
}

#[tauri::command]
pub async fn confirm_manual_gate(
    state: State<'_, DesktopState>,
    request: DesktopManualGateActionRequest,
) -> Result<DesktopTaskWriteResult, String> {
    confirm_desktop_manual_gate(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub async fn reject_manual_gate(
    state: State<'_, DesktopState>,
    request: DesktopManualGateActionRequest,
) -> Result<DesktopTaskWriteResult, String> {
    reject_desktop_manual_gate(&state.db, request)
        .await
        .map_err(normalize_error)
}

#[tauri::command]
pub fn read_recorder_snapshot(
    state: State<'_, DesktopState>,
    query: DesktopRecorderSnapshotQuery,
) -> Result<DesktopRecorderSnapshot, String> {
    let recorder = state
        .recorder
        .lock()
        .map_err(|_| "Failed to lock recorder state".to_string())?;
    let snapshot = recorder.snapshot.clone();

    if let Some(session_id) = query.session_id.as_deref() {
        if snapshot.session_id != session_id && snapshot.status != "recording" {
            return Ok(placeholder_recorder_snapshot(query));
        }
    }

    let mut resolved = snapshot;
    if query.profile_id.is_some() {
        resolved.profile_id = query.profile_id;
    }
    if query.platform_id.is_some() {
        resolved.platform_id = query.platform_id;
    }
    if query.template_id.is_some() {
        resolved.template_id = query.template_id;
    }
    Ok(resolved)
}

#[tauri::command]
pub fn start_behavior_recording(
    state: State<'_, DesktopState>,
    request: DesktopStartBehaviorRecordingRequest,
) -> Result<DesktopRecorderSnapshot, String> {
    let mut recorder = state
        .recorder
        .lock()
        .map_err(|_| "Failed to lock recorder state".to_string())?;
    let now = now_ts_string();
    let session_id = request
        .session_id
        .unwrap_or_else(|| format!("recorder-session-{now}"));
    let snapshot = DesktopRecorderSnapshot {
        session_id,
        status: "recording".to_string(),
        profile_id: request.profile_id,
        platform_id: request.platform_id,
        template_id: request.template_id,
        current_tab_id: Some("tab-active".to_string()),
        current_url: Some("about:blank".to_string()),
        is_dirty: false,
        can_undo: false,
        can_redo: false,
        step_count: 0,
        sensitive_step_count: 0,
        variable_count: 0,
        started_at: Some(now.clone()),
        stopped_at: None,
        updated_at: now.clone(),
        tabs: vec![persona_pilot::desktop::DesktopRecorderTabSnapshot {
            tab_id: "tab-active".to_string(),
            title: Some("Recording Session".to_string()),
            url: Some("about:blank".to_string()),
            active: true,
        }],
        steps: Vec::new(),
    };
    recorder.snapshot = snapshot.clone();
    Ok(snapshot)
}

#[tauri::command]
pub fn stop_behavior_recording(
    state: State<'_, DesktopState>,
    request: DesktopStopBehaviorRecordingRequest,
) -> Result<DesktopRecorderSnapshot, String> {
    let mut recorder = state
        .recorder
        .lock()
        .map_err(|_| "Failed to lock recorder state".to_string())?;
    if let Some(session_id) = request.session_id.as_deref() {
        if recorder.snapshot.session_id != session_id {
            return Err(format!("recorder session not found: {session_id}"));
        }
    }

    let now = now_ts_string();
    recorder.snapshot.status = "stopped".to_string();
    recorder.snapshot.stopped_at = Some(now.clone());
    recorder.snapshot.updated_at = now;
    Ok(recorder.snapshot.clone())
}

#[tauri::command]
pub fn append_behavior_recording_step(
    state: State<'_, DesktopState>,
    request: DesktopAppendBehaviorRecordingStepRequest,
) -> Result<DesktopRecorderSnapshot, String> {
    let mut recorder = state
        .recorder
        .lock()
        .map_err(|_| "Failed to lock recorder state".to_string())?;
    let now = now_ts_string();
    let requested_session_id = request.session_id.clone();
    let needs_new_session = recorder.snapshot.status != "recording"
        || requested_session_id
            .as_ref()
            .map(|session_id| recorder.snapshot.session_id != *session_id)
            .unwrap_or(false);
    let session_id = requested_session_id.unwrap_or_else(|| {
        if needs_new_session {
            format!("recorder-session-{now}")
        } else {
            recorder.snapshot.session_id.clone()
        }
    });

    if needs_new_session || recorder.snapshot.session_id != session_id {
        recorder.snapshot = create_recording_snapshot(&request, session_id, &now);
    }

    if recorder
        .snapshot
        .steps
        .iter()
        .any(|step| step.id == request.step_id)
    {
        recorder.snapshot.updated_at = now;
        return Ok(recorder.snapshot.clone());
    }

    if request.profile_id.is_some() {
        recorder.snapshot.profile_id = request.profile_id.clone();
    }
    if request.platform_id.is_some() {
        recorder.snapshot.platform_id = request.platform_id.clone();
    }
    if request.template_id.is_some() {
        recorder.snapshot.template_id = request.template_id.clone();
    }

    let tab_id = request
        .tab_id
        .clone()
        .unwrap_or_else(|| "tab-active".to_string());
    upsert_recorder_tab(&mut recorder.snapshot.tabs, &tab_id, request.url.clone());

    recorder
        .snapshot
        .steps
        .push(persona_pilot::desktop::DesktopRecorderStep {
            id: request.step_id.clone(),
            index: request.index,
            action_type: request.action_type.clone(),
            label: request.label.clone(),
            tab_id: Some(tab_id.clone()),
            url: request.url.clone(),
            selector: request.selector.clone(),
            selector_source: request.selector_source.clone(),
            input_key: request.input_key.clone(),
            value_preview: request.value_preview.clone(),
            value_source: request.value_source.clone(),
            wait_ms: request.wait_ms,
            sensitive: request.sensitive,
            captured_at: now.clone(),
            metadata: request.metadata.clone(),
        });

    recorder.snapshot.status = "recording".to_string();
    recorder.snapshot.current_tab_id = Some(tab_id);
    if request.url.is_some() {
        recorder.snapshot.current_url = request.url.clone();
    }
    recorder.snapshot.is_dirty = true;
    recorder.snapshot.can_undo = !recorder.snapshot.steps.is_empty();
    recorder.snapshot.can_redo = false;
    recorder.snapshot.step_count = recorder.snapshot.steps.len() as i64;
    recorder.snapshot.sensitive_step_count = recorder
        .snapshot
        .steps
        .iter()
        .filter(|step| step.sensitive)
        .count() as i64;
    recorder.snapshot.variable_count = count_recorder_variables(&recorder.snapshot.steps);
    recorder.snapshot.stopped_at = None;
    recorder.snapshot.updated_at = now;

    Ok(recorder.snapshot.clone())
}

#[tauri::command]
pub fn list_sync_windows(
    state: State<'_, DesktopState>,
) -> Result<Vec<DesktopSyncWindowState>, String> {
    let mut synchronizer = state
        .synchronizer
        .lock()
        .map_err(|_| "Failed to lock synchronizer state".to_string())?;
    let snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;
    let windows = snapshot.windows.clone();
    synchronizer.snapshot = snapshot;
    Ok(windows)
}

#[tauri::command]
pub fn read_sync_layout_state(
    state: State<'_, DesktopState>,
) -> Result<DesktopSyncLayoutState, String> {
    let synchronizer = state
        .synchronizer
        .lock()
        .map_err(|_| "Failed to lock synchronizer state".to_string())?;
    Ok(synchronizer.snapshot.layout.clone())
}

#[tauri::command]
pub fn read_synchronizer_snapshot(
    state: State<'_, DesktopState>,
) -> Result<DesktopSynchronizerSnapshot, String> {
    let mut synchronizer = state
        .synchronizer
        .lock()
        .map_err(|_| "Failed to lock synchronizer state".to_string())?;
    let snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;
    synchronizer.snapshot = snapshot.clone();
    Ok(snapshot)
}

#[tauri::command]
pub fn set_main_sync_window(
    state: State<'_, DesktopState>,
    window_id: String,
) -> Result<DesktopSynchronizerActionResult, String> {
    let mut synchronizer = state
        .synchronizer
        .lock()
        .map_err(|_| "Failed to lock synchronizer state".to_string())?;
    let mut snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;
    if !snapshot
        .windows
        .iter()
        .any(|window| window.window_id == window_id)
    {
        return Err(format!("sync window not found: {window_id}"));
    }

    let now = now_ts_string();
    for window in &mut snapshot.windows {
        window.is_main_window = window.window_id == window_id;
        if window.is_main_window {
            window.last_action_at = Some(now.clone());
        }
    }
    snapshot.layout.main_window_id = Some(window_id.clone());
    snapshot.layout.updated_at = now.clone();
    snapshot.updated_at = now;

    synchronizer.snapshot = snapshot.clone();
    let msg = format!("Set main sync window to {window_id}.");
    Ok(sync_action_result(
        "set_main_sync_window",
        snapshot,
        &msg,
    ))
}

#[tauri::command]
pub fn apply_window_layout(
    state: State<'_, DesktopState>,
    layout: DesktopSyncLayoutUpdate,
) -> Result<DesktopSynchronizerActionResult, String> {
    #[cfg(not(target_os = "windows"))]
    {
        return Err(
            "Native physical sync window layout is not supported on this platform.".to_string(),
        );
    }

    let mut synchronizer = state
        .synchronizer
        .lock()
        .map_err(|_| "Failed to lock synchronizer state".to_string())?;
    let mut snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;
    let changed = apply_sync_layout_update(&mut snapshot, layout)?;
    let applied_count = apply_native_window_layout(&snapshot)?;
    let mut snapshot = capture_live_synchronizer_snapshot(&snapshot)?;
    let now = now_ts_string();
    snapshot.layout.updated_at = now.clone();
    snapshot.updated_at = now;

    let message = if changed {
        format!("Updated synchronizer layout state and applied native physical layout to {applied_count} windows.")
    } else {
        format!("Layout update request produced no state delta; native physical layout was applied to {applied_count} windows.")
    };

    synchronizer.snapshot = snapshot.clone();
    Ok(sync_action_result(
        "apply_window_layout",
        snapshot,
        &message,
    ))
}

#[tauri::command]
pub fn apply_broadcast_plan(
    state: State<'_, DesktopState>,
    request: DesktopSynchronizerBroadcastRequest,
) -> Result<DesktopSynchronizerActionResult, String> {
    let plan_id = request.plan_id.trim().to_string();
    if plan_id.is_empty() {
        return Err("broadcast plan id is required".to_string());
    }

    let mut synchronizer = state
        .synchronizer
        .lock()
        .map_err(|_| "Failed to lock synchronizer state".to_string())?;
    let mut snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;
    let available_ids: HashSet<String> = snapshot
        .windows
        .iter()
        .map(|window| window.window_id.clone())
        .collect();
    let controller_window_id = request
        .controller_window_id
        .or_else(|| snapshot.layout.main_window_id.clone())
        .or_else(|| snapshot.focused_window_id.clone())
        .ok_or_else(|| "broadcast requires a controller window".to_string())?;

    if !available_ids.contains(&controller_window_id) {
        return Err(format!(
            "broadcast controller window not found: {controller_window_id}"
        ));
    }

    let target_window_ids = request.target_window_ids.unwrap_or_else(|| {
        snapshot
            .windows
            .iter()
            .filter(|window| window.window_id != controller_window_id && window.status != "missing")
            .map(|window| window.window_id.clone())
            .collect()
    });
    let mut unique_target_ids = HashSet::new();
    let mut ordered_target_ids = Vec::new();
    for window_id in target_window_ids {
        if !available_ids.contains(&window_id) {
            return Err(format!("broadcast target window not found: {window_id}"));
        }
        if window_id != controller_window_id && unique_target_ids.insert(window_id.clone()) {
            ordered_target_ids.push(window_id);
        }
    }

    let now = now_ts_string();
    for window in &mut snapshot.windows {
        if window.window_id == controller_window_id || unique_target_ids.contains(&window.window_id)
        {
            window.last_action_at = Some(now.clone());
        }
    }
    snapshot.layout.main_window_id = Some(controller_window_id.clone());
    snapshot.layout.updated_at = now.clone();
    snapshot.updated_at = now;

    let target_count = unique_target_ids.len();

    #[cfg(target_os = "windows")]
    let applied_count: Result<usize, String> = {
        let controller_window = snapshot
            .windows
            .iter()
            .find(|w| w.window_id == controller_window_id)
            .ok_or_else(|| "broadcast controller not in snapshot".to_string())?;
        let controller_hwnd = sync_window_hwnd(controller_window)?;

        let target_hwnds: Result<Vec<(String, HWND)>, String> = ordered_target_ids
            .iter()
            .map(|window_id| {
                let window = snapshot
                    .windows
                    .iter()
                    .find(|w| w.window_id == *window_id)
                    .ok_or_else(|| format!("target window not found: {window_id}"))?;
                let hwnd = sync_window_hwnd(window)?;
                Ok((window_id.clone(), hwnd))
            })
            .collect();

        apply_broadcast_native_layout(&plan_id, controller_hwnd, &target_hwnds?)
    };

    #[cfg(not(target_os = "windows"))]
    let applied_count: Result<usize, String> =
        Err("Native physical broadcast layout is not supported on this platform.".to_string());

    synchronizer.snapshot = snapshot.clone();
    let msg = match applied_count {
        Ok(count) => format!(
            "Broadcast plan {plan_id} applied: controller {controller_window_id}, {target_count} targets, native physical arrangement applied to {count} windows."
        ),
        Err(layout_err) => format!(
            "Broadcast plan {plan_id} recorded for controller {controller_window_id} with {target_count} target windows. Native physical arrangement skipped: {layout_err}"
        ),
    };
    Ok(sync_action_result("apply_broadcast_plan", snapshot, &msg))
}

#[tauri::command]
pub fn focus_sync_window(
    state: State<'_, DesktopState>,
    window_id: String,
) -> Result<DesktopSynchronizerActionResult, String> {
    focus_window(&window_id)?;

    let mut synchronizer = state
        .synchronizer
        .lock()
        .map_err(|_| "Failed to lock synchronizer state".to_string())?;
    let snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;
    synchronizer.snapshot = snapshot.clone();
    let msg = format!("Focused sync window {window_id}.");
    Ok(sync_action_result(
        "focus_sync_window",
        snapshot,
        &msg,
    ))
}
