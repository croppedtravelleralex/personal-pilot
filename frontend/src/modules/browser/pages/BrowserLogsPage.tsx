import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  ChevronsRight,
  RefreshCw,
  Trash2,
} from 'lucide-react'
import { Badge, Button, Card, Select } from '../../../shared/components'

type AppBindings = typeof import('../../../wailsjs/go/main/App')

interface LogEntry {
  time: string
  level: string
  component: string
  message: string
  fields?: Record<string, unknown>
}

interface LogPage {
  entries: LogEntry[]
  total: number
  offset: number
  limit: number
  hasMore: boolean
}

const LEVELS = ['ALL', 'DEBUG', 'INFO', 'WARN', 'ERROR']
const PAGE_SIZES = [25, 50, 100, 200]
const DEFAULT_PAGE_SIZE = 50
const AUTO_REFRESH_MS = 3000
const SEARCH_DEBOUNCE_MS = 300

const emptyPage = (limit: number): LogPage => ({
  entries: [],
  total: 0,
  offset: 0,
  limit,
  hasMore: false,
})

const levelVariant = (level: string) => {
  switch (level) {
    case 'ERROR': return 'error'
    case 'WARN': return 'warning'
    case 'DEBUG': return 'default'
    default: return 'info'
  }
}

const levelColor = (level: string) => {
  switch (level) {
    case 'ERROR': return 'text-red-500'
    case 'WARN': return 'text-yellow-500'
    case 'DEBUG': return 'text-[var(--color-text-muted)]'
    default: return 'text-[var(--color-text-secondary)]'
  }
}

async function getBindings(): Promise<AppBindings | null> {
  try {
    return await import('../../../wailsjs/go/main/App')
  } catch {
    return null
  }
}

async function fetchLogsPage(
  offset: number,
  limit: number,
  level: string,
  keyword: string
): Promise<LogPage> {
  const bindings = await getBindings()
  if (!bindings?.GetAppLogsPage) return emptyPage(limit)
  const page = await bindings.GetAppLogsPage(offset, limit, level, keyword)
  return {
    entries: page?.entries || [],
    total: page?.total || 0,
    offset: page?.offset || 0,
    limit: page?.limit || limit,
    hasMore: Boolean(page?.hasMore),
  }
}

async function clearLogs() {
  const bindings = await getBindings()
  await bindings?.ClearAppLogs?.()
}

function stringifyLogValue(value: unknown): string {
  if (typeof value === 'string') return value
  try {
    return JSON.stringify(value) ?? String(value)
  } catch {
    return String(value)
  }
}

function formatFields(fields?: Record<string, unknown>): string {
  if (!fields || Object.keys(fields).length === 0) return ''
  return Object.entries(fields)
    .map(([key, value]) => `${key}=${stringifyLogValue(value)}`)
    .join(' ')
}

