import {
  checkProfileProxies,
  DesktopServiceError,
  listProfilePage,
  openProfiles,
  readLocalApiSnapshot,
  readProfileDetail,
  startProfiles,
  stopProfiles,
  syncProfiles,
} from "../../services/desktop";
import type {
  DesktopEntityReference,
  DesktopProfileFingerprintSummary,
  DesktopLogItem,
  DesktopProfileBatchActionResult,
  DesktopProfileDetail,
  DesktopProfilePage,
  DesktopProfileRow,
  DesktopProfileBatchActionRequest,
  DesktopTaskItem,
} from "../../types/desktop";
import type {
  ProfileDataSource,
  ProfileDetail,
  ProfileFingerprintSummary,
  ProfileRow,
  ProfileRuntimeStatus,
  ProfilesBatchAction,
  ProxyHealth,
} from "./model";
import { buildFallbackProfileDetail, getFallbackProfileRows } from "./mock";

export interface ProfileListSnapshot {
  rows: ProfileRow[];
  totalCount: number;
  source: ProfileDataSource;
}

const PROFILE_BATCH_RUNNERS: Record<
  ProfilesBatchAction,
  (request: DesktopProfileBatchActionRequest) => Promise<DesktopProfileBatchActionResult>
> = {
  open: openProfiles,
  start: startProfiles,
  stop: stopProfiles,
  checkProxy: checkProfileProxies,
  sync: syncProfiles,
};

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

function buildRegionLabel(country: string | null | undefined, region: string | null | undefined): string {
  if (country && region) {
    return `${country} / ${region}`;
  }
  if (country) {
    return country;
  }
  if (region) {
    return region;
  }
  return "Pending region";
}

function mapRuntimeStatus(status: string): ProfileRuntimeStatus {
  switch (status) {
    case "running":
      return "running";
    case "starting":
    case "syncing":
      return "warming";
    case "error":
      return "error";
    default:
      return "idle";
  }
}

function mapProxyHealth(row: DesktopProfileRow): ProxyHealth {
  const healthStatus = row.health?.status?.toLowerCase() ?? "";
  const resolutionStatus = row.proxy?.resolutionStatus?.toLowerCase() ?? "";

  if (healthStatus.includes("error") || resolutionStatus.includes("fail")) {
    return "offline";
  }
  if (resolutionStatus.includes("pending") || resolutionStatus.includes("warn")) {
    return "warning";
  }
  if (row.proxy) {
    return "healthy";
  }
  return "offline";
}

function mapProfileRow(row: DesktopProfileRow): ProfileRow {
  const groupLabels = row.groupLabels.length > 0 ? row.groupLabels : ["Ungrouped"];
  const proxyLabel = row.proxy
    ? `${row.proxy.provider ?? "Direct"} | ${buildRegionLabel(row.proxy.country, row.proxy.region)}`
    : "No proxy linked";

  return {
    id: row.id,
    code: row.storeId,
    name: row.label,
    storeId: row.storeId,
    platformId: row.platformId,
    platformLabel: titleCase(row.platformId),
    statusLabel: titleCase(row.status),
    groupId: groupLabels[0].toLowerCase().replace(/\s+/g, "-"),
    groupLabel: groupLabels[0],
    groupLabels,
    tags: [...row.tags],
    browserLabel: `${row.deviceFamily} / ${row.locale}`,
    proxyLabel,
    proxyProvider: row.proxy?.provider ?? null,
    proxyHealth: mapProxyHealth(row),
    regionLabel: buildRegionLabel(row.countryAnchor, row.regionAnchor),
    localeLabel: row.locale,
    timezoneLabel: row.timezone,
    fingerprintLabel: row.fingerprintProfileId,
    runtimeStatus: mapRuntimeStatus(row.runtime.status),
    continuityScore: row.health?.continuityScore ?? null,
    activeSessionCount: row.runtime.activeSessionCount,
    pendingActionCount: row.runtime.pendingActionCount,
    lastActiveAt: row.runtime.lastTaskAt,
    lastOpenedAt: row.runtime.lastOpenedAt,
    lastSyncedAt: row.runtime.lastSyncedAt,
    updatedAt: row.updatedAt,
  };
}

