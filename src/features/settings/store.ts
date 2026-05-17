import {
  applyBrowserEnvironmentPolicy,
  applyLocalApiSettings,
  applyRuntimeSettings,
  openLocalAssetEntry,
  openLocalDirectory,
  readBrowserEnvironmentPolicy,
  readImportExportSkeleton,
  readLocalApiSnapshot,
  readLocalAssetWorkspace,
  readSettings,
  restoreBrowserEnvironmentPolicyDefaults,
  restoreLocalApiDefaults as restoreLocalApiDefaultsCommand,
  restoreRuntimeSettingsDefaults,
} from "../../services/desktop";
import { createStore } from "../../store/createStore";
import type {
  DesktopBrowserEnvironmentPolicyDraft as DesktopBrowserEnvironmentPolicyInput,
  DesktopBrowserEnvironmentPolicySnapshot,
  DesktopDirectoryTarget,
  DesktopImportExportSkeleton,
  DesktopLocalApiSettingsDraft as DesktopLocalApiSettingsInput,
  DesktopLocalApiSnapshot,
  DesktopLocalAssetEntryId,
  DesktopLocalAssetWorkspaceSnapshot,
  DesktopRuntimeSettingsDraft as DesktopRuntimeSettingsInput,
  DesktopSettingsSnapshot,
} from "../../types/desktop";

export interface RuntimeSettingsDraft {
  runnerKind: string;
  workerCount: string;
  heartbeatIntervalSeconds: string;
  claimRetryLimit: string;
  idleBackoffMinMs: string;
  idleBackoffMaxMs: string;
  reclaimAfterSeconds: string;
}

export interface LocalApiSettingsDraft {
  host: string;
  port: string;
  startMode: string;
  authMode: string;
  requestLoggingEnabled: string;
  requireLocalToken: string;
  readOnlySafeMode: string;
  maxConcurrentSessions: string;
}

export interface BrowserEnvironmentPolicyDraft {
  browserFamily: string;
  launchStrategy: string;
  profileStorageMode: string;
  defaultViewportPreset: string;
  keepUserDataBetweenRuns: string;
  allowExtensions: string;
  allowBookmarksSeed: string;
  allowProfileArchiveImport: string;
  headlessAllowed: string;
}

type SettingsPendingAction =
  | "applyRuntime"
  | "restoreRuntime"
  | "applyLocalApi"
  | "restoreLocalApi"
  | "applyBrowserEnvironment"
  | "restoreBrowserEnvironment";

interface SettingsState {
  snapshot: DesktopSettingsSnapshot | null;
  localApiSnapshot: DesktopLocalApiSnapshot | null;
  browserEnvironmentSnapshot: DesktopBrowserEnvironmentPolicySnapshot | null;
  assetWorkspace: DesktopLocalAssetWorkspaceSnapshot | null;
  importExportSkeleton: DesktopImportExportSkeleton | null;
  draft: RuntimeSettingsDraft;
  loadedDraft: RuntimeSettingsDraft | null;
  localApiDraft: LocalApiSettingsDraft;
  loadedLocalApiDraft: LocalApiSettingsDraft | null;
  browserEnvironmentDraft: BrowserEnvironmentPolicyDraft;
  loadedBrowserEnvironmentDraft: BrowserEnvironmentPolicyDraft | null;
  isLoading: boolean;
  refreshRequestId: number;
  error: string | null;
  info: string | null;
  openingTarget: DesktopDirectoryTarget | null;
  openingAssetEntryId: DesktopLocalAssetEntryId | null;
  pendingAction: SettingsPendingAction | null;
}

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function createRuntimeDraftFromSnapshot(snapshot: DesktopSettingsSnapshot): RuntimeSettingsDraft {
  return {
    runnerKind: snapshot.runnerKind,
    workerCount: String(snapshot.workerCount),
    heartbeatIntervalSeconds: String(snapshot.heartbeatIntervalSeconds),
    claimRetryLimit: String(snapshot.claimRetryLimit),
    idleBackoffMinMs: String(snapshot.idleBackoffMinMs),
    idleBackoffMaxMs: String(snapshot.idleBackoffMaxMs),
    reclaimAfterSeconds:
      snapshot.reclaimAfterSeconds === null ? "" : String(snapshot.reclaimAfterSeconds),
  };
}

