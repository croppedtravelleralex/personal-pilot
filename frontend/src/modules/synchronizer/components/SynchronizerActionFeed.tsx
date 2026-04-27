import { Navigation, ScrollText, MousePointerClick, Keyboard, RotateCw, X } from 'lucide-react'
import { useSyncStore } from '../store'
import type { SyncActionFeedEntry } from '../types'

const OP_ICONS: Record<SyncActionFeedEntry['operation'], typeof Navigation> = {
  navigate: Navigation,
  scroll: ScrollText,
  click: MousePointerClick,
  type: Keyboard,
  refresh: RotateCw,
  close: X,
  focus: MousePointerClick,
  arrange: Navigation,
}

const OP_LABELS: Record<SyncActionFeedEntry['operation'], string> = {
  navigate: '导航',
  scroll: '滚动',
  click: '点击',
  type: '输入',
  refresh: '刷新',
  close: '关闭',
  focus: '聚焦',
  arrange: '布局',
}

export function SynchronizerActionFeed() {
  const feed = useSyncStore((s) => s.actionFeed)

  if (feed.length === 0) {
    return (
      <div className="text-center py-8 text-xs text-[var(--color-text-muted)]">
        暂无操作记录。开始同步操作后会显示在这里。
      </div>
    )
  }

  return (
    <div className="space-y-1 max-h-64 overflow-y-auto">
      {feed.map((entry) => {
        const Icon = OP_ICONS[entry.operation] || MousePointerClick
        return (
          <div
            key={entry.id}
            className="flex items-center gap-2 rounded px-2 py-1.5 text-xs hover:bg-[var(--color-bg-secondary)] transition-colors"
          >
            <Icon className={`w-3 h-3 flex-shrink-0 ${
              entry.status === 'ok' ? 'text-emerald-500' : 'text-red-400'
            }`} />
            <span className="text-[var(--color-text-muted)] w-10 flex-shrink-0">
              {OP_LABELS[entry.operation]}
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
