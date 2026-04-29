import { create } from 'zustand'
import type { SyncGroup, SyncActionFeedEntry } from './types'

interface SynchronizerState {
  groups: SyncGroup[]
  activeGroupId: string | null
  actionFeed: SyncActionFeedEntry[]

  setGroups: (groups: SyncGroup[]) => void
  setActiveGroup: (id: string | null) => void
  addActionToFeed: (entry: SyncActionFeedEntry) => void
}

export const useSyncStore = create<SynchronizerState>((set) => ({
  groups: [],
  activeGroupId: null,
  actionFeed: [],

  setGroups: (groups) => set({ groups }),

  setActiveGroup: (id) => set({ activeGroupId: id }),

  addActionToFeed: (entry) =>
    set((s) => ({
      actionFeed: [entry, ...s.actionFeed].slice(0, 100),
    })),
}))
