import {
  browserProxyBatchCheckIPHealthFromDesktop,
  browserProxyBatchTestSpeedFromDesktop,
  browserProxyCheckIPHealthFromDesktop,
  browserProxyTestSpeedFromDesktop,
  deleteBrowserCoreFromDesktop,
  deleteBrowserProxyFromDesktop,
  downloadBrowserCoreFromDesktop,
  desktopRuntimeListen,
  fetchBrowserProxyClashFromDesktop,
  fixBrowserProxyNamesFromDesktop,
  importBrowserProxySubscriptionFromDesktop,
  listBrowserCoreExtendedInfoFromDesktop,
  listBrowserCoresFromDesktop,
  listBrowserProxiesByGroupFromDesktop,
  listBrowserProxiesFromDesktop,
  listBrowserProxyGroupsFromDesktop,
  openCorePathFromDesktop,
  openUserDataDirFromDesktop,
  readBrowserSettingsFromDesktop,
  readLaunchServerInfoFromDesktop,
  saveBrowserCoreFromDesktop,
  saveBrowserProxiesFromDesktop,
  saveBrowserSettingsFromDesktop,
  scanBrowserCoresFromDesktop,
  setDefaultBrowserCoreFromDesktop,
  testProxyConnectivityFromDesktop,
  testProxyRealConnectivityFromDesktop,
  validateBrowserCoreForKindFromDesktop,
  validateBrowserCoreFromDesktop,
  validateProxyConfigFromDesktop,
  DesktopServiceError,
  hasDesktopRuntime,
} from '../../services/desktop'
import type {
  BehaviorExecutionPermissionMode, BrowserProfile, BrowserProfileInput, BrowserTab, BrowserSettings,
  BrowserCore, BrowserCoreInput, BrowserCoreValidateResult, BrowserProxy, BrowserCoreExtended,
  CookieInfo, SnapshotInfo, BrowserBookmark, BrowserGroup, BrowserGroupInput, BrowserGroupWithCount,
  ProxyIPHealthResult, ActiveRecordingStatus, Recording, RecordingDetailPage, RecordingEventStats,
  RecordingSummary, RecordedEvent, PlaybackEventPayload, PlaybackProgressPayload, RecordingExportBundle, VariationConfig,
  BrowserInstanceRuntimeEvent,
  BrowserInstanceRuntimeEventName,
  BrowserRuntimeEventPayload,
} from './types'
import { backend } from '../../wailsjs/go/models'
import { DEFAULT_BEHAVIOR_EXECUTION_PERMISSION_MODE } from './types'

type Unsubscribe = () => void
type BrowserInstanceRuntimeEventHandler = (event: BrowserInstanceRuntimeEvent) => void
type AutomationActionParams = Record<string, unknown>

export interface SchedulerTaskTrigger {
  type: string
  cron?: string
  interval?: string
  event?: string
}

export interface SchedulerTaskAction {
  type: string
  target: string
  value: string
  timeout: number
}

export interface SchedulerTaskInfo {
  id: string
  name: string
  trigger: SchedulerTaskTrigger
  actions: SchedulerTaskAction[]
  maxRetries: number
  retryDelay: string
  dependsOn: string[]
  profileId: string
  enabled: boolean
  createdAt: string
  status: string
  lastRunAt: string
  lastError: string
  retryCount: number
}

export interface SchedulerTaskInput {
  name: string
  trigger: SchedulerTaskTrigger
  actions: SchedulerTaskAction[]
  maxRetries: number
  retryDelay: string
  dependsOn: string[]
  profileId: string
  enabled: boolean
}

export interface AutomationRuleInfo {
  id: string
  name: string
  triggerEvent: string
  condition?: string
  action: string
  actionParams?: AutomationActionParams
  cooldown: string
  enabled: boolean
  createdAt: string
  updatedAt: string
}

export interface AutomationRuleInput {
  name: string
  triggerEvent: string
  condition?: string
  action: string
  actionParams?: AutomationActionParams
  cooldown: string
  enabled: boolean
}

type BrowserNativeBindings = Partial<{
  BrowserProfileList: () => Promise<BrowserProfile[]>
  BrowserProfileListByTag: (tag: string) => Promise<BrowserProfile[]>
  BrowserGetAllTags: () => Promise<string[]>
  BrowserProfileCreate: (input: BrowserProfileInput) => Promise<BrowserProfile>
  BrowserProfileUpdate: (profileId: string, input: BrowserProfileInput) => Promise<BrowserProfile>
  BrowserProfileDelete: (profileId: string) => Promise<void>
  BrowserProfileCopy: (profileId: string, newName: string) => Promise<BrowserProfile>
  BrowserInstanceStart: (profileId: string) => Promise<BrowserProfile>
  BrowserInstanceStartByCode: (code: string) => Promise<BrowserProfile>
  BrowserInstanceStop: (profileId: string) => Promise<BrowserProfile>
  BrowserInstanceRestart: (profileId: string) => Promise<BrowserProfile>
  BrowserInstanceOpenUrl: (profileId: string, targetUrl: string) => Promise<boolean>
  BrowserInstanceGetTabs: (profileId: string) => Promise<BrowserTab[]>
  BrowserGetCookies: (profileId: string) => Promise<CookieInfo[]>
  BrowserClearCookies: (profileId: string) => Promise<void>
  BrowserExportCookies: (profileId: string) => Promise<string>
  BrowserSnapshotList: (profileId: string) => Promise<SnapshotInfo[]>
  BrowserSnapshotCreate: (profileId: string, name: string) => Promise<SnapshotInfo>
  BrowserSnapshotRestore: (profileId: string, snapshotId: string) => Promise<void>
  BrowserSnapshotDelete: (profileId: string, snapshotId: string) => Promise<void>
  BookmarkList: () => Promise<BrowserBookmark[]>
  BookmarkSave: (items: BrowserBookmark[]) => Promise<void>
  BookmarkReset: () => Promise<void>
  BrowserProfileSetKeywords: (profileId: string, keywords: string[]) => Promise<BrowserProfile>
  GetLaunchServerInfo: () => Promise<Partial<LaunchServerInfo>>
  BrowserProfileGetCode: (profileId: string) => Promise<string>
  BrowserProfileRegenerateCode: (profileId: string) => Promise<string>
  BrowserProfileSetCode: (profileId: string, code: string) => Promise<string>
  BrowserProfileBatchSetTags: (profileIds: string[], tags: string[], replace: boolean) => Promise<void>
  BrowserProfileBatchRemoveTags: (profileIds: string[], tags: string[]) => Promise<void>
  BrowserRenameTag: (oldName: string, newName: string) => Promise<void>
  ListGroups: () => Promise<BrowserGroupWithCount[]>
  CreateGroup: (input: BrowserGroupInput) => Promise<BrowserGroup>
  UpdateGroup: (groupId: string, input: BrowserGroupInput) => Promise<BrowserGroup>
  DeleteGroup: (groupId: string) => Promise<void>
  MoveInstancesToGroup: (profileIds: string[], groupId: string) => Promise<void>
  BehaviorStartRecording: (profileId: string) => Promise<void>
  BehaviorStopRecording: (profileId: string, name: string) => Promise<Recording>
  BehaviorRecordingList: () => Promise<Recording[]>
  BehaviorRecordingSummaryList: () => Promise<RecordingSummary[]>
  BehaviorRecordingStatus: () => Promise<Partial<ActiveRecordingStatus>>
  ActiveRecordingStatus: () => Promise<Partial<ActiveRecordingStatus>>
  BehaviorRecordingDelete: (id: string) => Promise<void>
  BehaviorGetRecording: (id: string) => Promise<Recording | null>
  BehaviorGetRecordingDetail: (id: string, eventOffset: number, eventLimit: number) => Promise<RecordingDetailPage | null>
  BehaviorPlayRecording: (profileId: string, recordingId: string, variation: VariationConfig) => Promise<void>
  BehaviorStopPlayback: (profileId: string) => Promise<void>
  BehaviorQuickRecord: (profileId: string) => Promise<Recording>
  BehaviorPresetList: () => Promise<Array<{ id: string; name: string; description: string }>>
  CleanupStaleRecordingSessions: () => Promise<void>
  BehaviorRecordingRename: (id: string, name: string) => Promise<void>
  BehaviorRecordingExport: (id: string) => Promise<RecordingExportBundle>
  BehaviorRecordingImport: (payload: string, name: string) => Promise<Recording>
  BehaviorRecordingCopy: (id: string, name: string) => Promise<Recording>
  BehaviorPlaybackReview: (profileId: string, decision: string) => Promise<void>
  BehaviorRecordingTrim: (id: string, startEvent: number, endEvent: number, name: string) => Promise<Recording>
  SchedulerListTasks: () => Promise<backend.SchedulerTaskInfo[]>
  SchedulerAddTask: (input: backend.SchedulerTaskInput) => Promise<backend.SchedulerTaskInfo>
  SchedulerRemoveTask: (taskId: string) => Promise<void>
  SchedulerRunTaskNow: (taskId: string) => Promise<void>
  AutomationRuleList: () => Promise<AutomationRuleInfo[]>
  AutomationRuleCreate: (input: AutomationRuleInput) => Promise<AutomationRuleInfo>
  AutomationRuleDelete: (ruleId: string) => Promise<void>
  AutomationRuleToggle: (ruleId: string, enabled: boolean) => Promise<void>
  AutomationRuleTestFire: (ruleId: string) => Promise<void>
}>

