import { Monitor, Globe, Clock } from 'lucide-react'
import type { SyncWindow } from '../types'
import { MainWindowBadge } from './MainWindowBadge'

interface WindowCardProps {
  window: SyncWindow
  isSelected: boolean
  onSelect: (id: string) => void
  onFocus: (id: string) => void
  onClose: (id: string) => void
}

const STATUS_COLORS: Record<SyncWindow['status'], string> = {
  running: 'bg-emerald-400',
  idle: 'bg-amber-400',
  loading: 'bg-blue-400 animate-pulse',
  error: 'bg-red-400',
}

const STATUS_LABELS: Record<SyncWindow['status'], string> = {
  running: '运行中',
  idle: '空闲',
  loading: '加载中',
  error: '异常',
}

export function WindowCard({ window: win, isSelected, onSelect, onFocus, onClose }: WindowCardProps) {
  return (
    <div
      className={[
        'relative rounded-lg border bg-[var(--color-bg-card)] p-3 transition-all cursor-pointer',
        isSelected
          ? 'border-[var(--color-primary)] ring-1 ring-[var(--color-primary)] shadow-sm'
          : 'border-[var(--color-border-muted)] hover:border-[var(--color-border-default)]',
      ].join(' ')}
      onClick={() => onSelect(win.id)}
      onDoubleClick={() => onFocus(win.id)}
    >
      {/* Status dot */}
      <div className="absolute top-2 right-2 flex items-center gap-1.5">
        <span className={`inline-block w-2 h-2 rounded-full ${STATUS_COLORS[win.status]}`} />
        <span className="text-[10px] text-[var(--color-text-muted)]">{STATUS_LABELS[win.status]}</span>
      </div>

      {/* Header */}
      <div className="flex items-center gap-2 mb-2 pr-14">
        <div className="flex h-7 w-7 items-center justify-center rounded bg-[var(--color-accent-muted)] text-[var(--color-accent)]">
          <Monitor className="w-3.5 h-3.5" />
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-1.5">
            <p className="text-xs font-medium text-[var(--color-text-primary)] truncate">{win.profileName}</p>
            <MainWindowBadge isMain={win.isMain} />
          </div>
          <p className="text-[11px] text-[var(--color-text-muted)] truncate">{win.title}</p>
        </div>
      </div>

      {/* URL */}
      <div className="flex items-center gap-1 mb-2 text-[11px] text-[var(--color-text-muted)]">
        <Globe className="w-3 h-3 flex-shrink-0" />
        <span className="truncate">{win.url}</span>
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1 text-[10px] text-[var(--color-text-muted)]">
          <Clock className="w-2.5 h-2.5" />
          <span>端口 {win.debugPort}</span>
        </div>
        <button
          onClick={(e) => { e.stopPropagation(); onClose(win.id) }}
          className="text-[10px] text-red-500 hover:text-red-600 font-medium"
        >
          关闭
        </button>
      </div>
    </div>
  )
}
