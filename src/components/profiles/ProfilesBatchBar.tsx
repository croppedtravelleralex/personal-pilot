import type { ProfilesBatchAction } from "../../features/profiles/model";
import { formatRelativeTimestamp } from "../../utils/format";

const BATCH_ACTIONS: Array<{ action: ProfilesBatchAction; label: string }> = [
  { action: "open", label: "Open" },
  { action: "start", label: "Start" },
  { action: "stop", label: "Stop" },
  { action: "checkProxy", label: "Check Proxy" },
  { action: "sync", label: "Sync" },
];

interface ProfilesBatchBarProps {
  selectedCount: number;
  selectedVisibleCount: number;
  status: "idle" | "running" | "success" | "error";
  activeAction: ProfilesBatchAction | null;
  feedbackMessage: string | null;
  feedbackTone: "neutral" | "success" | "error";
  feedbackUpdatedAt: string | null;
  onRunAction: (action: ProfilesBatchAction) => void;
  onClearSelection: () => void;
  onDismissFeedback: () => void;
}

function getActionButtonLabel(
  action: ProfilesBatchAction,
  status: ProfilesBatchBarProps["status"],
  activeAction: ProfilesBatchAction | null,
  fallbackLabel: string,
): string {
  if (status !== "running" || activeAction !== action) {
    return fallbackLabel;
  }

  switch (action) {
    case "open":
      return "Opening...";
    case "start":
      return "Starting...";
    case "stop":
      return "Stopping...";
    case "checkProxy":
      return "Checking...";
    case "sync":
      return "Syncing...";
    default:
      return fallbackLabel;
  }
}

export function ProfilesBatchBar({
  selectedCount,
  selectedVisibleCount,
  status,
  activeAction,
  feedbackMessage,
  feedbackTone,
  feedbackUpdatedAt,
  onRunAction,
  onClearSelection,
  onDismissFeedback,
}: ProfilesBatchBarProps) {
  const isActive = selectedCount > 0;
  const disabled = selectedCount === 0 || status === "running";
  const feedbackClassName =
    feedbackTone === "success"
      ? "profiles-batch-bar__note batch-toolbar__feedback batch-toolbar__feedback--success"
      : feedbackTone === "error"
        ? "profiles-batch-bar__note batch-toolbar__feedback batch-toolbar__feedback--error"
        : "profiles-batch-bar__note batch-toolbar__feedback";

  return (
    <section className={`profiles-batch-bar ${isActive ? "is-active" : "is-idle"}`}>
      <div className="profiles-batch-bar__summary">
        <span className="badge badge--info">BatchBar</span>
        <strong>
          {selectedCount === 0
            ? "Select rows to stage batch operations"
            : `${selectedCount} profiles selected`}
        </strong>
        <span>
          {selectedCount === 0
            ? "Selection is wired to the real table state. Batch actions are ready once rows are selected."
            : status === "running"
              ? `${selectedVisibleCount} visible in the current filtered result. Native action is running now.`
              : `${selectedVisibleCount} visible in the current filtered result. Native actions refresh the workbench after completion.`}
        </span>
      </div>

      <div className="profiles-batch-bar__actions">
        {BATCH_ACTIONS.map((item) => (
          <button
            key={item.action}
            className="button button--secondary"
            type="button"
            disabled={disabled}
            onClick={() => onRunAction(item.action)}
          >
            {getActionButtonLabel(item.action, status, activeAction, item.label)}
          </button>
        ))}
        <button
          className="button button--secondary"
          type="button"
          disabled={disabled}
          onClick={onClearSelection}
        >
          Clear Selection
        </button>
      </div>

      {feedbackMessage ? (
        <div className={feedbackClassName} role="status">
          <span>
            {feedbackMessage}
            {feedbackUpdatedAt ? ` Updated ${formatRelativeTimestamp(feedbackUpdatedAt)}.` : ""}
          </span>
          {status !== "running" ? (
            <button className="button button--secondary" type="button" onClick={onDismissFeedback}>
              Dismiss
            </button>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
