export interface BrowserProfile {
  profileId: string
  profileName: string
  userDataDir: string
  coreId: string
  fingerprintArgs: string[]
  proxyId: string
  proxyConfig: string
  proxyBindSourceId?: string
  proxyBindSourceUrl?: string
  proxyBindName?: string
  proxyBindUpdatedAt?: string
  launchArgs: string[]
  tags: string[]
  keywords: string[]
  groupId?: string
  running: boolean
  debugPort: number
  debugReady: boolean
  pid: number
  runtimeWarning: string
  lastError: string
  createdAt: string
  updatedAt: string
  lastStartAt?: string
  lastStopAt?: string
  launchCode?: string
  behaviorProfileId?: string
  humanizeSeed?: string
}

export interface BrowserProfileInput {
  profileName: string
  userDataDir: string
  coreId: string
  fingerprintArgs: string[]
  proxyId: string
  proxyConfig: string
  launchArgs: string[]
  tags: string[]
  keywords: string[]
  groupId?: string
  behaviorProfileId?: string
  humanizeSeed?: string
}

export interface BrowserTab {
  tabId: string
  title: string
  url: string
  active: boolean
}

export interface BrowserSettings {
  userDataRoot: string
  defaultFingerprintArgs: string[]
  defaultLaunchArgs: string[]
  defaultProxy: string
  startReadyTimeoutMs: number
  startStableWindowMs: number
}

export interface BrowserCore {
  coreId: string
  coreName: string
  corePath: string
  isDefault: boolean
}

export interface BrowserCoreInput {
  coreId: string
  coreName: string
  corePath: string
  isDefault: boolean
}

export interface BrowserCoreValidateResult {
  valid: boolean
  message: string
}

export interface BrowserProxy {
  proxyId: string
  proxyName: string
  proxyConfig: string
  dnsServers?: string
  groupName?: string
  sourceId?: string
  sourceUrl?: string
  sourceNamePrefix?: string
  sourceAutoRefresh?: boolean
  sourceRefreshIntervalM?: number
  sourceLastRefreshAt?: string
  lastLatencyMs?: number
  lastTestOk?: boolean
  lastTestedAt?: string
  lastIPHealthJson?: string
}

export interface ProxyIPHealthResult {
  proxyId: string
  ok: boolean
  source: string
  error: string
  latencyMs?: number
  ip: string
  fraudScore: number
  isResidential: boolean
  isBroadcast: boolean
  country: string
  region: string
  city: string
  asOrganization: string
  rawData: Record<string, any>
  updatedAt: string
}

export interface BrowserProxyImportFreeDirectProxiesInput {
  sourceUrls?: string[]
  groupName?: string
  limit?: number
  concurrency?: number
  timeoutMs?: number
}

export interface BrowserProxyImportFreeDirectProxiesResult {
  fetchedCount: number
  uniqueCount: number
  checkedCount: number
  importedCount: number
  failedCount: number
  skippedExistingCount: number
  sourceErrors: string[]
  importedProxies: BrowserProxy[]
  healthResults: ProxyIPHealthResult[]
  allProxies: BrowserProxy[]
}

export interface BrowserCoreExtended {
  coreId: string
  chromeVersion: string
  instanceCount: number
}

export interface CookieInfo {
  name: string
  value: string
  domain: string
  path: string
  expires: number
  httpOnly: boolean
  secure: boolean
  sameSite: string
}

export interface SnapshotInfo {
  snapshotId: string
  profileId: string
  name: string
  sizeMB: number
  createdAt: string
}

export interface BrowserBookmark {
  name: string
  url: string
}


// 分组相关类型
export interface BrowserGroup {
  groupId: string
  groupName: string
  parentId: string
  sortOrder: number
  createdAt: string
  updatedAt: string
}

export interface BrowserGroupInput {
  groupName: string
  parentId: string
  sortOrder: number
}

export interface BrowserGroupWithCount extends BrowserGroup {
  instanceCount: number
}

// ─── Behavior Recording ──────────────────────────────────────────────
// Mirrors behavior.* classes in wailsjs/go/models.ts (keep in sync)

export interface RecordedEvent {
  t: number
  type: string
  x?: number
  y?: number
  btn?: number
  key?: string
  text?: string
  dx?: number
  dy?: number
  inputType?: string
  targetPath?: string
  sensitive?: boolean
}

