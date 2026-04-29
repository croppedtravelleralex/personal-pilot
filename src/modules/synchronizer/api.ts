import type { SyncGroup, SyncOperation, SyncWindowPlacement, WorkbenchTask } from './types'

// Wails backend function bindings.
const wailsCall = <T>(name: string, ...args: unknown[]): Promise<T> => {
  const w = window as unknown as Record<string, unknown>
  const go = w['go'] as Record<string, Record<string, (...a: unknown[]) => Promise<unknown>>> | undefined
  if (!go?.main?.App) {
    return Promise.reject(new Error('Wails runtime not available'))
  }
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const fn = (go.main.App as any)[name]
  if (typeof fn !== 'function') {
    return Promise.reject(new Error(`Backend function ${name} not found`))
  }
  return fn(...args) as Promise<T>
}

export function listSyncGroups(): Promise<SyncGroup[]> {
  return wailsCall<SyncGroup[]>('SynchronizerListGroups')
}

export function broadcastNavigate(groupId: string, url: string): Promise<void> {
  return wailsCall('SynchronizerBroadcastNavigate', groupId, url)
}

export function broadcastRefresh(groupId: string): Promise<void> {
  return wailsCall('SynchronizerBroadcastRefresh', groupId)
}

export function navigateProfile(profileId: string, url: string): Promise<void> {
  return wailsCall('SynchronizerNavigateProfile', profileId, url)
}

export function refreshProfile(profileId: string): Promise<void> {
  return wailsCall('SynchronizerRefreshProfile', profileId)
}

export function captureProfileScreenshot(profileId: string): Promise<string> {
  return wailsCall<string>('SynchronizerCaptureScreenshot', profileId)
}

export function activateProfileWindow(profileId: string): Promise<void> {
  return wailsCall('SynchronizerActivateProfile', profileId)
}

export function arrangeProfileWindows(profileIds: string[], layout: 'grid' | 'main-left'): Promise<SyncWindowPlacement[]> {
  return wailsCall<SyncWindowPlacement[]>('SynchronizerArrangeProfiles', profileIds, layout)
}

export function getSyncOperationLog(limit?: number): Promise<SyncOperation[]> {
  return wailsCall<SyncOperation[]>('SynchronizerGetOperationLog', limit ?? 50)
}

export function listWorkbenchTasks(limit?: number): Promise<WorkbenchTask[]> {
  return wailsCall<WorkbenchTask[]>('SynchronizerListTasks', limit ?? 200)
}

export function saveWorkbenchTasks(tasks: WorkbenchTask[]): Promise<void> {
  return wailsCall('SynchronizerSaveTasks', tasks)
}
