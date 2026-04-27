import type { SyncGroup, LayoutPreset } from '../types'
import { WindowCard } from './WindowCard'
import { useSyncStore } from '../store'

interface WindowMatrixProps {
  group: SyncGroup
  onFocus: (id: string) => void
  onClose: (id: string) => void
}

const LAYOUT_GRID_CLASSES: Record<LayoutPreset, string> = {
  grid: 'grid grid-cols-2 gap-3',
  row: 'grid grid-cols-1 gap-2',
  column: 'flex gap-3 overflow-x-auto',
  focus: 'grid grid-cols-1 gap-3',
  custom: 'grid grid-cols-2 gap-3',
}

export function WindowMatrix({ group, onFocus, onClose }: WindowMatrixProps) {
  const selectedIds = useSyncStore((s) => s.selectedWindowIds)
  const toggleSelection = useSyncStore((s) => s.toggleWindowSelection)

  if (group.windows.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-16 text-[var(--color-text-muted)]">
        <p className="text-sm">此分组暂无窗口</p>
        <p className="text-xs mt-1">启动实例后会自动加入分组</p>
      </div>
    )
  }

  return (
    <div className={LAYOUT_GRID_CLASSES[group.layout]}>
      {group.windows.map((win) => (
        <WindowCard
          key={win.id}
          window={win}
          isSelected={selectedIds.has(win.id)}
          onSelect={toggleSelection}
          onFocus={onFocus}
          onClose={onClose}
        />
      ))}
    </div>
  )
}