function createLocalApiDraftFromSnapshot(snapshot: DesktopLocalApiSnapshot): LocalApiSettingsDraft {
  return {
    host: snapshot.host,
    port: String(snapshot.port),
    startMode: snapshot.startMode,
    authMode: snapshot.authMode,
    requestLoggingEnabled: String(snapshot.requestLoggingEnabled),
    requireLocalToken: String(snapshot.requireLocalToken),
    readOnlySafeMode: String(snapshot.readOnlySafeMode),
    maxConcurrentSessions: String(snapshot.maxConcurrentSessions),
  };
}

function createBrowserEnvironmentDraftFromSnapshot(
  snapshot: DesktopBrowserEnvironmentPolicySnapshot,
): BrowserEnvironmentPolicyDraft {
  return {
    browserFamily: snapshot.browserFamily,
    launchStrategy: snapshot.launchStrategy,
    profileStorageMode: snapshot.profileStorageMode,
    defaultViewportPreset: snapshot.defaultViewportPreset,
    keepUserDataBetweenRuns: String(snapshot.keepUserDataBetweenRuns),
    allowExtensions: String(snapshot.allowExtensions),
    allowBookmarksSeed: String(snapshot.allowBookmarksSeed),
    allowProfileArchiveImport: String(snapshot.allowProfileArchiveImport),
    headlessAllowed: String(snapshot.headlessAllowed),
  };
}

export const DEFAULT_RUNTIME_SETTINGS_DRAFT: RuntimeSettingsDraft = {
  runnerKind: "fake",
  workerCount: "1",
  heartbeatIntervalSeconds: "5",
  claimRetryLimit: "8",
  idleBackoffMinMs: "250",
  idleBackoffMaxMs: "3000",
  reclaimAfterSeconds: "",
};

export const DEFAULT_LOCAL_API_SETTINGS_DRAFT: LocalApiSettingsDraft = {
  host: "127.0.0.1",
  port: "3000",
  startMode: "manual",
  authMode: "desktop_session",
  requestLoggingEnabled: "true",
  requireLocalToken: "false",
  readOnlySafeMode: "false",
  maxConcurrentSessions: "4",
};

export const DEFAULT_BROWSER_ENVIRONMENT_POLICY_DRAFT: BrowserEnvironmentPolicyDraft = {
  browserFamily: "chrome",
  launchStrategy: "reuse_or_bootstrap",
  profileStorageMode: "per_profile",
  defaultViewportPreset: "desktop_1600",
  keepUserDataBetweenRuns: "true",
  allowExtensions: "true",
  allowBookmarksSeed: "true",
  allowProfileArchiveImport: "true",
  headlessAllowed: "false",
};

export function areSettingsDraftEqual(
  left: RuntimeSettingsDraft,
  right: RuntimeSettingsDraft,
): boolean {
  return (
    left.runnerKind === right.runnerKind &&
    left.workerCount === right.workerCount &&
    left.heartbeatIntervalSeconds === right.heartbeatIntervalSeconds &&
    left.claimRetryLimit === right.claimRetryLimit &&
    left.idleBackoffMinMs === right.idleBackoffMinMs &&
    left.idleBackoffMaxMs === right.idleBackoffMaxMs &&
    left.reclaimAfterSeconds === right.reclaimAfterSeconds
  );
}

export function areLocalApiSettingsDraftEqual(
  left: LocalApiSettingsDraft,
  right: LocalApiSettingsDraft,
): boolean {
  return (
    left.host === right.host &&
    left.port === right.port &&
    left.startMode === right.startMode &&
    left.authMode === right.authMode &&
    left.requestLoggingEnabled === right.requestLoggingEnabled &&
    left.requireLocalToken === right.requireLocalToken &&
    left.readOnlySafeMode === right.readOnlySafeMode &&
    left.maxConcurrentSessions === right.maxConcurrentSessions
  );
}

export function areBrowserEnvironmentPolicyDraftEqual(
  left: BrowserEnvironmentPolicyDraft,
  right: BrowserEnvironmentPolicyDraft,
): boolean {
  return (
    left.browserFamily === right.browserFamily &&
    left.launchStrategy === right.launchStrategy &&
    left.profileStorageMode === right.profileStorageMode &&
    left.defaultViewportPreset === right.defaultViewportPreset &&
    left.keepUserDataBetweenRuns === right.keepUserDataBetweenRuns &&
    left.allowExtensions === right.allowExtensions &&
    left.allowBookmarksSeed === right.allowBookmarksSeed &&
    left.allowProfileArchiveImport === right.allowProfileArchiveImport &&
    left.headlessAllowed === right.headlessAllowed
  );
}

