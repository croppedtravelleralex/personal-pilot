import {
  getProxyChangeCooldownRemainingSeconds,
  getProxyProviderRefreshStatusLabel,
  getProxyProviderRequestLabel,
  getProxyProviderSourceLabel,
  getProxyProviderStatusCodeLabel,
  getProxyProviderWriteEvidence,
  getProxyProviderWriteDetail,
  getProxyProviderWriteLabel,
  getProxyProviderWriteState,
} from "../../features/proxies/changeIpFeedback";
import type {
  ProxyDetailSnapshot,
  ProxyIpChangeFeedback,
  ProxyRowModel,
} from "../../features/proxies/model";
import { formatCount, formatRelativeTimestamp } from "../../utils/format";
import { EmptyState } from "../EmptyState";
import { InlineContentPreview } from "../InlineContentPreview";
import { Panel } from "../Panel";

interface UsagePanelProps {
  proxy: ProxyRowModel | null;
  detail: ProxyDetailSnapshot | null;
  detailSource: string | null;
  isLoading: boolean;
  error: string | null;
  hiddenByFilters: boolean;
  changeIpFeedback: ProxyIpChangeFeedback | null;
  isChangingIp: boolean;
  changeIpActionLabel: string;
  onChangeIp: () => void;
  onRetry: () => void;
}

function getUsageBadge(status: string): string {
  switch (status) {
    case "running":
      return "badge badge--info";
    case "ready":
      return "badge badge--warning";
    default:
      return "badge";
  }
}

function getRiskNote(proxy: ProxyRowModel, detail: ProxyDetailSnapshot | null): string {
  const effectiveHealth = detail?.health ?? proxy.health;

  if (effectiveHealth.state === "failed") {
    return effectiveHealth.failureReason ?? "Latest known check failed and needs operator follow-up.";
  }

  if (effectiveHealth.state === "warning") {
    return effectiveHealth.failureReason ?? "Verification surfaced warnings; review geo/latency posture.";
  }

  if ((effectiveHealth.latencyMs ?? 0) >= 800) {
    return `Latency is elevated at ${effectiveHealth.latencyMs}ms. Consider a fresh verify batch before reuse.`;
  }

  if (proxy.activeUsageCount > 0) {
    return `${formatCount(proxy.activeUsageCount)} active sessions are attached, so rotate carefully.`;
  }

  return "No immediate risk signal beyond the current local health view.";
}

function getRotationPosture(
  proxy: ProxyRowModel,
  changeIpFeedback: ProxyIpChangeFeedback | null,
  isChangingIp: boolean,
): { label: string; detail: string } {
  const residencyStatus = changeIpFeedback?.residencyStatus ?? proxy.rotation.residencyStatus;
  const rotationMode = changeIpFeedback?.rotationMode ?? proxy.rotation.rotationMode;

  if (isChangingIp) {
    return {
      label: "Rotation running",
      detail: "Submitting provider-side write task. Exit-IP movement is still unknown.",
    };
  }

  if (!changeIpFeedback) {
    return {
      label: residencyStatus.replace(/_/g, " "),
      detail: `No fresh local rotation record. Current residency=${residencyStatus}, mode=${rotationMode}.`,
    };
  }

  const writeState = getProxyProviderWriteState(changeIpFeedback);
  const writeLabel = getProxyProviderWriteLabel(writeState);
  const writeDetail = getProxyProviderWriteDetail(changeIpFeedback);
  const sourceLabel = getProxyProviderSourceLabel(
    changeIpFeedback,
    changeIpFeedback?.requestedProvider ?? proxy.rotation.requestedProvider ?? null,
  );
  const requestId = getProxyProviderRequestLabel(
    changeIpFeedback,
    proxy.rotation.trackingTaskId,
  );

  if (
    changeIpFeedback.phase === "error" ||
    writeState === "failed" ||
    writeState === "blocked"
  ) {
    return {
      label: writeState === "blocked" ? "blocked" : "write-failed",
      detail: `${writeDetail} source=${sourceLabel}, request=${requestId}. Treat exit-IP state as unchanged until a later detail refresh proves otherwise.`,
    };
  }

  if (writeState === "rollback_flagged") {
    return {
      label: "rollback-flagged",
      detail: `${writeDetail} source=${sourceLabel}, request=${requestId}. Treat write as unstable and verify residency/exit-IP on next detail refresh.`,
    };
  }

  if (
    writeState === "accepted" ||
    changeIpFeedback.phase === "accepted" ||
    changeIpFeedback.phase === "success"
  ) {
    return {
      label: "accepted",
      detail: `${writeDetail} source=${sourceLabel}, request=${requestId}. ${changeIpFeedback.rotationMode ?? rotationMode} (${changeIpFeedback.residencyStatus ?? residencyStatus}). Confirm exit-IP drift after detail refresh.`,
    };
  }

  return {
    label: "write-pending",
    detail: `${writeLabel} pending. Request is queued locally and waiting for provider-write feedback.`,
  };
}

