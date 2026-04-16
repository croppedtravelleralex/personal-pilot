import { useEffect } from "react";

import { useStore } from "../../store/createStore";
import { buildRuntimeOverview } from "./model";
import { runtimeActions, runtimeStore } from "./store";

export function useRuntimeViewModel() {
  const state = useStore(runtimeStore, (current) => current);

  useEffect(() => {
    if (!state.snapshot && !state.isLoading) {
      void runtimeActions.refresh();
    }
  }, [state.isLoading, state.snapshot]);

  useEffect(() => {
    if (
      !state.autoRefreshEnabled ||
      state.refreshIntervalMs <= 0 ||
      state.activeAction !== null
    ) {
      return;
    }

    const timerId = window.setInterval(() => {
      void runtimeActions.refresh();
    }, state.refreshIntervalMs);

    return () => {
      window.clearInterval(timerId);
    };
  }, [state.activeAction, state.autoRefreshEnabled, state.refreshIntervalMs]);

  return {
    state,
    summary: buildRuntimeOverview(
      state.snapshot,
      state.activeAction,
      state.autoRefreshEnabled,
      state.refreshIntervalMs,
    ),
    actions: runtimeActions,
  };
}
