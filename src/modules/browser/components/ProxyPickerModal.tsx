import { useEffect, useMemo, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Check, Clock3, Loader2, Search, Wifi, X } from 'lucide-react'
import type { BrowserProxy } from '../types'
import { browserProxyBatchTestSpeed, browserProxyTestSpeed, fetchBrowserProxies, fetchBrowserProxyGroups } from '../api'
import { desktopRuntimeListen } from '../../../services/desktop'

interface ProxyPickerModalProps {
  open: boolean
  currentProxyId: string
  onSelect: (proxy: BrowserProxy) => void
  onClose: () => void
}

type SpeedResult = { ok: boolean; latencyMs: number; error: string; testedAt?: string }

const ALL_GROUP = '__all__'
const BATCH_TEST_CONCURRENCY = 8

function getProxySpeed(proxy: BrowserProxy, speedMap: Record<string, SpeedResult>): SpeedResult | undefined {
  const live = speedMap[proxy.proxyId]
  if (live) return live
  if (!proxy.lastTestedAt) return undefined
  return {
    ok: proxy.lastTestOk ?? false,
    latencyMs: proxy.lastLatencyMs ?? -1,
    error: '',
    testedAt: proxy.lastTestedAt,
  }
}

function getProxySortTuple(proxy: BrowserProxy, speedMap: Record<string, SpeedResult>): [number, number, string] {
  if (proxy.proxyConfig === 'direct://') return [0, 0, proxy.proxyName || proxy.proxyId]
  const result = getProxySpeed(proxy, speedMap)
  if (result?.ok && result.latencyMs >= 0) return [1, result.latencyMs, proxy.proxyName || proxy.proxyId]
  if (result && !result.ok) return [2, Number.MAX_SAFE_INTEGER, proxy.proxyName || proxy.proxyId]
  return [3, Number.MAX_SAFE_INTEGER, proxy.proxyName || proxy.proxyId]
}

function formatLatency(result?: SpeedResult): string {
  if (!result) return '未测速'
  if (!result.ok) return '测速失败'
  if (result.latencyMs < 0) return '测速失败'
  return `${result.latencyMs} ms`
}

