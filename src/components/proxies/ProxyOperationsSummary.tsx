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
  ProxyDataSource,
  ProxyWriteOutcomeLabel,
} from "../../features/proxies/model";
import { formatCount, formatRelativeTimestamp } from "../../utils/format";
import { Panel } from "../Panel";

interface ProxyOperationsSummaryProps {
  dataSource: ProxyDataSource | null;
  summary: {
    total: number;
    loaded: number;
    visible: number;
    healthy: number;
    attention: number;
    used: number;
    activeUsage: number;
    ready: number;
    highRisk: number;
    providers: number;
    sources: number;
    localRotationTracked: number;
    localRotationSuccess: number;
    localRotationFailures: number;
    localRotationRunning: number;
    providerWriteAccepted: number;
    rollbackSignals: number;
    stickyActive: number;
    stickyExpired: number;
    stickyMode: number;
    providerAwareMode: number;
    coolingDown: number;
  };
  recentResults: ProxyIpChangeFeedback[];
}

function getResultBadge(result: ProxyIpChangeFeedback): string {
  const writeState = getProxyProviderWriteState(result);
  switch (writeState) {
    case "accepted":
      return "badge badge--info";
    case "rollback_flagged":
    case "blocked":
    case "failed":
      return "badge badge--failed";
    default:
      return "badge badge--warning";
  }
}

function getResultLabel(result: ProxyIpChangeFeedback): string {
  const evidence = getProxyProviderWriteEvidence(result);
  const writeState = getProxyProviderWriteState(result);
  if (evidence.rollbackSignal || writeState === "rollback_flagged") {
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

function getResultHumanLabel(result: ProxyIpChangeFeedback): string {
  const token = getResultLabel(result) as ProxyWriteOutcomeLabel;
  if (token === "write-pending") {
    return getProxyProviderWriteLabel(getProxyProviderWriteState(result));
  }
  return token;
}

function getAcceptedSignalLabel(acceptedWrite: boolean | null): string {
  if (acceptedWrite === true) {
    return "accepted";
  }
  if (acceptedWrite === false) {
    return "not-accepted";
  }
  return "unknown";
}

export function ProxyOperationsSummary({
  dataSource,
  summary,
  recentResults,
}: ProxyOperationsSummaryProps) {
  return (
    <Panel
      title="Proxy Operations Overview"
      subtitle="Dense operator summary for inventory posture, residency/rotation semantics, and immediate risk follow-up."
      actions={<span className="badge">{dataSource ?? "data-unreported"}</span>}
    >
      <div className="details-grid details-grid--two">
        <div className="details-grid__item">
          <dt>Inventory posture</dt>
          <dd>
            {formatCount(summary.visible)} visible / {formatCount(summary.total)} total
            <br />
            {formatCount(summary.ready)} ready supply, {formatCount(summary.used)} assigned
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Provider mix</dt>
          <dd>
            {formatCount(summary.providers)} providers
            <br />
            {formatCount(summary.sources)} sources feeding the workbench
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Risk watch</dt>
          <dd>
            {formatCount(summary.highRisk)} high-risk rows
            <br />
            {formatCount(summary.attention)} need review, {formatCount(summary.healthy)} healthy
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Change-IP watch</dt>
          <dd>
            {formatCount(summary.localRotationTracked)} tracked results
            <br />
            {formatCount(summary.providerWriteAccepted)} accepted writes, {formatCount(summary.rollbackSignals)} rollback-flagged, {formatCount(summary.localRotationRunning)} running
          </dd>
        </div>
      </div>

      <div className="banner usage-panel__banner">
        Change-IP posture here reflects desktop write feedback, rollback signal parsing, residency
        posture, and local cooldown heuristics. Exit-IP change is only considered observed after a
        later detail refresh shows new network output.
      </div>

      <div className="details-grid details-grid--two">
        <div className="details-grid__item">
          <dt>Residency posture</dt>
          <dd>
            {formatCount(summary.stickyActive)} sticky active, {formatCount(summary.stickyExpired)} sticky expired
            <br />
            {formatCount(summary.stickyMode)} sticky mode, {formatCount(summary.providerAwareMode)} provider-aware mode
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Cooling-down rows</dt>
          <dd>
            {formatCount(summary.coolingDown)}
            <br />
            Local observation window after recent rotate attempts
          </dd>
        </div>
        <div className="details-grid__item">
          <dt>Active usage links</dt>
          <dd>
            {formatCount(summary.activeUsage)}
            <br />
            Running profile attachments currently leaning on this inventory
          </dd>
        </div>
      </div>

      {recentResults.length > 0 ? (
        <div className="record-list">
          {recentResults.map((result) => {
            const evidence = getProxyProviderWriteEvidence(result);
            const refreshStatus = getProxyProviderRefreshStatusLabel(result);
            const statusCode = getProxyProviderStatusCodeLabel(result);

            return (
              <article className="record-card record-card--compact" key={result.proxyId}>
                <div className="record-card__top">
                  <div>
                    <strong>{result.proxyId}</strong>
                    <p className="record-card__subline">{result.message}</p>
                    <p className="record-card__subline">
                      accepted={getAcceptedSignalLabel(evidence.acceptedWrite)} / rollback=
                      {evidence.rollbackSignal ? "rollback-flagged" : "no-rollback-signal"}
                    </p>
                    <p className="record-card__subline">
                      source={getProxyProviderSourceLabel(result, result.requestedProvider)} /
                      request={getProxyProviderRequestLabel(result)}
                    </p>
                    <p className="record-card__subline">
                      execution={evidence.executionStatus ?? "status-unreported"} /
                      providerRefresh={refreshStatus} / statusCode={statusCode}
                    </p>
                  </div>
                  <span className={getResultBadge(result)}>{getResultHumanLabel(result)}</span>
                </div>
                <div className="record-card__footer">
                  <span>{formatRelativeTimestamp(result.updatedAt)}</span>
                  <span>{getResultLabel(result)}</span>
                </div>
              </article>
            );
          })}
        </div>
      ) : null}
    </Panel>
  );
}
