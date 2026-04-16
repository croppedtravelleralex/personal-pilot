export type DesktopTaskStatus =
  | "pending"
  | "queued"
  | "running"
  | "succeeded"
  | "failed"
  | "timed_out"
  | "cancelled"
  | string;

export interface DesktopTaskCounts {
  total: number;
  queued: number;
  running: number;
  succeeded: number;
  failed: number;
  timedOut: number;
  cancelled: number;
}

export interface DesktopWorkerSnapshot {
  runnerKind: string;
  workerCount: number;
  reclaimAfterSeconds: number | null;
  heartbeatIntervalSeconds: number;
  claimRetryLimit: number;
  idleBackoffMinMs: number;
  idleBackoffMaxMs: number;
}

export interface DesktopTaskItem {
  id: string;
  kind: string;
  status: DesktopTaskStatus;
  priority: number;
  personaId: string | null;
  platformId: string | null;
  manualGateRequestId: string | null;
  isBrowserTask: boolean;
  title: string | null;
  finalUrl: string | null;
  contentPreview: string | null;
  contentKind: string | null;
  contentReady: boolean | null;
  errorMessage: string | null;
  createdAt: string;
  startedAt: string | null;
  finishedAt: string | null;
}

export interface DesktopStatusSnapshot {
  service: string;
  runtimeMode: string;
  queueLen: number;
  counts: DesktopTaskCounts;
  worker: DesktopWorkerSnapshot;
  latestTasks: DesktopTaskItem[];
  latestBrowserTasks: DesktopTaskItem[];
  updatedAt: string;
}

export interface DesktopTaskQuery {
  page?: number;
  pageSize?: number;
  statusFilter?: string;
  search?: string;
}

export interface DesktopTaskPage {
  page: number;
  pageSize: number;
  total: number;
  items: DesktopTaskItem[];
}

export interface DesktopLogItem {
  id: string;
  taskId: string;
  runId: string | null;
  level: string;
  message: string;
  createdAt: string;
}

export interface DesktopLogQuery {
  page?: number;
  pageSize?: number;
  taskIdFilter?: string;
  levelFilter?: string;
  search?: string;
}

export interface DesktopLogPage {
  page: number;
  pageSize: number;
  total: number;
  items: DesktopLogItem[];
}

export type DesktopDirectoryTarget =
  | "projectRoot"
  | "dataDir"
  | "reportsDir"
  | "logsDir"
  | "packagedDataDir"
  | "packagedReportsDir"
  | "packagedLogsDir";

export type DesktopRuntimeStatusValue =
  | "stopped"
  | "managed_running"
  | "managed_stopped"
  | "external_running";

export interface DesktopRuntimeStatus {
  status: DesktopRuntimeStatusValue | string;
  running: boolean;
  managed: boolean;
  pid: number | null;
  startedAt: string | null;
  healthUrl: string;
  apiReachable: boolean;
  binaryPath: string | null;
  logDir: string | null;
  stdoutPath: string | null;
  stderrPath: string | null;
  lastExitCode: number | null;
}

export interface DesktopSettingsSnapshot {
  projectRoot: string;
  databaseUrl: string;
  databasePath: string;
  dataDir: string;
  reportsDir: string;
  logsDir: string;
  packagedDataDir: string;
  packagedReportsDir: string;
  packagedLogsDir: string;
  runnerKind: string;
  workerCount: number;
  reclaimAfterSeconds: number | null;
  heartbeatIntervalSeconds: number;
  claimRetryLimit: number;
  idleBackoffMinMs: number;
  idleBackoffMaxMs: number;
}

export interface DesktopRuntimeSettingsDraft {
  runnerKind: string;
  workerCount: number;
  reclaimAfterSeconds: number | null;
  heartbeatIntervalSeconds: number;
  claimRetryLimit: number;
  idleBackoffMinMs: number;
  idleBackoffMaxMs: number;
}

export interface DesktopSettingsMutationResult {
  action: "applied" | "restored";
  snapshot: DesktopSettingsSnapshot;
  updatedAt: string;
  message: string;
}

