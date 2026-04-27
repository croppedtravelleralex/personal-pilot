/** Types for Sync Windows (multi-window orchestration). */

export interface SyncWindow {
  id: string
  profileId: string
  profileName: string
  title: string
  url: string
  debugPort: number
  isMain: boolean
  position: WindowPosition
  size: WindowSize
  status: WindowStatus
  lastActiveAt: string
  groupId: string
}

export type WindowStatus = 'running' | 'idle' | 'loading' | 'error'

export interface WindowPosition {
  x: number
  y: number
}

export interface WindowSize {
  width: number
  height: number
}

export interface SyncGroup {
  id: string
  name: string
  windows: SyncWindow[]
  layout: LayoutPreset
  createdAt: string
}

export type LayoutPreset = 'grid' | 'row' | 'column' | 'focus' | 'custom'

export interface SyncOperation {
  id: string
  type: SyncOpType
  windowId: string
  groupId: string
  payload?: Record<string, unknown>
  timestamp: string
  status: 'pending' | 'success' | 'failed'
  error?: string
}

export type SyncOpType =
  | 'navigate'
  | 'scroll'
  | 'click'
  | 'type'
  | 'refresh'
  | 'close'
  | 'focus'
  | 'arrange'

export interface SyncActionFeedEntry {
  id: string
  operation: SyncOpType
  windowName: string
  detail: string
  timestamp: string
  status: 'ok' | 'error'
}

export interface WorkspaceSection {
  id: string
  name: string
  groups: SyncGroup[]
}
