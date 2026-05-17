import { EmptyState } from "../components/EmptyState";
import { Panel } from "../components/Panel";
import { StatCard } from "../components/StatCard";
import { useRuntimeViewModel } from "../features/runtime/hooks";
import { STATUS_AUTO_REFRESH_INTERVAL_MS } from "../features/status/model";
import { useStatusViewModel } from "../features/status/hooks";
import { formatCount, formatRelativeTimestamp, formatStatusLabel } from "../utils/format";

function getBadgeTone(tone: "neutral" | "success" | "warning" | "danger") {
  if (tone === "danger") {
    return "error";
  }

  if (tone === "success") {
    return "succeeded";
  }

  return tone === "neutral" ? "info" : tone;
}

function getTaskBadgeTone(status: string) {
  if (["failed", "timed_out", "cancelled"].includes(status)) {
    return "error";
  }

  if (status === "succeeded") {
    return "succeeded";
  }

  if (["running", "queued", "pending"].includes(status)) {
    return "warning";
  }

  return "info";
}

function renderTaskList(
  items: Array<{
    id: string;
    title: string | null;
    kind: string;
    status: string;
    createdAt: string;
    finishedAt: string | null;
    personaId?: string | null;
    platformId?: string | null;
    isBrowserTask?: boolean;
    manualGateRequestId?: string | null;
    errorMessage?: string | null;
    finalUrl?: string | null;
  }>,
  emptyTitle: string,
  emptyDetail: string,
) {
  if (items.length === 0) {
    return <EmptyState title={emptyTitle} detail={emptyDetail} />;
  }

  return (
    <div className="record-list">
      {items.map((item) => (
        <article className="record-card record-card--compact" key={item.id}>
          <div className="record-card__top">
            <div>
              <strong>{item.title ?? item.kind}</strong>
              <p className="record-card__subline">
                {item.kind} / {item.id}
              </p>
            </div>
            <span className={`badge badge--${getTaskBadgeTone(item.status)}`}>
              {formatStatusLabel(item.status)}
            </span>
          </div>
          <div className="automation-pill-list">
            {item.platformId ? <span className="automation-pill">{item.platformId}</span> : null}
            {item.personaId ? <span className="automation-pill">{item.personaId}</span> : null}
            {item.isBrowserTask ? <span className="automation-pill">Browser task</span> : null}
            {item.manualGateRequestId ? <span className="automation-pill">Manual gate</span> : null}
          </div>
          {item.errorMessage ? (
            <p className="record-card__content record-card__content--muted">{item.errorMessage}</p>
          ) : item.finalUrl ? (
            <p className="record-card__content record-card__content--muted">{item.finalUrl}</p>
          ) : null}
          <div className="record-card__footer">
            <span>Created {formatRelativeTimestamp(item.createdAt)}</span>
            <span>Finished {formatRelativeTimestamp(item.finishedAt)}</span>
          </div>
        </article>
      ))}
    </div>
  );
}

