import { invoke } from "@tauri-apps/api/core";

import type {
  DesktopBrowserEnvironmentPolicyDraft,
  DesktopBrowserEnvironmentPolicyMutationResult,
  DesktopBrowserEnvironmentPolicySnapshot,
  DesktopCompileTemplateRunRequest,
  DesktopCompileTemplateRunResult,
  DesktopCreateProfileInput,
  DesktopLaunchTemplateRunRequest,
  DesktopLaunchTemplateRunResult,
  DesktopDirectoryTarget,
  DesktopImportExportSkeleton,
  DesktopLocalApiMutationResult,
  DesktopLocalApiSettingsDraft,
  DesktopLocalApiSnapshot,
  DesktopLocalAssetEntryId,
  DesktopLocalAssetWorkspaceSnapshot,
  DesktopLogPage,
  DesktopLogQuery,
  DesktopProfileBatchActionRequest,
  DesktopProfileBatchActionResult,
  DesktopProfileDetail,
  DesktopProfileMutationResult,
  DesktopProfilePage,
  DesktopProfilePageQuery,
  DesktopProxyBatchCheckRequest,
  DesktopProxyBatchCheckResponse,
  DesktopProxyChangeIpRequest,
  DesktopProxyChangeIpResult,
  DesktopProxyHealth,
  DesktopProxyPage,
  DesktopProxyPageQuery,
  DesktopProxyUsageItem,
  DesktopReadRunDetailQuery,
  DesktopStartBehaviorRecordingRequest,
  DesktopStopBehaviorRecordingRequest,
  DesktopAppendBehaviorRecordingStepRequest,
  DesktopRecorderSnapshot,
  DesktopRecorderSnapshotQuery,
  DesktopRunDetail,
  DesktopRuntimeStatus,
  DesktopRuntimeSettingsDraft,
  DesktopSettingsMutationResult,
  DesktopSettingsSnapshot,
  DesktopSyncLayoutUpdate,
  DesktopSyncLayoutState,
  DesktopSyncWindowState,
  DesktopSynchronizerActionResult,
  DesktopSynchronizerSnapshot,
  DesktopStatusSnapshot,
  DesktopTaskWriteResult,
  DesktopTaskPage,
  DesktopTaskQuery,
  DesktopTemplateDeleteInput,
  DesktopTemplateMetadataPage,
  DesktopTemplateMetadataPageQuery,
  DesktopTemplateMutationResult,
  DesktopTemplateUpsertInput,
  DesktopUpdateProfileInput,
  DesktopManualGateActionRequest,
} from "../types/desktop";

export class DesktopServiceError extends Error {
  readonly code: string;
  readonly details: unknown;

  constructor(message: string, code = "desktop_error", details: unknown = null) {
    super(message);
    this.name = "DesktopServiceError";
    this.code = code;
    this.details = details;
  }
}

function normalizeDesktopError(error: unknown): DesktopServiceError {
  if (error instanceof DesktopServiceError) {
    return error;
  }

  if (typeof error === "string") {
    return new DesktopServiceError(error);
  }

  if (error instanceof Error) {
    return new DesktopServiceError(error.message, "desktop_error", {
      stack: error.stack,
    });
  }

  return new DesktopServiceError("Unknown desktop error", "desktop_error", error);
}

async function invokeTyped<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeDesktopError(error);
  }
}

function normalizeErrorText(value: unknown): string {
  if (typeof value === "string") {
    return value.toLowerCase();
  }

  if (value instanceof Error) {
    return `${value.message} ${value.stack ?? ""}`.toLowerCase();
  }

  if (value && typeof value === "object") {
    try {
      return JSON.stringify(value).toLowerCase();
    } catch {
      return String(value).toLowerCase();
    }
  }

  return String(value).toLowerCase();
}

function isMissingDesktopCommand(
  error: DesktopServiceError,
  command: string,
): boolean {
  const haystack = `${normalizeErrorText(error.message)} ${normalizeErrorText(error.details)}`;
  const normalizedCommand = command.toLowerCase();

  return (
    haystack.includes(normalizedCommand) &&
    (haystack.includes("not found") ||
      haystack.includes("unknown command") ||
      haystack.includes("unknown ipc") ||
      haystack.includes("not implemented"))
  );
}

function isNotReadyDesktopCommand(error: DesktopServiceError): boolean {
  const haystack = `${normalizeErrorText(error.message)} ${normalizeErrorText(error.details)}`;

  return (
    haystack.includes("todo:") ||
    haystack.includes("native contract is not implemented yet") ||
    haystack.includes("desktop command not ready") ||
    haystack.includes("desktop_command_not_ready")
  );
}

