import { createStore } from "../../store/createStore";
import * as desktop from "../../services/desktop";
import type { DesktopLogItem, DesktopTaskItem } from "../../types/desktop";

export type LogsViewMode = "runtime" | "action";

export interface RuntimeLogsState {
  items: DesktopLogItem[];
  total: number;
  page: number;
  pageSize: number;
  levelFilter: string;
  taskIdInput: string;
  appliedTaskId: string;
  searchInput: string;
  appliedSearch: string;
  isLoading: boolean;
  error: string | null;
  requestId: number;
}

export interface ActionLogsState {
  items: DesktopTaskItem[];
  total: number;
  page: number;
  pageSize: number;
  statusFilter: string;
  searchInput: string;
  appliedSearch: string;
  isLoading: boolean;
  error: string | null;
  requestId: number;
  selectedTaskId: string | null;
  selectedTaskSnapshot: DesktopTaskItem | null;
  selectedTaskLogs: DesktopLogItem[];
  selectedTaskLogsLoading: boolean;
  selectedTaskLogsError: string | null;
  selectedTaskLogsRequestId: number;
}

export interface LogsState {
  viewMode: LogsViewMode;
  info: string | null;
  runtime: RuntimeLogsState;
  action: ActionLogsState;
}

const logsStore = createStore<LogsState>({
  viewMode: "runtime",
  info: "Runtime logs stream raw local log rows from SQLite.",
  runtime: {
    items: [],
    total: 0,
    page: 1,
    pageSize: 100,
    levelFilter: "all",
    taskIdInput: "",
    appliedTaskId: "",
    searchInput: "",
    appliedSearch: "",
    isLoading: false,
    error: null,
    requestId: 0,
  },
  action: {
    items: [],
    total: 0,
    page: 1,
    pageSize: 20,
    statusFilter: "all",
    searchInput: "",
    appliedSearch: "",
    isLoading: false,
    error: null,
    requestId: 0,
    selectedTaskId: null,
    selectedTaskSnapshot: null,
    selectedTaskLogs: [],
    selectedTaskLogsLoading: false,
    selectedTaskLogsError: null,
    selectedTaskLogsRequestId: 0,
  },
});

