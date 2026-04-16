import type { DesktopTaskItem } from "../../types/desktop";
import type { TaskLaneSummary } from "../../features/tasks/hooks";
import { formatRelativeTimestamp, formatStatusLabel } from "../../utils/format";

interface TaskActionPanelProps {
  selectedCount: number;
  focusedTask: DesktopTaskItem | null;
  manualGateNote: string;
  actionPhase: "idle" | "running" | "success" | "error" | "blocked";
  activeAction: string | null;
  feedbackMessage: string;
  actionAttemptedCount: number;
  actionSucceededCount: number;
  actionFailedCount: number;
  actionSkippedCount: number;
  laneSummaries: TaskLaneSummary[];
  retryEligibleCount: number;
  cancelEligibleCount: number;
  manualGateEligibleCount: number;
  onManualGateNoteChange: (value: string) => void;
}

function getFocusedTaskReadiness(task: DesktopTaskItem | null) {
  if (!task) {
    return [];
  }

  const readiness = [];

  if (task.status === "failed" || task.status === "timed_out" || task.status === "cancelled") {
    readiness.push("Retry can be sent from the current page selection.");
  }
  if (task.status === "pending" || task.status === "queued" || task.status === "running") {
    readiness.push("Cancel can be requested, but execution may still complete if the worker is already finishing.");
  }
  if (task.manualGateRequestId) {
    readiness.push("Manual gate can be approved or rejected because a gate id is attached.");
  }
  if (!task.errorMessage && !task.contentPreview) {
    readiness.push("Diagnosis detail is limited to current summary fields because no error or preview payload is present.");
  }

  return readiness;
}

