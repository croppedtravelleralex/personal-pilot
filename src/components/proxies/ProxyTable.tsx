import {
  getProxyChangeCooldownRemainingSeconds,
  getProxyProviderWriteDetail,
  getProxyProviderWriteLabel,
  getProxyProviderWriteState,
} from "../../features/proxies/changeIpFeedback";
import type {
  ProxyIpChangeFeedback,
  ProxyRowModel,
  ProxySortField,
} from "../../features/proxies/model";
import { formatCount, formatRelativeTimestamp } from "../../utils/format";
import { EmptyState } from "../EmptyState";
import { Panel } from "../Panel";
import { VirtualList } from "../VirtualList";

interface FilterOption {
  value: string;
  label: string;
}

interface ProxyTableProps {
  rows: ProxyRowModel[];
  totalCount: number;
  selectedProxyId: string | null;
  selectedIds: string[];
  changeIpResults: Record<string, ProxyIpChangeFeedback>;
  allVisibleSelected: boolean;
  sortField: ProxySortField;
  sortOptions: FilterOption[];
  onSortFieldChange: (value: ProxySortField) => void;
  onOpenRow: (proxyId: string) => void;
  onToggleSelection: (proxyId: string) => void;
  onSetVisibleSelection: (proxyIds: string[]) => void;
}

function getDisplayHealthState(health: ProxyRowModel["health"]): ProxyRowModel["health"]["state"] {
  if (health.batchState === "queued") {
    return "queued";
  }

  if (health.batchState === "running") {
    return "checking";
  }

  return health.state;
}

function getHealthBadge(state: ProxyRowModel["health"]["state"]): string {
  switch (state) {
    case "healthy":
      return "badge badge--succeeded";
    case "warning":
    case "queued":
    case "checking":
      return "badge badge--warning";
    case "failed":
      return "badge badge--failed";
    default:
      return "badge";
  }
}

function getHealthLabel(state: ProxyRowModel["health"]["state"]): string {
  switch (state) {
    case "healthy":
      return "Healthy";
    case "warning":
      return "Warning";
    case "failed":
      return "Failed";
    case "queued":
      return "Queued";
    case "checking":
      return "Checking";
    default:
      return "Unknown";
  }
}

function getCooldownRemainingLabel(changeIpFeedback: ProxyIpChangeFeedback | null): string {
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

  return `Cooldown ${Math.ceil(remainingSeconds / 60)}m`;
}

function getRiskPosture(row: ProxyRowModel): { badge: string; label: string; detail: string } {
  if (row.health.state === "failed") {
    return {
      badge: "badge badge--failed",
      label: "Critical risk",
      detail: row.health.failureReason ?? "Connectivity failed in the latest known check.",
    };
  }

  if (row.health.state === "warning" || (row.health.latencyMs ?? 0) >= 800) {
    return {
      badge: "badge badge--warning",
      label: "Elevated risk",
      detail:
        row.health.failureReason ??
        `Latency ${row.health.latencyMs ?? "n/a"}ms or geo/grade drift needs review.`,
    };
  }

  if (row.activeUsageCount > 0) {
    return {
      badge: "badge badge--info",
      label: "Live traffic",
      detail: `${formatCount(row.activeUsageCount)} active profile links currently attached.`,
    };
  }

  if (row.usageCount > 0) {
    return {
      badge: "badge",
      label: "Assigned",
      detail: `${formatCount(row.usageCount)} stored profile mappings with no active traffic.`,
    };
  }

  return {
    badge: "badge badge--succeeded",
    label: "Ready pool",
    detail: "Healthy and currently unassigned, ready for the next allocation.",
  };
}

