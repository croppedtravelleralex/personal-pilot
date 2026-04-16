import { createStore } from "../../store/createStore";
import * as desktop from "../../services/desktop";
import type { DesktopStatusSnapshot } from "../../types/desktop";

interface StatusState {
  snapshot: DesktopStatusSnapshot | null;
  isLoading: boolean;
  error: string | null;
  requestId: number;
}

const statusStore = createStore<StatusState>({
  snapshot: null,
  isLoading: false,
  error: null,
  requestId: 0,
});

export const statusActions = {
  async refresh() {
    const requestId = statusStore.getState().requestId + 1;
    statusStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
    }));

    try {
      const snapshot = await desktop.getAppStatus();
      if (statusStore.getState().requestId !== requestId) {
        return;
      }

      statusStore.setState((current) => ({
        ...current,
        snapshot,
        isLoading: false,
        error: null,
      }));
    } catch (error) {
      if (statusStore.getState().requestId !== requestId) {
        return;
      }

      statusStore.setState((current) => ({
        ...current,
        isLoading: false,
        error: error instanceof Error ? error.message : "Failed to load status",
      }));
    }
  },
};

export { statusStore };
