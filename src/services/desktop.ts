import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import type {
  DesktopAppendBehaviorRecordingStepRequest,
  DesktopBrowserEnvironmentPolicyDraft,
  DesktopBrowserEnvironmentPolicyMutationResult,
  DesktopBrowserEnvironmentPolicySnapshot,
  DesktopBehaviorAuditContract,
  DesktopCamoufoxCapability,
  DesktopCamoufoxCapabilityRequest,
  DesktopCamoufoxSettingsDraft,
  DesktopCamoufoxSettingsMutationResult,
  DesktopCamoufoxSettingsSnapshot,
  DesktopCompileTemplateRunRequest,
  DesktopCompileTemplateRunResult,
  DesktopCreateProfileInput,
  DesktopCoreSyncGroup,
  DesktopCoreSyncOperation,
  DesktopCoreSyncWindowPlacement,
  DesktopCoreWorkbenchTask,
  DesktopDirectoryTarget,
  DesktopImportExportSkeleton,
  DesktopJsonValue,
  DesktopSessionBundleExport,
  DesktopSessionBundleExportRequest,
  DesktopSessionBundleImportPreflight,
  DesktopSessionBundleImportPreflightRequest,
  DesktopSessionBundleRestoreRequest,
  DesktopSessionBundleRestoreResult,
  DesktopProviderProductionReadiness,
  DesktopEvidenceReportHistory,
  DesktopReleaseSmokeContract,
  DesktopLaunchTemplateRunRequest,
  DesktopLaunchTemplateRunResult,
  DesktopLocalApiMutationResult,
  DesktopLocalApiSettingsDraft,
  DesktopLocalApiSnapshot,
  DesktopLocalAssetEntryId,
  DesktopLocalAssetWorkspaceSnapshot,
  DesktopLogPage,
  DesktopLogQuery,
  DesktopManualGateActionRequest,
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
  DesktopRecorderSnapshot,
  DesktopRecorderSnapshotQuery,
  DesktopRuntimeSettingsDraft,
  DesktopRuntimeStatus,
  DesktopRunDetail,
  DesktopSettingsMutationResult,
  DesktopSettingsSnapshot,
  DesktopStartBehaviorRecordingRequest,
  DesktopStatusSnapshot,
  DesktopStopBehaviorRecordingRequest,
  DesktopSynchronizerActionResult,
  DesktopSynchronizerBroadcastRequest,
  DesktopSynchronizerSnapshot,
  DesktopSyncLayoutState,
  DesktopSyncLayoutUpdate,
  DesktopSyncWindowState,
  DesktopTaskPage,
  DesktopTaskQuery,
  DesktopTaskWriteResult,
  DesktopTemplateDeleteInput,
  DesktopTemplateMetadataPage,
  DesktopTemplateMetadataPageQuery,
  DesktopTemplateMutationResult,
  DesktopTemplateUpsertInput,
  DesktopUpdateProfileInput,
  DesktopValidationReport,
  DesktopValidationBrowserSignal,
  DesktopValidationReportSummary,
  DesktopValidationProfileExport,
} from "../types/desktop";

export type DesktopServiceErrorCode =
  | "desktop_command_not_ready"
  | "desktop_invoke_unavailable"
  | "desktop_command_failed"
  | string;

export class DesktopServiceError extends Error {
  readonly code: DesktopServiceErrorCode;
  readonly cause?: unknown;

  constructor(message: string, code: DesktopServiceErrorCode, cause?: unknown) {
    super(message);
    this.name = "DesktopServiceError";
    this.code = code;
    this.cause = cause;
  }
}

type DesktopInvoke = <T = unknown>(command: string, args?: InvokeArgs) => Promise<T>;
type InvokeArgs = Record<string, unknown>;
type Unlisten = () => void;

interface DesktopEnvironmentInfo {
  buildType: string;
  platform: string;
  arch: string;
}

interface DesktopCoreStartStatus {
  eventUrl?: string | null;
  bridgeUrl?: string | null;
  pid?: number | null;
  [key: string]: unknown;
}

