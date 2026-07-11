import {
  desktopCoreStart,
  desktopEnvironment,
  clearAppLogs,
  confirmDestructivePreflight,
  desktopListen,
  desktopQuit,
  desktopQuitAppOnly,
  desktopQuitFull,
  desktopRpc,
  desktopWindowHide,
  desktopWindowMinimize,
  desktopWindowShow,
  exportSystemConfig,
  getAppLogs,
  importSystemConfig,
  initializeSystemData,
} from './desktop'
import type { DesktopRpcArgs } from './desktop'
import type { DesktopDestructivePreflight } from '../types/desktop'

type BridgeEventData = Array<unknown>
type BridgeRpcResult = unknown
type BridgeRpcMethod = (...args: DesktopRpcArgs) => Promise<BridgeRpcResult>
type BridgeAppProxy = Record<string, BridgeRpcMethod>
type RuntimeCallback = (...data: BridgeEventData) => void
type ListenerSet = Set<RuntimeCallback>

interface WailsRuntimeShim {
  EventsOn: (eventName: string, callback: RuntimeCallback) => () => void
  EventsOnMultiple: (eventName: string, callback: RuntimeCallback, maxCallbacks: number) => () => void
  EventsOnce: (eventName: string, callback: RuntimeCallback) => () => void
  EventsOff: (eventName: string, ...additionalEventNames: string[]) => void
  EventsOffAll: () => void
  EventsEmit: (eventName: string, ...data: BridgeEventData) => void
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
      App?: BridgeAppProxy
    }
  }
  runtime?: WailsRuntimeShim
  __ANT_TAURI_BRIDGE_READY__?: boolean
}

interface SidecarEventPayload {
  eventName: string
  data?: BridgeEventData
}

const bridgeWindow = window as BridgeWindow
const listeners = new Map<string, ListenerSet>()

const BRIDGE_RPC_METHOD_NAMES = new Set<string>([
  'BrowserProfileList',
  'BrowserProfileListByTag',
  'BrowserGetAllTags',
  'BrowserProfileCreate',
  'BrowserProfileUpdate',
  'BrowserProfileDelete',
  'BrowserProfileCopy',
  'BrowserInstanceStart',
  'BrowserInstanceStartByCode',
  'BrowserInstanceStop',
  'BrowserInstanceRestart',
  'BrowserInstanceOpenUrl',
  'BrowserInstanceGetTabs',
  'BrowserGetCookies',
  'BrowserClearCookies',
  'BrowserExportCookies',
  'BrowserSnapshotList',
  'BrowserSnapshotCreate',
  'BrowserSnapshotDelete',
  'BookmarkList',
  'BookmarkSave',
  'BookmarkReset',
  'BrowserProfileSetKeywords',
  'GetLaunchServerInfo',
  'BrowserProfileGetCode',
  'BrowserProfileRegenerateCode',
  'BrowserProfileSetCode',
  'BrowserProfileBatchSetTags',
  'BrowserProfileBatchRemoveTags',
  'BrowserRenameTag',
  'BrowserProxyDelete',
  'ListGroups',
  'CreateGroup',
  'UpdateGroup',
  'DeleteGroup',
  'MoveInstancesToGroup',
  'BehaviorStartRecording',
  'BehaviorStopRecording',
  'BehaviorRecordingList',
  'BehaviorRecordingSummaryList',
  'BehaviorRecordingStatus',
  'ActiveRecordingStatus',
  'BehaviorRecordingDelete',
  'BehaviorGetRecording',
  'BehaviorGetRecordingDetail',
  'BehaviorPlayRecording',
  'BehaviorStopPlayback',
  'BehaviorQuickRecord',
  'BehaviorPresetList',
  'CleanupStaleRecordingSessions',
  'BehaviorRecordingRename',
  'BehaviorRecordingExport',
  'BehaviorRecordingImport',
  'BehaviorRecordingCopy',
  'BehaviorPlaybackReview',
  'BehaviorRecordingTrim',
  'SchedulerListTasks',
  'SchedulerAddTask',
  'SchedulerRemoveTask',
  'SchedulerRunTaskNow',
  'AutomationRuleList',
  'AutomationRuleCreate',
  'AutomationRuleDelete',
  'AutomationRuleToggle',
  'AutomationRuleTestFire',
])

function emitLocal(eventName: string, ...data: BridgeEventData) {
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

function createAppProxy(): BridgeAppProxy {
  const directMethods: BridgeAppProxy = {
    BackupInitializeSystem: initializeSystemData,
    BackupExportPackage: exportSystemConfig,
    BackupImportPackage: importSystemConfig,
    GetAppLogs: getAppLogs,
    ClearAppLogs: clearAppLogs,
    BrowserSnapshotRestore: restoreBrowserSnapshot,
  }

  return new Proxy(
    {},
    {
      get(_target, property) {
        if (typeof property !== 'string') return undefined
        if (property === 'then') return undefined
        const directMethod = directMethods[property]
        if (directMethod) return directMethod
        if (BRIDGE_RPC_METHOD_NAMES.has(property)) {
          return (...args: DesktopRpcArgs) => desktopRpc(property, args)
        }
        return undefined
      },
    },
  ) as BridgeAppProxy
}

async function restoreBrowserSnapshot(profileId?: unknown, snapshotId?: unknown) {
  const pid = String(profileId || '')
  const sid = String(snapshotId || '')
  const preflight = await desktopRpc<DesktopDestructivePreflight>('BrowserSnapshotRestorePreflight', [pid, sid], { timeoutMs: 120000 })
  if (!confirmDestructivePreflight(preflight)) {
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
