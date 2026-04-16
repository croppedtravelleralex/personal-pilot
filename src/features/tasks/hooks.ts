import { startTransition, useDeferredValue, useEffect } from "react";

import { useDebouncedValue } from "../../hooks/useDebouncedValue";
import { useStore } from "../../store/createStore";
import type { DesktopTaskItem } from "../../types/desktop";
import { taskActions, tasksStore } from "./store";

const RETRYABLE_STATUSES = new Set(["failed", "timed_out", "cancelled"]);
const CANCELLABLE_STATUSES = new Set(["pending", "queued", "running"]);
const QUEUED_STATUSES = new Set(["pending", "queued"]);
const RUNNING_STATUSES = new Set(["running"]);
const FAILED_STATUSES = new Set(["failed", "timed_out", "cancelled"]);

type TaskLaneKey = "queued" | "running" | "failed" | "manualGate";

interface TaskLaneSummary {
  key: TaskLaneKey;
  label: string;
  description: string;
  visibleCount: number;
  selectedCount: number;
  readyCount: number;
  focusTaskId: string | null;
  actionLabel: string;
}

function getLaneTasks(
  lane: TaskLaneKey,
  items: DesktopTaskItem[],
) {
  return items.filter((item) => {
    if (lane === "queued") {
      return QUEUED_STATUSES.has(item.status);
    }
    if (lane === "running") {
      return RUNNING_STATUSES.has(item.status);
    }
    if (lane === "failed") {
      return FAILED_STATUSES.has(item.status);
    }

    return Boolean(item.manualGateRequestId);
  });
}

export function useTasksViewModel() {
  const state = useStore(tasksStore, (current) => current);
  const debouncedSearch = useDebouncedValue(state.searchInput, 300);
  const deferredSearch = useDeferredValue(debouncedSearch);

  useEffect(() => {
    if (state.appliedSearch !== deferredSearch) {
      startTransition(() => {
        taskActions.applySearch(deferredSearch);
      });
    }
  }, [deferredSearch, state.appliedSearch]);

  useEffect(() => {
    void taskActions.refresh();
  }, [state.page, state.pageSize, state.statusFilter, state.appliedSearch]);

  const totalPages = Math.max(1, Math.ceil(state.total / state.pageSize || 1));
  const selectedItems = state.items.filter((item) => state.selectedIds.includes(item.id));
  const focusedTask =
    state.items.find((item) => item.id === state.focusedTaskId) ??
    selectedItems[0] ??
    state.items[0] ??
    null;
  const visibleSelectedCount = selectedItems.length;
  const eligibleRetryCount = selectedItems.filter((item) => RETRYABLE_STATUSES.has(item.status)).length;
  const eligibleCancelCount = selectedItems.filter((item) => CANCELLABLE_STATUSES.has(item.status)).length;
  const eligibleManualGateCount = selectedItems.filter((item) => Boolean(item.manualGateRequestId)).length;
  const allVisibleSelected =
    state.items.length > 0 && state.items.every((item) => state.selectedIds.includes(item.id));
  const laneSummaries: TaskLaneSummary[] = [
    {
      key: "queued",
      label: "Queued / pending",
      description: "Ready to be picked up or already staged in queue.",
      visibleCount: getLaneTasks("queued", state.items).length,
      selectedCount: getLaneTasks("queued", selectedItems).length,
      readyCount: getLaneTasks("queued", selectedItems).length,
      focusTaskId: getLaneTasks("queued", state.items)[0]?.id ?? null,
      actionLabel: "Cancel-ready",
    },
    {
      key: "running",
      label: "Running",
      description: "Live executions that may accept cancel on a best-effort basis.",
      visibleCount: getLaneTasks("running", state.items).length,
      selectedCount: getLaneTasks("running", selectedItems).length,
      readyCount: getLaneTasks("running", selectedItems).length,
      focusTaskId: getLaneTasks("running", state.items)[0]?.id ?? null,
      actionLabel: "Best-effort cancel",
    },
    {
      key: "failed",
      label: "Failed / timed out",
      description: "Terminal work that operators can retry after diagnosis.",
      visibleCount: getLaneTasks("failed", state.items).length,
      selectedCount: getLaneTasks("failed", selectedItems).length,
      readyCount: getLaneTasks("failed", selectedItems).length,
      focusTaskId: getLaneTasks("failed", state.items)[0]?.id ?? null,
      actionLabel: "Retry-ready",
    },
    {
      key: "manualGate",
      label: "Manual gate",
      description: "Rows blocked on human confirmation or rejection.",
      visibleCount: getLaneTasks("manualGate", state.items).length,
      selectedCount: getLaneTasks("manualGate", selectedItems).length,
      readyCount: getLaneTasks("manualGate", selectedItems).length,
      focusTaskId: getLaneTasks("manualGate", state.items)[0]?.id ?? null,
      actionLabel: "Confirm / reject",
    },
  ];

  return {
    state,
    totalPages,
    selectedItems,
    focusedTask,
    visibleSelectedCount,
    allVisibleSelected,
    laneSummaries,
    eligibility: {
      retry: eligibleRetryCount,
      cancel: eligibleCancelCount,
      manualGate: eligibleManualGateCount,
    },
    actions: taskActions,
  };
}

export type { TaskLaneKey, TaskLaneSummary };