export interface DesktopDashboardStatsResponse {
  totalInstances?: number;
  runningInstances?: number;
  proxyCount?: number;
  coreCount?: number;
  memUsedMB?: number;
  appVersion?: string;
}

export interface DesktopLicenseStatusResponse {
  maxLimit?: number;
}

export interface DesktopEventLogQueryInput {
  after: string;
  before: string;
  namespace: string;
  severity: string;
  eventName: string;
  limit: number;
  offset: number;
}

export interface DesktopEventLogEntry {
  id: number;
  eventName: string;
  namespace: string;
  severity: string;
  payload: Record<string, unknown>;
  createdAt: string;
}

interface TauriEventApi {
  listen?: <T = unknown>(
    eventName: string,
    callback: (event: { payload: T }) => void,
  ) => Promise<Unlisten>;
}

interface TauriDialogApi {
  open?: (options?: InvokeArgs) => Promise<string | string[] | null>;
  save?: (options?: InvokeArgs) => Promise<string | null>;
}

interface TauriAppApi {
  exit?: (exitCode?: number) => Promise<void> | void;
}

interface TauriWindowHandle {
  hide?: () => Promise<void> | void;
  show?: () => Promise<void> | void;
  minimize?: () => Promise<void> | void;
  unminimize?: () => Promise<void> | void;
}

interface TauriWindowApi {
  getCurrentWindow?: () => TauriWindowHandle;
}

interface WailsRuntimeShim {
  EventsOn?: <T = unknown>(
    eventName: string,
    callback: (payload: T) => void,
  ) => Unlisten;
  BrowserOpenURL?: (url: string) => void;
}

interface TauriGlobal {
  core?: {
    invoke?: DesktopInvoke;
  };
  invoke?: DesktopInvoke;
  event?: TauriEventApi;
  dialog?: TauriDialogApi;
  app?: TauriAppApi;
  window?: TauriWindowApi;
}

interface IpcMessage {
  cmd: string;
  callback: number;
  error: number;
  payload?: InvokeArgs;
  args?: InvokeArgs;
}

interface DesktopWindow extends Window {
  __TAURI__?: TauriGlobal;
  __TAURI_IPC__?: (message: IpcMessage) => void;
  __TAURI_INTERNALS__?: unknown;
  runtime?: WailsRuntimeShim;
}

const desktopWindow = window as DesktopWindow;

const rpcNameMap: Record<string, string> = {
  BackupExportPackageToPath: "backup_export_package_to_path",
  BackupImportPackagePreflightFromPath: "backup_import_package_preflight_from_path",
  BackupImportPackageFromPathConfirmed: "backup_import_package_from_path_confirmed",
  BackupInitializeSystemPreflight: "backup_initialize_system_preflight",
  BackupInitializeSystemConfirmed: "backup_initialize_system_confirmed",
  BrowserSnapshotRestorePreflight: "browser_snapshot_restore_preflight",
  BrowserSnapshotRestoreConfirmed: "browser_snapshot_restore_confirmed",
};

const commandArgNames: Record<string, string[]> = {
  apply_browser_environment_policy: ["draft"],
  apply_broadcast_plan: ["request"],
  apply_camoufox_settings: ["draft"],
  apply_local_api_settings: ["draft"],
  apply_runtime_settings: ["draft"],
  apply_window_layout: ["layout"],
  append_behavior_recording_step: ["request"],
  cancel_task: ["taskId"],
  change_proxy_ip: ["request"],
  check_profile_proxies: ["request"],
  check_camoufox_capability: ["request"],
  check_proxy_batch: ["request"],
  compile_template_run: ["request"],
  confirm_manual_gate: ["request"],
  create_profile: ["input"],
  delete_template: ["input"],
  focus_sync_window: ["windowId"],
  launch_template_run: ["request"],
  list_log_page: ["query"],
  list_profile_page: ["query"],
  list_proxy_page: ["query"],
  list_task_page: ["query"],
  list_template_metadata_page: ["query"],
  open_local_asset_entry: ["entryId"],
  open_local_directory: ["target"],
  open_profiles: ["request"],
  read_profile_detail: ["profileId"],
  read_proxy_health: ["proxyId"],
  read_proxy_usage: ["proxyId"],
  read_recorder_snapshot: ["query"],
  read_run_detail: ["query"],
  reject_manual_gate: ["request"],
  retry_task: ["taskId"],
  save_template: ["input"],
  set_main_sync_window: ["windowId"],
  start_behavior_recording: ["request"],
  start_profiles: ["request"],
  stop_behavior_recording: ["request"],
  stop_profiles: ["request"],
  sync_profiles: ["request"],
  update_profile: ["input"],
  update_template: ["input"],
};

