import {
  getProxyProviderRefreshStatusLabel,
  getProxyProviderRequestLabel,
  getProxyProviderSourceLabel,
  getProxyProviderStatusCodeLabel,
  getProxyProviderWriteEvidence,
  getProxyProviderWriteLabel,
  getProxyProviderWriteState,
} from "../../features/proxies/changeIpFeedback";
import type {
  ProxyIpChangeFeedback,
  ProxyWriteOutcomeLabel,
} from "../../features/proxies/model";
import { formatCount, formatRelativeTimestamp } from "../../utils/format";

interface ProxyChangeToolbarProps {
  phase: "idle" | "running" | "completed" | "blocked" | "error";
  feedbackTone: "neutral" | "success" | "warning" | "error";
  targetLabel: string;
  selectedCount: number;
  completedCount: number;
  succeededCount: number;
  failedCount: number;
  coolingDownCount: number;
  message: string;
  activeProxyName: string | null;
  lastStartedAt: string | null;
  lastFinishedAt: string | null;
  recentResults: ProxyIpChangeFeedback[];
  onStart: () => void;
  onDismiss: () => void;
}

function getPhaseBadge(phase: ProxyChangeToolbarProps["phase"]): string {
  switch (phase) {
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

function getPhaseLabel(phase: ProxyChangeToolbarProps["phase"]): string {
  switch (phase) {
    case "running":
      return "Submitting writes";
    case "completed":
      return "Write submission done";
    case "blocked":
      return "Blocked";
    case "error":
      return "Write failed";
    default:
      return "Ready";
  }
}

function getResultOutcomeLabel(result: ProxyIpChangeFeedback): ProxyWriteOutcomeLabel {
  const evidence = getProxyProviderWriteEvidence(result);
  const writeState = getProxyProviderWriteState(result);
  if (evidence.rollbackSignal) {
    return "rollback-flagged";
  }
  if (evidence.acceptedWrite === true || writeState === "accepted") {
    return "accepted";
  }
  if (writeState === "blocked") {
    return "blocked";
  }
  if (writeState === "failed" || result.phase === "error" || evidence.acceptedWrite === false) {
    return "write-failed";
  }

  return "write-pending";
}

function getAcceptedWriteLabel(acceptedWrite: boolean | null): string {
  if (acceptedWrite === true) {
    return "accepted";
  }
  if (acceptedWrite === false) {
    return "not-accepted";
  }
  return "unknown";
}

function getRollbackSignalLabel(rollbackSignal: boolean): string {
  return rollbackSignal ? "rollback-flagged" : "no-rollback-signal";
}

function getExecutionStatusLine(result: ProxyIpChangeFeedback): string {
  const evidence = getProxyProviderWriteEvidence(result);
  const refreshStatus = getProxyProviderRefreshStatusLabel(result);
  const statusCode = getProxyProviderStatusCodeLabel(result);
  const refreshAt = evidence.providerRefreshAt
    ? ` @ ${formatRelativeTimestamp(evidence.providerRefreshAt)}`
    : "";
  return `execution=${evidence.executionStatus ?? "status-unreported"} / providerRefresh=${refreshStatus} / statusCode=${statusCode}${refreshAt}`;
}

function getResultBadge(result: ProxyIpChangeFeedback): string {
  const outcome = getResultOutcomeLabel(result);
  switch (outcome) {
    case "accepted":
      return "badge badge--info";
    case "rollback-flagged":
      return "badge badge--warning";
    case "blocked":
    case "write-failed":
      return "badge badge--failed";
    default:
      return "badge badge--warning";
  }
}

export function ProxyChangeToolbar({
  phase,
  feedbackTone,
  targetLabel,
  selectedCount,
  completedCount,
  succeededCount: acceptedCount,
  failedCount,
  coolingDownCount,
  message,
  activeProxyName,
  lastStartedAt,
  lastFinishedAt,
  recentResults,
  onStart,
  onDismiss,
}: ProxyChangeToolbarProps) {
  const controlsDisabled = phase === "running";
  const actionLabel =
    selectedCount > 0 ? `Change ${selectedCount} selected IP` : "Change current IP";

  return (
    <section className="toolbar-card toolbar-card--subtle">
      <div className="batch-toolbar__header">
        <div>
          <span className="shell__eyebrow">Proxy IP Workbench</span>
          <h2 className="panel__title">Change-IP Queue</h2>
          <p className="panel__subtitle">
            Submit `changeProxyIp` writes for the pinned proxy or current selection and track
            provider-write acceptance, residency mode, rollback signals, and cooldown windows.
          </p>
        </div>
        <span className={getPhaseBadge(phase)}>{getPhaseLabel(phase)}</span>
      </div>

      <div className="batch-toolbar__grid">
        <div className="batch-toolbar__scope">
          <span className="field__label">Target scope</span>
          <div className="details-grid details-grid--two">
            <div className="details-grid__item">
              <dt>Current mode</dt>
              <dd>{targetLabel}</dd>
            </div>
            <div className="details-grid__item">
              <dt>Selected rows</dt>
              <dd>{formatCount(selectedCount)}</dd>
            </div>
          </div>
        </div>

        <div className="batch-toolbar__meta">
          <div className="batch-toolbar__metric">
            <strong>{formatCount(completedCount)}</strong>
            <span>Processed</span>
          </div>
          <div className="batch-toolbar__metric">
            <strong>{formatCount(acceptedCount)}</strong>
            <span>Accepted writes</span>
          </div>
          <div className="batch-toolbar__metric">
            <strong>{formatCount(failedCount)}</strong>
            <span>Write failed</span>
          </div>
          <div className="batch-toolbar__metric">
            <strong>{formatCount(coolingDownCount)}</strong>
            <span>Cooling down</span>
          </div>
        </div>

        <div className="batch-toolbar__actions">
          <button className="button" type="button" disabled={controlsDisabled} onClick={onStart}>
            {phase === "running" ? "Submitting..." : actionLabel}
          </button>
        </div>
      </div>

      {activeProxyName ? (
        <div className="banner">Active target: {activeProxyName}</div>
      ) : null}

      <div className="banner usage-panel__banner">
        Execution boundary: accepted feedback means desktop queued a provider-write task. Actual
        exit-IP drift remains pending until health/detail refresh observes new network output.
      </div>

      <div
        className={`batch-toolbar__feedback${
          feedbackTone === "neutral" ? "" : ` batch-toolbar__feedback--${feedbackTone}`
        }`}
        role="status"
      >
        <span>
          {message} Latest activity {formatRelativeTimestamp(lastFinishedAt ?? lastStartedAt)}.
        </span>
        {phase !== "idle" ? (
          <button className="button button--secondary" type="button" onClick={onDismiss}>
            Dismiss
          </button>
        ) : null}
      </div>

      {recentResults.length > 0 ? (
        <div className="record-list">
          {recentResults.map((result) => {
            const evidence = getProxyProviderWriteEvidence(result);

            return (
              <article className="record-card record-card--compact" key={result.proxyId}>
                <div className="record-card__top">
                  <div>
                    <strong>{result.proxyId}</strong>
                    <p className="record-card__subline">{result.message}</p>
                    <p className="record-card__subline">
                      accepted={getAcceptedWriteLabel(evidence.acceptedWrite)} / rollback=
                      {getRollbackSignalLabel(evidence.rollbackSignal)}
                    </p>
                    <p className="record-card__subline">
                      source={getProxyProviderSourceLabel(result, result.requestedProvider)} /
                      request={getProxyProviderRequestLabel(result)}
                    </p>
                    <p className="record-card__subline">{getExecutionStatusLine(result)}</p>
                  </div>
                  <span className={getResultBadge(result)}>{getResultOutcomeLabel(result)}</span>
                </div>
                <div className="record-card__footer">
                  <span>{formatRelativeTimestamp(result.updatedAt)}</span>
                  <span>{getProxyProviderWriteLabel(getProxyProviderWriteState(result))}</span>
                </div>
              </article>
            );
          })}
        </div>
      ) : null}
    </section>
  );
}