function getCooldownLabel(changeIpFeedback: ProxyIpChangeFeedback | null): string {
  if (!changeIpFeedback) {
    return "No cooldown";
  }

  if (changeIpFeedback.phase === "running") {
    return "Rotation running";
  }

  const remainingSeconds = getProxyChangeCooldownRemainingSeconds(changeIpFeedback);
  if (remainingSeconds === null) {
    return "Cooldown unknown";
  }

  if (remainingSeconds <= 0) {
    return "Cooldown cleared";
  }

  return `${Math.ceil(remainingSeconds / 60)}m local cooldown`;
}

function getAcceptedSignalLabel(changeIpFeedback: ProxyIpChangeFeedback | null): string {
  const acceptedWrite = getProxyProviderWriteEvidence(changeIpFeedback).acceptedWrite;
  if (acceptedWrite === true) {
    return "accepted";
  }
  if (acceptedWrite === false) {
    return "not-accepted";
  }
  return "unknown";
}

export function UsagePanel({
  proxy,
  detail,
  detailSource,
  isLoading,
  error,
  hiddenByFilters,
  changeIpFeedback,
  isChangingIp,
  changeIpActionLabel,
  onChangeIp,
  onRetry,
}: UsagePanelProps) {
  const effectiveHealth = detail?.health ?? proxy?.health ?? null;
  const writeEvidence = getProxyProviderWriteEvidence(changeIpFeedback);
  const rollbackLabel = writeEvidence.rollbackSignal ? "rollback-flagged" : "no-rollback-signal";
  const sourceLabel = getProxyProviderSourceLabel(
    changeIpFeedback,
    changeIpFeedback?.requestedProvider ?? proxy?.rotation.requestedProvider ?? null,
  );
  const requestIdLabel = getProxyProviderRequestLabel(
    changeIpFeedback,
    proxy?.rotation.trackingTaskId ?? null,
  );
  const executionStatusLabel = writeEvidence.executionStatus ?? "status-unreported";
  const rollbackStatusLabel = writeEvidence.rollbackStatus ?? rollbackLabel;
  const providerRefreshLabel = getProxyProviderRefreshStatusLabel(changeIpFeedback);
  const providerStatusCodeLabel = getProxyProviderStatusCodeLabel(changeIpFeedback);
  const providerRefreshAt = writeEvidence.providerRefreshAt;
  const verificationStatus =
    effectiveHealth?.batchState === "queued"
      ? "Verification queued"
      : effectiveHealth?.batchState === "running"
        ? "Verification running"
        : null;
  const rotationPosture = proxy
    ? getRotationPosture(proxy, changeIpFeedback, isChangingIp)
    : { label: "No proxy", detail: "Select a row to inspect change-IP posture." };

  return (
    <Panel
      title="Proxy Detail"
      subtitle="Pinned operator detail for health, usage mapping, local rotation feedback, and realism boundaries."
      actions={
        proxy ? (
          <div className="proxy-table__actions">
            <button className="button button--secondary" type="button" onClick={onRetry}>
              Refresh detail
            </button>
            <button className="button" type="button" disabled={isChangingIp} onClick={onChangeIp}>
              {isChangingIp ? "Changing..." : changeIpActionLabel}
            </button>
          </div>
        ) : null
      }
    >
      {!proxy ? (
        <EmptyState
          title="No proxy selected"
          detail="Pick a row from the inventory table to inspect usage mapping and health detail."
        />
      ) : (
        <div className="usage-panel">
          {hiddenByFilters ? (
            <div className="banner usage-panel__banner">
              The selected proxy is outside the current filter scope, but its detail view stays pinned.
            </div>
          ) : null}

          {error ? (
            <div className="banner banner--error">
              <InlineContentPreview value={error} collapseAt={240} inlineLimit={4000} />
            </div>
          ) : null}
          {changeIpFeedback ? (
            <div
              className={`banner${
                changeIpFeedback.phase === "error"
                  ? " banner--error"
                  : ""
              } usage-panel__banner`}
            >
              <InlineContentPreview
                value={changeIpFeedback.message}
                collapseAt={240}
                inlineLimit={6000}
              />
            </div>
          ) : null}

          <div className="banner usage-panel__banner">
            Truth boundary: this panel tracks desktop write feedback plus last known proxy detail. It
            does not claim provider exit-IP drift until a later detail refresh shows changed network
            output.
          </div>

          <div className="usage-panel__hero">
            <div>
              <span className="shell__eyebrow">Selected Proxy</span>
              <h3 className="usage-panel__title">{proxy.name}</h3>
              <p className="panel__subtitle">
                {proxy.protocol.toUpperCase()} / {proxy.endpoint}:{proxy.port}
              </p>
            </div>
            <div className="usage-panel__badges">
              <span className="badge">{effectiveHealth?.summary ?? proxy.health.summary}</span>
              <span className="badge badge--info">{rotationPosture.label}</span>
              {verificationStatus ? <span className="badge badge--warning">{verificationStatus}</span> : null}
            </div>
          </div>

          <dl className="details-grid details-grid--two">
            <div className="details-grid__item">
              <dt>Provider / source</dt>
              <dd>
                {proxy.providerLabel}
                <br />
                {proxy.sourceLabel}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Exit IP / region</dt>
              <dd>
                {detail?.health.exitIp ?? proxy.exitIp ?? "Pending detail"}
                <br />
                {detail?.health.regionLabel ?? proxy.regionLabel ?? "Waiting for region"}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Auth mode</dt>
              <dd>{proxy.authLabel}</dd>
            </div>
            <div className="details-grid__item">
              <dt>Health status</dt>
              <dd>
                {effectiveHealth?.state ?? proxy.health.state}
                <br />
                Last check {formatRelativeTimestamp(detail?.health.lastCheckAt ?? proxy.health.lastCheckAt)}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Usage posture</dt>
              <dd>
                {formatCount(detail?.usageLinks.length ?? proxy.usageCount)} assigned
                <br />
                {formatCount(proxy.activeUsageCount)} active links
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Residency / rotation mode</dt>
              <dd>
                {changeIpFeedback?.residencyStatus ?? proxy.rotation.residencyStatus}
                <br />
                {changeIpFeedback?.rotationMode ?? proxy.rotation.rotationMode}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Requested provider / region</dt>
              <dd>
                {changeIpFeedback?.requestedProvider ?? proxy.rotation.requestedProvider ?? "inherit-provider"}
                <br />
                {changeIpFeedback?.requestedRegion ?? proxy.rotation.requestedRegion ?? "inherit-region"}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Sticky session / expires</dt>
              <dd>
                {changeIpFeedback?.sessionKey ?? proxy.rotation.sessionKey ?? "none"}
                <br />
                {formatRelativeTimestamp(changeIpFeedback?.expiresAt ?? proxy.rotation.expiresAt ?? null)}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Cooldown / latest local result</dt>
              <dd>
                {getCooldownLabel(changeIpFeedback)}
                <br />
                {formatRelativeTimestamp(changeIpFeedback?.updatedAt ?? null)}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Provider write / request</dt>
              <dd>
                {changeIpFeedback
                  ? `${getProxyProviderWriteLabel(getProxyProviderWriteState(changeIpFeedback))} (${detailSource ?? "list-only"})`
                  : `No recent write (${detailSource ?? "list-only"})`}
                <br />
                accepted={getAcceptedSignalLabel(changeIpFeedback)} / rollback={rollbackLabel} /
                source={sourceLabel} / request={requestIdLabel}
              </dd>
            </div>
            <div className="details-grid__item">
              <dt>Execution / rollback / refresh</dt>
              <dd>
                execution={executionStatusLabel} / rollback={rollbackStatusLabel}
                <br />
                providerRefresh={providerRefreshLabel} / statusCode={providerStatusCodeLabel}
                {providerRefreshAt
                  ? ` @ ${formatRelativeTimestamp(providerRefreshAt)}`
                  : ""}
              </dd>
            </div>
          </dl>

          <div className="usage-panel__notes">
            <strong>
              {isLoading
                ? "Loading detail..."
                : isChangingIp
                  ? "Submitting provider-write request..."
                  : verificationStatus ?? "Operator detail ready"}
            </strong>
            <InlineContentPreview
              className="record-card__content"
              value={rotationPosture.detail}
              collapseAt={220}
              inlineLimit={8000}
            />
            <InlineContentPreview
              className="record-card__content"
              value={getRiskNote(proxy, detail)}
              collapseAt={220}
              inlineLimit={8000}
            />
          </div>

          <div className="proxy-row__tags">
            {proxy.tags.map((tag) => (
              <span className="filter-chip filter-chip--active" key={tag}>
                {tag}
              </span>
            ))}
          </div>

          {(detail?.usageLinks ?? proxy.usageLinks).length === 0 ? (
            <EmptyState
              title="No profile usage"
              detail="This proxy is currently unassigned, which makes it a good candidate for future allocation."
            />
          ) : (
            <div className="record-list">
              {(detail?.usageLinks ?? proxy.usageLinks).map((usage) => (
                <article className="record-card record-card--compact" key={usage.id}>
                  <div className="record-card__top">
                    <div>
                      <strong>{usage.profileName}</strong>
                      <p className="record-card__subline">
                        {usage.groupName} / {usage.profileId}
                      </p>
                    </div>
                    <span className={getUsageBadge(usage.profileStatus)}>
                      {usage.profileStatus}
                    </span>
                  </div>
                  <div className="record-card__footer">
                    <span>Assigned {formatRelativeTimestamp(usage.assignedAt)}</span>
                    <span>Usage mapping detail</span>
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>
      )}
    </Panel>
  );
}
