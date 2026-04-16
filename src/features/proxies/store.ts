import { createStore } from "../../store/createStore";
import type {
  DesktopProxyBatchCheckResponse,
  DesktopProxyChangeIpResult,
} from "../../types/desktop";
import {
  type ProxyBatchCheckState,
  type ProxyChangeIpState,
  type ProxyDataSource,
  type ProxyDetailSnapshot,
  type ProxyFilterState,
  type ProxyHealthState,
  type ProxyRotationSummary,
  type ProxyRowModel,
  type ProxySortField,
  type ProxyTableState,
} from "./model";

const DEFAULT_BATCH_MESSAGE =
  "Batch check is wired to the native verify-batch command and refreshes the workbench after each run.";
const DEFAULT_CHANGE_IP_MESSAGE =
  "Change IP writes are submitted through the shared desktop contract with tracking feedback; provider-side exit change still requires later detail verification.";

interface ProxyChangeIpRequestContext {
  mode?: string | null;
  sessionKey?: string | null;
  requestedProvider?: string | null;
  requestedRegion?: string | null;
  stickyTtlSeconds?: number | null;
}

export interface ProxiesState {
  rows: ProxyRowModel[];
  totalCount: number;
  dataSource: ProxyDataSource | null;
  isLoadingList: boolean;
  listError: string | null;
  filters: ProxyFilterState;
  table: ProxyTableState;
  selectedProxyId: string | null;
  selectedIds: string[];
  detail: ProxyDetailSnapshot | null;
  detailSource: ProxyDataSource | null;
  isLoadingDetail: boolean;
  detailError: string | null;
  batchCheck: ProxyBatchCheckState;
  changeIp: ProxyChangeIpState;
}

const INITIAL_BATCH_STATE: ProxyBatchCheckState = {
  phase: "idle",
  scope: "filtered",
  targetIds: [],
  targetCount: 0,
  completedCount: 0,
  requestId: 0,
  feedbackTone: "neutral",
  lastMessage: DEFAULT_BATCH_MESSAGE,
  lastStartedAt: null,
  lastFinishedAt: null,
};

const INITIAL_CHANGE_IP_STATE: ProxyChangeIpState = {
  phase: "idle",
  requestId: 0,
  targetIds: [],
  completedCount: 0,
  succeededCount: 0,
  failedCount: 0,
  activeProxyId: null,
  feedbackTone: "neutral",
  lastMessage: DEFAULT_CHANGE_IP_MESSAGE,
  lastStartedAt: null,
  lastFinishedAt: null,
  results: {},
};

export const proxiesStore = createStore<ProxiesState>({
  rows: [],
  totalCount: 0,
  dataSource: null,
  isLoadingList: false,
  listError: null,
  filters: {
    searchInput: "",
    appliedSearch: "",
    healthFilter: "all",
    sourceFilter: "all",
    usageFilter: "all",
    regionFilter: "all",
    tagFilter: "all",
  },
  table: {
    sortField: "updated",
  },
  selectedProxyId: null,
  selectedIds: [],
  detail: null,
  detailSource: null,
  isLoadingDetail: false,
  detailError: null,
  batchCheck: INITIAL_BATCH_STATE,
  changeIp: INITIAL_CHANGE_IP_STATE,
});

const HEALTH_PRIORITY: Record<ProxyHealthState, number> = {
  failed: 0,
  warning: 1,
  unknown: 2,
  queued: 3,
  checking: 4,
  healthy: 5,
};

function toSortTimestamp(value: string | null): number {
  if (!value) {
    return 0;
  }

  const numericValue = Number(value);
  if (Number.isFinite(numericValue) && numericValue > 0) {
    return numericValue;
  }

  const parsedMs = Date.parse(value);
  return Number.isNaN(parsedMs) ? 0 : Math.floor(parsedMs / 1000);
}

function getSearchHaystack(row: ProxyRowModel): string {
  return [
    row.name,
    row.endpoint,
    row.providerLabel,
    row.sourceLabel,
    row.authLabel,
    row.exitIp ?? "",
    row.regionLabel ?? "",
    row.note,
    ...row.tags,
    ...row.usageLinks.map((usage) => usage.profileName),
  ]
    .join(" ")
    .toLowerCase();
}