function getInvoke(): DesktopInvoke {
  if (desktopWindow.__TAURI_INTERNALS__) {
    return tauriInvoke as DesktopInvoke;
  }

  const invoke = desktopWindow.__TAURI__?.core?.invoke ?? desktopWindow.__TAURI__?.invoke;
  if (invoke) {
    return invoke;
  }

  if (desktopWindow.__TAURI_IPC__) {
    return invokeThroughIpc;
  }

  throw new DesktopServiceError(
    "Desktop invoke is unavailable in this runtime.",
    "desktop_invoke_unavailable",
  );
}

function invokeThroughIpc<T = unknown>(command: string, args: InvokeArgs = {}): Promise<T> {
  const ipc = desktopWindow.__TAURI_IPC__;
  if (!ipc) {
    throw new DesktopServiceError(
      "Desktop IPC bridge is unavailable in this runtime.",
      "desktop_invoke_unavailable",
    );
  }

  return new Promise<T>((resolve, reject) => {
    const callbackId =
      window.crypto.getRandomValues(new Uint32Array(1))[0] ?? Date.now();
    const errorId =
      window.crypto.getRandomValues(new Uint32Array(1))[0] ?? Date.now() + 1;
    const windowRecord = window as unknown as Window & Record<string, unknown>;
    const callbackKey = `_${callbackId}`;
    const errorKey = `_${errorId}`;
    const cleanup = () => {
      delete windowRecord[callbackKey];
      delete windowRecord[errorKey];
    };

    windowRecord[callbackKey] = (value: unknown) => {
      cleanup();
      resolve(value as T);
    };
    windowRecord[errorKey] = (value: unknown) => {
      cleanup();
      reject(value);
    };

    ipc({
      cmd: command,
      callback: callbackId,
      error: errorId,
      payload: args,
      args,
    });
  });
}

function normalizeError(command: string, error: unknown): DesktopServiceError {
  if (error instanceof DesktopServiceError) {
    return error;
  }

  const message = error instanceof Error ? error.message : String(error);
  const lowerMessage = message.toLowerCase();
  const code =
    lowerMessage.includes("not found") ||
    lowerMessage.includes("unknown command") ||
    lowerMessage.includes("unknown method")
      ? "desktop_command_not_ready"
      : "desktop_command_failed";

  return new DesktopServiceError(
    `Desktop command ${command} failed: ${message}`,
    code,
    error,
  );
}

async function invokeDesktop<T>(command: string, args: InvokeArgs = {}): Promise<T> {
  try {
    const invoke = getInvoke();
    return (await invoke(command, args)) as T;
  } catch (error) {
    throw normalizeError(command, error);
  }
}

function pascalToSnake(name: string): string {
  return name
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1_$2")
    .replace(/[\s.-]+/g, "_")
    .toLowerCase();
}

function buildRpcArgs(command: string, args: unknown[]): InvokeArgs {
  const names = commandArgNames[command];
  if (names) {
    return Object.fromEntries(names.map((name, index) => [name, args[index]]));
  }

  if (args.length === 0) return {};
  if (args.length === 1) return { input: args[0] };
  return { args };
}

let coreStartPromise: Promise<DesktopCoreStartStatus> | null = null;

export function hasDesktopRuntime(): boolean {
  return Boolean(
    desktopWindow.__TAURI__ ||
      desktopWindow.__TAURI_IPC__ ||
      desktopWindow.__TAURI_INTERNALS__,
  );
}

export function desktopCoreStart(): Promise<DesktopCoreStartStatus> {
  if (!coreStartPromise) {
    coreStartPromise = invokeDesktop<DesktopCoreStartStatus>("start_personal_pilot_core").catch(
      (error) => {
        coreStartPromise = null;
        throw error;
      },
    );
  }
  return coreStartPromise;
}

