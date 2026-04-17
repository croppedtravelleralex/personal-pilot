import type {
  DesktopSyncLayoutMode,
  DesktopSyncLayoutState,
  DesktopSyncWindowState,
  DesktopSynchronizerBroadcastChannel,
  DesktopSynchronizerSnapshot,
} from "../../types/desktop";

export type SynchronizerDataSource = "native" | "mock";

export type SynchronizerActionKind =
  | "refresh"
  | "layout"
  | "setMain"
  | "focus"
  | "broadcastPlan"
  | "settings";

export type SynchronizerFeedTone = "info" | "success" | "warning" | "error";

export type SynchronizerExecutionMode =
  | "native_live"
  | "local_staged"
  | "local_fallback";

export type SynchronizerLayoutApplyResult =
  | "applied"
  | "partial"
  | "failed"
  | "intention_only";

export type SynchronizerCommandKey =
  | "readSnapshot"
  | "layout"
  | "setMain"
  | "focus"
  | "broadcastPlan";

export type SynchronizerWindowRoleFilter =
  | "all"
  | "main"
  | "focused"
  | "controlled"
  | "attention"
  | "selected";

export type SynchronizerVisibilityFilter = "all" | "visible" | "hidden" | "attention";

export type SynchronizerWindowGroupBy =
  | "none"
  | "platform"
  | "status"
  | "visibility";

export type SynchronizerTargetScreen = "auto" | "primary" | "extended";

export interface SynchronizerActionFeedItem {
  id: string;
  kind: SynchronizerActionKind;
  title: string;
  detail: string;
  tone: SynchronizerFeedTone;
  executionMode: SynchronizerExecutionMode;
  executionLabel: string;
  createdAt: string;
}

export interface SynchronizerLayoutOption {
  value: DesktopSyncLayoutMode;
  label: string;
  detail: string;
}

export interface SynchronizerRefreshIntervalOption {
  value: number;
  label: string;
}

export interface SynchronizerFilterState {
  searchText: string;
  platformFilter: string;
  statusFilter: string;
  visibilityFilter: SynchronizerVisibilityFilter;
  roleFilter: SynchronizerWindowRoleFilter;
  groupBy: SynchronizerWindowGroupBy;
}

export interface SynchronizerOperatorSettings {
  clickDelayMs: number;
  typingDelayMs: number;
  stopOnHidden: boolean;
  respectBusy: boolean;
  targetScreen: SynchronizerTargetScreen;
}

export type SynchronizerLayoutFlag = "syncNavigation" | "syncInput" | "syncScroll";

export interface SynchronizerBroadcastPlanTemplate {
  id: string;
  channel: DesktopSynchronizerBroadcastChannel;
  title: string;
  detail: string;
  scopeLabel: string;
  intensity: "safe" | "normal" | "strong";
  requiredFlags: SynchronizerLayoutFlag[];
}

export interface SynchronizerCommandCapability {
  key: SynchronizerCommandKey;
  label: string;
  status: SynchronizerExecutionMode;
  detail: string;
  lastUpdatedAt: string | null;
}

export const DEFAULT_SYNCHRONIZER_FILTERS: SynchronizerFilterState = {
  searchText: "",
  platformFilter: "all",
  statusFilter: "all",
  visibilityFilter: "all",
  roleFilter: "all",
  groupBy: "platform",
};

export const DEFAULT_SYNCHRONIZER_OPERATOR_SETTINGS: SynchronizerOperatorSettings = {
  clickDelayMs: 280,
  typingDelayMs: 140,
  stopOnHidden: true,
  respectBusy: true,
  targetScreen: "auto",
};

export const SYNCHRONIZER_LAYOUT_OPTIONS: SynchronizerLayoutOption[] = [
  {
    value: "grid",
    label: "Grid",
    detail: "Keep windows in a stable matrix for scanning and batch operation.",
  },
  {
    value: "overlap",
    label: "Overlap",
    detail: "Stack windows with a small offset to review multiple live sessions quickly.",
  },
  {
    value: "uniform_size",
    label: "Uniform Size",
    detail: "Normalize viewport size before replay or visual comparison.",
  },
];

