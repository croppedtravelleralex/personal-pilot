import { EventsOn } from '../../wailsjs/runtime'
import type {
  BrowserProfile, BrowserProfileInput, BrowserTab, BrowserSettings, BrowserCore, BrowserCoreInput,
  BrowserCoreValidateResult, BrowserProxy, BrowserCoreExtended, CookieInfo, SnapshotInfo,
  BrowserBookmark, BrowserGroup, BrowserGroupInput, BrowserGroupWithCount, ProxyIPHealthResult,
  ActiveRecordingStatus, Recording, RecordingDetailPage, RecordingEventStats, RecordingSummary, RecordedEvent,
  NaturalLanguageAction, NaturalLanguageTaskEvent, PlaybackEventPayload, PlaybackProgressPayload,
  RecordingExportBundle, VariationConfig,
} from './types'

type Unsubscribe = () => void

const getBindings = async () => {
  try {
    return await import('../../wailsjs/go/main/App')
  } catch {
    return null
  }
}

function noop() {}

function onRuntimeEvent<T>(eventName: string, callback: (payload: T) => void): Unsubscribe {
  try {
    return EventsOn(eventName, (payload: T) => callback(payload))
  } catch {
    return noop
  }
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileList) {
    return (await bindings.BrowserProfileList()) || []
  }
  return mockProfiles
}

export async function fetchBrowserProfilesByTag(tag: string): Promise<BrowserProfile[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileListByTag) {
    return (await bindings.BrowserProfileListByTag(tag)) || []
  }
  return mockProfiles.filter(p => p.tags?.includes(tag))
}

export async function fetchAllTags(): Promise<string[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserGetAllTags) {
    return (await bindings.BrowserGetAllTags()) || []
  }
  const set = new Set<string>()
  mockProfiles.forEach(p => p.tags?.forEach(t => set.add(t)))
  return Array.from(set).sort()
}

export async function createBrowserProfile(input: BrowserProfileInput): Promise<BrowserProfile | null> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileUpdate) {
    return (await bindings.BrowserProfileUpdate(profileId, input)) || null
  }
  const index = mockProfiles.findIndex(item => item.profileId === profileId)
  if (index === -1) return null
  mockProfiles[index] = { ...mockProfiles[index], ...input, updatedAt: new Date().toISOString() }
  return mockProfiles[index]
}

export async function deleteBrowserProfile(profileId: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileDelete) {
    await bindings.BrowserProfileDelete(profileId)
    return true
  }
  mockProfiles = mockProfiles.filter(item => item.profileId !== profileId)
  return true
}

export async function copyBrowserProfile(profileId: string, newName: string): Promise<BrowserProfile | null> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserInstanceStart) {
    return (await bindings.BrowserInstanceStart(profileId)) || null
  }
  mockProfiles = mockProfiles.map(item =>
    item.profileId === profileId ? { ...item, running: true, debugPort: 9222, debugReady: true, pid: Math.floor(Math.random() * 100000), runtimeWarning: '', lastStartAt: new Date().toISOString() } : item
  )
  return mockProfiles.find(item => item.profileId === profileId) || null
}

export async function startBrowserInstanceByCode(code: string): Promise<BrowserProfile | null> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserInstanceStop) {
    return (await bindings.BrowserInstanceStop(profileId)) || null
  }
  mockProfiles = mockProfiles.map(item =>
    item.profileId === profileId ? { ...item, running: false, debugReady: false, debugPort: 0, pid: 0, runtimeWarning: '', lastStopAt: new Date().toISOString() } : item
  )
  return mockProfiles.find(item => item.profileId === profileId) || null
}

export async function restartBrowserInstance(profileId: string): Promise<BrowserProfile | null> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserInstanceRestart) {
    return (await bindings.BrowserInstanceRestart(profileId)) || null
  }
  await stopBrowserInstance(profileId)
  return await startBrowserInstance(profileId)
}