function mapEntityLabel(entity: DesktopEntityReference | null): string | null {
  if (!entity) {
    return null;
  }
  return entity.name ?? entity.id;
}

function mapTaskSummary(task: DesktopTaskItem) {
  return {
    id: task.id,
    title: task.title ?? task.kind,
    status: task.status,
    createdAt: task.createdAt,
    finishedAt: task.finishedAt,
  };
}

function mapLogSummary(log: DesktopLogItem) {
  return {
    id: log.id,
    level: log.level,
    message: log.message,
    createdAt: log.createdAt,
  };
}

function mapFingerprintSummaryFromDesktop(
  summary: DesktopProfileFingerprintSummary | null,
  fallbackProfileId: string,
): ProfileFingerprintSummary | null {
  if (!summary) {
    return null;
  }

  return {
    profileId: summary.profileId || fallbackProfileId,
    familyId: summary.familyId,
    familyVariant: summary.familyVariant,
    schemaKind: summary.schemaKind,
    declaredControlFields: [...summary.declaredControlFields],
    declaredControlCount: summary.declaredControlCount,
    declaredSections: summary.declaredSections.map((section) => ({
      name: section.name,
      declaredFields: [...section.declaredFields],
      declaredCount: section.declaredCount,
    })),
    supportedRuntimeFields: [...summary.runtimeSupport.supportedFields],
    unsupportedControlFields: [...summary.runtimeSupport.unsupportedFields],
    consistency: {
      overallStatus: summary.consistency.status,
      coherenceScore: summary.consistency.coherenceScore,
      riskReasons: [...summary.consistency.riskReasons],
      hardFailureCount: summary.consistency.hardFailureCount,
      softWarningCount: summary.consistency.softWarningCount,
      checks: [],
    },
    consumption: {
      declaredFields: [],
      resolvedFields: [],
      appliedFields: [],
      ignoredFields: [],
      declaredCount: summary.consumption.declaredCount,
      resolvedCount: summary.consumption.resolvedCount,
      appliedCount: summary.consumption.appliedCount,
      ignoredCount: summary.consumption.ignoredCount,
      consumptionStatus: summary.consumption.status,
      consumptionVersion: summary.consumption.version,
      partialSupportWarning: summary.consumption.partialSupportWarning,
    },
    validationOk: summary.validationOk,
    validationIssues: [...summary.validationIssues],
    source: "desktop",
  };
}

function mapProfileDetail(detail: DesktopProfileDetail): ProfileDetail {
  const row = mapProfileRow(detail.profile);

  return {
    profileId: row.id,
    profile: row,
    credentialRef: detail.profile.credentialRef,
    fingerprintProfileLabel: mapEntityLabel(detail.fingerprintProfile) ?? row.fingerprintLabel,
    behaviorProfileLabel: mapEntityLabel(detail.behaviorProfile),
    networkPolicyLabel: mapEntityLabel(detail.networkPolicy) ?? detail.profile.networkPolicyId,
    continuityPolicyLabel:
      mapEntityLabel(detail.continuityPolicy) ?? detail.profile.continuityPolicyId,
    platformTemplateLabel: mapEntityLabel(detail.platformTemplate),
    storePlatformOverrideLabel: mapEntityLabel(detail.storePlatformOverride),
    proxyProvider: detail.profile.proxy?.provider ?? null,
    proxyRegion: detail.profile.proxy?.region ?? null,
    proxyCountry: detail.profile.proxy?.country ?? null,
    proxyResolutionStatus: detail.profile.proxy?.resolutionStatus ?? null,
    proxyUsageMode: detail.profile.proxy?.usageMode ?? null,
    proxySessionKey: detail.profile.proxy?.sessionKey ?? null,
    proxyRequestedProvider: detail.profile.proxy?.requestedProvider ?? null,
    proxyRequestedRegion: detail.profile.proxy?.requestedRegion ?? null,
    proxyResidencyStatus: detail.profile.proxy?.residencyStatus ?? null,
    proxyRotationMode: detail.profile.proxy?.rotationMode ?? null,
    proxyStickyTtlSeconds: detail.profile.proxy?.stickyTtlSeconds ?? null,
    proxyExpiresAt: detail.profile.proxy?.expiresAt ?? null,
    proxyLastVerifiedAt: detail.profile.proxy?.lastVerifiedAt ?? null,
    proxyLastUsedAt: detail.profile.proxy?.lastUsedAt ?? null,
    continuityStatus: detail.profile.health?.status ?? null,
    continuityScore: detail.profile.health?.continuityScore ?? null,
    activeSessionCount: detail.profile.health?.activeSessionCount ?? row.activeSessionCount,
    loginRiskCount: detail.profile.health?.loginRiskCount ?? 0,
    lastEventType: detail.profile.health?.lastEventType ?? null,
    fingerprintSummary: mapFingerprintSummaryFromDesktop(
      detail.fingerprintSummary,
      detail.profile.fingerprintProfileId,
    ),
    recentTasks: detail.recentTasks.map(mapTaskSummary),
    recentLogs: detail.recentLogs.map(mapLogSummary),
    source: "desktop",
  };
}

