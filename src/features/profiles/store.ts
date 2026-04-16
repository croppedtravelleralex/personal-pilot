import {
  createInitialColumnVisibility,
  type ProfileColumnId,
  type ProfileDataSource,
  type ProfileDetail,
  type ProfileDrawerTab,
  type ProfilePlatform,
  type ProfileRow,
  type ProfileRuntimeStatus,
  type ProfileWizardIntent,
  type ProfilesBatchAction,
  type ProfilesDensity,
  type ProfilesSortKey,
  type ProxyHealth,
} from "./model";

interface ProfilesListState {
  status: "idle" | "loading" | "ready" | "error";
  totalCount: number;
  source: ProfileDataSource | null;
  error: string | null;
  requestId: number;
}

interface ProfilesDrawerState {
  openedProfileId: string | null;
  activeTab: ProfileDrawerTab;
  status: "idle" | "loading" | "ready" | "error";
  detail: ProfileDetail | null;
  source: ProfileDataSource | null;
  error: string | null;
  requestId: number;
}

interface ProfilesBatchState {
  status: "idle" | "running" | "success" | "error";
  activeAction: ProfilesBatchAction | null;
  message: string | null;
  tone: "neutral" | "success" | "error";
  updatedAt: string | null;
}

export interface ProfilesWorkbenchState {
  profiles: ProfileRow[];
  searchInput: string;
  searchQuery: string;
  pendingSearchRequestId: number;
  appliedSearchRequestId: number;
  isFiltering: boolean;
  groupIds: string[];
  tagValues: string[];
  platformIds: ProfilePlatform[];
  runtimeStatuses: ProfileRuntimeStatus[];
  proxyHealth: ProxyHealth[];
  selectedIds: string[];
  columnVisibility: Record<ProfileColumnId, boolean>;
  sortBy: ProfilesSortKey;
  density: ProfilesDensity;
  list: ProfilesListState;
  drawer: ProfilesDrawerState;
  batch: ProfilesBatchState;
  actionNotice: string | null;
  pendingWizardIntent: ProfileWizardIntent | null;
}

export type ProfilesWorkbenchAction =
  | { type: "listRequested"; requestId: number }
  | {
      type: "listSucceeded";
      requestId: number;
      rows: ProfileRow[];
      totalCount: number;
      source: ProfileDataSource;
    }
  | { type: "listFailed"; requestId: number; error: string }
  | { type: "searchInputChanged"; value: string }
  | { type: "searchQueryPending"; requestId: number }
  | { type: "searchQueryApplied"; query: string; requestId: number }
  | { type: "groupToggled"; groupId: string }
  | { type: "tagToggled"; tagValue: string }
  | { type: "platformToggled"; platformId: ProfilePlatform }
  | { type: "runtimeStatusToggled"; status: ProfileRuntimeStatus }
  | { type: "proxyHealthToggled"; proxyHealth: ProxyHealth }
  | { type: "filtersCleared" }
  | { type: "viewReset" }
  | { type: "sortByChanged"; sortBy: ProfilesSortKey }
  | { type: "densityChanged"; density: ProfilesDensity }
  | { type: "columnVisibilityToggled"; columnId: ProfileColumnId }
  | { type: "rowSelectionToggled"; profileId: string }
  | { type: "visibleSelectionToggled"; profileIds: string[] }
  | { type: "selectionCleared" }
  | { type: "batchActionStarted"; action: ProfilesBatchAction; affectedCount: number }
  | {
      type: "batchActionSucceeded";
      action: ProfilesBatchAction;
      message: string;
      updatedAt: string;
    }
  | { type: "batchActionFailed"; action: ProfilesBatchAction; error: string }
  | { type: "batchFeedbackCleared" }
  | { type: "drawerOpened"; profileId: string; initialTab?: ProfileDrawerTab }
  | { type: "drawerClosed" }
  | { type: "drawerTabChanged"; tab: ProfileDrawerTab }
  | { type: "detailRequested"; requestId: number }
  | {
      type: "detailSucceeded";
      requestId: number;
      detail: ProfileDetail;
      source: ProfileDataSource;
    }
  | { type: "detailFailed"; requestId: number; error: string }
  | { type: "wizardRequested"; mode: "create" | "edit"; profileId?: string; profileName?: string }
  | { type: "noticeCleared" };