export async function openBrowserUrl(profileId: string, targetUrl: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserInstanceOpenUrl) {
    return (await bindings.BrowserInstanceOpenUrl(profileId, targetUrl)) === true
  }
  return true
}

export async function fetchBrowserTabs(profileId: string): Promise<BrowserTab[]> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
  if (bindings?.GetBrowserSettings) {
    return (await bindings.GetBrowserSettings()) || { userDataRoot: 'data', defaultFingerprintArgs: [], defaultLaunchArgs: [], defaultProxy: '', startReadyTimeoutMs: 3000, startStableWindowMs: 1200 }
  }
  return { userDataRoot: 'data', defaultFingerprintArgs: [], defaultLaunchArgs: [], defaultProxy: '', startReadyTimeoutMs: 3000, startStableWindowMs: 1200 }
}

export async function saveBrowserSettings(settings: BrowserSettings): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.SaveBrowserSettings) {
    await bindings.SaveBrowserSettings(settings)
    return true
  }
  return true
}

// ============================================================================
// Core API
// ============================================================================

export async function fetchBrowserCores(): Promise<BrowserCore[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserCoreList) {
    return (await bindings.BrowserCoreList()) || []
  }
  return mockCores
}

export async function saveBrowserCore(input: BrowserCoreInput): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserCoreSave) {
    await bindings.BrowserCoreSave(input)
    return true
  }
  const index = mockCores.findIndex(c => c.coreId === input.coreId)
  if (index >= 0) {
    mockCores[index] = input
  } else {
    mockCores.push({ ...input, coreId: input.coreId || `core-${Date.now()}` })
  }
  return true
}

export async function deleteBrowserCore(coreId: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserCoreDelete) {
    await bindings.BrowserCoreDelete(coreId)
    return true
  }
  mockCores = mockCores.filter(c => c.coreId !== coreId)
  return true
}

export async function setDefaultBrowserCore(coreId: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserCoreSetDefault) {
    await bindings.BrowserCoreSetDefault(coreId)
    return true
  }
  mockCores = mockCores.map(c => ({ ...c, isDefault: c.coreId === coreId }))
  return true
}

export async function validateBrowserCorePath(corePath: string): Promise<BrowserCoreValidateResult> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserCoreValidate) {
    return (await bindings.BrowserCoreValidate(corePath)) || { valid: false, message: '验证失败' }
  }
  return { valid: true, message: '路径有效（模拟）' }
}

export async function fetchCoreExtendedInfo(): Promise<BrowserCoreExtended[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserCoreExtendedInfo) {
    return (await bindings.BrowserCoreExtendedInfo()) || []
  }
  return []
}

export async function scanBrowserCores(): Promise<BrowserCore[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserCoreScan) {
    return (await bindings.BrowserCoreScan()) || []
  }
  return mockCores
}

export async function BrowserCoreDownload(coreName: string, url: string, proxyConfig?: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserCoreDownload) {
    await bindings.BrowserCoreDownload(coreName, url, proxyConfig || '')
    return true
  }
  return true
}

// ============================================================================
// Proxy API
// ============================================================================

export async function fetchBrowserProxies(): Promise<BrowserProxy[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyList) {
    return (await bindings.BrowserProxyList()) || []
  }
  return mockProxies
}

export async function fetchBrowserProxyGroups(): Promise<string[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyListGroups) {
    return (await bindings.BrowserProxyListGroups()) || []
  }
  return []
}

export async function fetchBrowserProxiesByGroup(groupName: string): Promise<BrowserProxy[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyListByGroup) {
    return (await bindings.BrowserProxyListByGroup(groupName)) || []
  }
  return mockProxies.filter(p => p.groupName === groupName)
}

