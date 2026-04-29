import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  Activity,
  Camera,
  CheckCircle,
  Clock,
  ExternalLink,
  Globe,
  LayoutGrid,
  Layers,
  Monitor,
  MousePointer2,
  Navigation,
  PanelLeft,
  Play,
  RefreshCw,
  Search,
  Send,
  Square,
  XCircle,
} from 'lucide-react'
import { Badge, Button, Card, Input, Select, toast } from '../../shared/components'
import { SynchronizerActionFeed } from './components/SynchronizerActionFeed'
import { useSyncStore } from './store'
import {
  captureProfileScreenshot,
  activateProfileWindow,
  arrangeProfileWindows,
  listSyncGroups,
  listWorkbenchTasks,
  navigateProfile,
  refreshProfile,
  saveWorkbenchTasks,
} from './api'
import type { SyncGroup, SyncWindow, WorkbenchTask, WorkbenchTaskType } from './types'
import type { BrowserProfile } from '../browser/types'
import { fetchBrowserProfiles, startBrowserInstance, stopBrowserInstance } from '../browser/api'

const TASK_CONCURRENCY = 3
const TASK_HISTORY_LIMIT = 200
const PROFILE_ROW_HEIGHT = 194
const PROFILE_LIST_OVERSCAN = 6

function useDebouncedValue<T>(value: T, delayMs: number): T {
  const [debounced, setDebounced] = useState(value)

  useEffect(() => {
    const timer = window.setTimeout(() => setDebounced(value), delayMs)
    return () => window.clearTimeout(timer)
  }, [value, delayMs])

  return debounced
}

function normalizeError(error: unknown) {
  if (error instanceof Error) return error.message
  if (typeof error === 'string') return error
  return '操作失败'
}

function taskLabel(type: WorkbenchTaskType) {
  switch (type) {
    case 'start':
      return '启动'
    case 'stop':
      return '停止'
    case 'navigate':
      return '打开 URL'
    case 'refresh':
      return '刷新'
    case 'screenshot':
      return '截图'
    case 'activate':
      return '激活'
    default:
      return type
  }
}

function statusBadge(profile: BrowserProfile, runtime?: SyncWindow) {
  if (profile.running && profile.debugReady && runtime?.status === 'running') {
    return <Badge variant="success" dot>运行中</Badge>
  }
  if (profile.running) {
    return <Badge variant="info" dot>接管中</Badge>
  }
  return <Badge variant="warning" dot>已停止</Badge>
}

function taskStatusBadge(status: WorkbenchTask['status']) {
  if (status === 'success') return <Badge variant="success">成功</Badge>
  if (status === 'error') return <Badge variant="error">失败</Badge>
  if (status === 'running') return <Badge variant="info">执行中</Badge>
  return <Badge variant="default">等待中</Badge>
}

interface PreviewMap {
  [profileId: string]: {
    dataUrl: string
    updatedAt: string
  }
}

function useVirtualProfiles(items: BrowserProfile[]) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(640)

  useEffect(() => {
    const element = containerRef.current
    if (!element) return

    const updateHeight = () => setViewportHeight(element.clientHeight || 640)
    updateHeight()

    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', updateHeight)
      return () => window.removeEventListener('resize', updateHeight)
    }

    const observer = new ResizeObserver(updateHeight)
    observer.observe(element)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    const element = containerRef.current
    if (!element) return
    if (scrollTop > items.length * PROFILE_ROW_HEIGHT) {
      element.scrollTop = 0
      setScrollTop(0)
    }
  }, [items.length, scrollTop])

  const startIndex = Math.max(0, Math.floor(scrollTop / PROFILE_ROW_HEIGHT) - PROFILE_LIST_OVERSCAN)
  const visibleCount = Math.ceil(viewportHeight / PROFILE_ROW_HEIGHT) + PROFILE_LIST_OVERSCAN * 2
  const endIndex = Math.min(items.length, startIndex + visibleCount)
  const virtualItems = items.slice(startIndex, endIndex).map((profile, offset) => ({
    profile,
    index: startIndex + offset,
  }))

  return {
    containerRef,
    onScroll: () => setScrollTop(containerRef.current?.scrollTop || 0),
    totalHeight: items.length * PROFILE_ROW_HEIGHT,
    virtualItems,
  }
}