function toggleStringValue<T extends string>(source: T[], value: T): T[] {
  return source.includes(value)
    ? source.filter((item) => item !== value)
    : [...source, value];
}

function createActionLabel(action: ProfilesBatchAction, affectedCount: number): string {
  const labelByAction: Record<ProfilesBatchAction, string> = {
    open: "Open",
    start: "Start",
    stop: "Stop",
    checkProxy: "Check Proxy",
    sync: "Sync",
  };

  return `${labelByAction[action]} is running for ${affectedCount} selected profiles.`;
}

export function createInitialProfilesWorkbenchState(): ProfilesWorkbenchState {
  return {
    profiles: [],
    searchInput: "",
    searchQuery: "",
    pendingSearchRequestId: 0,
    appliedSearchRequestId: 0,
    isFiltering: false,
    groupIds: [],
    tagValues: [],
    platformIds: [],
    runtimeStatuses: [],
    proxyHealth: [],
    selectedIds: [],
    columnVisibility: createInitialColumnVisibility(),
    sortBy: "lastActive",
    density: "compact",
    list: {
      status: "idle",
      totalCount: 0,
      source: null,
      error: null,
      requestId: 0,
    },
    drawer: {
      openedProfileId: null,
      activeTab: "overview",
      status: "idle",
      detail: null,
      source: null,
      error: null,
      requestId: 0,
    },
    batch: {
      status: "idle",
      activeAction: null,
      message: null,
      tone: "neutral",
      updatedAt: null,
    },
    actionNotice: null,
    pendingWizardIntent: null,
  };
}