export interface ClashImportURLResult {
  url: string
  content: string
  proxyCount: number
  dnsServers?: string
  suggestedGroup?: string
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyImportSubscriptionByURL) {
    const result = await bindings.BrowserProxyImportSubscriptionByURL(targetURL, groupName)
    return {
      url: String(result?.url || targetURL),
      importedCount: Number(result?.importedCount || 0),
      skippedCount: Number(result?.skippedCount || 0),
      totalCount: Number(result?.totalCount || 0),
      groupName: String(result?.groupName || groupName),
      allProxies: (result?.allProxies || []) as BrowserProxy[],
    }
  }

  const goApp = (window as any).go?.main?.App
  if (goApp?.BrowserProxyImportSubscriptionByURL) {
    const result = await goApp.BrowserProxyImportSubscriptionByURL(targetURL, groupName)
    return {
      url: String(result?.url || targetURL),
      importedCount: Number(result?.importedCount || 0),
      skippedCount: Number(result?.skippedCount || 0),
      totalCount: Number(result?.totalCount || 0),
      groupName: String(result?.groupName || groupName),
      allProxies: (result?.allProxies || []) as BrowserProxy[],
    }
  }

  throw new Error('当前环境不支持订阅 URL 导入')
}

export async function fetchClashImportFromURL(targetURL: string): Promise<ClashImportURLResult> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyFetchClashByURL) {
    return (await bindings.BrowserProxyFetchClashByURL(targetURL)) || {
      url: targetURL,
      content: '',
      proxyCount: 0,
    }
  }

  // 兜底：wailsjs 尚未刷新时，直接通过 window.go 调用后端绑定
  const goApp = (window as any).go?.main?.App
  if (goApp?.BrowserProxyFetchClashByURL) {
    return (await goApp.BrowserProxyFetchClashByURL(targetURL)) || {
      url: targetURL,
      content: '',
      proxyCount: 0,
    }
  }

  throw new Error('当前环境不支持 URL 导入 Clash 配置')
}

export async function saveBrowserProxies(proxies: BrowserProxy[]): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.SaveBrowserProxies) {
    await bindings.SaveBrowserProxies(proxies)
    return true
  }
  mockProxies = proxies
  return true
}

export async function validateProxyConfig(proxyConfig: string, proxyId: string): Promise<{ supported: boolean; errorMsg: string }> {
  const bindings: any = await getBindings()
  if (bindings?.ValidateProxyConfig) {
    return (await bindings.ValidateProxyConfig(proxyConfig, proxyId)) || { supported: true, errorMsg: '' }
  }
  return { supported: true, errorMsg: '' }
}

export async function testProxyConnectivity(proxyId: string, proxyConfig: string): Promise<{ proxyId: string; ok: boolean; latencyMs: number; error: string }> {
  const bindings: any = await getBindings()
  if (bindings?.TestProxyConnectivity) {
    return (await bindings.TestProxyConnectivity(proxyId, proxyConfig)) || { proxyId, ok: false, latencyMs: 0, error: '调用失败' }
  }
  // mock: simulate latency
  await new Promise(r => setTimeout(r, 300 + Math.random() * 500))
  return { proxyId, ok: true, latencyMs: Math.floor(100 + Math.random() * 200), error: '' }
}

export async function testProxyRealConnectivity(proxyId: string): Promise<{ proxyId: string; ok: boolean; latencyMs: number; error: string }> {
  const bindings: any = await getBindings()
  if (bindings?.TestProxyRealConnectivity) {
    return (await bindings.TestProxyRealConnectivity(proxyId)) || { proxyId, ok: false, latencyMs: 0, error: '调用失败' }
  }
  // mock: simulate latency 300-800ms
  await new Promise(r => setTimeout(r, 300 + Math.random() * 500))
  return { proxyId, ok: true, latencyMs: Math.floor(100 + Math.random() * 400), error: '' }
}

export async function browserProxyTestSpeed(proxyId: string): Promise<{ proxyId: string; ok: boolean; latencyMs: number; error: string }> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyTestSpeed) {
    return (await bindings.BrowserProxyTestSpeed(proxyId)) || { proxyId, ok: false, latencyMs: 0, error: '调用失败' }
  }
  await new Promise(r => setTimeout(r, 300 + Math.random() * 500))
  return { proxyId, ok: true, latencyMs: Math.floor(100 + Math.random() * 400), error: '' }
}

