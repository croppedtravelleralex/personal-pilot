import { InlineContentPreview } from "../InlineContentPreview";
import { Panel } from "../Panel";
import { SearchInput } from "../SearchInput";
import { VirtualList } from "../VirtualList";
import type { DesktopTaskItem } from "../../types/desktop";
import type { TaskLaneKey, TaskLaneSummary } from "../../features/tasks/hooks";
import type { TaskFeedbackTone, TaskWorkbenchAction } from "../../features/tasks/store";
import {
  formatCount,
  formatRelativeTimestamp,
  formatStatusLabel,
} from "../../utils/format";

const TASK_STATUS_OPTIONS = [
  { value: "all", label: "All statuses" },
  { value: "queued", label: "Queued" },
  { value: "running", label: "Running" },
  { value: "succeeded", label: "Succeeded" },
  { value: "failed", label: "Failed" },
  { value: "timed_out", label: "Timed out" },
  { value: "cancelled", label: "Cancelled" },
];

const PAGE_SIZE_OPTIONS = [25, 50, 100];

interface TasksWorkbenchProps {
  items: DesktopTaskItem[];
  total: number;
  page: number;
  pageSize: number;
  totalPages: number;
  statusFilter: string;
  searchInput: string;
  isLoading: boolean;
  error: string | null;
  selectedIds: string[];
  focusedTaskId: string | null;
  allVisibleSelected: boolean;
  feedbackMessage: string;
  feedbackTone: TaskFeedbackTone;
  feedbackUpdatedAt: string | null;
  actionPhase: "idle" | "running" | "success" | "error" | "blocked";
  activeAction: TaskWorkbenchAction | null;
  pendingTaskIds: string[];
  actionAttemptedCount: number;
  actionSucceededCount: number;
  actionFailedCount: number;
  actionSkippedCount: number;
  laneSummaries: TaskLaneSummary[];
  retryEligibleCount: number;
  cancelEligibleCount: number;
  manualGateEligibleCount: number;
  onSearchInputChange: (value: string) => void;
  onStatusFilterChange: (value: string) => void;
  onPageSizeChange: (value: number) => void;
  onPageChange: (page: number) => void;
  onRefresh: () => Promise<void> | void;
  onToggleSelection: (taskId: string) => void;
  onToggleVisibleSelection: () => void;
  onSelectTask: (taskId: string) => void;
  onClearSelection: () => void;
  onRunAction: (action: TaskWorkbenchAction) => void;
  onDismissFeedback: () => void;
  onApplyLaneSelection: (lane: TaskLaneKey) => void;
}

function getActionLabel(
  action: TaskWorkbenchAction,
  phase: TasksWorkbenchProps["actionPhase"],
  activeAction: TasksWorkbenchProps["activeAction"],
): string {
  if (phase !== "running" || activeAction !== action) {
    switch (action) {
      case "retry":
        return "Retry batch";
      case "cancel":
        return "Cancel batch";
      case "confirmManualGate":
        return "Approve gate";
      case "rejectManualGate":
        return "Reject gate";
    }
  }

  switch (action) {
    case "retry":
      return "Retrying...";
    case "cancel":
      return "Cancelling...";
    case "confirmManualGate":
      return "Approving...";
    case "rejectManualGate":
      return "Rejecting...";
  }
}

function getFeedbackClassName(tone: TaskFeedbackTone): string {
  if (tone === "success") {
    return "batch-toolbar__feedback batch-toolbar__feedback--success";
  }
  if (tone === "warning") {
    return "batch-toolbar__feedback batch-toolbar__feedback--warning";
  }
  if (tone === "error") {
    return "batch-toolbar__feedback batch-toolbar__feedback--error";
  }
  return "batch-toolbar__feedback";
}

function getRowSignals(item: DesktopTaskItem) {
  const signals = [];

  if (item.status === "pending" || item.status === "queued") {
    signals.push("Queue");
  }
  if (item.status === "running") {
    signals.push("Live");
  }
  if (item.status === "failed" || item.status === "timed_out" || item.status === "cancelled") {
    signals.push("Needs diagnosis");
  }
  if (item.manualGateRequestId) {
    signals.push("Manual gate");
  }
  if (item.errorMessage) {
    signals.push("Error detail");
  }
  if (item.contentReady === false) {
    signals.push("Content missing");
  }

  return signals;
}