function formatTestedAt(value?: string): string {
  if (!value) return '无测试记录'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

function latencyColor(result?: SpeedResult): string {
  if (!result) return 'text-[var(--color-text-muted)]'
  if (!result.ok || result.latencyMs < 0) return 'text-red-500'
  if (result.latencyMs < 300) return 'text-green-600'
  if (result.latencyMs < 800) return 'text-yellow-600'
  return 'text-red-500'
}

function protocolLabel(proxyConfig: string): string {
  if (proxyConfig === 'direct://') return 'DIRECT'
  const match = proxyConfig.trim().match(/^([a-zA-Z0-9+\-]+):\/\//)
  if (match && match[1]) return match[1].toUpperCase()
  return 'YAML'
}

export function ProxyPickerModal({ open, currentProxyId, onSelect, onClose }: ProxyPickerModalProps) {
  const [groups, setGroups] = useState<string[]>([])
  const [allProxies, setAllProxies] = useState<BrowserProxy[]>([])
  const [selectedGroup, setSelectedGroup] = useState<string>(ALL_GROUP)
  const [search, setSearch] = useState('')
  const [loading, setLoading] = useState(false)
  const [speedMap, setSpeedMap] = useState<Record<string, SpeedResult>>({})
  const [testingIds, setTestingIds] = useState<Set<string>>(new Set())
  const abortRef = useRef(false)
  const pendingSpeedPatchRef = useRef<Record<string, SpeedResult>>({})
  const pendingTestingDoneIdsRef = useRef<Set<string>>(new Set())
  const speedPatchTimerRef = useRef<number | null>(null)

  const flushSpeedPatch = () => {
    speedPatchTimerRef.current = null
    const patch = pendingSpeedPatchRef.current
    const doneIds = pendingTestingDoneIdsRef.current
    pendingSpeedPatchRef.current = {}
    pendingTestingDoneIdsRef.current = new Set()
    if (abortRef.current) return
    if (Object.keys(patch).length > 0) {
      setSpeedMap(prev => ({ ...prev, ...patch }))
    }
    if (doneIds.size > 0) {
      setTestingIds(prev => {
        const next = new Set(prev)
        doneIds.forEach(id => next.delete(id))
        return next
      })
    }
  }

  const queueSpeedPatch = (proxyId: string, result: SpeedResult) => {
    pendingSpeedPatchRef.current[proxyId] = result
    pendingTestingDoneIdsRef.current.add(proxyId)
    if (speedPatchTimerRef.current !== null) return
    speedPatchTimerRef.current = window.setTimeout(flushSpeedPatch, 120)
  }

  useEffect(() => {
    if (!open) return
    setSelectedGroup(ALL_GROUP)
    setSearch('')
    setSpeedMap({})
    setTestingIds(new Set())
    abortRef.current = false
    void loadData()
    return () => {
      abortRef.current = true
      if (speedPatchTimerRef.current !== null) {
        window.clearTimeout(speedPatchTimerRef.current)
        speedPatchTimerRef.current = null
      }
    }
  }, [open])

  const loadData = async () => {
    setLoading(true)
    try {
      const [groupList, proxyList] = await Promise.all([
        fetchBrowserProxyGroups(),
        fetchBrowserProxies(),
      ])
      setGroups(groupList)
      setAllProxies(proxyList)
      const initMap: Record<string, SpeedResult> = {}
      proxyList.forEach(proxy => {
        if (proxy.lastTestedAt) {
          initMap[proxy.proxyId] = {
            ok: proxy.lastTestOk ?? false,
            latencyMs: proxy.lastLatencyMs ?? -1,
            error: '',
            testedAt: proxy.lastTestedAt,
          }
        }
      })
      setSpeedMap(initMap)
    } finally {
      setLoading(false)
    }
  }

  const displayProxies = useMemo(() => {
    let list = allProxies
    if (selectedGroup !== ALL_GROUP) {
      list = list.filter(proxy => proxy.groupName === selectedGroup)
    }
    if (search.trim()) {
      const q = search.trim().toLowerCase()
      list = list.filter(proxy =>
        (proxy.proxyName || '').toLowerCase().includes(q) ||
        (proxy.groupName || '').toLowerCase().includes(q) ||
        (proxy.proxyConfig || '').toLowerCase().includes(q)
      )
    }
    return [...list].sort((a, b) => {
      const [rankA, latencyA, nameA] = getProxySortTuple(a, speedMap)
      const [rankB, latencyB, nameB] = getProxySortTuple(b, speedMap)
      if (rankA !== rankB) return rankA - rankB
      if (latencyA !== latencyB) return latencyA - latencyB
      return nameA.localeCompare(nameB, 'zh-CN')
    })
  }, [allProxies, search, selectedGroup, speedMap])

  const bestAvailable = useMemo(() => {
    return displayProxies.find(proxy => {
      const result = getProxySpeed(proxy, speedMap)
      return result?.ok && result.latencyMs >= 0
    })
  }, [displayProxies, speedMap])

  const testOne = async (proxyId: string, event: React.MouseEvent) => {
    event.stopPropagation()
    if (testingIds.has(proxyId)) return
    setTestingIds(prev => new Set(prev).add(proxyId))
    try {
      const result = await browserProxyTestSpeed(proxyId)
      if (!abortRef.current) {
        setSpeedMap(prev => ({
          ...prev,
          [proxyId]: {
            ok: result.ok,
            latencyMs: result.latencyMs,
            error: result.error,
            testedAt: new Date().toISOString(),
          },
        }))
      }
    } finally {
      setTestingIds(prev => {
        const next = new Set(prev)
        next.delete(proxyId)
        return next
      })
    }
  }

  const testAll = async () => {
    const ids = displayProxies
      .map(proxy => proxy.proxyId)
      .filter(id => id !== '__direct__')
    if (ids.length === 0) return
    abortRef.current = false
    setTestingIds(new Set(ids))
    const idSet = new Set(ids)
    const off = desktopRuntimeListen('proxy:speed:result', (data: { proxyId: string; ok: boolean; latencyMs: number; error: string }) => {
      if (abortRef.current || !idSet.has(data.proxyId)) return
      queueSpeedPatch(data.proxyId, {
        ok: data.ok,
        latencyMs: data.latencyMs,
        error: data.error,
        testedAt: new Date().toISOString(),
      })
    })
    try {
      const results = await browserProxyBatchTestSpeed(ids, BATCH_TEST_CONCURRENCY)
      if (!abortRef.current) {
        setSpeedMap(prev => {
          const next = { ...prev }
          results.forEach(result => {
            if (idSet.has(result.proxyId)) {
              next[result.proxyId] = {
                ok: result.ok,
                latencyMs: result.latencyMs,
                error: result.error,
                testedAt: new Date().toISOString(),
              }
            }
          })
          return next
        })
      }
    } finally {
      off()
      setTestingIds(prev => {
        const next = new Set(prev)
        ids.forEach(id => next.delete(id))
        return next
      })
    }
  }

  if (!open) return null

  const bestResult = bestAvailable ? getProxySpeed(bestAvailable, speedMap) : undefined
  const resolveProxyWithSpeed = (proxy: BrowserProxy): BrowserProxy => {
    const result = getProxySpeed(proxy, speedMap)
    if (!result) return proxy
    return {
      ...proxy,
      lastLatencyMs: result.latencyMs,
      lastTestOk: result.ok,
      lastTestedAt: result.testedAt || proxy.lastTestedAt,
    }
  }

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center" onClick={onClose}>
      <div className="absolute inset-0 bg-black/50 backdrop-blur-sm" />
      <div
        className="relative flex max-h-[640px] w-[820px] flex-col overflow-hidden rounded-xl border border-[var(--color-border)] bg-[var(--color-bg-elevated)] shadow-2xl"
        onClick={event => event.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-[var(--color-border)] px-5 py-4">
          <div>
            <div className="font-semibold text-[var(--color-text-primary)]">选择代理</div>
            <div className="mt-1 text-xs text-[var(--color-text-muted)]">
              默认按最近测速排序，最快可用节点在前
            </div>
          </div>
          <button onClick={onClose} className="text-[var(--color-text-muted)] transition-colors hover:text-[var(--color-text-primary)]">
            <X className="h-4 w-4" />
          </button>
        </div>

        <div className="flex min-h-0 flex-1">
          <div className="flex w-44 shrink-0 flex-col overflow-y-auto border-r border-[var(--color-border)] bg-[var(--color-bg-muted)] py-2">
            <GroupItem label="全部" active={selectedGroup === ALL_GROUP} count={allProxies.length} onClick={() => setSelectedGroup(ALL_GROUP)} />
            {groups.map(group => (
              <GroupItem
                key={group}
                label={group}
                active={selectedGroup === group}
                count={allProxies.filter(proxy => proxy.groupName === group).length}
                onClick={() => setSelectedGroup(group)}
              />
            ))}
            {groups.length === 0 && <p className="px-3 py-2 text-xs text-[var(--color-text-muted)]">暂无分组</p>}
          </div>

          <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
            <div className="space-y-2 border-b border-[var(--color-border)] px-3 py-3">
              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--color-text-muted)]" />
                  <input
                    type="text"
                    value={search}
                    onChange={event => setSearch(event.target.value)}
                    placeholder="搜索代理名称、分组或配置"
                    className="w-full rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-input)] py-1.5 pl-8 pr-3 text-sm text-[var(--color-text-primary)] placeholder-[var(--color-text-muted)] focus:border-[var(--color-primary)] focus:outline-none"
                  />
                </div>
                <button
                  onClick={() => void testAll()}
                  disabled={testingIds.size > 0 || displayProxies.length === 0}
                  className="flex shrink-0 items-center gap-1.5 rounded-lg border border-[var(--color-border)] px-3 py-1.5 text-xs text-[var(--color-text-secondary)] transition-colors hover:border-[var(--color-primary)] hover:text-[var(--color-primary)] disabled:cursor-not-allowed disabled:opacity-40"
                >
                  <Wifi className="h-3.5 w-3.5" />
                  测试当前列表
                </button>
              </div>
              {bestAvailable && bestResult && (
                <div className="flex items-center gap-2 rounded-md border border-green-500/25 bg-green-500/5 px-3 py-2 text-xs">
                  <span className="font-medium text-green-700">推荐最快</span>
                  <span className="min-w-0 truncate text-[var(--color-text-primary)]">{bestAvailable.proxyName || bestAvailable.proxyId}</span>
                  <span className={latencyColor(bestResult)}>{formatLatency(bestResult)}</span>
                  <span className="text-[var(--color-text-muted)]">最近 {formatTestedAt(bestResult.testedAt)}</span>
                </div>
              )}
            </div>

            <div className="min-h-0 flex-1 overflow-y-auto">
              {loading ? (
                <div className="flex h-24 items-center justify-center text-sm text-[var(--color-text-muted)]">加载中...</div>
              ) : displayProxies.length === 0 ? (
                <div className="flex h-24 items-center justify-center text-sm text-[var(--color-text-muted)]">暂无代理</div>
              ) : (
                displayProxies.map(proxy => (
                  <ProxyRow
                    key={proxy.proxyId}
                    proxy={proxy}
                    selected={proxy.proxyId === currentProxyId}
                    testing={testingIds.has(proxy.proxyId)}
                    speedResult={getProxySpeed(proxy, speedMap)}
                    onSelect={() => { onSelect(resolveProxyWithSpeed(proxy)); onClose() }}
                    onTest={event => void testOne(proxy.proxyId, event)}
                  />
                ))
              )}
            </div>
          </div>
        </div>

        <div className="border-t border-[var(--color-border)] px-5 py-3 text-xs text-[var(--color-text-muted)]">
          共 {displayProxies.length} 条，点击行即可选中
        </div>
      </div>
    </div>,
    document.body,
  )
}