const BROWSER_INSTANCE_RUNTIME_EVENT_NAMES: BrowserInstanceRuntimeEventName[] = [
  'browser:instance:started',
  'browser:instance:updated',
  'browser:instance:stopped',
  'browser:instance:crashed',
]

const getBindings = async () => {
  try {
    return await import('../../wailsjs/go/main/App') as BrowserNativeBindings
  } catch {
    return null
  }
}

async function tryDesktop<T>(call: () => Promise<T>): Promise<T | null> {
  try {
    return await call()
  } catch (error) {
    if (
      error instanceof DesktopServiceError &&
      (error.code === 'desktop_invoke_unavailable' || error.code === 'desktop_command_not_ready')
    ) {
      return null
    }
    throw error
  }
}

async function tryDesktopVoid(call: () => Promise<void>): Promise<boolean> {
  const result = await tryDesktop(async () => {
    await call()
    return true
  })
  return result === true
}

function defaultBrowserSettings(): BrowserSettings {
  return {
    userDataRoot: 'data',
    defaultFingerprintArgs: [],
    defaultLaunchArgs: [],
    defaultProxy: '',
    startReadyTimeoutMs: 3000,
    startStableWindowMs: 1200,
  }
}

function onRuntimeEvent<T>(eventName: string, callback: (payload: T) => void): Unsubscribe {
  return desktopRuntimeListen(eventName, (payload: T) => callback(payload))
}

function combineUnsubscribes(offs: Unsubscribe[]): Unsubscribe {
  let disposed = false
  return () => {
    if (disposed) return
    disposed = true
    offs.forEach(off => {
      try {
        off()
      } catch {
        // Runtime event cleanup should never break component unmount.
      }
    })
  }
}

function readRecord(payload: unknown): Record<string, unknown> {
  return payload && typeof payload === 'object' ? payload as Record<string, unknown> : {}
}

function readStringField(source: Record<string, unknown>, keys: string[]): string | undefined {
  for (const key of keys) {
    const value = source[key]
    if (typeof value === 'string' && value.trim()) return value.trim()
  }
  return undefined
}

function readNumberField(source: Record<string, unknown>, keys: string[]): number | undefined {
  for (const key of keys) {
    const value = source[key]
    if (typeof value === 'number' && Number.isFinite(value)) return value
    if (typeof value === 'string' && value.trim() && Number.isFinite(Number(value))) return Number(value)
  }
  return undefined
}

function readBooleanField(source: Record<string, unknown>, keys: string[]): boolean | undefined {
  for (const key of keys) {
    const value = source[key]
    if (typeof value === 'boolean') return value
    if (typeof value === 'string') {
      const normalized = value.trim().toLowerCase()
      if (normalized === 'true') return true
      if (normalized === 'false') return false
    }
  }
  return undefined
}

function stringField(source: Record<string, unknown>, key: string): string | undefined {
  const value = source[key]
  if (typeof value === 'string') return value
  if (value != null && typeof value !== 'object') return String(value)
  return undefined
}

function numberField(source: Record<string, unknown>, key: string): number | undefined {
  const value = source[key]
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string' && value.trim() && Number.isFinite(Number(value))) return Number(value)
  return undefined
}

