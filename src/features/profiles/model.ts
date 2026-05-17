export type ProfileRuntimeStatus = "running" | "warming" | "idle" | "error";

export type ProfilePlatform = string;

export type ProxyHealth = "healthy" | "warning" | "offline";

export type ProfilesDensity = "compact" | "comfortable";

export type ProfilesSortKey = "lastActive" | "name" | "status";

export type ProfilesBatchAction = "open" | "start" | "stop" | "checkProxy" | "sync";

export type ProfileColumnId =
  | "profile"
  | "group"
  | "tags"
  | "browser"
  | "proxy"
  | "region"
  | "fingerprint"
  | "runtime";

export type ProfileDrawerTab = "overview" | "proxy" | "runtime" | "logs";

export type ProfileDataSource = "desktop" | "fallback" | "mixed";

export interface ProfileRow {
  id: string;
  code: string;
  name: string;
  storeId: string;
  platformId: ProfilePlatform;
  platformLabel: string;
  statusLabel: string;
  groupId: string;
  groupLabel: string;
  groupLabels: string[];
  tags: string[];
  browserLabel: string;
  proxyLabel: string;
  proxyProvider: string | null;
  proxyHealth: ProxyHealth;
  regionLabel: string;
  localeLabel: string;
  timezoneLabel: string;
  fingerprintLabel: string;
  runtimeStatus: ProfileRuntimeStatus;
  continuityScore: number | null;
  activeSessionCount: number;
  pendingActionCount: number;
  lastActiveAt: string | null;
  lastOpenedAt: string | null;
  lastSyncedAt: string | null;
  updatedAt: string | null;
}

export interface ProfileTaskSummary {
  id: string;
  title: string;
  status: string;
  createdAt: string | null;
  finishedAt: string | null;
}

export interface ProfileLogSummary {
  id: string;
  level: string;
  message: string;
  createdAt: string | null;
}

export interface FingerprintSectionSummary {
  name: string;
  declaredFields: string[];
  declaredCount: number;
}

export interface FingerprintConsistencyCheckSummary {
  name: string;
  status: string;
  edgeType: string | null;
  reason: string;
}

export interface FingerprintConsistencySummary {
  overallStatus: string;
  coherenceScore: number | null;
  riskReasons: string[];
  hardFailureCount: number | null;
  softWarningCount: number | null;
  checks: FingerprintConsistencyCheckSummary[];
}

export interface FingerprintConsumptionSummary {
  declaredFields: string[];
  resolvedFields: string[];
  appliedFields: string[];
  ignoredFields: string[];
  declaredCount: number | null;
  resolvedCount: number | null;
  appliedCount: number | null;
  ignoredCount: number | null;
  consumptionStatus: string | null;
  consumptionVersion: string | null;
  partialSupportWarning: string | null;
}

export interface ProfileFingerprintSummary {
  profileId: string;
  familyId: string | null;
  familyVariant: string | null;
  schemaKind: string | null;
  declaredControlFields: string[];
  declaredControlCount: number | null;
  declaredSections: FingerprintSectionSummary[];
  supportedRuntimeFields: string[];
  unsupportedControlFields: string[];
  consistency: FingerprintConsistencySummary | null;
  consumption: FingerprintConsumptionSummary | null;
  validationOk: boolean | null;
  validationIssues: string[];
  source: "api" | "desktop" | "fallback";
}

export interface ProfileDetail {
  profileId: string;
  profile: ProfileRow;
  credentialRef: string | null;
  fingerprintProfileLabel: string;
  behaviorProfileLabel: string | null;
  networkPolicyLabel: string;
  continuityPolicyLabel: string;
  platformTemplateLabel: string | null;
  storePlatformOverrideLabel: string | null;
  proxyProvider: string | null;
  proxyRegion: string | null;
  proxyCountry: string | null;
  proxyResolutionStatus: string | null;
  proxyUsageMode: string | null;
  proxySessionKey?: string | null;
  proxyRequestedProvider?: string | null;
  proxyRequestedRegion?: string | null;
  proxyResidencyStatus?: string | null;
  proxyRotationMode?: string | null;
  proxyStickyTtlSeconds?: number | null;
  proxyExpiresAt?: string | null;
  proxyLastVerifiedAt: string | null;
  proxyLastUsedAt: string | null;
  continuityStatus: string | null;
  continuityScore: number | null;
  activeSessionCount: number;
  loginRiskCount: number;
  lastEventType: string | null;
  fingerprintSummary?: ProfileFingerprintSummary | null;
  recentTasks: ProfileTaskSummary[];
  recentLogs: ProfileLogSummary[];
  source: ProfileDataSource;
}

export interface FilterOption<T extends string = string> {
  value: T;
  label: string;
  count: number;
}

export interface ProfileColumnDefinition {
  id: ProfileColumnId;
  label: string;
  width: string;
  optional: boolean;
}

export interface ProfileWizardIntent {
  mode: "create" | "edit";
  profileId?: string;
}

export const PROFILE_COLUMN_DEFINITIONS: ProfileColumnDefinition[] = [
  { id: "profile", label: "Profile", width: "minmax(280px, 2.2fr)", optional: false },
  { id: "group", label: "Group", width: "minmax(132px, 1fr)", optional: true },
  { id: "tags", label: "Tags", width: "minmax(180px, 1.2fr)", optional: true },
  { id: "browser", label: "Browser", width: "minmax(160px, 1fr)", optional: true },
  { id: "proxy", label: "Proxy", width: "minmax(180px, 1.15fr)", optional: true },
  { id: "region", label: "Region", width: "minmax(130px, 0.9fr)", optional: true },
  { id: "fingerprint", label: "Fingerprint", width: "minmax(160px, 1fr)", optional: true },
  { id: "runtime", label: "Runtime", width: "minmax(150px, 0.95fr)", optional: true },
];

export function createInitialColumnVisibility(): Record<ProfileColumnId, boolean> {
  return PROFILE_COLUMN_DEFINITIONS.reduce<Record<ProfileColumnId, boolean>>(
    (visibility, column) => {
      visibility[column.id] = true;
      return visibility;
    },
    {
      profile: true,
      group: true,
      tags: true,
      browser: true,
      proxy: true,
      region: true,
      fingerprint: true,
      runtime: true,
    },
  );
}