export function profilesWorkbenchReducer(
  state: ProfilesWorkbenchState,
  action: ProfilesWorkbenchAction,
): ProfilesWorkbenchState {
  switch (action.type) {
    case "listRequested":
      return {
        ...state,
        list: {
          ...state.list,
          status: "loading",
          error: null,
          requestId: action.requestId,
        },
      };
    case "listSucceeded":
      if (action.requestId !== state.list.requestId) {
        return state;
      }

      return {
        ...state,
        profiles: action.rows,
        selectedIds: state.selectedIds.filter((profileId) =>
          action.rows.some((row) => row.id === profileId),
        ),
        list: {
          status: "ready",
          totalCount: action.totalCount,
          source: action.source,
          error: null,
          requestId: action.requestId,
        },
      };
    case "listFailed":
      if (action.requestId !== state.list.requestId) {
        return state;
      }

      return {
        ...state,
        list: {
          ...state.list,
          status: "error",
          error: action.error,
        },
      };
    case "searchInputChanged":
      return {
        ...state,
        searchInput: action.value,
      };
    case "searchQueryPending":
      return {
        ...state,
        pendingSearchRequestId: action.requestId,
        isFiltering: true,
      };
    case "searchQueryApplied":
      if (action.requestId !== state.pendingSearchRequestId) {
        return state;
      }

      return {
        ...state,
        searchQuery: action.query,
        appliedSearchRequestId: action.requestId,
        isFiltering: false,
        selectedIds: [],
      };
    case "groupToggled":
      return {
        ...state,
        groupIds: toggleStringValue(state.groupIds, action.groupId),
        selectedIds: [],
      };
    case "tagToggled":
      return {
        ...state,
        tagValues: toggleStringValue(state.tagValues, action.tagValue),
        selectedIds: [],
      };
    case "platformToggled":
      return {
        ...state,
        platformIds: toggleStringValue(state.platformIds, action.platformId),
        selectedIds: [],
      };
    case "runtimeStatusToggled":
      return {
        ...state,
        runtimeStatuses: toggleStringValue(state.runtimeStatuses, action.status),
        selectedIds: [],
      };
    case "proxyHealthToggled":
      return {
        ...state,
        proxyHealth: toggleStringValue(state.proxyHealth, action.proxyHealth),
        selectedIds: [],
      };
    case "filtersCleared":
      return {
        ...state,
        groupIds: [],
        tagValues: [],
        platformIds: [],
        runtimeStatuses: [],
        proxyHealth: [],
        selectedIds: [],
      };
    case "viewReset":
      return {
        ...state,
        searchInput: "",
        searchQuery: "",
        pendingSearchRequestId: 0,
        appliedSearchRequestId: 0,
        isFiltering: false,
        groupIds: [],
        tagValues: [],
        platformIds: [],
        runtimeStatuses: [],
        proxyHealth: [],
        selectedIds: [],
        columnVisibility: createInitialColumnVisibility(),
        sortBy: "lastActive",
        density: "compact",
      };
    case "sortByChanged":
      return {
        ...state,
        sortBy: action.sortBy,
      };
    case "densityChanged":
      return {
        ...state,
        density: action.density,
      };
    case "columnVisibilityToggled":
      if (action.columnId === "profile") {
        return state;
      }

      return {
        ...state,
        columnVisibility: {
          ...state.columnVisibility,
          [action.columnId]: !state.columnVisibility[action.columnId],
        },
      };
    case "rowSelectionToggled":
      return {
        ...state,
        selectedIds: toggleStringValue(state.selectedIds, action.profileId),
      };
    case "visibleSelectionToggled": {
      const allVisibleSelected =
        action.profileIds.length > 0 &&
        action.profileIds.every((profileId) => state.selectedIds.includes(profileId));

      return {
        ...state,
        selectedIds: allVisibleSelected
          ? state.selectedIds.filter((profileId) => !action.profileIds.includes(profileId))
          : Array.from(new Set([...state.selectedIds, ...action.profileIds])),
      };
    }
    case "selectionCleared":
      return {
        ...state,
        selectedIds: [],
      };
    case "batchActionStarted":
      return {
        ...state,
        batch: {
          status: "running",
          activeAction: action.action,
          message: createActionLabel(action.action, action.affectedCount),
          tone: "neutral",
          updatedAt: null,
        },
      };
    case "batchActionSucceeded":
      return {
        ...state,
        batch: {
          status: "success",
          activeAction: null,
          message: action.message,
          tone: "success",
          updatedAt: action.updatedAt,
        },
      };
    case "batchActionFailed":
      return {
        ...state,
        batch: {
          status: "error",
          activeAction: null,
          message: action.error,
          tone: "error",
          updatedAt: null,
        },
      };
    case "batchFeedbackCleared":
      return {
        ...state,
        batch: {
          status: "idle",
          activeAction: null,
          message: null,
          tone: "neutral",
          updatedAt: null,
        },
      };
    case "drawerOpened":
      return {
        ...state,
        drawer: {
          ...state.drawer,
          openedProfileId: action.profileId,
          activeTab: action.initialTab ?? state.drawer.activeTab,
          status: "loading",
          error: null,
          requestId: state.drawer.requestId,
        },
      };
    case "drawerClosed":
      return {
        ...state,
        drawer: {
          ...state.drawer,
          openedProfileId: null,
          status: "idle",
          detail: null,
          error: null,
        },
      };
    case "drawerTabChanged":
      return {
        ...state,
        drawer: {
          ...state.drawer,
          activeTab: action.tab,
        },
      };
    case "detailRequested":
      return {
        ...state,
        drawer: {
          ...state.drawer,
          status: "loading",
          error: null,
          requestId: action.requestId,
        },
      };
    case "detailSucceeded":
      if (action.requestId !== state.drawer.requestId) {
        return state;
      }

      return {
        ...state,
        drawer: {
          ...state.drawer,
          status: "ready",
          detail: action.detail,
          source: action.source,
          error: null,
        },
      };
    case "detailFailed":
      if (action.requestId !== state.drawer.requestId) {
        return state;
      }

      return {
        ...state,
        drawer: {
          ...state.drawer,
          status: "error",
          error: action.error,
        },
      };
    case "wizardRequested":
      return {
        ...state,
        pendingWizardIntent: {
          mode: action.mode,
          profileId: action.profileId,
        },
        actionNotice:
          action.mode === "create"
            ? "Blocked: profile creation wizard shell is reserved for a later slice."
            : `Blocked: edit flow for ${action.profileName ?? "selected profile"} is waiting for write contracts.`,
      };
    case "noticeCleared":
      return {
        ...state,
        actionNotice: null,
      };
    default:
      return state;
  }
}
