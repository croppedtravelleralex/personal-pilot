import { startTransition, useDeferredValue, useEffect } from "react";

import { useDebouncedValue } from "../../hooks/useDebouncedValue";
import { useStore } from "../../store/createStore";
import { buildLogsConsoleSummary } from "./model";
import { logActions, logsStore } from "./store";

export function useLogsViewModel() {
  const state = useStore(logsStore, (current) => current);
  const debouncedRuntimeSearch = useDebouncedValue(state.runtime.searchInput, 300);
  const debouncedRuntimeTaskId = useDebouncedValue(state.runtime.taskIdInput, 300);
  const debouncedActionSearch = useDebouncedValue(state.action.searchInput, 300);
  const deferredRuntimeSearch = useDeferredValue(debouncedRuntimeSearch);
  const deferredRuntimeTaskId = useDeferredValue(debouncedRuntimeTaskId);
  const deferredActionSearch = useDeferredValue(debouncedActionSearch);

  useEffect(() => {
    if (state.runtime.appliedSearch !== deferredRuntimeSearch) {
      startTransition(() => {
        logActions.applyRuntimeSearch(deferredRuntimeSearch);
      });
    }
  }, [deferredRuntimeSearch, state.runtime.appliedSearch]);

  useEffect(() => {
    if (state.runtime.appliedTaskId !== deferredRuntimeTaskId) {
      startTransition(() => {
        logActions.applyRuntimeTaskId(deferredRuntimeTaskId);
      });
    }
  }, [deferredRuntimeTaskId, state.runtime.appliedTaskId]);

  useEffect(() => {
    if (state.action.appliedSearch !== deferredActionSearch) {
      startTransition(() => {
        logActions.applyActionSearch(deferredActionSearch);
      });
    }
  }, [deferredActionSearch, state.action.appliedSearch]);

  useEffect(() => {
    if (state.viewMode !== "runtime") {
      return;
    }

    void logActions.refreshRuntime();
  }, [
    state.viewMode,
    state.runtime.page,
    state.runtime.pageSize,
    state.runtime.levelFilter,
    state.runtime.appliedSearch,
    state.runtime.appliedTaskId,
  ]);

  useEffect(() => {
    if (state.viewMode !== "action") {
      return;
    }

    void logActions.refreshActionTasks();
  }, [
    state.viewMode,
    state.action.page,
    state.action.pageSize,
    state.action.statusFilter,
    state.action.appliedSearch,
  ]);

  useEffect(() => {
    if (state.viewMode !== "action") {
      return;
    }

    void logActions.refreshSelectedActionLogs();
  }, [state.viewMode, state.action.selectedTaskId]);

  return {
    state,
    summary: buildLogsConsoleSummary(state),
    runtimeTotalPages: Math.max(
      1,
      Math.ceil(state.runtime.total / state.runtime.pageSize || 1),
    ),
    actionTotalPages: Math.max(
      1,
      Math.ceil(state.action.total / state.action.pageSize || 1),
    ),
    actions: logActions,
  };
}