function matchesFilters(row: ProxyRowModel, state: ProxiesState): boolean {
  const search = state.filters.appliedSearch.trim().toLowerCase();
  if (search && !getSearchHaystack(row).includes(search)) {
    return false;
  }

  if (state.filters.healthFilter !== "all" && row.health.state !== state.filters.healthFilter) {
    return false;
  }

  if (state.filters.sourceFilter !== "all" && row.source !== state.filters.sourceFilter) {
    return false;
  }

  const rowRegion = row.regionLabel ?? "Pending region";
  if (state.filters.regionFilter !== "all" && rowRegion !== state.filters.regionFilter) {
    return false;
  }

  if (state.filters.tagFilter !== "all" && !row.tags.includes(state.filters.tagFilter)) {
    return false;
  }

  switch (state.filters.usageFilter) {
    case "used":
      return row.usageCount > 0;
    case "unused":
      return row.usageCount === 0;
    case "active":
      return row.activeUsageCount > 0;
    default:
      return true;
  }
}

function sortRows(rows: ProxyRowModel[], sortField: ProxySortField): ProxyRowModel[] {
  return [...rows].sort((left, right) => {
    switch (sortField) {
      case "health":
        return (
          HEALTH_PRIORITY[right.health.state] - HEALTH_PRIORITY[left.health.state] ||
          right.activeUsageCount - left.activeUsageCount ||
          left.name.localeCompare(right.name)
        );
      case "usage":
        return (
          right.usageCount - left.usageCount ||
          right.activeUsageCount - left.activeUsageCount ||
          left.name.localeCompare(right.name)
        );
      case "name":
        return left.name.localeCompare(right.name);
      case "updated":
      default:
        return (
          toSortTimestamp(right.health.lastCheckAt ?? right.lastUpdatedAt) -
            toSortTimestamp(left.health.lastCheckAt ?? left.lastUpdatedAt) ||
          left.name.localeCompare(right.name)
        );
    }
  });
}

export function getFilteredProxyRows(state: ProxiesState): ProxyRowModel[] {
  return sortRows(
    state.rows.filter((row) => matchesFilters(row, state)),
    state.table.sortField,
  );
}

function getBatchTargetIds(state: ProxiesState): string[] {
  return state.batchCheck.scope === "selected"
    ? state.selectedIds
    : getFilteredProxyRows(state).map((row) => row.id);
}

function applyHealthBatchState(row: ProxyRowModel, targetIds: string[], batchState: "queued" | "running"): ProxyRowModel {
  if (!targetIds.includes(row.id)) {
    return row;
  }

  return {
    ...row,
    health: {
      ...row.health,
      batchState,
    },
  };
}

function applyDetailBatchState(
  detail: ProxyDetailSnapshot | null,
  targetIds: string[],
  batchState: "queued" | "running",
): ProxyDetailSnapshot | null {
  if (!detail || !targetIds.includes(detail.proxyId)) {
    return detail;
  }

  return {
    ...detail,
    health: {
      ...detail.health,
      batchState,
    },
  };
}

function createBatchSuccessMessage(response: DesktopProxyBatchCheckResponse): string {
  if (response.acceptedCount === 0) {
    return `Native verify batch finished with no accepted proxies. ${response.skippedCount} targets were skipped.`;
  }

  const skippedSuffix =
    response.skippedCount > 0 ? ` ${response.skippedCount} additional targets were skipped.` : "";

  return `Native verify batch ${response.batchId} staged ${response.acceptedCount} of ${response.requestedCount} proxies.${skippedSuffix}`;
}

function syncSelectedRowDetail(rows: ProxyRowModel[], detail: ProxyDetailSnapshot | null) {
  if (!detail) {
    return { rows, detail };
  }

  const nextRows = rows.map((row) =>
    row.id === detail.proxyId
      ? {
          ...row,
          usageLinks: detail.usageLinks,
          usageCount: detail.usageLinks.length || row.usageCount,
          activeUsageCount:
            detail.usageLinks.filter((item) => item.profileStatus === "running").length ||
            row.activeUsageCount,
          exitIp: detail.health.exitIp ?? row.exitIp,
          regionLabel: detail.health.regionLabel ?? row.regionLabel,
          rotation: detail.rotation ?? row.rotation,
          health: detail.health,
        }
      : row,
  );

  return { rows: nextRows, detail };
}

function normalizeChangeIpContext(
  context?: ProxyChangeIpRequestContext,
): Required<ProxyChangeIpRequestContext> {
  return {
    mode: context?.mode ?? null,
    sessionKey: context?.sessionKey ?? null,
    requestedProvider: context?.requestedProvider ?? null,
    requestedRegion: context?.requestedRegion ?? null,
    stickyTtlSeconds: context?.stickyTtlSeconds ?? null,
  };
}