export async function desktopRpc<T = unknown>(
  name: string,
  args: unknown[] = [],
  _options?: { timeoutMs?: number },
): Promise<T> {
  await desktopCoreStart();
  return invokeDesktop<T>("call_personal_pilot_core", {
    request: {
      name,
      args,
    },
  });
}

export function desktopListen<T = unknown>(
  eventName: string,
  callback: (payload: T) => void,
): Promise<Unlisten> {
  const listen = desktopWindow.__TAURI__?.event?.listen;
  if (!listen) {
    return Promise.reject(
      new DesktopServiceError(
        "Desktop event listener bridge is unavailable in this runtime.",
        "desktop_invoke_unavailable",
      ),
    );
  }

  return listen<T>(eventName, (event) => callback(event.payload));
}

export function desktopRuntimeListen<T = unknown>(
  eventName: string,
  callback: (payload: T) => void,
): Unlisten {
  const runtimeUnlisten = desktopWindow.runtime?.EventsOn?.(eventName, callback);
  if (runtimeUnlisten) {
    return runtimeUnlisten;
  }

  let disposed = false;
  let unlisten: Unlisten | null = null;
  void desktopListen<T>(eventName, callback)
    .then((nextUnlisten) => {
      if (disposed) {
        nextUnlisten();
        return;
      }
      unlisten = nextUnlisten;
    })
    .catch(() => {
      // Runtime event subscription is best-effort in non-desktop previews.
    });

  return () => {
    disposed = true;
    unlisten?.();
  };
}

export function desktopEnvironment(): Promise<DesktopEnvironmentInfo> {
  return Promise.resolve({
    buildType: window.location.hostname === "localhost" ? "development" : "production",
    platform: "windows",
    arch: "amd64",
  });
}

export function desktopQuit(): void {
  void desktopWindow.__TAURI__?.app?.exit?.(0);
}

export function desktopQuitAppOnly(): Promise<void> {
  return Promise.resolve(desktopWindow.__TAURI__?.app?.exit?.(0));
}

export function desktopQuitFull(): Promise<void> {
  return Promise.resolve(desktopWindow.__TAURI__?.app?.exit?.(0));
}

function currentWindow(): TauriWindowHandle | null {
  return desktopWindow.__TAURI__?.window?.getCurrentWindow?.() ?? null;
}

export function desktopWindowHide(): void {
  void currentWindow()?.hide?.();
}

export function desktopWindowShow(): void {
  void currentWindow()?.show?.();
}

export function desktopWindowMinimize(): void {
  void currentWindow()?.minimize?.();
}

export function desktopOpenExternalUrl(url: string): void {
  if (!url) return;
  const runtimeOpen = desktopWindow.runtime?.BrowserOpenURL;
  if (runtimeOpen) {
    runtimeOpen(url);
    return;
  }
  window.open(url, "_blank", "noopener,noreferrer");
}

export async function desktopSaveBackupPath(defaultPath: string): Promise<string | null> {
  const save = desktopWindow.__TAURI__?.dialog?.save;
  if (!save) {
    throw new DesktopServiceError(
      "Desktop save dialog is unavailable in this runtime.",
      "desktop_invoke_unavailable",
    );
  }

  return save({ defaultPath });
}

export async function desktopOpenBackupPath(): Promise<string | null> {
  const open = desktopWindow.__TAURI__?.dialog?.open;
  if (!open) {
    throw new DesktopServiceError(
      "Desktop open dialog is unavailable in this runtime.",
      "desktop_invoke_unavailable",
    );
  }

  const selected = await open({
    multiple: false,
    filters: [{ name: "Backup package", extensions: ["zip"] }],
  });
  return Array.isArray(selected) ? selected[0] ?? null : selected;
}

export const readDashboardStats = (): Promise<DesktopDashboardStatsResponse | null> =>
  desktopRpc<DesktopDashboardStatsResponse | null>("GetDashboardStats");

export const readLicenseStatus = (): Promise<DesktopLicenseStatusResponse | null> =>
  desktopRpc<DesktopLicenseStatusResponse | null>("GetLicenseStatus");

