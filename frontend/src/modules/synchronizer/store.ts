import { create } from 'zustand'
import type { SyncWindow, SyncGroup, SyncActionFeedEntry, LayoutPreset } from './types'

interface SynchronizerState {
  groups: SyncGroup[]
  activeGroupId: string | null
  actionFeed: SyncActionFeedEntry[]
  broadcastTarget: 'all' | 'group' | 'selection'
  selectedWindowIds: Set<string>

  // Actions
  setGroups: (groups: SyncGroup[]) => void
  setActiveGroup: (id: string | null) => void
  addActionToFeed: (entry: SyncActionFeedEntry) => void
  setBroadcastTarget: (target: 'all' | 'group' | 'selection') => void
  toggleWindowSelection: (id: string) => void
  selectAllInGroup: (groupId: string) => void
  clearSelection: () => void
  updateWindowStatus: (windowId: string, status: SyncWindow['status']) => void
  setGroupLayout: (groupId: string, layout: LayoutPreset) => void
}

export const useSyncStore = create<SynchronizerState>((set) => ({
  groups: [],
  activeGroupId: null,
  actionFeed: [],
  broadcastTarget: 'group',
  selectedWindowIds: new Set<string>(),

  setGroups: (groups) => set({ groups }),

  setActiveGroup: (id) => set({ activeGroupId: id }),

  addActionToFeed: (entry) =>
    set((s) => ({
      actionFeed: [entry, ...s.actionFeed].slice(0, 100),
    })),

  setBroadcastTarget: (target) => set({ broadcastTarget: target }),

  toggleWindowSelection: (id) =>
    set((s) => {
      const next = new Set(s.selectedWindowIds)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return { selectedWindowIds: next }
    }),

  selectAllInGroup: (groupId) =>
    set((s) => {
      const group = s.groups.find((g) => g.id === groupId)
      if (!group) return s
      return { selectedWindowIds: new Set(group.windows.map((w) => w.id)) }
    }),

  clearSelection: () => set({ selectedWindowIds: new Set<string>() }),

  updateWindowStatus: (windowId, status) =>
    set((s) => ({
      groups: s.groups.map((g) => ({
        ...g,
        windows: g.windows.map((w) =>
          w.id === windowId ? { ...w, status } : w,
        ),
      })),
    })),

  setGroupLayout: (groupId, layout) =>
    set((s) => ({
      groups: s.groups.map((g) =>
        g.id === groupId ? { ...g, layout } : g,
      ),
    })),
}))
