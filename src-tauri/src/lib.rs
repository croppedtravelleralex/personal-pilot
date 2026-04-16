mod commands;
mod state;

use std::io::Error;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            let desktop_state = state::build_desktop_state().map_err(|error| {
                Error::other(format!("failed to initialize desktop state: {error:#}"))
            })?;
            app.manage(desktop_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_status,
            commands::list_task_page,
            commands::list_log_page,
            commands::read_local_settings,
            commands::apply_runtime_settings,
            commands::restore_runtime_settings_defaults,
            commands::read_local_api_snapshot,
            commands::apply_local_api_settings,
            commands::restore_local_api_defaults,
            commands::read_browser_environment_policy,
            commands::apply_browser_environment_policy,
            commands::restore_browser_environment_policy_defaults,
            commands::read_local_asset_workspace,
            commands::read_import_export_skeleton,
            commands::open_local_asset_entry,
            commands::open_local_directory,
            commands::read_local_runtime_status,
            commands::start_local_runtime,
            commands::stop_local_runtime,
            commands::list_profile_page,
            commands::read_profile_detail,
            commands::create_profile,
            commands::update_profile,
            commands::start_profiles,
            commands::stop_profiles,
            commands::open_profiles,
            commands::check_profile_proxies,
            commands::sync_profiles,
            commands::list_proxy_page,
            commands::read_proxy_health,
            commands::read_proxy_usage,
            commands::check_proxy_batch,
            commands::change_proxy_ip,
            commands::list_template_metadata_page,
            commands::save_template,
            commands::update_template,
            commands::delete_template,
            commands::compile_template_run,
            commands::launch_template_run,
            commands::read_run_detail,
            commands::retry_task,
            commands::cancel_task,
            commands::confirm_manual_gate,
            commands::reject_manual_gate,
            commands::read_recorder_snapshot,
            commands::start_behavior_recording,
            commands::stop_behavior_recording,
            commands::append_behavior_recording_step,
            commands::list_sync_windows,
            commands::read_sync_layout_state,
            commands::read_synchronizer_snapshot,
            commands::set_main_sync_window,
            commands::apply_window_layout,
            commands::focus_sync_window,
            commands::broadcast_sync_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
