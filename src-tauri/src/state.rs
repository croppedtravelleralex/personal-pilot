use std::{
  process::Child,
  sync::Mutex,
  time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use persona_pilot::{
  db::init::{init_db, DbPool},
  desktop::{
    default_database_url, DesktopRecorderSnapshot, DesktopRecorderTabSnapshot,
    DesktopSyncLayoutState, DesktopSyncWindowBounds, DesktopSyncWindowState,
    DesktopSynchronizerSnapshot,
  },
  runner::{fake::FakeRunner, lightpanda::LightpandaRunner, RunnerKind, TaskRunner},
};
use std::sync::Arc;

pub struct ManagedRuntimeProcess {
  pub child: Child,
  pub pid: u32,
  pub started_at: String,
  pub binary_path: String,
  pub log_dir: String,
  pub stdout_path: String,
  pub stderr_path: String,
}

pub struct RuntimeControllerState {
  pub managed_process: Option<ManagedRuntimeProcess>,
  pub last_exit_code: Option<i32>,
}

pub struct RecorderControllerState {
  pub snapshot: DesktopRecorderSnapshot,
}

pub struct SynchronizerControllerState {
  pub snapshot: DesktopSynchronizerSnapshot,
}

pub struct DesktopState {
  pub db: DbPool,
  pub database_url: String,
  #[allow(dead_code)]
  pub runner: Arc<dyn TaskRunner>,
  pub runtime: Mutex<RuntimeControllerState>,
  pub recorder: Mutex<RecorderControllerState>,
  pub synchronizer: Mutex<SynchronizerControllerState>,
}

fn now_ts_string() -> String {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_secs().to_string())
    .unwrap_or_else(|_| "0".to_string())
}

fn default_recorder_snapshot() -> DesktopRecorderSnapshot {
  let now = now_ts_string();
  DesktopRecorderSnapshot {
    session_id: "recorder-idle".to_string(),
    status: "idle".to_string(),
    profile_id: None,
    platform_id: None,
    template_id: None,
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
    tabs: vec![DesktopRecorderTabSnapshot {
      tab_id: "tab-home".to_string(),
      title: Some("Recorder Idle".to_string()),
      url: Some("about:blank".to_string()),
      active: true,
    }],
    steps: Vec::new(),
  }
}

fn default_sync_windows() -> Vec<DesktopSyncWindowState> {
  vec![
    DesktopSyncWindowState {
      window_id: "desktop-shell-main".to_string(),
      native_handle: Some("main".to_string()),
      title: Some("PersonaPilot Main".to_string()),
      status: "focused".to_string(),
      order_index: 0,
      is_main_window: true,
      is_focused: true,
      is_minimized: false,
      is_visible: true,
      profile_id: None,
      profile_label: None,
      store_id: None,
      platform_id: None,
      last_seen_at: Some("0".to_string()),
      last_action_at: Some("0".to_string()),
      bounds: Some(DesktopSyncWindowBounds {
        x: 32,
        y: 40,
        width: 1200,
        height: 820,
      }),
    },
    DesktopSyncWindowState {
      window_id: "sync-window-a".to_string(),
      native_handle: None,
      title: Some("Sync Candidate A".to_string()),
      status: "ready".to_string(),
      order_index: 1,
      is_main_window: false,
      is_focused: false,
      is_minimized: false,
      is_visible: true,
      profile_id: None,
      profile_label: None,
      store_id: None,
      platform_id: None,
      last_seen_at: Some("0".to_string()),
      last_action_at: Some("0".to_string()),
      bounds: Some(DesktopSyncWindowBounds {
        x: 1256,
        y: 40,
        width: 960,
        height: 640,
      }),
    },
    DesktopSyncWindowState {
      window_id: "sync-window-b".to_string(),
      native_handle: None,
      title: Some("Sync Candidate B".to_string()),
      status: "ready".to_string(),
      order_index: 2,
      is_main_window: false,
      is_focused: false,
      is_minimized: false,
      is_visible: true,
      profile_id: None,
      profile_label: None,
      store_id: None,
      platform_id: None,
      last_seen_at: Some("0".to_string()),
      last_action_at: Some("0".to_string()),
      bounds: Some(DesktopSyncWindowBounds {
        x: 1256,
        y: 696,
        width: 960,
        height: 640,
      }),
    },
  ]
}

fn default_synchronizer_snapshot() -> DesktopSynchronizerSnapshot {
  let now = now_ts_string();
  DesktopSynchronizerSnapshot {
    windows: default_sync_windows(),
    layout: DesktopSyncLayoutState {
      mode: "grid".to_string(),
      main_window_id: Some("desktop-shell-main".to_string()),
      columns: Some(2),
      rows: Some(2),
      gap_px: 16,
      overlap_offset_x: None,
      overlap_offset_y: None,
      uniform_width: Some(960),
      uniform_height: Some(640),
      sync_scroll: false,
      sync_navigation: false,
      sync_input: false,
      updated_at: now.clone(),
    },
    focused_window_id: Some("desktop-shell-main".to_string()),
    updated_at: now,
  }
}

pub fn build_desktop_state() -> Result<DesktopState> {
  let database_url = default_database_url();
  let db = tauri::async_runtime::block_on(init_db(&database_url))?;
  let runner: Arc<dyn TaskRunner> = match RunnerKind::from_env() {
    RunnerKind::Fake => Arc::new(FakeRunner),
    RunnerKind::Lightpanda => Arc::new(LightpandaRunner::default()),
  };

  Ok(DesktopState {
    db,
    database_url,
    runner,
    runtime: Mutex::new(RuntimeControllerState {
      managed_process: None,
      last_exit_code: None,
    }),
    recorder: Mutex::new(RecorderControllerState {
      snapshot: default_recorder_snapshot(),
    }),
    synchronizer: Mutex::new(SynchronizerControllerState {
      snapshot: default_synchronizer_snapshot(),
    }),
  })
}