export async function browserProxyBatchTestSpeed(proxyIds: string[], concurrency: number = 20): Promise<{ proxyId: string; ok: boolean; latencyMs: number; error: string }[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyBatchTestSpeed) {
    return (await bindings.BrowserProxyBatchTestSpeed(proxyIds, concurrency)) || []
  }
  // mock
  await new Promise(r => setTimeout(r, 1000))
  return proxyIds.map(id => ({ proxyId: id, ok: true, latencyMs: Math.floor(100 + Math.random() * 400), error: '' }))
}

export async function browserProxyCheckIPHealth(proxyId: string): Promise<ProxyIPHealthResult> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyCheckIPHealth) {
    return (await bindings.BrowserProxyCheckIPHealth(proxyId)) || {
      proxyId,
      ok: false,
      source: 'ippure',
      error: '调用失败',
      ip: '',
      fraudScore: 0,
      isResidential: false,
      isBroadcast: false,
      country: '',
      region: '',
      city: '',
      asOrganization: '',
      rawData: {},
      updatedAt: new Date().toISOString(),
    }
  }
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserProxyBatchCheckIPHealth) {
    return (await bindings.BrowserProxyBatchCheckIPHealth(proxyIds, concurrency)) || []
  }
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
  const bindings: any = await getBindings()
  if (bindings?.OpenUserDataDir) {
    await bindings.OpenUserDataDir(userDataDir)
    return true
  }
  return false
}

export async function openCorePath(corePath: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.OpenCorePath) {
    await bindings.OpenCorePath(corePath)
    return true
  }
  return false
}

// ============================================================================
// Cookie API
// ============================================================================

export async function fetchBrowserCookies(profileId: string): Promise<CookieInfo[]> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserClearCookies) {
    await bindings.BrowserClearCookies(profileId)
    return true
  }
  return true
}

export async function exportBrowserCookies(profileId: string): Promise<string> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserExportCookies) {
    return (await bindings.BrowserExportCookies(profileId)) || ''
  }
  return '# Netscape HTTP Cookie File\n# Generated by BrowserManager\n\n.example.com\tTRUE\t/\tTRUE\t0\tsession\tabc123\n'
}

// ============================================================================
// Snapshot API
// ============================================================================

export async function listSnapshots(profileId: string): Promise<SnapshotInfo[]> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserSnapshotList) {
    return (await bindings.BrowserSnapshotList(profileId)) || []
  }
  return []
}

export async function createSnapshot(profileId: string, name: string): Promise<SnapshotInfo | null> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserSnapshotRestore) {
    await bindings.BrowserSnapshotRestore(profileId, snapshotId)
    return true
  }
  return true
}

export async function deleteSnapshot(profileId: string, snapshotId: string): Promise<boolean> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
  if (bindings?.BookmarkSave) {
    await bindings.BookmarkSave(items)
    return true
  }
  return true
}

export async function resetBookmarks(): Promise<boolean> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
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

function normalizeLaunchServerInfo(payload: any): LaunchServerInfo {
  const host = String(payload?.host || '127.0.0.1')
  const port = Number(payload?.port) || 0
  const preferredPort = Number(payload?.preferredPort) || 0
  const fallbackPort = preferredPort > 0 ? preferredPort : 19876
  const effectivePort = port > 0 ? port : fallbackPort
  const baseUrl = String(payload?.baseUrl || (effectivePort > 0 ? `http://${host}:${effectivePort}` : ''))
  const cdpUrl = String(payload?.cdpUrl || baseUrl)
  const activeDebugPort = Number(payload?.activeDebugPort) || 0
  const apiAuthPayload = payload?.apiAuth || {}
  const apiAuth = {
    requested: !!apiAuthPayload?.requested,
    configured: !!apiAuthPayload?.configured,
    enabled: !!apiAuthPayload?.enabled,
    header: String(apiAuthPayload?.header || 'X-Personal-Pilot-Api-Key'),
  }

  return {
    host,
    port: effectivePort,
    preferredPort,
    baseUrl,
    cdpUrl,
    activeDebugPort,
    ready: !!payload?.ready && port > 0,
    apiAuth,
  }
}

