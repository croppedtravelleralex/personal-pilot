import type {
  ProxyDetailSnapshot,
  ProxyIpChangeFeedback,
  ProxyRowModel,
} from "../../features/proxies/model";
import { formatCount, formatRelativeTimestamp } from "../../utils/format";
import { EmptyState } from "../EmptyState";
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

function parseTimestamp(value: string | null): number | null {
  if (!value) {
    return null;
  }

  const numericValue = Number(value);
  if (Number.isFinite(numericValue) && numericValue > 0) {
    return numericValue;
  }

  const parsedMs = Date.parse(value);
  if (Number.isNaN(parsedMs)) {
    return null;
  }

  return Math.floor(parsedMs / 1000);
}

function getUsageBadge(status: string): string {
  switch (status) {
    case "running":
      return "badge badge--succeeded";
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
      detail: "Local change-IP request is in flight. Provider-side exit movement is still unknown.",
    };
  }

  if (!changeIpFeedback) {
    return {
      label: residencyStatus.replace(/_/g, " "),
      detail: `No fresh local rotation record. Current residency=${residencyStatus}, mode=${rotationMode}.`,
    };
  }

  if (changeIpFeedback.phase === "error") {
    return {
      label: "Local rotation failed",
      detail: "Request failed locally. Treat exit-IP state as unchanged until a later detail refresh proves otherwise.",
    };
  }

  if (changeIpFeedback.phase === "success") {
    return {
      label: "Local rotation succeeded",
      detail: `Tracked request completed as ${changeIpFeedback.rotationMode ?? rotationMode} (${changeIpFeedback.residencyStatus ?? residencyStatus}). Confirm actual exit-IP drift after detail refresh.`,
    };
  }

  return {
    label: "Rotation queued",
    detail: "Request has been queued locally and is waiting for a result.",
  };
}

function getCooldownLabel(changeIpFeedback: ProxyIpChangeFeedback | null): string {
  if (!changeIpFeedback?.updatedAt) {
    return "No cooldown";
  }

  const updatedAt = parseTimestamp(changeIpFeedback.updatedAt);
  if (!updatedAt) {
    return "Cooldown unknown";
  }

  const windowSeconds =
    changeIpFeedback.phase === "success"
      ? 5 * 60
      : changeIpFeedback.phase === "error"
        ? 15 * 60
        : 0;

  if (windowSeconds === 0) {
    return changeIpFeedback.phase === "running" ? "Rotation running" : "No cooldown";
  }

  const remainingSeconds = windowSeconds - (Math.floor(Date.now() / 1000) - updatedAt);
  if (remainingSeconds <= 0) {
    return "Cooldown cleared";
  }

  return `${Math.ceil(remainingSeconds / 60)}m local cooldown`;
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

          {error ? <div className="banner banner--error">{error}</div> : null}
          {changeIpFeedback ? (
            <div
              className={`banner${
                changeIpFeedback.phase === "error"
                  ? " banner--error"
                  : ""
              } usage-panel__banner`}
            >
              {changeIpFeedback.message}
            </div>
          ) : null}

          <div className="banner usage-panel__banner">
            Truth boundary: this panel tracks local change-IP requests and last known proxy detail. It
            does not claim the provider actually changed exit IP until a later detail refresh shows a
            different exit IP or region.
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
              <dt>Data source</dt>
              <dd>
                {detailSource ?? "list-only"}
                <br />
                {changeIpFeedback?.trackingTaskId ?? proxy.rotation.trackingTaskId ?? "no-tracking-task"}
              </dd>
            </div>
          </dl>

          <div className="usage-panel__notes">
            <strong>
              {isLoading
                ? "Loading detail..."
                : isChangingIp
                  ? "Submitting local change-IP request..."
                  : verificationStatus ?? "Operator detail ready"}
            </strong>
            <p>{rotationPosture.detail}</p>
            <p>{getRiskNote(proxy, detail)}</p>
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