function asString(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value : null;
}

function asNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function asBoolean(value: unknown): boolean | null {
  return typeof value === "boolean" ? value : null;
}

function asStringArray(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter(Boolean);
}

function asObject(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function mapValidationIssues(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }
  return value
    .map((issue) => {
      const issueObj = asObject(issue);
      if (!issueObj) {
        return null;
      }
      const field = asString(issueObj.field);
      const level = asString(issueObj.level);
      const message = asString(issueObj.message);
      if (!field && !level && !message) {
        return null;
      }
      return [field, level, message].filter(Boolean).join(" | ");
    })
    .filter((item): item is string => Boolean(item));
}

function mapFingerprintSummaryFromApi(
  payload: unknown,
  fingerprintProfileId: string,
): ProfileFingerprintSummary | null {
  const root = asObject(payload);
  if (!root) {
    return null;
  }

  const declaredSections = Array.isArray(root.declared_sections)
    ? root.declared_sections
        .map((section) => {
          const sectionObj = asObject(section);
          if (!sectionObj) {
            return null;
          }
          const name = asString(sectionObj.name);
          if (!name) {
            return null;
          }
          return {
            name,
            declaredFields: asStringArray(sectionObj.declared_fields),
            declaredCount: asNumber(sectionObj.declared_count) ?? 0,
          };
        })
        .filter((item): item is NonNullable<typeof item> => Boolean(item))
    : [];

  const consistencyObj =
    asObject(root.consistency_assessment) ?? asObject(root.consistencyAssessment);
  const consumptionObj =
    asObject(root.consumption_explain) ?? asObject(root.consumptionExplain);
  const declaredControlFields = asStringArray(root.declared_control_fields);
  const supportedRuntimeFields = asStringArray(root.supported_runtime_fields);
  const unsupportedControlFields = asStringArray(root.unsupported_control_fields);

  return {
    profileId: asString(root.id) ?? fingerprintProfileId,
    familyId: asString(root.family_id) ?? asString(root.familyId),
    familyVariant: asString(root.family_variant) ?? asString(root.familyVariant),
    schemaKind: asString(root.schema_kind) ?? asString(root.schemaKind),
    declaredControlFields:
      declaredControlFields.length > 0
        ? declaredControlFields
        : asStringArray(root.declaredControlFields),
    declaredControlCount:
      asNumber(root.declared_control_count) ?? asNumber(root.declaredControlCount),
    declaredSections,
    supportedRuntimeFields:
      supportedRuntimeFields.length > 0
        ? supportedRuntimeFields
        : asStringArray(root.supportedRuntimeFields),
    unsupportedControlFields:
      unsupportedControlFields.length > 0
        ? unsupportedControlFields
        : asStringArray(root.unsupportedControlFields),
    consistency: consistencyObj
      ? {
          overallStatus:
            asString(consistencyObj.overall_status) ??
            asString(consistencyObj.overallStatus) ??
            "unknown",
          coherenceScore:
            asNumber(consistencyObj.coherence_score) ?? asNumber(consistencyObj.coherenceScore),
          riskReasons: (() => {
            const fromSnake = asStringArray(consistencyObj.risk_reasons);
            return fromSnake.length > 0 ? fromSnake : asStringArray(consistencyObj.riskReasons);
          })(),
          hardFailureCount:
            asNumber(consistencyObj.hard_failure_count) ??
            asNumber(consistencyObj.hardFailureCount),
          softWarningCount:
            asNumber(consistencyObj.soft_warning_count) ??
            asNumber(consistencyObj.softWarningCount),
          checks: Array.isArray(consistencyObj.checks)
            ? consistencyObj.checks
                .map((check) => {
                  const checkObj = asObject(check);
                  if (!checkObj) {
                    return null;
                  }
                  const name = asString(checkObj.name);
                  const status = asString(checkObj.status);
                  const reason = asString(checkObj.reason);
                  if (!name || !status || !reason) {
                    return null;
                  }
                  return {
                    name,
                    status,
                    edgeType: asString(checkObj.edge_type) ?? asString(checkObj.edgeType),
                    reason,
                  };
                })
                .filter((item): item is NonNullable<typeof item> => Boolean(item))
            : [],
        }
      : null,
    consumption: consumptionObj
      ? {
          declaredFields: (() => {
            const fromSnake = asStringArray(consumptionObj.declared_fields);
            return fromSnake.length > 0 ? fromSnake : asStringArray(consumptionObj.declaredFields);
          })(),
          resolvedFields: (() => {
            const fromSnake = asStringArray(consumptionObj.resolved_fields);
            return fromSnake.length > 0 ? fromSnake : asStringArray(consumptionObj.resolvedFields);
          })(),
          appliedFields: (() => {
            const fromSnake = asStringArray(consumptionObj.applied_fields);
            return fromSnake.length > 0 ? fromSnake : asStringArray(consumptionObj.appliedFields);
          })(),
          ignoredFields: (() => {
            const fromSnake = asStringArray(consumptionObj.ignored_fields);
            return fromSnake.length > 0 ? fromSnake : asStringArray(consumptionObj.ignoredFields);
          })(),
          declaredCount:
            asNumber(consumptionObj.declared_count) ?? asNumber(consumptionObj.declaredCount),
          resolvedCount:
            asNumber(consumptionObj.resolved_count) ?? asNumber(consumptionObj.resolvedCount),
          appliedCount:
            asNumber(consumptionObj.applied_count) ?? asNumber(consumptionObj.appliedCount),
          ignoredCount:
            asNumber(consumptionObj.ignored_count) ?? asNumber(consumptionObj.ignoredCount),
          consumptionStatus:
            asString(consumptionObj.consumption_status) ??
            asString(consumptionObj.consumptionStatus),
          consumptionVersion:
            asString(consumptionObj.consumption_version) ??
            asString(consumptionObj.consumptionVersion),
          partialSupportWarning:
            asString(consumptionObj.partial_support_warning) ??
            asString(consumptionObj.partialSupportWarning),
        }
      : null,
    validationOk: asBoolean(root.validation_ok) ?? asBoolean(root.validationOk),
    validationIssues: mapValidationIssues(root.validation_issues ?? root.validationIssues),
    source: "api",
  };
}