export async function fetchLaunchServerInfo(): Promise<LaunchServerInfo> {
  const bindings: any = await getBindings()
  if (bindings?.GetLaunchServerInfo) {
    return normalizeLaunchServerInfo(await bindings.GetLaunchServerInfo())
  }

  const goApp = (window as any).go?.main?.App
  if (goApp?.GetLaunchServerInfo) {
    return normalizeLaunchServerInfo(await goApp.GetLaunchServerInfo())
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
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileGetCode) {
    return (await bindings.BrowserProfileGetCode(profileId)) || ''
  }
  return ''
}

export async function regenerateBrowserProfileCode(profileId: string): Promise<string> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileRegenerateCode) {
    return (await bindings.BrowserProfileRegenerateCode(profileId)) || ''
  }
  return ''
}

export async function setBrowserProfileCode(profileId: string, code: string): Promise<string> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileSetCode) {
    return (await bindings.BrowserProfileSetCode(profileId, code)) || ''
  }
  return code.trim().toUpperCase()
}


export async function batchSetProfileTags(profileIds: string[], tags: string[], replace: boolean): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileBatchSetTags) {
    await bindings.BrowserProfileBatchSetTags(profileIds, tags, replace)
    return true
  }
  return true
}

export async function batchRemoveProfileTags(profileIds: string[], tags: string[]): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BrowserProfileBatchRemoveTags) {
    await bindings.BrowserProfileBatchRemoveTags(profileIds, tags)
    return true
  }
  return true
}

export async function renameBrowserTag(oldName: string, newName: string): Promise<boolean> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()
  if (bindings?.ListGroups) {
    return (await bindings.ListGroups()) || []
  }
  return []
}

export async function createGroup(input: BrowserGroupInput): Promise<BrowserGroup | null> {
  const bindings: any = await getBindings()
  if (bindings?.CreateGroup) {
    return (await bindings.CreateGroup(input)) || null
  }
  return null
}

export async function updateGroup(groupId: string, input: BrowserGroupInput): Promise<BrowserGroup | null> {
  const bindings: any = await getBindings()
  if (bindings?.UpdateGroup) {
    return (await bindings.UpdateGroup(groupId, input)) || null
  }
  return null
}

export async function deleteGroup(groupId: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.DeleteGroup) {
    await bindings.DeleteGroup(groupId)
    return true
  }
  return false
}

