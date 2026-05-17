import { useEffect } from "react";

import { useStore } from "../../store/createStore";
import { buildStatusOverview, STATUS_AUTO_REFRESH_INTERVAL_MS } from "./model";
import { statusActions, statusStore } from "./store";

export function useStatusViewModel() {
  const state = useStore(statusStore, (current) => current);

  useEffect(() => {
    if (!state.snapshot && !state.isLoading) {
      void statusActions.refresh();
    }
  }, [state.isLoading, state.snapshot]);

  useEffect(() => {
    const timerId = window.setInterval(() => {
      void statusActions.refresh();
    }, STATUS_AUTO_REFRESH_INTERVAL_MS);

    return () => {
      window.clearInterval(timerId);
    };
  }, []);

  return {
    state,
    summary: buildStatusOverview(state.snapshot),
    refresh: statusActions.refresh,
  };
}