function shouldReplaceDraft<T>(
  currentDraft: T,
  currentLoadedDraft: T | null,
  equality: (left: T, right: T) => boolean,
): boolean {
  return currentLoadedDraft === null || equality(currentDraft, currentLoadedDraft);
}

export const settingsStore = createStore<SettingsState>({
  snapshot: null,
  localApiSnapshot: null,
  browserEnvironmentSnapshot: null,
  assetWorkspace: null,
  importExportSkeleton: null,
  draft: DEFAULT_RUNTIME_SETTINGS_DRAFT,
  loadedDraft: null,
  localApiDraft: DEFAULT_LOCAL_API_SETTINGS_DRAFT,
  loadedLocalApiDraft: null,
  browserEnvironmentDraft: DEFAULT_BROWSER_ENVIRONMENT_POLICY_DRAFT,
  loadedBrowserEnvironmentDraft: null,
  isLoading: false,
  refreshRequestId: 0,
  error: null,
  info: null,
  openingTarget: null,
  openingAssetEntryId: null,
  pendingAction: null,
});

export const settingsActions = {
  async refresh() {
    const requestId = settingsStore.getState().refreshRequestId + 1;
    settingsStore.setState((current) => ({
      ...current,
      isLoading: true,
      refreshRequestId: requestId,
      error: null,
      info: null,
    }));

    try {
      const [
        snapshot,
        localApiSnapshot,
        browserEnvironmentSnapshot,
        assetWorkspace,
        importExportSkeleton,
      ] = await Promise.all([
        readSettings(),
        readLocalApiSnapshot(),
        readBrowserEnvironmentPolicy(),
        readLocalAssetWorkspace(),
        readImportExportSkeleton(),
      ]);

      if (settingsStore.getState().refreshRequestId !== requestId) {
        return;
      }

      const loadedDraft = createRuntimeDraftFromSnapshot(snapshot);
      const loadedLocalApiDraft = createLocalApiDraftFromSnapshot(localApiSnapshot);
      const loadedBrowserEnvironmentDraft =
        createBrowserEnvironmentDraftFromSnapshot(browserEnvironmentSnapshot);

      settingsStore.setState((current) => {
        const preservedDrafts: string[] = [];
        if (
          current.loadedDraft !== null &&
          !areSettingsDraftEqual(current.draft, current.loadedDraft)
        ) {
          preservedDrafts.push("runtime");
        }
        if (
          current.loadedLocalApiDraft !== null &&
          !areLocalApiSettingsDraftEqual(current.localApiDraft, current.loadedLocalApiDraft)
        ) {
          preservedDrafts.push("local API");
        }
        if (
          current.loadedBrowserEnvironmentDraft !== null &&
          !areBrowserEnvironmentPolicyDraftEqual(
            current.browserEnvironmentDraft,
            current.loadedBrowserEnvironmentDraft,
          )
        ) {
          preservedDrafts.push("browser environment");
        }

        return {
          ...current,
          snapshot,
          localApiSnapshot,
          browserEnvironmentSnapshot,
          assetWorkspace,
          importExportSkeleton,
          draft: shouldReplaceDraft(
            current.draft,
            current.loadedDraft,
            areSettingsDraftEqual,
          )
            ? loadedDraft
            : current.draft,
          loadedDraft,
          localApiDraft: shouldReplaceDraft(
            current.localApiDraft,
            current.loadedLocalApiDraft,
            areLocalApiSettingsDraftEqual,
          )
            ? loadedLocalApiDraft
            : current.localApiDraft,
          loadedLocalApiDraft,
          browserEnvironmentDraft: shouldReplaceDraft(
            current.browserEnvironmentDraft,
            current.loadedBrowserEnvironmentDraft,
            areBrowserEnvironmentPolicyDraftEqual,
          )
            ? loadedBrowserEnvironmentDraft
            : current.browserEnvironmentDraft,
          loadedBrowserEnvironmentDraft,
          isLoading: false,
          error: null,
          info:
            preservedDrafts.length > 0
              ? `Fresh local platform snapshots loaded. Preserved ${preservedDrafts.join(", ")} drafts.`
              : null,
        };
      });
    } catch (error) {
      if (settingsStore.getState().refreshRequestId !== requestId) {
        return;
      }

      settingsStore.setState((current) => ({
        ...current,
        isLoading: false,
        error: toErrorMessage(error),
      }));
    }
  },

  async openDirectory(target: DesktopDirectoryTarget) {
    settingsStore.setState((current) => ({
      ...current,
      openingTarget: target,
      error: null,
      info: null,
    }));

    try {
      await openLocalDirectory(target);
      settingsStore.setState((current) => ({
        ...current,
        openingTarget: current.openingTarget === target ? null : current.openingTarget,
      }));
    } catch (error) {
      settingsStore.setState((current) => ({
        ...current,
        openingTarget: current.openingTarget === target ? null : current.openingTarget,
        error: toErrorMessage(error),
      }));
    }
  },

  async openAssetEntry(entryId: DesktopLocalAssetEntryId) {
    settingsStore.setState((current) => ({
      ...current,
      openingAssetEntryId: entryId,
      error: null,
      info: null,
    }));

    try {
      await openLocalAssetEntry(entryId);
      settingsStore.setState((current) => ({
        ...current,
        openingAssetEntryId:
          current.openingAssetEntryId === entryId ? null : current.openingAssetEntryId,
      }));
    } catch (error) {
      settingsStore.setState((current) => ({
        ...current,
        openingAssetEntryId:
          current.openingAssetEntryId === entryId ? null : current.openingAssetEntryId,
        error: toErrorMessage(error),
      }));
    }
  },

  updateDraftField<Key extends keyof RuntimeSettingsDraft>(
    field: Key,
    value: RuntimeSettingsDraft[Key],
  ) {
    settingsStore.setState((current) => ({
      ...current,
      draft: {
        ...current.draft,
        [field]: value,
      },
      info: null,
    }));
  },

  updateLocalApiDraftField<Key extends keyof LocalApiSettingsDraft>(
    field: Key,
    value: LocalApiSettingsDraft[Key],
  ) {
    settingsStore.setState((current) => ({
      ...current,
      localApiDraft: {
        ...current.localApiDraft,
        [field]: value,
      },
      info: null,
    }));
  },

  updateBrowserEnvironmentDraftField<Key extends keyof BrowserEnvironmentPolicyDraft>(
    field: Key,
    value: BrowserEnvironmentPolicyDraft[Key],
  ) {
    settingsStore.setState((current) => ({
      ...current,
      browserEnvironmentDraft: {
        ...current.browserEnvironmentDraft,
        [field]: value,
      },
      info: null,
    }));
  },

  resetRuntimeDraft() {
    settingsStore.setState((current) => ({
      ...current,
      draft: current.loadedDraft ?? current.draft,
      info: current.loadedDraft
        ? "Runtime draft reset to the latest desktop snapshot."
        : "No runtime snapshot is available yet.",
    }));
  },

  resetLocalApiDraft() {
    settingsStore.setState((current) => ({
      ...current,
      localApiDraft: current.loadedLocalApiDraft ?? current.localApiDraft,
      info: current.loadedLocalApiDraft
        ? "Local API draft reset to the latest desktop snapshot."
        : "No Local API snapshot is available yet.",
    }));
  },

  resetBrowserEnvironmentDraft() {
    settingsStore.setState((current) => ({
      ...current,
      browserEnvironmentDraft:
        current.loadedBrowserEnvironmentDraft ?? current.browserEnvironmentDraft,
      info: current.loadedBrowserEnvironmentDraft
        ? "Browser environment draft reset to the latest desktop snapshot."
        : "No browser environment snapshot is available yet.",
    }));
  },

  resetAllDraftsToLoaded() {
    settingsStore.setState((current) => ({
      ...current,
      draft: current.loadedDraft ?? current.draft,
      localApiDraft: current.loadedLocalApiDraft ?? current.localApiDraft,
      browserEnvironmentDraft:
        current.loadedBrowserEnvironmentDraft ?? current.browserEnvironmentDraft,
      info: "All local drafts reset to the latest loaded desktop snapshots.",
    }));
  },

  async restoreDefaults() {
    settingsStore.setState((current) => ({
      ...current,
      pendingAction: "restoreRuntime",
      error: null,
      info: null,
    }));

    try {
      const result = await restoreRuntimeSettingsDefaults();
      const loadedDraft = createRuntimeDraftFromSnapshot(result.snapshot);

      settingsStore.setState((current) => ({
        ...current,
        snapshot: result.snapshot,
        draft: loadedDraft,
        loadedDraft,
        pendingAction: null,
        error: null,
        info: result.message,
      }));
    } catch (error) {
      settingsStore.setState((current) => ({
        ...current,
        pendingAction: null,
        error: toErrorMessage(error),
      }));
    }
  },

  async applyDraft() {
    settingsStore.setState((current) => ({
      ...current,
      pendingAction: "applyRuntime",
      error: null,
      info: null,
    }));

    try {
      const draft = toDesktopRuntimeSettingsDraft(settingsStore.getState().draft);
      const result = await applyRuntimeSettings(draft);
      const loadedDraft = createRuntimeDraftFromSnapshot(result.snapshot);

      settingsStore.setState((current) => ({
        ...current,
        snapshot: result.snapshot,
        draft: loadedDraft,
        loadedDraft,
        pendingAction: null,
        error: null,
        info: result.message,
      }));
    } catch (error) {
      settingsStore.setState((current) => ({
        ...current,
        pendingAction: null,
        error: toErrorMessage(error),
      }));
    }
  },

  async restoreLocalApiDefaults() {
    settingsStore.setState((current) => ({
      ...current,
      pendingAction: "restoreLocalApi",
      error: null,
      info: null,
    }));

    try {
      const result = await restoreLocalApiDefaultsCommand();
      const loadedLocalApiDraft = createLocalApiDraftFromSnapshot(result.snapshot);

      settingsStore.setState((current) => ({
        ...current,
        localApiSnapshot: result.snapshot,
        localApiDraft: loadedLocalApiDraft,
        loadedLocalApiDraft,
        pendingAction: null,
        error: null,
        info: result.message,
      }));
    } catch (error) {
      settingsStore.setState((current) => ({
        ...current,
        pendingAction: null,
        error: toErrorMessage(error),
      }));
    }
  },

  async applyLocalApiDraft() {
    settingsStore.setState((current) => ({
      ...current,
      pendingAction: "applyLocalApi",
      error: null,
      info: null,
    }));

    try {
      const draft = toDesktopLocalApiSettingsDraft(settingsStore.getState().localApiDraft);
      const result = await applyLocalApiSettings(draft);
      const loadedLocalApiDraft = createLocalApiDraftFromSnapshot(result.snapshot);

      settingsStore.setState((current) => ({
        ...current,
        localApiSnapshot: result.snapshot,
        localApiDraft: loadedLocalApiDraft,
        loadedLocalApiDraft,
        pendingAction: null,
        error: null,
        info: result.message,
      }));
    } catch (error) {
      settingsStore.setState((current) => ({
        ...current,
        pendingAction: null,
        error: toErrorMessage(error),
      }));
    }
  },

  async restoreBrowserEnvironmentDefaults() {
    settingsStore.setState((current) => ({
      ...current,
      pendingAction: "restoreBrowserEnvironment",
      error: null,
      info: null,
    }));

    try {
      const result = await restoreBrowserEnvironmentPolicyDefaults();
      const loadedBrowserEnvironmentDraft =
        createBrowserEnvironmentDraftFromSnapshot(result.snapshot);

      settingsStore.setState((current) => ({
        ...current,
        browserEnvironmentSnapshot: result.snapshot,
        browserEnvironmentDraft: loadedBrowserEnvironmentDraft,
        loadedBrowserEnvironmentDraft,
        pendingAction: null,
        error: null,
        info: result.message,
      }));
    } catch (error) {
      settingsStore.setState((current) => ({
        ...current,
        pendingAction: null,
        error: toErrorMessage(error),
      }));
    }
  },

  async applyBrowserEnvironmentDraft() {
    settingsStore.setState((current) => ({
      ...current,
      pendingAction: "applyBrowserEnvironment",
      error: null,
      info: null,
    }));

    try {
      const draft = toDesktopBrowserEnvironmentPolicyDraft(
        settingsStore.getState().browserEnvironmentDraft,
      );
      const result = await applyBrowserEnvironmentPolicy(draft);
      const loadedBrowserEnvironmentDraft =
        createBrowserEnvironmentDraftFromSnapshot(result.snapshot);

      settingsStore.setState((current) => ({
        ...current,
        browserEnvironmentSnapshot: result.snapshot,
        browserEnvironmentDraft: loadedBrowserEnvironmentDraft,
        loadedBrowserEnvironmentDraft,
        pendingAction: null,
        error: null,
        info: result.message,
      }));
    } catch (error) {
      settingsStore.setState((current) => ({
        ...current,
        pendingAction: null,
        error: toErrorMessage(error),
      }));
    }
  },
};