export function SynchronizerPage() {
  const { groups, activeGroupId, setGroups, setActiveGroup, addActionToFeed } = useSyncStore()

  const [profiles, setProfiles] = useState<BrowserProfile[]>([])
  const [loading, setLoading] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState('')
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'running' | 'stopped'>('all')
  const [groupFilter, setGroupFilter] = useState('all')
  const [targetUrl, setTargetUrl] = useState('')
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set())
  const [tasks, setTasks] = useState<WorkbenchTask[]>([])
  const [tasksLoaded, setTasksLoaded] = useState(false)
  const [taskRunning, setTaskRunning] = useState(false)
  const [previews, setPreviews] = useState<PreviewMap>({})
  const debouncedSearch = useDebouncedValue(search, 300)

  const runningById = useMemo(() => {
    const map = new Map<string, SyncWindow>()
    groups.forEach((group) => {
      group.windows.forEach((win) => map.set(win.profileId, win))
    })
    return map
  }, [groups])

  const refreshWorkspace = useCallback(async ({ silent = false }: { silent?: boolean } = {}) => {
    if (!silent) setLoading(true)
    setRefreshing(true)
    setError('')
    try {
      const [profileResult, groupResult] = await Promise.allSettled([
        fetchBrowserProfiles(),
        listSyncGroups(),
      ])

      if (profileResult.status === 'fulfilled') {
        setProfiles(profileResult.value || [])
      } else {
        throw profileResult.reason
      }

      if (groupResult.status === 'fulfilled') {
        const nextGroups = groupResult.value || []
        setGroups(nextGroups)
        if (nextGroups.length > 0 && (!activeGroupId || !nextGroups.some((g) => g.id === activeGroupId))) {
          setActiveGroup(nextGroups[0].id)
        }
      }
    } catch (err) {
      const message = normalizeError(err)
      setError(message.includes('Wails runtime not available') ? 'Wails 运行时不可用，请在应用中打开。' : message)
    } finally {
      if (!silent) setLoading(false)
      setRefreshing(false)
    }
  }, [activeGroupId, setActiveGroup, setGroups])

  useEffect(() => {
    void refreshWorkspace()
    const timer = window.setInterval(() => {
      if (document.visibilityState !== 'visible') return
      void refreshWorkspace({ silent: true })
    }, 5000)
    return () => window.clearInterval(timer)
  }, [refreshWorkspace])

  useEffect(() => {
    let cancelled = false
    listWorkbenchTasks(TASK_HISTORY_LIMIT)
      .then((savedTasks) => {
        if (!cancelled && Array.isArray(savedTasks)) {
          setTasks(savedTasks.slice(0, TASK_HISTORY_LIMIT))
        }
      })
      .catch(() => {
        // Local browser preview has no Wails runtime; task persistence is optional there.
      })
      .finally(() => {
        if (!cancelled) setTasksLoaded(true)
      })
    return () => {
      cancelled = true
    }
  }, [])

  useEffect(() => {
    if (!tasksLoaded) return
    const timer = window.setTimeout(() => {
      saveWorkbenchTasks(tasks.slice(0, TASK_HISTORY_LIMIT)).catch(() => {
        // Persistence failure should not block the operator queue.
      })
    }, 250)
    return () => window.clearTimeout(timer)
  }, [tasks, tasksLoaded])

  const groupOptions = useMemo(() => {
    const map = new Map<string, string>()
    profiles.forEach((profile) => {
      const id = profile.groupId || '__ungrouped__'
      map.set(id, id === '__ungrouped__' ? '未分组' : id)
    })
    groups.forEach((group) => map.set(group.id, group.name))
    return [
      { value: 'all', label: '全部分组' },
      ...Array.from(map.entries()).map(([value, label]) => ({ value, label })),
    ]
  }, [groups, profiles])

  const filteredProfiles = useMemo(() => {
    const keyword = debouncedSearch.trim().toLowerCase()
    return profiles.filter((profile) => {
      if (statusFilter === 'running' && !profile.running) return false
      if (statusFilter === 'stopped' && profile.running) return false
      if (groupFilter !== 'all') {
        const profileGroup = profile.groupId || '__ungrouped__'
        if (profileGroup !== groupFilter) return false
      }
      if (!keyword) return true
      const text = [
        profile.profileName,
        profile.profileId,
        profile.launchCode || '',
        profile.proxyId || '',
        profile.proxyBindName || '',
        ...(profile.tags || []),
        ...(profile.keywords || []),
      ].join(' ').toLowerCase()
      return text.includes(keyword)
    })
  }, [debouncedSearch, groupFilter, profiles, statusFilter])

  const visibleProfiles = filteredProfiles
  const virtualProfiles = useVirtualProfiles(visibleProfiles)
  const selectedProfiles = useMemo(
    () => profiles.filter((profile) => selectedIds.has(profile.profileId)),
    [profiles, selectedIds],
  )
  const runningCount = useMemo(() => profiles.filter((profile) => profile.running).length, [profiles])
  const pendingTaskCount = useMemo(
    () => tasks.filter((task) => task.status === 'pending' || task.status === 'running').length,
    [tasks],
  )

  const toggleSelect = (profileId: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(profileId)) {
        next.delete(profileId)
      } else {
        next.add(profileId)
      }
      return next
    })
  }

  const selectVisible = () => {
    setSelectedIds(new Set(visibleProfiles.map((profile) => profile.profileId)))
  }

  const clearSelection = () => setSelectedIds(new Set())

  const updateTask = (id: string, patch: Partial<WorkbenchTask>) => {
    setTasks((prev) => prev.map((task) => (
      task.id === id ? { ...task, ...patch, updatedAt: new Date().toISOString() } : task
    )))
  }

  const makeTasks = (type: WorkbenchTaskType, targets: BrowserProfile[], detail: string) => {
    const now = new Date().toISOString()
    return targets.map((profile) => ({
      id: crypto.randomUUID(),
      type,
      profileId: profile.profileId,
      profileName: profile.profileName,
      detail,
      status: 'pending' as const,
      createdAt: now,
      updatedAt: now,
    }))
  }

  const executeTask = async (task: WorkbenchTask) => {
    updateTask(task.id, { status: 'running', error: '' })
    try {
      switch (task.type) {
        case 'start':
          await startBrowserInstance(task.profileId)
          break
        case 'stop':
          await stopBrowserInstance(task.profileId)
          break
        case 'navigate':
          await navigateProfile(task.profileId, task.detail)
          break
        case 'refresh':
          await refreshProfile(task.profileId)
          break
        case 'screenshot': {
          const dataUrl = await captureProfileScreenshot(task.profileId)
          setPreviews((prev) => ({
            ...prev,
            [task.profileId]: {
              dataUrl,
              updatedAt: new Date().toISOString(),
            },
          }))
          break
        }
        case 'activate':
          await activateProfileWindow(task.profileId)
          break
        default:
          throw new Error(`未知任务: ${task.type}`)
      }

      updateTask(task.id, { status: 'success' })
      addActionToFeed({
        id: task.id,
        operation: task.type,
        windowName: task.profileName,
        detail: task.detail || taskLabel(task.type),
        timestamp: new Date().toISOString(),
        status: 'ok',
      })
    } catch (err) {
      const message = normalizeError(err)
      updateTask(task.id, { status: 'error', error: message })
      addActionToFeed({
        id: task.id,
        operation: task.type,
        windowName: task.profileName,
        detail: message,
        timestamp: new Date().toISOString(),
        status: 'error',
      })
    }
  }

  const enqueueTasks = async (type: WorkbenchTaskType, targets: BrowserProfile[], detail = '') => {
    if (taskRunning) {
      toast.warning('任务队列正在执行')
      return
    }
    if (targets.length === 0) {
      toast.warning('请选择至少一个实例')
      return
    }
    if (type === 'navigate' && !detail.trim()) {
      toast.warning('请输入目标 URL')
      return
    }

    const nextTasks = makeTasks(type, targets, detail.trim())
    setTasks((prev) => [...nextTasks, ...prev].slice(0, TASK_HISTORY_LIMIT))
    setTaskRunning(true)

    const queue = [...nextTasks]
    const workerCount = Math.min(TASK_CONCURRENCY, queue.length)
    const workers = Array.from({ length: workerCount }, async () => {
      while (queue.length > 0) {
        const task = queue.shift()
        if (task) {
          await executeTask(task)
        }
      }
    })

    await Promise.all(workers)
    setTaskRunning(false)
    void refreshWorkspace({ silent: true })
  }

  const selectedLiveProfiles = selectedProfiles.filter((profile) => {
    const runtime = runningById.get(profile.profileId)
    return profile.running && Boolean(runtime?.pid || profile.pid)
  })
  const selectedRunningProfiles = selectedProfiles.filter((profile) => profile.running && profile.debugReady)
  const allVisibleRunning = visibleProfiles.filter((profile) => profile.running && profile.debugReady)

  const activeGroup = groups.find((group) => group.id === activeGroupId) || groups[0]

  const handleArrange = async (layout: 'grid' | 'main-left') => {
    if (taskRunning) {
      toast.warning('任务队列正在执行')
      return
    }
    if (selectedLiveProfiles.length === 0) {
      toast.warning('请选择至少一个运行中的实例')
      return
    }

    setTaskRunning(true)
    try {
      const placements = await arrangeProfileWindows(
        selectedLiveProfiles.map((profile) => profile.profileId),
        layout,
      )
      const successCount = placements.filter((item) => !item.error && item.found).length
      const failedCount = placements.length - successCount
      addActionToFeed({
        id: crypto.randomUUID(),
        operation: 'arrange',
        windowName: '选中窗口',
        detail: `${layout === 'grid' ? '平铺' : '主辅'} ${successCount}/${placements.length}`,
        timestamp: new Date().toISOString(),
        status: successCount > 0 ? 'ok' : 'error',
      })
      if (failedCount > 0) {
        toast.warning(`已排列 ${successCount} 个，${failedCount} 个未找到窗口`)
      } else {
        toast.success('窗口排列完成')
      }
    } catch (err) {
      const message = normalizeError(err)
      addActionToFeed({
        id: crypto.randomUUID(),
        operation: 'arrange',
        windowName: '选中窗口',
        detail: message,
        timestamp: new Date().toISOString(),
        status: 'error',
      })
      toast.error(message)
    } finally {
      setTaskRunning(false)
      void refreshWorkspace({ silent: true })
    }
  }

  return (
    <div className="space-y-4 animate-fade-in">
      <div className="grid grid-cols-1 xl:grid-cols-[minmax(0,1fr)_360px] gap-4">
        <div className="space-y-4 min-w-0">
          <Card>
            <div className="flex flex-col gap-4">
              <div className="flex items-start justify-between gap-4">
                <div>
                  <div className="inline-flex items-center gap-2 px-2.5 py-1 rounded-full bg-[var(--color-accent-muted)] text-[var(--color-accent)] text-xs font-medium mb-3">
                    <Monitor className="w-3.5 h-3.5" /> Instance Workbench
                  </div>
                  <h1 className="text-xl font-semibold text-[var(--color-text-primary)]">
                    实例工作台
                  </h1>
                  <div className="flex flex-wrap items-center gap-2 mt-3">
                    <Badge variant="default">总数 {profiles.length}</Badge>
                    <Badge variant="success">运行 {runningCount}</Badge>
                    <Badge variant="warning">停止 {profiles.length - runningCount}</Badge>
                    <Badge variant={pendingTaskCount > 0 ? 'info' : 'default'}>队列 {pendingTaskCount}</Badge>
                  </div>
                </div>
                <Button size="sm" variant="secondary" onClick={() => refreshWorkspace()} loading={refreshing}>
                  <RefreshCw className="w-3.5 h-3.5" /> 刷新
                </Button>
              </div>

              <div className="grid grid-cols-1 lg:grid-cols-[minmax(180px,1fr)_160px_160px] gap-2">
                <div className="relative">
                  <Search className="w-4 h-4 absolute left-3 top-2.5 text-[var(--color-text-muted)]" />
                  <Input
                    value={search}
                    onChange={(event) => setSearch(event.target.value)}
                    placeholder="搜索实例、标签、Code、代理"
                    className="pl-9"
                  />
                </div>
                <Select
                  value={statusFilter}
                  onChange={(event) => setStatusFilter(event.target.value as typeof statusFilter)}
                  options={[
                    { value: 'all', label: '全部状态' },
                    { value: 'running', label: '仅运行中' },
                    { value: 'stopped', label: '仅已停止' },
                  ]}
                />
                <Select
                  value={groupFilter}
                  onChange={(event) => setGroupFilter(event.target.value)}
                  options={groupOptions}
                />
              </div>

              <div className="flex flex-col lg:flex-row gap-2">
                <Input
                  value={targetUrl}
                  onChange={(event) => setTargetUrl(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') {
                      void enqueueTasks('navigate', selectedRunningProfiles, targetUrl)
                    }
                  }}
                  placeholder="https://example.com"
                  className="flex-1"
                />
                <div className="flex flex-wrap gap-2">
                  <Button size="sm" onClick={() => enqueueTasks('navigate', selectedRunningProfiles, targetUrl)} disabled={selectedRunningProfiles.length === 0 || taskRunning}>
                    <Send className="w-3.5 h-3.5" /> 打开
                  </Button>
                  <Button size="sm" variant="secondary" onClick={() => enqueueTasks('refresh', selectedRunningProfiles)} disabled={selectedRunningProfiles.length === 0 || taskRunning}>
                    <RefreshCw className="w-3.5 h-3.5" /> 刷新
                  </Button>
                  <Button size="sm" variant="secondary" onClick={() => enqueueTasks('screenshot', selectedRunningProfiles)} disabled={selectedRunningProfiles.length === 0 || taskRunning}>
                    <Camera className="w-3.5 h-3.5" /> 截图
                  </Button>
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <Button size="sm" variant="ghost" onClick={selectVisible} disabled={visibleProfiles.length === 0}>
                  全选当前
                </Button>
                <Button size="sm" variant="ghost" onClick={clearSelection} disabled={selectedIds.size === 0}>
                  清空选择
                </Button>
                <span className="text-xs text-[var(--color-text-muted)]">
                  已选 {selectedIds.size} 个
                </span>
                <span className="w-px h-4 bg-[var(--color-border-muted)] mx-1" />
                <Button size="sm" onClick={() => enqueueTasks('start', selectedProfiles)} disabled={selectedProfiles.length === 0 || taskRunning}>
                  <Play className="w-3.5 h-3.5" /> 启动
                </Button>
                <Button size="sm" variant="secondary" onClick={() => enqueueTasks('stop', selectedProfiles)} disabled={selectedProfiles.length === 0 || taskRunning}>
                  <Square className="w-3.5 h-3.5" /> 停止
                </Button>
                <Button size="sm" variant="secondary" onClick={() => enqueueTasks('activate', selectedLiveProfiles.slice(0, 1))} disabled={selectedLiveProfiles.length === 0 || taskRunning}>
                  <MousePointer2 className="w-3.5 h-3.5" /> 激活首个
                </Button>
                <Button size="sm" variant="ghost" onClick={() => handleArrange('grid')} disabled={selectedLiveProfiles.length === 0 || taskRunning}>
                  <LayoutGrid className="w-3.5 h-3.5" /> 平铺
                </Button>
                <Button size="sm" variant="ghost" onClick={() => handleArrange('main-left')} disabled={selectedLiveProfiles.length === 0 || taskRunning}>
                  <PanelLeft className="w-3.5 h-3.5" /> 主辅
                </Button>
                <Button size="sm" variant="secondary" onClick={() => enqueueTasks('screenshot', allVisibleRunning)} disabled={allVisibleRunning.length === 0 || taskRunning}>
                  <Camera className="w-3.5 h-3.5" /> 刷新运行预览
                </Button>
              </div>
            </div>
          </Card>

          {error && (
            <Card>
              <div className="flex items-center gap-3 text-sm text-[var(--color-error)]">
                <XCircle className="w-4 h-4" />
                {error}
              </div>
            </Card>
          )}

          {groups.length > 0 && (
            <div className="flex items-center gap-2 overflow-x-auto pb-1">
              {groups.map((group) => (
                <button
                  key={group.id}
                  onClick={() => {
                    setActiveGroup(group.id)
                    setGroupFilter(group.id)
                  }}
                  className={[
                    'flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm font-medium whitespace-nowrap transition-colors',
                    activeGroupId === group.id
                      ? 'bg-[var(--color-accent)] text-white shadow-sm'
                      : 'bg-[var(--color-bg-secondary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-accent-muted)] hover:text-[var(--color-text-primary)]',
                  ].join(' ')}
                >
                  <Layers className="w-3.5 h-3.5" />
                  {group.name}
                  <span className="ml-1 rounded-full bg-white/20 px-1.5 py-0.5 text-[10px]">
                    {group.windows.length}
                  </span>
                </button>
              ))}
            </div>
          )}

          {loading ? (
            <Card>
              <div className="py-16 text-center text-sm text-[var(--color-text-muted)]">
                加载中...
              </div>
            </Card>
          ) : visibleProfiles.length === 0 ? (
            <Card>
              <div className="py-16 text-center text-sm text-[var(--color-text-muted)]">
                没有匹配的实例
              </div>
            </Card>
          ) : (
            <div
              ref={virtualProfiles.containerRef}
              onScroll={virtualProfiles.onScroll}
              className="relative max-h-[calc(100vh-340px)] min-h-[420px] overflow-y-auto pr-1"
            >
              <div className="relative" style={{ height: virtualProfiles.totalHeight }}>
                {virtualProfiles.virtualItems.map(({ profile, index }) => {
                const runtime = runningById.get(profile.profileId)
                const preview = previews[profile.profileId]
                const selected = selectedIds.has(profile.profileId)

                return (
                  <div
                    key={profile.profileId}
                    className="absolute left-0 right-0"
                    style={{ transform: `translateY(${index * PROFILE_ROW_HEIGHT}px)` }}
                  >
                    <Card padding="none" className={selected ? 'border-[var(--color-accent)]' : undefined}>
                    <div className="grid grid-cols-[168px_minmax(0,1fr)] min-h-[178px]">
                      <button
                        type="button"
                        className="relative bg-[var(--color-bg-base)] border-r border-[var(--color-border-muted)] overflow-hidden text-left"
                        onClick={() => profile.running && profile.debugReady ? enqueueTasks('screenshot', [profile]) : toggleSelect(profile.profileId)}
                      >
                        {preview ? (
                          <img src={preview.dataUrl} alt="" className="w-full h-full object-cover" />
                        ) : (
                          <div className="h-full flex flex-col items-center justify-center text-[var(--color-text-muted)] gap-2">
                            <Monitor className="w-8 h-8 opacity-40" />
                            <span className="text-xs">暂无预览</span>
                          </div>
                        )}
                        <div className="absolute left-2 top-2">
                          {statusBadge(profile, runtime)}
                        </div>
                      </button>

                      <div className="p-4 min-w-0 flex flex-col gap-3">
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0">
                            <div className="flex items-center gap-2 min-w-0">
                              <input
                                type="checkbox"
                                checked={selected}
                                onChange={() => toggleSelect(profile.profileId)}
                                className="w-4 h-4 accent-[var(--color-accent)] shrink-0"
                              />
                              <Link to={`/browser/detail/${profile.profileId}`} className="font-medium text-sm text-[var(--color-text-primary)] hover:text-[var(--color-accent)] truncate">
                                {profile.profileName}
                              </Link>
                            </div>
                            <div className="flex flex-wrap gap-1 mt-2">
                              {(profile.tags || []).slice(0, 4).map((tag) => (
                                <Badge key={tag} size="sm">{tag}</Badge>
                              ))}
                            </div>
                          </div>
                          <Link to={`/browser/edit/${profile.profileId}`}>
                            <Button size="sm" variant="ghost">
                              <ExternalLink className="w-3.5 h-3.5" /> 配置
                            </Button>
                          </Link>
                        </div>

                        <div className="space-y-1.5 min-w-0">
                          <div className="flex items-center gap-2 text-xs text-[var(--color-text-muted)] min-w-0">
                            <Globe className="w-3.5 h-3.5 shrink-0" />
                            <span className="truncate">{runtime?.title || runtime?.url || '未读取页面状态'}</span>
                          </div>
                          <div className="flex items-center gap-2 text-xs text-[var(--color-text-muted)] min-w-0">
                            <Activity className="w-3.5 h-3.5 shrink-0" />
                            <span className="truncate">
                              PID {runtime?.pid || profile.pid || '-'} · CDP {runtime?.debugPort || profile.debugPort || '-'} · {profile.proxyBindName || profile.proxyId || '未绑定代理'}
                            </span>
                          </div>
                          <div className="flex items-center gap-2 text-xs text-[var(--color-text-muted)] min-w-0">
                            <Clock className="w-3.5 h-3.5 shrink-0" />
                            <span className="truncate">
                              {preview ? `预览 ${new Date(preview.updatedAt).toLocaleTimeString()}` : `更新 ${new Date(profile.updatedAt).toLocaleString()}`}
                            </span>
                          </div>
                        </div>

                        <div className="flex flex-wrap gap-2 mt-auto">
                          <Button size="sm" variant="ghost" onClick={() => enqueueTasks('activate', [profile])} disabled={!profile.running || !(runtime?.pid || profile.pid) || taskRunning}>
                            <MousePointer2 className="w-3.5 h-3.5" /> 激活
                          </Button>
                          {profile.running ? (
                            <Button size="sm" variant="secondary" onClick={() => enqueueTasks('stop', [profile])} disabled={taskRunning}>
                              <Square className="w-3.5 h-3.5" /> 停止
                            </Button>
                          ) : (
                            <Button size="sm" onClick={() => enqueueTasks('start', [profile])} disabled={taskRunning}>
                              <Play className="w-3.5 h-3.5" /> 启动
                            </Button>
                          )}
                          <Button size="sm" variant="secondary" onClick={() => enqueueTasks('navigate', [profile], targetUrl)} disabled={!profile.running || !profile.debugReady || taskRunning}>
                            <Navigation className="w-3.5 h-3.5" /> 打开
                          </Button>
                          <Button size="sm" variant="ghost" onClick={() => enqueueTasks('refresh', [profile])} disabled={!profile.running || !profile.debugReady || taskRunning}>
                            <RefreshCw className="w-3.5 h-3.5" /> 刷新
                          </Button>
                          <Button size="sm" variant="ghost" onClick={() => enqueueTasks('screenshot', [profile])} disabled={!profile.running || !profile.debugReady || taskRunning}>
                            <Camera className="w-3.5 h-3.5" /> 预览
                          </Button>
                        </div>
                      </div>
                    </div>
                  </Card>
                  </div>
                )
                })}
              </div>
            </div>
          )}
        </div>

        <div className="space-y-4 min-w-0">
          <Card title="任务队列" subtitle={taskRunning ? '执行中' : '空闲'}>
            {tasks.length === 0 ? (
              <div className="py-10 text-center text-xs text-[var(--color-text-muted)]">
                暂无任务
              </div>
            ) : (
              <div className="space-y-2 max-h-[360px] overflow-y-auto pr-1">
                {tasks.slice(0, 80).map((task) => (
                  <div key={task.id} className="rounded-lg border border-[var(--color-border-muted)] px-3 py-2">
                    <div className="flex items-center justify-between gap-2">
                      <div className="min-w-0">
                        <p className="text-xs font-medium text-[var(--color-text-primary)] truncate">
                          {task.profileName}
                        </p>
                        <p className="text-[11px] text-[var(--color-text-muted)] truncate">
                          {taskLabel(task.type)} · {task.detail || '-'}
                        </p>
                      </div>
                      {taskStatusBadge(task.status)}
                    </div>
                    {task.error && (
                      <p className="text-[11px] text-[var(--color-error)] mt-1 truncate">
                        {task.error}
                      </p>
                    )}
                  </div>
                ))}
              </div>
            )}
          </Card>

          <Card title="操作记录" subtitle={activeGroup ? activeGroup.name : '全部'}>
            <SynchronizerActionFeed />
          </Card>

          <Card title="运行分组">
            {groups.length === 0 ? (
              <div className="py-8 text-center text-xs text-[var(--color-text-muted)]">
                暂无运行中实例
              </div>
            ) : (
              <div className="space-y-2">
                {groups.map((group: SyncGroup) => (
                  <button
                    key={group.id}
                    className="w-full flex items-center justify-between rounded-lg px-3 py-2 bg-[var(--color-bg-secondary)] hover:bg-[var(--color-accent-muted)] text-left"
                    onClick={() => {
                      setActiveGroup(group.id)
                      setGroupFilter(group.id)
                    }}
                  >
                    <span className="text-sm text-[var(--color-text-primary)] truncate">{group.name}</span>
                    <Badge variant="info">{group.windows.length}</Badge>
                  </button>
                ))}
              </div>
            )}
          </Card>

          <Card>
            <div className="grid grid-cols-2 gap-3 text-center">
              <div>
                <CheckCircle className="w-5 h-5 mx-auto text-[var(--color-success)] mb-1" />
                <p className="text-xs text-[var(--color-text-muted)]">真实窗口</p>
              </div>
              <div>
                <XCircle className="w-5 h-5 mx-auto text-[var(--color-warning)] mb-1" />
                <p className="text-xs text-[var(--color-text-muted)]">不做内嵌</p>
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>
  )
}