export interface DesktopLocalApiSnapshot {
  host: string;
  port: number;
  baseUrl: string;
  healthUrl: string;
  configPath: string;
  bindMode: string;
  startMode: string;
  authMode: string;
  requestLoggingEnabled: boolean;
  requireLocalToken: boolean;
  readOnlySafeMode: boolean;
  maxConcurrentSessions: number;
  updatedAt: string;
}

export interface DesktopLocalApiSettingsDraft {
  host: string;
  port: number;
  startMode: string;
  authMode: string;
  requestLoggingEnabled: boolean;
  requireLocalToken: boolean;
  readOnlySafeMode: boolean;
  maxConcurrentSessions: number;
}

export interface DesktopLocalApiMutationResult {
  action: "applied" | "restored";
  snapshot: DesktopLocalApiSnapshot;
  updatedAt: string;
  message: string;
}

export interface DesktopBrowserEnvironmentPolicySnapshot {
  browserFamily: string;
  launchStrategy: string;
  profileStorageMode: string;
  environmentRoot: string;
  profileWorkspaceDir: string;
  downloadsDir: string;
  extensionsDir: string;
  bookmarksCatalogPath: string;
  profileArchiveDir: string;
  defaultViewportPreset: string;
  keepUserDataBetweenRuns: boolean;
  allowExtensions: boolean;
  allowBookmarksSeed: boolean;
  allowProfileArchiveImport: boolean;
  headlessAllowed: boolean;
  updatedAt: string;
}

export interface DesktopBrowserEnvironmentPolicyDraft {
  browserFamily: string;
  launchStrategy: string;
  profileStorageMode: string;
  defaultViewportPreset: string;
  keepUserDataBetweenRuns: boolean;
  allowExtensions: boolean;
  allowBookmarksSeed: boolean;
  allowProfileArchiveImport: boolean;
  headlessAllowed: boolean;
}

export interface DesktopBrowserEnvironmentPolicyMutationResult {
  action: "applied" | "restored";
  snapshot: DesktopBrowserEnvironmentPolicySnapshot;
  updatedAt: string;
  message: string;
}

export type DesktopLocalAssetEntryId =
  | "runtimePolicy"
  | "localApiConfig"
  | "browserEnvironmentPolicy"
  | "browserProfilesDir"
  | "browserDownloadsDir"
  | "browserExtensionsDir"
  | "bookmarkCatalog"
  | "profileArchiveDir"
  | "importQueueDir"
  | "exportQueueDir";

export type DesktopLocalAssetEntryKind =
  | "config_file"
  | "directory"
  | "catalog_file";

export interface DesktopLocalAssetEntry {
  id: DesktopLocalAssetEntryId;
  label: string;
  kind: DesktopLocalAssetEntryKind | string;
  path: string;
  status: string;
  description: string;
}

export interface DesktopLocalAssetWorkspaceSnapshot {
  workspaceRoot: string;
  controlRoot: string;
  browserEnvironmentRoot: string;
  importQueueDir: string;
  exportQueueDir: string;
  localApiConfigPath: string;
  runtimePolicyPath: string;
  browserEnvironmentPolicyPath: string;
  entries: DesktopLocalAssetEntry[];
  updatedAt: string;
}

export interface DesktopImportExportFieldDefinition {
  key: string;
  label: string;
  required: boolean;
  description: string;
  example: string | null;
}

export interface DesktopImportExportSkeleton {
  mode: string;
  importManifestPath: string;
  exportManifestPath: string;
  importQueueDir: string;
  exportQueueDir: string;
  supportedImportKinds: string[];
  supportedExportKinds: string[];
  importFields: DesktopImportExportFieldDefinition[];
  exportFields: DesktopImportExportFieldDefinition[];
  notes: string[];
  updatedAt: string;
}

export type DesktopJsonValue =
  | string
  | number
  | boolean
  | null
  | DesktopJsonValue[]
  | { [key: string]: DesktopJsonValue };

export type DesktopProfileStatus = "active" | "draft" | "disabled" | string;

export type DesktopProfileRuntimeStatus =
  | "idle"
  | "starting"
  | "running"
  | "stopped"
  | "syncing"
  | "error"
  | string;

export type DesktopProxyStatus =
  | "active"
  | "candidate"
  | "candidate_rejected"
  | "cooldown"
  | "disabled"
  | string;

