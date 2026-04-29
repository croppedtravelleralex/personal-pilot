import { useEffect, useState, useRef, useCallback, useMemo } from 'react'
import { Activity, AlertTriangle, Info, AlertCircle, XCircle, Search, Trash2, Pause, Play, Download, ChevronLeft, ChevronRight } from 'lucide-react'
import { Card, Button, Table, toast } from '../../shared/components'
import type { TableColumn } from '../../shared/components'
import {
  EventLogQuery,
  EventLogCount,
  EventLogPrune,
  EventLogExport,
} from '../../wailsjs/go/main/App'
import type { backend, events } from '../../wailsjs/go/models'

// ─── Types ────────────────────────────────────────────────────────────────────

type Severity = 'info' | 'warn' | 'error' | 'critical'

interface EventEntry {
  id: number
  name: string
  namespace: string
  severity: Severity
  payload: Record<string, unknown>
  timestamp: Date
}

interface NamespaceStats {
  namespace: string
  total: number
  info: number
  warn: number
  error: number
  critical: number
}

// ─── All known event names (synced from backend/internal/events/registry.go) ──
const ALL_EVENT_NAMES = [
  // Browser
  'browser:instance:started', 'browser:instance:stopped', 'browser:instance:crashed', 'browser:instance:updated',
  // Proxy
  'proxy:bridge:died', 'proxy:bridge:failed',
  'proxy:speed:result', 'proxy:iphealth:result',
  // Risk: Fingerprint
  'risk:fingerprint:mismatch', 'risk:fingerprint:timezone-ip',
  // Risk: Proxy
  'risk:proxy:high-latency', 'risk:proxy:health-drop', 'risk:proxy:datacenter', 'risk:proxy:auth-failure',
  // Risk: Network
  'risk:webrtc:leak', 'risk:dns:leak',
  // Risk: Captcha
  'risk:captcha:detected', 'risk:captcha:failed',
  // Risk: Browser/Session/System/Profile/Node
  'risk:browser:crash-loop',
  'risk:session:rate-limit', 'risk:session:cookie-cleared', 'risk:session:security-challenge',
  'risk:system:low-disk', 'risk:system:memory-pressure',
  'risk:profile:corrupted',
  'risk:node:banned', 'risk:node:geo-jump', 'risk:node:offline',
  // App / Download
  'app:request-close',
  'download:progress',
  // Phase 2: Account
  'account:login:attempt', 'account:login:success', 'account:login:failed',
  'account:login:captcha', 'account:login:verify', 'account:login:blocked',
  'account:login:timeout', 'account:login:session-expired',
  'account:profile:updated', 'account:profile:avatar-changed',
  'account:profile:verified', 'account:profile:reported',
  'account:follow:follow', 'account:follow:unfollow',
  'account:follow:block', 'account:follow:mute',
  // Phase 2: Content
  'content:publish:draft', 'content:publish:scheduled', 'content:publish:submitted',
  'content:publish:processing', 'content:publish:published', 'content:publish:failed',
  'content:publish:reviewed', 'content:publish:deleted',
  'content:image:upload-start', 'content:image:progress', 'content:image:done', 'content:image:failed',
  'content:comment:post', 'content:comment:reply', 'content:comment:delete', 'content:comment:report',
  'content:like:like', 'content:like:unlike',
  // Phase 2: Automation
  'automation:task:created', 'automation:task:started', 'automation:task:progress',
  'automation:task:paused', 'automation:task:resumed', 'automation:task:retried',
  'automation:task:completed', 'automation:task:failed', 'automation:task:cancelled', 'automation:task:expired',
  'automation:batch:started', 'automation:batch:item-complete', 'automation:batch:summary', 'automation:batch:failed',
  // Phase 2: Proxy quality
  'proxy:quality:latency-spike', 'proxy:quality:health-drop', 'proxy:quality:node-rotated',
  'proxy:quality:pool-exhausted', 'proxy:quality:best-node', 'proxy:quality:score-change',
  'proxy:quality:node-added', 'proxy:quality:node-removed',
  'proxy:rotation:triggered', 'proxy:rotation:completed', 'proxy:rotation:skipped', 'proxy:rotation:failed',
  // Phase 2: Data
  'data:scrape:started', 'data:scrape:progress', 'data:scrape:page-done',
  'data:scrape:rate-limited', 'data:scrape:completed', 'data:scrape:exported',
]

