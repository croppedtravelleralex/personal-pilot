import { Grid3X3, Columns, Rows, Maximize2, Layout } from 'lucide-react'
import type { LayoutPreset } from '../types'

interface LayoutToolbarProps {
  currentLayout: LayoutPreset
  selectedCount: number
  totalCount: number
  onLayoutChange: (layout: LayoutPreset) => void
  onSelectAll: () => void
  onClearSelection: () => void
  onRefreshAll: () => void
}

const LAYOUT_OPTIONS: { key: LayoutPreset; icon: typeof Grid3X3; label: string }[] = [
  { key: 'grid', icon: Grid3X3, label: '网格' },
  { key: 'row', icon: Rows, label: '单列' },
  { key: 'column', icon: Columns, label: '横排' },
  { key: 'focus', icon: Maximize2, label: '聚焦' },
  { key: 'custom', icon: Layout, label: '自定义' },
]

export function LayoutToolbar({
  currentLayout,
  selectedCount,
  totalCount,
  onLayoutChange,
  onSelectAll,
  onClearSelection,
  onRefreshAll,
}: LayoutToolbarProps) {
  return (
    <div className="flex items-center gap-2 p-2 rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)]">
      {/* Layout presets */}
      <div className="flex items-center gap-0.5">
        {LAYOUT_OPTIONS.map((opt) => (
          <button
            key={opt.key}
            onClick={() => onLayoutChange(opt.key)}
            title={opt.label}
            className={[
              'p-1.5 rounded transition-colors',
              currentLayout === opt.key
                ? 'bg-[var(--color-accent)] text-white'
                : 'text-[var(--color-text-muted)] hover:bg-[var(--color-accent-muted)] hover:text-[var(--color-text-primary)]',
            ].join(' ')}
          >
            <opt.icon className="w-3.5 h-3.5" />
          </button>
        ))}
      </div>

      <div className="w-px h-5 bg-[var(--color-border-muted)]" />

      {/* Selection controls */}
      <div className="flex items-center gap-1 text-xs text-[var(--color-text-muted)]">
        <button onClick={onSelectAll} className="hover:text-[var(--color-text-primary)] transition-colors">
          全选
        </button>
        <span>|</span>
        <button onClick={onClearSelection} className="hover:text-[var(--color-text-primary)] transition-colors">
          取消
        </button>
        <span className="ml-1">
          {selectedCount}/{totalCount}
        </span>
      </div>

      <div className="w-px h-5 bg-[var(--color-border-muted)]" />

      {/* Actions */}
      <button
        onClick={onRefreshAll}
        className="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)] transition-colors"
      >
        全部刷新
      </button>
    </div>
  )
}