export interface RecordingBase {
  id: string
  name: string
  description: string
  durationMs: number
  viewportW: number
  viewportH: number
  startUrl?: string
  currentUrl?: string
  title?: string
  devicePixelRatio?: number
  scale?: number
  createdAt: string
}

export interface Recording extends RecordingBase {
  events: RecordedEvent[]
}

export interface RecordingSummary extends RecordingBase {
  eventCount: number
  events?: RecordedEvent[]
}

export interface RecordingEventStats {
  total: number
  move: number
  click: number
  key: number
  scroll: number
}

export interface RecordingDetailPage {
  recording: RecordingSummary
  events: RecordedEvent[]
  eventOffset: number
  eventLimit: number
  eventTotal: number
  stats: RecordingEventStats
}

export interface RecordingExportBundle {
  schemaVersion?: string
  exportedAt?: string
  recording?: Recording
  recordings?: Recording[]
  [key: string]: unknown
}

export interface ActiveRecordingStatus {
  active: boolean
  count: number
  profileId?: string
  profileIds: string[]
  inMemoryProfileIds?: string[]
  recoverableProfileIds?: string[]
}

export const BEHAVIOR_EXECUTION_PERMISSION_MODES = ['ask_each_time', 'auto_review', 'full_access'] as const

export type BehaviorExecutionPermissionMode = typeof BEHAVIOR_EXECUTION_PERMISSION_MODES[number]

export const DEFAULT_BEHAVIOR_EXECUTION_PERMISSION_MODE: BehaviorExecutionPermissionMode = 'ask_each_time'

export const BEHAVIOR_HUMAN_BOUNDARIES = [
  'captcha',
  '2fa',
  'device_trust',
  'password',
  'delete',
  'account_settings',
  'switch_account',
] as const

export type BehaviorHumanBoundary = typeof BEHAVIOR_HUMAN_BOUNDARIES[number]

export const BEHAVIOR_HUMAN_BOUNDARY_LABELS: Record<BehaviorHumanBoundary, string> = {
  captcha: '验证码',
  '2fa': '2FA',
  device_trust: '设备信任',
  password: '密码',
  delete: '删除',
  account_settings: '账号设置',
  switch_account: '切换账号',
}

export interface BehaviorLowConfidencePausePolicy {
  enabled: boolean
  revealTargetScreenshot: false
  revealCandidateElements: false
  revealRecommendedPoint: false
  promptFields: Array<'reason' | 'action'>
}

export interface BehaviorExecutionPolicy {
  permissionMode: BehaviorExecutionPermissionMode
  humanBoundaries: BehaviorHumanBoundary[]
  lowConfidencePause: BehaviorLowConfidencePausePolicy
}

export interface BehaviorTemplateSemantics {
  schemaVersion: 1
  source: 'recording_template'
  intent: 'unknown'
  thirdPartyDetection: false
  screenshotCandidates: false
  humanBoundaries: BehaviorHumanBoundary[]
}

export interface VariationConfig {
  intensity: number
  timingJitter: number
  positionJitter: number
  speedVariation: number
  microCorrections: boolean
  extraPauses: boolean
  executionPolicy?: BehaviorExecutionPolicy
  templateSemantics?: BehaviorTemplateSemantics
}

export interface PlaybackProgressPayload {
  profileId?: string
  recordingId?: string
  eventIndex: number
  eventTotal: number
  percent: number
  elapsedMs: number
  status: string
  reason?: string
  action?: string
}

export interface PlaybackEventPayload {
  profileId?: string
  recordingId?: string
  error?: string
}

export type PlaybackReviewDecision = 'continue' | 'skip' | 'stop'

export interface NaturalLanguageAction {
  type: 'goto' | 'click' | 'scroll' | 'type' | 'wait'
  description?: string
  selector?: string
  url?: string
  text?: string
  durationMs?: number
}

export interface NaturalLanguageTaskEvent {
  profileId?: string
  step?: number
  total?: number
  action?: string
  actions?: NaturalLanguageAction[]
  recordingId?: string
  durationMs?: number
  eventCount?: number
  error?: string
  reason?: string
  message?: string
}
