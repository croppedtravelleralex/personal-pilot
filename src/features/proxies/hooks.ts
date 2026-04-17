import { startTransition, useDeferredValue, useEffect, useEffectEvent, useMemo } from "react";

import { useDebouncedValue } from "../../hooks/useDebouncedValue";
import { DesktopServiceError } from "../../services/desktop";
import { useStore } from "../../store/createStore";
import {
  loadProxyDetailSnapshot,
  loadProxyListSnapshot,
  type ProxyChangeIpInput,
  runProxyChangeIp,
  runProxyBatchCheck,
} from "./adapters";
import {
  getProxyProviderWriteState,
  isProxyChangeCoolingDown,
  parseProxyTimestamp,
} from "./changeIpFeedback";
import type {
  ProxyChangeIpState,
  ProxyHealthState,
  ProxyRowModel,
  ProxySortField,
  ProxyUsageFilter,
} from "./model";
import type { ProxyChangeIpFailureInput } from "./store";
import { getFilteredProxyRows, proxiesStore, proxyActions } from "./store";

interface FilterOption<T extends string = string> {
  value: T;
  label: string;
}

const HEALTH_OPTIONS: FilterOption<"all" | ProxyHealthState>[] = [
  { value: "all", label: "All health" },
  { value: "healthy", label: "Healthy" },
  { value: "warning", label: "Warning" },
  { value: "failed", label: "Failed" },
  { value: "unknown", label: "Unknown" },
  { value: "queued", label: "Queued" },
  { value: "checking", label: "Checking" },
];

const USAGE_OPTIONS: FilterOption<ProxyUsageFilter>[] = [
  { value: "all", label: "All usage" },
  { value: "active", label: "Active usage" },
  { value: "used", label: "Assigned" },
  { value: "unused", label: "Unused" },
];

const SORT_OPTIONS: FilterOption<ProxySortField>[] = [
  { value: "updated", label: "Last checked" },
  { value: "health", label: "Health priority" },
  { value: "usage", label: "Usage count" },
  { value: "name", label: "Proxy name" },
];

function toErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function toChangeIpErrorMessage(error: unknown): string {
  if (error instanceof DesktopServiceError && error.code === "desktop_command_not_ready") {
    return "changeProxyIp is unavailable in this runtime. Keep the latest tracked provider-write posture and retry after desktop contract sync.";
  }

  return toErrorMessage(error);
}

function buildChangeIpFailureInput(error: unknown): ProxyChangeIpFailureInput {
  if (error instanceof DesktopServiceError) {
    return {
      message: toChangeIpErrorMessage(error),
      details:
        error.details ??
        ({
          code: error.code,
          message: error.message,
        } as const),
    };
  }

  if (error instanceof Error) {
    return {
      message: toChangeIpErrorMessage(error),
      details: {
        message: error.message,
        stack: error.stack ?? null,
      },
    };
  }

  return {
    message: toChangeIpErrorMessage(error),
    details: error,
  };
}

function getChangeIpTargetIds(state: ReturnType<typeof proxiesStore.getState>): string[] {
  const selectedIds =
    state.selectedIds.length > 0
      ? state.selectedIds
      : state.selectedProxyId
        ? [state.selectedProxyId]
        : [];

  return Array.from(new Set(selectedIds));
}

function getChangeIpTargetLabel(state: ProxyChangeIpState, selectedCount: number): string {
  if (state.phase === "running") {
    return state.targetIds.length > 1 ? `Selected ${state.targetIds.length}` : "Current pinned proxy";
  }

  return selectedCount > 0 ? `Selected ${selectedCount}` : "Current pinned proxy";
}

function buildChangeIpRequestForRow(row: ProxyRowModel): ProxyChangeIpInput {
  const hasStickySession = Boolean(row.rotation.sessionKey);
  return {
    proxyId: row.id,
    mode:
      row.rotation.rotationMode ||
      (hasStickySession ? "sticky_refresh" : "provider_aware_rotate"),
    sessionKey: row.rotation.sessionKey,
    requestedProvider: row.rotation.requestedProvider ?? null,
    requestedRegion: row.rotation.requestedRegion ?? null,
    stickyTtlSeconds:
      typeof row.rotation.stickyTtlSeconds === "number" && row.rotation.stickyTtlSeconds > 0
        ? row.rotation.stickyTtlSeconds
        : null,
  };
}