// The desktop RPC returns proxy rows as unchecked records. Normalize the
// known BrowserProxy fields so callers receive a typed array instead of a cast.
function normalizeProxyRecords(rows: Array<Record<string, unknown>> | undefined): BrowserProxy[] {
  if (!rows) return []
  const proxies: BrowserProxy[] = []
  for (const row of rows) {
    const proxyId = stringField(row, 'proxyId') || stringField(row, 'proxy_id') || stringField(row, 'id')
    if (!proxyId) continue
    const proxy: BrowserProxy = {
      proxyId,
      proxyName: stringField(row, 'proxyName') || stringField(row, 'proxy_name') || proxyId,
      proxyConfig: stringField(row, 'proxyConfig') || stringField(row, 'proxy_config') || '',
    }
    const dnsServers = stringField(row, 'dnsServers')
    if (dnsServers) proxy.dnsServers = dnsServers
    const groupName = stringField(row, 'groupName')
    if (groupName) proxy.groupName = groupName
    const sourceId = stringField(row, 'sourceId')
    if (sourceId) proxy.sourceId = sourceId
    const sourceUrl = stringField(row, 'sourceUrl')
    if (sourceUrl) proxy.sourceUrl = sourceUrl
    const sourceNamePrefix = stringField(row, 'sourceNamePrefix')
    if (sourceNamePrefix) proxy.sourceNamePrefix = sourceNamePrefix
    if (typeof row.sourceAutoRefresh === 'boolean') proxy.sourceAutoRefresh = row.sourceAutoRefresh
    const sourceRefreshIntervalM = numberField(row, 'sourceRefreshIntervalM')
    if (sourceRefreshIntervalM !== undefined) proxy.sourceRefreshIntervalM = sourceRefreshIntervalM
    const sourceLastRefreshAt = stringField(row, 'sourceLastRefreshAt')
    if (sourceLastRefreshAt) proxy.sourceLastRefreshAt = sourceLastRefreshAt
    const lastLatencyMs = numberField(row, 'lastLatencyMs')
    if (lastLatencyMs !== undefined) proxy.lastLatencyMs = lastLatencyMs
    if (typeof row.lastTestOk === 'boolean') proxy.lastTestOk = row.lastTestOk
    const lastTestedAt = stringField(row, 'lastTestedAt')
    if (lastTestedAt) proxy.lastTestedAt = lastTestedAt
    const lastIPHealthJson = stringField(row, 'lastIPHealthJson')
    if (lastIPHealthJson) proxy.lastIPHealthJson = lastIPHealthJson
    proxies.push(proxy)
  }
  return proxies
}

export function normalizeBrowserRuntimeEventPayload(payload: unknown): BrowserRuntimeEventPayload {
  if (typeof payload === 'string') {
    return { profileId: payload.trim() }
  }

  const source = readRecord(payload)
  const profileId = readStringField(source, ['profileId', 'profile_id', 'id']) || ''
  const normalized: BrowserRuntimeEventPayload = { profileId }
  const profileName = readStringField(source, ['profileName', 'profile_name', 'name'])
  const runtimeWarning = readStringField(source, ['runtimeWarning', 'runtime_warning', 'warning'])
  const error = readStringField(source, ['error', 'lastError', 'last_error', 'message'])
  const debugPort = readNumberField(source, ['debugPort', 'debug_port'])
  const pid = readNumberField(source, ['pid'])
  const debugReady = readBooleanField(source, ['debugReady', 'debug_ready'])
  const running = readBooleanField(source, ['running'])
  const reused = readBooleanField(source, ['reused'])

  if (profileName) normalized.profileName = profileName
  if (runtimeWarning) normalized.runtimeWarning = runtimeWarning
  if (error) normalized.error = error
  if (debugPort !== undefined) normalized.debugPort = debugPort
  if (pid !== undefined) normalized.pid = pid
  if (debugReady !== undefined) normalized.debugReady = debugReady
  if (running !== undefined) normalized.running = running
  if (reused !== undefined) normalized.reused = reused
  return normalized
}

export function onBrowserInstanceRuntimeEvents(
  handler: BrowserInstanceRuntimeEventHandler,
): Unsubscribe {
  return combineUnsubscribes(BROWSER_INSTANCE_RUNTIME_EVENT_NAMES.map((eventName) => (
    onRuntimeEvent(eventName, (payload: unknown) => handler({
      eventName,
      payload: normalizeBrowserRuntimeEventPayload(payload),
      rawPayload: payload,
    }))
  )))
}

let mockProfiles: BrowserProfile[] = [
  {
    profileId: 'mock-1',
    profileName: '默认指纹配置',
    userDataDir: 'data/default',
    coreId: 'default',
    fingerprintArgs: ['--fingerprint-brand=Chrome', '--fingerprint-platform=windows'],
    proxyId: '',
    proxyConfig: '',
    launchArgs: ['--disable-features=Translate'],
    tags: ['默认'],
    keywords: [],
    running: false,
    debugPort: 0,
    debugReady: false,
    pid: 0,
    runtimeWarning: '',
    lastError: '',
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
]

let mockCores: BrowserCore[] = []

let mockProxies: BrowserProxy[] = []

// ============================================================================
// Profile API
// ============================================================================

export async function fetchBrowserProfiles(): Promise<BrowserProfile[]> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileList) {
    return (await bindings.BrowserProfileList()) || []
  }
  return mockProfiles
}

export async function fetchBrowserProfilesByTag(tag: string): Promise<BrowserProfile[]> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileListByTag) {
    return (await bindings.BrowserProfileListByTag(tag)) || []
  }
  return mockProfiles.filter(p => p.tags?.includes(tag))
}

export async function fetchAllTags(): Promise<string[]> {
  const bindings = await getBindings()
  if (bindings?.BrowserGetAllTags) {
    return (await bindings.BrowserGetAllTags()) || []
  }
  const set = new Set<string>()
  mockProfiles.forEach(p => p.tags?.forEach(t => set.add(t)))
  return Array.from(set).sort()
}

export async function createBrowserProfile(input: BrowserProfileInput): Promise<BrowserProfile | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileCreate) {
    return (await bindings.BrowserProfileCreate(input)) || null
  }
  const profile: BrowserProfile = {
    profileId: `mock-${Date.now()}`,
    ...input,
    keywords: input.keywords || {},
    running: false,
    debugPort: 0,
    debugReady: false,
    pid: 0,
    runtimeWarning: '',
    lastError: '',
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  }
  mockProfiles = [profile, ...mockProfiles]
  return profile
}

export async function updateBrowserProfile(profileId: string, input: BrowserProfileInput): Promise<BrowserProfile | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileUpdate) {
    return (await bindings.BrowserProfileUpdate(profileId, input)) || null
  }
  const index = mockProfiles.findIndex(item => item.profileId === profileId)
  if (index === -1) return null
  const current = mockProfiles[index]!
  const updated: BrowserProfile = { ...current, ...input, updatedAt: new Date().toISOString() }
  mockProfiles[index] = updated
  return updated
}

export async function deleteBrowserProfile(profileId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileDelete) {
    await bindings.BrowserProfileDelete(profileId)
    return true
  }
  mockProfiles = mockProfiles.filter(item => item.profileId !== profileId)
  return true
}

export async function copyBrowserProfile(profileId: string, newName: string): Promise<BrowserProfile | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileCopy) {
    return (await bindings.BrowserProfileCopy(profileId, newName)) || null
  }
  // mock
  const src = mockProfiles.find(p => p.profileId === profileId)
  if (!src) return null
  const copy: BrowserProfile = {
    ...src,
    profileId: `mock-${Date.now()}`,
    profileName: newName || src.profileName + ' (副本)',
    userDataDir: `mock-${Date.now()}`,
    running: false,
    debugReady: false,
    runtimeWarning: '',
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  }
  mockProfiles = [copy, ...mockProfiles]
  return copy
}

// ============================================================================
// Instance API
// ============================================================================

