import type { DesktopLogItem, DesktopTaskItem } from "../../types/desktop";
import type { LogsState } from "./store";

type LogsMetricTone = "neutral" | "success" | "warning" | "danger";
type LogsNoticeTone = "info" | "warning" | "error";

export interface LogsConsoleMetric {
  label: string;
  value: string;
  detail: string;
  tone: LogsMetricTone;
}

export interface LogsAttentionItem {
  id: string;
  tone: LogsNoticeTone;
  title: string;
  detail: string;
}

export interface LogsSelectedTaskSummary {
  label: string;
  detail: string;
  tone: LogsMetricTone;
}

export interface LogsConsoleSummary {
  metrics: LogsConsoleMetric[];
  attentionItems: LogsAttentionItem[];
  selectedTask: LogsSelectedTaskSummary;
  runtimeErrorCount: number;
  runtimeWarnCount: number;
  actionFailureCount: number;
  actionRunningCount: number;
  manualGateCount: number;
  selectedTraceCount: number;
  selectedTraceLabel: string;
  operatorHeadline: string;
  operatorDetail: string;
}

function countLogsByLevel(items: DesktopLogItem[], level: string) {
  return items.filter((item) => item.level.toLowerCase() === level).length;
}

function findLatestLogByLevel(items: DesktopLogItem[], level: string) {
  return items.find((item) => item.level.toLowerCase() === level) ?? null;
}

function findLatestTaskByStatus(items: DesktopTaskItem[], statuses: string[]) {
  return items.find((item) => statuses.includes(item.status)) ?? null;
}

function getSelectedTaskSummary(task: DesktopTaskItem | null): LogsSelectedTaskSummary {
  if (!task) {
    return {
      label: "No task selected",
      detail: "Pick an action task to inspect its runtime trace and failure context.",
      tone: "neutral",
    };
  }

  if (["failed", "timed_out", "cancelled"].includes(task.status)) {
    return {
      label: "Needs review",
      detail:
        task.errorMessage ??
        "This task did not complete successfully. Open the detail panel to inspect the runtime trace.",
      tone: "danger",
    };
  }

  if (["running", "queued", "pending"].includes(task.status)) {
    return {
      label: "Still active",
      detail: "The selected task is still moving through the queue or currently executing.",
      tone: "warning",
    };
  }

  return {
    label: "Healthy",
    detail: "The selected task completed successfully and can be used as a clean reference sample.",
    tone: "success",
  };
}

