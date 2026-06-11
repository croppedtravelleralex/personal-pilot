import { useEffect, useState, useRef, useCallback, useMemo } from 'react'
import { Activity, AlertTriangle, Info, AlertCircle, XCircle, Search, Filter, Trash2, Pause, Play, Download, Clock, ChevronLeft, ChevronRight } from 'lucide-react'
import { Card, Button, toast } from '../../shared/components'
import {
  countEventLog,
  desktopRuntimeListen,
  exportEventLog,
  pruneEventLog,
  queryEventLog,
} from '../../services/desktop'
import type { DesktopEventLogEntry, DesktopEventLogQueryInput } from '../../services/desktop'

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

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error && error.message ? error.message : fallback
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

type MonitorTab = 'live' | 'history'

export function EventMonitorPage() {
  const [activeTab, setActiveTab] = useState<MonitorTab>('live')
  const [events, setEvents] = useState<EventEntry[]>([])
  const [paused, setPaused] = useState(false)
  const [search, setSearch] = useState('')
  const [filterNs, setFilterNs] = useState<string>('')
  const [filterSev, setFilterSev] = useState<Severity | ''>('')
  const nextId = useRef(0)
  const pausedBuffer = useRef<EventEntry[]>([])
  const pausedRef = useRef(paused)

  useEffect(() => {
    pausedRef.current = paused
  }, [paused])

  // Subscribe to all events
  useEffect(() => {
    const unsubs: (() => void)[] = []

    for (const eventName of ALL_EVENT_NAMES) {
      const sev = eventToSeverity(eventName)
      const ns = eventNamespace(eventName)

      const unsub = desktopRuntimeListen(eventName, (data: unknown) => {
        const entry: EventEntry = {
          id: nextId.current++,
          name: eventName,
          namespace: ns,
          severity: sev,
          payload: extractPayload(data),
          timestamp: new Date(),
        }

        if (pausedRef.current) {
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
    if (search) {
      const q = search.toLowerCase()
      result = result.filter(e =>
        e.name.toLowerCase().includes(q) ||
        JSON.stringify(e.payload).toLowerCase().includes(q)
      )
    }
    if (filterNs) result = result.filter(e => e.namespace === filterNs)
    if (filterSev) result = result.filter(e => e.severity === filterSev)
    return result
  }, [events, search, filterNs, filterSev])

  return (
    <div className="flex h-full flex-col gap-4 overflow-hidden p-6">
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
            <div className="h-full overflow-auto">
              <table className="w-full text-sm">
                <thead className="sticky top-0 bg-[var(--color-bg-card)] border-b border-[var(--color-border-default)]">
                  <tr>
                    <th className="px-4 py-2.5 text-left font-medium text-[var(--color-text-muted)] w-[60px]">级别</th>
                    <th className="px-4 py-2.5 text-left font-medium text-[var(--color-text-muted)]">事件名</th>
                    <th className="px-4 py-2.5 text-left font-medium text-[var(--color-text-muted)] w-[100px]">命名空间</th>
                    <th className="px-4 py-2.5 text-left font-medium text-[var(--color-text-muted)]">Payload</th>
                    <th className="px-4 py-2.5 text-right font-medium text-[var(--color-text-muted)] w-[100px]">时间</th>
                  </tr>
                </thead>
                <tbody>
                  {filtered.length === 0 && (
                    <tr>
                      <td colSpan={5} className="px-4 py-12 text-center text-[var(--color-text-muted)]">
                        {events.length === 0 ? (
                          <div className="flex flex-col items-center gap-2">
                            <Filter className="h-8 w-8 opacity-40" />
                            <p>等待事件...</p>
                            <p className="text-xs">启动浏览器或触发操作以查看实时事件</p>
                          </div>
                        ) : (
                          '没有匹配的事件'
                        )}
                      </td>
                    </tr>
                  )}
                  {filtered.map(entry => (
                    <tr
                      key={entry.id}
                      className={`border-b border-[var(--color-border-default)] transition-colors hover:bg-[var(--color-bg-hover)] ${
                        entry.severity === 'critical' ? 'bg-red-50/30' :
                        entry.severity === 'error' ? 'bg-orange-50/20' : ''
                      }`}
                    >
                      <td className="px-4 py-2.5">
                        <SeverityBadge severity={entry.severity} />
                      </td>
                      <td className="px-4 py-2.5 font-mono text-xs text-[var(--color-text-primary)]">
                        {entry.name}
                      </td>
                      <td className="px-4 py-2.5">
                        <span className="rounded bg-[var(--color-bg-hover)] px-1.5 py-0.5 text-xs font-mono text-[var(--color-text-muted)]">
                          {entry.namespace}
                        </span>
                      </td>
                      <td className="max-w-[300px] px-4 py-2.5">
                        <pre className="truncate text-xs text-[var(--color-text-muted)]">
                          {JSON.stringify(entry.payload)}
                        </pre>
                      </td>
                      <td className="px-4 py-2.5 text-right text-xs text-[var(--color-text-muted)] whitespace-nowrap">
                        {entry.timestamp.toLocaleTimeString('zh-CN', { hour12: false })}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
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
  const [entries, setEntries] = useState<DesktopEventLogEntry[]>([])
  const [totalCount, setTotalCount] = useState(0)
  const [loading, setLoading] = useState(false)
  const [page, setPage] = useState(0)
  const [selectedNs, setSelectedNs] = useState<string[]>([])

  const totalPages = Math.max(1, Math.ceil(totalCount / PAGE_SIZE))

  const buildQuery = useCallback((): DesktopEventLogQueryInput => {
    const range = TIME_RANGES.find(r => r.key === timeRange) || TIME_RANGES[0]
    const after = new Date(Date.now() - range.hours * 3600 * 1000).toISOString()
    return {
      after,
      before: '',
      namespace: filterNs,
      severity: filterSev,
      eventName: filterName,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    }
  }, [timeRange, filterNs, filterSev, filterName, page])

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const q = buildQuery()
      const [result, count] = await Promise.all([
        queryEventLog(q),
        countEventLog(q),
      ])
      setEntries(result || [])
      setTotalCount(count || 0)
    } catch {
      setEntries([])
      setTotalCount(0)
    } finally {
      setLoading(false)
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
        const result = await queryEventLog(q)
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
      const json = await exportEventLog(q)
      const blob = new Blob([json], { type: 'application/json' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = `event-log-${new Date().toISOString().slice(0, 19).replace(/:/g, '-')}.json`
      a.click()
      URL.revokeObjectURL(url)
      toast.success('导出成功')
    } catch (error: unknown) {
      toast.error(errorMessage(error, '导出失败'))
    }
  }

  const handlePrune = async () => {
    if (!window.confirm('确定要清理当前时间范围之前的所有日志吗？此操作不可撤销。')) return
    try {
      const range = TIME_RANGES.find(r => r.key === timeRange) || TIME_RANGES[0]
      const before = new Date(Date.now() - range.hours * 3600 * 1000).toISOString()
      const count = await pruneEventLog(before)
      toast.success(`已清理 ${count} 条日志`)
      await refresh()
    } catch (error: unknown) {
      toast.error(errorMessage(error, '清理失败'))
    }
  }

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
        <div className="overflow-auto max-h-[500px]">
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-[var(--color-bg-card)] border-b border-[var(--color-border-default)]">
              <tr>
                <th className="px-4 py-2.5 text-left font-medium text-[var(--color-text-muted)] w-[60px]">级别</th>
                <th className="px-4 py-2.5 text-left font-medium text-[var(--color-text-muted)]">事件名</th>
                <th className="px-4 py-2.5 text-left font-medium text-[var(--color-text-muted)] w-[100px]">命名空间</th>
                <th className="px-4 py-2.5 text-left font-medium text-[var(--color-text-muted)]">Payload</th>
                <th className="px-4 py-2.5 text-right font-medium text-[var(--color-text-muted)] w-[160px]">时间</th>
              </tr>
            </thead>
            <tbody>
              {loading && (
                <tr>
                  <td colSpan={5} className="px-4 py-12 text-center text-[var(--color-text-muted)]">
                    <Clock className="h-6 w-6 mx-auto mb-2 animate-spin opacity-40" />
                    <p>加载中...</p>
                  </td>
                </tr>
              )}
              {!loading && entries.length === 0 && (
                <tr>
                  <td colSpan={5} className="px-4 py-12 text-center text-[var(--color-text-muted)]">
                    暂无历史记录
                  </td>
                </tr>
              )}
              {!loading && entries.map(entry => (
                <tr
                  key={entry.id}
                  className={`border-b border-[var(--color-border-default)] transition-colors hover:bg-[var(--color-bg-hover)] ${
                    entry.severity === 'critical' ? 'bg-red-50/30' :
                    entry.severity === 'error' ? 'bg-orange-50/20' : ''
                  }`}
                >
                  <td className="px-4 py-2.5">
                    <SeverityBadge severity={entry.severity as Severity} />
                  </td>
                  <td className="px-4 py-2.5 font-mono text-xs text-[var(--color-text-primary)]">
                    {entry.eventName}
                  </td>
                  <td className="px-4 py-2.5">
                    <span className="rounded bg-[var(--color-bg-hover)] px-1.5 py-0.5 text-xs font-mono text-[var(--color-text-muted)]">
                      {entry.namespace}
                    </span>
                  </td>
                  <td className="max-w-[300px] px-4 py-2.5">
                    <pre className="truncate text-xs text-[var(--color-text-muted)]">
                      {typeof entry.payload === 'string' ? entry.payload : JSON.stringify(entry.payload)}
                    </pre>
                  </td>
                  <td className="px-4 py-2.5 text-right text-xs text-[var(--color-text-muted)] whitespace-nowrap">
                    {new Date(entry.createdAt).toLocaleString('zh-CN', { hour12: false })}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
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