function getRotationPosture(
  row: ProxyRowModel,
  changeIpFeedback: ProxyIpChangeFeedback | null,
): { badge: string; label: string; detail: string } {
  const residencyStatus = changeIpFeedback?.residencyStatus ?? row.rotation.residencyStatus;
  const rotationMode = changeIpFeedback?.rotationMode ?? row.rotation.rotationMode;

  if (!changeIpFeedback) {
    if (residencyStatus === "sticky_active") {
      return {
        badge: "badge badge--info",
        label: "Sticky active",
        detail: `Session ${row.rotation.sessionKey ?? "unknown"} is sticky-bound (${rotationMode}).`,
      };
    }
    if (residencyStatus === "sticky_expired") {
      return {
        badge: "badge badge--warning",
        label: "Sticky expired",
        detail: `Session ${row.rotation.sessionKey ?? "unknown"} expired and needs sticky rebind.`,
      };
    }
    return {
      badge: "badge",
      label: "No recent run",
      detail:
        row.activeUsageCount > 0
          ? `Live usage attached; current residency is ${residencyStatus}.`
          : `No recent local rotate attempt tracked. Current mode ${rotationMode}.`,
    };
  }

  if (changeIpFeedback.phase === "running") {
    return {
      badge: "badge badge--warning",
      label: "Rotation running",
      detail: "Submitting provider-side write task. Exit-IP outcome is not known yet.",
    };
  }

  const writeState = getProxyProviderWriteState(changeIpFeedback);
  const writeLabel = getProxyProviderWriteLabel(writeState);
  const writeDetail = getProxyProviderWriteDetail(changeIpFeedback);

  if (
    changeIpFeedback.phase === "error" ||
    writeState === "failed" ||
    writeState === "blocked"
  ) {
    return {
      badge: "badge badge--failed",
      label: writeLabel,
      detail: `${writeDetail} Status=${changeIpFeedback.status ?? "unknown"}.`,
    };
  }

  if (writeState === "rollback_flagged") {
    return {
      badge: "badge badge--warning",
      label: writeLabel,
      detail: `${writeDetail} Verify residency=${changeIpFeedback.residencyStatus ?? residencyStatus}.`,
    };
  }

  if (writeState === "accepted") {
    return {
      badge: "badge badge--info",
      label: writeLabel,
      detail: `${writeDetail} Exit-IP drift is not observed yet.`,
    };
  }

  return {
    badge: "badge badge--warning",
    label: writeLabel,
    detail: `${writeDetail} ${changeIpFeedback.rotationMode ?? rotationMode} / ${changeIpFeedback.residencyStatus ?? residencyStatus}.`,
  };
}

function ProxyRow({
  row,
  changeIpFeedback,
  isActive,
  isChecked,
  onOpen,
  onToggleSelection,
}: {
  row: ProxyRowModel;
  changeIpFeedback: ProxyIpChangeFeedback | null;
  isActive: boolean;
  isChecked: boolean;
  onOpen: () => void;
  onToggleSelection: () => void;
}) {
  const displayHealthState = getDisplayHealthState(row.health);
  const riskPosture = getRiskPosture(row);
  const rotationPosture = getRotationPosture(row, changeIpFeedback);

  return (
    <article
      className={`proxy-row ${isActive ? "proxy-row--active" : ""}`}
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onOpen();
        }
      }}
      role="button"
      tabIndex={0}
    >
      <div className="proxy-table__cell proxy-table__cell--selection">
        <input
          type="checkbox"
          checked={isChecked}
          onChange={() => onToggleSelection()}
          onClick={(event) => event.stopPropagation()}
          aria-label={`Select ${row.name}`}
        />
      </div>

      <div className="proxy-table__cell proxy-table__cell--identity">
        <strong>{row.name}</strong>
        <span className="proxy-row__subline">
          {row.protocol.toUpperCase()} lane / {row.endpoint}:{row.port}
        </span>
        <span className="proxy-row__subline">{row.note}</span>
      </div>

      <div className="proxy-table__cell proxy-table__cell--health">
        <span className={getHealthBadge(displayHealthState)}>
          {getHealthLabel(displayHealthState)}
        </span>
        <strong>{row.health.summary}</strong>
        <span className="proxy-row__subline">
          {displayHealthState === "queued"
            ? "Verification queued in native batch"
            : displayHealthState === "checking"
              ? "Verification is currently running"
              : `Last check ${formatRelativeTimestamp(row.health.lastCheckAt)}`}
        </span>
        <span className="proxy-row__subline">
          {row.health.latencyMs !== null ? `Latency ${row.health.latencyMs}ms` : "Latency pending"}
        </span>
      </div>

      <div className="proxy-table__cell">
        <strong>{row.exitIp ?? "Pending"}</strong>
        <span className="proxy-row__subline">{row.regionLabel ?? "Waiting for region"}</span>
        <span className="proxy-row__subline">
          {row.health.exitIp ? "Detail-confirmed exit IP" : "List-level exit view"}
        </span>
      </div>

      <div className="proxy-table__cell">
        <strong>{row.providerLabel}</strong>
        <span className="proxy-row__subline">
          {row.sourceLabel} / {row.authLabel}
        </span>
        <div className="proxy-row__tags">
          {row.tags.map((tag) => (
            <span className="filter-chip" key={tag}>
              {tag}
            </span>
          ))}
        </div>
      </div>

      <div className="proxy-table__cell">
        <span className={riskPosture.badge}>{riskPosture.label}</span>
        <span className={rotationPosture.badge}>{rotationPosture.label}</span>
        <strong>{getCooldownRemainingLabel(changeIpFeedback)}</strong>
        <span className="proxy-row__subline">{riskPosture.detail}</span>
        <span className="proxy-row__subline">{rotationPosture.detail}</span>
        <span className="proxy-row__subline">
          {changeIpFeedback?.requestedProvider ?? row.rotation.requestedProvider ?? "inherit-provider"} /{" "}
          {changeIpFeedback?.requestedRegion ?? row.rotation.requestedRegion ?? "inherit-region"}
        </span>
        <span className="proxy-row__subline">
          {changeIpFeedback?.trackingTaskId
            ? `Tracking ${changeIpFeedback.trackingTaskId}`
            : "Tracking pending"}
        </span>
      </div>

      <div className="proxy-table__cell">
        <strong>{formatCount(row.usageCount)}</strong>
        <span className="proxy-row__subline">
          {formatCount(row.activeUsageCount)} active profiles
        </span>
        <span className="proxy-row__subline">
          {row.usageLinks[0]?.profileName ?? "No profile names loaded in list view"}
        </span>
      </div>
    </article>
  );
}