export type DesktopTemplateStatus = "active" | "draft" | "disabled" | string;

export type DesktopTemplateReadinessLevel =
  | "draft"
  | "baseline"
  | "sample_ready"
  | "ready"
  | string;

export type DesktopTemplateSource =
  | "platform_template"
  | "store_platform_override"
  | "recorder_template"
  | string;

export type DesktopRecorderSessionStatus =
  | "idle"
  | "recording"
  | "paused"
  | "stopped"
  | string;

export type DesktopRecorderActionType =
  | "visit"
  | "click"
  | "input"
  | "select"
  | "scroll"
  | "wait"
  | "tab"
  | string;

export type DesktopRecorderValueSource =
  | "literal"
  | "variable"
  | "secret"
  | "derived"
  | string;

export type DesktopSyncWindowStatus =
  | "ready"
  | "focused"
  | "busy"
  | "minimized"
  | "missing"
  | string;

export type DesktopSyncLayoutMode =
  | "grid"
  | "overlap"
  | "uniform_size"
  | string;

export interface DesktopEntityReference {
  id: string;
  name: string | null;
  status: string | null;
  version: number | null;
}

export interface DesktopProfileRuntimeSummary {
  status: DesktopProfileRuntimeStatus;
  currentTaskId: string | null;
  lastTaskId: string | null;
  lastTaskStatus: DesktopTaskStatus | null;
  lastTaskAt: string | null;
  lastOpenedAt: string | null;
  lastSyncedAt: string | null;
  activeSessionCount: number;
  pendingActionCount: number;
}

export interface DesktopProfileProxySummary {
  proxyId: string | null;
  provider: string | null;
  region: string | null;
  country: string | null;
  resolutionStatus: string | null;
  usageMode: string | null;
  sessionKey: string | null;
  requestedProvider: string | null;
  requestedRegion: string | null;
  residencyStatus: string | null;
  rotationMode: string | null;
  stickyTtlSeconds: number | null;
  expiresAt: string | null;
  lastVerifiedAt: string | null;
  lastUsedAt: string | null;
}

export interface DesktopProfileHealthSummary {
  status: string;
  continuityScore: number;
  activeSessionCount: number;
  loginRiskCount: number;
  lastEventType: string | null;
  lastTaskAt: string | null;
  snapshotAt: string;
}

export interface DesktopProfileRow {
  id: string;
  label: string;
  storeId: string;
  platformId: string;
  deviceFamily: string;
  status: DesktopProfileStatus;
  countryAnchor: string;
  regionAnchor: string | null;
  locale: string;
  timezone: string;
  groupLabels: string[];
  tags: string[];
  fingerprintProfileId: string;
  behaviorProfileId: string | null;
  networkPolicyId: string;
  continuityPolicyId: string;
  credentialRef: string | null;
  runtime: DesktopProfileRuntimeSummary;
  proxy: DesktopProfileProxySummary | null;
  health: DesktopProfileHealthSummary | null;
  createdAt: string;
  updatedAt: string;
}

export interface DesktopFingerprintSectionSummary {
  name: string;
  declaredCount: number;
  declaredFields: string[];
}

export interface DesktopFingerprintRuntimeSupportSummary {
  supportedFields: string[];
  unsupportedFields: string[];
  supportedCount: number;
  unsupportedCount: number;
}

export interface DesktopFingerprintConsumptionSummary {
  status: string;
  version: string;
  declaredCount: number;
  resolvedCount: number;
  appliedCount: number;
  ignoredCount: number;
  partialSupportWarning: string | null;
}

export interface DesktopFingerprintConsistencySummary {
  status: string;
  coherenceScore: number;
  hardFailureCount: number;
  softWarningCount: number;
  riskReasons: string[];
}

export interface DesktopProfileFingerprintSummary {
  profileId: string;
  profileVersion: number;
  familyId: string | null;
  familyVariant: string | null;
  schemaKind: string;
  declaredControlFields: string[];
  declaredControlCount: number;
  declaredSections: DesktopFingerprintSectionSummary[];
  runtimeSupport: DesktopFingerprintRuntimeSupportSummary;
  consistency: DesktopFingerprintConsistencySummary;
  consumption: DesktopFingerprintConsumptionSummary;
  validationOk: boolean;
  validationIssues: string[];
}