export function useProxiesViewModel() {
  const state = useStore(proxiesStore, (current) => current);
  const debouncedSearch = useDebouncedValue(state.filters.searchInput, 300);
  const deferredSearch = useDeferredValue(debouncedSearch);

  const loadList = useEffectEvent(async () => {
    proxyActions.setListLoading(true);

    try {
      const snapshot = await loadProxyListSnapshot();
      proxyActions.setRows(snapshot.rows, snapshot.totalCount, snapshot.source);
    } catch (error) {
      proxyActions.setListError(toErrorMessage(error));
    }
  });

  const loadDetail = useEffectEvent(async (proxyId: string) => {
    proxyActions.setDetailLoading(true);

    try {
      const detail = await loadProxyDetailSnapshot(proxyId, proxiesStore.getState().rows);
      proxyActions.setDetail(detail);
    } catch (error) {
      proxyActions.setDetailError(toErrorMessage(error));
    }
  });

  const startBatchCheck = useEffectEvent(async () => {
    const snapshot = proxiesStore.getState();
    const targetIds =
      snapshot.batchCheck.scope === "selected"
        ? snapshot.selectedIds
        : getFilteredProxyRows(snapshot).map((row) => row.id);

    if (targetIds.length === 0) {
      proxyActions.startBatchCheckBlocked(
        snapshot.batchCheck.scope === "selected"
          ? "Please select at least one proxy row before starting batch check."
          : "No proxies are available under the current filters.",
      );
      return;
    }

    const requestId = proxyActions.startBatchCheckRequested(targetIds);

    try {
      const response = await runProxyBatchCheck(targetIds);
      const selectedProxyId = proxiesStore.getState().selectedProxyId;

      await loadList();

      if (selectedProxyId && targetIds.includes(selectedProxyId)) {
        await loadDetail(selectedProxyId);
      }

      proxyActions.startBatchCheckSucceeded(requestId, targetIds, response);
    } catch (error) {
      proxyActions.startBatchCheckFailed(
        requestId,
        toErrorMessage(error),
        String(Math.floor(Date.now() / 1000)),
      );
    }
  });

  const startChangeIp = useEffectEvent(async () => {
    const snapshot = proxiesStore.getState();
    const targetIds = getChangeIpTargetIds(snapshot);

    if (targetIds.length === 0) {
      proxyActions.startChangeIpBlocked(
        "Select one or more proxies, or pin a current proxy, before requesting IP rotation.",
      );
      return;
    }

    const requestId = proxyActions.startChangeIpRequested(targetIds);

    for (const [index, proxyId] of targetIds.entries()) {
      const targetRow = snapshot.rows.find((row) => row.id === proxyId) ?? null;
      const requestInput = targetRow ? buildChangeIpRequestForRow(targetRow) : { proxyId };
      proxyActions.setChangeIpStepRunning(
        requestId,
        proxyId,
        `Changing IP ${index + 1}/${targetIds.length} for ${proxyId}.`,
        requestInput,
      );

      try {
        const result = await runProxyChangeIp(requestInput);
        proxyActions.recordChangeIpSuccess(requestId, proxyId, result);
      } catch (error) {
        const failure = buildChangeIpFailureInput(error);
        proxyActions.recordChangeIpFailure(
          requestId,
          proxyId,
          failure,
          String(Math.floor(Date.now() / 1000)),
          requestInput,
        );
      }
    }

    await loadList();

    const selectedProxyId = proxiesStore.getState().selectedProxyId;
    if (selectedProxyId) {
      await loadDetail(selectedProxyId);
    }

    proxyActions.finishChangeIpRun(requestId);
  });

  useEffect(() => {
    void loadList();
  }, [loadList]);

  useEffect(() => {
    if (state.filters.appliedSearch !== deferredSearch) {
      startTransition(() => {
        proxyActions.applySearch(deferredSearch);
      });
    }
  }, [deferredSearch, state.filters.appliedSearch]);

  useEffect(() => {
    if (!state.selectedProxyId) {
      return;
    }

    void loadDetail(state.selectedProxyId);
  }, [loadDetail, state.selectedProxyId]);

  const rows = useMemo(() => getFilteredProxyRows(state), [state]);
  const selectedProxy = useMemo(
    () => state.rows.find((row) => row.id === state.selectedProxyId) ?? null,
    [state.rows, state.selectedProxyId],
  );

  const selectedProxyHidden = Boolean(
    selectedProxy && !rows.some((row) => row.id === selectedProxy.id),
  );
  const selectedProxyChangeIpFeedback =
    selectedProxy ? state.changeIp.results[selectedProxy.id] ?? null : null;
  const changeIpTargetLabel = getChangeIpTargetLabel(state.changeIp, state.selectedIds.length);
  const recentChangeResults = useMemo(
    () =>
      Object.values(state.changeIp.results)
        .sort((left, right) => {
          const rightTimestamp = parseProxyTimestamp(right.updatedAt) ?? 0;
          const leftTimestamp = parseProxyTimestamp(left.updatedAt) ?? 0;
          return rightTimestamp - leftTimestamp;
        })
        .slice(0, 6),
    [state.changeIp.results],
  );

  const tagOptions = useMemo(
    () =>
      ["all", ...new Set(state.rows.flatMap((row) => row.tags))].map((value) => ({
        value,
        label: value === "all" ? "All tags" : value,
      })),
    [state.rows],
  );

  const regionOptions = useMemo(
    () =>
      ["all", ...new Set(state.rows.map((row) => row.regionLabel ?? "Pending region"))].map(
        (value) => ({
          value,
          label: value === "all" ? "All regions" : value,
        }),
      ),
    [state.rows],
  );

  const sourceOptions = useMemo(() => {
    const sourceMap = new Map<string, string>();
    state.rows.forEach((row) => {
      sourceMap.set(row.source, row.sourceLabel);
    });

    return [
      { value: "all", label: "All sources" },
      ...[...sourceMap.entries()].map(([value, label]) => ({
        value,
        label,
      })),
    ];
  }, [state.rows]);

  const summary = useMemo(() => {
    const healthy = state.rows.filter((row) => row.health.state === "healthy").length;
    const warning = state.rows.filter((row) => row.health.state === "warning").length;
    const failed = state.rows.filter((row) => row.health.state === "failed").length;
    const activeUsage = state.rows.reduce((sum, row) => sum + row.activeUsageCount, 0);
    const used = state.rows.filter((row) => row.usageCount > 0).length;
    const ready = state.rows.filter(
      (row) => row.health.state === "healthy" && row.usageCount === 0,
    ).length;
    const highRisk = state.rows.filter(
      (row) =>
        row.health.state === "failed" ||
        row.health.state === "warning" ||
        (row.health.latencyMs ?? 0) >= 800,
    ).length;
    const providers = new Set(state.rows.map((row) => row.providerLabel)).size;
    const sources = new Set(state.rows.map((row) => row.sourceLabel)).size;
    const localRotationTracked = Object.keys(state.changeIp.results).length;
    const localRotationSuccess = Object.values(state.changeIp.results).filter(
      (result) => result.phase === "success",
    ).length;
    const localRotationFailures = Object.values(state.changeIp.results).filter(
      (result) => result.phase === "error",
    ).length;
    const localRotationRunning = Object.values(state.changeIp.results).filter(
      (result) => result.phase === "running",
    ).length;
    const providerWriteAccepted = Object.values(state.changeIp.results).filter(
      (result) => getProxyProviderWriteState(result) === "accepted",
    ).length;
    const rollbackSignals = Object.values(state.changeIp.results).filter(
      (result) => getProxyProviderWriteState(result) === "rollback_flagged",
    ).length;
    const stickyActive = state.rows.filter(
      (row) => row.rotation.residencyStatus === "sticky_active",
    ).length;
    const stickyExpired = state.rows.filter(
      (row) => row.rotation.residencyStatus === "sticky_expired",
    ).length;
    const stickyMode = state.rows.filter((row) =>
      row.rotation.rotationMode.includes("sticky"),
    ).length;
    const providerAwareMode = state.rows.filter((row) =>
      row.rotation.rotationMode.includes("provider"),
    ).length;
    const coolingDown = Object.values(state.changeIp.results).filter(isProxyChangeCoolingDown).length;

    return {
      total: state.totalCount || state.rows.length,
      loaded: state.rows.length,
      visible: rows.length,
      healthy,
      attention: warning + failed,
      used,
      activeUsage,
      ready,
      highRisk,
      providers,
      sources,
      localRotationTracked,
      localRotationSuccess,
      localRotationFailures,
      localRotationRunning,
      providerWriteAccepted,
      rollbackSignals,
      stickyActive,
      stickyExpired,
      stickyMode,
      providerAwareMode,
      coolingDown,
    };
  }, [rows.length, state.changeIp.results, state.rows, state.totalCount]);

  const batchTargetCount =
    state.batchCheck.scope === "selected" ? state.selectedIds.length : rows.length;

  const allVisibleSelected =
    rows.length > 0 && rows.every((row) => state.selectedIds.includes(row.id));

  return {
    state,
    rows,
    selectedProxy,
    selectedProxyHidden,
    selectedProxyChangeIpFeedback,
    summary,
    batchTargetCount,
    allVisibleSelected,
    changeIpTargetLabel,
    recentChangeResults,
    healthOptions: HEALTH_OPTIONS,
    usageOptions: USAGE_OPTIONS,
    sortOptions: SORT_OPTIONS,
    sourceOptions,
    regionOptions,
    tagOptions,
    actions: {
      ...proxyActions,
      startBatchCheck: () => void startBatchCheck(),
      startChangeIp: () => void startChangeIp(),
      reload: () => void loadList(),
      reloadSelectedProxy: () => {
        if (state.selectedProxyId) {
          void loadDetail(state.selectedProxyId);
        }
      },
    },
  };
}