function buildRotationSummaryFromChangeResult(
  result: DesktopProxyChangeIpResult,
): ProxyRotationSummary {
  return {
    residencyStatus: result.residencyStatus,
    rotationMode: result.rotationMode,
    sessionKey: result.sessionKey,
    requestedProvider: result.requestedProvider,
    requestedRegion: result.requestedRegion,
    stickyTtlSeconds: result.stickyTtlSeconds,
    expiresAt: result.expiresAt,
    note: result.note,
    trackingTaskId: result.trackingTaskId,
  };
}

function mergeRotationSummaryWithChangeResult(
  base: ProxyRotationSummary | null | undefined,
  result: DesktopProxyChangeIpResult,
): ProxyRotationSummary {
  const seed = base ?? buildRotationSummaryFromChangeResult(result);

  return {
    residencyStatus: result.residencyStatus || seed.residencyStatus,
    rotationMode: result.rotationMode || seed.rotationMode,
    sessionKey: result.sessionKey ?? seed.sessionKey,
    requestedProvider: result.requestedProvider ?? seed.requestedProvider,
    requestedRegion: result.requestedRegion ?? seed.requestedRegion,
    stickyTtlSeconds: result.stickyTtlSeconds ?? seed.stickyTtlSeconds,
    expiresAt: result.expiresAt ?? seed.expiresAt,
    note: result.note || seed.note,
    trackingTaskId: result.trackingTaskId || seed.trackingTaskId,
  };
}