function GroupItem({ label, active, count, onClick }: { label: string; active: boolean; count: number; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className={`flex w-full items-center justify-between gap-2 px-3 py-2 text-left text-sm transition-colors ${
        active
          ? 'bg-[var(--color-primary)]/10 font-medium text-[var(--color-primary)]'
          : 'text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-hover)]'
      }`}
    >
      <span className="truncate">{label}</span>
      <span className="shrink-0 text-xs opacity-60">{count}</span>
    </button>
  )
}

interface ProxyRowProps {
  proxy: BrowserProxy
  selected: boolean
  testing: boolean
  speedResult?: SpeedResult
  onSelect: () => void
  onTest: (event: React.MouseEvent) => void
}

function ProxyRow({ proxy, selected, testing, speedResult, onSelect, onTest }: ProxyRowProps) {
  return (
    <div
      onClick={onSelect}
      className={`grid cursor-pointer grid-cols-[minmax(0,1fr)_120px_130px_40px_24px] items-center gap-3 border-b border-[var(--color-border)]/40 px-4 py-2.5 transition-colors last:border-0 ${
        selected ? 'bg-[var(--color-primary)]/10' : 'hover:bg-[var(--color-bg-hover)]'
      }`}
    >
      <div className="min-w-0">
        <div className="truncate text-sm font-medium text-[var(--color-text-primary)]">
          {proxy.proxyName || proxy.proxyId}
          {proxy.groupName && <span className="ml-2 text-xs font-normal text-[var(--color-primary)]/70">[{proxy.groupName}]</span>}
        </div>
        <div className="mt-0.5 truncate text-xs text-[var(--color-text-muted)]">
          {protocolLabel(proxy.proxyConfig)} · {proxy.proxyConfig}
        </div>
      </div>

      <div className={`text-right text-xs font-medium ${latencyColor(speedResult)}`}>
        {testing ? (
          <span className="inline-flex items-center justify-end gap-1 text-[var(--color-text-muted)]">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
            测试中
          </span>
        ) : (
          formatLatency(speedResult)
        )}
      </div>

      <div className="flex items-center justify-end gap-1 text-xs text-[var(--color-text-muted)]">
        <Clock3 className="h-3 w-3" />
        {formatTestedAt(speedResult?.testedAt)}
      </div>

      <button
        onClick={onTest}
        disabled={testing || proxy.proxyConfig === 'direct://'}
        title="重新测速"
        className="shrink-0 rounded p-1 text-[var(--color-text-muted)] transition-colors hover:bg-[var(--color-primary)]/10 hover:text-[var(--color-primary)] disabled:cursor-not-allowed disabled:opacity-40"
      >
        <Wifi className="h-3.5 w-3.5" />
      </button>

      {selected ? <Check className="h-4 w-4 shrink-0 text-[var(--color-primary)]" /> : <span />}
    </div>
  )
}
