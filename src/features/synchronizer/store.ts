import * as desktop from "../../services/desktop";
import { createStore } from "../../store/createStore";
import type {
  DesktopSyncLayoutMode,
  DesktopSyncWindowState,
  DesktopSynchronizerSnapshot,
} from "../../types/desktop";
import { createMockSynchronizerSnapshot } from "./mock";
import {
  buildLayoutDraft,
  cloneSynchronizerSnapshot,
  createInitialCommandCapabilities,
  DEFAULT_SYNCHRONIZER_FILTERS,
  DEFAULT_SYNCHRONIZER_OPERATOR_SETTINGS,
  SYNCHRONIZER_BROADCAST_PLAN_TEMPLATES,
  getExecutionModeLabel,
  getFocusedWindow,
  getMainWindow,
  sortWindowsByOrder,
  type SynchronizerActionFeedItem,
  type SynchronizerActionKind,
  type SynchronizerBroadcastPlanTemplate,
  type SynchronizerCommandCapability,
  type SynchronizerCommandKey,
  type SynchronizerDataSource,
  type SynchronizerExecutionMode,
  type SynchronizerFeedTone,
  type SynchronizerFilterState,
  type SynchronizerLayoutFlag,
  type SynchronizerOperatorSettings,
} from "./model";

type SynchronizerActiveAction = "layout" | "setMain" | "focus" | "broadcastPlan" | null;
type SynchronizerStatTone = "neutral" | "success" | "warning" | "danger";
type SynchronizerNoticeTone = "info" | "warning" | "error";

export interface SynchronizerWindowGroup {
  id: string;
  label: string;
  detail: string;
  windows: DesktopSyncWindowState[];
}

export interface SynchronizerFilterOption {
  value: string;
  label: string;
}

export interface SynchronizerState {
  snapshot: DesktopSynchronizerSnapshot;
  selectedWindowId: string | null;
  dataSource: SynchronizerDataSource;
  isLoading: boolean;
  error: string | null;
  info: string | null;
  requestId: number;
  activeAction: SynchronizerActiveAction;
  autoRefreshEnabled: boolean;
  refreshIntervalMs: number;
  actionFeed: SynchronizerActionFeedItem[];
  filters: SynchronizerFilterState;
  operatorSettings: SynchronizerOperatorSettings;
  stagedBroadcastPlanId: string | null;
  runningBroadcastPlanId: string | null;
  capabilities: Record<SynchronizerCommandKey, SynchronizerCommandCapability>;
}

export interface SynchronizerAttentionItem {
  id: string;
  tone: SynchronizerNoticeTone;
  title: string;
  detail: string;
}

export interface SynchronizerConsoleSummary {
  postureLabel: string;
  postureDetail: string;
  postureTone: SynchronizerStatTone;
  coverageLabel: string;
  coverageDetail: string;
  alignmentLabel: string;
  alignmentDetail: string;
  alignmentTone: SynchronizerStatTone;
  nextActionLabel: string;
  nextActionDetail: string;
  nextActionTone: SynchronizerStatTone;
  cadenceLabel: string;
  cadenceDetail: string;
  attentionItems: SynchronizerAttentionItem[];
}

const INITIAL_SNAPSHOT = createMockSynchronizerSnapshot();

const synchronizerStore = createStore<SynchronizerState>({
  snapshot: INITIAL_SNAPSHOT,
  selectedWindowId:
    INITIAL_SNAPSHOT.focusedWindowId ?? INITIAL_SNAPSHOT.windows[0]?.windowId ?? null,
  dataSource: "mock",
  isLoading: false,
  error: null,
  info: "Synchronizer console is ready. A desktop snapshot will replace the local matrix after the first refresh.",
  requestId: 0,
  activeAction: null,
  autoRefreshEnabled: true,
  refreshIntervalMs: 15000,
  actionFeed: [],
  filters: DEFAULT_SYNCHRONIZER_FILTERS,
  operatorSettings: DEFAULT_SYNCHRONIZER_OPERATOR_SETTINGS,
  stagedBroadcastPlanId: "nav-mirror",
  runningBroadcastPlanId: null,
  capabilities: createInitialCommandCapabilities(),
});

function nowTs(): string {
  return String(Math.floor(Date.now() / 1000));
}

interface SynchronizerBroadcastExecutionRequest {
  planId: string;
  sourceWindowId: string | null;
  targetWindowIds: string[];
  requiredFlags: SynchronizerLayoutFlag[];
  layout: DesktopSynchronizerSnapshot["layout"];
  operatorSettings: SynchronizerOperatorSettings;
  requestedAt: string;
}

interface DesktopSynchronizerBroadcastResult {
  snapshot: DesktopSynchronizerSnapshot;
  message?: string;
  updatedAt?: string;
}

type DesktopSynchronizerBroadcastInvoker = (
  request: SynchronizerBroadcastExecutionRequest,
) => Promise<DesktopSynchronizerBroadcastResult>;

const BROADCAST_CONTRACT_CANDIDATES = [
  "runSyncBroadcast",
  "runSynchronizerBroadcast",
  "applySyncBroadcast",
  "applySynchronizerBroadcast",
  "executeSyncBroadcast",
  "executeSynchronizerBroadcast",
  "broadcastSyncAction",
  "broadcastSyncActions",
] as const;

function resolveBroadcastInvoker(): {
  key: string;
  invoke: DesktopSynchronizerBroadcastInvoker;
} | null {
  const namespace = desktop as unknown as Record<string, unknown>;
  for (const key of BROADCAST_CONTRACT_CANDIDATES) {
    const candidate = namespace[key];
    if (typeof candidate === "function") {
      return {
        key,
        invoke: candidate as DesktopSynchronizerBroadcastInvoker,
      };
    }
  }

  return null;
}

