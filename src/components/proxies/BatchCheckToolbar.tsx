import type { ProxyBatchScope } from "../../features/proxies/model";
import { formatCount, formatRelativeTimestamp } from "../../utils/format";

interface BatchCheckToolbarProps {
  scope: ProxyBatchScope;
  phase: "idle" | "queued" | "running" | "completed" | "blocked" | "error";
  feedbackTone: "neutral" | "success" | "warning" | "error";
  selectedCount: number;
  filteredCount: number;
  targetCount: number;
  completedCount: number;
  message: string;
  lastStartedAt: string | null;
  lastFinishedAt: string | null;
  onScopeChange: (scope: ProxyBatchScope) => void;
  onStart: () => void;
  onSelectVisible: () => void;
  onClearSelection: () => void;
  onDismiss: () => void;
}

function getPhaseLabel(phase: BatchCheckToolbarProps["phase"]): string {
  switch (phase) {
    case "queued":
      return "Queued";
    case "running":
      return "Running";
    case "completed":
      return "Completed";
    case "blocked":
      return "Blocked";
    case "error":
      return "Failed";
    default:
      return "Ready";
  }
}

function getPhaseBadge(phase: BatchCheckToolbarProps["phase"]): string {
  switch (phase) {
    case "queued":
    case "running":
      return "badge badge--warning";
    case "completed":
      return "badge badge--info";
    case "blocked":
    case "error":
      return "badge badge--failed";
    default:
      return "badge";
  }
}

export function BatchCheckToolbar({
  scope,
  phase,
  feedbackTone,
  selectedCount,
  filteredCount,
  targetCount,
  completedCount,
  message,
  lastStartedAt,
  lastFinishedAt,
  onScopeChange,
  onStart,
  onSelectVisible,
  onClearSelection,
  onDismiss,
}: BatchCheckToolbarProps) {
  const controlsDisabled = phase === "running";

  return (
    <section className="toolbar-card batch-toolbar">
      <div className="batch-toolbar__header">
        <div>
          <span className="shell__eyebrow">Batch Check</span>
          <h2 className="panel__title">Verification Queue</h2>
          <p className="panel__subtitle">
            Selection scope stays real, while native verify batches refresh the workbench and keep
            operator attention on health drift before reassignment or rotation.
          </p>
        </div>
        <span className={getPhaseBadge(phase)}>{getPhaseLabel(phase)}</span>
      </div>

      <div className="batch-toolbar__grid">
        <div className="batch-toolbar__scope">
          <span className="field__label">Batch scope</span>
          <div className="segmented-control">
            <button
              className={`segmented-control__item ${
                scope === "filtered" ? "segmented-control__item--active" : ""
              }`}
              type="button"
              disabled={controlsDisabled}
              onClick={() => onScopeChange("filtered")}
            >
              Filtered ({formatCount(filteredCount)})
            </button>
            <button
              className={`segmented-control__item ${
                scope === "selected" ? "segmented-control__item--active" : ""
              }`}
              type="button"
              disabled={controlsDisabled}
              onClick={() => onScopeChange("selected")}
            >
              Selected ({formatCount(selectedCount)})
            </button>
          </div>
        </div>

        <div className="batch-toolbar__meta">
          <div className="batch-toolbar__metric">
            <strong>{formatCount(targetCount)}</strong>
            <span>Current target</span>
          </div>
          <div className="batch-toolbar__metric">
            <strong>{formatCount(completedCount)}</strong>
            <span>Completed</span>
          </div>
          <div className="batch-toolbar__metric">
            <strong>{formatRelativeTimestamp(lastFinishedAt ?? lastStartedAt)}</strong>
            <span>Latest activity</span>
          </div>
        </div>

        <div className="batch-toolbar__actions">
          <button
            className="button button--secondary"
            type="button"
            disabled={controlsDisabled}
            onClick={onSelectVisible}
          >
            Select visible rows
          </button>
          <button
            className="button button--secondary"
            type="button"
            disabled={controlsDisabled}
            onClick={onClearSelection}
          >
            Clear selection
          </button>
          <button className="button" type="button" disabled={controlsDisabled} onClick={onStart}>
            {phase === "running" ? "Checking..." : "Start check"}
          </button>
        </div>
      </div>

      <div
        className={`batch-toolbar__feedback${
          feedbackTone === "neutral" ? "" : ` batch-toolbar__feedback--${feedbackTone}`
        }`}
        role="status"
      >
        <span>{message}</span>
        {phase !== "idle" ? (
          <button className="button button--secondary" type="button" onClick={onDismiss}>
            Dismiss
          </button>
        ) : null}
      </div>
    </section>
  );
}