export function TaskActionPanel({
  selectedCount,
  focusedTask,
  manualGateNote,
  actionPhase,
  activeAction,
  feedbackMessage,
  actionAttemptedCount,
  actionSucceededCount,
  actionFailedCount,
  actionSkippedCount,
  laneSummaries,
  retryEligibleCount,
  cancelEligibleCount,
  manualGateEligibleCount,
  onManualGateNoteChange,
}: TaskActionPanelProps) {
  const focusedReadiness = getFocusedTaskReadiness(focusedTask);

  return (
    <section className="panel">
      <div className="panel__header">
        <div>
          <span className="shell__eyebrow">Task Diagnosis</span>
          <h2 className="panel__title">Focused Task + Operator Boundaries</h2>
          <p className="panel__subtitle">
            This panel keeps the task-side truth visible: what the selected/focused row can do now, what the current batch actually attempted, and where the UI stops making promises.
          </p>
        </div>
        <span className={`badge ${actionPhase === "running" ? "badge--warning" : "badge--info"}`}>
          {activeAction ?? "idle"}
        </span>
      </div>

      <div className="page-stack">
        <div className="automation-metric-strip">
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Selected</span>
            <strong>{selectedCount}</strong>
            <small>Visible-page action scope</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Attempted</span>
            <strong>{actionAttemptedCount}</strong>
            <small>{actionSucceededCount} success / {actionFailedCount} fail</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Skipped</span>
            <strong>{actionSkippedCount}</strong>
            <small>Mixed-selection ineligible rows</small>
          </article>
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Gate note</span>
            <strong>{manualGateEligibleCount}</strong>
            <small>Rows ready for confirm / reject</small>
          </article>
        </div>

        <div
          style={{
            display: "grid",
            gridTemplateColumns: "minmax(0, 1.35fr) minmax(280px, 0.95fr)",
            gap: "1rem",
            alignItems: "start",
          }}
        >
          <div className="page-stack">
            {focusedTask ? (
              <article className="record-card">
                <div className="record-card__top">
                  <div>
                    <strong>{focusedTask.title ?? focusedTask.kind}</strong>
                    <p className="record-card__subline">Task {focusedTask.id}</p>
                  </div>
                  <span className={`badge badge--${focusedTask.status}`}>
                    {formatStatusLabel(focusedTask.status)}
                  </span>
                </div>
                <div className="record-card__meta">
                  <span>Profile {focusedTask.personaId ?? "N/A"}</span>
                  <span>Platform {focusedTask.platformId ?? "N/A"}</span>
                  <span>Priority {focusedTask.priority}</span>
                  <span>{focusedTask.isBrowserTask ? "Browser task" : "Non-browser task"}</span>
                  <span>{focusedTask.contentReady === false ? "Content not ready" : "Content ready / unknown"}</span>
                  <span>
                    {focusedTask.manualGateRequestId
                      ? `Manual gate ${focusedTask.manualGateRequestId}`
                      : "No manual gate"}
                  </span>
                </div>
                <p className="record-card__content">
                  {focusedTask.errorMessage ??
                    focusedTask.contentPreview ??
                    focusedTask.finalUrl ??
                    "No extra task detail is available yet."}
                </p>
                <div className="record-card__footer">
                  <span>Created {formatRelativeTimestamp(focusedTask.createdAt)}</span>
                  <span>Started {formatRelativeTimestamp(focusedTask.startedAt)}</span>
                  <span>Finished {formatRelativeTimestamp(focusedTask.finishedAt)}</span>
                </div>
              </article>
            ) : (
              <div className="banner banner--info">Select a task row to inspect its action context and diagnosis boundary.</div>
            )}

            <article className="record-card" style={{ minHeight: "unset" }}>
              <div className="record-card__top">
                <div>
                  <strong>Focused task readiness</strong>
                  <p className="record-card__subline">Action intent translated into operator-safe language.</p>
                </div>
                <span className="badge badge--info">Diagnosis</span>
              </div>
              {focusedReadiness.length > 0 ? (
                <div className="page-stack">
                  {focusedReadiness.map((item) => (
                    <div key={item} className="banner banner--info">
                      {item}
                    </div>
                  ))}
                </div>
              ) : (
                <div className="banner banner--info">
                  Focus a task to see whether it is retryable, cancellable, or waiting on a manual gate.
                </div>
              )}
            </article>

            <label className="field">
              <span className="field__label">Manual gate note</span>
              <textarea
                className="field__input"
                rows={4}
                value={manualGateNote}
                onChange={(event) => onManualGateNoteChange(event.target.value)}
                placeholder="Optional operator note for confirm / reject actions."
              />
            </label>
          </div>

          <div className="page-stack">
            <article className="record-card" style={{ minHeight: "unset" }}>
              <div className="record-card__top">
                <div>
                  <strong>Batch readiness by lane</strong>
                  <p className="record-card__subline">Current page only.</p>
                </div>
                <span className="badge badge--info">Selection</span>
              </div>
              <div className="page-stack">
                {laneSummaries.map((lane) => (
                  <div key={lane.key} className="toolbar-summary">
                    {lane.label}: {lane.selectedCount} selected / {lane.visibleCount} visible / {lane.readyCount} ready
                  </div>
                ))}
              </div>
            </article>

            <article className="record-card" style={{ minHeight: "unset" }}>
              <div className="record-card__top">
                <div>
                  <strong>Reality boundaries</strong>
                  <p className="record-card__subline">Hard lines the UI now states explicitly.</p>
                </div>
                <span className="badge badge--warning">No hidden promises</span>
              </div>
              <div className="page-stack">
                <div className="banner banner--info">
                  Cancel is best-effort only. A running task may still finish if the worker has already crossed its internal point of no return.
                </div>
                <div className="banner banner--info">
                  Confirm / reject only targets rows that already have a manual gate request id. Mixed selections are allowed, but ineligible rows are skipped.
                </div>
                <div className="banner banner--info">
                  Retry only targets failed, timed out, or cancelled tasks on the current visible page selection.
                </div>
                <div className="banner banner--info">
                  Live feedback reports dispatch truth from this UI layer; it does not infer deeper worker progress that the current Tauri contract does not expose.
                </div>
              </div>
            </article>

            <article className="record-card" style={{ minHeight: "unset" }}>
              <div className="record-card__top">
                <div>
                  <strong>Current batch message</strong>
                  <p className="record-card__subline">Latest operator-facing feedback.</p>
                </div>
                <span className="badge badge--info">{activeAction ?? "idle"}</span>
              </div>
              <p className="record-card__content">{feedbackMessage}</p>
              <div className="record-card__footer">
                <span>Retry-ready {retryEligibleCount}</span>
                <span>Cancel-ready {cancelEligibleCount}</span>
                <span>Manual-gate-ready {manualGateEligibleCount}</span>
              </div>
            </article>
          </div>
        </div>
      </div>
    </section>
  );
}
