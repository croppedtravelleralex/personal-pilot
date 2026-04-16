import { useEffect } from "react";

import { useStore } from "../../store/createStore";
import {
  SYNCHRONIZER_GROUP_BY_OPTIONS,
  SYNCHRONIZER_LAYOUT_OPTIONS,
  SYNCHRONIZER_REFRESH_INTERVAL_OPTIONS,
  SYNCHRONIZER_ROLE_FILTER_OPTIONS,
  SYNCHRONIZER_TARGET_SCREEN_OPTIONS,
  SYNCHRONIZER_VISIBILITY_FILTER_OPTIONS,
  SYNCHRONIZER_BROADCAST_PLAN_TEMPLATES,
} from "./model";
import {
  getSynchronizerConsoleSummary,
  getSynchronizerSummary,
  synchronizerActions,
  synchronizerStore,
} from "./store";

export function useSynchronizerViewModel() {
  const state = useStore(synchronizerStore, (current) => current);
  const summary = getSynchronizerSummary(state);
  const consoleSummary = getSynchronizerConsoleSummary(state, summary);

  useEffect(() => {
    if (state.requestId === 0 && !state.isLoading) {
      void synchronizerActions.refresh();
    }
  }, [state.isLoading, state.requestId]);

  useEffect(() => {
    if (
      !state.autoRefreshEnabled ||
      state.refreshIntervalMs <= 0 ||
      state.activeAction !== null
    ) {
      return;
    }

    const timerId = window.setInterval(() => {
      void synchronizerActions.refresh();
    }, state.refreshIntervalMs);

    return () => {
      window.clearInterval(timerId);
    };
  }, [state.activeAction, state.autoRefreshEnabled, state.refreshIntervalMs]);

  return {
    state,
    summary,
    consoleSummary,
    layoutOptions: SYNCHRONIZER_LAYOUT_OPTIONS,
    refreshIntervalOptions: SYNCHRONIZER_REFRESH_INTERVAL_OPTIONS,
    groupByOptions: SYNCHRONIZER_GROUP_BY_OPTIONS,
    roleFilterOptions: SYNCHRONIZER_ROLE_FILTER_OPTIONS,
    visibilityFilterOptions: SYNCHRONIZER_VISIBILITY_FILTER_OPTIONS,
    targetScreenOptions: SYNCHRONIZER_TARGET_SCREEN_OPTIONS,
    broadcastPlanTemplates: SYNCHRONIZER_BROADCAST_PLAN_TEMPLATES,
    actions: synchronizerActions,
  };
}
