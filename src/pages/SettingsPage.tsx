import type {
  DesktopDirectoryTarget,
  DesktopLocalAssetEntry,
  DesktopSettingsSnapshot,
} from "../types/desktop";
import { EmptyState } from "../components/EmptyState";
import { Panel } from "../components/Panel";
import { useRuntimeViewModel } from "../features/runtime/hooks";
import { useSettingsViewModel } from "../features/settings/hooks";
import {
  formatRelativeTimestamp,
  formatRunnerLabel,
  formatStatusLabel,
} from "../utils/format";

const LOCAL_DIRECTORY_ITEMS: Array<{
  label: string;
  target: DesktopDirectoryTarget;
  getValue: (snapshot: DesktopSettingsSnapshot) => string;
}> = [
  {
    label: "Project root",
    target: "projectRoot",
    getValue: (snapshot) => snapshot.projectRoot,
  },
  {
    label: "Data dir",
    target: "dataDir",
    getValue: (snapshot) => snapshot.dataDir,
  },
  {
    label: "Reports dir",
    target: "reportsDir",
    getValue: (snapshot) => snapshot.reportsDir,
  },
  {
    label: "Logs dir",
    target: "logsDir",
    getValue: (snapshot) => snapshot.logsDir,
  },
];

const PACKAGED_DIRECTORY_ITEMS: Array<{
  label: string;
  target: DesktopDirectoryTarget;
  getValue: (snapshot: DesktopSettingsSnapshot) => string;
}> = [
  {
    label: "Packaged data dir",
    target: "packagedDataDir",
    getValue: (snapshot) => snapshot.packagedDataDir,
  },
  {
    label: "Packaged reports dir",
    target: "packagedReportsDir",
    getValue: (snapshot) => snapshot.packagedReportsDir,
  },
  {
    label: "Packaged logs dir",
    target: "packagedLogsDir",
    getValue: (snapshot) => snapshot.packagedLogsDir,
  },
];

const RUNNER_KIND_OPTIONS = [
  { value: "fake", label: "Fake" },
  { value: "lightpanda", label: "Lightpanda" },
];

const LOCAL_API_START_MODE_OPTIONS = [
  { value: "manual", label: "Manual" },
  { value: "auto_on_shell_open", label: "Auto on shell open" },
];

const LOCAL_API_AUTH_MODE_OPTIONS = [
  { value: "desktop_session", label: "Desktop session" },
  { value: "loopback_token", label: "Loopback token" },
];

const BROWSER_FAMILY_OPTIONS = [
  { value: "chrome", label: "Chrome shell" },
  { value: "edge", label: "Edge shell" },
  { value: "lightpanda", label: "Lightpanda shell" },
];

const BROWSER_LAUNCH_STRATEGY_OPTIONS = [
  { value: "reuse_or_bootstrap", label: "Reuse or bootstrap" },
  { value: "clean_bootstrap", label: "Clean bootstrap" },
  { value: "attach_existing", label: "Attach existing" },
];

const BROWSER_STORAGE_MODE_OPTIONS = [
  { value: "per_profile", label: "Per profile" },
  { value: "shared_workspace", label: "Shared workspace" },
];

const VIEWPORT_PRESET_OPTIONS = [
  { value: "desktop_1600", label: "Desktop 1600" },
  { value: "desktop_1920", label: "Desktop 1920" },
  { value: "laptop_1440", label: "Laptop 1440" },
];

const BOOLEAN_OPTIONS = [
  { value: "true", label: "Enabled" },
  { value: "false", label: "Disabled" },
];

function getRuntimeBadgeTone(status: string | null): "success" | "warning" | "info" {
  if (status === "managed_running") {
    return "success";
  }
  if (status === "external_running" || status === "managed_stopped") {
    return "warning";
  }
  return "info";
}

function getAssetBadgeTone(entry: DesktopLocalAssetEntry): "success" | "warning" {
  return entry.status === "ready" ? "success" : "warning";
}

