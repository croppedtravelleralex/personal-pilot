import type { ProxyIpChangeFeedback, ProxyDataSource } from "../../features/proxies/model";
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
    stickyActive: number;
    stickyExpired: number;
    stickyMode: number;
    providerAwareMode: number;
    coolingDown: number;
  };
  recentResults: ProxyIpChangeFeedback[];
}

function getResultBadge(result: ProxyIpChangeFeedback): string {
  switch (result.phase) {
    case "success":
      return "badge badge--succeeded";
    case "error":
      return "badge badge--failed";
    default:
      return "badge badge--warning";
  }
}

function getResultLabel(result: ProxyIpChangeFeedback): string {
  switch (result.phase) {
    case "success":
      return result.status ?? "Local success";
    case "error":
      return result.status ?? "Needs review";
    default:
      return "Running";
  }
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
      actions={<span className="badge">{dataSource ?? "unknown-source"}</span>}
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
            {formatCount(summary.localRotationSuccess)} success, {formatCount(summary.localRotationFailures)} failed, {formatCount(summary.localRotationRunning)} running
          </dd>
        </div>
      </div>

      <div className="banner usage-panel__banner">
        Change-IP posture here reflects local tracked requests, cooldown heuristics, and latest feedback.
        It does not claim the provider actually switched exit IP until a later detail refresh exposes a new exit IP.
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
          {recentResults.map((result) => (
            <article className="record-card record-card--compact" key={result.proxyId}>
              <div className="record-card__top">
                <div>
                  <strong>{result.proxyId}</strong>
                  <p className="record-card__subline">{result.message}</p>
                </div>
                <span className={getResultBadge(result)}>{getResultLabel(result)}</span>
              </div>
              <div className="record-card__footer">
                <span>{formatRelativeTimestamp(result.updatedAt)}</span>
                <span>{result.phase}</span>
              </div>
            </article>
          ))}
        </div>
      ) : null}
    </Panel>
  );
}
