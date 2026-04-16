use std::{
  collections::HashSet,
  env,
  fs::{self, OpenOptions},
  net::{SocketAddr, TcpStream},
  os::windows::process::CommandExt,
  path::{Path, PathBuf},
  process::{Command, Stdio},
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use persona_pilot::desktop::{
  apply_desktop_browser_environment_policy, apply_desktop_local_api_settings,
  apply_desktop_runtime_settings, check_desktop_profile_proxies,
  change_desktop_proxy_ip, confirm_desktop_manual_gate, compile_desktop_template_run,
  create_desktop_profile, delete_desktop_template,
  launch_desktop_template_run, read_desktop_run_detail,
  load_desktop_logs, load_desktop_profile_detail, load_desktop_profile_page,
  load_desktop_proxy_health, load_desktop_proxy_page, load_desktop_proxy_usage,
  load_desktop_status, load_desktop_tasks, load_desktop_template_metadata_page,
  open_desktop_profiles, read_desktop_browser_environment_policy,
  read_desktop_import_export_skeleton, read_desktop_local_api_snapshot,
  read_desktop_local_asset_workspace, read_desktop_settings,
  reject_desktop_manual_gate, retry_desktop_task,
  resolve_desktop_local_asset_entry_path,
  restore_desktop_browser_environment_policy_defaults,
  restore_desktop_local_api_defaults, restore_desktop_runtime_settings_defaults,
  run_desktop_proxy_batch_check, save_desktop_template, start_desktop_profiles,
  stop_desktop_profiles, sync_desktop_profiles, update_desktop_profile,
  update_desktop_template, DesktopBrowserEnvironmentPolicyDraft,
  DesktopBrowserEnvironmentPolicyMutationResult,
  DesktopBrowserEnvironmentPolicySnapshot, DesktopCompileTemplateRunRequest,
  DesktopCompileTemplateRunResult, DesktopCreateProfileInput, DesktopLogPage,
  DesktopLaunchTemplateRunRequest, DesktopLaunchTemplateRunResult,
  DesktopImportExportSkeleton, DesktopLocalApiMutationResult,
  DesktopLocalApiSettingsDraft, DesktopLocalApiSnapshot,
  DesktopLocalAssetWorkspaceSnapshot, DesktopLogQuery,
  DesktopManualGateActionRequest,
  DesktopProfileBatchActionRequest, DesktopProfileBatchActionResult,
  DesktopProfileDetail, DesktopProfileMutationResult, DesktopProfilePage,
  DesktopProfilePageQuery, DesktopProxyBatchCheckRequest,
  DesktopProxyBatchCheckResponse, DesktopProxyChangeIpRequest,
  DesktopProxyChangeIpResult, DesktopProxyHealth, DesktopProxyPage,
  DesktopProxyPageQuery, DesktopProxyUsageItem, DesktopRecorderSnapshot,
  DesktopReadRunDetailQuery, DesktopRunDetail,
  DesktopRecorderSnapshotQuery, DesktopRuntimeSettingsDraft,
  DesktopSettingsMutationResult, DesktopSettingsSnapshot,
  DesktopStartBehaviorRecordingRequest, DesktopStatusSnapshot,
  DesktopStopBehaviorRecordingRequest, DesktopAppendBehaviorRecordingStepRequest,
  DesktopSyncLayoutState,
  DesktopSyncLayoutUpdate, DesktopSyncWindowBounds, DesktopSyncWindowState,
  DesktopSynchronizerActionResult, DesktopSynchronizerBroadcastRequest,
  DesktopSynchronizerSnapshot,
  DesktopTaskPage, DesktopTaskQuery, DesktopTaskWriteResult, DesktopTemplateDeleteInput,
  DesktopTemplateMetadataPage, DesktopTemplateMetadataPageQuery,
  DesktopTemplateMutationResult, DesktopTemplateUpsertInput,
  DesktopUpdateProfileInput,
};
use tauri::State;

#[cfg(target_os = "windows")]
use windows::{
  Win32::{
    Foundation::{BOOL, HWND, LPARAM, RECT},
    UI::WindowsAndMessaging::{
      BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowTextLengthW,
      GetWindowTextW, IsIconic, IsWindow, IsWindowVisible, SetForegroundWindow, ShowWindow,
      SW_RESTORE,
    },
  },
};

use crate::state::{DesktopState, ManagedRuntimeProcess};

const CREATE_NO_WINDOW: u32 = 0x08000000;
const LOCAL_RUNTIME_HEALTH_URL: &str = "http://127.0.0.1:3000/health";

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
  let url = format!("{}/tasks/{}/cancel", snapshot.base_url.trim_end_matches('/'), task_id);
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
  let release_binary = project_root.join("target").join("release").join("PersonaPilot.exe");
  if release_binary.exists() {
    return Ok(release_binary);
  }

  let debug_binary = project_root.join("target").join("debug").join("PersonaPilot.exe");
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
        return Ok(status_from_managed_process(process, api_reachable, last_exit_code));
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
    return Err("No visible desktop windows were detected for synchronizer snapshot.".to_string());
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
    .filter(|main_id| context.windows.iter().any(|window| &window.window_id == *main_id))
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
    "grid" | "overlap" | "uniform_size" => Ok(normalized),
    _ => Err(format!(
      "unsupported sync layout mode: {mode}. expected one of: grid, overlap, uniform_size"
    )),
  }
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

