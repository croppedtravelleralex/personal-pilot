import { EventsOn } from '../../wailsjs/runtime'
import { desktopRpc } from '../../services/desktop'
import type { BrowserProfile } from '../browser/types'
import type {
  BrowserInstanceLifecycleEvent,
  BrowserInstanceLifecycleEventName,
  FingerprintHealthCheckStatus,
  FingerprintHealthLevel,
  SyncGroup,
  SyncOperation,
  SyncWindowPlacement,
  WorkbenchFingerprintHealthCheck,
  WorkbenchFingerprintHealthProfile,
  WorkbenchFingerprintSnapshot,
  WorkbenchIdentityStrengthReport,
  WorkbenchTask,
} from './types'

// Sidecar RPC stays routed through services/desktop.ts, which owns Tauri invoke.
const backendCall = <T>(name: string, ...args: unknown[]): Promise<T> => desktopRpc<T>(name, args)

type Unsubscribe = () => void

const INSTANCE_EVENT_NAMES: BrowserInstanceLifecycleEventName[] = [
  'browser:instance:started',
  'browser:instance:updated',
  'browser:instance:stopped',
  'browser:instance:crashed',
]

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
    offs.forEach((off) => {
      try {
        off()
      } catch {
        // Runtime event cleanup should never break page unmount.
      }
    })
  }
}

function normalizeErrorMessage(error: unknown) {
  if (error instanceof Error) return error.message
  if (typeof error === 'string') return error
  return ''
}

function isMissingBackendFunction(error: unknown) {
  const message = normalizeErrorMessage(error).toLowerCase()
  return (
    message.includes('not found') ||
    message.includes('unknown method') ||
    message.includes('unsupported method') ||
    message.includes('no method')
  )
}

function readRecord(payload: unknown): Record<string, unknown> {
  return payload && typeof payload === 'object' ? payload as Record<string, unknown> : {}
}

function readString(source: Record<string, unknown>, keys: string[], fallback = '') {
  for (const key of keys) {
    const value = source[key]
    if (typeof value === 'string' && value.trim()) return value
  }
  return fallback
}

function readNumber(source: Record<string, unknown>, keys: string[], fallback = 0) {
  for (const key of keys) {
    const value = source[key]
    if (typeof value === 'number' && Number.isFinite(value)) return value
    if (typeof value === 'string' && value.trim() !== '') {
      const parsed = Number(value)
      if (Number.isFinite(parsed)) return parsed
    }
  }
  return fallback
}

function readOptionalNumber(source: Record<string, unknown>, keys: string[]) {
  for (const key of keys) {
    const value = source[key]
    if (typeof value === 'number' && Number.isFinite(value)) return value
    if (typeof value === 'string' && value.trim() !== '') {
      const parsed = Number(value)
      if (Number.isFinite(parsed)) return parsed
    }
  }
  return undefined
}

function clampScore(score: number) {
  if (!Number.isFinite(score)) return 0
  return Math.max(0, Math.min(100, Math.round(score)))
}

function levelFromScore(score: number): FingerprintHealthLevel {
  if (score >= 90) return 'good'
  if (score >= 70) return 'warning'
  return 'risk'
}

function normalizeHealthLevel(value: unknown, score: number): FingerprintHealthLevel {
  const text = String(value || '').toLowerCase()
  if (text === 'good' || text === 'coherent' || text === 'healthy' || text === 'pass') return 'good'
  if (text === 'warning' || text === 'warn' || text === 'suspicious') return 'warning'
  if (text === 'risk' || text === 'inconsistent' || text === 'danger' || text === 'bad' || text === 'error' || text === 'fail') return 'risk'
  if (text === 'unknown') return 'unknown'
  return levelFromScore(score)
}