export function buildLogsConsoleSummary(state: LogsState): LogsConsoleSummary {
  const runtimeErrorCount = countLogsByLevel(state.runtime.items, "error");
  const runtimeWarnCount = countLogsByLevel(state.runtime.items, "warn");
  const actionFailureCount = state.action.items.filter((item) =>
    ["failed", "timed_out", "cancelled"].includes(item.status),
  ).length;
  const actionRunningCount = state.action.items.filter((item) =>
    ["running", "queued", "pending"].includes(item.status),
  ).length;
  const manualGateCount = state.action.items.filter((item) => item.manualGateRequestId).length;
  const selectedTraceCount = state.action.selectedTaskLogs.length;
  const selectedTask = getSelectedTaskSummary(state.action.selectedTaskSnapshot);
  const attentionItems: LogsAttentionItem[] = [];
  const latestRuntimeError = findLatestLogByLevel(state.runtime.items, "error");
  const latestFailedTask = findLatestTaskByStatus(state.action.items, [
    "failed",
    "timed_out",
    "cancelled",
  ]);

  if (state.runtime.error) {
    attentionItems.push({
      id: "runtime-error",
      tone: "error",
      title: "Runtime log stream failed to load",
      detail: state.runtime.error,
    });
  }

  if (state.action.error) {
    attentionItems.push({
      id: "action-error",
      tone: "error",
      title: "Action task stream failed to load",
      detail: state.action.error,
    });
  }

  if (state.action.selectedTaskLogsError) {
    attentionItems.push({
      id: "selected-task-logs",
      tone: "warning",
      title: "Selected task trace is incomplete",
      detail: state.action.selectedTaskLogsError,
    });
  }

  if (latestRuntimeError) {
    attentionItems.push({
      id: "runtime-signal",
      tone: "warning",
      title: `${runtimeErrorCount} runtime errors on the current page`,
      detail: latestRuntimeError.message,
    });
  }

  if (latestFailedTask) {
    attentionItems.push({
      id: "task-signal",
      tone: "warning",
      title: `${actionFailureCount} action tasks need review`,
      detail: latestFailedTask.title ?? latestFailedTask.errorMessage ?? latestFailedTask.kind,
    });
  }

  if (state.runtime.appliedTaskId) {
    attentionItems.push({
      id: "task-pin",
      tone: "info",
      title: "Runtime logs are pinned to one task",
      detail: `The runtime view is currently scoped to task ${state.runtime.appliedTaskId}.`,
    });
  }

  if (state.action.selectedTaskSnapshot?.manualGateRequestId) {
    attentionItems.push({
      id: "manual-gate",
      tone: "info",
      title: "Selected task is waiting on a manual gate",
      detail: `Gate request ${state.action.selectedTaskSnapshot.manualGateRequestId} is attached to the selected task.`,
    });
  }

  const selectedTraceLabel =
    !state.action.selectedTaskSnapshot
      ? "No task linked"
      : selectedTraceCount > 0
        ? `${selectedTraceCount} runtime rows linked`
        : "No runtime rows linked yet";
  const operatorHeadline =
    runtimeErrorCount > 0
      ? "Runtime noise is the primary review lane."
      : actionFailureCount > 0
        ? "Task outcomes need review even though runtime noise is quieter."
        : manualGateCount > 0
          ? "Manual gates are the main operator checkpoint."
          : "The local logs console is mostly in watch mode.";
  const operatorDetail =
    runtimeErrorCount > 0
      ? `The current runtime page shows ${runtimeErrorCount} errors and ${runtimeWarnCount} warnings, so infrastructure-side trace review should come first.`
      : actionFailureCount > 0
        ? `${actionFailureCount} action tasks on the current page are not in a success state, so the task lane deserves priority.`
        : manualGateCount > 0
          ? `${manualGateCount} action tasks on the current page carry manual-gate markers, so human approval latency is the likely bottleneck.`
          : "No immediate runtime or action anomalies dominate the current local page sample.";

  return {
    metrics: [
      {
        label: "Current view",
        value: state.viewMode === "runtime" ? "Runtime logs" : "Action logs",
        detail:
          state.viewMode === "runtime"
            ? state.runtime.appliedTaskId
              ? `Scoped to task ${state.runtime.appliedTaskId}`
              : "Broad runtime feed for the local desktop"
            : selectedTraceLabel,
        tone: "neutral",
      },
      {
        label: "Runtime signal",
        value: String(runtimeErrorCount),
        detail: `${runtimeWarnCount} warnings on the current page`,
        tone: runtimeErrorCount > 0 ? "danger" : runtimeWarnCount > 0 ? "warning" : "success",
      },
      {
        label: "Action risk",
        value: String(actionFailureCount),
        detail: `${actionRunningCount} queued or running tasks on the current page`,
        tone: actionFailureCount > 0 ? "warning" : actionRunningCount > 0 ? "neutral" : "success",
      },
      {
        label: "Manual gates",
        value: String(manualGateCount),
        detail: "Manual-review markers on the current action page",
        tone: manualGateCount > 0 ? "warning" : "success",
      },
      {
        label: "Selected task",
        value: selectedTask.label,
        detail: selectedTask.detail,
        tone: selectedTask.tone,
      },
    ],
    attentionItems: attentionItems.slice(0, 4),
    selectedTask,
    runtimeErrorCount,
    runtimeWarnCount,
    actionFailureCount,
    actionRunningCount,
    manualGateCount,
    selectedTraceCount,
    selectedTraceLabel,
    operatorHeadline,
    operatorDetail,
  };
}