export const proxyActions = {
  setRows(rows: ProxyRowModel[], totalCount: number, dataSource: ProxyDataSource) {
    proxiesStore.setState((current) => {
      const selectedProxyId =
        current.selectedProxyId && rows.some((row) => row.id === current.selectedProxyId)
          ? current.selectedProxyId
          : rows[0]?.id ?? null;

      return {
        ...current,
        rows,
        totalCount,
        dataSource,
        isLoadingList: false,
        listError: null,
        selectedProxyId,
        selectedIds: current.selectedIds.filter((id) => rows.some((row) => row.id === id)),
      };
    });
  },
  setListLoading(isLoadingList: boolean) {
    proxiesStore.setState((current) => ({
      ...current,
      isLoadingList,
      listError: isLoadingList ? null : current.listError,
    }));
  },
  setListError(listError: string) {
    proxiesStore.setState((current) => ({
      ...current,
      isLoadingList: false,
      listError,
    }));
  },
  setSearchInput(searchInput: string) {
    proxiesStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        searchInput,
      },
    }));
  },
  applySearch(appliedSearch: string) {
    proxiesStore.setState((current) => {
      if (current.filters.appliedSearch === appliedSearch) {
        return current;
      }

      return {
        ...current,
        filters: {
          ...current.filters,
          appliedSearch,
        },
        selectedIds: [],
      };
    });
  },
  setHealthFilter(healthFilter: ProxyFilterState["healthFilter"]) {
    proxiesStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        healthFilter,
      },
      selectedIds: [],
    }));
  },
  setSourceFilter(sourceFilter: ProxyFilterState["sourceFilter"]) {
    proxiesStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        sourceFilter,
      },
      selectedIds: [],
    }));
  },
  setUsageFilter(usageFilter: ProxyFilterState["usageFilter"]) {
    proxiesStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        usageFilter,
      },
      selectedIds: [],
    }));
  },
  setRegionFilter(regionFilter: string) {
    proxiesStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        regionFilter,
      },
      selectedIds: [],
    }));
  },
  setTagFilter(tagFilter: string) {
    proxiesStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        tagFilter,
      },
      selectedIds: [],
    }));
  },
  clearFilters() {
    proxiesStore.setState((current) => ({
      ...current,
      filters: {
        ...current.filters,
        searchInput: "",
        appliedSearch: "",
        healthFilter: "all",
        sourceFilter: "all",
        usageFilter: "all",
        regionFilter: "all",
        tagFilter: "all",
      },
      selectedIds: [],
    }));
  },
  setSortField(sortField: ProxySortField) {
    proxiesStore.setState((current) => ({
      ...current,
      table: {
        ...current.table,
        sortField,
      },
    }));
  },
  selectProxy(selectedProxyId: string) {
    proxiesStore.setState((current) => ({
      ...current,
      selectedProxyId,
      detailError: null,
    }));
  },
  toggleSelection(proxyId: string) {
    proxiesStore.setState((current) => {
      const selectedIds = current.selectedIds.includes(proxyId)
        ? current.selectedIds.filter((item) => item !== proxyId)
        : [...current.selectedIds, proxyId];

      return {
        ...current,
        selectedProxyId: proxyId,
        selectedIds,
      };
    });
  },
  setSelection(proxyIds: string[]) {
    proxiesStore.setState((current) => ({
      ...current,
      selectedIds: current.rows
        .map((row) => row.id)
        .filter((rowId) => proxyIds.includes(rowId)),
      selectedProxyId: proxyIds[0] ?? current.selectedProxyId,
    }));
  },
  clearSelection() {
    proxiesStore.setState((current) => ({
      ...current,
      selectedIds: [],
    }));
  },
  setBatchScope(scope: ProxyBatchCheckState["scope"]) {
    proxiesStore.setState((current) => ({
      ...current,
      batchCheck: {
        ...current.batchCheck,
        scope,
      },
    }));
  },
  setDetailLoading(isLoadingDetail: boolean) {
    proxiesStore.setState((current) => ({
      ...current,
      isLoadingDetail,
      detailError: isLoadingDetail ? null : current.detailError,
    }));
  },
  setDetail(detail: ProxyDetailSnapshot) {
    proxiesStore.setState((current) => {
      const merged = syncSelectedRowDetail(current.rows, detail);
      return {
        ...current,
        rows: merged.rows,
        detail,
        detailSource: detail.source,
        isLoadingDetail: false,
        detailError: null,
      };
    });
  },
  setDetailError(detailError: string) {
    proxiesStore.setState((current) => ({
      ...current,
      isLoadingDetail: false,
      detailError,
    }));
  },
  dismissBatchFeedback() {
    proxiesStore.setState((current) => ({
      ...current,
      batchCheck: {
        ...current.batchCheck,
        phase: "idle",
        feedbackTone: "neutral",
        lastMessage: DEFAULT_BATCH_MESSAGE,
      },
    }));
  },
  dismissChangeIpFeedback() {
    proxiesStore.setState((current) => ({
      ...current,
      changeIp: {
        ...current.changeIp,
        phase: "idle",
        feedbackTone: "neutral",
        lastMessage: DEFAULT_CHANGE_IP_MESSAGE,
      },
    }));
  },
  startBatchCheckBlocked(message: string) {
    proxiesStore.setState((current) => ({
      ...current,
      batchCheck: {
        ...current.batchCheck,
        phase: "blocked",
        targetIds: [],
        targetCount: 0,
        completedCount: 0,
        feedbackTone: "warning",
        lastMessage: message,
        lastFinishedAt: String(Math.floor(Date.now() / 1000)),
      },
    }));
  },
  startChangeIpBlocked(message: string) {
    proxiesStore.setState((current) => ({
      ...current,
      changeIp: {
        ...current.changeIp,
        phase: "blocked",
        targetIds: [],
        completedCount: 0,
        succeededCount: 0,
        failedCount: 0,
        activeProxyId: null,
        feedbackTone: "warning",
        lastMessage: message,
        lastFinishedAt: String(Math.floor(Date.now() / 1000)),
      },
    }));
  },
  startBatchCheckRequested(targetIds: string[]) {
    const startedAt = String(Math.floor(Date.now() / 1000));
    let nextRequestId = 0;

    proxiesStore.setState((current) => {
      nextRequestId = current.batchCheck.requestId + 1;

      return {
        ...current,
        batchCheck: {
          ...current.batchCheck,
          phase: "running",
          targetIds,
          targetCount: targetIds.length,
          completedCount: 0,
          requestId: nextRequestId,
          feedbackTone: "neutral",
          lastMessage: `Submitting native verify batch for ${targetIds.length} proxies.`,
          lastStartedAt: startedAt,
          lastFinishedAt: null,
        },
      };
    });

    return nextRequestId;
  },
  startChangeIpRequested(targetIds: string[]) {
    const startedAt = String(Math.floor(Date.now() / 1000));
    let nextRequestId = 0;

    proxiesStore.setState((current) => {
      nextRequestId = current.changeIp.requestId + 1;

      return {
        ...current,
        changeIp: {
          ...current.changeIp,
          phase: "running",
          requestId: nextRequestId,
          targetIds,
          completedCount: 0,
          succeededCount: 0,
          failedCount: 0,
          activeProxyId: targetIds[0] ?? null,
          feedbackTone: "neutral",
          lastMessage: `Submitting change IP for ${targetIds.length} proxy target${targetIds.length === 1 ? "" : "s"}.`,
          lastStartedAt: startedAt,
          lastFinishedAt: null,
          results: Object.fromEntries(
            targetIds.map((proxyId) => [
              proxyId,
              {
                proxyId,
                phase: "running" as const,
                message: "Queued for provider-write submission.",
                status: "submitting",
                mode: null,
                sessionKey: null,
                requestedProvider: null,
                requestedRegion: null,
                stickyTtlSeconds: null,
                note: "Preparing changeProxyIp payload.",
                residencyStatus: null,
                rotationMode: null,
                trackingTaskId: null,
                expiresAt: null,
                updatedAt: startedAt,
              },
            ]),
          ),
        },
      };
    });

    return nextRequestId;
  },
  setChangeIpStepRunning(
    requestId: number,
    proxyId: string,
    message: string,
    requestContext?: ProxyChangeIpRequestContext,
  ) {
    proxiesStore.setState((current) => {
      if (current.changeIp.requestId !== requestId) {
        return current;
      }

      const previous = current.changeIp.results[proxyId];
      const context = normalizeChangeIpContext(requestContext);
      const updatedAt = String(Math.floor(Date.now() / 1000));

      return {
        ...current,
        changeIp: {
          ...current.changeIp,
          phase: "running",
          activeProxyId: proxyId,
          lastMessage: message,
          results: {
            ...current.changeIp.results,
            [proxyId]: {
              proxyId,
              phase: "running",
              message,
              status: previous?.status ?? "submitting",
              mode: context.mode ?? previous?.mode ?? null,
              sessionKey: context.sessionKey ?? previous?.sessionKey ?? null,
              requestedProvider:
                context.requestedProvider ?? previous?.requestedProvider ?? null,
              requestedRegion: context.requestedRegion ?? previous?.requestedRegion ?? null,
              stickyTtlSeconds:
                context.stickyTtlSeconds ?? previous?.stickyTtlSeconds ?? null,
              note:
                previous?.note ??
                "Submitting provider-aware write task through changeProxyIp.",
              residencyStatus: previous?.residencyStatus ?? null,
              rotationMode: context.mode ?? previous?.rotationMode ?? null,
              trackingTaskId: previous?.trackingTaskId ?? null,
              expiresAt: previous?.expiresAt ?? null,
              updatedAt,
            },
          },
        },
      };
    });
  },
  recordChangeIpSuccess(
    requestId: number,
    proxyId: string,
    result: DesktopProxyChangeIpResult,
  ) {
    proxiesStore.setState((current) => {
      if (current.changeIp.requestId !== requestId) {
        return current;
      }

      const nextRows = current.rows.map((row) =>
        row.id === proxyId
          ? {
              ...row,
              rotation: mergeRotationSummaryWithChangeResult(row.rotation, result),
            }
          : row,
      );
      const rowAfterMerge = nextRows.find((row) => row.id === proxyId) ?? null;
      const nextDetail =
        current.detail && current.detail.proxyId === proxyId
          ? {
              ...current.detail,
              rotation: mergeRotationSummaryWithChangeResult(
                current.detail.rotation ?? rowAfterMerge?.rotation ?? null,
                result,
              ),
            }
          : current.detail;

      return {
        ...current,
        rows: nextRows,
        detail: nextDetail,
        changeIp: {
          ...current.changeIp,
          completedCount: current.changeIp.completedCount + 1,
          succeededCount: current.changeIp.succeededCount + 1,
          feedbackTone: "warning",
          lastMessage: result.message,
          lastFinishedAt: result.updatedAt,
          results: {
            ...current.changeIp.results,
            [proxyId]: {
              proxyId,
              phase: "success",
              message: result.message,
              status: result.status,
              mode: result.mode,
              sessionKey: result.sessionKey,
              requestedProvider: result.requestedProvider,
              requestedRegion: result.requestedRegion,
              stickyTtlSeconds: result.stickyTtlSeconds,
              note: result.note,
              residencyStatus: result.residencyStatus,
              rotationMode: result.rotationMode,
              trackingTaskId: result.trackingTaskId,
              expiresAt: result.expiresAt,
              updatedAt: result.updatedAt,
            },
          },
        },
      };
    });
  },
  recordChangeIpFailure(
    requestId: number,
    proxyId: string,
    error: string,
    finishedAt: string,
    requestContext?: ProxyChangeIpRequestContext,
  ) {
    proxiesStore.setState((current) => {
      if (current.changeIp.requestId !== requestId) {
        return current;
      }

      const previous = current.changeIp.results[proxyId];
      const context = normalizeChangeIpContext(requestContext);

      return {
        ...current,
        changeIp: {
          ...current.changeIp,
          completedCount: current.changeIp.completedCount + 1,
          failedCount: current.changeIp.failedCount + 1,
          feedbackTone: "error",
          lastMessage: error,
          lastFinishedAt: finishedAt,
          results: {
            ...current.changeIp.results,
            [proxyId]: {
              proxyId,
              phase: "error",
              message: error,
              status: previous?.status ?? "local_request_failed",
              mode: context.mode ?? previous?.mode ?? null,
              sessionKey: context.sessionKey ?? previous?.sessionKey ?? null,
              requestedProvider:
                context.requestedProvider ?? previous?.requestedProvider ?? null,
              requestedRegion: context.requestedRegion ?? previous?.requestedRegion ?? null,
              stickyTtlSeconds:
                context.stickyTtlSeconds ?? previous?.stickyTtlSeconds ?? null,
              note:
                previous?.note ??
                "Provider-side write not confirmed because local request failed.",
              residencyStatus: previous?.residencyStatus ?? null,
              rotationMode: context.mode ?? previous?.rotationMode ?? null,
              trackingTaskId: previous?.trackingTaskId ?? null,
              expiresAt: previous?.expiresAt ?? null,
              updatedAt: finishedAt,
            },
          },
        },
      };
    });
  },
  finishChangeIpRun(requestId: number) {
    proxiesStore.setState((current) => {
      if (current.changeIp.requestId !== requestId) {
        return current;
      }

      const hasSuccess = current.changeIp.succeededCount > 0;
      const hasFailure = current.changeIp.failedCount > 0;
      const targetCount = current.changeIp.targetIds.length;
      const resultItems = Object.values(current.changeIp.results);
      const acceptedWrites = resultItems.filter((result) => {
        const status = result.status?.toLowerCase() ?? "";
        return result.phase === "success" && (status.includes("queued") || Boolean(result.trackingTaskId));
      }).length;
      const rollbackSignals = resultItems.filter((result) =>
        /rollback|revert|compensat/i.test(
          `${result.status ?? ""} ${result.message ?? ""} ${result.note ?? ""}`,
        ),
      ).length;

      return {
        ...current,
        changeIp: {
          ...current.changeIp,
          phase: hasSuccess ? "completed" : hasFailure ? "error" : "idle",
          activeProxyId: null,
          feedbackTone: hasSuccess && hasFailure
            ? "warning"
            : hasSuccess
              ? "warning"
              : "error",
          lastMessage:
            targetCount === 0
              ? DEFAULT_CHANGE_IP_MESSAGE
              : `Change IP submission finished for ${current.changeIp.completedCount}/${targetCount} targets. Accepted writes ${acceptedWrites}, local failures ${current.changeIp.failedCount}, rollback signals ${rollbackSignals}.`,
          lastFinishedAt:
            current.changeIp.lastFinishedAt ?? String(Math.floor(Date.now() / 1000)),
        },
      };
    });
  },
  startBatchCheckSucceeded(
    requestId: number,
    targetIds: string[],
    response: DesktopProxyBatchCheckResponse,
  ) {
    proxiesStore.setState((current) => {
      if (current.batchCheck.requestId !== requestId) {
        return current;
      }

      return {
        ...current,
        rows:
          response.acceptedCount > 0
            ? current.rows.map((row) => applyHealthBatchState(row, targetIds, "queued"))
            : current.rows,
        detail:
          response.acceptedCount > 0
            ? applyDetailBatchState(current.detail, targetIds, "queued")
            : current.detail,
        batchCheck: {
          ...current.batchCheck,
          phase: "completed",
          targetIds,
          targetCount: response.requestedCount,
          completedCount: response.acceptedCount,
          feedbackTone: response.acceptedCount > 0 ? "success" : "warning",
          lastMessage: createBatchSuccessMessage(response),
          lastFinishedAt: response.updatedAt,
        },
      };
    });
  },
  startBatchCheckFailed(requestId: number, error: string, finishedAt: string) {
    proxiesStore.setState((current) => {
      if (current.batchCheck.requestId !== requestId) {
        return current;
      }

      return {
        ...current,
        batchCheck: {
          ...current.batchCheck,
          phase: "error",
          completedCount: 0,
          feedbackTone: "error",
          lastMessage: error,
          lastFinishedAt: finishedAt,
        },
      };
    });
  },
};