export async function moveInstancesToGroup(profileIds: string[], groupId: string): Promise<boolean> {
  const bindings: any = await getBindings()
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

function normalizeRecordingDetail(payload: any, fallbackOffset: number, fallbackLimit: number): RecordingDetailPage | null {
  if (!payload) return null

  const source = payload.recording || payload.summary || payload
  const fullEvents = Array.isArray(source.events) ? source.events : []
  const hasEnvelope = !!(payload.recording || payload.summary || payload.eventTotal !== undefined || payload.eventOffset !== undefined || payload.offset !== undefined || payload.total !== undefined)
  const hasPagedEvents = hasEnvelope && Array.isArray(payload.events)
  const eventOffset = Math.max(0, Number(payload.eventOffset ?? payload.offset ?? fallbackOffset) || 0)
  const eventLimit = Math.max(1, Number(payload.eventLimit ?? payload.limit ?? fallbackLimit) || fallbackLimit)
  const events = hasPagedEvents
    ? payload.events
    : fullEvents.slice(eventOffset, eventOffset + eventLimit)
  const eventTotal = Number(payload.eventTotal ?? payload.total ?? source.eventCount ?? fullEvents.length ?? events.length) || 0
  const statsSource = payload.stats as Partial<RecordingEventStats> | undefined
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

function normalizeNaturalLanguageAction(action: Record<string, unknown>): NaturalLanguageAction {
  const rawType = String(action.type || 'wait')
  const allowedTypes = new Set<NaturalLanguageAction['type']>(['goto', 'click', 'scroll', 'type', 'wait'])
  const type = allowedTypes.has(rawType as NaturalLanguageAction['type'])
    ? rawType as NaturalLanguageAction['type']
    : 'wait'

  return {
    type,
    description: typeof action.description === 'string' ? action.description : undefined,
    selector: typeof action.selector === 'string' ? action.selector : undefined,
    url: typeof action.url === 'string' ? action.url : undefined,
    text: typeof action.text === 'string' ? action.text : undefined,
    durationMs: typeof action.durationMs === 'number' ? action.durationMs : undefined,
  }
}

export async function startRecording(profileId: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (!bindings?.BehaviorStartRecording) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.BehaviorStartRecording(profileId)
  return true
}

export async function stopRecording(profileId: string, name: string): Promise<Recording | null> {
  const bindings: any = await getBindings()
  if (!bindings?.BehaviorStopRecording) {
    throw new Error('Wails bindings are not available')
  }
  return (await bindings.BehaviorStopRecording(profileId, name)) || null
}

export async function fetchRecordings(): Promise<Recording[]> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorRecordingList) {
    return (await bindings.BehaviorRecordingList()) || []
  }
  return []
}

export async function fetchRecordingSummaries(): Promise<RecordingSummary[]> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorRecordingSummaryList) {
    const list = await bindings.BehaviorRecordingSummaryList()
    return (list || []).map(normalizeRecordingSummary)
  }
  if (bindings?.BehaviorRecordingMetaList) {
    const list = await bindings.BehaviorRecordingMetaList()
    return (list || []).map(normalizeRecordingSummary)
  }
  if (bindings?.BehaviorRecordingList) {
    const list = await bindings.BehaviorRecordingList()
    return (list || []).map(normalizeRecordingSummary)
  }
  return []
}

export async function fetchRecordingStatus(): Promise<ActiveRecordingStatus> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorRecordingStatus) {
    return normalizeRecordingStatus(await bindings.BehaviorRecordingStatus())
  }
  if (bindings?.ActiveRecordingStatus) {
    return normalizeRecordingStatus(await bindings.ActiveRecordingStatus())
  }
  return normalizeRecordingStatus(undefined)
}

export async function deleteRecording(id: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorRecordingDelete) {
    await bindings.BehaviorRecordingDelete(id)
    return true
  }
  return false
}

export async function getRecording(id: string): Promise<Recording | null> {
  const bindings: any = await getBindings()
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
  const bindings: any = await getBindings()

  if (bindings?.BehaviorGetRecordingDetail) {
    return normalizeRecordingDetail(await bindings.BehaviorGetRecordingDetail(id, eventOffset, eventLimit), eventOffset, eventLimit)
  }

  if (bindings?.BehaviorGetRecordingMeta && bindings?.BehaviorGetRecordingEvents) {
    const [summary, eventPage] = await Promise.all([
      bindings.BehaviorGetRecordingMeta(id),
      bindings.BehaviorGetRecordingEvents(id, eventOffset, eventLimit),
    ])
    return normalizeRecordingDetail({ recording: summary, ...eventPage }, eventOffset, eventLimit)
  }

  const recording = await getRecording(id)
  return normalizeRecordingDetail(recording, eventOffset, eventLimit)
}

export async function playRecording(profileId: string, recordingId: string, variation: VariationConfig): Promise<boolean> {
  const bindings: any = await getBindings()
  if (!bindings?.BehaviorPlayRecording) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.BehaviorPlayRecording(profileId, recordingId, variation)
  return true
}

export async function stopPlayback(profileId: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorStopPlayback) {
    await bindings.BehaviorStopPlayback(profileId)
    return true
  }
  return false
}