async function tryLoadFingerprintSummary(
  fingerprintProfileId: string,
): Promise<ProfileFingerprintSummary | null> {
  if (!fingerprintProfileId.trim()) {
    return null;
  }

  try {
    const localApi = await readLocalApiSnapshot();
    const baseUrl = localApi.baseUrl.trim().replace(/\/+$/, "");
    if (!baseUrl) {
      return null;
    }

    const response = await fetch(
      `${baseUrl}/fingerprint-profiles/${encodeURIComponent(fingerprintProfileId)}`,
      {
        method: "GET",
      },
    );
    if (!response.ok) {
      return null;
    }

    const payload = (await response.json()) as unknown;
    return mapFingerprintSummaryFromApi(payload, fingerprintProfileId);
  } catch {
    return null;
  }
}

function isNotReadyError(error: unknown): error is DesktopServiceError {
  return error instanceof DesktopServiceError && error.code === "desktop_command_not_ready";
}

function buildFallbackPage(): ProfileListSnapshot {
  const rows = getFallbackProfileRows();
  return {
    rows,
    totalCount: rows.length,
    source: "fallback",
  };
}

export async function loadProfilesSnapshot(): Promise<ProfileListSnapshot> {
  try {
    const page: DesktopProfilePage = await listProfilePage({
      page: 1,
      pageSize: 500,
    });

    return {
      rows: page.items.map(mapProfileRow),
      totalCount: page.total,
      source: "desktop",
    };
  } catch (error) {
    if (isNotReadyError(error)) {
      return buildFallbackPage();
    }
    throw error;
  }
}

