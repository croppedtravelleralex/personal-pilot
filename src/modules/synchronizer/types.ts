/** Types for Sync Windows (multi-window orchestration). */

export interface SyncWindow {
  profileId: string
  profileName: string
  url: string
  title: string
  debugPort: number
  pid: number
  status: 'running' | 'loading'
  groupId: string
}

export interface SyncGroup {
  id: string
  name: string
  windows: SyncWindow[]
}

export interface SyncOperation {
  id: string
  type: string
  groupId: string
  payload?: Record<string, unknown>
  timestamp: string
  status: string
  error?: string
}

export interface SyncWindowPlacement {
  profileId: string
  profileName: string
  pid: number
  found: boolean
  x: number
  y: number
  width: number
  height: number
  error?: string
}

export interface SyncActionFeedEntry {
  id: string
  operation: string
  windowName: string
  detail: string
  timestamp: string
  status: 'ok' | 'error'
}

export type WorkbenchTaskType = 'start' | 'stop' | 'navigate' | 'refresh' | 'screenshot' | 'activate' | 'fingerprint-health'
export type WorkbenchTaskStatus = 'pending' | 'running' | 'success' | 'error'

export interface WorkbenchTask {
  id: string
  type: WorkbenchTaskType
  profileId: string
  profileName: string
  detail: string
  status: WorkbenchTaskStatus
  createdAt: string
  updatedAt: string
  error?: string
}

export type BrowserInstanceLifecycleEventName =
  | 'browser:instance:started'
  | 'browser:instance:updated'
  | 'browser:instance:stopped'
  | 'browser:instance:crashed'

export interface BrowserInstanceLifecycleEvent {
  eventName: BrowserInstanceLifecycleEventName
  profileId?: string
  profileName?: string
  error?: string
  payload: unknown
}

export type FingerprintHealthLevel = 'good' | 'warning' | 'risk' | 'unknown'
export type FingerprintHealthCheckStatus = 'pass' | 'warning' | 'fail' | 'info'

export interface WorkbenchFingerprintSnapshot {
  userAgent?: string
  platform?: string
  hardwareConcurrency?: number
  deviceMemory?: number
  colorDepth?: number
  pixelDepth?: number
  screenWidth?: number
  screenHeight?: number
  availWidth?: number
  availHeight?: number
  devicePixelRatio?: number
  maxTouchPoints?: number
  vendor?: string
  timezone?: string
  language?: string
  languages?: string[]
  canvasHash?: string
  webglVendor?: string
  webglRenderer?: string
  fontHash?: string
}

export interface WorkbenchFingerprintHealthCheck {
  id: string
  status: FingerprintHealthCheckStatus
  message: string
  expected?: string
  actual?: string
  penalty?: number
}

export interface WorkbenchFingerprintHealthProfile {
  profileId: string
  profileName?: string
  score: number
  level: FingerprintHealthLevel
  checks: WorkbenchFingerprintHealthCheck[]
  fingerprint?: WorkbenchFingerprintSnapshot
  capturedAt: string
  source: string
  error?: string
}