export const logActions = {
  setViewMode(viewMode: LogsViewMode) {
    logsStore.setState((current) => ({
      ...current,
      viewMode,
      info:
        viewMode === "runtime"
          ? "Runtime logs stream raw local log rows from SQLite."
          : "Action logs pivot around task actions and their related runtime entries.",
    }));
  },
  setRuntimePage(page: number) {
    logsStore.setState((current) => ({
      ...current,
      runtime: {
        ...current.runtime,
        page: Math.max(1, page),
      },
    }));
  },
  setRuntimePageSize(pageSize: number) {
    logsStore.setState((current) => ({
      ...current,
      runtime: {
        ...current.runtime,
        page: 1,
        pageSize,
      },
    }));
  },
  setRuntimeLevelFilter(levelFilter: string) {
    logsStore.setState((current) => ({
      ...current,
      runtime: {
        ...current.runtime,
        page: 1,
        levelFilter,
      },
    }));
  },
  setRuntimeTaskIdInput(taskIdInput: string) {
    logsStore.setState((current) => ({
      ...current,
      runtime: {
        ...current.runtime,
        taskIdInput,
      },
    }));
  },
  applyRuntimeTaskId(appliedTaskId: string) {
    logsStore.setState((current) => ({
      ...current,
      runtime:
        current.runtime.appliedTaskId === appliedTaskId
          ? current.runtime
          : {
              ...current.runtime,
              page: 1,
              appliedTaskId,
            },
    }));
  },
  setRuntimeSearchInput(searchInput: string) {
    logsStore.setState((current) => ({
      ...current,
      runtime: {
        ...current.runtime,
        searchInput,
      },
    }));
  },
  applyRuntimeSearch(appliedSearch: string) {
    logsStore.setState((current) => ({
      ...current,
      runtime:
        current.runtime.appliedSearch === appliedSearch
          ? current.runtime
          : {
              ...current.runtime,
              page: 1,
              appliedSearch,
            },
    }));
  },
  setActionPage(page: number) {
    logsStore.setState((current) => ({
      ...current,
      action: {
        ...current.action,
        page: Math.max(1, page),
      },
    }));
  },
  setActionPageSize(pageSize: number) {
    logsStore.setState((current) => ({
      ...current,
      action: {
        ...current.action,
        page: 1,
        pageSize,
      },
    }));
  },
  setActionStatusFilter(statusFilter: string) {
    logsStore.setState((current) => ({
      ...current,
      action: {
        ...current.action,
        page: 1,
        statusFilter,
      },
    }));
  },
  setActionSearchInput(searchInput: string) {
    logsStore.setState((current) => ({
      ...current,
      action: {
        ...current.action,
        searchInput,
      },
    }));
  },
  applyActionSearch(appliedSearch: string) {
    logsStore.setState((current) => ({
      ...current,
      action:
        current.action.appliedSearch === appliedSearch
          ? current.action
          : {
              ...current.action,
              page: 1,
              appliedSearch,
            },
    }));
  },
  selectActionTask(taskId: string) {
    logsStore.setState((current) => ({
      ...current,
      info: `Selected action task ${taskId}. You can inspect its runtime logs from the detail panel.`,
      action: {
        ...current.action,
        selectedTaskId: taskId,
        selectedTaskSnapshot:
          current.action.items.find((item) => item.id === taskId) ??
          current.action.selectedTaskSnapshot,
        selectedTaskLogsError: null,
      },
    }));
  },
  openTaskInRuntime(taskId: string) {
    logsStore.setState((current) => ({
      ...current,
      viewMode: "runtime",
      info: `Runtime logs are now scoped to task ${taskId}.`,
      runtime: {
        ...current.runtime,
        page: 1,
        taskIdInput: taskId,
        appliedTaskId: taskId,
      },
    }));
  },
  async refreshRuntime() {
    const snapshot = logsStore.getState().runtime;
    const requestId = snapshot.requestId + 1;
    logsStore.setState((current) => ({
      ...current,
      runtime: {
        ...current.runtime,
        isLoading: true,
        error: null,
        requestId,
      },
    }));

    try {
      const page = await desktop.listLogPage({
        page: snapshot.page,
        pageSize: snapshot.pageSize,
        levelFilter:
          snapshot.levelFilter === "all" ? undefined : snapshot.levelFilter,
        taskIdFilter: snapshot.appliedTaskId || undefined,
        search: snapshot.appliedSearch || undefined,
      });

      if (logsStore.getState().runtime.requestId !== requestId) {
        return;
      }

      logsStore.setState((current) => ({
        ...current,
        runtime: {
          ...current.runtime,
          items: page.items,
          total: page.total,
          page: page.page,
          pageSize: page.pageSize,
          isLoading: false,
          error: null,
        },
      }));
    } catch (error) {
      if (logsStore.getState().runtime.requestId !== requestId) {
        return;
      }

      logsStore.setState((current) => ({
        ...current,
        runtime: {
          ...current.runtime,
          isLoading: false,
          error: error instanceof Error ? error.message : "Failed to load logs",
        },
      }));
    }
  },
  async refreshActionTasks() {
    const snapshot = logsStore.getState().action;
    const requestId = snapshot.requestId + 1;
    logsStore.setState((current) => ({
      ...current,
      action: {
        ...current.action,
        isLoading: true,
        error: null,
        requestId,
      },
    }));

    try {
      const page = await desktop.listTaskPage({
        page: snapshot.page,
        pageSize: snapshot.pageSize,
        statusFilter:
          snapshot.statusFilter === "all" ? undefined : snapshot.statusFilter,
        search: snapshot.appliedSearch || undefined,
      });

      if (logsStore.getState().action.requestId !== requestId) {
        return;
      }

      logsStore.setState((current) => {
        const selectedTask =
          page.items.find((item) => item.id === current.action.selectedTaskId) ??
          page.items[0] ??
          null;

        return {
          ...current,
          action: {
            ...current.action,
            items: page.items,
            total: page.total,
            page: page.page,
            pageSize: page.pageSize,
            isLoading: false,
            error: null,
            selectedTaskId: selectedTask?.id ?? null,
            selectedTaskSnapshot: selectedTask,
          },
        };
      });
    } catch (error) {
      if (logsStore.getState().action.requestId !== requestId) {
        return;
      }

      logsStore.setState((current) => ({
        ...current,
        action: {
          ...current.action,
          isLoading: false,
          error:
            error instanceof Error ? error.message : "Failed to load action logs",
        },
      }));
    }
  },
  async refreshSelectedActionLogs() {
    const snapshot = logsStore.getState().action;
    if (!snapshot.selectedTaskId) {
      logsStore.setState((current) => ({
        ...current,
        action: {
          ...current.action,
          selectedTaskLogs: [],
          selectedTaskLogsLoading: false,
          selectedTaskLogsError: null,
        },
      }));
      return;
    }

    const requestId = snapshot.selectedTaskLogsRequestId + 1;
    logsStore.setState((current) => ({
      ...current,
      action: {
        ...current.action,
        selectedTaskLogsLoading: true,
        selectedTaskLogsError: null,
        selectedTaskLogsRequestId: requestId,
      },
    }));

    try {
      const page = await desktop.listLogPage({
        page: 1,
        pageSize: 20,
        taskIdFilter: snapshot.selectedTaskId,
      });

      if (logsStore.getState().action.selectedTaskLogsRequestId !== requestId) {
        return;
      }

      logsStore.setState((current) => ({
        ...current,
        action: {
          ...current.action,
          selectedTaskLogs: page.items,
          selectedTaskLogsLoading: false,
          selectedTaskLogsError: null,
        },
      }));
    } catch (error) {
      if (logsStore.getState().action.selectedTaskLogsRequestId !== requestId) {
        return;
      }

      logsStore.setState((current) => ({
        ...current,
        action: {
          ...current.action,
          selectedTaskLogsLoading: false,
          selectedTaskLogsError:
            error instanceof Error ? error.message : "Failed to load task logs",
        },
      }));
    }
  },
};

export { logsStore };