// ─── Helpers ──────────────────────────────────────────────────────────────────

function eventNamespace(name: string): string {
  const idx = name.indexOf(':')
  if (idx === -1) return name
  const idx2 = name.indexOf(':', idx + 1)
  if (idx2 === -1) return name.substring(0, idx)
  return name.substring(0, idx2)
}

function eventToSeverity(name: string): Severity {
  if (name.includes(':critical') || name.includes('crash-loop') || name.includes('leak') ||
      name.includes('banned') || name.includes('corrupted') || name.includes('pool-exhausted') ||
      name.includes('blocked') || name.includes('reported') || name.includes('low-disk')) {
    return 'critical'
  }
  if (name.includes(':failed') || name.includes(':error') || name.includes('auth-failure') ||
      name.includes('offline') || name.includes('health-drop')) {
    return 'error'
  }
  if (name.includes(':warn') || name.includes('mismatch') || name.includes('detected') ||
      name.includes('rate-limit') || name.includes('retried') || name.includes('expired') ||
      name.includes('latency-spike') || name.includes('cleared') || name.includes('challenge') ||
      name.includes('datacenter') || name.includes('pressure') || name.includes('geo-jump') ||
      name.includes('skipped') || name.includes('captcha') || name.includes('verify') ||
      name.includes('session-expired') || name.includes('node-removed')) {
    return 'warn'
  }
  return 'info'
}

function extractPayload(data: unknown): Record<string, unknown> {
  if (data && typeof data === 'object' && !Array.isArray(data)) {
    return data as Record<string, unknown>
  }
  if (typeof data === 'string') return { value: data }
  if (typeof data === 'number' || typeof data === 'boolean') return { value: data }
  return {}
}

// ─── Severity Badge ───────────────────────────────────────────────────────────

const SEVERITY_CONFIG: Record<Severity, { icon: React.ReactNode; className: string; label: string }> = {
  info:    { icon: <Info className="h-3.5 w-3.5" />, className: 'bg-blue-100 text-blue-700 border-blue-200', label: 'INFO' },
  warn:    { icon: <AlertTriangle className="h-3.5 w-3.5" />, className: 'bg-amber-100 text-amber-700 border-amber-200', label: 'WARN' },
  error:   { icon: <AlertCircle className="h-3.5 w-3.5" />, className: 'bg-orange-100 text-orange-700 border-orange-200', label: 'ERROR' },
  critical:{ icon: <XCircle className="h-3.5 w-3.5" />, className: 'bg-red-100 text-red-700 border-red-200', label: 'CRIT' },
}