export interface DesktopProfileDetail {
  profile: DesktopProfileRow;
  fingerprintProfile: DesktopEntityReference;
  fingerprintSummary: DesktopProfileFingerprintSummary | null;
  behaviorProfile: DesktopEntityReference | null;
  networkPolicy: DesktopEntityReference;
  continuityPolicy: DesktopEntityReference;
  platformTemplate: DesktopEntityReference | null;
  storePlatformOverride: DesktopEntityReference | null;
  recentTasks: DesktopTaskItem[];
  recentLogs: DesktopLogItem[];
}

export interface DesktopProfilePageQuery {
  page?: number;
  pageSize?: number;
  search?: string;
  groupFilters?: string[];
  tagFilters?: string[];
  statusFilters?: string[];
  platformFilters?: string[];
}

export interface DesktopProfilePage {
  page: number;
  pageSize: number;
  total: number;
  items: DesktopProfileRow[];
}

export interface DesktopProxyHealthSummary {
  proxyId: string;
  overallScore: number | null;
  grade: string | null;
  trustScore: number | null;
  smokeStatus: string | null;
  verifyStatus: string | null;
  geoMatchOk: boolean | null;
  latencyMs: number | null;
  checkedAt: string | null;
}

export interface DesktopProxyHealth extends DesktopProxyHealthSummary {
  reachable: boolean | null;
  protocolOk: boolean | null;
  upstreamOk: boolean | null;
  exitIp: string | null;
  exitCountry: string | null;
  exitRegion: string | null;
  anonymityLevel: string | null;
  verifyConfidence: number | null;
  verifyScoreDelta: number | null;
  verifySource: string | null;
  probeError: string | null;
  probeErrorCategory: string | null;
  summary: DesktopJsonValue | null;
}

export interface DesktopProxyUsageSummary {
  linkedProfileCount: number;
  activeSessionCount: number;
  lastUsedAt: string | null;
  sessionKey: string | null;
  requestedRegion: string | null;
  requestedProvider: string | null;
  residencyStatus: string | null;
  rotationMode: string | null;
  stickyTtlSeconds: number | null;
  expiresAt: string | null;
}

export interface DesktopProxyUsageItem {
  sessionKey: string;
  profileId: string | null;
  profileLabel: string | null;
  storeId: string | null;
  platformId: string | null;
  siteKey: string | null;
  status: string;
  requestedRegion: string | null;
  requestedProvider: string | null;
  lastUsedAt: string;
  lastSuccessAt: string | null;
  lastFailureAt: string | null;
  expiresAt: string | null;
}

export interface DesktopProxyRow {
  id: string;
  endpointLabel: string;
  scheme: string;
  host: string;
  port: number;
  hasCredentials: boolean;
  provider: string | null;
  sourceLabel: string | null;
  region: string | null;
  country: string | null;
  status: DesktopProxyStatus;
  score: number;
  successCount: number;
  failureCount: number;
  lastCheckedAt: string | null;
  lastUsedAt: string | null;
  cooldownUntil: string | null;
  lastSeenAt: string | null;
  promotedAt: string | null;
  health: DesktopProxyHealthSummary | null;
  usage: DesktopProxyUsageSummary;
  createdAt: string;
  updatedAt: string;
}

export interface DesktopProxyPageQuery {
  page?: number;
  pageSize?: number;
  search?: string;
  statusFilters?: string[];
  regionFilters?: string[];
  providerFilters?: string[];
  sourceFilters?: string[];
}

export interface DesktopProxyPage {
  page: number;
  pageSize: number;
  total: number;
  items: DesktopProxyRow[];
}

export interface DesktopTemplateVariableDefinition {
  key: string;
  label: string | null;
  source: string;
  required: boolean;
  sensitive: boolean;
  defaultValue: DesktopJsonValue | null;
}

export interface DesktopTemplateCoverageSummary {
  warmPathCount: number;
  revisitPathCount: number;
  statefulPathCount: number;
  writeOperationPathCount: number;
  highRiskPathCount: number;
  variableCount: number;
  stepCount: number;
}