function parsePositiveInteger(value: string, fieldLabel: string, allowZero = false): number {
  const normalized = value.trim();
  if (!normalized) {
    throw new Error(`${fieldLabel} is required.`);
  }

  const parsed = Number.parseInt(normalized, 10);
  if (!Number.isFinite(parsed) || (!allowZero && parsed <= 0) || (allowZero && parsed < 0)) {
    throw new Error(
      `${fieldLabel} must be a valid ${allowZero ? "non-negative" : "positive"} integer.`,
    );
  }

  return parsed;
}

function parseBooleanValue(value: string, fieldLabel: string): boolean {
  if (value === "true") {
    return true;
  }
  if (value === "false") {
    return false;
  }
  throw new Error(`${fieldLabel} must be either true or false.`);
}

function toDesktopRuntimeSettingsDraft(
  draft: RuntimeSettingsDraft,
): DesktopRuntimeSettingsInput {
  const reclaimAfterSeconds = draft.reclaimAfterSeconds.trim();
  const idleBackoffMinMs = parsePositiveInteger(draft.idleBackoffMinMs, "Idle backoff min", true);
  const idleBackoffMaxMs = parsePositiveInteger(draft.idleBackoffMaxMs, "Idle backoff max", true);

  if (idleBackoffMaxMs < idleBackoffMinMs) {
    throw new Error("Idle backoff max must be greater than or equal to idle backoff min.");
  }

  return {
    runnerKind: draft.runnerKind.trim() || "fake",
    workerCount: parsePositiveInteger(draft.workerCount, "Worker count"),
    reclaimAfterSeconds: reclaimAfterSeconds
      ? parsePositiveInteger(reclaimAfterSeconds, "Reclaim after", true)
      : null,
    heartbeatIntervalSeconds: parsePositiveInteger(
      draft.heartbeatIntervalSeconds,
      "Heartbeat interval",
    ),
    claimRetryLimit: parsePositiveInteger(draft.claimRetryLimit, "Claim retry limit"),
    idleBackoffMinMs,
    idleBackoffMaxMs,
  };
}

