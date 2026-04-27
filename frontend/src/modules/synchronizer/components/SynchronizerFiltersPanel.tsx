import { useState } from 'react'
import { Search, Filter } from 'lucide-react'

interface FiltersPanelProps {
  onSearch: (query: string) => void
  onFilterStatus: (status: string) => void
}

const STATUS_FILTERS = [
  { value: '', label: '全部' },
  { value: 'running', label: '运行中' },
  { value: 'idle', label: '空闲' },
  { value: 'loading', label: '加载中' },
  { value: 'error', label: '异常' },
]

export function SynchronizerFiltersPanel({ onSearch, onFilterStatus }: FiltersPanelProps) {
  const [activeStatus, setActiveStatus] = useState('')

  return (
    <div className="flex items-center gap-3">
      <div className="relative flex-1 max-w-xs">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-[var(--color-text-muted)]" />
        <input
          type="text"
          placeholder="搜索窗口..."
          onChange={(e) => onSearch(e.target.value)}
          className="w-full pl-8 pr-3 py-1.5 text-xs rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-secondary)] text-[var(--color-text-primary)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:border-[var(--color-accent)]"
        />
      </div>
      <div className="flex items-center gap-1">
        <Filter className="w-3.5 h-3.5 text-[var(--color-text-muted)]" />
        {STATUS_FILTERS.map((f) => (
          <button
            key={f.value}
            onClick={() => { setActiveStatus(f.value); onFilterStatus(f.value) }}
            className={[
              'px-2 py-1 rounded text-xs font-medium transition-colors',
              activeStatus === f.value
                ? 'bg-[var(--color-accent)] text-white'
                : 'text-[var(--color-text-muted)] hover:bg-[var(--color-accent-muted)] hover:text-[var(--color-text-primary)]',
            ].join(' ')}
          >
            {f.label}
          </button>
        ))}
      </div>
    </div>
  )
}