function SeverityBadge({ severity }: { severity: Severity }) {
  const cfg = SEVERITY_CONFIG[severity]
  return (
    <span className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs font-medium ${cfg.className}`}>
      {cfg.icon}
      {cfg.label}
    </span>
  )
}

// ─── Component ─────────────────────────────────────────────────────────────────

const MAX_EVENTS = 500
const SEARCH_DEBOUNCE_MS = 300
const EVENT_ROW_HEIGHT = 56
const EVENT_TABLE_TINT_STYLE = `
.event-monitor-table tbody tr:has([data-event-severity="critical"]) {
  background-color: rgb(254 242 242 / 0.3);
}
.event-monitor-table tbody tr:has([data-event-severity="error"]) {
  background-color: rgb(255 247 237 / 0.2);
}
.event-monitor-table tbody tr:hover {
  background-color: var(--color-bg-hover);
}
`

type MonitorTab = 'live' | 'history'

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value)

  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs)
    return () => window.clearTimeout(timer)
  }, [value, delayMs])

  return debounced
}

export function EventMonitorPage() {
  const [activeTab, setActiveTab] = useState<MonitorTab>('live')
  const [events, setEvents] = useState<EventEntry[]>([])
  const [paused, setPaused] = useState(false)
  const [search, setSearch] = useState('')
  const debouncedSearch = useDebouncedValue(search, SEARCH_DEBOUNCE_MS)
  const [filterNs, setFilterNs] = useState<string>('')
  const [filterSev, setFilterSev] = useState<Severity | ''>('')
  const nextId = useRef(0)
  const pausedBuffer = useRef<EventEntry[]>([])

  // Subscribe to all events
  useEffect(() => {
    const runtime = (window as any).runtime
    if (!runtime?.EventsOn) return

    const unsubs: (() => void)[] = []

    for (const eventName of ALL_EVENT_NAMES) {
      const sev = eventToSeverity(eventName)
      const ns = eventNamespace(eventName)

      const unsub = runtime.EventsOn(eventName, (data: unknown) => {
        const entry: EventEntry = {
          id: nextId.current++,
          name: eventName,
          namespace: ns,
          severity: sev,
          payload: extractPayload(data),
          timestamp: new Date(),
        }

        if (paused) {
          pausedBuffer.current.push(entry)
        } else {
          setEvents(prev => {
            const next = [entry, ...prev]
            return next.length > MAX_EVENTS ? next.slice(0, MAX_EVENTS) : next
          })
        }
      })

      unsubs.push(unsub)
    }

    return () => {
      for (const unsub of unsubs) {
        try { unsub() } catch { /* ignore */ }
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Resume: flush paused buffer
  const handleTogglePause = useCallback(() => {
    if (paused) {
      const buffered = pausedBuffer.current
      pausedBuffer.current = []
      if (buffered.length > 0) {
        setEvents(prev => {
          const next = [...buffered.reverse(), ...prev]
          return next.length > MAX_EVENTS ? next.slice(0, MAX_EVENTS) : next
        })
      }
    }
    setPaused(p => !p)
  }, [paused])

  const handleClear = useCallback(() => {
    setEvents([])
    pausedBuffer.current = []
    nextId.current = 0
  }, [])

  // Stats
  const stats = useMemo(() => {
    const nsMap = new Map<string, NamespaceStats>()
    let totalInfo = 0, totalWarn = 0, totalError = 0, totalCritical = 0

    for (const e of events) {
      let s = nsMap.get(e.namespace)
      if (!s) {
        s = { namespace: e.namespace, total: 0, info: 0, warn: 0, error: 0, critical: 0 }
        nsMap.set(e.namespace, s)
      }
      s.total++
      switch (e.severity) {
        case 'info': s.info++; totalInfo++; break
        case 'warn': s.warn++; totalWarn++; break
        case 'error': s.error++; totalError++; break
        case 'critical': s.critical++; totalCritical++; break
      }
    }

    return { byNs: Array.from(nsMap.values()).sort((a, b) => b.total - a.total), totalInfo, totalWarn, totalError, totalCritical }
  }, [events])

  // Filtered events
  const filtered = useMemo(() => {
    let result = events
    if (debouncedSearch) {
      const q = debouncedSearch.toLowerCase()
      result = result.filter(e =>
        e.name.toLowerCase().includes(q) ||
        JSON.stringify(e.payload).toLowerCase().includes(q)
      )
    }
    if (filterNs) result = result.filter(e => e.namespace === filterNs)
    if (filterSev) result = result.filter(e => e.severity === filterSev)
    return result
  }, [events, debouncedSearch, filterNs, filterSev])

  const liveColumns = useMemo<TableColumn<EventEntry>[]>(() => [
    {
      key: 'severity',
      title: '级别',
      width: 80,
      render: (_value, entry) => (
        <span data-event-severity={entry.severity}>
          <SeverityBadge severity={entry.severity} />
        </span>
      ),
    },
    {
      key: 'name',
      title: '事件名',
      render: (_value, entry) => (
        <span className="font-mono text-xs text-[var(--color-text-primary)]">{entry.name}</span>
      ),
    },
    {
      key: 'namespace',
      title: '命名空间',
      width: 120,
      render: (_value, entry) => (
        <span className="rounded bg-[var(--color-bg-hover)] px-1.5 py-0.5 font-mono text-xs text-[var(--color-text-muted)]">
          {entry.namespace}
        </span>
      ),
    },
    {
      key: 'payload',
      title: 'Payload',
      render: (_value, entry) => (
        <pre className="max-w-[300px] truncate text-xs text-[var(--color-text-muted)]">
          {JSON.stringify(entry.payload)}
        </pre>
      ),
    },
    {
      key: 'timestamp',
      title: '时间',
      width: 110,
      align: 'right',
      render: (_value, entry) => (
        <span className="whitespace-nowrap text-xs text-[var(--color-text-muted)]">
          {entry.timestamp.toLocaleTimeString('zh-CN', { hour12: false })}
        </span>
      ),
    },
  ], [])

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden p-6">
      <style>{EVENT_TABLE_TINT_STYLE}</style>
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-[var(--color-text-primary)]">事件监控</h1>
          <p className="mt-1 text-sm text-[var(--color-text-muted)]">
            实时显示所有浏览器和运营事件，共 {ALL_EVENT_NAMES.length} 种事件类型
          </p>
        </div>
        {activeTab === 'live' && (
          <div className="flex items-center gap-2">
            <button
              onClick={handleTogglePause}
              className={`inline-flex items-center gap-2 rounded-lg border px-4 py-2 text-sm font-medium transition-colors ${
                paused
                  ? 'border-amber-200 bg-amber-50 text-amber-700 hover:bg-amber-100'
                  : 'border-[var(--color-border-default)] bg-[var(--color-bg-card)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)]'
              }`}
            >
              {paused ? <><Play className="h-4 w-4" /> 继续</> : <><Pause className="h-4 w-4" /> 暂停</>}
            </button>
            <button
              onClick={handleClear}
              className="inline-flex items-center gap-2 rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-card)] px-4 py-2 text-sm font-medium text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] transition-colors"
            >
              <Trash2 className="h-4 w-4" />
              清空
            </button>
          </div>
        )}
      </div>

      {/* Tab bar */}
      <div className="flex border-b border-[var(--color-border-default)]">
        <button
          onClick={() => setActiveTab('live')}
          className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 ${
            activeTab === 'live'
              ? 'border-[var(--color-primary)] text-[var(--color-primary)]'
              : 'border-transparent text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]'
          }`}
        >
          实时监控
        </button>
        <button
          onClick={() => setActiveTab('history')}
          className={`px-4 py-2 text-sm font-medium transition-colors border-b-2 ${
            activeTab === 'history'
              ? 'border-[var(--color-primary)] text-[var(--color-primary)]'
              : 'border-transparent text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]'
          }`}
        >
          历史记录
        </button>
      </div>

      {activeTab === 'live' && (
        <>
          {/* Stats cards */}
          <div className="grid grid-cols-6 gap-3">
            <StatCard label="事件总数" value={events.length} color="bg-blue-500" icon={<Activity className="h-4 w-4 text-white" />} />
            <StatCard label="INFO" value={stats.totalInfo} color="bg-blue-400" icon={<Info className="h-4 w-4 text-white" />} />
            <StatCard label="WARN" value={stats.totalWarn} color="bg-amber-400" icon={<AlertTriangle className="h-4 w-4 text-white" />} />
            <StatCard label="ERROR" value={stats.totalError} color="bg-orange-400" icon={<AlertCircle className="h-4 w-4 text-white" />} />
            <StatCard label="CRITICAL" value={stats.totalCritical} color="bg-red-500" icon={<XCircle className="h-4 w-4 text-white" />} />
            <StatCard label="已监听" value={ALL_EVENT_NAMES.length} color="bg-emerald-500" icon={<Activity className="h-4 w-4 text-white" />} />
          </div>

          {/* Per-namespace stats */}
          {stats.byNs.length > 0 && (
            <Card className="p-4">
              <h3 className="mb-2 text-sm font-medium text-[var(--color-text-primary)]">命名空间分布</h3>
              <div className="flex flex-wrap gap-2">
                {stats.byNs.map(ns => (
                  <button
                    key={ns.namespace}
                    onClick={() => setFilterNs(filterNs === ns.namespace ? '' : ns.namespace)}
                    className={`inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 text-xs font-medium transition-colors ${
                      filterNs === ns.namespace
                        ? 'border-blue-300 bg-blue-50 text-blue-700'
                        : 'border-[var(--color-border-default)] bg-[var(--color-bg-card)] text-[var(--color-text-muted)] hover:bg-[var(--color-bg-hover)]'
                    }`}
                  >
                    {ns.namespace}
                    <span className={filterNs === ns.namespace ? 'text-blue-500' : 'text-[var(--color-text-muted)]'}>
                      ({ns.total})
                    </span>
                  </button>
                ))}
                {filterNs && (
                  <button
                    onClick={() => setFilterNs('')}
                    className="inline-flex items-center rounded-md border border-red-200 bg-red-50 px-2 py-1 text-xs text-red-600 hover:bg-red-100"
                  >
                    清除
                  </button>
                )}
              </div>
            </Card>
          )}

          {/* Filters bar */}
          <div className="flex items-center gap-3">
            <div className="relative flex-1">
              <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[var(--color-text-muted)]" />
              <input
                type="text"
                placeholder="搜索事件名或 payload..."
                value={search}
                onChange={e => setSearch(e.target.value)}
                className="w-full rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-card)] py-2 pl-9 pr-4 text-sm text-[var(--color-text-primary)] placeholder:text-[var(--color-text-muted)] focus:border-blue-400 focus:outline-none"
              />
            </div>
            <select
              value={filterSev}
              onChange={e => setFilterSev(e.target.value as Severity | '')}
              className="rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-card)] px-3 py-2 text-sm text-[var(--color-text-primary)] focus:border-blue-400 focus:outline-none"
            >
              <option value="">全部级别</option>
              <option value="info">INFO</option>
              <option value="warn">WARN</option>
              <option value="error">ERROR</option>
              <option value="critical">CRITICAL</option>
            </select>
          </div>

          {/* Event table */}
          <div className="flex-1 overflow-hidden rounded-lg border border-[var(--color-border-default)]">
            <Table
              columns={liveColumns}
              data={filtered}
              rowKey={entry => String(entry.id)}
              emptyText={events.length === 0 ? '等待事件...' : '没有匹配的事件'}
              maxHeight="100%"
              className="event-monitor-table h-full text-sm"
              virtualized
              virtualRowHeight={EVENT_ROW_HEIGHT}
            />
          </div>
        </>
      )}

      {activeTab === 'history' && <HistoryTab />}
    </div>
  )
}