export async function startBrowserInstance(profileId: string): Promise<BrowserProfile | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserInstanceStart) {
    return (await bindings.BrowserInstanceStart(profileId)) || null
  }
  mockProfiles = mockProfiles.map(item =>
    item.profileId === profileId ? { ...item, running: true, debugPort: 9222, debugReady: true, pid: Math.floor(Math.random() * 100000), runtimeWarning: '', lastStartAt: new Date().toISOString() } : item
  )
  return mockProfiles.find(item => item.profileId === profileId) || null
}

export async function startBrowserInstanceByCode(code: string): Promise<BrowserProfile | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserInstanceStartByCode) {
    return (await bindings.BrowserInstanceStartByCode(code)) || null
  }
  const normalized = code.trim().toUpperCase()
  const profile = mockProfiles.find(item => (item.launchCode || '').toUpperCase() === normalized)
  if (!profile) {
    throw new Error('launch code not found')
  }
  return await startBrowserInstance(profile.profileId)
}

export async function stopBrowserInstance(profileId: string): Promise<BrowserProfile | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserInstanceStop) {
    return (await bindings.BrowserInstanceStop(profileId)) || null
  }
  mockProfiles = mockProfiles.map(item =>
    item.profileId === profileId ? { ...item, running: false, debugReady: false, debugPort: 0, pid: 0, runtimeWarning: '', lastStopAt: new Date().toISOString() } : item
  )
  return mockProfiles.find(item => item.profileId === profileId) || null
}

export async function restartBrowserInstance(profileId: string): Promise<BrowserProfile | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserInstanceRestart) {
    return (await bindings.BrowserInstanceRestart(profileId)) || null
  }
  await stopBrowserInstance(profileId)
  return await startBrowserInstance(profileId)
}

export async function openBrowserUrl(profileId: string, targetUrl: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserInstanceOpenUrl) {
    return (await bindings.BrowserInstanceOpenUrl(profileId, targetUrl)) === true
  }
  return true
}

export async function fetchBrowserTabs(profileId: string): Promise<BrowserTab[]> {
  const bindings = await getBindings()
  if (bindings?.BrowserInstanceGetTabs) {
    return (await bindings.BrowserInstanceGetTabs(profileId)) || []
  }
  return [
    { tabId: 'tab-1', title: '新标签页', url: 'about:blank', active: true },
    { tabId: 'tab-2', title: '示例站点', url: 'https://example.com', active: false },
  ]
}

// ============================================================================
// Settings API
// ============================================================================

export async function fetchBrowserSettings(): Promise<BrowserSettings> {
  const settings = await tryDesktop(() => readBrowserSettingsFromDesktop())
  if (settings) return settings
  return defaultBrowserSettings()
}

export async function saveBrowserSettings(settings: BrowserSettings): Promise<boolean> {
  await tryDesktopVoid(() => saveBrowserSettingsFromDesktop(settings))
  return true
}

// ============================================================================
// Core API
// ============================================================================

export async function fetchBrowserCores(): Promise<BrowserCore[]> {
  const cores = await tryDesktop(() => listBrowserCoresFromDesktop())
  if (cores) return cores
  return mockCores
}

export async function saveBrowserCore(input: BrowserCoreInput): Promise<boolean> {
  await tryDesktopVoid(() => saveBrowserCoreFromDesktop(input))
  const index = mockCores.findIndex(c => c.coreId === input.coreId)
  if (index >= 0) {
    mockCores[index] = input
  } else {
    mockCores.push({ ...input, coreId: input.coreId || `core-${Date.now()}` })
  }
  return true
}

export async function deleteBrowserCore(coreId: string): Promise<boolean> {
  await tryDesktopVoid(() => deleteBrowserCoreFromDesktop(coreId))
  mockCores = mockCores.filter(c => c.coreId !== coreId)
  return true
}

export async function setDefaultBrowserCore(coreId: string): Promise<boolean> {
  await tryDesktopVoid(() => setDefaultBrowserCoreFromDesktop(coreId))
  mockCores = mockCores.map(c => ({ ...c, isDefault: c.coreId === coreId }))
  return true
}

export async function validateBrowserCorePath(corePath: string, kind: BrowserCoreInput['kind'] = 'chromium'): Promise<BrowserCoreValidateResult> {
  const kindResult = await tryDesktop(() => validateBrowserCoreForKindFromDesktop(corePath, kind || 'chromium'))
  if (kindResult) return kindResult
  const legacyResult = await tryDesktop(() => validateBrowserCoreFromDesktop(corePath))
  if (legacyResult) return legacyResult
  return { valid: true, message: '路径有效（模拟）' }
}

export async function fetchCoreExtendedInfo(): Promise<BrowserCoreExtended[]> {
  return await tryDesktop(() => listBrowserCoreExtendedInfoFromDesktop()) || []
}

export async function scanBrowserCores(): Promise<BrowserCore[]> {
  const cores = await tryDesktop(() => scanBrowserCoresFromDesktop())
  if (cores) return cores
  return mockCores
}

export async function BrowserCoreDownload(coreName: string, url: string, proxyConfig?: string): Promise<boolean> {
  await tryDesktopVoid(() => downloadBrowserCoreFromDesktop(coreName, url, proxyConfig || ''))
  return true
}

// ============================================================================
// Proxy API
// ============================================================================

export async function fetchBrowserProxies(): Promise<BrowserProxy[]> {
  const proxies = await tryDesktop(() => listBrowserProxiesFromDesktop())
  if (proxies) return proxies
  return mockProxies
}

export async function fetchBrowserProxyGroups(): Promise<string[]> {
  return await tryDesktop(() => listBrowserProxyGroupsFromDesktop()) || []
}

export async function fetchBrowserProxiesByGroup(groupName: string): Promise<BrowserProxy[]> {
  const proxies = await tryDesktop(() => listBrowserProxiesByGroupFromDesktop(groupName))
  if (proxies) return proxies
  return mockProxies.filter(p => p.groupName === groupName)
}

export interface ClashImportURLResult {
  url: string
  content: string
  proxyCount: number
  dnsServers?: string
  suggestedGroup?: string
  autoFallback?: boolean
  importedCount?: number
  skippedCount?: number
  totalCount?: number
  groupName?: string
  allProxies?: BrowserProxy[]
}

export interface SubscriptionImportResult {
  url: string
  importedCount: number
  skippedCount: number
  totalCount: number
  groupName: string
  allProxies: BrowserProxy[]
}

export async function fetchSubscriptionImportFromURL(targetURL: string, groupName: string): Promise<SubscriptionImportResult> {
  const result = await importBrowserProxySubscriptionFromDesktop(targetURL, groupName)
  return {
    url: String(result?.url || targetURL),
    importedCount: Number(result?.importedCount || 0),
    skippedCount: Number(result?.skippedCount || 0),
    totalCount: Number(result?.totalCount || 0),
    groupName: String(result?.groupName || groupName),
    allProxies: normalizeProxyRecords(result?.allProxies),
  }
}

