import { createStore } from "../../store/createStore";
import * as desktop from "../../services/desktop";
import type {
  DesktopManualGateActionRequest,
  DesktopTaskItem,
  DesktopTaskWriteResult,
} from "../../types/desktop";

type TaskWorkbenchAction =
  | "retry"
  | "cancel"
  | "confirmManualGate"
  | "rejectManualGate";

type TaskActionPhase = "idle" | "running" | "success" | "error" | "blocked";
type TaskFeedbackTone = "neutral" | "success" | "warning" | "error";

interface TaskActionState {
  phase: TaskActionPhase;
  activeAction: TaskWorkbenchAction | null;
  pendingTaskIds: string[];
  attemptedCount: number;
  succeededCount: number;
  failedCount: number;
  skippedCount: number;
  message: string;
  tone: TaskFeedbackTone;
  updatedAt: string | null;
}

interface TasksState {
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
  selectedIds: string[];
  focusedTaskId: string | null;
  manualGateNote: string;
  action: TaskActionState;
}

interface TaskDesktopContracts {
  retryTask(taskId: string): Promise<DesktopTaskWriteResult>;
  cancelTask(taskId: string): Promise<DesktopTaskWriteResult>;
  confirmManualGate(
    request: DesktopManualGateActionRequest,
  ): Promise<DesktopTaskWriteResult>;
  rejectManualGate(
    request: DesktopManualGateActionRequest,
  ): Promise<DesktopTaskWriteResult>;
}

const taskDesktop = desktop as typeof desktop & Partial<TaskDesktopContracts>;

const RETRYABLE_STATUSES = new Set(["failed", "timed_out", "cancelled"]);
const CANCELLABLE_STATUSES = new Set(["pending", "queued", "running"]);

function createActionState(): TaskActionState {
  return {
    phase: "idle",
    activeAction: null,
    pendingTaskIds: [],
    attemptedCount: 0,
    succeededCount: 0,
    failedCount: 0,
    skippedCount: 0,
    message: "Select tasks to unlock retry, cancel, and manual-gate actions.",
    tone: "neutral",
    updatedAt: null,
  };
}

function getTaskActionMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

function getAvailableCommand(
  action: TaskWorkbenchAction,
): TaskDesktopContracts[TaskWorkbenchAction] {
  const command = taskDesktop[action];
  if (!command) {
    throw new Error(
      `Shared desktop API is not wired for ${action} yet. Wait for worker 1 to expose it in src/services/desktop.ts.`,
    );
  }

  return command as TaskDesktopContracts[TaskWorkbenchAction];
}

function dedupeStringValues(values: string[]): string[] {
  return Array.from(new Set(values));
}

function getActionLabel(action: TaskWorkbenchAction): string {
  switch (action) {
    case "retry":
      return "retry";
    case "cancel":
      return "cancel";
    case "confirmManualGate":
      return "confirm manual gate";
    case "rejectManualGate":
      return "reject manual gate";
  }
}

function getBlockedMessage(action: TaskWorkbenchAction): string {
  switch (action) {
    case "retry":
      return "Selected tasks are not in a retryable state. Only failed, timed out, or cancelled tasks are eligible.";
    case "cancel":
      return "Selected tasks are not cancellable. Only pending, queued, or running tasks can receive a best-effort cancel request.";
    case "confirmManualGate":
    case "rejectManualGate":
      return "Selected tasks do not carry a manual gate request yet.";
  }
}

function getRunningMessage(
  action: TaskWorkbenchAction,
  eligibleCount: number,
  skippedCount: number,
): string {
  const skippedSuffix =
    skippedCount > 0 ? ` ${skippedCount} mixed-selection row(s) were skipped before dispatch.` : "";

  switch (action) {
    case "retry":
      return `Submitting retry for ${eligibleCount} eligible task(s).${skippedSuffix}`;
    case "cancel":
      return `Submitting cancel for ${eligibleCount} eligible task(s). Cancel is best-effort, so already-finishing work may still complete.${skippedSuffix}`;
    case "confirmManualGate":
      return `Submitting manual-gate approval for ${eligibleCount} eligible task(s).${skippedSuffix}`;
    case "rejectManualGate":
      return `Submitting manual-gate rejection for ${eligibleCount} eligible task(s).${skippedSuffix}`;
  }
}

function getCompletionMessage(
  action: TaskWorkbenchAction,
  succeededCount: number,
  failedCount: number,
  skippedCount: number,
  fallback: string,
): string {
  const actionLabel = getActionLabel(action);
  const skippedSuffix =
    skippedCount > 0 ? ` ${skippedCount} ineligible row(s) were skipped.` : "";
  const cancelSuffix =
    action === "cancel"
      ? " Cancel remains best-effort until the next refresh confirms the final state."
      : "";

  if (failedCount > 0) {
    return `${actionLabel} finished with ${succeededCount} success and ${failedCount} failure.${skippedSuffix} ${fallback}${cancelSuffix}`.trim();
  }

  return `${actionLabel} completed for ${succeededCount} task(s).${skippedSuffix}${cancelSuffix}`;
}