fn normalize_sync_broadcast_channel(channel: &str) -> Result<String, String> {
  let normalized = channel.trim().to_lowercase();
  if normalized.is_empty() {
    return Err("broadcast channel cannot be empty".to_string());
  }

  match normalized.as_str() {
    "scroll" | "navigation" | "input" => Ok(normalized),
    _ => Err(format!(
      "unsupported broadcast channel: {channel}. expected one of: scroll, navigation, input"
    )),
  }
}

fn normalize_sync_target_window_ids(window_ids: Option<Vec<String>>) -> Vec<String> {
  let mut unique = HashSet::new();
  let mut normalized = Vec::new();

  for window_id in window_ids.unwrap_or_default() {
    let trimmed = window_id.trim();
    if trimmed.is_empty() {
      continue;
    }

    let owned = trimmed.to_string();
    if unique.insert(owned.clone()) {
      normalized.push(owned);
    }
  }

  normalized
}

fn describe_sync_window_inventory(snapshot: &DesktopSynchronizerSnapshot) -> String {
  let mut ids = snapshot
    .windows
    .iter()
    .map(|window| window.window_id.as_str())
    .collect::<Vec<_>>();
  ids.sort_unstable();
  format!("[{}]", ids.join(", "))
}

fn ensure_sync_windows_exist(
  snapshot: &DesktopSynchronizerSnapshot,
  window_ids: &[String],
  role: &str,
) -> Result<(), String> {
  let missing = window_ids
    .iter()
    .filter(|window_id| {
      !snapshot
        .windows
        .iter()
        .any(|window| &window.window_id == *window_id)
    })
    .cloned()
    .collect::<Vec<_>>();
  if missing.is_empty() {
    return Ok(());
  }

  let noun = if missing.len() > 1 {
    format!("{role} sync windows")
  } else {
    format!("{role} sync window")
  };
  Err(format!(
    "{noun} not found: {}; available sync windows: {}",
    missing.join(", "),
    describe_sync_window_inventory(snapshot)
  ))
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
  load_desktop_tasks(&state.db, query).await.map_err(normalize_error)
}

#[tauri::command]
pub async fn list_log_page(
  state: State<'_, DesktopState>,
  query: DesktopLogQuery,
) -> Result<DesktopLogPage, String> {
  load_desktop_logs(&state.db, query).await.map_err(normalize_error)
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
  Ok(read_desktop_browser_environment_policy(Some(&state.database_url)))
}

#[tauri::command]
pub fn apply_browser_environment_policy(
  state: State<'_, DesktopState>,
  draft: DesktopBrowserEnvironmentPolicyDraft,
) -> Result<DesktopBrowserEnvironmentPolicyMutationResult, String> {
  apply_desktop_browser_environment_policy(&state.database_url, draft)
    .map_err(normalize_error)
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
  Ok(read_desktop_local_asset_workspace(Some(&state.database_url)))
}

#[tauri::command]
pub fn read_import_export_skeleton(
  state: State<'_, DesktopState>,
) -> Result<DesktopImportExportSkeleton, String> {
  Ok(read_desktop_import_export_skeleton(Some(&state.database_url)))
}

#[tauri::command]
pub fn open_local_asset_entry(
  state: State<'_, DesktopState>,
  entry_id: String,
) -> Result<(), String> {
  let (path, select_file) = resolve_desktop_local_asset_entry_path(&state.database_url, &entry_id)
    .map_err(normalize_error)?;

  if select_file {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).map_err(|error| {
        format!("Failed to prepare asset parent {}: {error}", parent.display())
      })?;
    }
  } else {
    fs::create_dir_all(&path)
      .map_err(|error| format!("Failed to prepare asset directory {}: {error}", path.display()))?;
  }

  open_path_in_explorer(&path, select_file)
}