export function BrowserLogsPage() {
  const [page, setPage] = useState<LogPage>(() => emptyPage(DEFAULT_PAGE_SIZE))
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE)
  const [levelFilter, setLevelFilter] = useState('ALL')
  const [keyword, setKeyword] = useState('')
  const [debouncedKeyword, setDebouncedKeyword] = useState('')
  const [followLatest, setFollowLatest] = useState(true)
  const [loading, setLoading] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const requestIdRef = useRef(0)
  const bottomRef = useRef<HTMLDivElement>(null)

  const totalPages = Math.max(1, Math.ceil(page.total / pageSize))
  const currentPage = page.total === 0 ? 1 : Math.floor(page.offset / pageSize) + 1
  const rangeStart = page.total === 0 ? 0 : page.offset + 1
  const rangeEnd = page.offset + page.entries.length
  const canPrevious = page.offset > 0
  const canNext = page.offset + page.entries.length < page.total

  const rows = useMemo(
    () => page.entries.map((entry, index) => ({
      ...entry,
      rowId: `${page.offset + index}-${entry.time}-${entry.level}-${entry.component}`,
      fieldText: formatFields(entry.fields),
    })),
    [page.entries, page.offset]
  )

  const loadPage = useCallback(
    async (offset: number, showLoading = true) => {
      const requestId = requestIdRef.current + 1
      requestIdRef.current = requestId
      if (showLoading) {
        setLoading(true)
      } else {
        setRefreshing(true)
      }

      try {
        const nextPage = await fetchLogsPage(offset, pageSize, levelFilter, debouncedKeyword)
        if (requestId !== requestIdRef.current) return

        setPage({
          entries: nextPage.entries || [],
          total: nextPage.total || 0,
          offset: nextPage.offset || 0,
          limit: nextPage.limit || pageSize,
          hasMore: Boolean(nextPage.hasMore),
        })
      } finally {
        if (requestId === requestIdRef.current) {
          setLoading(false)
          setRefreshing(false)
        }
      }
    },
    [debouncedKeyword, levelFilter, pageSize]
  )

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setDebouncedKeyword(keyword.trim())
    }, SEARCH_DEBOUNCE_MS)
    return () => window.clearTimeout(timer)
  }, [keyword])

  useEffect(() => {
    setFollowLatest(true)
    loadPage(-1)
  }, [loadPage])

  useEffect(() => {
    if (!followLatest) return
    const timer = window.setInterval(() => {
      loadPage(-1, false)
    }, AUTO_REFRESH_MS)
    return () => window.clearInterval(timer)
  }, [followLatest, loadPage])

  useEffect(() => {
    if (followLatest) {
      bottomRef.current?.scrollIntoView({ behavior: 'smooth', block: 'end' })
    }
  }, [followLatest, rows])

  const handleClear = async () => {
    requestIdRef.current += 1
    setLoading(true)
    try {
      await clearLogs()
      setPage(emptyPage(pageSize))
      setFollowLatest(true)
    } finally {
      setLoading(false)
      setRefreshing(false)
    }
  }

  const goToOffset = (offset: number) => {
    setFollowLatest(offset < 0)
    loadPage(offset)
  }

  return (
    <div className="space-y-4 animate-fade-in">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h1 className="text-xl font-semibold text-[var(--color-text-primary)]">日志查看</h1>
          <p className="text-sm text-[var(--color-text-muted)] mt-1">
            {rangeStart}-{rangeEnd} / {page.total} 条
            {refreshing ? ' · 刷新中' : ''}
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="secondary" size="sm" onClick={() => loadPage(followLatest ? -1 : page.offset)} loading={loading}>
            <RefreshCw className="w-4 h-4" />刷新
          </Button>
          <Button variant="secondary" size="sm" onClick={handleClear}>
            <Trash2 className="w-4 h-4" />清空
          </Button>
        </div>
      </div>

      <div className="flex items-center gap-3 flex-wrap">
        <div className="flex gap-1">
          {LEVELS.map((level) => (
            <button
              key={level}
              onClick={() => setLevelFilter(level)}
              className={`h-8 px-2.5 text-xs rounded-md transition-colors ${
                levelFilter === level
                  ? 'bg-[var(--color-accent)] text-white'
                  : 'bg-[var(--color-bg-muted)] text-[var(--color-text-muted)] hover:text-[var(--color-text-primary)]'
              }`}
            >
              {level}
            </button>
          ))}
        </div>

        <input
          value={keyword}
          onChange={(event) => setKeyword(event.target.value)}
          placeholder="搜索消息、组件或字段"
          className="h-8 px-3 text-sm rounded-md border border-[var(--color-border-default)] bg-[var(--color-bg-secondary)] text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-[var(--color-accent)] w-56"
        />

        <Select
          value={String(pageSize)}
          onChange={(event) => setPageSize(Number(event.target.value))}
          options={PAGE_SIZES.map((size) => ({ value: String(size), label: `${size} 条/页` }))}
          className="w-28 h-8 text-xs"
        />

        <label className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)] cursor-pointer select-none ml-auto">
          <input
            type="checkbox"
            checked={followLatest}
            onChange={(event) => {
              setFollowLatest(event.target.checked)
              if (event.target.checked) loadPage(-1)
            }}
            className="w-3.5 h-3.5"
          />
          跟随最新
        </label>
      </div>

      <Card padding="none">
        <div
          className="overflow-auto font-mono text-xs"
          style={{ maxHeight: 'calc(100vh - 314px)' }}
        >
          {rows.length === 0 ? (
            <div className="py-16 text-center text-sm text-[var(--color-text-muted)]">暂无日志</div>
          ) : (
            <table className="min-w-full table-fixed">
              <thead className="sticky top-0 z-10 bg-[var(--color-bg-muted)]">
                <tr>
                  <th className="px-3 py-2 text-left text-[var(--color-text-muted)] font-semibold w-40">时间</th>
                  <th className="px-3 py-2 text-left text-[var(--color-text-muted)] font-semibold w-20">级别</th>
                  <th className="px-3 py-2 text-left text-[var(--color-text-muted)] font-semibold w-32">组件</th>
                  <th className="px-3 py-2 text-left text-[var(--color-text-muted)] font-semibold">消息</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--color-border-muted)]">
                {rows.map((entry) => (
                  <tr key={entry.rowId} className="hover:bg-[var(--color-bg-muted)]/40">
                    <td className="px-3 py-1.5 text-[var(--color-text-muted)] whitespace-nowrap">{entry.time}</td>
                    <td className="px-3 py-1.5">
                      <Badge variant={levelVariant(entry.level)} className="text-[10px]">{entry.level}</Badge>
                    </td>
                    <td className="px-3 py-1.5 text-[var(--color-text-muted)] truncate" title={entry.component}>
                      {entry.component}
                    </td>
                    <td className={`px-3 py-1.5 whitespace-pre-wrap break-words ${levelColor(entry.level)}`}>
                      <span>{entry.message}</span>
                      {entry.fieldText && (
                        <span className="ml-2 text-[var(--color-text-muted)]">{entry.fieldText}</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <div ref={bottomRef} />
        </div>
      </Card>

      <div className="flex items-center justify-between gap-3 text-xs text-[var(--color-text-muted)]">
        <span>第 {currentPage} / {totalPages} 页</span>
        <div className="flex items-center gap-1">
          <Button variant="secondary" size="sm" onClick={() => goToOffset(0)} disabled={!canPrevious || loading}>
            <ChevronsLeft className="w-4 h-4" />
          </Button>
          <Button variant="secondary" size="sm" onClick={() => goToOffset(Math.max(0, page.offset - pageSize))} disabled={!canPrevious || loading}>
            <ChevronLeft className="w-4 h-4" />
          </Button>
          <Button variant="secondary" size="sm" onClick={() => goToOffset(page.offset + pageSize)} disabled={!canNext || loading}>
            <ChevronRight className="w-4 h-4" />
          </Button>
          <Button variant="secondary" size="sm" onClick={() => goToOffset(-1)} disabled={loading}>
            <ChevronsRight className="w-4 h-4" />
            最新
          </Button>
        </div>
      </div>
    </div>
  )
}