const tasksStore = createStore<TasksState>({
  items: [],
  total: 0,
  page: 1,
  pageSize: 50,
  statusFilter: "all",
  searchInput: "",
  appliedSearch: "",
  isLoading: false,
  error: null,
  requestId: 0,
  selectedIds: [],
  focusedTaskId: null,
  manualGateNote: "",
  action: createActionState(),
});

export const taskActions = {
  setPage(page: number) {
    tasksStore.setState((current) => ({
      ...current,
      page: Math.max(1, page),
    }));
  },
  setPageSize(pageSize: number) {
    tasksStore.setState((current) => ({
      ...current,
      page: 1,
      pageSize,
    }));
  },
  setStatusFilter(statusFilter: string) {
    tasksStore.setState((current) => ({
      ...current,
      page: 1,
      statusFilter,
    }));
  },
  setSearchInput(searchInput: string) {
    tasksStore.setState((current) => ({
      ...current,
      searchInput,
    }));
  },
  applySearch(appliedSearch: string) {
    tasksStore.setState((current) => {
      if (current.appliedSearch === appliedSearch) {
        return current;
      }

      return {
        ...current,
        page: 1,
        appliedSearch,
      };
    });
  },
  toggleSelection(taskId: string) {
    tasksStore.setState((current) => {
      const selectedIds = current.selectedIds.includes(taskId)
        ? current.selectedIds.filter((item) => item !== taskId)
        : [...current.selectedIds, taskId];
      const focusedTaskId =
        current.focusedTaskId && selectedIds.includes(current.focusedTaskId)
          ? current.focusedTaskId
          : selectedIds[0] ?? current.focusedTaskId;

      return {
        ...current,
        selectedIds,
        focusedTaskId,
      };
    });
  },
  setSelection(taskIds: string[]) {
    tasksStore.setState((current) => {
      const visibleTaskIds = new Set(current.items.map((item) => item.id));
      const selectedIds = dedupeStringValues(taskIds).filter((taskId) =>
        visibleTaskIds.has(taskId),
      );

      return {
        ...current,
        selectedIds,
        focusedTaskId:
          current.focusedTaskId && selectedIds.includes(current.focusedTaskId)
            ? current.focusedTaskId
            : selectedIds[0] ?? null,
      };
    });
  },
  clearSelection() {
    tasksStore.setState((current) => ({
      ...current,
      selectedIds: [],
      focusedTaskId: null,
    }));
  },
  focusTask(taskId: string) {
    tasksStore.setState((current) => ({
      ...current,
      focusedTaskId: taskId,
      selectedIds: current.selectedIds.includes(taskId)
        ? current.selectedIds
        : [...current.selectedIds, taskId],
    }));
  },
  setManualGateNote(manualGateNote: string) {
    tasksStore.setState((current) => ({
      ...current,
      manualGateNote,
    }));
  },
  dismissActionFeedback() {
    tasksStore.setState((current) => ({
      ...current,
      action:
        current.action.phase === "running"
          ? current.action
          : {
              ...createActionState(),
              updatedAt: current.action.updatedAt,
            },
    }));
  },
  async refresh() {
    const snapshot = tasksStore.getState();
    const requestId = snapshot.requestId + 1;
    tasksStore.setState((current) => ({
      ...current,
      isLoading: true,
      error: null,
      requestId,
    }));

    try {
      const page = await desktop.listTaskPage({
        page: snapshot.page,
        pageSize: snapshot.pageSize,
        statusFilter:
          snapshot.statusFilter === "all" ? undefined : snapshot.statusFilter,
        search: snapshot.appliedSearch || undefined,
      });

      if (tasksStore.getState().requestId !== requestId) {
        return;
      }

      tasksStore.setState((current) => {
        const visibleTaskIds = new Set(page.items.map((item) => item.id));
        const selectedIds = current.selectedIds.filter((taskId) => visibleTaskIds.has(taskId));
        const focusedTaskId =
          current.focusedTaskId && visibleTaskIds.has(current.focusedTaskId)
            ? current.focusedTaskId
            : selectedIds[0] ?? page.items[0]?.id ?? null;

        return {
          ...current,
          items: page.items,
          total: page.total,
          page: page.page,
          pageSize: page.pageSize,
          isLoading: false,
          error: null,
          selectedIds,
          focusedTaskId,
        };
      });
    } catch (error) {
      if (tasksStore.getState().requestId !== requestId) {
        return;
      }

      tasksStore.setState((current) => ({
        ...current,
        isLoading: false,
        error: error instanceof Error ? error.message : "Failed to load tasks",
      }));
    }
  },
  async runAction(action: TaskWorkbenchAction) {
    const snapshot = tasksStore.getState();
    if (snapshot.action.phase === "running") {
      return;
    }

    const selectedTasks = snapshot.items.filter((item) =>
      snapshot.selectedIds.includes(item.id),
    );

    if (selectedTasks.length === 0) {
      tasksStore.setState((current) => ({
        ...current,
        action: {
          phase: "blocked",
          activeAction: action,
          pendingTaskIds: [],
          attemptedCount: 0,
          succeededCount: 0,
          failedCount: 0,
          skippedCount: 0,
          message: "Select at least one task before running a task action.",
          tone: "warning",
          updatedAt: new Date().toISOString(),
        },
      }));
      return;
    }

    const eligibleTasks = selectedTasks.filter((task) => {
      if (action === "retry") {
        return RETRYABLE_STATUSES.has(task.status);
      }
      if (action === "cancel") {
        return CANCELLABLE_STATUSES.has(task.status);
      }
      return Boolean(task.manualGateRequestId);
    });

    if (eligibleTasks.length === 0) {
      tasksStore.setState((current) => ({
        ...current,
        action: {
          phase: "blocked",
          activeAction: action,
          pendingTaskIds: [],
          attemptedCount: 0,
          succeededCount: 0,
          failedCount: 0,
          skippedCount: selectedTasks.length,
          message: getBlockedMessage(action),
          tone: "warning",
          updatedAt: new Date().toISOString(),
        },
      }));
      return;
    }

    const skippedCount = selectedTasks.length - eligibleTasks.length;

    tasksStore.setState((current) => ({
      ...current,
      action: {
        phase: "running",
        activeAction: action,
        pendingTaskIds: eligibleTasks.map((task) => task.id),
        attemptedCount: eligibleTasks.length,
        succeededCount: 0,
        failedCount: 0,
        skippedCount,
        message: getRunningMessage(action, eligibleTasks.length, skippedCount),
        tone: "neutral",
        updatedAt: new Date().toISOString(),
      },
    }));

    try {
      const command = getAvailableCommand(action);
      const note = snapshot.manualGateNote.trim();
      const results = await Promise.allSettled(
        eligibleTasks.map(async (task) => {
          if (action === "retry") {
            return command(task.id);
          }
          if (action === "cancel") {
            return command(task.id);
          }

          return command({
            manualGateRequestId: task.manualGateRequestId as string,
            note: note || undefined,
          });
        }),
      );

      const succeeded = results.filter((result) => result.status === "fulfilled");
      const failed = results.filter((result) => result.status === "rejected");
      const latestUpdatedAt =
        [...succeeded]
          .reverse()
          .find((result) => result.status === "fulfilled" && result.value.updatedAt)?.value
          .updatedAt ??
        new Date().toISOString();
      const successMessage =
        succeeded.length > 0
          ? succeeded
              .map((result) => (result.status === "fulfilled" ? result.value.message : null))
              .filter((value): value is string => Boolean(value))
              .at(-1) ?? null
          : null;

      tasksStore.setState((current) => ({
        ...current,
        action: {
          phase: failed.length > 0 ? "error" : "success",
          activeAction: action,
          pendingTaskIds: [],
          attemptedCount: eligibleTasks.length,
          succeededCount: succeeded.length,
          failedCount: failed.length,
          skippedCount,
          message:
            successMessage && failed.length === 0
              ? `${successMessage}${action === "cancel" ? " Cancel remains best-effort until the next refresh confirms the final state." : ""}${
                  skippedCount > 0 ? ` ${skippedCount} ineligible row(s) were skipped.` : ""
                }`
              : getCompletionMessage(
                  action,
                  succeeded.length,
                  failed.length,
                  skippedCount,
                  failed[0]?.status === "rejected"
                    ? getTaskActionMessage(failed[0].reason, "Please inspect the latest failure.")
                    : "Please inspect the latest failure.",
                ),
          tone: failed.length > 0 ? "error" : "success",
          updatedAt: latestUpdatedAt,
        },
        manualGateNote:
          action === "confirmManualGate" || action === "rejectManualGate"
            ? ""
            : current.manualGateNote,
      }));

      await taskActions.refresh();
    } catch (error) {
      tasksStore.setState((current) => ({
        ...current,
        action: {
          phase: "error",
          activeAction: action,
          pendingTaskIds: [],
          attemptedCount: eligibleTasks.length,
          succeededCount: 0,
          failedCount: eligibleTasks.length,
          skippedCount,
          message: getTaskActionMessage(error, `Failed to run ${action}.`),
          tone: "error",
          updatedAt: new Date().toISOString(),
        },
      }));
    }
  },
};

export { tasksStore };
export type { TaskFeedbackTone, TaskWorkbenchAction };