export function OverviewPage() {
  const { state, summary: statusSummary, refresh } = useStatusViewModel();
  const runtime = useRuntimeViewModel();
  const snapshot = state.snapshot;
  const runtimeSnapshot = runtime.state.snapshot;
  const combinedAttentionItems = [
    ...runtime.summary.attentionItems,
    ...statusSummary.attentionItems,
  ].slice(0, 6);
  const topStatCards = [
    ...statusSummary.reviewBuckets.map((item) => ({
      label: item.label,
      value: item.value,
      hint: item.detail,
      tone: item.tone,
    })),
    ...runtime.summary.postureFacts.map((item) => ({
      label: item.label,
      value: item.value,
      hint: item.detail,
      tone: item.tone,
    })),
  ];

  return (
    <div className="page-stack">
      {state.error ? <div className="banner banner--error">{state.error}</div> : null}
      {runtime.state.error ? <div className="banner banner--error">{runtime.state.error}</div> : null}
      {runtime.state.info ? <div className="banner banner--info">{runtime.state.info}</div> : null}

      <div className="toolbar-card">
        <div className="automation-center__hero">
          <div className="automation-center__hero-copy">
            <span className="shell__eyebrow">Boss Console</span>
            <h2>Local Automation Control Tower</h2>
            <p>
              Pull queue posture, review debt, runtime control, and the latest local anomalies
              into one desktop surface without pretending there is any cloud control plane behind it.
            </p>
          </div>
          <div className="automation-center__hero-aside">
            <span className={`badge badge--${getBadgeTone(statusSummary.postureTone)}`}>
              Queue {statusSummary.postureLabel}
            </span>
            <span className={`badge badge--${getBadgeTone(runtime.summary.postureTone)}`}>
              Runtime {runtime.summary.postureLabel}
            </span>
            <span className="badge badge--info">
              Status auto {Math.round(STATUS_AUTO_REFRESH_INTERVAL_MS / 1000)}s
            </span>
            <span className="badge badge--info">{runtime.summary.cadenceLabel}</span>
            <button className="button button--secondary" type="button" onClick={() => void refresh()}>
              {state.isLoading ? "Refreshing..." : "Refresh status"}
            </button>
            <button
              className="button button--secondary"
              type="button"
              onClick={() => void runtime.actions.refresh()}
            >
              {runtime.state.isLoading ? "Refreshing runtime..." : "Refresh runtime"}
            </button>
            <button
              className="button button--secondary"
              type="button"
              onClick={() => void runtime.actions.stop()}
              disabled={!runtimeSnapshot?.running || runtime.state.activeAction === "stop"}
            >
              {runtime.state.activeAction === "stop" ? "Stopping..." : "Stop runtime"}
            </button>
            <button
              className="button"
              type="button"
              onClick={() => void runtime.actions.start()}
              disabled={Boolean(runtimeSnapshot?.running) || runtime.state.activeAction === "start"}
            >
              {runtime.state.activeAction === "start" ? "Starting..." : "Start runtime"}
            </button>
          </div>
        </div>

        <div className="automation-metric-strip">
          {statusSummary.metrics.slice(0, 4).map((metric) => (
            <article className="automation-metric-strip__item" key={metric.label}>
              <span className="automation-metric-strip__label">{metric.label}</span>
              <strong>{metric.value}</strong>
              <small>{metric.detail}</small>
            </article>
          ))}
          <article className="automation-metric-strip__item">
            <span className="automation-metric-strip__label">Runtime control</span>
            <strong>{runtime.summary.controlLabel}</strong>
            <small>{runtime.summary.controlDetail}</small>
          </article>
        </div>

        <div className="page-grid page-grid--two">
          <article className="contract-card">
            <div className="contract-card__top">
              <strong>{statusSummary.operationsHeadline}</strong>
              <span className={`badge badge--${getBadgeTone(statusSummary.postureTone)}`}>
                {statusSummary.queuePressureLabel}
              </span>
            </div>
            <p>{statusSummary.operationsDetail}</p>
          </article>
          <article className="contract-card">
            <div className="contract-card__top">
              <strong>{runtime.summary.postureLabel}</strong>
              <span className={`badge badge--${getBadgeTone(runtime.summary.postureTone)}`}>
                {runtime.summary.runtimeAgeLabel}
              </span>
            </div>
            <p>{runtime.summary.postureDetail}</p>
          </article>
        </div>

        {combinedAttentionItems.length > 0 ? (
          <div className="contract-list">
            {combinedAttentionItems.map((item) => (
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
            No active local attention items. This only reflects the current desktop snapshot window.
          </div>
        )}
      </div>

      <div className="stat-grid">
        {topStatCards.map((metric) => (
          <StatCard
            key={`${metric.label}-${metric.value}`}
            label={metric.label}
            value={metric.value}
            hint={metric.hint}
            tone={metric.tone}
          />
        ))}
      </div>

      <div className="page-grid page-grid--two">
        <Panel
          title="Exception Board"
          subtitle="The local review lane for failures, manual gates, and recent abnormal work"
          actions={
            <div className="toolbar-actions">
              <span className={`badge badge--${statusSummary.failureDebt > 0 ? "warning" : "succeeded"}`}>
                {statusSummary.failureDebt} failure debt
              </span>
              <span className={`badge badge--${statusSummary.manualGateCount > 0 ? "info" : "succeeded"}`}>
                {statusSummary.manualGateCount} manual gates
              </span>
            </div>
          }
        >
          {snapshot
            ? renderTaskList(
                statusSummary.reviewTasks,
                "No review queue",
                "Recent local task history does not currently show failures or manual-gate items.",
              )
            : (
              <EmptyState
                title="No attention board yet"
                detail="Refresh the dashboard to load the latest review items."
              />
            )}
        </Panel>

        <Panel
          title="Runtime Posture"
          subtitle="Who owns the local runtime, whether it is reachable, and how complete the path evidence is"
          actions={
            <div className="toolbar-actions">
              <span className={`badge badge--${getBadgeTone(runtime.summary.postureTone)}`}>
                {runtime.summary.postureLabel}
              </span>
              <span className="badge badge--info">{runtime.summary.pathCoverageLabel}</span>
            </div>
          }
        >
          <div className="details-grid details-grid--stacked">
            <article className="details-grid__item">
              <dt>Control</dt>
              <dd>
                {runtime.summary.controlLabel}
                <br />
                {runtime.summary.controlDetail}
              </dd>
            </article>
            <article className="details-grid__item">
              <dt>Ownership / PID</dt>
              <dd>
                {runtime.summary.ownershipLabel}
                <br />
                {runtime.summary.ownershipDetail}
              </dd>
            </article>
            <article className="details-grid__item">
              <dt>Health endpoint</dt>
              <dd>
                {runtime.summary.healthLabel}
                <br />
                {runtime.summary.healthDetail}
              </dd>
            </article>
            <article className="details-grid__item">
              <dt>Run age</dt>
              <dd>{runtime.summary.runtimeAgeLabel}</dd>
            </article>
            <article className="details-grid__item">
              <dt>Binary / log dir</dt>
              <dd>
                {runtimeSnapshot?.binaryPath ?? "No binary path"}
                <br />
                {runtimeSnapshot?.logDir ?? "No log directory"}
              </dd>
            </article>
            <article className="details-grid__item">
              <dt>Stdout / stderr</dt>
              <dd>
                {runtimeSnapshot?.stdoutPath ?? "No stdout path"}
                <br />
                {runtimeSnapshot?.stderrPath ?? "No stderr path"}
              </dd>
            </article>
          </div>
        </Panel>
      </div>

      <div className="page-grid page-grid--two">
        <Panel title="Queue and Worker Policy" subtitle="Operational summary for the local queue, retry policy, and sample freshness">
          {snapshot || runtimeSnapshot ? (
            <dl className="details-grid details-grid--stacked">
              <div className="details-grid__item">
                <dt>Queue pressure</dt>
                <dd>
                  {statusSummary.queuePressureLabel}
                  <br />
                  {statusSummary.queuePressureDetail}
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Success rate</dt>
                <dd>{statusSummary.successRateLabel}</dd>
              </div>
              <div className="details-grid__item">
                <dt>Worker heartbeat</dt>
                <dd>
                  {snapshot?.worker.heartbeatIntervalSeconds ?? "N/A"}s
                  <br />
                  reclaim {snapshot?.worker.reclaimAfterSeconds ?? "disabled"}
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Retry / backoff</dt>
                <dd>
                  retry {snapshot?.worker.claimRetryLimit ?? "N/A"}
                  <br />
                  {snapshot
                    ? `${snapshot.worker.idleBackoffMinMs}-${snapshot.worker.idleBackoffMaxMs}ms`
                    : "N/A"}
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Runner lane</dt>
                <dd>
                  {snapshot?.worker.runnerKind ?? "Unknown"}
                  <br />
                  {snapshot?.worker.workerCount ?? 0} workers
                </dd>
              </div>
              <div className="details-grid__item">
                <dt>Latest local update</dt>
                <dd>{snapshot ? formatRelativeTimestamp(snapshot.updatedAt) : "Waiting for status snapshot"}</dd>
              </div>
            </dl>
          ) : (
            <EmptyState
              title="No queue snapshot yet"
              detail="Refresh the dashboard to load the local queue and worker policy."
            />
          )}
        </Panel>

        <Panel title="Control Lanes" subtitle="How this desktop console is meant to be navigated by an operator">
          <div className="record-list">
            <article className="record-card record-card--compact">
              <div className="record-card__top">
                <strong>Dashboard lane</strong>
                <span className="badge badge--info">Posture</span>
              </div>
              <p className="record-card__content record-card__content--muted">
                Use this page for backlog pressure, manual-gate debt, local runtime control, and the latest task slices.
              </p>
            </article>
            <article className="record-card record-card--compact">
              <div className="record-card__top">
                <strong>Logs lane</strong>
                <span className="badge badge--warning">Trace</span>
              </div>
              <p className="record-card__content record-card__content--muted">
                Use Logs when you need runtime rows, task-linked traces, or action-by-action exception detail from the local database.
              </p>
            </article>
            <article className="record-card record-card--compact">
              <div className="record-card__top">
                <strong>Truth boundary</strong>
                <span className="badge badge--info">Local only</span>
              </div>
              <p className="record-card__content record-card__content--muted">
                Every card on this page comes from the local desktop status snapshot or local runtime status. There is no remote fleet, cloud sync, or hidden central scheduler implied here.
              </p>
            </article>
          </div>
        </Panel>
      </div>

      <div className="page-grid page-grid--two">
        <Panel title="Latest Tasks" subtitle="Most recent queue activity across the local desktop runtime">
          {snapshot
            ? renderTaskList(
                snapshot.latestTasks,
                "No recent tasks",
                "Recent task activity will appear once the desktop status feed returns items.",
              )
            : (
              <EmptyState
                title="No task feed loaded"
                detail="Refresh status to populate the latest task list."
              />
            )}
        </Panel>

        <Panel title="Browser Lane" subtitle="Recent browser-oriented execution samples in the local snapshot window">
          {snapshot
            ? renderTaskList(
                snapshot.latestBrowserTasks,
                "No browser tasks",
                "Browser task activity will appear here when the local status feed includes it.",
              )
            : (
              <EmptyState
                title="No browser task feed loaded"
                detail="Refresh status to populate browser task activity."
              />
            )}
        </Panel>
      </div>

      {snapshot ? (
        <Panel title="Task Totals" subtitle="High-level throughput counters from the local desktop status feed">
          <dl className="details-grid details-grid--two">
            <div className="details-grid__item">
              <dt>Total</dt>
              <dd>{formatCount(snapshot.counts.total)}</dd>
            </div>
            <div className="details-grid__item">
              <dt>Queued</dt>
              <dd>{formatCount(snapshot.counts.queued)}</dd>
            </div>
            <div className="details-grid__item">
              <dt>Succeeded</dt>
              <dd>{formatCount(snapshot.counts.succeeded)}</dd>
            </div>
            <div className="details-grid__item">
              <dt>Failed / timed out</dt>
              <dd>
                {formatCount(snapshot.counts.failed)}
                <br />
                {formatCount(snapshot.counts.timedOut)}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Cancelled</dt>
              <dd>{formatCount(snapshot.counts.cancelled)}</dd>
            </div>
            <div className="details-grid__item">
              <dt>Manual-review items</dt>
              <dd>{formatCount(statusSummary.manualGateCount)}</dd>
            </div>
          </dl>
        </Panel>
      ) : null}
    </div>
  );
}