export function ProxyTable({
  rows,
  totalCount,
  selectedProxyId,
  selectedIds,
  changeIpResults,
  allVisibleSelected,
  sortField,
  sortOptions,
  onSortFieldChange,
  onOpenRow,
  onToggleSelection,
  onSetVisibleSelection,
}: ProxyTableProps) {
  const visibleIds = rows.map((row) => row.id);
  const tableHeight = Math.max(240, Math.min(680, rows.length * 132));

  return (
    <Panel
      title="Proxy Inventory"
      subtitle="Virtualized local proxy workbench with denser provider, network, risk, and change-IP posture signals."
      actions={
        <div className="proxy-table__actions">
          <label className="field">
            <span className="field__label">Sort</span>
            <select
              className="field__input"
              value={sortField}
              onChange={(event) => onSortFieldChange(event.target.value as ProxySortField)}
            >
              {sortOptions.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
        </div>
      }
    >
      {rows.length === 0 ? (
        <EmptyState
          title="No proxies match current filters"
          detail="Adjust search, tags, health, source, or usage filters to widen the list."
        />
      ) : (
        <div className="proxy-table">
          <div className="proxy-table__meta">
            <span>
              Visible {formatCount(rows.length)} / Total {formatCount(totalCount)}
            </span>
            <button
              className="button button--secondary"
              type="button"
              onClick={() => onSetVisibleSelection(allVisibleSelected ? [] : visibleIds)}
            >
              {allVisibleSelected ? "Clear visible selection" : "Select visible rows"}
            </button>
          </div>

          <div className="proxy-table__header proxy-row">
            <div className="proxy-table__cell proxy-table__cell--selection">
              <input
                type="checkbox"
                readOnly
                checked={allVisibleSelected}
                aria-label="Select all visible proxy rows"
              />
            </div>
            <div className="proxy-table__cell proxy-table__cell--identity">Proxy</div>
            <div className="proxy-table__cell proxy-table__cell--health">Health</div>
            <div className="proxy-table__cell">Network</div>
            <div className="proxy-table__cell">Provider / Source</div>
            <div className="proxy-table__cell">Ops posture</div>
            <div className="proxy-table__cell">Usage</div>
          </div>

          <VirtualList
            items={rows}
            height={tableHeight}
            itemHeight={132}
            getKey={(item) => item.id}
            renderItem={(item) => (
              <ProxyRow
                row={item}
                changeIpFeedback={changeIpResults[item.id] ?? null}
                isActive={selectedProxyId === item.id}
                isChecked={selectedIds.includes(item.id)}
                onOpen={() => onOpenRow(item.id)}
                onToggleSelection={() => onToggleSelection(item.id)}
              />
            )}
          />
        </div>
      )}
    </Panel>
  );
}