function renderBooleanSelect(
  value: string,
  onChange: (nextValue: string) => void,
) {
  return (
    <select
      className="field__input"
      value={value}
      onChange={(event) => onChange(event.target.value)}
    >
      {BOOLEAN_OPTIONS.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  );
}

export function SettingsPage() {
  const {
    state,
    isDirty,
    runtimeIsDirty,
    localApiIsDirty,
    browserEnvironmentIsDirty,
    actions,
  } = useSettingsViewModel();
  const runtime = useRuntimeViewModel();
  const snapshot = state.snapshot;
  const localApiSnapshot = state.localApiSnapshot;
  const browserEnvironmentSnapshot = state.browserEnvironmentSnapshot;
  const assetWorkspace = state.assetWorkspace;
  const importExportSkeleton = state.importExportSkeleton;
  const runtimeSnapshot = runtime.state.snapshot;
  const readyAssetCount =
    assetWorkspace?.entries.filter((entry) => entry.status === "ready").length ?? 0;

  return (
    <div className="page-stack">
      {state.error ? <div className="banner banner--error">{state.error}</div> : null}
      {runtime.state.error ? (
        <div className="banner banner--error">{runtime.state.error}</div>
      ) : null}
      {state.info ? <div className="banner banner--info">{state.info}</div> : null}

      <div className="toolbar-card settings-toolbar">
        <div>
          <span className="shell__eyebrow">Local Platform Console</span>
          <h2>Settings + Local API + Assets</h2>
          <p>
            Keep runtime policy, loopback API control, browser environment policy, and local asset
            workspace visible from one Win11 desktop shell without adding any cloud control plane.
          </p>
        </div>
        <div className="settings-toolbar__actions">
          <span className={`badge badge--${isDirty ? "warning" : "success"}`}>
            {isDirty ? "Drafts pending" : "All drafts synced"}
          </span>
          <button className="button button--secondary" type="button" onClick={() => void actions.refresh()}>
            {state.isLoading ? "Refreshing..." : "Refresh console"}
          </button>
          <button className="button button--secondary" type="button" onClick={() => actions.resetAllDraftsToLoaded()}>
            Reset all drafts
          </button>
        </div>
      </div>

      <div className="automation-metric-strip automation-metric-strip--compact">
        <article className="automation-metric-strip__item">
          <span className="automation-metric-strip__label">Runtime policy</span>
          <strong>{snapshot ? formatRunnerLabel(snapshot.runnerKind) : "Pending"}</strong>
          <small>
            {snapshot
              ? `${snapshot.workerCount} workers, ${snapshot.heartbeatIntervalSeconds}s heartbeat`
              : "Read settings to load runtime policy"}
          </small>
        </article>
        <article className="automation-metric-strip__item">
          <span className="automation-metric-strip__label">Local API bind</span>
          <strong>
            {localApiSnapshot ? `${localApiSnapshot.host}:${localApiSnapshot.port}` : "Pending"}
          </strong>
          <small>
            {localApiSnapshot
              ? `${formatStatusLabel(localApiSnapshot.startMode)} / ${formatStatusLabel(localApiSnapshot.authMode)}`
              : "Loopback-only API control not loaded yet"}
          </small>
        </article>
        <article className="automation-metric-strip__item">
          <span className="automation-metric-strip__label">Asset workspace</span>
          <strong>
            {assetWorkspace ? `${readyAssetCount}/${assetWorkspace.entries.length}` : "Pending"}
          </strong>
          <small>
            {assetWorkspace
              ? "Ready vs provision-on-demand entries inside the local asset workspace"
              : "Asset workspace snapshot not loaded yet"}
          </small>
        </article>
      </div>

      <div className="page-grid page-grid--two">
        <Panel
          title="Runtime Settings"
          subtitle="Existing runtime policy stays editable and continues to persist through the desktop service layer"
          actions={
            <div className="inline-actions">
              <button
                className="button button--secondary"
                type="button"
                onClick={() => actions.resetRuntimeDraft()}
              >
                Reset runtime draft
              </button>
              <button
                className="button button--secondary"
                type="button"
                onClick={() => void actions.restoreDefaults()}
                disabled={state.pendingAction !== null}
              >
                {state.pendingAction === "restoreRuntime"
                  ? "Restoring..."
                  : "Restore defaults"}
              </button>
              <button
                className="button"
                type="button"
                onClick={() => void actions.applyDraft()}
                disabled={!runtimeIsDirty || state.pendingAction !== null}
              >
                {state.pendingAction === "applyRuntime" ? "Applying..." : "Apply settings"}
              </button>
            </div>
          }
        >
          <div className="settings-form">
            <label className="field">
              <span className="field__label">Runner kind</span>
              <select
                className="field__input"
                value={state.draft.runnerKind}
                onChange={(event) => actions.updateDraftField("runnerKind", event.target.value)}
              >
                {RUNNER_KIND_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span className="field__label">Worker count</span>
              <input
                className="field__input"
                type="number"
                min="1"
                value={state.draft.workerCount}
                onChange={(event) => actions.updateDraftField("workerCount", event.target.value)}
              />
            </label>

            <label className="field">
              <span className="field__label">Heartbeat interval (s)</span>
              <input
                className="field__input"
                type="number"
                min="1"
                value={state.draft.heartbeatIntervalSeconds}
                onChange={(event) =>
                  actions.updateDraftField("heartbeatIntervalSeconds", event.target.value)
                }
              />
            </label>

            <label className="field">
              <span className="field__label">Claim retry limit</span>
              <input
                className="field__input"
                type="number"
                min="1"
                value={state.draft.claimRetryLimit}
                onChange={(event) =>
                  actions.updateDraftField("claimRetryLimit", event.target.value)
                }
              />
            </label>

            <label className="field">
              <span className="field__label">Idle backoff min (ms)</span>
              <input
                className="field__input"
                type="number"
                min="0"
                value={state.draft.idleBackoffMinMs}
                onChange={(event) =>
                  actions.updateDraftField("idleBackoffMinMs", event.target.value)
                }
              />
            </label>

            <label className="field">
              <span className="field__label">Idle backoff max (ms)</span>
              <input
                className="field__input"
                type="number"
                min="0"
                value={state.draft.idleBackoffMaxMs}
                onChange={(event) =>
                  actions.updateDraftField("idleBackoffMaxMs", event.target.value)
                }
              />
            </label>

            <label className="field settings-form__wide">
              <span className="field__label">Reclaim after (s)</span>
              <input
                className="field__input"
                type="number"
                min="0"
                value={state.draft.reclaimAfterSeconds}
                placeholder="Leave empty to disable reclaim"
                onChange={(event) =>
                  actions.updateDraftField("reclaimAfterSeconds", event.target.value)
                }
              />
              <span className="field__hint">
                Apply persists this local runtime policy. Existing Tasks and runs board behavior
                stays intact.
              </span>
            </label>
          </div>
        </Panel>

        <Panel
          title="Local API Control"
          subtitle="Loopback host, start policy, auth mode, and live runtime reachability in one control surface"
          actions={
            <div className="inline-actions">
              <button
                className="button button--secondary"
                onClick={() => void runtime.actions.refresh()}
                type="button"
              >
                {runtime.state.isLoading ? "Refreshing..." : "Refresh runtime"}
              </button>
              <button
                className="button"
                onClick={() => void runtime.actions.start()}
                type="button"
                disabled={
                  runtime.state.activeAction !== null || runtimeSnapshot?.running === true
                }
              >
                {runtime.state.activeAction === "start" ? "Starting..." : "Start runtime"}
              </button>
              <button
                className="button button--secondary"
                onClick={() => void runtime.actions.stop()}
                type="button"
                disabled={
                  runtime.state.activeAction !== null || runtimeSnapshot?.managed !== true
                }
              >
                {runtime.state.activeAction === "stop" ? "Stopping..." : "Stop runtime"}
              </button>
              <button
                className="button button--secondary"
                type="button"
                onClick={() => actions.resetLocalApiDraft()}
              >
                Reset API draft
              </button>
              <button
                className="button button--secondary"
                type="button"
                onClick={() => void actions.restoreLocalApiDefaults()}
                disabled={state.pendingAction !== null}
              >
                {state.pendingAction === "restoreLocalApi" ? "Restoring..." : "Restore API defaults"}
              </button>
              <button
                className="button"
                type="button"
                onClick={() => void actions.applyLocalApiDraft()}
                disabled={!localApiIsDirty || state.pendingAction !== null}
              >
                {state.pendingAction === "applyLocalApi" ? "Applying..." : "Apply API settings"}
              </button>
            </div>
          }
        >
          {runtime.state.info ? <div className="banner banner--info">{runtime.state.info}</div> : null}
          {localApiSnapshot ? (
            <div className="details-grid details-grid--two">
              <div className="details-grid__item">
                <dt>Runtime status</dt>
                <dd>
                  <span className={`badge badge--${getRuntimeBadgeTone(runtimeSnapshot?.status ?? null)}`}>
                    {runtimeSnapshot ? formatStatusLabel(runtimeSnapshot.status) : "Pending"}
                  </span>
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Health URL</dt>
                <dd>{localApiSnapshot.healthUrl}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Base URL</dt>
                <dd>{localApiSnapshot.baseUrl}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Reachable</dt>
                <dd>{runtimeSnapshot?.apiReachable ? "Yes" : "No"}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Bind mode</dt>
                <dd>{formatStatusLabel(localApiSnapshot.bindMode)}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Config file</dt>
                <dd>{localApiSnapshot.configPath}</dd>
              </div>
              <div className="details-grid__item">
                <dt>PID</dt>
                <dd>{runtimeSnapshot?.pid ?? "N/A"}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Updated</dt>
                <dd>{formatRelativeTimestamp(localApiSnapshot.updatedAt)}</dd>
              </div>
            </div>
          ) : (
            <EmptyState
              title="Local API control unavailable"
              detail="Refresh this panel to load loopback API settings and runtime reachability."
            />
          )}

          <div className="settings-form">
            <label className="field">
              <span className="field__label">Host</span>
              <input
                className="field__input"
                type="text"
                value={state.localApiDraft.host}
                onChange={(event) => actions.updateLocalApiDraftField("host", event.target.value)}
              />
            </label>

            <label className="field">
              <span className="field__label">Port</span>
              <input
                className="field__input"
                type="number"
                min="1"
                max="65535"
                value={state.localApiDraft.port}
                onChange={(event) => actions.updateLocalApiDraftField("port", event.target.value)}
              />
            </label>

            <label className="field">
              <span className="field__label">Start mode</span>
              <select
                className="field__input"
                value={state.localApiDraft.startMode}
                onChange={(event) =>
                  actions.updateLocalApiDraftField("startMode", event.target.value)
                }
              >
                {LOCAL_API_START_MODE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span className="field__label">Auth mode</span>
              <select
                className="field__input"
                value={state.localApiDraft.authMode}
                onChange={(event) =>
                  actions.updateLocalApiDraftField("authMode", event.target.value)
                }
              >
                {LOCAL_API_AUTH_MODE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span className="field__label">Request logging</span>
              {renderBooleanSelect(state.localApiDraft.requestLoggingEnabled, (nextValue) =>
                actions.updateLocalApiDraftField("requestLoggingEnabled", nextValue),
              )}
            </label>

            <label className="field">
              <span className="field__label">Loopback token</span>
              {renderBooleanSelect(state.localApiDraft.requireLocalToken, (nextValue) =>
                actions.updateLocalApiDraftField("requireLocalToken", nextValue),
              )}
            </label>

            <label className="field">
              <span className="field__label">Read-only safe mode</span>
              {renderBooleanSelect(state.localApiDraft.readOnlySafeMode, (nextValue) =>
                actions.updateLocalApiDraftField("readOnlySafeMode", nextValue),
              )}
            </label>

            <label className="field">
              <span className="field__label">Max concurrent sessions</span>
              <input
                className="field__input"
                type="number"
                min="1"
                value={state.localApiDraft.maxConcurrentSessions}
                onChange={(event) =>
                  actions.updateLocalApiDraftField("maxConcurrentSessions", event.target.value)
                }
              />
            </label>

            <div className="field settings-form__wide">
              <span className="field__hint">
                This contract is intentionally loopback-only. It improves visibility and local
                control without introducing a remote surface.
              </span>
              <div className="inline-actions">
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => void actions.openAssetEntry("localApiConfig")}
                  disabled={state.openingAssetEntryId === "localApiConfig"}
                >
                  {state.openingAssetEntryId === "localApiConfig"
                    ? "Opening..."
                    : "Open API config"}
                </button>
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => void actions.openDirectory("logsDir")}
                  disabled={state.openingTarget === "logsDir"}
                >
                  {state.openingTarget === "logsDir" ? "Opening..." : "Open runtime logs"}
                </button>
              </div>
            </div>
          </div>
        </Panel>
      </div>

      <div className="page-grid page-grid--two">
        <Panel
          title="Browser Environment Policy"
          subtitle="Local browser shell, storage strategy, extension gates, bookmark seeding, and headless allowance"
          actions={
            <div className="inline-actions">
              <button
                className="button button--secondary"
                type="button"
                onClick={() => actions.resetBrowserEnvironmentDraft()}
              >
                Reset browser draft
              </button>
              <button
                className="button button--secondary"
                type="button"
                onClick={() => void actions.restoreBrowserEnvironmentDefaults()}
                disabled={state.pendingAction !== null}
              >
                {state.pendingAction === "restoreBrowserEnvironment"
                  ? "Restoring..."
                  : "Restore defaults"}
              </button>
              <button
                className="button"
                type="button"
                onClick={() => void actions.applyBrowserEnvironmentDraft()}
                disabled={!browserEnvironmentIsDirty || state.pendingAction !== null}
              >
                {state.pendingAction === "applyBrowserEnvironment"
                  ? "Applying..."
                  : "Apply policy"}
              </button>
            </div>
          }
        >
          {browserEnvironmentSnapshot ? (
            <div className="details-grid details-grid--two">
              <div className="details-grid__item">
                <dt>Environment root</dt>
                <dd>{browserEnvironmentSnapshot.environmentRoot}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Profile workspace</dt>
                <dd>{browserEnvironmentSnapshot.profileWorkspaceDir}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Downloads dir</dt>
                <dd>{browserEnvironmentSnapshot.downloadsDir}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Extensions dir</dt>
                <dd>{browserEnvironmentSnapshot.extensionsDir}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Bookmarks catalog</dt>
                <dd>{browserEnvironmentSnapshot.bookmarksCatalogPath}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Updated</dt>
                <dd>{formatRelativeTimestamp(browserEnvironmentSnapshot.updatedAt)}</dd>
              </div>
            </div>
          ) : (
            <EmptyState
              title="Browser environment policy unavailable"
              detail="Refresh to inspect local browser environment policy and asset roots."
            />
          )}

          <div className="settings-form">
            <label className="field">
              <span className="field__label">Browser family</span>
              <select
                className="field__input"
                value={state.browserEnvironmentDraft.browserFamily}
                onChange={(event) =>
                  actions.updateBrowserEnvironmentDraftField("browserFamily", event.target.value)
                }
              >
                {BROWSER_FAMILY_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span className="field__label">Launch strategy</span>
              <select
                className="field__input"
                value={state.browserEnvironmentDraft.launchStrategy}
                onChange={(event) =>
                  actions.updateBrowserEnvironmentDraftField(
                    "launchStrategy",
                    event.target.value,
                  )
                }
              >
                {BROWSER_LAUNCH_STRATEGY_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span className="field__label">Profile storage</span>
              <select
                className="field__input"
                value={state.browserEnvironmentDraft.profileStorageMode}
                onChange={(event) =>
                  actions.updateBrowserEnvironmentDraftField(
                    "profileStorageMode",
                    event.target.value,
                  )
                }
              >
                {BROWSER_STORAGE_MODE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span className="field__label">Viewport preset</span>
              <select
                className="field__input"
                value={state.browserEnvironmentDraft.defaultViewportPreset}
                onChange={(event) =>
                  actions.updateBrowserEnvironmentDraftField(
                    "defaultViewportPreset",
                    event.target.value,
                  )
                }
              >
                {VIEWPORT_PRESET_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="field">
              <span className="field__label">Keep user data</span>
              {renderBooleanSelect(state.browserEnvironmentDraft.keepUserDataBetweenRuns, (nextValue) =>
                actions.updateBrowserEnvironmentDraftField("keepUserDataBetweenRuns", nextValue),
              )}
            </label>

            <label className="field">
              <span className="field__label">Allow extensions</span>
              {renderBooleanSelect(state.browserEnvironmentDraft.allowExtensions, (nextValue) =>
                actions.updateBrowserEnvironmentDraftField("allowExtensions", nextValue),
              )}
            </label>

            <label className="field">
              <span className="field__label">Allow bookmarks seed</span>
              {renderBooleanSelect(state.browserEnvironmentDraft.allowBookmarksSeed, (nextValue) =>
                actions.updateBrowserEnvironmentDraftField("allowBookmarksSeed", nextValue),
              )}
            </label>

            <label className="field">
              <span className="field__label">Allow archive import</span>
              {renderBooleanSelect(
                state.browserEnvironmentDraft.allowProfileArchiveImport,
                (nextValue) =>
                  actions.updateBrowserEnvironmentDraftField(
                    "allowProfileArchiveImport",
                    nextValue,
                  ),
              )}
            </label>

            <label className="field settings-form__wide">
              <span className="field__label">Headless allowed</span>
              {renderBooleanSelect(state.browserEnvironmentDraft.headlessAllowed, (nextValue) =>
                actions.updateBrowserEnvironmentDraftField("headlessAllowed", nextValue),
              )}
              <span className="field__hint">
                This policy stays local and deliberately frames future profile bootstrap,
                extensions, bookmarks, and recorder/compiler attach points.
              </span>
            </label>
          </div>
        </Panel>

        <Panel
          title="Local Asset Workspace"
          subtitle="Open local entry points for profiles, downloads, extensions, bookmarks, policies, and staged import/export"
        >
          {assetWorkspace ? (
            <div className="contract-list">
              {assetWorkspace.entries.map((entry) => (
                <article className="record-card record-card--compact" key={entry.id}>
                  <div className="record-card__top">
                    <div>
                      <strong>{entry.label}</strong>
                      <p className="record-card__subline">{entry.description}</p>
                    </div>
                    <span className={`badge badge--${getAssetBadgeTone(entry)}`}>
                      {entry.status === "ready" ? "Ready" : "Provision on demand"}
                    </span>
                  </div>
                  <p className="record-card__content record-card__content--muted">{entry.path}</p>
                  <div className="record-card__footer">
                    <span>{formatStatusLabel(entry.kind)}</span>
                    <button
                      className="button button--secondary"
                      type="button"
                      onClick={() => void actions.openAssetEntry(entry.id)}
                      disabled={state.openingAssetEntryId === entry.id}
                    >
                      {state.openingAssetEntryId === entry.id ? "Opening..." : "Open entry"}
                    </button>
                  </div>
                </article>
              ))}
            </div>
          ) : (
            <EmptyState
              title="No local asset workspace yet"
              detail="Refresh to inspect local asset roots for browser environments and import/export staging."
            />
          )}
        </Panel>
      </div>

      <div className="page-grid page-grid--two">
        <Panel
          title="Import / Export Skeleton"
          subtitle="Manifest-first local import and export queue design, ready for future compiler and recorder integrations"
          actions={
            <div className="inline-actions">
              <button
                className="button button--secondary"
                type="button"
                onClick={() => void actions.openAssetEntry("importQueueDir")}
                disabled={state.openingAssetEntryId === "importQueueDir"}
              >
                {state.openingAssetEntryId === "importQueueDir"
                  ? "Opening..."
                  : "Open import queue"}
              </button>
              <button
                className="button button--secondary"
                type="button"
                onClick={() => void actions.openAssetEntry("exportQueueDir")}
                disabled={state.openingAssetEntryId === "exportQueueDir"}
              >
                {state.openingAssetEntryId === "exportQueueDir"
                  ? "Opening..."
                  : "Open export queue"}
              </button>
            </div>
          }
        >
          {importExportSkeleton ? (
            <>
              <div className="details-grid details-grid--two">
                <div className="details-grid__item">
                  <dt>Mode</dt>
                  <dd>{formatStatusLabel(importExportSkeleton.mode)}</dd>
                </div>
                <div className="details-grid__item">
                  <dt>Updated</dt>
                  <dd>{formatRelativeTimestamp(importExportSkeleton.updatedAt)}</dd>
                </div>
                <div className="details-grid__item">
                  <dt>Import manifest</dt>
                  <dd>{importExportSkeleton.importManifestPath}</dd>
                </div>
                <div className="details-grid__item">
                  <dt>Export manifest</dt>
                  <dd>{importExportSkeleton.exportManifestPath}</dd>
                </div>
              </div>

              <div className="contract-list">
                <article className="contract-card">
                  <div className="contract-card__top">
                    <strong>Supported import kinds</strong>
                    <span className="badge badge--info">
                      {importExportSkeleton.supportedImportKinds.length} kinds
                    </span>
                  </div>
                  <p>{importExportSkeleton.supportedImportKinds.join(", ")}</p>
                </article>
                <article className="contract-card">
                  <div className="contract-card__top">
                    <strong>Supported export kinds</strong>
                    <span className="badge badge--info">
                      {importExportSkeleton.supportedExportKinds.length} kinds
                    </span>
                  </div>
                  <p>{importExportSkeleton.supportedExportKinds.join(", ")}</p>
                </article>
                <article className="contract-card">
                  <div className="contract-card__top">
                    <strong>Import manifest fields</strong>
                    <span className="badge badge--info">
                      {importExportSkeleton.importFields.length} fields
                    </span>
                  </div>
                  <p>
                    {importExportSkeleton.importFields
                      .map((field) => `${field.key}${field.required ? " (required)" : ""}`)
                      .join(", ")}
                  </p>
                </article>
                <article className="contract-card">
                  <div className="contract-card__top">
                    <strong>Export bundle fields</strong>
                    <span className="badge badge--info">
                      {importExportSkeleton.exportFields.length} fields
                    </span>
                  </div>
                  <p>
                    {importExportSkeleton.exportFields
                      .map((field) => `${field.key}${field.required ? " (required)" : ""}`)
                      .join(", ")}
                  </p>
                </article>
                <article className="contract-card">
                  <div className="contract-card__top">
                    <strong>Operator notes</strong>
                    <span className="badge badge--warning">Local only</span>
                  </div>
                  <p>{importExportSkeleton.notes.join(" ")}</p>
                </article>
              </div>
            </>
          ) : (
            <EmptyState
              title="No import / export skeleton yet"
              detail="Refresh to inspect the local manifest-first asset workflow."
            />
          )}
        </Panel>

        <Panel
          title="Directory Atlas"
          subtitle="Keep existing open-folder recovery and packaged layout visibility intact"
        >
          {snapshot ? (
            <div className="details-grid details-grid--stacked">
              {LOCAL_DIRECTORY_ITEMS.map((item) => (
                <article className="details-grid__item" key={item.target}>
                  <dt>{item.label}</dt>
                  <dd className="details-grid__value">{item.getValue(snapshot)}</dd>
                  <div className="details-grid__actions">
                    <button
                      className="button button--secondary"
                      type="button"
                      onClick={() => void actions.openDirectory(item.target)}
                      disabled={state.openingTarget === item.target}
                    >
                      {state.openingTarget === item.target ? "Opening..." : "Open folder"}
                    </button>
                  </div>
                </article>
              ))}
              {PACKAGED_DIRECTORY_ITEMS.map((item) => (
                <article className="details-grid__item" key={item.target}>
                  <dt>{item.label}</dt>
                  <dd className="details-grid__value">{item.getValue(snapshot)}</dd>
                  <div className="details-grid__actions">
                    <button
                      className="button button--secondary"
                      type="button"
                      onClick={() => void actions.openDirectory(item.target)}
                      disabled={state.openingTarget === item.target}
                    >
                      {state.openingTarget === item.target ? "Opening..." : "Open folder"}
                    </button>
                  </div>
                </article>
              ))}
              <article className="details-grid__item">
                <dt>Database URL</dt>
                <dd>{snapshot.databaseUrl}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Database path</dt>
                <dd>{snapshot.databasePath}</dd>
              </article>
            </div>
          ) : (
            <EmptyState
              title="No settings loaded"
              detail="Refresh to read the local desktop configuration and packaged layout."
            />
          )}
        </Panel>
      </div>

      <Panel
        title="Contract Status"
        subtitle="The local platform baseline now exposes more real write commands without leaking raw Tauri errors into UI state"
      >
        <div className="contract-list">
          <article className="contract-card">
            <div className="contract-card__top">
              <strong>Runtime policy</strong>
              <span className="badge badge--info">Native ready</span>
            </div>
            <p>
              Apply and restore continue to persist runner, worker, heartbeat, reclaim, and idle
              backoff settings through the shared desktop service layer.
            </p>
          </article>
          <article className="contract-card">
            <div className="contract-card__top">
              <strong>Local API control</strong>
              <span className="badge badge--info">Native ready</span>
            </div>
            <p>
              Loopback host, auth mode, request logging, and max local sessions now have typed
              read/apply/restore contracts and a stable local config file path.
            </p>
          </article>
          <article className="contract-card">
            <div className="contract-card__top">
              <strong>Browser environment policy</strong>
              <span className="badge badge--info">Native ready</span>
            </div>
            <p>
              Browser family, launch strategy, profile storage mode, extension/bookmark gates, and
              headless allowance are now first-class local contracts.
            </p>
          </article>
          <article className="contract-card">
            <div className="contract-card__top">
              <strong>Template compile manifest</strong>
              <span className="badge badge--warning">Local manifest write</span>
            </div>
            <p>
              `compileTemplateRun` now writes a local compile manifest for accepted profiles. It is
              still a local skeleton, but no longer a not-ready wrapper.
            </p>
          </article>
        </div>
      </Panel>
    </div>
  );
}