// Keep contract wrappers callable before Rust commands land by surfacing a stable
// "not ready" error instead of leaking raw Tauri missing-command text.
async function invokeDesktopContract<T>(
  command: string,
  contractName: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invokeTyped<T>(command, args);
  } catch (error) {
    const normalized = normalizeDesktopError(error);
    if (isMissingDesktopCommand(normalized, command) || isNotReadyDesktopCommand(normalized)) {
      throw new DesktopServiceError(
        `TODO: ${contractName} native contract is not implemented yet.`,
        "desktop_command_not_ready",
        { command, args },
      );
    }
    throw normalized;
  }
}

export function getAppStatus(): Promise<DesktopStatusSnapshot> {
  return invokeTyped<DesktopStatusSnapshot>("get_app_status");
}

export function listTaskPage(
  query: DesktopTaskQuery,
): Promise<DesktopTaskPage> {
  return invokeTyped<DesktopTaskPage>("list_task_page", { query });
}

export function listLogPage(
  query: DesktopLogQuery,
): Promise<DesktopLogPage> {
  return invokeTyped<DesktopLogPage>("list_log_page", { query });
}

export function readSettings(): Promise<DesktopSettingsSnapshot> {
  return invokeTyped<DesktopSettingsSnapshot>("read_local_settings");
}

export function applyRuntimeSettings(
  draft: DesktopRuntimeSettingsDraft,
): Promise<DesktopSettingsMutationResult> {
  return invokeDesktopContract<DesktopSettingsMutationResult>(
    "apply_runtime_settings",
    "applyRuntimeSettings",
    { draft },
  );
}

export function restoreRuntimeSettingsDefaults(): Promise<DesktopSettingsMutationResult> {
  return invokeDesktopContract<DesktopSettingsMutationResult>(
    "restore_runtime_settings_defaults",
    "restoreRuntimeSettingsDefaults",
  );
}

export function readLocalApiSnapshot(): Promise<DesktopLocalApiSnapshot> {
  return invokeDesktopContract<DesktopLocalApiSnapshot>(
    "read_local_api_snapshot",
    "readLocalApiSnapshot",
  );
}

export function applyLocalApiSettings(
  draft: DesktopLocalApiSettingsDraft,
): Promise<DesktopLocalApiMutationResult> {
  return invokeDesktopContract<DesktopLocalApiMutationResult>(
    "apply_local_api_settings",
    "applyLocalApiSettings",
    { draft },
  );
}

export function restoreLocalApiDefaults(): Promise<DesktopLocalApiMutationResult> {
  return invokeDesktopContract<DesktopLocalApiMutationResult>(
    "restore_local_api_defaults",
    "restoreLocalApiDefaults",
  );
}

export function readBrowserEnvironmentPolicy(): Promise<DesktopBrowserEnvironmentPolicySnapshot> {
  return invokeDesktopContract<DesktopBrowserEnvironmentPolicySnapshot>(
    "read_browser_environment_policy",
    "readBrowserEnvironmentPolicy",
  );
}

export function applyBrowserEnvironmentPolicy(
  draft: DesktopBrowserEnvironmentPolicyDraft,
): Promise<DesktopBrowserEnvironmentPolicyMutationResult> {
  return invokeDesktopContract<DesktopBrowserEnvironmentPolicyMutationResult>(
    "apply_browser_environment_policy",
    "applyBrowserEnvironmentPolicy",
    { draft },
  );
}

export function restoreBrowserEnvironmentPolicyDefaults(): Promise<DesktopBrowserEnvironmentPolicyMutationResult> {
  return invokeDesktopContract<DesktopBrowserEnvironmentPolicyMutationResult>(
    "restore_browser_environment_policy_defaults",
    "restoreBrowserEnvironmentPolicyDefaults",
  );
}

export function readLocalAssetWorkspace(): Promise<DesktopLocalAssetWorkspaceSnapshot> {
  return invokeDesktopContract<DesktopLocalAssetWorkspaceSnapshot>(
    "read_local_asset_workspace",
    "readLocalAssetWorkspace",
  );
}

export function readImportExportSkeleton(): Promise<DesktopImportExportSkeleton> {
  return invokeDesktopContract<DesktopImportExportSkeleton>(
    "read_import_export_skeleton",
    "readImportExportSkeleton",
  );
}

export function openLocalAssetEntry(
  entryId: DesktopLocalAssetEntryId,
): Promise<void> {
  return invokeDesktopContract<void>("open_local_asset_entry", "openLocalAssetEntry", {
    entry_id: entryId,
  });
}

export function openLocalDirectory(
  target: DesktopDirectoryTarget,
): Promise<void> {
  return invokeTyped<void>("open_local_directory", { target });
}

export function readLocalRuntimeStatus(): Promise<DesktopRuntimeStatus> {
  return invokeTyped<DesktopRuntimeStatus>("read_local_runtime_status");
}