function getBroadcastCapabilityHint(): string {
  const resolved = resolveBroadcastInvoker();
  if (resolved) {
    return `Broadcast contract "${resolved.key}" is exported in this build. Native execution will be used when command readiness checks pass.`;
  }

  return "No broadcast contract is exported in this build yet. Plans remain staged with explicit native-readiness feedback.";
}

function getBroadcastTargetWindows(
  state: SynchronizerState,
  summary: ReturnType<typeof getSynchronizerSummary>,
): DesktopSyncWindowState[] {
  const sourceWindowId =
    summary.mainWindow?.windowId ?? summary.focusedWindow?.windowId ?? state.selectedWindowId;

  return summary.filteredWindows.filter((window) => {
    if (window.status === "missing") {
      return false;
    }
    if (sourceWindowId && window.windowId === sourceWindowId) {
      return false;
    }
    if (state.operatorSettings.stopOnHidden && (!window.isVisible || window.isMinimized)) {
      return false;
    }
    if (state.operatorSettings.respectBusy && window.status === "busy") {
      return false;
    }
    return true;
  });
}

function buildBroadcastRequest(
  state: SynchronizerState,
  summary: ReturnType<typeof getSynchronizerSummary>,
  plan: SynchronizerBroadcastPlanTemplate,
): SynchronizerBroadcastExecutionRequest {
  const sourceWindowId =
    summary.mainWindow?.windowId ?? summary.focusedWindow?.windowId ?? state.selectedWindowId;
  const targetWindowIds = getBroadcastTargetWindows(state, summary).map(
    (window) => window.windowId,
  );

  return {
    planId: plan.id,
    sourceWindowId: sourceWindowId ?? null,
    targetWindowIds,
    requiredFlags: [...plan.requiredFlags],
    layout: cloneSynchronizerSnapshot(state.snapshot).layout,
    operatorSettings: { ...state.operatorSettings },
    requestedAt: nowTs(),
  };
}

function isSynchronizerSnapshot(value: unknown): value is DesktopSynchronizerSnapshot {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as {
    windows?: unknown;
    layout?: unknown;
    updatedAt?: unknown;
  };

  return (
    Array.isArray(candidate.windows) &&
    typeof candidate.layout === "object" &&
    typeof candidate.updatedAt === "string"
  );
}