export const reloadDesktopConfig = (): Promise<void> => desktopRpc<void>("ReloadConfig");

export const generateDesktopCdKeys = (count: number): Promise<string[] | null> =>
  desktopRpc<string[] | null>("GenerateCDKeys", [count]);

export const fetchRemoteAuthorProfileFromDesktop = (
  authorURL: string,
  timeoutMs: number,
): Promise<Record<string, unknown>> =>
  desktopRpc<Record<string, unknown>>("FetchRemoteAuthorProfile", [authorURL, timeoutMs], {
    timeoutMs,
  });

export const queryEventLog = (
  query: DesktopEventLogQueryInput,
): Promise<DesktopEventLogEntry[]> => desktopRpc<DesktopEventLogEntry[]>("EventLogQuery", [query]);

export const countEventLog = (query: DesktopEventLogQueryInput): Promise<number> =>
  desktopRpc<number>("EventLogCount", [query]);

export const pruneEventLog = (before: string): Promise<number> =>
  desktopRpc<number>("EventLogPrune", [before]);

export const exportEventLog = (query: DesktopEventLogQueryInput): Promise<string> =>
  desktopRpc<string>("EventLogExport", [query]);

export const synchronizerListGroups = (): Promise<DesktopCoreSyncGroup[]> =>
  desktopRpc<DesktopCoreSyncGroup[]>("SynchronizerListGroups");

export const synchronizerBroadcastNavigate = (groupId: string, url: string): Promise<void> =>
  desktopRpc<void>("SynchronizerBroadcastNavigate", [groupId, url]);

export const synchronizerBroadcastRefresh = (groupId: string): Promise<void> =>
  desktopRpc<void>("SynchronizerBroadcastRefresh", [groupId]);

export const synchronizerNavigateProfile = (profileId: string, url: string): Promise<void> =>
  desktopRpc<void>("SynchronizerNavigateProfile", [profileId, url]);

export const synchronizerRefreshProfile = (profileId: string): Promise<void> =>
  desktopRpc<void>("SynchronizerRefreshProfile", [profileId]);

export const synchronizerCaptureScreenshot = (profileId: string): Promise<string> =>
  desktopRpc<string>("SynchronizerCaptureScreenshot", [profileId]);

export const synchronizerActivateProfile = (profileId: string): Promise<void> =>
  desktopRpc<void>("SynchronizerActivateProfile", [profileId]);

export const browserInstanceStatus = (profileId: string): Promise<unknown | null> =>
  desktopRpc<unknown | null>("BrowserInstanceStatus", [profileId]);

export const workbenchFingerprintHealthProfile = (profileId: string): Promise<unknown> =>
  desktopRpc<unknown>("WorkbenchFingerprintHealthProfile", [profileId]);

export const workbenchFingerprintProfile = (profileId: string): Promise<unknown> =>
  desktopRpc<unknown>("WorkbenchFingerprintProfile", [profileId]);

export const identityReportProfile = (profileId: string): Promise<unknown> =>
  desktopRpc<unknown>("IdentityReportProfile", [profileId]);

export const synchronizerArrangeProfiles = (
  profileIds: string[],
  layout: "grid" | "main-left",
): Promise<DesktopCoreSyncWindowPlacement[]> =>
  desktopRpc<DesktopCoreSyncWindowPlacement[]>("SynchronizerArrangeProfiles", [profileIds, layout]);

export const synchronizerGetOperationLog = (limit = 50): Promise<DesktopCoreSyncOperation[]> =>
  desktopRpc<DesktopCoreSyncOperation[]>("SynchronizerGetOperationLog", [limit]);

export const synchronizerListTasks = (limit = 200): Promise<DesktopCoreWorkbenchTask[]> =>
  desktopRpc<DesktopCoreWorkbenchTask[]>("SynchronizerListTasks", [limit]);

export const synchronizerSaveTasks = (tasks: DesktopCoreWorkbenchTask[]): Promise<void> =>
  desktopRpc<void>("SynchronizerSaveTasks", [tasks]);

