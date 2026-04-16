import {
  PROFILE_COLUMN_DEFINITIONS,
  type FilterOption,
  type ProfileColumnDefinition,
  type ProfilePlatform,
  type ProfileRuntimeStatus,
  type ProxyHealth,
} from "./model";
import type { ProfilesWorkbenchState } from "./store";

const STATUS_ORDER: Record<ProfileRuntimeStatus, number> = {
  running: 0,
  warming: 1,
  idle: 2,
  error: 3,
};

const STATUS_LABELS: Record<ProfileRuntimeStatus, string> = {
  running: "Running",
  warming: "Warming",
  idle: "Idle",
  error: "Needs Attention",
};

const PROXY_LABELS: Record<ProxyHealth, string> = {
  healthy: "Healthy",
  warning: "Unstable",
  offline: "Offline",
};

function titleCase(value: string): string {
  return value
    .split(/[_\-\s]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function countByValue<T extends string>(
  values: T[],
  labelBuilder: (value: T) => string,
): FilterOption<T>[] {
  const counts = new Map<T, number>();

  values.forEach((value) => {
    counts.set(value, (counts.get(value) ?? 0) + 1);
  });

  return Array.from(counts.entries())
    .map(([value, count]) => ({
      value,
      label: labelBuilder(value),
      count,
    }))
    .sort((left, right) => right.count - left.count || left.label.localeCompare(right.label));
}

function countGroups(records: ProfilesWorkbenchState["profiles"]): FilterOption[] {
  const counts = new Map<string, number>();

  records.forEach((record) => {
    record.groupLabels.forEach((groupLabel) => {
      counts.set(groupLabel, (counts.get(groupLabel) ?? 0) + 1);
    });
  });

  return Array.from(counts.entries())
    .map(([value, count]) => ({
      value,
      label: value,
      count,
    }))
    .sort((left, right) => right.count - left.count || left.label.localeCompare(right.label));
}

function parseTimestamp(value: string | null): number {
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

export function normalizeSearchQuery(value: string): string {
  return value.trim().toLowerCase();
}

export function selectFilteredProfiles(state: ProfilesWorkbenchState) {
  const searchQuery = normalizeSearchQuery(state.searchQuery);

  const filtered = state.profiles.filter((record) => {
    if (
      state.groupIds.length > 0 &&
      !record.groupLabels.some((groupLabel) => state.groupIds.includes(groupLabel))
    ) {
      return false;
    }
    if (
      state.tagValues.length > 0 &&
      !record.tags.some((tagValue) => state.tagValues.includes(tagValue))
    ) {
      return false;
    }
    if (state.platformIds.length > 0 && !state.platformIds.includes(record.platformId)) {
      return false;
    }
    if (
      state.runtimeStatuses.length > 0 &&
      !state.runtimeStatuses.includes(record.runtimeStatus)
    ) {
      return false;
    }
    if (state.proxyHealth.length > 0 && !state.proxyHealth.includes(record.proxyHealth)) {
      return false;
    }
    if (!searchQuery) {
      return true;
    }

    const haystack = [
      record.code,
      record.name,
      record.storeId,
      record.groupLabels.join(" "),
      record.platformLabel,
      record.proxyLabel,
      record.regionLabel,
      record.fingerprintLabel,
      record.localeLabel,
      record.timezoneLabel,
      record.tags.join(" "),
    ]
      .join(" ")
      .toLowerCase();

    return haystack.includes(searchQuery);
  });

  filtered.sort((left, right) => {
    if (state.sortBy === "name") {
      return left.name.localeCompare(right.name);
    }

    if (state.sortBy === "status") {
      const orderDelta = STATUS_ORDER[left.runtimeStatus] - STATUS_ORDER[right.runtimeStatus];
      return orderDelta !== 0 ? orderDelta : left.name.localeCompare(right.name);
    }

    const timestampDelta =
      parseTimestamp(right.lastActiveAt ?? right.updatedAt) -
      parseTimestamp(left.lastActiveAt ?? left.updatedAt);
    return timestampDelta !== 0 ? timestampDelta : left.name.localeCompare(right.name);
  });

  return filtered;
}

export function selectVisibleColumns(state: ProfilesWorkbenchState): ProfileColumnDefinition[] {
  return PROFILE_COLUMN_DEFINITIONS.filter(
    (column) => !column.optional || state.columnVisibility[column.id],
  );
}

export function selectGroupOptions(state: ProfilesWorkbenchState): FilterOption[] {
  return countGroups(state.profiles);
}

export function selectTagOptions(state: ProfilesWorkbenchState): FilterOption[] {
  return countByValue(
    state.profiles.flatMap((record) => record.tags),
    (value) => value,
  );
}

export function selectPlatformOptions(state: ProfilesWorkbenchState): FilterOption<ProfilePlatform>[] {
  return countByValue(
    state.profiles.map((record) => record.platformId),
    (value) => titleCase(value),
  );
}

export function selectRuntimeStatusOptions(
  state: ProfilesWorkbenchState,
): FilterOption<ProfileRuntimeStatus>[] {
  return countByValue(
    state.profiles.map((record) => record.runtimeStatus),
    (value) => STATUS_LABELS[value],
  );
}

export function selectProxyHealthOptions(
  state: ProfilesWorkbenchState,
): FilterOption<ProxyHealth>[] {
  return countByValue(
    state.profiles.map((record) => record.proxyHealth),
    (value) => PROXY_LABELS[value],
  );
}

export function selectWorkbenchSummary(
  state: ProfilesWorkbenchState,
  filteredProfiles: ReturnType<typeof selectFilteredProfiles>,
) {
  const selectedVisibleCount = filteredProfiles.filter((record) =>
    state.selectedIds.includes(record.id),
  ).length;

  return {
    loadedCount: state.profiles.length,
    totalCount: state.list.totalCount || state.profiles.length,
    visibleCount: filteredProfiles.length,
    selectedCount: state.selectedIds.length,
    selectedVisibleCount,
    runningCount: state.profiles.filter((record) => record.runtimeStatus === "running").length,
    healthyProxyCount: state.profiles.filter((record) => record.proxyHealth === "healthy").length,
    activeFilterCount:
      state.groupIds.length +
      state.tagValues.length +
      state.platformIds.length +
      state.runtimeStatuses.length +
      state.proxyHealth.length +
      (state.searchQuery ? 1 : 0),
  };
}

export function selectSelectionState(
  state: ProfilesWorkbenchState,
  filteredProfiles: ReturnType<typeof selectFilteredProfiles>,
) {
  const visibleIds = filteredProfiles.map((record) => record.id);
  const selectedVisibleCount = visibleIds.filter((profileId) =>
    state.selectedIds.includes(profileId),
  ).length;

  return {
    selectedVisibleCount,
    allVisibleSelected: visibleIds.length > 0 && selectedVisibleCount === visibleIds.length,
    partiallySelected:
      selectedVisibleCount > 0 && selectedVisibleCount < visibleIds.length,
  };
}