export interface DesktopTemplateMetadata {
  id: string;
  name: string;
  platformId: string;
  storeId: string | null;
  source: DesktopTemplateSource;
  status: DesktopTemplateStatus;
  readinessLevel: DesktopTemplateReadinessLevel;
  preferredLocale: string | null;
  preferredTimezone: string | null;
  allowedRegions: string[];
  coverage: DesktopTemplateCoverageSummary;
  variableDefinitions: DesktopTemplateVariableDefinition[];
  createdAt: string;
  updatedAt: string;
}

export interface DesktopTemplateMetadataPageQuery {
  page?: number;
  pageSize?: number;
  search?: string;
  platformFilters?: string[];
  readinessFilters?: string[];
  statusFilters?: string[];
  sourceFilters?: string[];
}

export interface DesktopTemplateMetadataPage {
  page: number;
  pageSize: number;
  total: number;
  items: DesktopTemplateMetadata[];
}

export interface DesktopRecorderSnapshotQuery {
  sessionId?: string;
  profileId?: string;
  platformId?: string;
  templateId?: string;
}

export interface DesktopRecorderTabSnapshot {
  tabId: string;
  title: string | null;
  url: string | null;
  active: boolean;
}

export interface DesktopRecorderStep {
  id: string;
  index: number;
  actionType: DesktopRecorderActionType;
  label: string;
  tabId: string | null;
  url: string | null;
  selector: string | null;
  selectorSource: string | null;
  inputKey: string | null;
  valuePreview: string | null;
  valueSource: DesktopRecorderValueSource | null;
  waitMs: number | null;
  sensitive: boolean;
  capturedAt: string;
  metadata: DesktopJsonValue | null;
}

export interface DesktopRecorderSnapshot {
  sessionId: string;
  status: DesktopRecorderSessionStatus;
  profileId: string | null;
  platformId: string | null;
  templateId: string | null;
  currentTabId: string | null;
  currentUrl: string | null;
  isDirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
  stepCount: number;
  sensitiveStepCount: number;
  variableCount: number;
  startedAt: string | null;
  stoppedAt: string | null;
  updatedAt: string;
  tabs: DesktopRecorderTabSnapshot[];
  steps: DesktopRecorderStep[];
}

export interface DesktopSyncWindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface DesktopSyncWindowState {
  windowId: string;
  nativeHandle: string | null;
  title: string | null;
  status: DesktopSyncWindowStatus;
  orderIndex: number;
  isMainWindow: boolean;
  isFocused: boolean;
  isMinimized: boolean;
  isVisible: boolean;
  profileId: string | null;
  profileLabel: string | null;
  storeId: string | null;
  platformId: string | null;
  lastSeenAt: string | null;
  lastActionAt: string | null;
  bounds: DesktopSyncWindowBounds | null;
}

export interface DesktopSyncLayoutState {
  mode: DesktopSyncLayoutMode;
  mainWindowId: string | null;
  columns: number | null;
  rows: number | null;
  gapPx: number;
  overlapOffsetX: number | null;
  overlapOffsetY: number | null;
  uniformWidth: number | null;
  uniformHeight: number | null;
  syncScroll: boolean;
  syncNavigation: boolean;
  syncInput: boolean;
  updatedAt: string;
}

export interface DesktopSynchronizerSnapshot {
  windows: DesktopSyncWindowState[];
  layout: DesktopSyncLayoutState;
  focusedWindowId: string | null;
  updatedAt: string;
}

export interface DesktopCreateProfileInput {
  id: string;
  storeId: string;
  platformId: string;
  deviceFamily?: string;
  countryAnchor: string;
  regionAnchor?: string | null;
  locale: string;
  timezone: string;
  fingerprintProfileId: string;
  behaviorProfileId?: string | null;
  networkPolicyId: string;
  continuityPolicyId: string;
  credentialRef?: string | null;
  status?: DesktopProfileStatus;
}