function normalizeCheckStatus(value: unknown, legacyPassed?: boolean): FingerprintHealthCheckStatus {
  const text = String(value || '').toLowerCase()
  if (text === 'pass' || text === 'passed' || text === 'ok' || text === 'success' || text === 'good') return 'pass'
  if (text === 'warning' || text === 'warn') return 'warning'
  if (text === 'fail' || text === 'failed' || text === 'error' || text === 'risk') return 'fail'
  if (text === 'info') return 'info'
  if (legacyPassed === true) return 'pass'
  if (legacyPassed === false) {
    if (text.includes('soft') || text.includes('warn')) return 'warning'
    if (text.includes('info')) return 'info'
    return 'fail'
  }
  if (text.includes('hard') || text.includes('danger') || text.includes('bad') || text.includes('inconsistent')) return 'fail'
  if (text.includes('soft') || text.includes('suspicious')) return 'warning'
  return 'info'
}

function normalizeFingerprintHealthCheck(payload: unknown): WorkbenchFingerprintHealthCheck {
  const source = readRecord(payload)
  const legacyPassed = source.passed === undefined && source.ok === undefined && source.success === undefined
    ? undefined
    : Boolean(source.passed ?? source.ok ?? source.success)
  const check: WorkbenchFingerprintHealthCheck = {
    id: readString(source, ['id', 'checkId', 'dimension', 'name', 'key'], 'unknown'),
    status: normalizeCheckStatus(source.status ?? source.severity, legacyPassed),
    message: readString(source, ['message', 'detail', 'summary'], '-'),
  }
  const expected = readString(source, ['expected'])
  const actual = readString(source, ['actual', 'actualValue'])
  const penalty = readOptionalNumber(source, ['penalty'])
  if (expected) check.expected = expected
  if (actual) check.actual = actual
  if (penalty !== undefined) check.penalty = Math.max(0, Math.round(penalty))
  return check
}

function normalizeFingerprintHealthProfile(
  profileId: string,
  payload: unknown,
): WorkbenchFingerprintHealthProfile {
  const source = readRecord(payload)
  const rawChecks = Array.isArray(source.checks)
    ? source.checks
    : Array.isArray(source.checkItems)
      ? source.checkItems
      : []
  const checks = rawChecks.map(normalizeFingerprintHealthCheck)
  const score = clampScore(readNumber(source, ['score', 'coherenceScore', 'healthScore'], checks.length > 0 ? 100 : 0))
  const fingerprint = readRecord(source.fingerprint ?? source.snapshot)

  return {
    profileId: readString(source, ['profileId', 'profileID'], profileId),
    profileName: readString(source, ['profileName'], '') || undefined,
    score,
    level: normalizeHealthLevel(source.level ?? source.status, score),
    checks,
    fingerprint: Object.keys(fingerprint).length > 0 ? fingerprint as WorkbenchFingerprintSnapshot : undefined,
    capturedAt: readString(source, ['capturedAt', 'updatedAt', 'checkedAt'], new Date().toISOString()),
    source: readString(source, ['source'], 'local-cdp'),
    error: readString(source, ['error'], '') || undefined,
  }
}

function normalizeIdentityReport(
  profileId: string,
  payload: unknown,
): WorkbenchIdentityStrengthReport {
  const source = readRecord(payload)
  const rawSubscores = readRecord(source.subscores)
  const rawDimensions = Array.isArray(source.dimensions) ? source.dimensions : []
  const rawSummary = Array.isArray(source.summary) ? source.summary : []
  const score = clampScore(readNumber(source, ['score'], 0))
  const fingerprint = readRecord(source.fingerprint ?? source.snapshot)

  return {
    profileId: readString(source, ['profileId', 'profileID'], profileId),
    profileName: readString(source, ['profileName'], '') || undefined,
    score,
    level: normalizeIdentityLevel(source.level, score),
    subscores: {
      fingerprintVisible: clampScore(readNumber(rawSubscores, ['fingerprintVisible'], 0)),
      consistency: clampScore(readNumber(rawSubscores, ['consistency'], 0)),
      profilePersistence: clampScore(readNumber(rawSubscores, ['profilePersistence'], 0)),
      proxyNetwork: clampScore(readNumber(rawSubscores, ['proxyNetwork'], 0)),
      behaviorNaturalness: clampScore(readNumber(rawSubscores, ['behaviorNaturalness'], 0)),
      automationSafety: clampScore(readNumber(rawSubscores, ['automationSafety'], 0)),
    },
    dimensions: rawDimensions.map(normalizeIdentityDimension),
    fingerprint: Object.keys(fingerprint).length > 0 ? fingerprint as WorkbenchFingerprintSnapshot : undefined,
    capturedAt: readString(source, ['capturedAt', 'updatedAt', 'checkedAt'], new Date().toISOString()),
    source: readString(source, ['source'], 'local-cdp'),
    summary: rawSummary.filter((item): item is string => typeof item === 'string' && item.trim().length > 0),
  }
}