export async function loadProfileDetailSnapshot(
  profileId: string,
  knownRows: ProfileRow[],
): Promise<ProfileDetail> {
  try {
    const detail = await readProfileDetail(profileId);
    const mapped = mapProfileDetail(detail);
    const fingerprintSummary =
      mapped.fingerprintSummary ??
      (await tryLoadFingerprintSummary(detail.profile.fingerprintProfileId));
    return {
      ...mapped,
      fingerprintSummary,
    };
  } catch (error) {
    if (!isNotReadyError(error)) {
      throw error;
    }

    const knownRow = knownRows.find((row) => row.id === profileId);
    const fallback =
      buildFallbackProfileDetail(profileId) ??
      (knownRow
        ? {
            profileId: knownRow.id,
            profile: knownRow,
            credentialRef: null,
            fingerprintProfileLabel: knownRow.fingerprintLabel,
            behaviorProfileLabel: null,
            networkPolicyLabel: "network-default",
            continuityPolicyLabel: "continuity-default",
            platformTemplateLabel: null,
            storePlatformOverrideLabel: null,
            proxyProvider: knownRow.proxyProvider,
            proxyRegion: knownRow.regionLabel,
            proxyCountry: knownRow.regionLabel.split("/")[0]?.trim() ?? null,
            proxyResolutionStatus:
              knownRow.proxyHealth === "healthy" ? "resolved" : "pending",
            proxyUsageMode: "sticky",
            proxySessionKey: null,
            proxyRequestedProvider: knownRow.proxyProvider,
            proxyRequestedRegion: knownRow.regionLabel,
            proxyResidencyStatus: "fallback_pending",
            proxyRotationMode: "sticky_refresh",
            proxyStickyTtlSeconds: null,
            proxyExpiresAt: null,
            proxyLastVerifiedAt: knownRow.updatedAt,
            proxyLastUsedAt: knownRow.lastActiveAt,
            continuityStatus: knownRow.runtimeStatus,
            continuityScore: knownRow.continuityScore,
            activeSessionCount: knownRow.activeSessionCount,
            loginRiskCount: 0,
            lastEventType: null,
            fingerprintSummary: null,
            recentTasks: [],
            recentLogs: [],
            source: "fallback" as const,
          }
        : null);

    if (!fallback) {
      throw new DesktopServiceError(
        `Profile detail fallback is missing for ${profileId}.`,
        "profile_detail_missing",
      );
    }

    return fallback;
  }
}

export async function runProfilesBatchAction(
  action: ProfilesBatchAction,
  profileIds: string[],
): Promise<DesktopProfileBatchActionResult> {
  const normalizedProfileIds = Array.from(
    new Set(profileIds.map((profileId) => profileId.trim()).filter(Boolean)),
  );

  return PROFILE_BATCH_RUNNERS[action]({
    profileIds: normalizedProfileIds,
  });
}
