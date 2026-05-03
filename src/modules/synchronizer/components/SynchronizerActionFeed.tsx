import { LayoutGrid, MousePointer2, Navigation, Play, RotateCw, Square } from 'lucide-react'
import { useSyncStore } from '../store'

const OP_ICONS: Record<string, typeof Navigation> = {
  navigate: Navigation,
  refresh: RotateCw,
  screenshot: Square,
  start: Play,
  stop: Square,
  activate: MousePointer2,
  arrange: LayoutGrid,
}

const OP_LABELS: Record<string, string> = {
  navigate: '导航',
  refresh: '刷新',
  screenshot: '已禁用',
  start: '启动',
  stop: '停止',
  activate: '激活',
  arrange: '排列',
}

export function SynchronizerActionFeed() {
  const feed = useSyncStore((s) => s.actionFeed)

  if (feed.length === 0) {
    return (
      <div className="text-center py-8 text-xs text-[var(--color-text-muted)]">
        暂无操作记录。执行工作台任务后会显示在这里。
      </div>
    )
  }

  return (
    <div className="space-y-1 max-h-64 overflow-y-auto">
      {feed.map((entry) => {
        const Icon = OP_ICONS[entry.operation] || Navigation
        return (
          <div
            key={entry.id}
            className="flex items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-[var(--color-bg-secondary)] transition-colors"
          >
            <Icon
              className={`w-3 h-3 flex-shrink-0 ${
                entry.status === 'ok' ? 'text-emerald-500' : 'text-red-400'
              }`}
            />
            <span className="text-[var(--color-text-muted)] w-10 flex-shrink-0">
              {OP_LABELS[entry.operation] || entry.operation}
            </span>
            <span className="text-[var(--color-text-primary)] truncate flex-1">
              {entry.windowName}
            </span>
            <span className="text-[var(--color-text-muted)] flex-shrink-0">
              {entry.detail}
            </span>
            <span className="text-[10px] text-[var(--color-text-muted)] flex-shrink-0 w-14 text-right">
              {new Date(entry.timestamp).toLocaleTimeString()}
            </span>
          </div>
        )
      })}
    </div>
  )
}
