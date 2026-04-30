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
  appVersion?: string
  appName?: string
  product?: string
  productSub?: string
  platform?: string
  webdriver?: boolean
  cookieEnabled?: boolean
  doNotTrack?: string
  pdfViewerEnabled?: boolean
  online?: boolean
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
  timezoneOffset?: number
  language?: string
  languages?: string[]
  intlLocale?: string
  intlCalendar?: string
  intlNumberingSystem?: string
  dateFormatSample?: string
  numberFormatSample?: string
  uaDataBrands?: string[]
  uaDataMobile?: boolean
  uaDataPlatform?: string
  uaDataPlatformVersion?: string
  uaDataArchitecture?: string
  uaDataBitness?: string
  uaDataModel?: string
  uaDataFullVersionList?: string[]
  innerWidth?: number
  innerHeight?: number
  outerWidth?: number
  outerHeight?: number
  visualViewportWidth?: number
  visualViewportHeight?: number
  visualViewportScale?: number
  pointerFine?: boolean
  pointerCoarse?: boolean
  hoverHover?: boolean
  hoverNone?: boolean
  prefersColorScheme?: string
  prefersReducedMotion?: string
  networkEffectiveType?: string
  networkDownlink?: number
  networkRtt?: number
  networkSaveData?: boolean
  storageQuota?: number
  storageUsage?: number
  canvasHash?: string
  webglVendor?: string
  webglRenderer?: string
  webglExtensionsHash?: string
  webglMaxTextureSize?: number
  webglMaxVertexAttribs?: number
  webglMaxViewportDims?: string
  fontHash?: string
  audioHash?: string
  pluginsHash?: string
  mimeTypesHash?: string
  webgpuAvailable?: boolean
  webrtcSupported?: boolean
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

export type IdentityStrengthLevel = 'strong' | 'normal' | 'weak' | 'risk' | 'unknown'
export type IdentityDimensionStatus = 'pass' | 'warning' | 'fail' | 'info'

export interface IdentitySubscores {
  fingerprintVisible: number
  consistency: number
  profilePersistence: number
  proxyNetwork: number
  behaviorNaturalness: number
  automationSafety: number
}

export interface IdentityDimension {
  id: string
  category: string
  layer: string
  status: IdentityDimensionStatus
  message: string
  expected?: string
  actual?: string
  penalty?: number
}

export interface WorkbenchIdentityStrengthReport {
  profileId: string
  profileName?: string
  score: number
  level: IdentityStrengthLevel
  subscores: IdentitySubscores
  dimensions: IdentityDimension[]
  fingerprint?: WorkbenchFingerprintSnapshot
  capturedAt: string
  source: string
  summary: string[]
}
