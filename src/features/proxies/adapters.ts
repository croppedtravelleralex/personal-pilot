import {
  checkProxyBatch,
  changeProxyIp,
  DesktopServiceError,
  listProxyPage,
  readProxyHealth,
  readProxyUsage,
} from "../../services/desktop";
import type {
  DesktopProxyBatchCheckResponse,
  DesktopProxyChangeIpRequest,
  DesktopProxyChangeIpResult,
  DesktopProxyHealth,
  DesktopProxyRow,
  DesktopProxyUsageItem,
} from "../../types/desktop";
import type {
  ProxyDataSource,
  ProxyDetailSnapshot,
  ProxyHealthSnapshot,
  ProxyHealthState,
  ProxyRotationSummary,
  ProxyRowModel,
  ProxyUsageLink,
} from "./model";
import { buildFallbackProxyDetail, createFallbackProxyRows } from "./mock";

export interface ProxyListSnapshot {
  rows: ProxyRowModel[];
  totalCount: number;
  source: ProxyDataSource;
}

function titleCase(value: string | null | undefined): string {
  if (!value) {
    return "Unknown";
  }

  return value
    .split(/[_\-\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function buildRegionLabel(country: string | null | undefined, region: string | null | undefined): string | null {
  if (country && region) {
    return `${country} / ${region}`;
  }
  return country ?? region ?? null;
}

function computeStickyTtlSeconds(expiresAt: string | null | undefined): number | null {
  if (!expiresAt) {
    return null;
  }
  const expiresAtSeconds = Number(expiresAt);
  if (!Number.isFinite(expiresAtSeconds) || expiresAtSeconds <= 0) {
    return null;
  }
  const remaining = Math.floor(expiresAtSeconds - Date.now() / 1000);
  return remaining > 0 ? remaining : 0;
}

function mapHealthState(row: DesktopProxyRow): ProxyHealthState {
  const smokeStatus = row.health?.smokeStatus?.toLowerCase() ?? "";
  const verifyStatus = row.health?.verifyStatus?.toLowerCase() ?? "";
  const grade = row.health?.grade?.toLowerCase() ?? "";

  if (smokeStatus.includes("fail") || verifyStatus.includes("fail")) {
    return "failed";
  }
  if (smokeStatus.includes("warn") || verifyStatus.includes("warn") || grade === "c") {
    return "warning";
  }
  if (smokeStatus.includes("queue") || verifyStatus.includes("queue")) {
    return "queued";
  }
  if (smokeStatus.includes("run") || verifyStatus.includes("run")) {
    return "checking";
  }
  if (row.health) {
    return "healthy";
  }
  return "unknown";
}

function mapHealthSummary(row: DesktopProxyRow): ProxyHealthSnapshot {
  const regionLabel = buildRegionLabel(row.country, row.region);
  const state = mapHealthState(row);
  const latencyMs = row.health?.latencyMs ?? null;
  const summary =
    row.health?.grade && row.health?.overallScore !== null
      ? `Grade ${row.health.grade} / Score ${row.health.overallScore}`
      : latencyMs !== null
        ? `Latency ${latencyMs}ms`
        : titleCase(row.status);

  return {
    state,
    summary,
    lastCheckAt: row.health?.checkedAt ?? row.lastCheckedAt,
    latencyMs,
    exitIp: null,
    regionLabel,
    failureReason: null,
    batchState: "idle",
  };
}

function mapRotationSummaryFromRow(row: DesktopProxyRow): ProxyRotationSummary {
  const requestedProvider = row.usage.requestedProvider ?? row.provider ?? null;
  const requestedRegion = row.usage.requestedRegion ?? row.region ?? null;
  const sessionKey = row.usage.sessionKey ?? null;
  const expiresAt = row.usage.expiresAt ?? null;
  const residencyStatus =
    row.usage.residencyStatus ??
    (sessionKey ? (expiresAt ? "sticky_active" : "sticky_unbounded") : "stateless_rotation");
  const rotationMode =
    row.usage.rotationMode ??
    (sessionKey ? "sticky_refresh" : requestedProvider || requestedRegion ? "provider_aware_rotate" : "pool_rotate");

  return {
    residencyStatus,
    rotationMode,
    sessionKey,
    requestedProvider,
    requestedRegion,
    stickyTtlSeconds:
      row.usage.stickyTtlSeconds ?? computeStickyTtlSeconds(expiresAt),
    expiresAt,
    note: null,
    trackingTaskId: null,
  };
}

function buildTags(row: DesktopProxyRow): string[] {
  const tags = [
    row.scheme,
    row.country?.toLowerCase() ?? null,
    row.provider?.toLowerCase() ?? null,
    row.status.toLowerCase(),
  ].filter((value): value is string => Boolean(value));

  return Array.from(new Set(tags));
}

function mapProxyRow(row: DesktopProxyRow): ProxyRowModel {
  return {
    id: row.id,
    name: row.endpointLabel,
    endpoint: row.host,
    port: row.port,
    protocol: row.scheme,
    source: row.sourceLabel ?? "desktop",
    sourceLabel: row.sourceLabel ?? "Desktop",
    providerLabel: row.provider ?? "Unknown provider",
    authLabel: row.hasCredentials ? "Credentials configured" : "No credentials",
    tags: buildTags(row),
    note:
      row.health?.geoMatchOk === false
        ? "Geo match warning requires follow-up."
        : row.usage.linkedProfileCount > 0
          ? "Linked to active profile inventory."
          : "Ready for future assignment.",
    exitIp: null,
    regionLabel: buildRegionLabel(row.country, row.region),
    usageCount: row.usage.linkedProfileCount,
    activeUsageCount: row.usage.activeSessionCount,
    usageLinks: [],
    rotation: mapRotationSummaryFromRow(row),
    health: mapHealthSummary(row),
    lastUpdatedAt: row.updatedAt,
  };
}

function mapProxyHealthDetail(
  detail: DesktopProxyHealth,
  fallbackRow: ProxyRowModel | null,
): ProxyHealthSnapshot {
  return {
    state:
      detail.reachable === false
        ? "failed"
        : detail.geoMatchOk === false || detail.verifyStatus?.toLowerCase().includes("warn")
          ? "warning"
          : detail.checkedAt
            ? "healthy"
            : fallbackRow?.health.state ?? "unknown",
    summary:
      detail.grade && detail.overallScore !== null
        ? `Grade ${detail.grade} / Score ${detail.overallScore}`
        : detail.probeError ?? fallbackRow?.health.summary ?? "No health detail",
    lastCheckAt: detail.checkedAt,
    latencyMs: detail.latencyMs,
    exitIp: detail.exitIp,
    regionLabel: buildRegionLabel(detail.exitCountry, detail.exitRegion),
    failureReason: detail.probeError,
    batchState: "completed",
  };
}

function mapUsageItem(item: DesktopProxyUsageItem): ProxyUsageLink {
  return {
    id: item.sessionKey,
    profileId: item.profileId ?? "unknown-profile",
    profileName: item.profileLabel ?? item.profileId ?? "Unknown profile",
    groupName: item.storeId ?? item.siteKey ?? "Unassigned group",
    profileStatus:
      item.status === "running"
        ? "running"
        : item.status === "ready" || item.status === "idle"
          ? "ready"
          : "paused",
    assignedAt: item.lastUsedAt,
  };
}

function mapRotationSummaryFromUsage(
  usage: DesktopProxyUsageItem[],
): ProxyRotationSummary | null {
  const latest = usage[0];
  if (!latest) {
    return null;
  }
  const residencyStatus =
    latest.status === "expired"
      ? "sticky_expired"
      : latest.status === "degraded"
        ? "sticky_degraded"
        : latest.expiresAt
          ? "sticky_active"
          : "stateless_rotation";
  const rotationMode = latest.sessionKey
    ? residencyStatus === "sticky_expired"
      ? "sticky_rebind"
      : "sticky_refresh"
    : "provider_aware_rotate";

  return {
    residencyStatus,
    rotationMode,
    sessionKey: latest.sessionKey,
    requestedProvider: latest.requestedProvider,
    requestedRegion: latest.requestedRegion,
    stickyTtlSeconds: computeStickyTtlSeconds(latest.expiresAt),
    expiresAt: latest.expiresAt,
    note: null,
    trackingTaskId: null,
  };
}

function isNotReadyError(error: unknown): error is DesktopServiceError {
  return error instanceof DesktopServiceError && error.code === "desktop_command_not_ready";
}

export async function loadProxyListSnapshot(): Promise<ProxyListSnapshot> {
  try {
    const page = await listProxyPage({
      page: 1,
      pageSize: 500,
    });

    return {
      rows: page.items.map(mapProxyRow),
      totalCount: page.total,
      source: "desktop",
    };
  } catch (error) {
    if (isNotReadyError(error)) {
      const rows = createFallbackProxyRows();
      return {
        rows,
        totalCount: rows.length,
        source: "fallback",
      };
    }
    throw error;
  }
}

export async function loadProxyDetailSnapshot(
  proxyId: string,
  knownRows: ProxyRowModel[],
): Promise<ProxyDetailSnapshot> {
  const fallbackRow = knownRows.find((row) => row.id === proxyId) ?? null;

  try {
    const [health, usage] = await Promise.all([readProxyHealth(proxyId), readProxyUsage(proxyId)]);

    return {
      proxyId,
      health: mapProxyHealthDetail(health, fallbackRow),
      usageLinks: usage.map(mapUsageItem),
      rotation: mapRotationSummaryFromUsage(usage) ?? fallbackRow?.rotation ?? null,
      source: "desktop",
    };
  } catch (error) {
    if (!isNotReadyError(error)) {
      throw error;
    }

    const fallback =
      buildFallbackProxyDetail(proxyId) ??
      (fallbackRow
        ? {
            proxyId,
            health: {
              ...fallbackRow.health,
            },
            usageLinks: [...fallbackRow.usageLinks],
            rotation: { ...fallbackRow.rotation },
            source: "fallback" as const,
          }
        : null);

    if (!fallback) {
      throw new DesktopServiceError(
        `Proxy detail fallback is missing for ${proxyId}.`,
        "proxy_detail_missing",
      );
    }

    return fallback;
  }
}

export async function runProxyBatchCheck(
  proxyIds: string[],
): Promise<DesktopProxyBatchCheckResponse> {
  const normalizedProxyIds = Array.from(
    new Set(proxyIds.map((proxyId) => proxyId.trim()).filter(Boolean)),
  );

  return checkProxyBatch({
    proxyIds: normalizedProxyIds,
    limit: normalizedProxyIds.length,
  });
}

export interface ProxyChangeIpInput {
  proxyId: string;
  mode?: string | null;
  sessionKey?: string | null;
  requestedProvider?: string | null;
  requestedRegion?: string | null;
  stickyTtlSeconds?: number | null;
}

export async function runProxyChangeIp(
  request: ProxyChangeIpInput,
): Promise<DesktopProxyChangeIpResult> {
  const payload: DesktopProxyChangeIpRequest = {
    proxyId: request.proxyId.trim(),
    mode: request.mode ?? null,
    sessionKey: request.sessionKey ?? null,
    requestedProvider: request.requestedProvider ?? null,
    requestedRegion: request.requestedRegion ?? null,
    stickyTtlSeconds: request.stickyTtlSeconds ?? null,
  };

  return changeProxyIp(payload);
}