export async function fixBrowserProxyNames(): Promise<{ ok: boolean; fixed: number; total: number; message?: string; error?: string }> {
  const result = await fixBrowserProxyNamesFromDesktop()
  if (!result) {
    return { ok: false, fixed: 0, total: 0, error: '调用失败' }
  }

  return {
    ok: Boolean(result.ok),
    fixed: Number(result.fixed || 0),
    total: Number(result.total || 0),
    message: result.message,
    error: result.error,
  }
}

export async function fetchClashImportFromURL(targetURL: string): Promise<ClashImportURLResult> {
  const result = await fetchBrowserProxyClashFromDesktop(targetURL)
  if (!result) {
    return {
      url: targetURL,
      content: '',
      proxyCount: 0,
    }
  }

  return {
    url: String(result.url || targetURL),
    content: String(result.content || ''),
    proxyCount: Number(result.proxyCount || 0),
    dnsServers: result.dnsServers,
    suggestedGroup: result.suggestedGroup,
    autoFallback: result.autoFallback,
    importedCount: result.importedCount,
    skippedCount: result.skippedCount,
    totalCount: result.totalCount,
    groupName: result.groupName,
    allProxies: normalizeProxyRecords(result.allProxies),
  }
}

export async function saveBrowserProxies(proxies: BrowserProxy[]): Promise<boolean> {
  await tryDesktopVoid(() => saveBrowserProxiesFromDesktop(proxies))
  mockProxies = proxies
  return true
}

export async function deleteBrowserProxy(proxyId: string): Promise<boolean> {
  try {
    await deleteBrowserProxyFromDesktop(proxyId)
  } catch (error) {
    if (hasDesktopRuntime()) {
      throw error
    }
    if (
      !(error instanceof DesktopServiceError) ||
      (error.code !== 'desktop_invoke_unavailable' && error.code !== 'desktop_command_not_ready')
    ) {
      throw error
    }
  }
  mockProxies = mockProxies.filter(p => p.proxyId !== proxyId)
  return true
}

export async function validateProxyConfig(proxyConfig: string, proxyId: string): Promise<{ supported: boolean; errorMsg: string }> {
  const result = await tryDesktop(() => validateProxyConfigFromDesktop(proxyConfig, proxyId))
  if (result) return result
  return { supported: true, errorMsg: '' }
}

export async function testProxyConnectivity(proxyId: string, proxyConfig: string): Promise<{ proxyId: string; ok: boolean; latencyMs: number; error: string }> {
  const result = await tryDesktop(() => testProxyConnectivityFromDesktop(proxyId, proxyConfig))
  if (result) return result
  // mock: simulate latency
  await new Promise(r => setTimeout(r, 300 + Math.random() * 500))
  return { proxyId, ok: true, latencyMs: Math.floor(100 + Math.random() * 200), error: '' }
}

export async function testProxyRealConnectivity(proxyId: string): Promise<{ proxyId: string; ok: boolean; latencyMs: number; error: string }> {
  const result = await tryDesktop(() => testProxyRealConnectivityFromDesktop(proxyId))
  if (result) return result
  // mock: simulate latency 300-800ms
  await new Promise(r => setTimeout(r, 300 + Math.random() * 500))
  return { proxyId, ok: true, latencyMs: Math.floor(100 + Math.random() * 400), error: '' }
}

export async function browserProxyTestSpeed(proxyId: string): Promise<{ proxyId: string; ok: boolean; latencyMs: number; error: string }> {
  const result = await tryDesktop(() => browserProxyTestSpeedFromDesktop(proxyId))
  if (result) return result
  await new Promise(r => setTimeout(r, 300 + Math.random() * 500))
  return { proxyId, ok: true, latencyMs: Math.floor(100 + Math.random() * 400), error: '' }
}

export async function browserProxyBatchTestSpeed(proxyIds: string[], concurrency: number = 20): Promise<{ proxyId: string; ok: boolean; latencyMs: number; error: string }[]> {
  const result = await tryDesktop(() => browserProxyBatchTestSpeedFromDesktop(proxyIds, concurrency))
  if (result) return result
  // mock
  await new Promise(r => setTimeout(r, 1000))
  return proxyIds.map(id => ({ proxyId: id, ok: true, latencyMs: Math.floor(100 + Math.random() * 400), error: '' }))
}

export async function browserProxyCheckIPHealth(proxyId: string): Promise<ProxyIPHealthResult> {
  const result = await tryDesktop(() => browserProxyCheckIPHealthFromDesktop(proxyId))
  if (result) return result
  await new Promise(r => setTimeout(r, 600))
  return {
    proxyId,
    ok: true,
    source: 'ippure',
    error: '',
    ip: '127.0.0.1',
    fraudScore: Math.floor(Math.random() * 100),
    isResidential: Math.random() > 0.5,
    isBroadcast: false,
    country: 'Mock',
    region: 'Mock',
    city: 'Mock',
    asOrganization: 'Mock ISP',
    rawData: {},
    updatedAt: new Date().toISOString(),
  }
}

export async function browserProxyBatchCheckIPHealth(proxyIds: string[], concurrency: number = 10): Promise<ProxyIPHealthResult[]> {
  const result = await tryDesktop(() => browserProxyBatchCheckIPHealthFromDesktop(proxyIds, concurrency))
  if (result) return result
  await new Promise(r => setTimeout(r, 1200))
  return proxyIds.map(proxyId => ({
    proxyId,
    ok: true,
    source: 'ippure',
    error: '',
    ip: '127.0.0.1',
    fraudScore: Math.floor(Math.random() * 100),
    isResidential: Math.random() > 0.5,
    isBroadcast: false,
    country: 'Mock',
    region: 'Mock',
    city: 'Mock',
    asOrganization: 'Mock ISP',
    rawData: {},
    updatedAt: new Date().toISOString(),
  }))
}

export async function openUserDataDir(userDataDir: string): Promise<boolean> {
  return await tryDesktopVoid(() => openUserDataDirFromDesktop(userDataDir))
}

export async function openCorePath(corePath: string): Promise<boolean> {
  return await tryDesktopVoid(() => openCorePathFromDesktop(corePath))
}

// ============================================================================
// Cookie API
// ============================================================================

export async function fetchBrowserCookies(profileId: string): Promise<CookieInfo[]> {
  const bindings = await getBindings()
  if (bindings?.BrowserGetCookies) {
    return (await bindings.BrowserGetCookies(profileId)) || []
  }
  // mock data
  return [
    { name: 'session', value: 'abc123', domain: '.example.com', path: '/', expires: Date.now() / 1000 + 3600, httpOnly: true, secure: true, sameSite: 'Lax' },
    { name: 'pref', value: 'dark', domain: 'example.com', path: '/', expires: -1, httpOnly: false, secure: false, sameSite: 'None' },
  ]
}

export async function clearBrowserCookies(profileId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserClearCookies) {
    await bindings.BrowserClearCookies(profileId)
    return true
  }
  return true
}