export const workbenchListDetectionResults = (
  profileId = "",
  kind = "",
  limit = 50,
): Promise<unknown[]> =>
  desktopRpc<unknown[]>("WorkbenchListDetectionResults", [profileId, kind, limit]);

export const workbenchSaveDetectionResult = (result: unknown): Promise<void> =>
  desktopRpc<void>("WorkbenchSaveDetectionResult", [result]);

export const workbenchGetUiState = (): Promise<unknown> =>
  desktopRpc<unknown>("WorkbenchGetUiState");

export const workbenchSaveUiState = (state: unknown): Promise<void> =>
  desktopRpc<void>("WorkbenchSaveUiState", [state]);

export const workbenchListDetectorSites = (): Promise<unknown[]> =>
  desktopRpc<unknown[]>("WorkbenchListDetectorSites");

export const workbenchRunDetectorSite = (
  profileId: string,
  detectorId: string,
): Promise<unknown> =>
  desktopRpc<unknown>("WorkbenchRunDetectorSite", [profileId, detectorId]);

export const collectValidationReport = (
  browserSignals: DesktopValidationBrowserSignal[] = [],
): Promise<DesktopValidationReport> =>
  invokeDesktop("collect_validation_report", { browserSignals });

export const listValidationReports = (): Promise<DesktopValidationReportSummary[]> =>
  invokeDesktop("list_validation_reports");

export const exportValidationProfileEvidence = (
  profileId?: string | null,
): Promise<DesktopValidationProfileExport> =>
  invokeDesktop("export_validation_profile_evidence", { profileId: profileId ?? null });

export const getAppStatus = (): Promise<DesktopStatusSnapshot> =>
  invokeDesktop("get_app_status");

export const listTaskPage = (query: DesktopTaskQuery): Promise<DesktopTaskPage> =>
  invokeDesktop("list_task_page", { query });

export const listLogPage = (query: DesktopLogQuery): Promise<DesktopLogPage> =>
  invokeDesktop("list_log_page", { query });

export const readSettings = (): Promise<DesktopSettingsSnapshot> =>
  invokeDesktop("read_local_settings");

export const applyRuntimeSettings = (
  draft: DesktopRuntimeSettingsDraft,
): Promise<DesktopSettingsMutationResult> =>
  invokeDesktop("apply_runtime_settings", { draft });

export const restoreRuntimeSettingsDefaults = (): Promise<DesktopSettingsMutationResult> =>
  invokeDesktop("restore_runtime_settings_defaults");

export const readLocalApiSnapshot = (): Promise<DesktopLocalApiSnapshot> =>
  invokeDesktop("read_local_api_snapshot");

export const applyLocalApiSettings = (
  draft: DesktopLocalApiSettingsDraft,
): Promise<DesktopLocalApiMutationResult> =>
  invokeDesktop("apply_local_api_settings", { draft });

export const restoreLocalApiDefaults = (): Promise<DesktopLocalApiMutationResult> =>
  invokeDesktop("restore_local_api_defaults");

export const readBrowserEnvironmentPolicy =
  (): Promise<DesktopBrowserEnvironmentPolicySnapshot> =>
    invokeDesktop("read_browser_environment_policy");

export const applyBrowserEnvironmentPolicy = (
  draft: DesktopBrowserEnvironmentPolicyDraft,
): Promise<DesktopBrowserEnvironmentPolicyMutationResult> =>
  invokeDesktop("apply_browser_environment_policy", { draft });

export const restoreBrowserEnvironmentPolicyDefaults =
  (): Promise<DesktopBrowserEnvironmentPolicyMutationResult> =>
    invokeDesktop("restore_browser_environment_policy_defaults");

export const readCamoufoxSettings = (): Promise<DesktopCamoufoxSettingsSnapshot> =>
  invokeDesktop("read_camoufox_settings");

export const applyCamoufoxSettings = (
  draft: DesktopCamoufoxSettingsDraft,
): Promise<DesktopCamoufoxSettingsMutationResult> =>
  invokeDesktop("apply_camoufox_settings", { draft });

export const checkCamoufoxCapability = (
  request: DesktopCamoufoxCapabilityRequest = {},
): Promise<DesktopCamoufoxCapability> =>
  invokeDesktop("check_camoufox_capability", { request });