function toDesktopLocalApiSettingsDraft(
  draft: LocalApiSettingsDraft,
): DesktopLocalApiSettingsInput {
  const host = draft.host.trim().toLowerCase();
  if (host !== "127.0.0.1" && host !== "localhost") {
    throw new Error("Local API host must stay on loopback: use 127.0.0.1 or localhost.");
  }

  const port = parsePositiveInteger(draft.port, "Local API port");
  if (port > 65535) {
    throw new Error("Local API port must be less than or equal to 65535.");
  }

  return {
    host,
    port,
    startMode: draft.startMode.trim() || "manual",
    authMode: draft.authMode.trim() || "desktop_session",
    requestLoggingEnabled: parseBooleanValue(
      draft.requestLoggingEnabled,
      "Request logging enabled",
    ),
    requireLocalToken: parseBooleanValue(draft.requireLocalToken, "Require local token"),
    readOnlySafeMode: parseBooleanValue(draft.readOnlySafeMode, "Read only safe mode"),
    maxConcurrentSessions: parsePositiveInteger(
      draft.maxConcurrentSessions,
      "Max concurrent sessions",
    ),
  };
}

function toDesktopBrowserEnvironmentPolicyDraft(
  draft: BrowserEnvironmentPolicyDraft,
): DesktopBrowserEnvironmentPolicyInput {
  return {
    browserFamily: draft.browserFamily.trim() || "chrome",
    launchStrategy: draft.launchStrategy.trim() || "reuse_or_bootstrap",
    profileStorageMode: draft.profileStorageMode.trim() || "per_profile",
    defaultViewportPreset: draft.defaultViewportPreset.trim() || "desktop_1600",
    keepUserDataBetweenRuns: parseBooleanValue(
      draft.keepUserDataBetweenRuns,
      "Keep user data between runs",
    ),
    allowExtensions: parseBooleanValue(draft.allowExtensions, "Allow extensions"),
    allowBookmarksSeed: parseBooleanValue(draft.allowBookmarksSeed, "Allow bookmarks seed"),
    allowProfileArchiveImport: parseBooleanValue(
      draft.allowProfileArchiveImport,
      "Allow profile archive import",
    ),
    headlessAllowed: parseBooleanValue(draft.headlessAllowed, "Headless allowed"),
  };
}