export function TasksWorkbench({
  items,
  total,
  page,
  pageSize,
  totalPages,
  statusFilter,
  searchInput,
  isLoading,
  error,
  selectedIds,
  focusedTaskId,
  allVisibleSelected,
  feedbackMessage,
  feedbackTone,
  feedbackUpdatedAt,
  actionPhase,
  activeAction,
  pendingTaskIds,
  actionAttemptedCount,
  actionSucceededCount,
  actionFailedCount,
  actionSkippedCount,
  laneSummaries,
  retryEligibleCount,
  cancelEligibleCount,
  manualGateEligibleCount,
  onSearchInputChange,
  onStatusFilterChange,
  onPageSizeChange,
  onPageChange,
  onRefresh,
  onToggleSelection,
  onToggleVisibleSelection,
  onSelectTask,
  onClearSelection,
  onRunAction,
  onDismissFeedback,
  onApplyLaneSelection,
}: TasksWorkbenchProps) {
  const pageStart = total === 0 ? 0 : (page - 1) * pageSize + 1;
  const pageEnd = Math.min(total, page * pageSize);
  const controlsDisabled = actionPhase === "running";
  const selectedCount = selectedIds.length;
  const liveControlCount = pendingTaskIds.length;

  return (
    <Panel
      title="Tasks Console"
      subtitle="Execution control is reorganized around queue lanes, live operator actions, and diagnosis-first inspection while keeping the existing paged + virtualized data path."
      actions={
        <span className={`badge ${isLoading ? "badge--warning" : "badge--info"}`}>
          {isLoading ? "Refreshing" : "Workbench ready"}
        </span>
      }
    >
      <div className="page-stack">
        {error ? (
          <div className="banner banner--error">
            <InlineContentPreview value={error} collapseAt={240} inlineLimit={4000} />
          </div>
        ) : null}

        <div className="automation-metric-strip">
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Selection scope</span>
            <strong>{formatCount(selectedCount)}</strong>
            <small>{formatCount(items.length)} rows on this page</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Retry-ready</span>
            <strong>{formatCount(retryEligibleCount)}</strong>
            <small>Failed / timed out / cancelled</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Cancel-ready</span>
            <strong>{formatCount(cancelEligibleCount)}</strong>
            <small>Queued / running, best-effort</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Manual gate</span>
            <strong>{formatCount(manualGateEligibleCount)}</strong>
            <small>Rows already carrying gate ids</small>
          </article>
        </div>

        <section className="toolbar-card batch-toolbar">
          <div className="batch-toolbar__header">
            <div>
              <span className="shell__eyebrow">Operator Deck</span>
              <h2 className="panel__title">Batch Control + Lane Focus</h2>
              <p className="panel__subtitle">
                Batch actions only target eligible rows already visible on the current page. Mixed selections stay allowed, but ineligible rows are skipped and reported back.
              </p>
            </div>
            <span className={`badge ${controlsDisabled ? "badge--warning" : "badge--info"}`}>
              {controlsDisabled ? "Action running" : "Ready for dispatch"}
            </span>
          </div>

          <div className="toolbar-grid">
            <SearchInput
              label="Search tasks"
              value={searchInput}
              placeholder="Run id, title, profile, platform"
              onChange={onSearchInputChange}
            />
            <label className="field">
              <span className="field__label">Status</span>
              <select
                className="field__input"
                value={statusFilter}
                onChange={(event) => onStatusFilterChange(event.target.value)}
              >
                {TASK_STATUS_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span className="field__label">Page size</span>
              <select
                className="field__input"
                value={pageSize}
                onChange={(event) => onPageSizeChange(Number(event.target.value))}
              >
                {PAGE_SIZE_OPTIONS.map((option) => (
                  <option key={option} value={option}>
                    {option} rows
                  </option>
                ))}
              </select>
            </label>
            <div className="toolbar-actions">
              <button
                className="button button--secondary"
                type="button"
                onClick={() => onPageChange(page - 1)}
                disabled={page <= 1}
              >
                Previous
              </button>
              <button
                className="button button--secondary"
                type="button"
                onClick={() => onPageChange(page + 1)}
                disabled={page >= totalPages}
              >
                Next
              </button>
              <button className="button" type="button" onClick={() => void onRefresh()}>
                {isLoading ? "Refreshing..." : "Refresh"}
              </button>
            </div>
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
              gap: "0.75rem",
            }}
          >
            {laneSummaries.map((lane) => (
              <article
                key={lane.key}
                className="record-card"
                style={{
                  minHeight: "unset",
                  borderColor: lane.selectedCount > 0 ? "rgba(86, 160, 255, 0.5)" : undefined,
                }}
              >
                <div className="record-card__top">
                  <div>
                    <strong>{lane.label}</strong>
                    <p className="record-card__subline">{lane.description}</p>
                  </div>
                  <span className="badge badge--info">{lane.actionLabel}</span>
                </div>
                <div className="record-card__meta">
                  <span>Visible {formatCount(lane.visibleCount)}</span>
                  <span>Selected {formatCount(lane.selectedCount)}</span>
                  <span>Ready now {formatCount(lane.readyCount)}</span>
                </div>
                <div className="record-card__footer">
                  <button
                    className="button button--secondary"
                    type="button"
                    disabled={controlsDisabled || lane.visibleCount === 0}
                    onClick={() => onApplyLaneSelection(lane.key)}
                  >
                    Select lane
                  </button>
                  <button
                    className="button button--secondary"
                    type="button"
                    disabled={!lane.focusTaskId}
                    onClick={() => lane.focusTaskId && onSelectTask(lane.focusTaskId)}
                  >
                    Focus first
                  </button>
                </div>
              </article>
            ))}
          </div>

          <div className="batch-toolbar__actions">
            <button
              className="button button--secondary"
              type="button"
              disabled={controlsDisabled || items.length === 0}
              onClick={onToggleVisibleSelection}
            >
              {allVisibleSelected ? "Clear visible rows" : "Select visible rows"}
            </button>
            <button
              className="button button--secondary"
              type="button"
              disabled={controlsDisabled || selectedCount === 0}
              onClick={onClearSelection}
            >
              Clear selection
            </button>
            <button
              className="button button--secondary"
              type="button"
              disabled={controlsDisabled || retryEligibleCount === 0}
              onClick={() => onRunAction("retry")}
            >
              {getActionLabel("retry", actionPhase, activeAction)}
            </button>
            <button
              className="button button--secondary"
              type="button"
              disabled={controlsDisabled || cancelEligibleCount === 0}
              onClick={() => onRunAction("cancel")}
            >
              {getActionLabel("cancel", actionPhase, activeAction)}
            </button>
            <button
              className="button button--secondary"
              type="button"
              disabled={controlsDisabled || manualGateEligibleCount === 0}
              onClick={() => onRunAction("confirmManualGate")}
            >
              {getActionLabel("confirmManualGate", actionPhase, activeAction)}
            </button>
            <button
              className="button button--secondary"
              type="button"
              disabled={controlsDisabled || manualGateEligibleCount === 0}
              onClick={() => onRunAction("rejectManualGate")}
            >
              {getActionLabel("rejectManualGate", actionPhase, activeAction)}
            </button>
          </div>

          <div
            style={{
              display: "grid",
              gridTemplateColumns: "minmax(0, 1.5fr) minmax(280px, 1fr)",
              gap: "0.75rem",
            }}
          >
            <div className={getFeedbackClassName(feedbackTone)} role="status">
              <span>
                {feedbackMessage}
                {feedbackUpdatedAt ? ` Updated ${formatRelativeTimestamp(feedbackUpdatedAt)}.` : ""}
              </span>
              {actionPhase !== "running" ? (
                <button className="button button--secondary" type="button" onClick={onDismissFeedback}>
                  Dismiss
                </button>
              ) : null}
            </div>

            <article className="record-card" style={{ minHeight: "unset" }}>
              <div className="record-card__top">
                <div>
                  <strong>Live control feedback</strong>
                  <p className="record-card__subline">Current dispatch truth, not optimistic UI.</p>
                </div>
                <span className={`badge ${actionPhase === "running" ? "badge--warning" : "badge--info"}`}>
                  {activeAction ?? "idle"}
                </span>
              </div>
              <div className="record-card__meta">
                <span>Attempted {formatCount(actionAttemptedCount)}</span>
                <span>Succeeded {formatCount(actionSucceededCount)}</span>
                <span>Failed {formatCount(actionFailedCount)}</span>
                <span>Skipped {formatCount(actionSkippedCount)}</span>
              </div>
              <p className="record-card__content">
                {liveControlCount > 0
                  ? `${formatCount(liveControlCount)} task(s) are still marked as pending in the current batch dispatch.`
                  : "No live dispatch is currently pending in the UI."}
              </p>
              <div className="record-card__footer">
                <span>Cancel remains best-effort until refresh confirms the final state.</span>
              </div>
            </article>
          </div>

          <div className="toolbar-summary">
            Showing {formatCount(pageStart)}-{formatCount(pageEnd)} of {formatCount(total)} tasks
          </div>
        </section>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "minmax(0, 1.9fr)",
            gap: "1rem",
            alignItems: "start",
          }}
        >
          <VirtualList
            items={items}
            height={720}
            itemHeight={252}
            getKey={(item) => item.id}
            renderItem={(item) => {
              const selected = selectedIds.includes(item.id);
              const focused = item.id === focusedTaskId;
              const pending = pendingTaskIds.includes(item.id);
              const signals = getRowSignals(item);

              return (
                <article
                  className={[
                    "record-card",
                    "record-card--interactive",
                    selected ? "record-card--selected" : "",
                    focused ? "record-card--selected" : "",
                  ]
                    .filter(Boolean)
                    .join(" ")}
                  onClick={() => onSelectTask(item.id)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === " ") {
                      event.preventDefault();
                      onSelectTask(item.id);
                    }
                  }}
                  tabIndex={0}
                >
                  <div className="record-card__top">
                    <label
                      className="field"
                      style={{ display: "inline-flex", alignItems: "center", gap: "0.6rem" }}
                      onClick={(event) => event.stopPropagation()}
                    >
                      <input
                        type="checkbox"
                        checked={selected}
                        onChange={() => onToggleSelection(item.id)}
                      />
                      <strong>{item.title ?? item.kind}</strong>
                    </label>
                    <span className={`badge badge--${item.status}`}>
                      {formatStatusLabel(item.status)}
                    </span>
                  </div>

                  <div className="record-card__meta">
                    <span>ID {item.id}</span>
                    <span>Priority {item.priority}</span>
                    <span>Profile {item.personaId ?? "N/A"}</span>
                    <span>Platform {item.platformId ?? "N/A"}</span>
                    <span>{item.isBrowserTask ? "Browser task" : "Non-browser task"}</span>
                    <span>{item.contentReady === false ? "Content not ready" : "Content ready / unknown"}</span>
                    <span>{item.manualGateRequestId ? `Gate ${item.manualGateRequestId}` : "No gate"}</span>
                  </div>

                  {signals.length > 0 ? (
                    <div
                      style={{
                        display: "flex",
                        flexWrap: "wrap",
                        gap: "0.45rem",
                        marginBottom: "0.75rem",
                      }}
                    >
                      {signals.map((signal) => (
                        <span key={signal} className="badge badge--info">
                          {signal}
                        </span>
                      ))}
                    </div>
                  ) : null}

                  <InlineContentPreview
                    className="record-card__content"
                    value={
                      item.errorMessage ??
                      item.contentPreview ??
                      item.finalUrl ??
                      "No preview or diagnostic detail was recorded for this task yet."
                    }
                    collapseAt={220}
                    expandable={false}
                    copyable={false}
                    mono={!item.errorMessage && !item.contentPreview && Boolean(item.finalUrl)}
                    muted={!item.errorMessage && !item.contentPreview}
                  />

                  <div className="record-card__footer">
                    <span>Created {formatRelativeTimestamp(item.createdAt)}</span>
                    <span>Started {formatRelativeTimestamp(item.startedAt)}</span>
                    <span>Finished {formatRelativeTimestamp(item.finishedAt)}</span>
                    <span>{pending ? "Dispatch pending..." : "Idle in UI"}</span>
                  </div>
                </article>
              );
            }}
          />
        </div>
      </div>
    </Panel>
  );
}
