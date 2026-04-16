import { createStore } from "../../store/createStore";
import * as desktop from "../../services/desktop";
import type { DesktopRuntimeStatus } from "../../types/desktop";

interface RuntimeState {
  snapshot: DesktopRuntimeStatus | null;
  isLoading: boolean;
  activeAction: "start" | "stop" | null;
  error: string | null;
  info: string | null;
  requestId: number;
  autoRefreshEnabled: boolean;
  refreshIntervalMs: number;
}

const runtimeStore = createStore<RuntimeState>({
  snapshot: null,
  isLoading: false,
  activeAction: null,
  error: null,
  info: "Runtime auto refresh is enabled while the local console is open.",
  requestId: 0,
  autoRefreshEnabled: true,
  refreshIntervalMs: 15000,
});

function getRuntimeInfo(snapshot: DesktopRuntimeStatus): string | null {
  if (snapshot.status === "external_running") {
    return "Runtime is reachable, but it was started outside the desktop controller.";
  }

  if (snapshot.running && !snapshot.apiReachable) {
    return "Runtime process is up, but the health endpoint is not reachable yet.";
  }

  if (snapshot.status === "managed_stopped" && snapshot.lastExitCode !== null) {
    return `Last managed runtime exited with code ${snapshot.lastExitCode}.`;
  }

  if (!snapshot.running) {
    return "Runtime is currently stopped. Use Start runtime to recover locally.";
  }

  return "Runtime is healthy and under desktop control.";
}

function updateSnapshot(snapshot: DesktopRuntimeStatus, info?: string | null) {
  runtimeStore.setState((current) => ({
    ...current,
    snapshot,
    isLoading: false,
    activeAction: null,
    error: null,
    info: info ?? getRuntimeInfo(snapshot),
  }));
}

export const runtimeActions = {
  setAutoRefreshEnabled(autoRefreshEnabled: boolean) {
    runtimeStore.setState((current) => ({
      ...current,
      autoRefreshEnabled,
      info: autoRefreshEnabled
        ? "Runtime auto refresh is enabled while the local console is open."
        : "Runtime auto refresh is paused. Use Refresh runtime for manual polling.",
    }));
  },
  setRefreshIntervalMs(refreshIntervalMs: number) {
    runtimeStore.setState((current) => ({
      ...current,
      refreshIntervalMs,
    }));
  },
  async refresh() {
    const requestId = runtimeStore.getState().requestId + 1;
    runtimeStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
    }));

    try {
      const snapshot = await desktop.readLocalRuntimeStatus();
      if (runtimeStore.getState().requestId !== requestId) {
        return;
      }
      updateSnapshot(snapshot);
    } catch (error) {
      if (runtimeStore.getState().requestId !== requestId) {
        return;
      }
      runtimeStore.setState((current) => ({
        ...current,
        isLoading: false,
        activeAction: null,
        error:
          error instanceof Error ? error.message : "Failed to read local runtime status",
      }));
    }
  },
  async start() {
    runtimeStore.setState((current) => ({
      ...current,
      activeAction: "start",
      error: null,
      info: "Starting managed runtime...",
    }));

    try {
      const snapshot = await desktop.startLocalRuntime();
      updateSnapshot(snapshot, "Managed runtime start command completed.");
    } catch (error) {
      runtimeStore.setState((current) => ({
        ...current,
        activeAction: null,
        error:
          error instanceof Error ? error.message : "Failed to start local runtime",
      }));
    }
  },
  async stop() {
    runtimeStore.setState((current) => ({
      ...current,
      activeAction: "stop",
      error: null,
      info: "Stopping managed runtime...",
    }));

    try {
      const snapshot = await desktop.stopLocalRuntime();
      updateSnapshot(snapshot, "Managed runtime stop command completed.");
    } catch (error) {
      runtimeStore.setState((current) => ({
        ...current,
        activeAction: null,
        error:
          error instanceof Error ? error.message : "Failed to stop local runtime",
      }));
    }
  },
};

export { runtimeStore };