export const readLocalAssetWorkspace = (): Promise<DesktopLocalAssetWorkspaceSnapshot> =>
  invokeDesktop("read_local_asset_workspace");

export const readImportExportSkeleton = (): Promise<DesktopImportExportSkeleton> =>
  invokeDesktop("read_import_export_skeleton");

export const exportSessionBundle = (
  request: DesktopSessionBundleExportRequest,
): Promise<DesktopSessionBundleExport> => invokeDesktop("export_session_bundle", { request });

export const preflightSessionBundleImport = (
  request: DesktopSessionBundleImportPreflightRequest,
): Promise<DesktopSessionBundleImportPreflight> =>
  invokeDesktop("preflight_session_bundle_import", { request });

export const restoreSessionBundle = (
  request: DesktopSessionBundleRestoreRequest,
): Promise<DesktopSessionBundleRestoreResult> =>
  invokeDesktop("restore_session_bundle", { request });

export const readProviderProductionReadiness =
  (): Promise<DesktopProviderProductionReadiness> =>
    invokeDesktop("read_provider_production_readiness");

export const readReleaseSmokeContract = (): Promise<DesktopReleaseSmokeContract> =>
  invokeDesktop("read_release_smoke_contract");

export const listEvidenceReports = (): Promise<DesktopEvidenceReportHistory> =>
  invokeDesktop("list_evidence_reports");

export const readBehaviorAuditContract = (): Promise<DesktopBehaviorAuditContract> =>
  invokeDesktop("read_behavior_audit_contract");

export const openLocalAssetEntry = (entryId: DesktopLocalAssetEntryId): Promise<void> =>
  invokeDesktop("open_local_asset_entry", { entryId });

export const openLocalDirectory = (target: DesktopDirectoryTarget): Promise<void> =>
  invokeDesktop("open_local_directory", { target });

export const readLocalRuntimeStatus = (): Promise<DesktopRuntimeStatus> =>
  invokeDesktop("read_local_runtime_status");

export const startLocalRuntime = (): Promise<DesktopRuntimeStatus> =>
  invokeDesktop("start_local_runtime");

export const stopLocalRuntime = (): Promise<DesktopRuntimeStatus> =>
  invokeDesktop("stop_local_runtime");

export const listProfilePage = (
  query: DesktopProfilePageQuery,
): Promise<DesktopProfilePage> => invokeDesktop("list_profile_page", { query });

export const readProfileDetail = (profileId: string): Promise<DesktopProfileDetail> =>
  invokeDesktop("read_profile_detail", { profileId });

export const createProfile = (
  input: DesktopCreateProfileInput,
): Promise<DesktopProfileMutationResult> => invokeDesktop("create_profile", { input });

export const updateProfile = (
  input: DesktopUpdateProfileInput,
): Promise<DesktopProfileMutationResult> => invokeDesktop("update_profile", { input });