export async function quickRecord(profileId: string): Promise<Recording | null> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorQuickRecord) {
    return (await bindings.BehaviorQuickRecord(profileId)) || null
  }
  return null
}

export async function fetchBehaviorPresets(): Promise<Array<{ id: string; name: string; description: string }>> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorPresetList) {
    return (await bindings.BehaviorPresetList()) || []
  }
  return []
}

export async function cleanupRecordingSessions(): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.CleanupStaleRecordingSessions) {
    await bindings.CleanupStaleRecordingSessions()
    return true
  }
  return false
}

export async function renameRecording(id: string, name: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorRecordingRename) {
    await bindings.BehaviorRecordingRename(id, name)
    return true
  }
  return false
}

export async function exportRecording(id: string): Promise<RecordingExportBundle | null> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorRecordingExport) {
    return (await bindings.BehaviorRecordingExport(id)) || null
  }
  return null
}

export async function importRecording(payload: string, name: string): Promise<Recording | null> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorRecordingImport) {
    return (await bindings.BehaviorRecordingImport(payload, name)) || null
  }
  return null
}

export async function copyRecording(id: string, name: string): Promise<Recording | null> {
  const bindings: any = await getBindings()
  if (bindings?.BehaviorRecordingCopy) {
    return (await bindings.BehaviorRecordingCopy(id, name)) || null
  }
  return null
}

export async function trimRecording(id: string, startEvent: number, endEvent: number, name: string): Promise<Recording | null> {
  const bindings: any = await getBindings()
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

export async function planNaturalLanguageTask(taskDescription: string): Promise<NaturalLanguageAction[]> {
  const bindings: any = await getBindings()
  if (!bindings?.LLMPlanOnly) {
    throw new Error('Wails bindings are not available')
  }
  const actions = await bindings.LLMPlanOnly(taskDescription)
  return (actions || []).map((action: Record<string, unknown>) => normalizeNaturalLanguageAction(action))
}

export async function executeNaturalLanguageTask(profileId: string, taskDescription: string): Promise<boolean> {
  const bindings: any = await getBindings()
  if (!bindings?.LLMExecuteTask) {
    throw new Error('Wails bindings are not available')
  }
  await bindings.LLMExecuteTask(profileId, taskDescription)
  return true
}

export function onNaturalLanguageTaskEvents(callbacks: {
  onPlanning?: (payload: NaturalLanguageTaskEvent) => void
  onPlanReady?: (payload: NaturalLanguageTaskEvent) => void
  onExecuting?: (payload: NaturalLanguageTaskEvent) => void
  onStepDone?: (payload: NaturalLanguageTaskEvent) => void
  onComplete?: (payload: NaturalLanguageTaskEvent) => void
  onFailed?: (payload: NaturalLanguageTaskEvent) => void
  onCancelled?: (payload: NaturalLanguageTaskEvent) => void
}): Unsubscribe {
  const offs: Unsubscribe[] = []
  if (callbacks.onPlanning) offs.push(onRuntimeEvent('llm:task:planning', callbacks.onPlanning))
  if (callbacks.onPlanReady) offs.push(onRuntimeEvent('llm:task:plan-ready', callbacks.onPlanReady))
  if (callbacks.onExecuting) offs.push(onRuntimeEvent('llm:task:executing', callbacks.onExecuting))
  if (callbacks.onStepDone) offs.push(onRuntimeEvent('llm:task:step-done', callbacks.onStepDone))
  if (callbacks.onComplete) offs.push(onRuntimeEvent('llm:task:complete', callbacks.onComplete))
  if (callbacks.onFailed) offs.push(onRuntimeEvent('llm:task:failed', callbacks.onFailed))
  if (callbacks.onCancelled) {
    offs.push(onRuntimeEvent('llm:task:cancelled', callbacks.onCancelled))
    offs.push(onRuntimeEvent('llm:task:canceled', callbacks.onCancelled))
  }
  return combineUnsubscribes(offs)
}