export function startLocalRuntime(): Promise<DesktopRuntimeStatus> {
  return invokeTyped<DesktopRuntimeStatus>("start_local_runtime");
}

export function stopLocalRuntime(): Promise<DesktopRuntimeStatus> {
  return invokeTyped<DesktopRuntimeStatus>("stop_local_runtime");
}

export function listProfilePage(
  query: DesktopProfilePageQuery,
): Promise<DesktopProfilePage> {
  return invokeDesktopContract<DesktopProfilePage>(
    "list_profile_page",
    "listProfilePage",
    { query },
  );
}

export function readProfileDetail(
  profileId: string,
): Promise<DesktopProfileDetail> {
  return invokeDesktopContract<DesktopProfileDetail>(
    "read_profile_detail",
    "readProfileDetail",
    { profile_id: profileId },
  );
}

export function createProfile(
  input: DesktopCreateProfileInput,
): Promise<DesktopProfileMutationResult> {
  return invokeDesktopContract<DesktopProfileMutationResult>(
    "create_profile",
    "createProfile",
    { input },
  );
}

export function updateProfile(
  input: DesktopUpdateProfileInput,
): Promise<DesktopProfileMutationResult> {
  return invokeDesktopContract<DesktopProfileMutationResult>(
    "update_profile",
    "updateProfile",
    { input },
  );
}