export async function exportBrowserCookies(profileId: string): Promise<string> {
  const bindings = await getBindings()
  if (bindings?.BrowserExportCookies) {
    return (await bindings.BrowserExportCookies(profileId)) || ''
  }
  return '# Netscape HTTP Cookie File\n# Generated by BrowserManager\n\n.example.com\tTRUE\t/\tTRUE\t0\tsession\tabc123\n'
}

// ============================================================================
// Snapshot API
// ============================================================================

export async function listSnapshots(profileId: string): Promise<SnapshotInfo[]> {
  const bindings = await getBindings()
  if (bindings?.BrowserSnapshotList) {
    return (await bindings.BrowserSnapshotList(profileId)) || []
  }
  return []
}

export async function createSnapshot(profileId: string, name: string): Promise<SnapshotInfo | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserSnapshotCreate) {
    return (await bindings.BrowserSnapshotCreate(profileId, name)) || null
  }
  // mock
  return {
    snapshotId: `snap-${Date.now()}`,
    profileId,
    name,
    sizeMB: 12.5,
    createdAt: new Date().toISOString(),
  }
}

export async function restoreSnapshot(profileId: string, snapshotId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserSnapshotRestore) {
    await bindings.BrowserSnapshotRestore(profileId, snapshotId)
    return true
  }
  return true
}

export async function deleteSnapshot(profileId: string, snapshotId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserSnapshotDelete) {
    await bindings.BrowserSnapshotDelete(profileId, snapshotId)
    return true
  }
  return true
}

// ============================================================================
// Bookmark API
// ============================================================================

export async function fetchBookmarks(): Promise<BrowserBookmark[]> {
  const bindings = await getBindings()
  if (bindings?.BookmarkList) {
    return (await bindings.BookmarkList()) || []
  }
  return [
    { name: 'Google', url: 'https://www.google.com/' },
    { name: 'Gmail', url: 'https://mail.google.com/' },
    { name: 'Claude', url: 'https://claude.ai/' },
    { name: 'ChatGPT', url: 'https://chatgpt.com/' },
    { name: 'YouTube', url: 'https://www.youtube.com/' },
  ]
}

export async function saveBookmarks(items: BrowserBookmark[]): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BookmarkSave) {
    await bindings.BookmarkSave(items)
    return true
  }
  return true
}

export async function resetBookmarks(): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BookmarkReset) {
    await bindings.BookmarkReset()
    return true
  }
  return true
}

// ============================================================================
// Keywords API
// ============================================================================

export async function setProfileKeywords(profileId: string, keywords: string[]): Promise<BrowserProfile | null> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileSetKeywords) {
    return (await bindings.BrowserProfileSetKeywords(profileId, keywords)) || null
  }
  mockProfiles = mockProfiles.map(p =>
    p.profileId === profileId ? { ...p, keywords, updatedAt: new Date().toISOString() } : p
  )
  return mockProfiles.find(p => p.profileId === profileId) || null
}

// ============================================================================
// LaunchCode API
// ============================================================================

export interface LaunchServerInfo {
  host: string
  port: number
  preferredPort: number
  baseUrl: string
  cdpUrl: string
  activeDebugPort: number
  ready: boolean
  apiAuth: {
    requested: boolean
    configured: boolean
    enabled: boolean
    header: string
  }
}

function normalizeLaunchServerInfo(payload: unknown): LaunchServerInfo {
  const source = readRecord(payload)
  const host = String(source.host || '127.0.0.1')
  const port = Number(source.port) || 0
  const preferredPort = Number(source.preferredPort) || 0
  const fallbackPort = preferredPort > 0 ? preferredPort : 19876
  const effectivePort = port > 0 ? port : fallbackPort
  const baseUrl = String(source.baseUrl || (effectivePort > 0 ? `http://${host}:${effectivePort}` : ''))
  const cdpUrl = String(source.cdpUrl || baseUrl)
  const activeDebugPort = Number(source.activeDebugPort) || 0
  const apiAuthPayload = readRecord(source.apiAuth)
  const apiAuth = {
    requested: !!apiAuthPayload.requested,
    configured: !!apiAuthPayload.configured,
    enabled: !!apiAuthPayload.enabled,
    header: String(apiAuthPayload.header || 'X-Personal-Pilot-Api-Key'),
  }

  return {
    host,
    port: effectivePort,
    preferredPort,
    baseUrl,
    cdpUrl,
    activeDebugPort,
    ready: !!source.ready && port > 0,
    apiAuth,
  }
}

export async function fetchLaunchServerInfo(): Promise<LaunchServerInfo> {
  const launchServerInfo = await readLaunchServerInfoFromDesktop()
  if (launchServerInfo) {
    return normalizeLaunchServerInfo(launchServerInfo)
  }

  return {
    host: '127.0.0.1',
    port: 19876,
    preferredPort: 19876,
    baseUrl: 'http://127.0.0.1:19876',
    cdpUrl: 'http://127.0.0.1:19876',
    activeDebugPort: 0,
    ready: false,
    apiAuth: {
      requested: false,
      configured: false,
      enabled: false,
      header: 'X-Personal-Pilot-Api-Key',
    },
  }
}

export async function getBrowserProfileCode(profileId: string): Promise<string> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileGetCode) {
    return (await bindings.BrowserProfileGetCode(profileId)) || ''
  }
  return ''
}

export async function regenerateBrowserProfileCode(profileId: string): Promise<string> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileRegenerateCode) {
    return (await bindings.BrowserProfileRegenerateCode(profileId)) || ''
  }
  return ''
}

export async function setBrowserProfileCode(profileId: string, code: string): Promise<string> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileSetCode) {
    return (await bindings.BrowserProfileSetCode(profileId, code)) || ''
  }
  return code.trim().toUpperCase()
}


export async function batchSetProfileTags(profileIds: string[], tags: string[], replace: boolean): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileBatchSetTags) {
    await bindings.BrowserProfileBatchSetTags(profileIds, tags, replace)
    return true
  }
  return true
}

export async function batchRemoveProfileTags(profileIds: string[], tags: string[]): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserProfileBatchRemoveTags) {
    await bindings.BrowserProfileBatchRemoveTags(profileIds, tags)
    return true
  }
  return true
}

export async function renameBrowserTag(oldName: string, newName: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserRenameTag) {
    await bindings.BrowserRenameTag(oldName, newName)
    return true
  }
  return true
}

// ============================================================================
// Group API
// ============================================================================

export async function fetchGroups(): Promise<BrowserGroupWithCount[]> {
  const bindings = await getBindings()
  if (bindings?.ListGroups) {
    return (await bindings.ListGroups()) || []
  }
  return []
}

export async function createGroup(input: BrowserGroupInput): Promise<BrowserGroup | null> {
  const bindings = await getBindings()
  if (bindings?.CreateGroup) {
    return (await bindings.CreateGroup(input)) || null
  }
  return null
}

