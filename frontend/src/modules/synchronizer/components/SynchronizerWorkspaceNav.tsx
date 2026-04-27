import { Layers } from 'lucide-react'
import type { SyncGroup } from '../types'

interface WorkspaceNavProps {
  groups: SyncGroup[]
  activeGroupId: string | null
  onSelectGroup: (id: string) => void
}

export function SynchronizerWorkspaceNav({ groups, activeGroupId, onSelectGroup }: WorkspaceNavProps) {
  return (
    <div className="flex items-center gap-2 overflow-x-auto pb-1">
      {groups.map((group) => (
        <button
          key={group.id}
          onClick={() => onSelectGroup(group.id)}
          className={[
            'flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm font-medium whitespace-nowrap transition-colors',
            activeGroupId === group.id
              ? 'bg-[var(--color-accent)] text-white shadow-sm'
              : 'bg-[var(--color-bg-secondary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-accent-muted)] hover:text-[var(--color-text-primary)]',
          ].join(' ')}
        >
          <Layers className="w-3.5 h-3.5" />
          {group.name}
          <span className="ml-1 rounded-full bg-white/20 px-1.5 py-0.5 text-[10px]">
            {group.windows.length}
          </span>
        </button>
      ))}
    </div>
  )
}
