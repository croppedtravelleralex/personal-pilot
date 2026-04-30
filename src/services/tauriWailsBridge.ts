import {
  desktopCoreStart,
  desktopEnvironment,
  desktopListen,
  desktopOpenBackupPath,
  desktopQuit,
  desktopQuitAppOnly,
  desktopQuitFull,
  desktopRpc,
  desktopSaveBackupPath,
  desktopWindowHide,
  desktopWindowMinimize,
  desktopWindowShow,
} from './desktop'

type RuntimeCallback = (...data: unknown[]) => void
type ListenerSet = Set<RuntimeCallback>

interface WailsRuntimeShim {
  EventsOn: (eventName: string, callback: RuntimeCallback) => () => void
  EventsOnMultiple: (eventName: string, callback: RuntimeCallback, maxCallbacks: number) => () => void
  EventsOnce: (eventName: string, callback: RuntimeCallback) => () => void
  EventsOff: (eventName: string, ...additionalEventNames: string[]) => void
  EventsOffAll: () => void
  EventsEmit: (eventName: string, ...data: unknown[]) => void
  Environment: typeof desktopEnvironment
  Quit: typeof desktopQuit
  Hide: typeof desktopWindowHide
  Show: typeof desktopWindowShow
  WindowHide: typeof desktopWindowHide
  WindowShow: typeof desktopWindowShow
  WindowMinimise: typeof desktopWindowMinimize
  WindowUnminimise: typeof desktopWindowShow
  BrowserOpenURL: (url: string) => void
  ClipboardGetText: () => Promise<string>
  ClipboardSetText: (text: string) => Promise<void>
  [key: string]: unknown
}

interface BridgeWindow extends Window {
  go?: {
    main?: {
      App?: Record<string, (...args: unknown[]) => Promise<unknown>>
    }
  }
  runtime?: WailsRuntimeShim
  __ANT_TAURI_BRIDGE_READY__?: boolean
}

interface SidecarEventPayload {
  eventName: string
  data?: unknown[]
}

interface DestructivePreflight {
  operation?: string
  targetProfileId?: string
  targetUserDataDir?: string
  requiresStop?: boolean
  writesCookie?: boolean
  destructivePaths?: string[]
  warnings?: Array<{ code?: string; message?: string }>
  blockers?: Array<{ code?: string; message?: string }>
  confirmationToken?: string
  confirmationPrompt?: string
}

const bridgeWindow = window as BridgeWindow
const listeners = new Map<string, ListenerSet>()

function emitLocal(eventName: string, ...data: unknown[]) {
  const callbacks = listeners.get(eventName)
  if (!callbacks) return
  for (const callback of Array.from(callbacks)) {
    callback(...data)
  }
}

function onMultiple(eventName: string, callback: RuntimeCallback, maxCallbacks: number) {
  const callbacks = listeners.get(eventName) ?? new Set<RuntimeCallback>()
  let count = 0
  const wrapped: RuntimeCallback = (...data) => {
    count += 1
    callback(...data)
    if (maxCallbacks > 0 && count >= maxCallbacks) {
      callbacks.delete(wrapped)
    }
  }
  callbacks.add(wrapped)
  listeners.set(eventName, callbacks)
  return () => {
    callbacks.delete(wrapped)
    if (callbacks.size === 0) listeners.delete(eventName)
  }
}

function createAppProxy(): Record<string, (...args: unknown[]) => Promise<unknown>> {
  return new Proxy(
    {},
    {
      get(_target, property) {
        if (typeof property !== 'string') return undefined
        if (property === 'BackupInitializeSystem') return initializeSystemData
        if (property === 'BackupExportPackage') return exportBackupPackage
        if (property === 'BackupImportPackage') return importBackupPackage
        if (property === 'BrowserSnapshotRestore') return restoreBrowserSnapshot
        return (...args: unknown[]) => desktopRpc(property, args)
      },
    },
  ) as Record<string, (...args: unknown[]) => Promise<unknown>>
}

function backupDefaultFilename() {
  const now = new Date()
  const pad = (value: number) => String(value).padStart(2, '0')
  const stamp = [
    now.getFullYear(),
    pad(now.getMonth() + 1),
    pad(now.getDate()),
    '-',
    pad(now.getHours()),
    pad(now.getMinutes()),
    pad(now.getSeconds()),
  ].join('')
  return `personal-pilot-backup-${stamp}.zip`
}

function describePreflight(preflight: DestructivePreflight): string {
  const lines = [
    preflight.confirmationPrompt || 'Confirm this destructive operation before continuing.',
  ]
  if (preflight.targetProfileId) lines.push(`Profile: ${preflight.targetProfileId}`)
  if (preflight.targetUserDataDir) lines.push(`Target: ${preflight.targetUserDataDir}`)
  if (preflight.writesCookie) lines.push('Cookie assets will be written.')
  if (preflight.requiresStop) lines.push('Affected running profiles must be stopped first.')
  if (preflight.destructivePaths?.length) {
    lines.push('Paths:')
    lines.push(...preflight.destructivePaths.slice(0, 8).map(path => `- ${path}`))
  }
  if (preflight.warnings?.length) {
    lines.push('Warnings:')
    lines.push(...preflight.warnings.slice(0, 5).map(item => `- ${item.message || item.code || 'warning'}`))
  }
  return lines.join('\n')
}

function assertPreflightCanContinue(preflight: DestructivePreflight) {
  const blockers = preflight.blockers || []
  if (blockers.length > 0) {
    const first = blockers[0]
    throw new Error(first.message || first.code || 'destructive preflight blocked')
  }
  if (!preflight.confirmationToken) {
    throw new Error('destructive preflight did not return a confirmation token')
  }
}

