import { EmptyState } from "../components/EmptyState";
import {
  InlineContentPreview,
  truncateInlineContent,
} from "../components/InlineContentPreview";
import { Panel } from "../components/Panel";
import { SearchInput } from "../components/SearchInput";
import { StatCard } from "../components/StatCard";
import { VirtualList } from "../components/VirtualList";
import { useLogsViewModel } from "../features/logs/hooks";
import {
  formatCount,
  formatRelativeTimestamp,
  formatStatusLabel,
} from "../utils/format";

const LOG_LEVEL_OPTIONS = [
  { value: "all", label: "All levels" },
  { value: "info", label: "Info" },
  { value: "warn", label: "Warn" },
  { value: "error", label: "Error" },
  { value: "debug", label: "Debug" },
];

const PAGE_SIZE_OPTIONS = [50, 100, 150];
const ACTION_PAGE_SIZE_OPTIONS = [20, 50, 100];
const ACTION_STATUS_OPTIONS = [
  { value: "all", label: "All statuses" },
  { value: "queued", label: "Queued" },
  { value: "running", label: "Running" },
  { value: "succeeded", label: "Succeeded" },
  { value: "failed", label: "Failed" },
  { value: "timed_out", label: "Timed out" },
  { value: "cancelled", label: "Cancelled" },
];

function getBadgeTone(tone: "neutral" | "success" | "warning" | "danger") {
  if (tone === "danger") {
    return "error";
  }

  if (tone === "success") {
    return "succeeded";
  }

  return tone === "neutral" ? "info" : tone;
}

function getLogBadgeTone(level: string) {
  const normalized = level.toLowerCase();
  if (normalized === "error") {
    return "error";
  }
  if (normalized === "warn") {
    return "warning";
  }
  if (normalized === "info") {
    return "info";
  }
  return "info";
}