// ─── History Tab ───────────────────────────────────────────────────────────────

const TIME_RANGES: { key: string; label: string; hours: number }[] = [
  { key: '1h', label: '最近 1 小时', hours: 1 },
  { key: '6h', label: '最近 6 小时', hours: 6 },
  { key: '24h', label: '最近 24 小时', hours: 24 },
  { key: '7d', label: '最近 7 天', hours: 24 * 7 },
]

const PAGE_SIZE = 50

function HistoryTab() {
  const [timeRange, setTimeRange] = useState('1h')
  const [filterNs, setFilterNs] = useState('')
  const [filterSev, setFilterSev] = useState('')
  const [filterName, setFilterName] = useState('')
  const debouncedFilterName = useDebouncedValue(filterName, SEARCH_DEBOUNCE_MS)
  const [entries, setEntries] = useState<events.EventLogEntry[]>([])
  const [totalCount, setTotalCount] = useState(0)
  const [loading, setLoading] = useState(false)
  const [page, setPage] = useState(0)
  const [selectedNs, setSelectedNs] = useState<string[]>([])
  const refreshRequestId = useRef(0)

  const totalPages = Math.max(1, Math.ceil(totalCount / PAGE_SIZE))

  const buildQuery = useCallback((): backend.EventLogQueryInput => {
    const range = TIME_RANGES.find(r => r.key === timeRange) || TIME_RANGES[0]
    const after = new Date(Date.now() - range.hours * 3600 * 1000).toISOString()
    return {
      after,
      before: '',
      namespace: filterNs,
      severity: filterSev,
      eventName: debouncedFilterName,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    }
  }, [timeRange, filterNs, filterSev, debouncedFilterName, page])

  const refresh = useCallback(async () => {
    const requestId = ++refreshRequestId.current
    setLoading(true)
    try {
      const q = buildQuery()
      const [result, count] = await Promise.all([
        EventLogQuery(q),
        EventLogCount(q),
      ])
      if (requestId !== refreshRequestId.current) return
      setEntries(result || [])
      setTotalCount(count || 0)
    } catch {
      if (requestId !== refreshRequestId.current) return
      setEntries([])
      setTotalCount(0)
    } finally {
      if (requestId === refreshRequestId.current) {
        setLoading(false)
      }
    }
  }, [buildQuery])

  // Refresh on query change
  useEffect(() => {
    void refresh()
  }, [refresh])

  // Load namespace list
  useEffect(() => {
    void (async () => {
      try {
        const q = {
          after: new Date(Date.now() - 7 * 24 * 3600 * 1000).toISOString(),
          before: '',
          namespace: '',
          severity: '',
          eventName: '',
          limit: 0,
          offset: 0,
        }
        const result = await EventLogQuery(q)
        const nsSet = new Set<string>()
        for (const e of (result || [])) {
          if (e.namespace) nsSet.add(e.namespace)
        }
        setSelectedNs(Array.from(nsSet).sort())
      } catch {
        setSelectedNs([])
      }
    })()
  }, [])

  const handleExport = async () => {
    try {
      const q = buildQuery()
      q.limit = 0 // no limit for export
      q.offset = 0
      const json = await EventLogExport(q)
      const blob = new Blob([json], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `event-log-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.json`
      a.click()
      URL.revokeObjectURL(url)
      toast.success('导出成功')
    } catch (e: any) {
      toast.error(e?.message || '导出失败')
    }
  }

  const handlePrune = async () => {
    if (!window.confirm('确定要清理当前时间范围之前的所有日志吗？此操作不可撤销。')) return
    try {
      const range = TIME_RANGES.find(r => r.key === timeRange) || TIME_RANGES[0]
      const before = new Date(Date.now() - range.hours * 3600 * 1000).toISOString()
      const count = await EventLogPrune(before)
      toast.success(`已清理 ${count} 条日志`)
      await refresh()
    } catch (e: any) {
      toast.error(e?.message || '清理失败')
    }
  }

  const historyColumns = useMemo<TableColumn<events.EventLogEntry>[]>(() => [
    {
      key: 'severity',
      title: '级别',
      width: 80,
      render: (_value, entry) => (
        <span data-event-severity={entry.severity}>
          <SeverityBadge severity={entry.severity as Severity} />
        </span>
      ),
    },
    {
      key: 'eventName',
      title: '事件名',
      render: (_value, entry) => (
        <span className="font-mono text-xs text-[var(--color-text-primary)]">{entry.eventName}</span>
      ),
    },
    {
      key: 'namespace',
      title: '命名空间',
      width: 120,
      render: (_value, entry) => (
        <span className="rounded bg-[var(--color-bg-hover)] px-1.5 py-0.5 font-mono text-xs text-[var(--color-text-muted)]">
          {entry.namespace}
        </span>
      ),
    },
    {
      key: 'payload',
      title: 'Payload',
      render: (_value, entry) => (
        <pre className="max-w-[300px] truncate text-xs text-[var(--color-text-muted)]">
          {typeof entry.payload === 'string' ? entry.payload : JSON.stringify(entry.payload)}
        </pre>
      ),
    },
    {
      key: 'createdAt',
      title: '时间',
      width: 180,
      align: 'right',
      render: (_value, entry) => (
        <span className="whitespace-nowrap text-xs text-[var(--color-text-muted)]">
          {new Date(entry.createdAt).toLocaleString('zh-CN', { hour12: false })}
        </span>
      ),
    },
  ], [])

  return (
    <div className="space-y-4">
      {/* Controls */}
      <div className="flex flex-wrap items-center gap-3">
        {/* Time range */}
        <div className="flex rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-card)] p-0.5">
          {TIME_RANGES.map(r => (
            <button
              key={r.key}
              onClick={() => { setTimeRange(r.key); setPage(0) }}
              className={`rounded px-3 py-1.5 text-xs font-medium transition-colors ${
                timeRange === r.key
                  ? 'bg-[var(--color-bg-hover)] text-[var(--color-text-primary)] shadow-sm'
                  : 'text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]'
              }`}
            >
              {r.key}
            </button>
          ))}
        </div>

        {/* Namespace filter */}
        <select
          value={filterNs}
          onChange={e => { setFilterNs(e.target.value); setPage(0) }}
          className="rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-card)] px-3 py-2 text-xs text-[var(--color-text-primary)]"
        >
          <option value="">全部命名空间</option>
          {selectedNs.map(ns => (
            <option key={ns} value={ns}>{ns}</option>
          ))}
        </select>

        {/* Severity filter */}
        <select
          value={filterSev}
          onChange={e => { setFilterSev(e.target.value); setPage(0) }}
          className="rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-card)] px-3 py-2 text-xs text-[var(--color-text-primary)]"
        >
          <option value="">全部级别</option>
          <option value="info">INFO</option>
          <option value="warn">WARN</option>
          <option value="error">ERROR</option>
          <option value="critical">CRITICAL</option>
        </select>

        {/* Search input */}
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--color-text-muted)]" />
          <input
            type="text"
            placeholder="搜索事件名..."
            value={filterName}
            onChange={e => { setFilterName(e.target.value); setPage(0) }}
            className="w-full rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-card)] py-2 pl-9 pr-4 text-xs text-[var(--color-text-primary)] placeholder:text-[var(--color-text-muted)]"
          />
        </div>

        <Button size="sm" variant="secondary" onClick={handleExport}>
          <Download className="w-3.5 h-3.5" /> 导出
        </Button>
        <Button size="sm" variant="ghost" onClick={handlePrune}>
          <Trash2 className="w-3.5 h-3.5 text-red-500" /> 清理旧日志
        </Button>
      </div>

      {/* Summary bar */}
      <div className="flex items-center gap-4 text-xs text-[var(--color-text-muted)]">
        <span>共 <strong className="text-[var(--color-text-primary)]">{totalCount}</strong> 条记录</span>
        {totalPages > 1 && (
          <>
            <span>第 {page + 1}/{totalPages} 页</span>
            <div className="flex gap-1">
              <button
                onClick={() => setPage(p => Math.max(0, p - 1))}
                disabled={page === 0}
                className="p-1 rounded hover:bg-[var(--color-bg-hover)] disabled:opacity-30"
              >
                <ChevronLeft className="w-3.5 h-3.5" />
              </button>
              <button
                onClick={() => setPage(p => Math.min(totalPages - 1, p + 1))}
                disabled={page >= totalPages - 1}
                className="p-1 rounded hover:bg-[var(--color-bg-hover)] disabled:opacity-30"
              >
                <ChevronRight className="w-3.5 h-3.5" />
              </button>
            </div>
          </>
        )}
      </div>

      {/* Table */}
      <div className="rounded-lg border border-[var(--color-border-default)] overflow-hidden">
        <Table
          columns={historyColumns}
          data={entries}
          rowKey={entry => String(entry.id)}
          loading={loading}
          emptyText="暂无历史记录"
          maxHeight="500px"
          className="event-monitor-table text-sm"
          virtualRowHeight={EVENT_ROW_HEIGHT}
        />
      </div>
    </div>
  )
}

// ─── Stat Card ─────────────────────────────────────────────────────────────────

function StatCard({ label, value, color, icon }: { label: string; value: number; color: string; icon: React.ReactNode }) {
  return (
    <div className="rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-card)] px-4 py-3">
      <div className="flex items-center gap-2">
        <div className={`flex h-7 w-7 items-center justify-center rounded-md ${color}`}>
          {icon}
        </div>
        <div>
          <div className="text-xs text-[var(--color-text-muted)]">{label}</div>
          <div className="text-lg font-semibold text-[var(--color-text-primary)]">{value}</div>
        </div>
      </div>
    </div>
  )
}