export function startProfiles(
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> {
  return invokeDesktopContract<DesktopProfileBatchActionResult>(
    "start_profiles",
    "startProfiles",
    { request },
  );
}

export function stopProfiles(
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> {
  return invokeDesktopContract<DesktopProfileBatchActionResult>(
    "stop_profiles",
    "stopProfiles",
    { request },
  );
}

export function openProfiles(
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> {
  return invokeDesktopContract<DesktopProfileBatchActionResult>(
    "open_profiles",
    "openProfiles",
    { request },
  );
}

export function checkProfileProxies(
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> {
  return invokeDesktopContract<DesktopProfileBatchActionResult>(
    "check_profile_proxies",
    "checkProfileProxies",
    { request },
  );
}

export function syncProfiles(
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> {
  return invokeDesktopContract<DesktopProfileBatchActionResult>(
    "sync_profiles",
    "syncProfiles",
    { request },
  );
}

export function listProxyPage(
  query: DesktopProxyPageQuery,
): Promise<DesktopProxyPage> {
  return invokeDesktopContract<DesktopProxyPage>(
    "list_proxy_page",
    "listProxyPage",
    { query },
  );
}

export function readProxyHealth(proxyId: string): Promise<DesktopProxyHealth> {
  return invokeDesktopContract<DesktopProxyHealth>(
    "read_proxy_health",
    "readProxyHealth",
    { proxy_id: proxyId },
  );
}

export function readProxyUsage(
  proxyId: string,
): Promise<DesktopProxyUsageItem[]> {
  return invokeDesktopContract<DesktopProxyUsageItem[]>(
    "read_proxy_usage",
    "readProxyUsage",
    { proxy_id: proxyId },
  );
}

export function checkProxyBatch(
  request: DesktopProxyBatchCheckRequest,
): Promise<DesktopProxyBatchCheckResponse> {
  return invokeDesktopContract<DesktopProxyBatchCheckResponse>(
    "check_proxy_batch",
    "checkProxyBatch",
    { request },
  );
}

export function changeProxyIp(
  request: DesktopProxyChangeIpRequest,
): Promise<DesktopProxyChangeIpResult> {
  return invokeDesktopContract<DesktopProxyChangeIpResult>(
    "change_proxy_ip",
    "changeProxyIp",
    { request },
  );
}

export function listTemplateMetadataPage(
  query: DesktopTemplateMetadataPageQuery,
): Promise<DesktopTemplateMetadataPage> {
  return invokeDesktopContract<DesktopTemplateMetadataPage>(
    "list_template_metadata_page",
    "listTemplateMetadataPage",
    { query },
  );
}

export function saveTemplate(
  input: DesktopTemplateUpsertInput,
): Promise<DesktopTemplateMutationResult> {
  return invokeDesktopContract<DesktopTemplateMutationResult>(
    "save_template",
    "saveTemplate",
    { input },
  );
}

export function updateTemplate(
  input: DesktopTemplateUpsertInput,
): Promise<DesktopTemplateMutationResult> {
  return invokeDesktopContract<DesktopTemplateMutationResult>(
    "update_template",
    "updateTemplate",
    { input },
  );
}

export function deleteTemplate(
  input: DesktopTemplateDeleteInput,
): Promise<DesktopTemplateMutationResult> {
  return invokeDesktopContract<DesktopTemplateMutationResult>(
    "delete_template",
    "deleteTemplate",
    { input },
  );
}

export function compileTemplateRun(
  request: DesktopCompileTemplateRunRequest,
): Promise<DesktopCompileTemplateRunResult> {
  return invokeDesktopContract<DesktopCompileTemplateRunResult>(
    "compile_template_run",
    "compileTemplateRun",
    { request },
  );
}

export function launchTemplateRun(
  request: DesktopLaunchTemplateRunRequest,
): Promise<DesktopLaunchTemplateRunResult> {
  return invokeDesktopContract<DesktopLaunchTemplateRunResult>(
    "launch_template_run",
    "launchTemplateRun",
    { request },
  );
}

export function readRunDetail(
  query: DesktopReadRunDetailQuery,
): Promise<DesktopRunDetail> {
  return invokeDesktopContract<DesktopRunDetail>(
    "read_run_detail",
    "readRunDetail",
    { query },
  );
}

export function retryTask(taskId: string): Promise<DesktopTaskWriteResult> {
  return invokeDesktopContract<DesktopTaskWriteResult>(
    "retry_task",
    "retryTask",
    { task_id: taskId },
  );
}

export function cancelTask(taskId: string): Promise<DesktopTaskWriteResult> {
  return invokeDesktopContract<DesktopTaskWriteResult>(
    "cancel_task",
    "cancelTask",
    { task_id: taskId },
  );
}

export function confirmManualGate(
  request: DesktopManualGateActionRequest,
): Promise<DesktopTaskWriteResult> {
  return invokeDesktopContract<DesktopTaskWriteResult>(
    "confirm_manual_gate",
    "confirmManualGate",
    { request },
  );
}

export function rejectManualGate(
  request: DesktopManualGateActionRequest,
): Promise<DesktopTaskWriteResult> {
  return invokeDesktopContract<DesktopTaskWriteResult>(
    "reject_manual_gate",
    "rejectManualGate",
    { request },
  );
}

export function readRecorderSnapshot(
  query: DesktopRecorderSnapshotQuery = {},
): Promise<DesktopRecorderSnapshot> {
  return invokeDesktopContract<DesktopRecorderSnapshot>(
    "read_recorder_snapshot",
    "readRecorderSnapshot",
    { query },
  );
}

export function startBehaviorRecording(
  request: DesktopStartBehaviorRecordingRequest,
): Promise<DesktopRecorderSnapshot> {
  return invokeDesktopContract<DesktopRecorderSnapshot>(
    "start_behavior_recording",
    "startBehaviorRecording",
    { request },
  );
}

export function stopBehaviorRecording(
  request: DesktopStopBehaviorRecordingRequest = {},
): Promise<DesktopRecorderSnapshot> {
  return invokeDesktopContract<DesktopRecorderSnapshot>(
    "stop_behavior_recording",
    "stopBehaviorRecording",
    { request },
  );
}

export function appendBehaviorRecordingStep(
  request: DesktopAppendBehaviorRecordingStepRequest,
): Promise<DesktopRecorderSnapshot> {
  return invokeDesktopContract<DesktopRecorderSnapshot>(
    "append_behavior_recording_step",
    "appendBehaviorRecordingStep",
    { request },
  );
}

export function listSyncWindows(): Promise<DesktopSyncWindowState[]> {
  return invokeDesktopContract<DesktopSyncWindowState[]>(
    "list_sync_windows",
    "listSyncWindows",
  );
}

export function readSyncLayoutState(): Promise<DesktopSyncLayoutState> {
  return invokeDesktopContract<DesktopSyncLayoutState>(
    "read_sync_layout_state",
    "readSyncLayoutState",
  );
}

export function readSynchronizerSnapshot(): Promise<DesktopSynchronizerSnapshot> {
  return invokeDesktopContract<DesktopSynchronizerSnapshot>(
    "read_synchronizer_snapshot",
    "readSynchronizerSnapshot",
  );
}

export function setMainSyncWindow(
  windowId: string,
): Promise<DesktopSynchronizerActionResult> {
  return invokeDesktopContract<DesktopSynchronizerActionResult>(
    "set_main_sync_window",
    "setMainSyncWindow",
    { window_id: windowId },
  );
}

export function applyWindowLayout(
  layout: DesktopSyncLayoutUpdate,
): Promise<DesktopSynchronizerActionResult> {
  return invokeDesktopContract<DesktopSynchronizerActionResult>(
    "apply_window_layout",
    "applyWindowLayout",
    { layout },
  );
}

export function focusSyncWindow(
  windowId: string,
): Promise<DesktopSynchronizerActionResult> {
  return invokeDesktopContract<DesktopSynchronizerActionResult>(
    "focus_sync_window",
    "focusSyncWindow",
    { window_id: windowId },
  );
}