export const SYNCHRONIZER_REFRESH_INTERVAL_OPTIONS: SynchronizerRefreshIntervalOption[] = [
  { value: 5000, label: "5s" },
  { value: 15000, label: "15s" },
  { value: 30000, label: "30s" },
  { value: 60000, label: "60s" },
];

export const SYNCHRONIZER_GROUP_BY_OPTIONS: Array<{
  value: SynchronizerWindowGroupBy;
  label: string;
}> = [
  { value: "platform", label: "Group by platform" },
  { value: "status", label: "Group by status" },
  { value: "visibility", label: "Group by visibility" },
  { value: "none", label: "Flat list" },
];

export const SYNCHRONIZER_ROLE_FILTER_OPTIONS: Array<{
  value: SynchronizerWindowRoleFilter;
  label: string;
}> = [
  { value: "all", label: "All lanes" },
  { value: "main", label: "Main only" },
  { value: "focused", label: "Focused only" },
  { value: "controlled", label: "Controlled members" },
  { value: "attention", label: "Needs attention" },
  { value: "selected", label: "Selected lane" },
];

export const SYNCHRONIZER_VISIBILITY_FILTER_OPTIONS: Array<{
  value: SynchronizerVisibilityFilter;
  label: string;
}> = [
  { value: "all", label: "All visibility" },
  { value: "visible", label: "Visible only" },
  { value: "hidden", label: "Hidden/minimized" },
  { value: "attention", label: "Attention only" },
];

export const SYNCHRONIZER_TARGET_SCREEN_OPTIONS: Array<{
  value: SynchronizerTargetScreen;
  label: string;
}> = [
  { value: "auto", label: "Auto" },
  { value: "primary", label: "Primary screen" },
  { value: "extended", label: "Extended screen" },
];

export const SYNCHRONIZER_BROADCAST_PLAN_TEMPLATES: SynchronizerBroadcastPlanTemplate[] = [
  {
    id: "nav-mirror",
    channel: "navigation",
    title: "Mirror navigation from main",
    detail:
      "Run navigation synchronization from the main window to the current controlled scope when navigation sync is enabled.",
    scopeLabel: "Main -> controlled windows",
    intensity: "safe",
    requiredFlags: ["syncNavigation"],
  },
  {
    id: "input-burst",
    channel: "input",
    title: "Shared input burst",
    detail:
      "Apply synchronized input pacing from the focused or main lane using the current operator click/typing safeguards.",
    scopeLabel: "Focused/main lane -> selected scope",
    intensity: "normal",
    requiredFlags: ["syncInput"],
  },
  {
    id: "scroll-checkpoint",
    channel: "scroll",
    title: "Scroll alignment checkpoint",
    detail:
      "Apply a scroll-alignment pass for visual compare and QA across the visible operator scope.",
    scopeLabel: "Visible windows",
    intensity: "safe",
    requiredFlags: ["syncScroll"],
  },
  {
    id: "layout-regroup",
    channel: "navigation",
    title: "Regrouped navigation checkpoint",
    detail:
      "Use a wider navigation checkpoint after regrouping the matrix so the native synchronizer records refreshed scope and intent.",
    scopeLabel: "Whole matrix",
    intensity: "strong",
    requiredFlags: [],
  },
];

export function getExecutionModeLabel(mode: SynchronizerExecutionMode): string {
  if (mode === "native_live") {
    return "Live native";
  }

  if (mode === "local_staged") {
    return "Prepared";
  }

  return "Fallback";
}

export function getLayoutApplyResultLabel(result: SynchronizerLayoutApplyResult): string {
  if (result === "applied") {
    return "applied";
  }

  if (result === "partial") {
    return "partial";
  }

  if (result === "failed") {
    return "failed";
  }

  return "intention-only";
}

