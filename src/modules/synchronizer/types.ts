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

export type WorkbenchTaskType = 'start' | 'stop' | 'navigate' | 'refresh' | 'screenshot' | 'activate'
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