function createFeedItem(
  kind: SynchronizerActionKind,
  title: string,
  detail: string,
  tone: SynchronizerFeedTone,
  executionMode: SynchronizerExecutionMode,
): SynchronizerActionFeedItem {
  return {
    id: `${kind}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    kind,
    title,
    detail,
    tone,
    executionMode,
    executionLabel: getExecutionModeLabel(executionMode),
    createdAt: nowTs(),
  };
}

function appendFeed(
  items: SynchronizerActionFeedItem[],
  next: SynchronizerActionFeedItem,
): SynchronizerActionFeedItem[] {
  return [next, ...items].slice(0, 12);
}

function updateCapability(
  capabilities: SynchronizerState["capabilities"],
  key: SynchronizerCommandKey,
  status: SynchronizerExecutionMode,
  detail: string,
) {
  return {
    ...capabilities,
    [key]: {
      ...capabilities[key],
      status,
      detail,
      lastUpdatedAt: nowTs(),
    },
  };
}

function patchBroadcastCapabilityProbe(
  capabilities: SynchronizerState["capabilities"],
): SynchronizerState["capabilities"] {
  const current = capabilities.broadcastPlan;
  if (current.status === "native_live") {
    return capabilities;
  }

  return updateCapability(capabilities, "broadcastPlan", "local_staged", getBroadcastCapabilityHint());
}

function setSnapshot(
  state: SynchronizerState,
  snapshot: DesktopSynchronizerSnapshot,
  dataSource: SynchronizerDataSource,
  info: string | null,
  error: string | null,
): SynchronizerState {
  const nextSnapshot = cloneSynchronizerSnapshot(snapshot);
  const selectedWindowId = nextSnapshot.windows.some(
    (window) => window.windowId === state.selectedWindowId,
  )
    ? state.selectedWindowId
    : nextSnapshot.focusedWindowId ?? nextSnapshot.windows[0]?.windowId ?? null;

  return {
    ...state,
    snapshot: nextSnapshot,
    selectedWindowId,
    dataSource,
    isLoading: false,
    info,
    error,
    activeAction: null,
  };
}

function getSelectedWindow(state: SynchronizerState): DesktopSyncWindowState | null {
  return (
    state.snapshot.windows.find((window) => window.windowId === state.selectedWindowId) ?? null
  );
}

function getWindowLabel(window: DesktopSyncWindowState | null, fallback: string): string {
  return window?.profileLabel ?? window?.title ?? window?.windowId ?? fallback;
}

function getActionLabel(action: SynchronizerActiveAction): string {
  if (action === "setMain") {
    return "main window update";
  }

  if (action === "broadcastPlan") {
    return "broadcast execution";
  }

  return action ?? "sync action";
}

function applyPreviewAction(
  kind: Exclude<SynchronizerActiveAction, null>,
  title: string,
  detail: string,
  updater: (snapshot: DesktopSynchronizerSnapshot) => DesktopSynchronizerSnapshot,
  tone: SynchronizerFeedTone,
  info: string,
  error: string | null,
) {
  synchronizerStore.setState((current) => {
    const nextSnapshot = updater(cloneSynchronizerSnapshot(current.snapshot));
    const selectedWindowId = nextSnapshot.windows.some(
      (window) => window.windowId === current.selectedWindowId,
    )
      ? kind === "focus"
        ? nextSnapshot.focusedWindowId ?? current.selectedWindowId
        : current.selectedWindowId
      : nextSnapshot.focusedWindowId ?? nextSnapshot.windows[0]?.windowId ?? null;

    return {
      ...current,
      snapshot: nextSnapshot,
      selectedWindowId,
      activeAction: null,
      info,
      error,
      capabilities: updateCapability(
        current.capabilities,
        kind,
        "local_staged",
        detail,
      ),
      actionFeed: appendFeed(
        current.actionFeed,
        createFeedItem(kind, title, detail, tone, "local_staged"),
      ),
    };
  });
}

async function runSynchronizerAction(
  kind: Exclude<SynchronizerActiveAction, null>,
  fallbackTitle: string,
  fallbackDetail: string,
  invokeAction: () => Promise<DesktopSynchronizerSnapshot>,
  updater: (snapshot: DesktopSynchronizerSnapshot) => DesktopSynchronizerSnapshot,
  options?: {
    preferredSelectedWindowId?: string;
    successInfo?: string;
    successCapabilityDetail?: string;
    successFeedDetail?: string;
    notReadyInfo?: string;
    nativeFailureInfo?: string;
  },
) {
  synchronizerStore.setState((current) => ({
    ...current,
    activeAction: kind,
    error: null,
  }));

  try {
    const snapshot = await invokeAction();
    synchronizerStore.setState((current) => ({
      ...setSnapshot(
        current,
        snapshot,
        "native",
        options?.successInfo ?? "Synchronizer command completed against the native desktop service.",
        null,
      ),
      selectedWindowId:
        options?.preferredSelectedWindowId ??
        (kind === "focus" ? snapshot.focusedWindowId : current.selectedWindowId) ??
        current.selectedWindowId,
      capabilities: updateCapability(
        current.capabilities,
        kind,
        "native_live",
        options?.successCapabilityDetail ??
          "Command completed through the desktop synchronizer contract.",
      ),
      actionFeed: appendFeed(
        current.actionFeed,
        createFeedItem(
          kind,
          fallbackTitle,
          options?.successFeedDetail ?? "Applied through native sync contract.",
          "success",
          "native_live",
        ),
      ),
    }));
  } catch (error) {
    const message =
      error instanceof Error ? error.message : "Failed to apply synchronizer action";
    const current = synchronizerStore.getState();
    const isCommandPending =
      error instanceof desktop.DesktopServiceError &&
      error.code === "desktop_command_not_ready";

    if (isCommandPending || current.dataSource === "mock") {
      applyPreviewAction(
        kind,
        fallbackTitle,
        fallbackDetail,
        updater,
        isCommandPending ? "warning" : "info",
        isCommandPending
          ? options?.notReadyInfo ??
              "This desktop build does not expose the requested sync write command yet. Local console state has been updated so operators can keep staging."
          : options?.nativeFailureInfo ??
              "Native synchronizer action failed, but the local console state is still available for planning.",
        isCommandPending ? null : message,
      );
      return;
    }

    synchronizerStore.setState((state) => ({
      ...state,
      activeAction: null,
      error: message,
      info: "Synchronizer action did not land on the native desktop service.",
      capabilities: updateCapability(
        state.capabilities,
        kind,
        "local_fallback",
        message,
      ),
      actionFeed: appendFeed(
        state.actionFeed,
        createFeedItem(kind, "Synchronizer action failed", message, "error", "local_fallback"),
      ),
    }));
  }
}

function buildPlatformOptions(windows: DesktopSyncWindowState[]): SynchronizerFilterOption[] {
  const values = [...new Set(windows.map((window) => window.platformId).filter(Boolean))];
  return [
    { value: "all", label: "All platforms" },
    ...values.map((value) => ({ value, label: value })),
  ];
}

function buildStatusOptions(windows: DesktopSyncWindowState[]): SynchronizerFilterOption[] {
  const values = [...new Set(windows.map((window) => window.status))];
  return [
    { value: "all", label: "All states" },
    ...values.map((value) => ({ value, label: value.replaceAll("_", " ") })),
  ];
}

function matchesVisibility(
  window: DesktopSyncWindowState,
  filter: SynchronizerFilterState["visibilityFilter"],
) {
  if (filter === "visible") {
    return window.isVisible && !window.isMinimized;
  }

  if (filter === "hidden") {
    return !window.isVisible || window.isMinimized;
  }

  if (filter === "attention") {
    return (
      window.status === "busy" ||
      window.status === "missing" ||
      window.isMinimized ||
      !window.isVisible
    );
  }

  return true;
}

function matchesRole(
  window: DesktopSyncWindowState,
  state: SynchronizerState,
  filter: SynchronizerFilterState["roleFilter"],
) {
  if (filter === "main") {
    return window.isMainWindow;
  }

  if (filter === "focused") {
    return window.isFocused;
  }

  if (filter === "controlled") {
    return !window.isMainWindow && window.status !== "missing";
  }

  if (filter === "attention") {
    return window.status === "busy" || window.status === "missing" || window.isMinimized;
  }

  if (filter === "selected") {
    return window.windowId === state.selectedWindowId;
  }

  return true;
}

function groupWindows(
  windows: DesktopSyncWindowState[],
  groupBy: SynchronizerFilterState["groupBy"],
): SynchronizerWindowGroup[] {
  if (groupBy === "none") {
    return [
      {
        id: "all",
        label: "Filtered scope",
        detail: `${windows.length} windows in current operator scope`,
        windows,
      },
    ];
  }

  const bucketMap = new Map<string, SynchronizerWindowGroup>();

  windows.forEach((window) => {
    const key =
      groupBy === "platform"
        ? window.platformId ?? "unassigned"
        : groupBy === "status"
          ? window.status
          : window.isVisible && !window.isMinimized
            ? "visible"
            : "hidden";

    if (!bucketMap.has(key)) {
      bucketMap.set(key, {
        id: key,
        label: key.replaceAll("_", " "),
        detail:
          groupBy === "platform"
            ? "Platform lane"
            : groupBy === "status"
              ? "Runtime state lane"
              : key === "visible"
                ? "Visible to operator"
                : "Hidden or minimized",
        windows: [],
      });
    }

    bucketMap.get(key)?.windows.push(window);
  });

  return [...bucketMap.values()].map((group) => ({
    ...group,
    windows: sortWindowsByOrder(group.windows),
    detail: `${group.detail} - ${group.windows.length} windows`,
  }));
}

export const synchronizerActions = {
  async refresh() {
    const requestId = synchronizerStore.getState().requestId + 1;
    synchronizerStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
    }));

    try {
      const snapshot = await desktop.readSynchronizerSnapshot();
      if (synchronizerStore.getState().requestId !== requestId) {
        return;
      }

      synchronizerStore.setState((current) => ({
        ...setSnapshot(
          current,
          snapshot,
          "native",
          "Live synchronizer snapshot loaded from the desktop service.",
          null,
        ),
        capabilities: patchBroadcastCapabilityProbe(
          updateCapability(
            current.capabilities,
            "readSnapshot",
            "native_live",
            "Reading live desktop synchronizer snapshots.",
          ),
        ),
        actionFeed: appendFeed(
          current.actionFeed,
          createFeedItem(
            "refresh",
            "Synchronizer refreshed",
            "Native window matrix and layout state were refreshed.",
            "success",
            "native_live",
          ),
        ),
      }));
    } catch (error) {
      if (synchronizerStore.getState().requestId !== requestId) {
        return;
      }

      const message =
        error instanceof Error ? error.message : "Failed to refresh synchronizer";
      const current = synchronizerStore.getState();
      const isCommandPending =
        error instanceof desktop.DesktopServiceError &&
        error.code === "desktop_command_not_ready";

      synchronizerStore.setState((state) => ({
        ...setSnapshot(
          state,
          current.snapshot,
          current.dataSource,
          isCommandPending
            ? "This desktop build does not expose synchronizer read contracts yet. The local matrix remains available."
            : current.dataSource === "native"
              ? "Native refresh failed. The last successful desktop snapshot is being kept visible so operators do not fall back to decorative mock data."
              : "Native refresh failed. The local matrix is being kept available for operator review.",
          isCommandPending ? null : message,
        ),
        capabilities: patchBroadcastCapabilityProbe(
          updateCapability(
            state.capabilities,
            "readSnapshot",
            current.dataSource === "native" ? "native_live" : "local_fallback",
            isCommandPending
              ? "Desktop read contract is not wired yet. Local snapshot fallback is active."
              : current.dataSource === "native"
                ? "Latest refresh failed, but the last successful native snapshot is still shown."
                : message,
          ),
        ),
        actionFeed: appendFeed(
          state.actionFeed,
          createFeedItem(
            "refresh",
            isCommandPending
              ? "Synchronizer read contract missing"
              : current.dataSource === "native"
                ? "Synchronizer native snapshot kept"
                : "Synchronizer fallback active",
            isCommandPending
              ? "Using the local window matrix because no desktop synchronizer snapshot is available in this build."
              : current.dataSource === "native"
                ? "Native refresh failed, so the last successful desktop snapshot remains visible."
                : message,
            isCommandPending ? "warning" : "error",
            current.dataSource === "native" ? "native_live" : "local_fallback",
          ),
        ),
      }));
    }
  },
  selectWindow(windowId: string) {
    synchronizerStore.setState((current) => ({
      ...current,
      selectedWindowId: windowId,
    }));
  },
  setSearchText(searchText: string) {
    synchronizerStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        searchText,
      },
    }));
  },
  setPlatformFilter(platformFilter: string) {
    synchronizerStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        platformFilter,
      },
    }));
  },
  setStatusFilter(statusFilter: string) {
    synchronizerStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        statusFilter,
      },
    }));
  },
  setVisibilityFilter(visibilityFilter: SynchronizerFilterState["visibilityFilter"]) {
    synchronizerStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        visibilityFilter,
      },
    }));
  },
  setRoleFilter(roleFilter: SynchronizerFilterState["roleFilter"]) {
    synchronizerStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        roleFilter,
      },
    }));
  },
  setGroupBy(groupBy: SynchronizerFilterState["groupBy"]) {
    synchronizerStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        groupBy,
      },
    }));
  },
  resetFilters() {
    synchronizerStore.setState((current) => ({
      ...current,
      filters: DEFAULT_SYNCHRONIZER_FILTERS,
    }));
  },
  stageBroadcastPlan(plan: SynchronizerBroadcastPlanTemplate) {
    synchronizerStore.setState((current) => {
      const summary = getSynchronizerSummary(current);
      const sourceWindowLabel = getWindowLabel(summary.mainWindow ?? summary.focusedWindow, "source not pinned");
      const targetCount = getBroadcastTargetWindows(current, summary).length;
      const missingFlags = plan.requiredFlags.filter((flag) => !current.snapshot.layout[flag]);
      const missingFlagsLabel = missingFlags.join(" / ");
      const capabilityHint = getBroadcastCapabilityHint();

      return {
        ...current,
        stagedBroadcastPlanId: plan.id,
        runningBroadcastPlanId: null,
        info:
          missingFlags.length > 0
            ? `${plan.title} is prepared for ${targetCount} windows, but ${missingFlagsLabel} is currently disabled.`
            : `${plan.title} is prepared for ${targetCount} windows. ${capabilityHint}`,
        capabilities: updateCapability(
          current.capabilities,
          "broadcastPlan",
          "local_staged",
          `${plan.title} prepared for ${targetCount} target windows with ${sourceWindowLabel} as source context. ${capabilityHint}`,
        ),
        actionFeed: appendFeed(
          current.actionFeed,
          createFeedItem(
            "broadcastPlan",
            `${plan.title} prepared`,
            missingFlags.length > 0
              ? `${plan.scopeLabel} - ${targetCount} windows in scope - enable ${missingFlagsLabel} before execution.`
              : `${plan.scopeLabel} - ${targetCount} windows in scope - native execution will be attempted when available.`,
            "warning",
            "local_staged",
          ),
        ),
      };
    });
  },
  async runBroadcastPlan(planId?: string) {
    const state = synchronizerStore.getState();
    const resolvedPlanId = planId ?? state.stagedBroadcastPlanId;
    const plan =
      SYNCHRONIZER_BROADCAST_PLAN_TEMPLATES.find((item) => item.id === resolvedPlanId) ?? null;

    if (!plan) {
      synchronizerStore.setState((current) => ({
        ...current,
        info: "Select a broadcast plan before execution.",
        actionFeed: appendFeed(
          current.actionFeed,
          createFeedItem(
            "broadcastPlan",
            "Broadcast plan missing",
            "No broadcast plan is selected, so execution was skipped.",
            "warning",
            "local_staged",
          ),
        ),
      }));
      return;
    }

    const summary = getSynchronizerSummary(state);
    const missingFlags = plan.requiredFlags.filter((flag) => !state.snapshot.layout[flag]);
    if (missingFlags.length > 0) {
      const missingFlagsLabel = missingFlags.join(" / ");
      synchronizerStore.setState((current) => ({
        ...current,
        stagedBroadcastPlanId: plan.id,
        runningBroadcastPlanId: null,
        info: `${plan.title} is blocked until ${missingFlagsLabel} is enabled.`,
        capabilities: updateCapability(
          current.capabilities,
          "broadcastPlan",
          "local_staged",
          `${plan.title} is prepared but blocked by disabled flags: ${missingFlagsLabel}.`,
        ),
        actionFeed: appendFeed(
          current.actionFeed,
          createFeedItem(
            "broadcastPlan",
            `${plan.title} blocked`,
            `Enable ${missingFlagsLabel} to execute ${plan.title}.`,
            "warning",
            "local_staged",
          ),
        ),
      }));
      return;
    }

    const request = buildBroadcastRequest(state, summary, plan);
    if (request.targetWindowIds.length === 0) {
      synchronizerStore.setState((current) => ({
        ...current,
        stagedBroadcastPlanId: plan.id,
        runningBroadcastPlanId: null,
        info:
          "No eligible target windows remain after current visibility and busy safeguards were applied.",
        capabilities: updateCapability(
          current.capabilities,
          "broadcastPlan",
          "local_staged",
          `${plan.title} is prepared, but current safeguards reduced target count to zero.`,
        ),
        actionFeed: appendFeed(
          current.actionFeed,
          createFeedItem(
            "broadcastPlan",
            `${plan.title} skipped`,
            "No eligible target windows matched the current scope and safeguards.",
            "warning",
            "local_staged",
          ),
        ),
      }));
      return;
    }

    const resolved = resolveBroadcastInvoker();
    synchronizerStore.setState((current) => ({
      ...current,
      stagedBroadcastPlanId: plan.id,
      runningBroadcastPlanId: plan.id,
    }));

    await runSynchronizerAction(
      "broadcastPlan",
      "Broadcast execution completed",
      `${plan.title} is prepared for ${request.targetWindowIds.length} target windows while native broadcast write is unavailable.`,
      async () => {
        if (!resolved) {
          throw new desktop.DesktopServiceError(
            "TODO: runBroadcastPlan native contract is not implemented yet.",
            "desktop_command_not_ready",
          );
        }

        const result = await resolved.invoke(request);
        if (!isSynchronizerSnapshot(result?.snapshot)) {
          throw new desktop.DesktopServiceError(
            "Native broadcast contract returned an invalid synchronizer snapshot payload.",
            "desktop_error",
            result,
          );
        }
        return result.snapshot;
      },
      (snapshot) => ({
        ...snapshot,
        updatedAt: nowTs(),
      }),
      {
        successInfo: resolved
          ? `${plan.title} executed through ${resolved.key} for ${request.targetWindowIds.length} target windows.`
          : undefined,
        successCapabilityDetail: resolved
          ? `Native broadcast contract "${resolved.key}" executed for ${request.targetWindowIds.length} targets.`
          : undefined,
        successFeedDetail: resolved
          ? `${plan.scopeLabel} - ${request.targetWindowIds.length} targets - native broadcast write succeeded.`
          : undefined,
        notReadyInfo:
          "Native broadcast contract is not exposed in this build yet. The prepared plan remains available for later execution.",
        nativeFailureInfo:
          "Native broadcast execution failed. The prepared plan and current snapshot were kept for retry.",
      },
    );

    synchronizerStore.setState((current) => ({
      ...current,
      runningBroadcastPlanId: null,
      capabilities: patchBroadcastCapabilityProbe(current.capabilities),
    }));
  },
  setAutoRefreshEnabled(autoRefreshEnabled: boolean) {
    synchronizerStore.setState((current) => {
      if (current.autoRefreshEnabled === autoRefreshEnabled) {
        return current;
      }

      return {
        ...current,
        autoRefreshEnabled,
        info: autoRefreshEnabled
          ? `Auto refresh resumed at ${Math.round(current.refreshIntervalMs / 1000)}s cadence.`
          : "Auto refresh paused. The matrix will stay on the last snapshot until you refresh manually.",
        actionFeed: appendFeed(
          current.actionFeed,
          createFeedItem(
            "refresh",
            autoRefreshEnabled ? "Auto refresh enabled" : "Auto refresh paused",
            autoRefreshEnabled
              ? "Synchronizer will keep polling the local desktop snapshot in the background."
              : "Synchronizer will stay on the current snapshot until an operator refreshes it.",
            autoRefreshEnabled ? "success" : "warning",
            current.capabilities.readSnapshot.status,
          ),
        ),
      };
    });
  },
  setRefreshIntervalMs(refreshIntervalMs: number) {
    synchronizerStore.setState((current) => {
      if (current.refreshIntervalMs === refreshIntervalMs) {
        return current;
      }

      return {
        ...current,
        refreshIntervalMs,
        info: `Auto refresh cadence updated to ${Math.round(refreshIntervalMs / 1000)}s.`,
        actionFeed: appendFeed(
          current.actionFeed,
          createFeedItem(
            "settings",
            "Refresh cadence updated",
            `Synchronizer polling cadence is now ${Math.round(refreshIntervalMs / 1000)} seconds.`,
            "info",
            current.capabilities.readSnapshot.status,
          ),
        ),
      };
    });
  },
  setOperatorSetting<K extends keyof SynchronizerOperatorSettings>(
    key: K,
    value: SynchronizerOperatorSettings[K],
  ) {
    synchronizerStore.setState((current) => {
      if (current.operatorSettings[key] === value) {
        return current;
      }

      return {
        ...current,
        operatorSettings: {
          ...current.operatorSettings,
          [key]: value,
        },
        info: `Execution setting updated: ${key}.`,
        actionFeed: appendFeed(
          current.actionFeed,
          createFeedItem(
            "settings",
            "Sync execution setting updated",
            `${String(key)} is now ${String(value)}. This affects broadcast payload generation and local safeguards.`,
            "info",
            "local_staged",
          ),
        ),
      };
    });
  },
  async setLayoutMode(mode: DesktopSyncLayoutMode) {
    const current = synchronizerStore.getState();
    const previewLayout = buildLayoutDraft(
      current.snapshot.layout,
      mode,
      current.snapshot.windows.length,
      nowTs(),
    );

    await runSynchronizerAction(
      "layout",
      "Layout updated",
      `Layout switched to ${mode.replaceAll("_", " ")} in the local console state.`,
      async () => {
        const result = await desktop.applyWindowLayout({
          mode: previewLayout.mode,
          columns: previewLayout.columns,
          rows: previewLayout.rows,
          gapPx: previewLayout.gapPx,
          overlapOffsetX: previewLayout.overlapOffsetX,
          overlapOffsetY: previewLayout.overlapOffsetY,
          uniformWidth: previewLayout.uniformWidth,
          uniformHeight: previewLayout.uniformHeight,
        });
        return result.snapshot;
      },
      (snapshot) => ({
        ...snapshot,
        layout: buildLayoutDraft(snapshot.layout, mode, snapshot.windows.length, nowTs()),
        updatedAt: nowTs(),
      }),
      {
        successCapabilityDetail:
          "Layout preset was written through the synchronizer internal-state contract (state write only; not physical window rearrangement).",
        successFeedDetail:
          "Synchronizer internal layout state updated (no physical window rearrangement).",
      },
    );
  },
  async setLayoutSyncFlag(flag: SynchronizerLayoutFlag, value: boolean) {
    const label =
      flag === "syncScroll"
        ? "scroll sync"
        : flag === "syncNavigation"
          ? "navigation sync"
          : "input sync";

    await runSynchronizerAction(
      "layout",
      "Sync guardrail updated",
      `Updated ${label} to ${value ? "on" : "off"} in the local console state.`,
      async () => {
        const result = await desktop.applyWindowLayout({
          [flag]: value,
        });
        return result.snapshot;
      },
      (snapshot) => ({
        ...snapshot,
        layout: {
          ...snapshot.layout,
          [flag]: value,
          updatedAt: nowTs(),
        },
        updatedAt: nowTs(),
      }),
      {
        successCapabilityDetail:
          "Sync guardrails were written through synchronizer internal-state contract (state write only).",
        successFeedDetail:
          "Synchronizer internal layout guardrail updated (state write only).",
      },
    );
  },
  async setMainWindow(windowId: string) {
    await runSynchronizerAction(
      "setMain",
      "Main window updated",
      "Main-window anchor was staged locally because this desktop build cannot apply the synchronizer state write command.",
      async () => {
        const result = await desktop.setMainSyncWindow(windowId);
        return result.snapshot;
      },
      (snapshot) => ({
        ...snapshot,
        windows: sortWindowsByOrder(
          snapshot.windows.map((window) => ({
            ...window,
            isMainWindow: window.windowId === windowId,
          })),
        ),
        layout: {
          ...snapshot.layout,
          mainWindowId: windowId,
          updatedAt: nowTs(),
        },
        updatedAt: nowTs(),
      }),
      {
        successCapabilityDetail:
          "Main-window anchor was written through synchronizer internal-state contract.",
        successFeedDetail: "Synchronizer internal main-window anchor updated.",
      },
    );
  },
  async focusWindow(windowId: string) {
    await runSynchronizerAction(
      "focus",
      "Focus target updated",
      "Selected window is now the local focus target while native focus write is unavailable.",
      async () => {
        const result = await desktop.focusSyncWindow(windowId);
        return result.snapshot;
      },
      (snapshot) => ({
        ...snapshot,
        windows: sortWindowsByOrder(
          snapshot.windows.map((window) => ({
            ...window,
            isFocused: window.windowId === windowId,
            status:
              window.windowId === windowId
                ? "focused"
                : window.status === "focused"
                  ? "ready"
                  : window.status,
          })),
        ),
        focusedWindowId: windowId,
        updatedAt: nowTs(),
      }),
      {
        preferredSelectedWindowId: windowId,
        successCapabilityDetail: "Focus write reached native Win32 focus control.",
        successFeedDetail: "Focus target updated through native Win32 focus control.",
      },
    );
  },
};

export function getSynchronizerSummary(state: SynchronizerState) {
  const windows = sortWindowsByOrder(state.snapshot.windows);
  const mainWindow = getMainWindow(windows);
  const focusedWindow = getFocusedWindow(windows);
  const searchText = state.filters.searchText.trim().toLowerCase();
  const filteredWindows = windows.filter((window) => {
    const searchBlob = [
      window.profileLabel,
      window.title,
      window.windowId,
      window.storeId,
      window.platformId,
    ]
      .filter(Boolean)
      .join(" ")
      .toLowerCase();

    if (searchText && !searchBlob.includes(searchText)) {
      return false;
    }

    if (
      state.filters.platformFilter !== "all" &&
      window.platformId !== state.filters.platformFilter
    ) {
      return false;
    }

    if (state.filters.statusFilter !== "all" && window.status !== state.filters.statusFilter) {
      return false;
    }

    if (!matchesVisibility(window, state.filters.visibilityFilter)) {
      return false;
    }

    return matchesRole(window, state, state.filters.roleFilter);
  });

  return {
    windows,
    filteredWindows,
    groupedWindows: groupWindows(filteredWindows, state.filters.groupBy),
    selectedWindow: getSelectedWindow(state),
    mainWindow,
    focusedWindow,
    visibleCount: windows.filter((window) => window.isVisible).length,
    minimizedCount: windows.filter((window) => window.isMinimized).length,
    missingCount: windows.filter((window) => window.status === "missing").length,
    busyCount: windows.filter((window) => window.status === "busy").length,
    controlledCount: windows.filter((window) => !window.isMainWindow && window.status !== "missing")
      .length,
    filterCount: filteredWindows.length,
    platformOptions: buildPlatformOptions(windows),
    statusOptions: buildStatusOptions(windows),
  };
}

export function getSynchronizerConsoleSummary(
  state: SynchronizerState,
  summary: ReturnType<typeof getSynchronizerSummary>,
): SynchronizerConsoleSummary {
  const totalCount = summary.windows.length;
  const attentionItems: SynchronizerAttentionItem[] = [];
  const mainFocusDrift =
    summary.mainWindow &&
    summary.focusedWindow &&
    summary.mainWindow.windowId !== summary.focusedWindow.windowId;
  const selectionFocusDrift =
    summary.selectedWindow &&
    summary.focusedWindow &&
    summary.selectedWindow.windowId !== summary.focusedWindow.windowId;

  if (state.dataSource === "mock") {
    attentionItems.push({
      id: "preview",
      tone: "warning",
      title: "Local matrix fallback is active",
      detail:
        "Live synchronizer snapshot is unavailable in the current session. Operators can still prepare layout/main/focus writes and broadcast payloads with explicit capability feedback.",
    });
  }

  if (summary.missingCount > 0) {
    attentionItems.push({
      id: "missing",
      tone: "error",
      title: `${summary.missingCount} windows are missing from the native surface`,
      detail:
        "These sessions no longer expose a native handle. Refresh after relaunching or reattaching the underlying profile window.",
    });
  }

  if (summary.busyCount > 0) {
    attentionItems.push({
      id: "busy",
      tone: "warning",
      title: `${summary.busyCount} windows are busy`,
      detail:
        "Busy sessions may ignore focus or layout changes until their current work settles. Treat them as hot lanes.",
    });
  }

  if (!summary.mainWindow && totalCount > 0) {
    attentionItems.push({
      id: "main-window",
      tone: "warning",
      title: "Primary sync driver is not set",
      detail:
        "Pick one main window so the whole team sees which session is supposed to lead sync actions and manual review.",
    });
  }

  if (state.runningBroadcastPlanId) {
    attentionItems.push({
      id: "broadcast-running",
      tone: "info",
      title: "Broadcast execution is in progress",
      detail:
        "The synchronizer is waiting for a broadcast execution result from the selected plan.",
    });
  } else if (state.stagedBroadcastPlanId) {
    attentionItems.push({
      id: "broadcast-plan",
      tone: "info",
      title:
        state.capabilities.broadcastPlan.status === "native_live"
          ? "Broadcast path is native-ready in this session"
          : "A broadcast action plan is prepared",
      detail:
        state.capabilities.broadcastPlan.status === "native_live"
          ? "At least one broadcast execution has already landed through a native contract in this session."
          : "The selected broadcast plan is prepared. Execution will auto-switch to native once a typed contract is exposed and ready.",
    });
  }

  if (mainFocusDrift) {
    attentionItems.push({
      id: "focus-drift",
      tone: "info",
      title: "Main window and focused window are different",
      detail: `${getWindowLabel(summary.mainWindow, "Main window")} is marked as main, while ${getWindowLabel(summary.focusedWindow, "Focused window")} currently owns native focus.`,
    });
  } else if (selectionFocusDrift) {
    attentionItems.push({
      id: "selection-drift",
      tone: "info",
      title: "Console selection does not match native focus",
      detail: `${getWindowLabel(summary.selectedWindow, "Selected window")} is selected in the console, but another window currently owns native focus.`,
    });
  }

  if (state.error) {
    attentionItems.unshift({
      id: "action-error",
      tone: "error",
      title: "Latest sync command failed",
      detail: state.error,
    });
  }

  let postureLabel = "Stable";
  let postureDetail =
    "Window roles, focus, broadcast plan readiness, and action feedback look healthy for routine operator work.";
  let postureTone: SynchronizerStatTone = "success";

  if (state.activeAction) {
    postureLabel = "Applying";
    postureDetail = `The console is waiting for a ${getActionLabel(state.activeAction)} result.`;
    postureTone = "neutral";
  } else if (state.error || summary.missingCount > 0) {
    postureLabel = "Action Needed";
    postureDetail = "One or more windows need recovery or the latest sync command did not land cleanly.";
    postureTone = "danger";
  } else if (state.dataSource === "mock" || summary.busyCount > 0 || mainFocusDrift) {
    postureLabel = "Watch Closely";
    postureDetail =
      "The console is usable, but fallback mode, hot sessions, or focus drift still require operator attention.";
    postureTone = "warning";
  }

  let alignmentLabel = "Aligned";
  let alignmentDetail =
    "Main window, selection, and native focus are pointing to the same operating lane.";
  let alignmentTone: SynchronizerStatTone = "success";

  if (!summary.mainWindow) {
    alignmentLabel = "No Main Window";
    alignmentDetail = "A primary sync leader has not been assigned yet.";
    alignmentTone = "warning";
  } else if (mainFocusDrift) {
    alignmentLabel = "Focus Drift";
    alignmentDetail = `${getWindowLabel(summary.mainWindow, "Main window")} leads sync, but ${getWindowLabel(summary.focusedWindow, "Focused window")} currently has focus.`;
    alignmentTone = "warning";
  } else if (selectionFocusDrift) {
    alignmentLabel = "Selection Drift";
    alignmentDetail =
      "The console selection is looking at a different window than the native focus target.";
    alignmentTone = "neutral";
  }

  const selectedWindow = summary.selectedWindow;
  let nextActionLabel = "Pick a window";
  let nextActionDetail = "Select a card in the matrix to stage focus or main-window changes.";
  let nextActionTone: SynchronizerStatTone = "warning";

  if (selectedWindow) {
    if (selectedWindow.status === "missing") {
      nextActionLabel = "Recover missing window";
      nextActionDetail =
        "This profile is no longer attached to a live native window. Refresh after the profile is reopened.";
      nextActionTone = "danger";
    } else if (selectedWindow.status === "busy") {
      nextActionLabel = "Wait for busy session";
      nextActionDetail =
        "The selected window is in the middle of work. Avoid aggressive focus or layout moves until it settles.";
      nextActionTone = "warning";
    } else if (!selectedWindow.isFocused) {
      nextActionLabel = "Focus selected window";
      nextActionDetail =
        "Bring the selected window to the front before validating layout, sync drift, or manual replay behavior.";
      nextActionTone = "warning";
    } else if (!selectedWindow.isMainWindow) {
      nextActionLabel = "Decide if this should be main";
      nextActionDetail =
        "The selected window is focused and visible. Promote it to main if it should lead future sync operations.";
      nextActionTone = "neutral";
    } else if (!selectedWindow.isVisible || selectedWindow.isMinimized) {
      nextActionLabel = "Restore visibility";
      nextActionDetail =
        "The selected main window is hidden or minimized. Restore it before relying on it as the sync driver.";
      nextActionTone = "warning";
    } else {
      nextActionLabel = "Window is ready";
      nextActionDetail =
        "The selected window is visible, focused, and already acting as the primary sync lane.";
      nextActionTone = "success";
    }
  }

  return {
    postureLabel,
    postureDetail,
    postureTone,
    coverageLabel: `${summary.filterCount}/${totalCount} in scope`,
    coverageDetail: `${summary.visibleCount} visible / ${summary.controlledCount} controlled / ${summary.missingCount} missing / ${summary.busyCount} busy`,
    alignmentLabel,
    alignmentDetail,
    alignmentTone,
    nextActionLabel,
    nextActionDetail,
    nextActionTone,
    cadenceLabel: state.autoRefreshEnabled
      ? `${Math.round(state.refreshIntervalMs / 1000)}s auto`
      : "Manual refresh",
    cadenceDetail: state.autoRefreshEnabled
      ? "The console keeps polling window state in the background."
      : "Snapshot updates only land when an operator refreshes manually.",
    attentionItems,
  };
}

synchronizerStore.setState((current) => ({
  ...current,
  capabilities: patchBroadcastCapabilityProbe(current.capabilities),
  actionFeed: appendFeed(
    current.actionFeed,
    createFeedItem(
      "refresh",
      "Synchronizer console ready",
      "Local window matrix is standing by until the first desktop synchronizer snapshot loads.",
      "warning",
      "local_fallback",
    ),
  ),
}));

export { synchronizerStore };