export async function updateGroup(groupId: string, input: BrowserGroupInput): Promise<BrowserGroup | null> {
  const bindings = await getBindings()
  if (bindings?.UpdateGroup) {
    return (await bindings.UpdateGroup(groupId, input)) || null
  }
  return null
}

export async function deleteGroup(groupId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.DeleteGroup) {
    await bindings.DeleteGroup(groupId)
    return true
  }
  return false
}

export async function moveInstancesToGroup(profileIds: string[], groupId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.MoveInstancesToGroup) {
    await bindings.MoveInstancesToGroup(profileIds, groupId)
    return true
  }
  return false
}

// ============================================================================
// Behavior Recording & Playback API
// ============================================================================

function normalizeRecordingSummary(source: Partial<RecordingSummary & Recording>): RecordingSummary {
  const events = Array.isArray(source.events) ? source.events : []
  const eventCount = Number(source.eventCount ?? events.length) || 0

  return {
    id: String(source.id || ''),
    name: String(source.name || ''),
    description: String(source.description || ''),
    durationMs: Number(source.durationMs) || 0,
    viewportW: Number(source.viewportW) || 0,
    viewportH: Number(source.viewportH) || 0,
    startUrl: source.startUrl,
    currentUrl: source.currentUrl,
    title: source.title,
    devicePixelRatio: source.devicePixelRatio,
    scale: source.scale,
    createdAt: String(source.createdAt || ''),
    eventCount,
  }
}

function buildRecordingEventStats(events: RecordedEvent[], totalOverride?: number): RecordingEventStats {
  const move = events.filter(e => e.type === 'move').length
  const click = events.filter(e => e.type === 'click' || e.type === 'down' || e.type === 'up').length
  const key = events.filter(e => e.type === 'key').length
  const scroll = events.filter(e => e.type === 'scroll').length

  return {
    total: totalOverride ?? events.length,
    move,
    click,
    key,
    scroll,
  }
}

function normalizeRecordingStats(source: Partial<RecordingEventStats> | undefined, events: RecordedEvent[], total: number): RecordingEventStats {
  if (!source) return buildRecordingEventStats(events, total)
  return {
    total: Number(source.total ?? total) || 0,
    move: Number(source.move) || 0,
    click: Number(source.click) || 0,
    key: Number(source.key) || 0,
    scroll: Number(source.scroll) || 0,
  }
}

function normalizeRecordingStatus(source: Partial<ActiveRecordingStatus> | undefined): ActiveRecordingStatus {
  const profileIds = Array.isArray(source?.profileIds) ? source.profileIds.filter(Boolean).map(String) : []
  const inMemoryProfileIds = Array.isArray(source?.inMemoryProfileIds) ? source.inMemoryProfileIds.filter(Boolean).map(String) : []
  const recoverableProfileIds = Array.isArray(source?.recoverableProfileIds) ? source.recoverableProfileIds.filter(Boolean).map(String) : []
  const count = Number(source?.count ?? profileIds.length) || 0

  return {
    active: Boolean(source?.active ?? profileIds.length > 0),
    count,
    profileId: source?.profileId ? String(source.profileId) : profileIds[0],
    profileIds,
    inMemoryProfileIds,
    recoverableProfileIds,
  }
}

function normalizeRecordingDetail(payload: unknown, fallbackOffset: number, fallbackLimit: number): RecordingDetailPage | null {
  if (!payload) return null

  const envelope = readRecord(payload)
  const source = readRecord(envelope.recording || envelope.summary || payload)
  const fullEvents = Array.isArray(source.events) ? source.events as RecordedEvent[] : []
  const hasEnvelope = !!(envelope.recording || envelope.summary || envelope.eventTotal !== undefined || envelope.eventOffset !== undefined || envelope.offset !== undefined || envelope.total !== undefined)
  const hasPagedEvents = hasEnvelope && Array.isArray(envelope.events)
  const eventOffset = Math.max(0, Number(envelope.eventOffset ?? envelope.offset ?? fallbackOffset) || 0)
  const eventLimit = Math.max(1, Number(envelope.eventLimit ?? envelope.limit ?? fallbackLimit) || fallbackLimit)
  const events = hasPagedEvents
    ? envelope.events as RecordedEvent[]
    : fullEvents.slice(eventOffset, eventOffset + eventLimit)
  const eventTotal = Number(envelope.eventTotal ?? envelope.total ?? source.eventCount ?? fullEvents.length ?? events.length) || 0
  const statsSource = envelope.stats as Partial<RecordingEventStats> | undefined
  const statsEvents = fullEvents.length > 0 ? fullEvents : events

  return {
    recording: normalizeRecordingSummary({ ...source, eventCount: eventTotal }),
    events,
    eventOffset,
    eventLimit,
    eventTotal,
    stats: normalizeRecordingStats(statsSource, statsEvents, eventTotal),
  }
}

export async function startRecording(profileId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (!bindings?.BehaviorStartRecording) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.BehaviorStartRecording(profileId)
  return true
}

export async function stopRecording(profileId: string, name: string): Promise<Recording | null> {
  const bindings = await getBindings()
  if (!bindings?.BehaviorStopRecording) {
    throw new Error('Wails bindings are not available')
  }
  return (await bindings.BehaviorStopRecording(profileId, name)) || null
}

export async function fetchRecordings(): Promise<Recording[]> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingList) {
    return (await bindings.BehaviorRecordingList()) || []
  }
  return []
}

export async function fetchRecordingSummaries(): Promise<RecordingSummary[]> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingSummaryList) {
    const list = await bindings.BehaviorRecordingSummaryList()
    return (list || []).map(normalizeRecordingSummary)
  }
  if (bindings?.BehaviorRecordingList) {
    const list = await bindings.BehaviorRecordingList()
    return (list || []).map(normalizeRecordingSummary)
  }
  return []
}

export async function fetchRecordingStatus(): Promise<ActiveRecordingStatus> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingStatus) {
    return normalizeRecordingStatus(await bindings.BehaviorRecordingStatus())
  }
  if (bindings?.ActiveRecordingStatus) {
    return normalizeRecordingStatus(await bindings.ActiveRecordingStatus())
  }
  return normalizeRecordingStatus(undefined)
}

export async function deleteRecording(id: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingDelete) {
    await bindings.BehaviorRecordingDelete(id)
    return true
  }
  return false
}

export async function getRecording(id: string): Promise<Recording | null> {
  const bindings = await getBindings()
  if (bindings?.BehaviorGetRecording) {
    const recording = await bindings.BehaviorGetRecording(id)
    if (!recording) return null
    return { ...recording, events: Array.isArray(recording.events) ? recording.events : [] }
  }
  return null
}