function normalizeIdentityLevel(value: unknown, score: number): WorkbenchIdentityStrengthReport['level'] {
  const text = String(value || '').toLowerCase()
  if (text === 'strong') return 'strong'
  if (text === 'normal' || text === 'good') return 'normal'
  if (text === 'weak' || text === 'warning') return 'weak'
  if (text === 'risk' || text === 'fail' || text === 'failed') return 'risk'
  if (score >= 90) return 'strong'
  if (score >= 75) return 'normal'
  if (score >= 55) return 'weak'
  if (score > 0) return 'risk'
  return 'unknown'
}

function normalizeIdentityDimension(payload: unknown): WorkbenchIdentityStrengthReport['dimensions'][number] {
  const source = readRecord(payload)
  return {
    id: readString(source, ['id', 'dimensionId', 'key'], 'unknown'),
    category: readString(source, ['category'], 'identity'),
    layer: readString(source, ['layer'], ''),
    status: normalizeCheckStatus(source.status),
    message: readString(source, ['message', 'detail', 'summary'], '-'),
    expected: readString(source, ['expected'], '') || undefined,
    actual: readString(source, ['actual'], '') || undefined,
    penalty: readOptionalNumber(source, ['penalty']),
  }
}

function checkFromSnapshot(
  id: string,
  status: FingerprintHealthCheckStatus,
  message: string,
): WorkbenchFingerprintHealthCheck {
  return { id, status, message }
}

function buildHealthFromSnapshot(profileId: string, fingerprint: WorkbenchFingerprintSnapshot): WorkbenchFingerprintHealthProfile {
  const checks = [
    checkFromSnapshot('user_agent_present', fingerprint.userAgent ? 'pass' : 'fail', fingerprint.userAgent || 'missing user agent'),
    checkFromSnapshot('platform_present', fingerprint.platform ? 'pass' : 'fail', fingerprint.platform || 'missing platform'),
    checkFromSnapshot(
      'screen_size_present',
      fingerprint.screenWidth && fingerprint.screenHeight ? 'pass' : 'fail',
      fingerprint.screenWidth && fingerprint.screenHeight ? `${fingerprint.screenWidth}x${fingerprint.screenHeight}` : 'missing screen size',
    ),
    checkFromSnapshot('timezone_present', fingerprint.timezone ? 'pass' : 'warning', fingerprint.timezone || 'missing timezone'),
    checkFromSnapshot('language_present', fingerprint.language ? 'pass' : 'warning', fingerprint.language || 'missing language'),
    checkFromSnapshot('webgl_present', fingerprint.webglVendor || fingerprint.webglRenderer ? 'pass' : 'warning', fingerprint.webglRenderer || fingerprint.webglVendor || 'missing WebGL'),
    checkFromSnapshot('canvas_hash_present', fingerprint.canvasHash && fingerprint.canvasHash !== 'error' ? 'pass' : 'fail', fingerprint.canvasHash || 'missing canvas hash'),
    checkFromSnapshot('font_hash_present', fingerprint.fontHash && fingerprint.fontHash !== 'error' ? 'pass' : 'fail', fingerprint.fontHash ? 'font metrics captured' : 'missing font metrics'),
  ]
  const penalty = checks.reduce((sum, check) => {
    if (check.status === 'fail') return sum + 15
    if (check.status === 'warning') return sum + 6
    return sum
  }, 0)
  const score = clampScore(100 - penalty)

  return {
    profileId,
    score,
    level: levelFromScore(score),
    checks,
    fingerprint,
    capturedAt: new Date().toISOString(),
    source: 'local-cdp',
  }
}