export interface DesktopUpdateProfileInput {
  id: string;
  storeId?: string;
  platformId?: string;
  deviceFamily?: string;
  countryAnchor?: string;
  regionAnchor?: string | null;
  locale?: string;
  timezone?: string;
  fingerprintProfileId?: string;
  behaviorProfileId?: string | null;
  networkPolicyId?: string;
  continuityPolicyId?: string;
  credentialRef?: string | null;
  status?: DesktopProfileStatus;
}

export interface DesktopProfileMutationResult {
  action: "created" | "updated";
  profile: DesktopProfileDetail;
  updatedAt: string;
}

export interface DesktopProfileBatchActionRequest {
  profileIds: string[];
}

export interface DesktopProfileBatchActionResult {
  action: string;
  profileIds: string[];
  updatedAt: string;
  message: string;
  taskIds: string[];
  proxyIds?: string[];
  verifyBatchId?: string | null;
}

export interface DesktopProxyBatchCheckRequest {
  proxyIds?: string[];
  provider?: string | null;
  region?: string | null;
  limit?: number;
  onlyStale?: boolean;
  staleAfterSeconds?: number;
  taskTimeoutSeconds?: number;
  minScore?: number;
  recentlyUsedWithinSeconds?: number;
  failedOnly?: boolean;
  maxPerProvider?: number;
}

export interface DesktopProxyBatchCheckProviderSummary {
  provider: string;
  accepted: number;
  skippedDueToCap: number;
}

export interface DesktopProxyBatchCheckResponse {
  batchId: string;
  status: string;
  requestedCount: number;
  acceptedCount: number;
  skippedCount: number;
  staleAfterSeconds: number;
  taskTimeoutSeconds: number;
  providerSummary: DesktopProxyBatchCheckProviderSummary[];
  filters: DesktopJsonValue | null;
  createdAt: string;
  updatedAt: string;
}

export interface DesktopProxyChangeIpRequest {
  proxyId: string;
  mode?: string | null;
  sessionKey?: string | null;
  requestedProvider?: string | null;
  requestedRegion?: string | null;
  stickyTtlSeconds?: number | null;
}

export interface DesktopProxyChangeIpResult {
  proxyId: string;
  status: string;
  mode: string;
  sessionKey: string | null;
  requestedProvider: string | null;
  requestedRegion: string | null;
  stickyTtlSeconds: number | null;
  note: string;
  residencyStatus: string;
  rotationMode: string;
  trackingTaskId: string;
  expiresAt: string | null;
  updatedAt: string;
  message: string;
}

export interface DesktopTemplateUpsertInput {
  id: string;
  name: string;
  platformId: string;
  storeId?: string | null;
  status?: DesktopTemplateStatus;
  readinessLevel?: DesktopTemplateReadinessLevel;
  allowedRegions?: string[];
  preferredLocale?: string | null;
  preferredTimezone?: string | null;
  warmPaths?: string[];
  revisitPaths?: string[];
  statefulPaths?: string[];
  writeOperationPaths?: string[];
  highRiskPaths?: string[];
  continuityChecks?: DesktopJsonValue | null;
  identityMarkers?: DesktopJsonValue | null;
  loginLossSignals?: DesktopJsonValue | null;
  recoverySteps?: DesktopJsonValue | null;
  behaviorDefaults?: DesktopJsonValue | null;
  eventChainTemplates?: DesktopJsonValue | null;
  pageSemantics?: DesktopJsonValue | null;
}

export interface DesktopTemplateDeleteInput {
  id: string;
  storeId?: string | null;
}

export interface DesktopTemplateMutationResult {
  action: "created" | "updated" | "deleted";
  template: DesktopTemplateMetadata;
  updatedAt: string;
}

export interface DesktopCompileTemplateRunRequest {
  templateId: string;
  storeId?: string | null;
  profileIds: string[];
  variableBindings: Record<string, DesktopJsonValue>;
  dryRun?: boolean;
}

export interface DesktopCompileTemplateRunResult {
  templateId: string;
  storeId: string | null;
  acceptedProfileCount: number;
  acceptedProfileIds: string[];
  variableKeys: string[];
  manifestPath: string;
  dryRun: boolean;
  status: string;
  compiledAt: string;
  message: string;
}