export async function fetchRecordingDetail(
  id: string,
  options: { eventOffset?: number; eventLimit?: number } = {},
): Promise<RecordingDetailPage | null> {
  const eventOffset = Math.max(0, Math.floor(options.eventOffset ?? 0))
  const eventLimit = Math.max(1, Math.floor(options.eventLimit ?? 100))
  const bindings = await getBindings()

  if (bindings?.BehaviorGetRecordingDetail) {
    return normalizeRecordingDetail(await bindings.BehaviorGetRecordingDetail(id, eventOffset, eventLimit), eventOffset, eventLimit)
  }

  const recording = await getRecording(id)
  return normalizeRecordingDetail(recording, eventOffset, eventLimit)
}

export async function playRecording(profileId: string, recordingId: string, variation: VariationConfig): Promise<boolean> {
  const bindings = await getBindings()
  if (!bindings?.BehaviorPlayRecording) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.BehaviorPlayRecording(profileId, recordingId, variation)
  return true
}

export async function stopPlayback(profileId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BehaviorStopPlayback) {
    await bindings.BehaviorStopPlayback(profileId)
    return true
  }
  return false
}

export async function quickRecord(profileId: string): Promise<Recording | null> {
  const bindings = await getBindings()
  if (bindings?.BehaviorQuickRecord) {
    return (await bindings.BehaviorQuickRecord(profileId)) || null
  }
  return null
}

export async function fetchBehaviorPresets(): Promise<Array<{ id: string; name: string; description: string }>> {
  const bindings = await getBindings()
  if (bindings?.BehaviorPresetList) {
    return (await bindings.BehaviorPresetList()) || []
  }
  return []
}

export async function fetchSchedulerTasks(): Promise<SchedulerTaskInfo[]> {
  const bindings = await getBindings()
  if (bindings?.SchedulerListTasks) {
    return (await bindings.SchedulerListTasks()) || []
  }
  return []
}

export async function createSchedulerTask(input: SchedulerTaskInput): Promise<SchedulerTaskInfo | null> {
  const bindings = await getBindings()
  if (!bindings?.SchedulerAddTask) {
    throw new Error('Wails bindings are not available')
  }
  return (await bindings.SchedulerAddTask(new backend.SchedulerTaskInput(input))) || null
}

export async function deleteSchedulerTask(taskId: string): Promise<void> {
  const bindings = await getBindings()
  if (!bindings?.SchedulerRemoveTask) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.SchedulerRemoveTask(taskId)
}

export async function runSchedulerTaskNow(taskId: string): Promise<void> {
  const bindings = await getBindings()
  if (!bindings?.SchedulerRunTaskNow) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.SchedulerRunTaskNow(taskId)
}

export async function fetchAutomationRules(): Promise<AutomationRuleInfo[]> {
  const bindings = await getBindings()
  if (bindings?.AutomationRuleList) {
    return (await bindings.AutomationRuleList()) || []
  }
  return []
}

export async function createAutomationRule(input: AutomationRuleInput): Promise<AutomationRuleInfo | null> {
  const bindings = await getBindings()
  if (!bindings?.AutomationRuleCreate) {
    throw new Error('Wails bindings are not available')
  }
  return (await bindings.AutomationRuleCreate(input)) || null
}

export async function deleteAutomationRule(ruleId: string): Promise<void> {
  const bindings = await getBindings()
  if (!bindings?.AutomationRuleDelete) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.AutomationRuleDelete(ruleId)
}

export async function toggleAutomationRule(ruleId: string, enabled: boolean): Promise<void> {
  const bindings = await getBindings()
  if (!bindings?.AutomationRuleToggle) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.AutomationRuleToggle(ruleId, enabled)
}

export async function testFireAutomationRule(ruleId: string): Promise<void> {
  const bindings = await getBindings()
  if (!bindings?.AutomationRuleTestFire) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.AutomationRuleTestFire(ruleId)
}

export async function cleanupRecordingSessions(): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.CleanupStaleRecordingSessions) {
    await bindings.CleanupStaleRecordingSessions()
    return true
  }
  return false
}

export async function renameRecording(id: string, name: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingRename) {
    await bindings.BehaviorRecordingRename(id, name)
    return true
  }
  return false
}

export async function exportRecording(id: string): Promise<RecordingExportBundle | null> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingExport) {
    return (await bindings.BehaviorRecordingExport(id)) || null
  }
  return null
}

export async function importRecording(payload: string, name: string): Promise<Recording | null> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingImport) {
    return (await bindings.BehaviorRecordingImport(payload, name)) || null
  }
  return null
}

export async function copyRecording(id: string, name: string): Promise<Recording | null> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingCopy) {
    return (await bindings.BehaviorRecordingCopy(id, name)) || null
  }
  return null
}

export function buildPlaybackVariation(
  variation?: Partial<VariationConfig>,
  executionPermissionMode?: BehaviorExecutionPermissionMode,
): VariationConfig {
  return {
    intensity: variation?.intensity ?? 0.3,
    timingJitter: variation?.timingJitter ?? 200,
    positionJitter: variation?.positionJitter ?? 5,
    speedVariation: variation?.speedVariation ?? 0.2,
    microCorrections: variation?.microCorrections ?? true,
    extraPauses: variation?.extraPauses ?? true,
    executionPolicy: {
      permissionMode: executionPermissionMode || DEFAULT_BEHAVIOR_EXECUTION_PERMISSION_MODE,
    },
  }
}

export async function reviewPlayback(profileId: string, decision: string): Promise<void> {
  const bindings = await getBindings()
  if (bindings?.BehaviorPlaybackReview) {
    await bindings.BehaviorPlaybackReview(profileId, decision)
    return
  }
  throw new Error('当前环境不支持回放审查')
}

export async function activateBrowserProfile(profileId: string): Promise<boolean> {
  const bindings = await getBindings()
  if (bindings?.BrowserInstanceOpenUrl) {
    await bindings.BrowserInstanceOpenUrl(profileId, 'about:blank')
    return true
  }
  return false
}

export async function trimRecording(id: string, startEvent: number, endEvent: number, name: string): Promise<Recording | null> {
  const bindings = await getBindings()
  if (bindings?.BehaviorRecordingTrim) {
    return (await bindings.BehaviorRecordingTrim(id, startEvent, endEvent, name)) || null
  }
  return null
}

export function onPlaybackEvents(callbacks: {
  onProgress?: (payload: PlaybackProgressPayload) => void
  onCompleted?: (payload: PlaybackEventPayload) => void
  onFailed?: (payload: PlaybackEventPayload) => void
}): Unsubscribe {
  const offs: Unsubscribe[] = []
  if (callbacks.onProgress) {
    offs.push(onRuntimeEvent('automation:playback:progress', callbacks.onProgress))
  }
  if (callbacks.onCompleted) {
    offs.push(onRuntimeEvent('automation:playback:completed', callbacks.onCompleted))
  }
  if (callbacks.onFailed) {
    offs.push(onRuntimeEvent('automation:playback:failed', callbacks.onFailed))
  }
  return combineUnsubscribes(offs)
}