function normalizeLifecycleEvent(
  eventName: BrowserInstanceLifecycleEventName,
  payload: unknown,
): BrowserInstanceLifecycleEvent {
  if (typeof payload === 'string') {
    return { eventName, profileId: payload, payload }
  }
  const source = readRecord(payload)
  const profileId = readString(source, ['profileId', 'profileID'])
  const profileName = readString(source, ['profileName'])
  const error = readString(source, ['error'])
  return {
    eventName,
    profileId: profileId || undefined,
    profileName: profileName || undefined,
    error: error || undefined,
    payload,
  }
}

export function onBrowserInstanceLifecycle(
  callback: (event: BrowserInstanceLifecycleEvent) => void,
): Unsubscribe {
  return combineUnsubscribes(INSTANCE_EVENT_NAMES.map((eventName) => (
    onRuntimeEvent(eventName, (payload: unknown) => callback(normalizeLifecycleEvent(eventName, payload)))
  )))
}

export function listSyncGroups(): Promise<SyncGroup[]> {
  return backendCall<SyncGroup[]>('SynchronizerListGroups')
}

export function broadcastNavigate(groupId: string, url: string): Promise<void> {
  return backendCall('SynchronizerBroadcastNavigate', groupId, url)
}

export function broadcastRefresh(groupId: string): Promise<void> {
  return backendCall('SynchronizerBroadcastRefresh', groupId)
}

export function navigateProfile(profileId: string, url: string): Promise<void> {
  return backendCall('SynchronizerNavigateProfile', profileId, url)
}

export function refreshProfile(profileId: string): Promise<void> {
  return backendCall('SynchronizerRefreshProfile', profileId)
}

export function captureProfileScreenshot(profileId: string): Promise<string> {
  return backendCall<string>('SynchronizerCaptureScreenshot', profileId)
}

export function activateProfileWindow(profileId: string): Promise<void> {
  return backendCall('SynchronizerActivateProfile', profileId)
}

export function getBrowserInstanceStatus(profileId: string): Promise<BrowserProfile | null> {
  return backendCall<BrowserProfile | null>('BrowserInstanceStatus', profileId)
}

export async function checkWorkbenchFingerprintHealthProfile(profileId: string): Promise<WorkbenchFingerprintHealthProfile> {
  try {
    return normalizeFingerprintHealthProfile(
      profileId,
      await backendCall<unknown>('WorkbenchFingerprintHealthProfile', profileId),
    )
  } catch (error) {
    if (!isMissingBackendFunction(error)) throw error
    const fingerprint = await backendCall<WorkbenchFingerprintSnapshot>('WorkbenchFingerprintProfile', profileId)
    return buildHealthFromSnapshot(profileId, fingerprint)
  }
}

export async function checkWorkbenchIdentityReport(profileId: string): Promise<WorkbenchIdentityStrengthReport> {
  return normalizeIdentityReport(
    profileId,
    await backendCall<unknown>('IdentityReportProfile', profileId),
  )
}

export function arrangeProfileWindows(profileIds: string[], layout: 'grid' | 'main-left'): Promise<SyncWindowPlacement[]> {
  return backendCall<SyncWindowPlacement[]>('SynchronizerArrangeProfiles', profileIds, layout)
}

export function getSyncOperationLog(limit?: number): Promise<SyncOperation[]> {
  return backendCall<SyncOperation[]>('SynchronizerGetOperationLog', limit ?? 50)
}

export function listWorkbenchTasks(limit?: number): Promise<WorkbenchTask[]> {
  return backendCall<WorkbenchTask[]>('SynchronizerListTasks', limit ?? 200)
}

export function saveWorkbenchTasks(tasks: WorkbenchTask[]): Promise<void> {
  return backendCall('SynchronizerSaveTasks', tasks)
}