function confirmPreflight(preflight: DestructivePreflight): boolean {
  assertPreflightCanContinue(preflight)
  return window.confirm(describePreflight(preflight))
}

async function exportBackupPackage() {
  const savePath = await desktopSaveBackupPath(backupDefaultFilename())
  if (!savePath) {
    return { cancelled: true, message: 'export cancelled' }
  }
  return desktopRpc('BackupExportPackageToPath', [savePath], { timeoutMs: 120000 })
}

async function importBackupPackage(resetFirst?: unknown) {
  const zipPath = await desktopOpenBackupPath()
  if (!zipPath) {
    return { cancelled: true, message: 'import cancelled' }
  }
  const shouldReset = Boolean(resetFirst)
  const preflight = await desktopRpc<DestructivePreflight>('BackupImportPackagePreflightFromPath', [zipPath, shouldReset], { timeoutMs: 120000 })
  if (!confirmPreflight(preflight)) {
    return { cancelled: true, message: 'import cancelled' }
  }
  return desktopRpc('BackupImportPackageFromPathConfirmed', [
    zipPath,
    shouldReset,
    { confirmed: true, confirmationToken: preflight.confirmationToken },
  ], { timeoutMs: 120000 })
}

async function initializeSystemData() {
  const preflight = await desktopRpc<DestructivePreflight>('BackupInitializeSystemPreflight', [], { timeoutMs: 120000 })
  if (!confirmPreflight(preflight)) {
    return { cancelled: true, message: 'initialize cancelled' }
  }
  return desktopRpc('BackupInitializeSystemConfirmed', [
    { confirmed: true, confirmationToken: preflight.confirmationToken },
  ], { timeoutMs: 120000 })
}

async function restoreBrowserSnapshot(profileId?: unknown, snapshotId?: unknown) {
  const pid = String(profileId || '')
  const sid = String(snapshotId || '')
  const preflight = await desktopRpc<DestructivePreflight>('BrowserSnapshotRestorePreflight', [pid, sid], { timeoutMs: 120000 })
  if (!confirmPreflight(preflight)) {
    return undefined
  }
  return desktopRpc('BrowserSnapshotRestoreConfirmed', [
    pid,
    sid,
    { confirmed: true, confirmationToken: preflight.confirmationToken },
  ], { timeoutMs: 120000 })
}

function installRuntimeShim() {
  const runtime: WailsRuntimeShim = {
    EventsOn: (eventName, callback) => onMultiple(eventName, callback, -1),
    EventsOnMultiple: onMultiple,
    EventsOnce: (eventName, callback) => onMultiple(eventName, callback, 1),
    EventsOff: (eventName, ...additionalEventNames) => {
      listeners.delete(eventName)
      for (const name of additionalEventNames) listeners.delete(name)
    },
    EventsOffAll: () => listeners.clear(),
    EventsEmit: emitLocal,
    Environment: desktopEnvironment,
    Quit: desktopQuit,
    Hide: desktopWindowHide,
    Show: desktopWindowShow,
    WindowHide: desktopWindowHide,
    WindowShow: desktopWindowShow,
    WindowMinimise: desktopWindowMinimize,
    WindowUnminimise: desktopWindowShow,
    BrowserOpenURL: (url: string) => {
      window.open(url, '_blank', 'noopener,noreferrer')
    },
    ClipboardGetText: async () => navigator.clipboard?.readText?.() ?? '',
    ClipboardSetText: async (text: string) => {
      await navigator.clipboard?.writeText?.(text)
    },
    LogPrint: console.log,
    LogTrace: console.trace,
    LogDebug: console.debug,
    LogInfo: console.info,
    LogWarning: console.warn,
    LogError: console.error,
    LogFatal: console.error,
  }
  bridgeWindow.runtime = runtime
}

async function attachTauriEvents() {
  await desktopListen('app:request-close', () => {
    emitLocal('app:request-close')
  }).catch((error) => {
    console.warn('failed to attach app close event', error)
  })
}

async function attachSidecarEvents() {
  try {
    const status = await desktopCoreStart()
    if (!status.eventUrl || typeof EventSource === 'undefined') return
    const source = new EventSource(status.eventUrl)
    source.onmessage = (message) => {
      try {
        const payload = JSON.parse(message.data) as SidecarEventPayload
        if (payload.eventName) {
          emitLocal(payload.eventName, ...(payload.data ?? []))
        }
      } catch (error) {
        console.warn('invalid sidecar event payload', error)
      }
    }
    source.onerror = () => {
      source.close()
      window.setTimeout(() => {
        void attachSidecarEvents()
      }, 1500)
    }
  } catch (error) {
    console.error('failed to start personal-pilot core', error)
  }
}

if (!bridgeWindow.__ANT_TAURI_BRIDGE_READY__) {
  bridgeWindow.__ANT_TAURI_BRIDGE_READY__ = true
  bridgeWindow.go = bridgeWindow.go ?? {}
  bridgeWindow.go.main = bridgeWindow.go.main ?? {}
  bridgeWindow.go.main.App = bridgeWindow.go.main.App ?? createAppProxy()
  bridgeWindow.go.main.App.QuitAppOnly = desktopQuitAppOnly
  bridgeWindow.go.main.App.ForceQuit = desktopQuitFull
  installRuntimeShim()
  void attachTauriEvents()
  void attachSidecarEvents()
}