export function LogsPage() {
  const { state, summary, runtimeTotalPages, actionTotalPages, actions } = useLogsViewModel();
  const runtime = state.runtime;
  const action = state.action;
  const runtimePageStart =
    runtime.total === 0 ? 0 : (runtime.page - 1) * runtime.pageSize + 1;
  const runtimePageEnd = Math.min(runtime.total, runtime.page * runtime.pageSize);
  const actionPageStart =
    action.total === 0 ? 0 : (action.page - 1) * action.pageSize + 1;
  const actionPageEnd = Math.min(action.total, action.page * action.pageSize);

  return (
    <div className="page-stack">
      {runtime.error ? (
        <div className="banner banner--error">
          <InlineContentPreview value={runtime.error} collapseAt={280} inlineLimit={4000} />
        </div>
      ) : null}
      {action.error ? (
        <div className="banner banner--error">
          <InlineContentPreview value={action.error} collapseAt={280} inlineLimit={4000} />
        </div>
      ) : null}
      {state.info ? (
        <div className="banner banner--info">
          <InlineContentPreview value={state.info} collapseAt={280} inlineLimit={4000} />
        </div>
      ) : null}

      <div className="toolbar-card logs-toolbar">
        <div className="automation-center__hero">
          <div className="automation-center__hero-copy">
            <span className="shell__eyebrow">Logs Console</span>
            <h2>Local Runtime and Action Review Desk</h2>
            <p>
              Keep raw runtime noise and task-facing outcomes in one local review console so
              operators can pivot from an exception summary into the actual SQLite-backed traces.
            </p>
          </div>
          <div className="automation-center__hero-aside">
            <span className="badge badge--info">
              {state.viewMode === "runtime" ? "Runtime focus" : "Action focus"}
            </span>
            <span className={`badge badge--${getBadgeTone(summary.selectedTask.tone)}`}>
              {summary.selectedTask.label}
            </span>
            <span className="badge badge--info">{summary.selectedTraceLabel}</span>
            {runtime.appliedTaskId ? (
              <button
                className="button button--secondary"
                type="button"
                onClick={() => {
                  actions.setRuntimeTaskIdInput("");
                  actions.applyRuntimeTaskId("");
                }}
              >
                Clear task scope
              </button>
            ) : null}
            <button
              className="button"
              type="button"
              onClick={() =>
                void (state.viewMode === "runtime"
                  ? actions.refreshRuntime()
                  : actions.refreshActionTasks())
              }
            >
              {state.viewMode === "runtime"
                ? runtime.isLoading
                  ? "Refreshing..."
                  : "Refresh runtime"
                : action.isLoading
                  ? "Refreshing..."
                  : "Refresh actions"}
            </button>
          </div>
        </div>

        <div className="automation-metric-strip">
          {summary.metrics.slice(0, 4).map((metric) => (
            <article className="automation-metric-strip__item" key={metric.label}>
              <span className="automation-metric-strip__label">{metric.label}</span>
              <strong>{metric.value}</strong>
              <small>{metric.detail}</small>
            </article>
          ))}
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Truth boundary</span>
            <strong>Local only</strong>
            <small>SQLite task rows and local runtime logs only</small>
          </article>
        </div>

        <div className="page-grid page-grid--two">
          <article className="contract-card">
            <div className="contract-card__top">
              <strong>{summary.operatorHeadline}</strong>
              <span className="badge badge--info">
                {state.viewMode === "runtime" ? "Trace lane" : "Task lane"}
              </span>
            </div>
            <p>{summary.operatorDetail}</p>
          </article>
          <article className="contract-card">
            <div className="contract-card__top">
              <strong>Navigation posture</strong>
              <span className={`badge badge--${getBadgeTone(summary.selectedTask.tone)}`}>
                {summary.selectedTraceLabel}
              </span>
            </div>
            <p>
              Runtime view is for infrastructure-side noise and scoped task traces. Action view is for
              manual gates, task outcomes, and the selected task&apos;s related runtime excerpt.
            </p>
          </article>
        </div>

        {summary.attentionItems.length > 0 ? (
          <div className="contract-list">
            {summary.attentionItems.map((item) => (
              <article className="contract-card" key={item.id}>
                <div className="contract-card__top">
                  <strong>{item.title}</strong>
                  <span className={`badge badge--${item.tone}`}>{item.tone}</span>
                </div>
                <p>{item.detail}</p>
              </article>
            ))}
          </div>
        ) : (
          <div className="banner banner--info">
            No dominant local log issue on the current page sample. This does not imply anything about remote or historical runs outside the loaded pages.
          </div>
        )}
      </div>

      <div className="stat-grid">
        {summary.metrics.map((metric) => (
          <StatCard
            key={`stat-${metric.label}`}
            label={metric.label}
            value={metric.value}
            hint={metric.detail}
            tone={metric.tone}
          />
        ))}
      </div>

      <Panel
        title="Log Navigation"
        subtitle="Switch between local runtime rows and action-task review without leaving the console"
        actions={
          <div className="segmented-control">
            <button
              className={`segmented-control__item${
                state.viewMode === "runtime" ? " segmented-control__item--active" : ""
              }`}
              type="button"
              onClick={() => actions.setViewMode("runtime")}
            >
              Runtime Logs
            </button>
            <button
              className={`segmented-control__item${
                state.viewMode === "action" ? " segmented-control__item--active" : ""
              }`}
              type="button"
              onClick={() => actions.setViewMode("action")}
            >
              Action Logs
            </button>
          </div>
        }
      >
        {state.viewMode === "runtime" ? (
          <>
            <div className="toolbar-grid toolbar-grid--logs">
              <SearchInput
                label="Message search"
                value={runtime.searchInput}
                placeholder="Search log message text"
                onChange={actions.setRuntimeSearchInput}
              />
              <SearchInput
                label="Task id"
                value={runtime.taskIdInput}
                placeholder="Filter by task id"
                onChange={actions.setRuntimeTaskIdInput}
              />
              <label className="field">
                <span className="field__label">Level</span>
                <select
                  className="field__input"
                  value={runtime.levelFilter}
                  onChange={(event) => actions.setRuntimeLevelFilter(event.target.value)}
                >
                  {LOG_LEVEL_OPTIONS.map((option) => (
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
                  value={runtime.pageSize}
                  onChange={(event) => actions.setRuntimePageSize(Number(event.target.value))}
                >
                  {PAGE_SIZE_OPTIONS.map((pageSize) => (
                    <option key={pageSize} value={pageSize}>
                      {pageSize} rows
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <div className="toolbar-actions toolbar-actions--spread">
              <span className="toolbar-summary">
                Showing {formatCount(runtimePageStart)}-{formatCount(runtimePageEnd)} of{" "}
                {formatCount(runtime.total)}
              </span>
              <div className="toolbar-actions">
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => actions.setRuntimePage(runtime.page - 1)}
                  disabled={runtime.page <= 1}
                >
                  Previous
                </button>
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => actions.setRuntimePage(runtime.page + 1)}
                  disabled={runtime.page >= runtimeTotalPages}
                >
                  Next
                </button>
                <button
                  className="button"
                  type="button"
                  onClick={() => void actions.refreshRuntime()}
                >
                  {runtime.isLoading ? "Refreshing..." : "Refresh"}
                </button>
              </div>
            </div>
          </>
        ) : (
          <>
            <div className="toolbar-grid toolbar-grid--logs">
              <SearchInput
                label="Task search"
                value={action.searchInput}
                placeholder="Search task title, kind, or content preview"
                onChange={actions.setActionSearchInput}
              />
              <label className="field">
                <span className="field__label">Status</span>
                <select
                  className="field__input"
                  value={action.statusFilter}
                  onChange={(event) => actions.setActionStatusFilter(event.target.value)}
                >
                  {ACTION_STATUS_OPTIONS.map((option) => (
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
                  value={action.pageSize}
                  onChange={(event) => actions.setActionPageSize(Number(event.target.value))}
                >
                  {ACTION_PAGE_SIZE_OPTIONS.map((pageSize) => (
                    <option key={pageSize} value={pageSize}>
                      {pageSize} rows
                    </option>
                  ))}
                </select>
              </label>
              <div className="field">
                <span className="field__label">Selected action</span>
                <div className="logs-toolbar__selected-task">
                  {action.selectedTaskSnapshot?.title ??
                    action.selectedTaskSnapshot?.kind ??
                    "Pick a task to inspect its related logs"}
                </div>
              </div>
            </div>
            <div className="toolbar-actions toolbar-actions--spread">
              <span className="toolbar-summary">
                Showing {formatCount(actionPageStart)}-{formatCount(actionPageEnd)} of{" "}
                {formatCount(action.total)} action tasks
              </span>
              <div className="toolbar-actions">
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => actions.setActionPage(action.page - 1)}
                  disabled={action.page <= 1}
                >
                  Previous
                </button>
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => actions.setActionPage(action.page + 1)}
                  disabled={action.page >= actionTotalPages}
                >
                  Next
                </button>
                <button
                  className="button"
                  type="button"
                  onClick={() => void actions.refreshActionTasks()}
                >
                  {action.isLoading ? "Refreshing..." : "Refresh"}
                </button>
              </div>
            </div>
          </>
        )}
      </Panel>

      {state.viewMode === "runtime" ? (
        <div className="page-grid page-grid--two">
          <Panel
            title="Runtime Signal Board"
            subtitle="What the current local runtime page is telling the operator before they inspect raw rows"
            actions={
              <div className="toolbar-actions">
                <span className={`badge badge--${summary.runtimeErrorCount > 0 ? "error" : "succeeded"}`}>
                  {summary.runtimeErrorCount} errors
                </span>
                <span className={`badge badge--${summary.runtimeWarnCount > 0 ? "warning" : "info"}`}>
                  {summary.runtimeWarnCount} warnings
                </span>
              </div>
            }
          >
            <div className="details-grid details-grid--stacked">
              <article className="details-grid__item">
                <dt>Scope</dt>
                <dd>
                  {runtime.appliedTaskId ? `Task ${runtime.appliedTaskId}` : "Whole local runtime feed"}
                  <br />
                  {runtime.appliedSearch ? `Search: ${runtime.appliedSearch}` : "No text scope applied"}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Level filter</dt>
                <dd>{runtime.levelFilter === "all" ? "All levels" : runtime.levelFilter.toUpperCase()}</dd>
              </article>
              <article className="details-grid__item">
                <dt>Loaded page</dt>
                <dd>
                  {formatCount(runtimePageStart)}-{formatCount(runtimePageEnd)} of {formatCount(runtime.total)}
                </dd>
              </article>
              <article className="details-grid__item">
                <dt>Reality boundary</dt>
                <dd>
                  Local runtime rows only
                  <br />
                  No cloud fleet log stream is represented here
                </dd>
              </article>
            </div>
          </Panel>

          <Panel
            title="Local Runtime Log Stream"
            subtitle="Paged local log rows with quick operator context"
          >
            {runtime.items.length === 0 ? (
              <EmptyState
                title="No runtime logs found"
                detail="Adjust filters or wait for local runs to emit logs."
              />
            ) : (
              <VirtualList
                items={runtime.items}
                height={640}
                itemHeight={148}
                getKey={(item) => item.id}
                renderItem={(item) => (
                  <article className="record-card record-card--log">
                    <div className="record-card__top">
                      <div>
                        <strong>{truncateInlineContent(item.message, 150)}</strong>
                        <p className="record-card__subline">
                          Task {item.taskId} {item.runId ? `| Run ${item.runId}` : ""}
                        </p>
                      </div>
                      <span className={`badge badge--${getLogBadgeTone(item.level)}`}>
                        {item.level.toUpperCase()}
                      </span>
                    </div>
                    <div className="automation-pill-list">
                      <span className="automation-pill">Log {item.id}</span>
                      <span className="automation-pill">
                        {item.level.toLowerCase() === "error"
                          ? "Needs review"
                          : item.level.toLowerCase() === "warn"
                            ? "Watch retries"
                            : "Routine trace"}
                      </span>
                    </div>
                    <InlineContentPreview
                      className="record-card__content"
                      value={item.message}
                      collapseAt={220}
                      expandable={false}
                      copyable={false}
                    />
                    <div className="record-card__footer">
                      <span>Log id: {item.id}</span>
                      <span>{formatRelativeTimestamp(item.createdAt)}</span>
                    </div>
                  </article>
                )}
              />
            )}
          </Panel>
        </div>
      ) : (
        <div className="page-grid page-grid--two">
          <Panel
            title="Action Timeline"
            subtitle="Task-centric action stream backed by the local queue table"
            actions={
              <div className="toolbar-actions">
                <span className={`badge badge--${summary.actionFailureCount > 0 ? "warning" : "succeeded"}`}>
                  {summary.actionFailureCount} need review
                </span>
                <span className={`badge badge--${summary.manualGateCount > 0 ? "info" : "succeeded"}`}>
                  {summary.manualGateCount} manual gates
                </span>
              </div>
            }
          >
            {action.items.length === 0 ? (
              <EmptyState
                title="No action tasks found"
                detail="Run local actions or loosen filters to populate the action log view."
              />
            ) : (
              <VirtualList
                items={action.items}
                height={640}
                itemHeight={196}
                getKey={(item) => item.id}
                renderItem={(item) => (
                  <article
                    className={`record-card record-card--interactive${
                      action.selectedTaskId === item.id ? " record-card--selected" : ""
                    }`}
                    onClick={() => actions.selectActionTask(item.id)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        actions.selectActionTask(item.id);
                      }
                    }}
                    role="button"
                    tabIndex={0}
                  >
                    <div className="record-card__top">
                      <div>
                        <strong>{item.title ?? item.kind}</strong>
                        <p className="record-card__subline">
                          {item.id} | {item.kind}
                        </p>
                      </div>
                      <span className={`badge badge--${getBadgeTone(
                        ["failed", "timed_out", "cancelled"].includes(item.status)
                          ? "danger"
                          : ["running", "queued", "pending"].includes(item.status)
                            ? "warning"
                            : item.status === "succeeded"
                              ? "success"
                              : "neutral",
                      )}`}>
                        {formatStatusLabel(item.status)}
                      </span>
                    </div>
                    <div className="record-card__meta">
                      <span>Priority {item.priority}</span>
                      <span>{item.isBrowserTask ? "Browser task" : "System task"}</span>
                      {item.manualGateRequestId ? <span>Manual gate</span> : null}
                      <span>{formatRelativeTimestamp(item.createdAt)}</span>
                    </div>
                    <div className="automation-pill-list">
                      {item.platformId ? <span className="automation-pill">{item.platformId}</span> : null}
                      {item.personaId ? <span className="automation-pill">{item.personaId}</span> : null}
                      {item.contentReady ? <span className="automation-pill">Content ready</span> : null}
                    </div>
                    {item.contentPreview ? (
                      <InlineContentPreview
                        className="record-card__content"
                        value={item.contentPreview}
                        collapseAt={220}
                        expandable={false}
                        copyable={false}
                      />
                    ) : item.errorMessage ? (
                      <InlineContentPreview
                        className="record-card__content"
                        bodyClassName="record-card__content--muted"
                        value={item.errorMessage}
                        collapseAt={220}
                        expandable={false}
                        copyable={false}
                        muted
                      />
                    ) : item.finalUrl ? (
                      <InlineContentPreview
                        className="record-card__content"
                        bodyClassName="record-card__content--muted"
                        value={item.finalUrl}
                        collapseAt={200}
                        expandable={false}
                        copyable={false}
                        mono
                        muted
                      />
                    ) : null}
                  </article>
                )}
              />
            )}
          </Panel>

          <Panel
            title="Action Detail"
            subtitle="Selected task summary plus related runtime log excerpts"
            actions={
              <div className="toolbar-actions">
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => void actions.refreshSelectedActionLogs()}
                  disabled={!action.selectedTaskId}
                >
                  {action.selectedTaskLogsLoading ? "Refreshing..." : "Refresh task logs"}
                </button>
                <button
                  className="button button--secondary"
                  type="button"
                  onClick={() => action.selectedTaskId && actions.openTaskInRuntime(action.selectedTaskId)}
                  disabled={!action.selectedTaskId}
                >
                  Open in runtime logs
                </button>
              </div>
            }
          >
            {action.selectedTaskSnapshot ? (
              <div className="task-log-detail">
                <div
                  className={`banner banner--${
                    summary.selectedTask.tone === "danger"
                      ? "error"
                      : summary.selectedTask.tone === "success"
                        ? "info"
                        : summary.selectedTask.tone === "neutral"
                          ? "info"
                          : "warning"
                  }`}
                >
                  <strong>{summary.selectedTask.label}</strong>
                  <br />
                  {summary.selectedTask.detail}
                </div>

                <div className="details-grid details-grid--stacked">
                  <article className="details-grid__item">
                    <dt>Task</dt>
                    <dd>{action.selectedTaskSnapshot.title ?? action.selectedTaskSnapshot.kind}</dd>
                  </article>
                  <article className="details-grid__item">
                    <dt>Status</dt>
                    <dd>{formatStatusLabel(action.selectedTaskSnapshot.status)}</dd>
                  </article>
                  <article className="details-grid__item">
                    <dt>Persona / Platform</dt>
                    <dd>
                      {action.selectedTaskSnapshot.personaId ?? "N/A"} |{" "}
                      {action.selectedTaskSnapshot.platformId ?? "N/A"}
                    </dd>
                  </article>
                  <article className="details-grid__item">
                    <dt>Control flags</dt>
                    <dd>
                      {action.selectedTaskSnapshot.isBrowserTask ? "Browser task" : "System task"}
                      <br />
                      {action.selectedTaskSnapshot.manualGateRequestId
                        ? `Manual gate ${action.selectedTaskSnapshot.manualGateRequestId}`
                        : "No manual gate"}
                    </dd>
                  </article>
                  <article className="details-grid__item">
                    <dt>Content posture</dt>
                    <dd>
                      {action.selectedTaskSnapshot.contentKind ?? "No content kind"}
                      <br />
                      {action.selectedTaskSnapshot.contentReady ? "Content ready" : "Content not ready"}
                    </dd>
                  </article>
                  <article className="details-grid__item">
                    <dt>Trace linkage</dt>
                    <dd>{summary.selectedTraceLabel}</dd>
                  </article>
                  <article className="details-grid__item">
                    <dt>Final URL</dt>
                    <dd>
                      <InlineContentPreview
                        value={action.selectedTaskSnapshot.finalUrl}
                        empty="N/A"
                        collapseAt={180}
                        inlineLimit={6000}
                        mono
                      />
                    </dd>
                  </article>
                  <article className="details-grid__item">
                    <dt>Created</dt>
                    <dd>{formatRelativeTimestamp(action.selectedTaskSnapshot.createdAt)}</dd>
                  </article>
                </div>

                {action.selectedTaskSnapshot.errorMessage ? (
                  <div className="banner banner--error">
                    <InlineContentPreview
                      value={action.selectedTaskSnapshot.errorMessage}
                      collapseAt={240}
                      inlineLimit={6000}
                    />
                  </div>
                ) : null}

                {action.selectedTaskLogsError ? (
                  <div className="banner banner--error">
                    <InlineContentPreview
                      value={action.selectedTaskLogsError}
                      collapseAt={240}
                      inlineLimit={6000}
                    />
                  </div>
                ) : null}

                <div className="task-log-list">
                  {action.selectedTaskLogs.length === 0 ? (
                    <EmptyState
                      title="No related runtime logs"
                      detail="This task has no matching local runtime log rows yet, or the related work has not emitted them."
                    />
                  ) : (
                    action.selectedTaskLogs.map((item) => (
                      <article className="task-log-card" key={item.id}>
                        <div className="task-log-card__top">
                          <strong>{truncateInlineContent(item.message, 150)}</strong>
                          <span className={`badge badge--${getLogBadgeTone(item.level)}`}>
                            {item.level.toUpperCase()}
                          </span>
                        </div>
                        <InlineContentPreview
                          className="record-card__content"
                          value={item.message}
                          collapseAt={220}
                          expandable={false}
                          copyable={false}
                        />
                        <div className="record-card__footer">
                          <span>{item.runId ? `Run ${item.runId}` : "No run id"}</span>
                          <span>{formatRelativeTimestamp(item.createdAt)}</span>
                        </div>
                      </article>
                    ))
                  )}
                </div>
              </div>
            ) : (
              <EmptyState
                title="No action selected"
                detail="Choose a task on the left to inspect the action trail and related local runtime logs."
              />
            )}
          </Panel>
        </div>
      )}
    </div>
  );
}