#[tauri::command]
pub fn open_local_directory(
  state: State<'_, DesktopState>,
  target: String,
) -> Result<(), String> {
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
pub fn start_local_runtime(
  state: State<'_, DesktopState>,
) -> Result<DesktopRuntimeStatus, String> {
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

  fs::create_dir_all(&log_dir)
    .map_err(|error| format!("Failed to create runtime log directory {}: {error}", log_dir.display()))?;

  let stdout_file = OpenOptions::new()
    .create(true)
    .append(true)
    .open(&stdout_path)
    .map_err(|error| format!("Failed to open runtime stdout log {}: {error}", stdout_path.display()))?;
  let stderr_file = OpenOptions::new()
    .create(true)
    .append(true)
    .open(&stderr_path)
    .map_err(|error| format!("Failed to open runtime stderr log {}: {error}", stderr_path.display()))?;

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

  let child = command
    .spawn()
    .map_err(|error| format!("Failed to start local runtime {}: {error}", binary_path.display()))?;
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
pub fn stop_local_runtime(
  state: State<'_, DesktopState>,
) -> Result<DesktopRuntimeStatus, String> {
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

  process
    .child
    .kill()
    .map_err(|error| format!("Failed to stop local runtime process {}: {error}", process.pid))?;
  let status = process
    .child
    .wait()
    .map_err(|error| format!("Failed to wait for local runtime process {}: {error}", process.pid))?;
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

  recorder.snapshot.steps.push(persona_pilot::desktop::DesktopRecorderStep {
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
  if !snapshot.windows.iter().any(|window| window.window_id == window_id) {
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
  Ok(sync_action_result(
    "set_main_sync_window",
    snapshot,
    &format!("Set main sync window to {window_id}."),
  ))
}

#[tauri::command]
pub fn apply_window_layout(
  state: State<'_, DesktopState>,
  layout: DesktopSyncLayoutUpdate,
) -> Result<DesktopSynchronizerActionResult, String> {
  let mut synchronizer = state
    .synchronizer
    .lock()
    .map_err(|_| "Failed to lock synchronizer state".to_string())?;
  let mut snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;
  let changed = apply_sync_layout_update(&mut snapshot, layout)?;
  let now = now_ts_string();
  snapshot.layout.updated_at = now.clone();
  snapshot.updated_at = now;

  synchronizer.snapshot = snapshot.clone();
  Ok(sync_action_result(
    "apply_window_layout",
    snapshot,
    if changed {
      "Updated synchronizer layout state through native contract. Physical desktop window repositioning is not implemented yet."
    } else {
      "Layout update request produced no state delta. Physical desktop window repositioning is not implemented yet."
    },
  ))
}

#[tauri::command]
pub fn broadcast_sync_action(
  state: State<'_, DesktopState>,
  request: DesktopSynchronizerBroadcastRequest,
) -> Result<DesktopSynchronizerActionResult, String> {
  let mut synchronizer = state
    .synchronizer
    .lock()
    .map_err(|_| "Failed to lock synchronizer state".to_string())?;
  let mut snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;

  let channel = normalize_sync_broadcast_channel(&request.channel)?;
  let source_window_id = request.source_window_id.and_then(|window_id| {
    let trimmed = window_id.trim().to_string();
    if trimmed.is_empty() {
      None
    } else {
      Some(trimmed)
    }
  });

  if let Some(source_id) = source_window_id.as_ref() {
    ensure_sync_windows_exist(&snapshot, std::slice::from_ref(source_id), "source")?;
  }

  let explicit_targets = normalize_sync_target_window_ids(request.target_window_ids);
  if !explicit_targets.is_empty() {
    ensure_sync_windows_exist(&snapshot, explicit_targets.as_slice(), "target")?;
  }

  let target_window_ids = if explicit_targets.is_empty() {
    snapshot
      .windows
      .iter()
      .filter(|window| {
        source_window_id
          .as_ref()
          .map(|source_id| &window.window_id != source_id)
          .unwrap_or(true)
      })
      .map(|window| window.window_id.clone())
      .collect::<Vec<_>>()
  } else {
    explicit_targets
  };

  if target_window_ids.is_empty() {
    return Err(
      "broadcast request produced no target windows; provide explicit targets or a valid source."
        .to_string(),
    );
  }

  let mut intent_applied = false;
  match channel.as_str() {
    "scroll" => {
      if !snapshot.layout.sync_scroll {
        snapshot.layout.sync_scroll = true;
        intent_applied = true;
      }
    }
    "navigation" => {
      if !snapshot.layout.sync_navigation {
        snapshot.layout.sync_navigation = true;
        intent_applied = true;
      }
    }
    "input" => {
      if !snapshot.layout.sync_input {
        snapshot.layout.sync_input = true;
        intent_applied = true;
      }
    }
    _ => {}
  }

  let now = now_ts_string();
  let target_set = target_window_ids.iter().cloned().collect::<HashSet<_>>();
  for window in &mut snapshot.windows {
    let is_source = source_window_id
      .as_ref()
      .map(|source_id| &window.window_id == source_id)
      .unwrap_or(false);
    if is_source || target_set.contains(&window.window_id) {
      window.last_action_at = Some(now.clone());
    }
  }

  snapshot.layout.updated_at = now.clone();
  snapshot.updated_at = now;

  let source_hint = source_window_id
    .as_ref()
    .map(|source_id| format!(" source={source_id};"))
    .unwrap_or_default();
  let intent_hint = request
    .intent_label
    .as_ref()
    .map(|intent| intent.trim().to_string())
    .filter(|intent| !intent.is_empty())
    .map(|intent| format!(" intent={intent};"))
    .unwrap_or_default();

  synchronizer.snapshot = snapshot.clone();
  Ok(sync_action_result(
    "broadcast_sync_action",
    snapshot,
    &format!(
      "Recorded native broadcast intent channel={channel}; targets={};layoutFlagUpdated={intent_applied};{}{} physical multi-window dispatch is not implemented yet.",
      target_set.len(),
      source_hint,
      intent_hint
    ),
  ))
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
  let mut snapshot = capture_live_synchronizer_snapshot(&synchronizer.snapshot)?;
  let now = now_ts_string();
  for window in &mut snapshot.windows {
    if window.window_id == window_id {
      window.last_action_at = Some(now.clone());
      break;
    }
  }
  snapshot.layout.updated_at = now.clone();
  snapshot.updated_at = now;
  synchronizer.snapshot = snapshot.clone();
  Ok(sync_action_result(
    "focus_sync_window",
    snapshot,
    &format!("Focused sync window {window_id}."),
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn test_sync_window(window_id: &str, order_index: i64) -> DesktopSyncWindowState {
    DesktopSyncWindowState {
      window_id: window_id.to_string(),
      native_handle: Some(window_id.to_string()),
      title: Some(format!("window-{window_id}")),
      status: "ready".to_string(),
      order_index,
      is_main_window: false,
      is_focused: false,
      is_minimized: false,
      is_visible: true,
      profile_id: None,
      profile_label: None,
      store_id: None,
      platform_id: None,
      last_seen_at: Some("0".to_string()),
      last_action_at: None,
      bounds: None,
    }
  }

  fn test_sync_snapshot(window_ids: &[&str]) -> DesktopSynchronizerSnapshot {
    DesktopSynchronizerSnapshot {
      windows: window_ids
        .iter()
        .enumerate()
        .map(|(index, window_id)| test_sync_window(window_id, index as i64))
        .collect(),
      layout: DesktopSyncLayoutState {
        mode: "grid".to_string(),
        main_window_id: None,
        columns: None,
        rows: None,
        gap_px: 12,
        overlap_offset_x: None,
        overlap_offset_y: None,
        uniform_width: None,
        uniform_height: None,
        sync_scroll: false,
        sync_navigation: false,
        sync_input: false,
        updated_at: "0".to_string(),
      },
      focused_window_id: None,
      updated_at: "0".to_string(),
    }
  }

  #[test]
  fn normalize_sync_broadcast_channel_accepts_supported_values() {
    assert_eq!(
      normalize_sync_broadcast_channel(" Navigation ").expect("channel should normalize"),
      "navigation"
    );
    assert_eq!(
      normalize_sync_broadcast_channel("scroll").expect("channel should normalize"),
      "scroll"
    );
  }

  #[test]
  fn normalize_sync_broadcast_channel_rejects_invalid_values() {
    let empty_error =
      normalize_sync_broadcast_channel("  ").expect_err("empty channel must be rejected");
    assert!(empty_error.contains("cannot be empty"));

    let unsupported_error =
      normalize_sync_broadcast_channel("clipboard").expect_err("unsupported channel must fail");
    assert!(unsupported_error.contains("unsupported broadcast channel"));
  }

  #[test]
  fn normalize_sync_target_window_ids_trims_and_deduplicates() {
    let normalized = normalize_sync_target_window_ids(Some(vec![
      "1001".to_string(),
      " 1002 ".to_string(),
      "1001".to_string(),
      "   ".to_string(),
    ]));
    assert_eq!(normalized, vec!["1001".to_string(), "1002".to_string()]);
  }

  #[test]
  fn ensure_sync_windows_exist_reports_available_inventory() {
    let snapshot = test_sync_snapshot(&["1001", "1002"]);
    let missing = vec!["1003".to_string(), "1004".to_string()];
    let error = ensure_sync_windows_exist(&snapshot, &missing, "target")
      .expect_err("missing target windows must fail");
    assert!(error.contains("target sync windows not found: 1003, 1004"));
    assert!(error.contains("available sync windows: [1001, 1002]"));
  }
}
