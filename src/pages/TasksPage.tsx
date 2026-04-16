import { TaskActionPanel } from "../components/tasks/TaskActionPanel";
import { TasksWorkbench } from "../components/tasks/TasksWorkbench";
import type { TaskLaneKey } from "../features/tasks/hooks";
import { useTasksViewModel } from "../features/tasks/hooks";

type TasksViewModel = ReturnType<typeof useTasksViewModel>;

interface TasksWorkbenchSectionProps {
  viewModel: TasksViewModel;
  mergedIntoAutomation?: boolean;
}

function TasksWorkbenchSection({
  viewModel,
  mergedIntoAutomation = false,
}: TasksWorkbenchSectionProps) {
  const {
    state,
    totalPages,
    selectedItems,
    focusedTask,
    visibleSelectedCount,
    allVisibleSelected,
    laneSummaries,
    eligibility,
    actions,
  } = viewModel;

  function applyLaneSelection(lane: TaskLaneKey) {
    const laneIds = state.items
      .filter((item) => {
        if (lane === "queued") {
          return item.status === "pending" || item.status === "queued";
        }
        if (lane === "running") {
          return item.status === "running";
        }
        if (lane === "failed") {
          return item.status === "failed" || item.status === "timed_out" || item.status === "cancelled";
        }

        return Boolean(item.manualGateRequestId);
      })
      .map((item) => item.id);

    actions.setSelection(laneIds);
    if (laneIds[0]) {
      actions.focusTask(laneIds[0]);
    }
  }

  return (
    <div className="page-stack">
      <TasksWorkbench
        items={state.items}
        total={state.total}
        page={state.page}
        pageSize={state.pageSize}
        totalPages={totalPages}
        statusFilter={state.statusFilter}
        searchInput={state.searchInput}
        isLoading={state.isLoading}
        error={state.error}
        selectedIds={state.selectedIds}
        focusedTaskId={state.focusedTaskId}
        allVisibleSelected={allVisibleSelected}
        feedbackMessage={state.action.message}
        feedbackTone={state.action.tone}
        feedbackUpdatedAt={state.action.updatedAt}
        actionPhase={state.action.phase}
        activeAction={state.action.activeAction}
        pendingTaskIds={state.action.pendingTaskIds}
        actionAttemptedCount={state.action.attemptedCount}
        actionSucceededCount={state.action.succeededCount}
        actionFailedCount={state.action.failedCount}
        actionSkippedCount={state.action.skippedCount}
        laneSummaries={laneSummaries}
        retryEligibleCount={eligibility.retry}
        cancelEligibleCount={eligibility.cancel}
        manualGateEligibleCount={eligibility.manualGate}
        onSearchInputChange={actions.setSearchInput}
        onStatusFilterChange={actions.setStatusFilter}
        onPageSizeChange={actions.setPageSize}
        onPageChange={actions.setPage}
        onRefresh={actions.refresh}
        onToggleSelection={actions.toggleSelection}
        onToggleVisibleSelection={() =>
          actions.setSelection(allVisibleSelected ? [] : state.items.map((item) => item.id))
        }
        onSelectTask={actions.focusTask}
        onClearSelection={actions.clearSelection}
        onRunAction={(action) => void actions.runAction(action)}
        onDismissFeedback={actions.dismissActionFeedback}
        onApplyLaneSelection={applyLaneSelection}
      />

      <TaskActionPanel
        selectedCount={selectedItems.length}
        focusedTask={focusedTask}
        manualGateNote={state.manualGateNote}
        actionPhase={state.action.phase}
        activeAction={state.action.activeAction}
        feedbackMessage={state.action.message}
        actionAttemptedCount={state.action.attemptedCount}
        actionSucceededCount={state.action.succeededCount}
        actionFailedCount={state.action.failedCount}
        actionSkippedCount={state.action.skippedCount}
        laneSummaries={laneSummaries}
        retryEligibleCount={eligibility.retry}
        cancelEligibleCount={eligibility.cancel}
        manualGateEligibleCount={eligibility.manualGate}
        onManualGateNoteChange={actions.setManualGateNote}
      />
      <div className="toolbar-summary">
        {visibleSelectedCount} selected rows are visible on the current page.
        {mergedIntoAutomation
          ? " Tasks queue control now lives inside Automation Center while keeping existing debounce, paging, virtualization, and visible-page bulk-action boundaries."
          : " Search debounce, paging, and virtualization remain unchanged, and all bulk actions still stay inside the visible page boundary."}
      </div>
    </div>
  );
}

export function TasksPage() {
  const viewModel = useTasksViewModel();
  return <TasksWorkbenchSection viewModel={viewModel} />;
}

interface AutomationTasksSectionProps {
  viewModel: TasksViewModel;
}

export function AutomationTasksSection({ viewModel }: AutomationTasksSectionProps) {
  return <TasksWorkbenchSection viewModel={viewModel} mergedIntoAutomation />;
}