export function createInitialCommandCapabilities(): Record<
  SynchronizerCommandKey,
  SynchronizerCommandCapability
> {
  return {
    readSnapshot: {
      key: "readSnapshot",
      label: "Snapshot read",
      status: "local_fallback",
      detail: "Waiting for the first desktop snapshot handshake.",
      lastUpdatedAt: null,
    },
    layout: {
      key: "layout",
      label: "Layout write",
      status: "local_staged",
      detail:
        'Layout write records native layout state first, then attempts physical Win32 placement. Until the first successful command in this session, the console keeps the capability staged; physical outcomes surface as "applied/partial/failed" after execution.',
      lastUpdatedAt: null,
    },
    setMain: {
      key: "setMain",
      label: "Main-window write",
      status: "local_staged",
      detail:
        "Main-window write updates the synchronizer internal anchor state, not physical desktop window arrangement.",
      lastUpdatedAt: null,
    },
    focus: {
      key: "focus",
      label: "Focus write",
      status: "local_staged",
      detail:
        "Focus write uses native Win32 focus control when available; otherwise the selected lane remains staged locally.",
      lastUpdatedAt: null,
    },
    broadcastPlan: {
      key: "broadcastPlan",
      label: "Broadcast write",
      status: "local_staged",
      detail:
        "Broadcast write records native intent/state and target scope through the typed contract. Physical multi-window dispatch is not executed in the current build.",
      lastUpdatedAt: null,
    },
  };
}

export function cloneSyncWindow(window: DesktopSyncWindowState): DesktopSyncWindowState {
  return {
    ...window,
    bounds: window.bounds ? { ...window.bounds } : null,
  };
}

export function cloneSyncLayout(layout: DesktopSyncLayoutState): DesktopSyncLayoutState {
  return { ...layout };
}

export function cloneSynchronizerSnapshot(
  snapshot: DesktopSynchronizerSnapshot,
): DesktopSynchronizerSnapshot {
  return {
    ...snapshot,
    windows: snapshot.windows.map(cloneSyncWindow),
    layout: cloneSyncLayout(snapshot.layout),
  };
}

export function sortWindowsByOrder(
  windows: DesktopSyncWindowState[],
): DesktopSyncWindowState[] {
  return [...windows].sort((left, right) => {
    return left.orderIndex - right.orderIndex || left.windowId.localeCompare(right.windowId);
  });
}

export function getMainWindow(
  windows: DesktopSyncWindowState[],
): DesktopSyncWindowState | null {
  return windows.find((window) => window.isMainWindow) ?? null;
}

export function getFocusedWindow(
  windows: DesktopSyncWindowState[],
): DesktopSyncWindowState | null {
  return windows.find((window) => window.isFocused) ?? null;
}

export function buildLayoutDraft(
  current: DesktopSyncLayoutState,
  mode: DesktopSyncLayoutMode,
  windowCount: number,
  updatedAt: string,
): DesktopSyncLayoutState {
  const normalizedCount = Math.max(1, windowCount);

  if (mode === "grid") {
    const columns = normalizedCount <= 4 ? 2 : 3;
    return {
      ...current,
      mode,
      columns,
      rows: Math.ceil(normalizedCount / columns),
      gapPx: 16,
      overlapOffsetX: null,
      overlapOffsetY: null,
      uniformWidth: 960,
      uniformHeight: 720,
      updatedAt,
    };
  }

  if (mode === "overlap") {
    return {
      ...current,
      mode,
      columns: null,
      rows: null,
      gapPx: 0,
      overlapOffsetX: 42,
      overlapOffsetY: 34,
      uniformWidth: null,
      uniformHeight: null,
      updatedAt,
    };
  }

  return {
    ...current,
    mode,
    columns: null,
    rows: null,
    gapPx: 12,
    overlapOffsetX: null,
    overlapOffsetY: null,
    uniformWidth: 1200,
    uniformHeight: 760,
    updatedAt,
  };
}
