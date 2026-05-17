import { startTransition, useEffect, useEffectEvent, useReducer, useRef } from "react";

import type {
  ProfileColumnId,
  ProfileDrawerTab,
  ProfilePlatform,
  ProfileRuntimeStatus,
  ProfilesBatchAction,
  ProfilesDensity,
  ProfilesSortKey,
  ProxyHealth,
} from "./model";
import {
  loadProfileDetailSnapshot,
  loadProfilesSnapshot,
  runProfilesBatchAction,
} from "./adapters";
import {
  normalizeSearchQuery,
  selectFilteredProfiles,
  selectGroupOptions,
  selectPlatformOptions,
  selectProxyHealthOptions,
  selectRuntimeStatusOptions,
  selectSelectionState,
  selectTagOptions,
  selectVisibleColumns,
  selectWorkbenchSummary,
} from "./selectors";
import {
  createInitialProfilesWorkbenchState,
  profilesWorkbenchReducer,
  type ProfilesWorkbenchAction,
} from "./store";

const SEARCH_DEBOUNCE_MS = 320;

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useProfilesWorkbenchViewModel() {
  const [state, dispatch] = useReducer(
    profilesWorkbenchReducer,
    undefined,
    createInitialProfilesWorkbenchState,
  );
  const searchRequestIdRef = useRef(0);
  const listRequestIdRef = useRef(0);
  const detailRequestIdRef = useRef(0);

  const loadList = useEffectEvent(async () => {
    const requestId = ++listRequestIdRef.current;
    dispatch({ type: "listRequested", requestId });

    try {
      const snapshot = await loadProfilesSnapshot();
      dispatch({
        type: "listSucceeded",
        requestId,
        rows: snapshot.rows,
        totalCount: snapshot.totalCount,
        source: snapshot.source,
      });
    } catch (error) {
      dispatch({
        type: "listFailed",
        requestId,
        error: toErrorMessage(error),
      });
    }
  });

  const loadDetail = useEffectEvent(async (profileId: string) => {
    const requestId = ++detailRequestIdRef.current;
    dispatch({ type: "detailRequested", requestId });

    try {
      const detail = await loadProfileDetailSnapshot(profileId, state.profiles);
      dispatch({
        type: "detailSucceeded",
        requestId,
        detail,
        source: detail.source,
      });
    } catch (error) {
      dispatch({
        type: "detailFailed",
        requestId,
        error: toErrorMessage(error),
      });
    }
  });

  const applySearchQuery = useEffectEvent((query: string, requestId: number) => {
    startTransition(() => {
      dispatch({ type: "searchQueryApplied", query, requestId });
    });
  });

  const runBatchAction = useEffectEvent(async (action: ProfilesBatchAction) => {
    if (state.batch.status === "running") {
      return;
    }

    const selectedIds = state.selectedIds;
    if (selectedIds.length === 0) {
      dispatch({
        type: "batchActionFailed",
        action,
        error: "Select at least one profile row before running a batch action.",
      });
      return;
    }

    dispatch({
      type: "batchActionStarted",
      action,
      affectedCount: selectedIds.length,
    });

    try {
      const result = await runProfilesBatchAction(action, selectedIds);
      const openedProfileId = state.drawer.openedProfileId;

      await loadList();

      if (openedProfileId && result.profileIds.includes(openedProfileId)) {
        await loadDetail(openedProfileId);
      }

      dispatch({
        type: "batchActionSucceeded",
        action,
        message: result.message,
        updatedAt: result.updatedAt,
      });
    } catch (error) {
      dispatch({
        type: "batchActionFailed",
        action,
        error: toErrorMessage(error),
      });
    }
  });

  useEffect(() => {
    void loadList();
  }, [loadList]);

  useEffect(() => {
    const nextSearchQuery = normalizeSearchQuery(state.searchInput);

    if (nextSearchQuery === state.searchQuery) {
      return;
    }

    searchRequestIdRef.current += 1;
    const requestId = searchRequestIdRef.current;

    dispatch({ type: "searchQueryPending", requestId });

    const timerId = window.setTimeout(() => {
      applySearchQuery(nextSearchQuery, requestId);
    }, SEARCH_DEBOUNCE_MS);

    return () => window.clearTimeout(timerId);
  }, [applySearchQuery, state.searchInput, state.searchQuery]);

  useEffect(() => {
    if (!state.drawer.openedProfileId) {
      return;
    }

    void loadDetail(state.drawer.openedProfileId);
  }, [loadDetail, state.drawer.openedProfileId]);

  const filteredProfiles = selectFilteredProfiles(state);
  const visibleColumns = selectVisibleColumns(state);
  const selection = selectSelectionState(state, filteredProfiles);
  const summary = selectWorkbenchSummary(state, filteredProfiles);
  const visibleIds = filteredProfiles.map((record) => record.id);

  function dispatchTransition(action: ProfilesWorkbenchAction) {
    startTransition(() => {
      dispatch(action);
    });
  }

  function setSortBy(sortBy: ProfilesSortKey) {
    dispatchTransition({ type: "sortByChanged", sortBy });
  }

  function setDensity(density: ProfilesDensity) {
    dispatchTransition({ type: "densityChanged", density });
  }

  function openProfileDrawer(profileId: string, initialTab?: ProfileDrawerTab) {
    dispatch({
      type: "drawerOpened",
      profileId,
      initialTab,
    });
  }

  return {
    state,
    filteredProfiles,
    visibleColumns,
    selection,
    summary,
    filterOptions: {
      groups: selectGroupOptions(state),
      tags: selectTagOptions(state),
      platforms: selectPlatformOptions(state),
      runtimeStatuses: selectRuntimeStatusOptions(state),
      proxyHealth: selectProxyHealthOptions(state),
    },
    actions: {
      reload: () => void loadList(),
      setSearchInput: (value: string) => dispatch({ type: "searchInputChanged", value }),
      toggleGroup: (groupId: string) => dispatchTransition({ type: "groupToggled", groupId }),
      toggleTag: (tagValue: string) => dispatchTransition({ type: "tagToggled", tagValue }),
      togglePlatform: (platformId: ProfilePlatform) =>
        dispatchTransition({ type: "platformToggled", platformId }),
      toggleRuntimeStatus: (status: ProfileRuntimeStatus) =>
        dispatchTransition({ type: "runtimeStatusToggled", status }),
      toggleProxyHealth: (proxyHealth: ProxyHealth) =>
        dispatchTransition({ type: "proxyHealthToggled", proxyHealth }),
      clearFilters: () => dispatchTransition({ type: "filtersCleared" }),
      resetView: () => dispatchTransition({ type: "viewReset" }),
      setSortBy,
      setDensity,
      toggleColumn: (columnId: ProfileColumnId) =>
        dispatchTransition({ type: "columnVisibilityToggled", columnId }),
      toggleRowSelection: (profileId: string) =>
        dispatch({ type: "rowSelectionToggled", profileId }),
      toggleVisibleSelection: () =>
        dispatch({ type: "visibleSelectionToggled", profileIds: visibleIds }),
      clearSelection: () => dispatch({ type: "selectionCleared" }),
      runBatchAction: (action: ProfilesBatchAction) => void runBatchAction(action),
      dismissBatchFeedback: () => dispatch({ type: "batchFeedbackCleared" }),
      requestCreateProfile: () =>
        dispatch({ type: "wizardRequested", mode: "create" }),
      requestEditProfile: (profileId: string, profileName: string) =>
        dispatch({
          type: "wizardRequested",
          mode: "edit",
          profileId,
          profileName,
        }),
      requestProfileDrawer: (profileId: string) => openProfileDrawer(profileId, "overview"),
      openProfileLogs: (profileId: string) => openProfileDrawer(profileId, "logs"),
      closeDrawer: () => dispatch({ type: "drawerClosed" }),
      setDrawerTab: (tab: ProfileDrawerTab) => dispatch({ type: "drawerTabChanged", tab }),
      retryDrawer: () => {
        if (state.drawer.openedProfileId) {
          void loadDetail(state.drawer.openedProfileId);
        }
      },
      dismissNotice: () => dispatch({ type: "noticeCleared" }),
    },
  };
}