export const startProfiles = (
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> => invokeDesktop("start_profiles", { request });

export const stopProfiles = (
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> => invokeDesktop("stop_profiles", { request });

export const openProfiles = (
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> => invokeDesktop("open_profiles", { request });

export const checkProfileProxies = (
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> =>
  invokeDesktop("check_profile_proxies", { request });

export const syncProfiles = (
  request: DesktopProfileBatchActionRequest,
): Promise<DesktopProfileBatchActionResult> => invokeDesktop("sync_profiles", { request });

export const listProxyPage = (query: DesktopProxyPageQuery): Promise<DesktopProxyPage> =>
  invokeDesktop("list_proxy_page", { query });

export const readProxyHealth = (proxyId: string): Promise<DesktopProxyHealth> =>
  invokeDesktop("read_proxy_health", { proxyId });

export const readProxyUsage = (proxyId: string): Promise<DesktopProxyUsageItem[]> =>
  invokeDesktop("read_proxy_usage", { proxyId });

export const checkProxyBatch = (
  request: DesktopProxyBatchCheckRequest,
): Promise<DesktopProxyBatchCheckResponse> => invokeDesktop("check_proxy_batch", { request });

export const changeProxyIp = (
  request: DesktopProxyChangeIpRequest,
): Promise<DesktopProxyChangeIpResult> => invokeDesktop("change_proxy_ip", { request });

export const listTemplateMetadataPage = (
  query: DesktopTemplateMetadataPageQuery,
): Promise<DesktopTemplateMetadataPage> =>
  invokeDesktop("list_template_metadata_page", { query });

export const saveTemplate = (
  input: DesktopTemplateUpsertInput,
): Promise<DesktopTemplateMutationResult> => invokeDesktop("save_template", { input });

export const updateTemplate = (
  input: DesktopTemplateUpsertInput,
): Promise<DesktopTemplateMutationResult> => invokeDesktop("update_template", { input });

export const deleteTemplate = (
  input: DesktopTemplateDeleteInput,
): Promise<DesktopTemplateMutationResult> => invokeDesktop("delete_template", { input });

export const compileTemplateRun = (
  request: DesktopCompileTemplateRunRequest,
): Promise<DesktopCompileTemplateRunResult> =>
  invokeDesktop("compile_template_run", { request });

export const launchTemplateRun = (
  request: DesktopLaunchTemplateRunRequest,
): Promise<DesktopLaunchTemplateRunResult> =>
  invokeDesktop("launch_template_run", { request });

export const readRunDetail = (query: DesktopReadRunDetailQuery): Promise<DesktopRunDetail> =>
  invokeDesktop("read_run_detail", { query });

export const retryTask = (taskId: string): Promise<DesktopTaskWriteResult> =>
  invokeDesktop("retry_task", { taskId });

export const cancelTask = (taskId: string): Promise<DesktopTaskWriteResult> =>
  invokeDesktop("cancel_task", { taskId });

export const retry = retryTask;

export const cancel = cancelTask;

export const confirmManualGate = (
  request: DesktopManualGateActionRequest,
): Promise<DesktopTaskWriteResult> => invokeDesktop("confirm_manual_gate", { request });

export const rejectManualGate = (
  request: DesktopManualGateActionRequest,
): Promise<DesktopTaskWriteResult> => invokeDesktop("reject_manual_gate", { request });

export const readRecorderSnapshot = (
  query: DesktopRecorderSnapshotQuery,
): Promise<DesktopRecorderSnapshot> => invokeDesktop("read_recorder_snapshot", { query });

export const startBehaviorRecording = (
  request: DesktopStartBehaviorRecordingRequest,
): Promise<DesktopRecorderSnapshot> =>
  invokeDesktop("start_behavior_recording", { request });

export const stopBehaviorRecording = (
  request: DesktopStopBehaviorRecordingRequest,
): Promise<DesktopRecorderSnapshot> => invokeDesktop("stop_behavior_recording", { request });

export const appendBehaviorRecordingStep = (
  request: DesktopAppendBehaviorRecordingStepRequest,
): Promise<DesktopRecorderSnapshot> =>
  invokeDesktop("append_behavior_recording_step", { request });

export const listSyncWindows = (): Promise<DesktopSyncWindowState[]> =>
  invokeDesktop("list_sync_windows");

export const readSyncLayoutState = (): Promise<DesktopSyncLayoutState> =>
  invokeDesktop("read_sync_layout_state");

export const readSynchronizerSnapshot = (): Promise<DesktopSynchronizerSnapshot> =>
  invokeDesktop("read_synchronizer_snapshot");

export const setMainSyncWindow = (
  windowId: string,
): Promise<DesktopSynchronizerActionResult> =>
  invokeDesktop("set_main_sync_window", { windowId });

export const applyWindowLayout = (
  layout: DesktopSyncLayoutUpdate,
): Promise<DesktopSynchronizerActionResult> =>
  invokeDesktop("apply_window_layout", { layout });

export const applyBroadcastPlan = (
  request: DesktopSynchronizerBroadcastRequest,
): Promise<DesktopSynchronizerActionResult> =>
  invokeDesktop("apply_broadcast_plan", { request });

export const focusSyncWindow = (
  windowId: string,
): Promise<DesktopSynchronizerActionResult> =>
  invokeDesktop("focus_sync_window", { windowId });

export type { DesktopJsonValue };