export interface DesktopLaunchTemplateRunRequest {
  templateId: string;
  storeId?: string | null;
  profileIds: string[];
  variableBindings: Record<string, DesktopJsonValue>;
  dryRun?: boolean;
  mode?: string;
  launchNote?: string | null;
  sourceRunId?: string | null;
  recorderSessionId?: string | null;
  targetScope?: string | null;
}

export interface DesktopLaunchTemplateRunResult {
  runId: string;
  taskId: string | null;
  status: string;
  message: string;
  manualGateRequestId: string | null;
  launchedAt: string;
  acceptedProfileCount: number;
  acceptedProfileIds: string[];
  taskIds: string[];
  taskCount: number;
  manifestPath: string;
  launchSummary: DesktopLaunchTemplateRunSummary;
  raw: DesktopJsonValue | null;
}

export interface DesktopReadRunDetailQuery {
  runId?: string | null;
  taskId?: string | null;
}

export interface DesktopRunArtifact {
  id: string;
  label: string;
  path: string | null;
  status: string | null;
  createdAt: string | null;
}

export interface DesktopLaunchTemplateRunSummary {
  templateId: string;
  launchKind: string;
  launchMode: string;
  primaryTaskId: string | null;
  taskCount: number;
  acceptedProfileCount: number;
  acceptedProfileIds: string[];
  sourceRunId: string | null;
  recorderSessionId: string | null;
  targetScope: string | null;
  launchNote: string | null;
  compiledAt: string;
  launchedAt: string;
  manifestPath: string;
}

export interface DesktopRunTimelineEntry {
  id: string;
  label: string;
  status: string;
  detail: string | null;
  createdAt: string | null;
}

export interface DesktopRunDetail {
  runId: string;
  taskId: string | null;
  status: string;
  headline: string;
  message: string | null;
  failureReason: string | null;
  manualGateRequestId: string | null;
  manualGateStatus: string | null;
  updatedAtLabel: string | null;
  createdAtLabel: string | null;
  taskStatus: string;
  runAttempt: number | null;
  runnerKind: string | null;
  artifactCount: number;
  logCount: number;
  timelineCount: number;
  artifacts: DesktopRunArtifact[];
  timeline: DesktopRunTimelineEntry[];
  summary: DesktopRunDetailSummary;
  raw: DesktopJsonValue | null;
}

export interface DesktopRunDetailSummary {
  taskStatus: string;
  runStatus: string;
  runAttempt: number | null;
  runnerKind: string | null;
  artifactCount: number;
  logCount: number;
  timelineCount: number;
}

export interface DesktopTaskWriteResult {
  taskId: string;
  status: string;
  message: string;
  updatedAt: string;
  runId: string | null;
  manualGateRequestId: string | null;
}

export interface DesktopManualGateActionRequest {
  manualGateRequestId: string;
  note?: string | null;
}

export interface DesktopStartBehaviorRecordingRequest {
  sessionId?: string;
  profileId?: string | null;
  platformId?: string | null;
  templateId?: string | null;
}

export interface DesktopStopBehaviorRecordingRequest {
  sessionId?: string;
}

export interface DesktopAppendBehaviorRecordingStepRequest {
  sessionId?: string;
  profileId?: string | null;
  platformId?: string | null;
  templateId?: string | null;
  stepId: string;
  index: number;
  actionType: DesktopRecorderActionType;
  label: string;
  tabId?: string | null;
  url?: string | null;
  selector?: string | null;
  selectorSource?: string | null;
  inputKey?: string | null;
  valuePreview?: string | null;
  valueSource?: DesktopRecorderValueSource | null;
  waitMs?: number | null;
  sensitive: boolean;
  metadata?: DesktopJsonValue | null;
}

export interface DesktopSyncLayoutUpdate {
  mode?: DesktopSyncLayoutMode;
  columns?: number | null;
  rows?: number | null;
  gapPx?: number;
  overlapOffsetX?: number | null;
  overlapOffsetY?: number | null;
  uniformWidth?: number | null;
  uniformHeight?: number | null;
  syncScroll?: boolean;
  syncNavigation?: boolean;
  syncInput?: boolean;
}

export interface DesktopSynchronizerActionResult {
  action: string;
  snapshot: DesktopSynchronizerSnapshot;
  updatedAt: string;
  message: string;
}
