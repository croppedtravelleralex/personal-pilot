import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router-dom'
import {
  Activity,
  CheckCircle,
  Clock,
  ExternalLink,
  Fingerprint,
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
  ShieldCheck,
  Square,
  XCircle,
} from 'lucide-react'
import { Badge, Button, Card, Input, Select, toast } from '../../shared/components'
import { SynchronizerActionFeed } from './components/SynchronizerActionFeed'
import { useSyncStore } from './store'
import {
  activateProfileWindow,
  arrangeProfileWindows,
  checkWorkbenchIdentityReport,
  checkWorkbenchFingerprintHealthProfile,
  getBrowserInstanceStatus,
  getWorkbenchUiState,
  listWorkbenchDetectionResults,
  listWorkbenchDetectorSites,
  listSyncGroups,
  listWorkbenchTasks,
  navigateProfile,
  onBrowserInstanceLifecycle,
  refreshProfile,
  runWorkbenchDetectorSite,
  saveWorkbenchDetectionResult,
  saveWorkbenchUiState,
  saveWorkbenchTasks,
} from './api'
import type {
  WorkbenchDetectionKind,
  WorkbenchDetectionResult,
  WorkbenchDetectorSite,
  SyncGroup,
  SyncWindow,
  WorkbenchFingerprintHealthProfile,
  WorkbenchIdentityStrengthReport,
  WorkbenchTask,
  WorkbenchTaskType,
  WorkbenchUiState,
} from './types'
import type { BrowserProfile } from '../browser/types'
import { fetchBrowserProfiles, startBrowserInstance, stopBrowserInstance } from '../browser/api'

const TASK_CONCURRENCY = 3
const TASK_HISTORY_LIMIT = 200
const PROFILE_ROW_HEIGHT = 236
const PROFILE_LIST_OVERSCAN = 6
const INSTANCE_EVENT_REFRESH_DEBOUNCE_MS = 300
const START_STATUS_POLL_INTERVAL_MS = 350
const START_STATUS_POLL_TIMEOUT_MS = 8000

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

function taskLabel(type: WorkbenchTask['type']) {
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
      return '已禁用功能'
    case 'activate':
      return '激活'
    case 'fingerprint-health':
      return '指纹体检'
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

type FingerprintHealthMap = Record<string, WorkbenchFingerprintHealthProfile>
type FingerprintHealthUiLevel = WorkbenchFingerprintHealthProfile['level'] | 'coherent' | 'suspicious' | 'inconsistent'
type FingerprintHealthCheck = WorkbenchFingerprintHealthProfile['checks'][number]
type FingerprintHealthCheckStatus = 'passed' | 'warning' | 'failed' | 'unknown'
type IdentityReportMap = Record<string, WorkbenchIdentityStrengthReport>

const FINGERPRINT_HEALTH_LEVEL_UI: Record<
  'good' | 'warning' | 'risk' | 'unknown',
  { label: string; variant: 'default' | 'success' | 'error' | 'warning' | 'info' }
> = {
  good: { label: '良好', variant: 'success' },
  warning: { label: '预警', variant: 'warning' },
  risk: { label: '风险', variant: 'error' },
  unknown: { label: '未知', variant: 'default' },
}

const IDENTITY_LEVEL_UI: Record<
  WorkbenchIdentityStrengthReport['level'],
  { label: string; variant: 'default' | 'success' | 'error' | 'warning' | 'info' }
> = {
  strong: { label: '强身份', variant: 'success' },
  normal: { label: '稳定', variant: 'info' },
  weak: { label: '偏弱', variant: 'warning' },
  risk: { label: '风险', variant: 'error' },
  unknown: { label: '未知', variant: 'default' },
}

function sleep(ms: number) {
  return new Promise<void>((resolve) => window.setTimeout(resolve, ms))
}

function readRecordValue(source: unknown, key: string): unknown {
  if (!source || typeof source !== 'object') return undefined
  return (source as Record<string, unknown>)[key]
}

function readStringField(source: unknown, keys: string[], fallback = '') {
  for (const key of keys) {
    const value = readRecordValue(source, key)
    if (typeof value === 'string' && value.trim()) return value
  }
  return fallback
}

function normalizeFingerprintHealthLevel(level: FingerprintHealthUiLevel): keyof typeof FINGERPRINT_HEALTH_LEVEL_UI {
  switch (level) {
    case 'good':
    case 'coherent':
      return 'good'
    case 'warning':
    case 'suspicious':
      return 'warning'
    case 'risk':
    case 'inconsistent':
      return 'risk'
    default:
      return 'unknown'
  }
}

function fingerprintHealthLevelLabel(level: FingerprintHealthUiLevel) {
  return FINGERPRINT_HEALTH_LEVEL_UI[normalizeFingerprintHealthLevel(level)].label
}

function fingerprintHealthBadge(health: WorkbenchFingerprintHealthProfile) {
  const ui = FINGERPRINT_HEALTH_LEVEL_UI[normalizeFingerprintHealthLevel(health.level)]
  return (
    <Badge variant={ui.variant} dot>
      {ui.label} {health.score}
    </Badge>
  )
}

function identityReportBadge(report: WorkbenchIdentityStrengthReport) {
  const ui = IDENTITY_LEVEL_UI[report.level] || IDENTITY_LEVEL_UI.unknown
  return (
    <Badge variant={ui.variant} dot>
      {ui.label} {report.score}
    </Badge>
  )
}

function getIdentityReportTime(report: WorkbenchIdentityStrengthReport) {
  return readStringField(report, ['capturedAt', 'updatedAt', 'checkedAt'])
}

function getIdentityReportTimestamp(report: WorkbenchIdentityStrengthReport) {
  const parsed = Date.parse(getIdentityReportTime(report))
  return Number.isFinite(parsed) ? parsed : 0
}

function getIdentityReportSummary(report: WorkbenchIdentityStrengthReport) {
  if (Array.isArray(report.summary) && report.summary.length > 0) return report.summary.slice(0, 2).join('；')
  const abnormal = (report.dimensions || []).filter((dim) => dim.status === 'fail' || dim.status === 'warning')
  if (abnormal.length === 0) return `全部 ${report.dimensions?.length || 0} 项通过`
  return abnormal.slice(0, 2).map((dim) => `${dim.id}：${dim.message}`).join('；')
}

function getFingerprintHealthTime(health: WorkbenchFingerprintHealthProfile) {
  return readStringField(health, ['capturedAt', 'updatedAt', 'checkedAt'])
}

function getFingerprintHealthTimestamp(health: WorkbenchFingerprintHealthProfile) {
  const parsed = Date.parse(getFingerprintHealthTime(health))
  return Number.isFinite(parsed) ? parsed : 0
}

function getFingerprintHealthSource(health: WorkbenchFingerprintHealthProfile) {
  return readStringField(health, ['source'])
}

function getFingerprintCheckStatus(check: FingerprintHealthCheck): FingerprintHealthCheckStatus {
  const status = readStringField(check, ['status']).toLowerCase()
  if (['passed', 'pass', 'ok', 'good', 'success'].includes(status)) return 'passed'
  if (['warning', 'warn'].includes(status)) return 'warning'
  if (['failed', 'fail', 'risk', 'error'].includes(status)) return 'failed'

  const passed = readRecordValue(check, 'passed')
  if (typeof passed === 'boolean') return passed ? 'passed' : 'failed'
  return 'unknown'
}

function getFingerprintCheckName(check: FingerprintHealthCheck) {
  return readStringField(check, ['id', 'dimension', 'name', 'key'], '检查项')
}

function getFingerprintCheckMessage(check: FingerprintHealthCheck) {
  return readStringField(check, ['message', 'detail', 'summary'], '-')
}

function getFingerprintHealthSummary(health: WorkbenchFingerprintHealthProfile) {
  const checks = health.checks || []
  if (checks.length === 0) return '暂无检查项'
  const abnormal = checks.filter((check) => {
    const status = getFingerprintCheckStatus(check)
    return status === 'failed' || status === 'warning'
  })
  if (abnormal.length === 0) return `全部 ${checks.length} 项通过`
  return abnormal.slice(0, 2).map((check) => (
    `${getFingerprintCheckName(check)}：${getFingerprintCheckMessage(check)}`
  )).join('；')
}

function latestDetectionTime(health?: WorkbenchFingerprintHealthProfile, report?: WorkbenchIdentityStrengthReport) {
  return Math.max(
    health ? getFingerprintHealthTimestamp(health) : 0,
    report ? getIdentityReportTimestamp(report) : 0,
  )
}

function proxyQualityLabel(report?: WorkbenchIdentityStrengthReport) {
  if (!report) return { label: '代理待校验', variant: 'default' as const }
  const proxyScore = report.subscores?.proxyNetwork ?? 0
  const consistencyScore = report.subscores?.consistency ?? 0
  if (proxyScore >= 90 && consistencyScore >= 90) return { label: '严格代理', variant: 'success' as const }
  if (proxyScore >= 70) return { label: '中等代理', variant: 'info' as const }
  return { label: '代理风险', variant: 'warning' as const }
}

function detectionFromFingerprintHealth(health: WorkbenchFingerprintHealthProfile): WorkbenchDetectionResult {
  return {
    id: '',
    profileId: health.profileId,
    profileName: health.profileName || '',
    kind: 'fingerprint_health',
    score: health.score,
    level: health.level,
    source: health.source || 'local-cdp',
    summary: [getFingerprintHealthSummary(health)],
    payload: health as unknown as Record<string, unknown>,
    createdAt: health.capturedAt || new Date().toISOString(),
  }
}

function detectionFromIdentityReport(report: WorkbenchIdentityStrengthReport): WorkbenchDetectionResult {
  return {
    id: '',
    profileId: report.profileId,
    profileName: report.profileName || '',
    kind: 'identity_report',
    score: report.score,
    level: report.level,
    source: report.source || 'local-cdp',
    summary: report.summary?.length ? report.summary : [getIdentityReportSummary(report)],
    payload: report as unknown as Record<string, unknown>,
    createdAt: report.capturedAt || new Date().toISOString(),
  }
}

function healthFromDetection(result: WorkbenchDetectionResult): WorkbenchFingerprintHealthProfile | null {
  if (result.kind !== 'fingerprint_health') return null
  const payload = result.payload as Partial<WorkbenchFingerprintHealthProfile>
  if (!payload || !result.profileId) return null
  return {
    profileId: payload.profileId || result.profileId,
    profileName: payload.profileName || result.profileName,
    score: typeof payload.score === 'number' ? payload.score : result.score,
    level: (payload.level as WorkbenchFingerprintHealthProfile['level']) || 'unknown',
    checks: Array.isArray(payload.checks) ? payload.checks : [],
    fingerprint: payload.fingerprint,
    capturedAt: payload.capturedAt || result.createdAt,
    source: payload.source || result.source,
    error: payload.error,
  }
}

function identityFromDetection(result: WorkbenchDetectionResult): WorkbenchIdentityStrengthReport | null {
  if (result.kind !== 'identity_report') return null
  const payload = result.payload as Partial<WorkbenchIdentityStrengthReport>
  if (!payload || !result.profileId) return null
  return {
    profileId: payload.profileId || result.profileId,
    profileName: payload.profileName || result.profileName,
    score: typeof payload.score === 'number' ? payload.score : result.score,
    level: (payload.level as WorkbenchIdentityStrengthReport['level']) || 'unknown',
    subscores: payload.subscores || {
      fingerprintVisible: 0,
      consistency: 0,
      profilePersistence: 0,
      proxyNetwork: 0,
      behaviorNaturalness: 0,
      automationSafety: 0,
    },
    dimensions: Array.isArray(payload.dimensions) ? payload.dimensions : [],
    fingerprint: payload.fingerprint,
    capturedAt: payload.capturedAt || result.createdAt,
    source: payload.source || result.source,
    summary: Array.isArray(payload.summary) ? payload.summary : result.summary,
  }
}

function useVirtualProfiles(items: BrowserProfile[]) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [scrollTop, setScrollTop] = useState(0)
  const [viewportHeight, setViewportHeight] = useState(640)
  const setScrollPosition = useCallback((nextScrollTop: number) => {
    const value = Math.max(0, nextScrollTop)
    setScrollTop(value)
    if (containerRef.current) {
      containerRef.current.scrollTop = value
    }
  }, [])

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
    scrollTop,
    setScrollPosition,
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
  const [fingerprintHealthById, setFingerprintHealthById] = useState<FingerprintHealthMap>({})
  const [fingerprintHealthCheckingIds, setFingerprintHealthCheckingIds] = useState<Set<string>>(new Set())
  const [identityReportById, setIdentityReportById] = useState<IdentityReportMap>({})
  const [identityReportCheckingIds, setIdentityReportCheckingIds] = useState<Set<string>>(new Set())
  const [detectorSites, setDetectorSites] = useState<WorkbenchDetectorSite[]>([])
  const [detectorResults, setDetectorResults] = useState<WorkbenchDetectionResult[]>([])
  const [detectorRunningIds, setDetectorRunningIds] = useState<Set<string>>(new Set())
  const [thirdPartyEnabled, setThirdPartyEnabled] = useState(false)
  const [selectedReportKind, setSelectedReportKind] = useState<WorkbenchDetectionKind | ''>('')
  const [selectedReportProfileId, setSelectedReportProfileId] = useState('')
  const [selectedReportId, setSelectedReportId] = useState('')
  const [expandedItems, setExpandedItems] = useState<Set<string>>(new Set())
  const [uiStateLoaded, setUiStateLoaded] = useState(false)
  const activeGroupIdRef = useRef(activeGroupId)
  const uiStateLoadedRef = useRef(false)
  const latestUiStateRef = useRef<WorkbenchUiState | null>(null)
  const debouncedSearch = useDebouncedValue(search, 300)

  useEffect(() => {
    activeGroupIdRef.current = activeGroupId
  }, [activeGroupId])

  useEffect(() => {
    uiStateLoadedRef.current = uiStateLoaded
  }, [uiStateLoaded])

  useEffect(() => {
    let cancelled = false
    Promise.allSettled([
      getWorkbenchUiState(),
      listWorkbenchDetectionResults('', '', 200),
      listWorkbenchDetectorSites(),
    ]).then(([stateResult, detectionResult, detectorResult]) => {
      if (cancelled) return
      if (stateResult.status === 'fulfilled') {
        const state = stateResult.value
        setSearch(state.search || '')
        setStatusFilter(state.statusFilter || 'all')
        setGroupFilter(state.groupFilter || 'all')
        setTargetUrl(state.targetUrl || '')
        setSelectedIds(new Set(state.selectedIds || []))
        setSelectedReportKind(state.selectedReportKind || '')
        setSelectedReportProfileId(state.selectedReportProfileId || '')
        setSelectedReportId(state.selectedReportId || '')
        setExpandedItems(new Set(state.expandedItems || []))
        setThirdPartyEnabled(Boolean(state.thirdPartyEnabled))
        if (state.activeGroupId) {
          setActiveGroup(state.activeGroupId)
        }
        window.setTimeout(() => {
          if (!cancelled) virtualProfiles.setScrollPosition(state.scrollTop || 0)
        }, 0)
      }
      if (detectionResult.status === 'fulfilled') {
        const nextHealth: FingerprintHealthMap = {}
        const nextIdentity: IdentityReportMap = {}
        const nextDetectorRuns: WorkbenchDetectionResult[] = []
        detectionResult.value.forEach((result) => {
          const health = healthFromDetection(result)
          if (health && !nextHealth[health.profileId]) nextHealth[health.profileId] = health
          const identity = identityFromDetection(result)
          if (identity && !nextIdentity[identity.profileId]) nextIdentity[identity.profileId] = identity
          if (result.kind === 'detector_site_run') nextDetectorRuns.push(result)
        })
        setFingerprintHealthById(nextHealth)
        setIdentityReportById(nextIdentity)
        setDetectorResults(nextDetectorRuns)
      }
      if (detectorResult.status === 'fulfilled') {
        setDetectorSites(detectorResult.value)
      }
    }).finally(() => {
      if (!cancelled) setUiStateLoaded(true)
    })
    return () => {
      cancelled = true
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const runningById = useMemo(() => {
    const map = new Map<string, SyncWindow>()
    groups.forEach((group) => {
      group.windows.forEach((win) => map.set(win.profileId, win))
    })
    return map
  }, [groups])

  const refreshWorkspace = useCallback(async ({ silent = false }: { silent?: boolean } = {}) => {
    if (!silent) {
      setLoading(true)
      setRefreshing(true)
      setError('')
    }
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
        const currentActiveGroupId = activeGroupIdRef.current
        setGroups(nextGroups)
        if (nextGroups.length > 0 && (!currentActiveGroupId || !nextGroups.some((g) => g.id === currentActiveGroupId))) {
          setActiveGroup(nextGroups[0].id)
        }
      }
    } catch (err) {
      const message = normalizeError(err)
      if (!silent) {
        setError(message.includes('Wails runtime not available') ? 'Wails 运行时不可用，请在应用中打开。' : message)
      }
    } finally {
      if (!silent) {
        setLoading(false)
        setRefreshing(false)
      }
    }
  }, [setActiveGroup, setGroups])

  useEffect(() => {
    void refreshWorkspace()
    const timer = window.setInterval(() => {
      if (document.visibilityState !== 'visible') return
      void refreshWorkspace({ silent: true })
    }, 5000)
    return () => window.clearInterval(timer)
  }, [refreshWorkspace])

  useEffect(() => {
    let refreshTimer: number | undefined
    const off = onBrowserInstanceLifecycle(() => {
      if (refreshTimer) window.clearTimeout(refreshTimer)
      refreshTimer = window.setTimeout(() => {
        if (document.visibilityState !== 'visible') return
        void refreshWorkspace({ silent: true })
      }, INSTANCE_EVENT_REFRESH_DEBOUNCE_MS)
    })

    return () => {
      if (refreshTimer) window.clearTimeout(refreshTimer)
      off()
    }
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
  const profileNameById = useMemo(() => {
    const map = new Map<string, string>()
    profiles.forEach((profile) => map.set(profile.profileId, profile.profileName))
    return map
  }, [profiles])
  const fingerprintHealthResults = useMemo(() => (
    Object.values(fingerprintHealthById)
      .sort((a, b) => getFingerprintHealthTimestamp(b) - getFingerprintHealthTimestamp(a))
      .slice(0, 12)
  ), [fingerprintHealthById])
  const identityReportResults = useMemo(() => (
    Object.values(identityReportById)
      .sort((a, b) => getIdentityReportTimestamp(b) - getIdentityReportTimestamp(a))
      .slice(0, 12)
  ), [identityReportById])
  const currentUiState = useMemo<WorkbenchUiState>(() => ({
    search,
    statusFilter,
    groupFilter,
    activeGroupId: activeGroupId || '',
    selectedIds: Array.from(selectedIds),
    scrollTop: Math.round(virtualProfiles.scrollTop),
    targetUrl,
    selectedReportKind,
    selectedReportId,
    selectedReportProfileId,
    expandedItems: Array.from(expandedItems),
    thirdPartyEnabled,
    updatedAt: new Date().toISOString(),
  }), [
    activeGroupId,
    expandedItems,
    groupFilter,
    search,
    selectedIds,
    selectedReportId,
    selectedReportKind,
    selectedReportProfileId,
    statusFilter,
    targetUrl,
    thirdPartyEnabled,
    virtualProfiles.scrollTop,
  ])

  useEffect(() => {
    latestUiStateRef.current = currentUiState
  }, [currentUiState])

  useEffect(() => {
    if (!uiStateLoaded) return
    const timer = window.setTimeout(() => {
      saveWorkbenchUiState(currentUiState).catch(() => {
        // UI state persistence is best-effort and must not block workbench use.
      })
    }, 350)
    return () => window.clearTimeout(timer)
  }, [currentUiState, uiStateLoaded])

  useEffect(() => () => {
    if (!uiStateLoadedRef.current || !latestUiStateRef.current) return
    saveWorkbenchUiState(latestUiStateRef.current).catch(() => {
      // Route changes should try to preserve state, but should never block unmount.
    })
  }, [])

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

  const mergeProfileStatus = useCallback((profile: BrowserProfile | null) => {
    if (!profile) return
    setProfiles((prev) => prev.map((item) => (
      item.profileId === profile.profileId ? { ...item, ...profile } : item
    )))
  }, [])

  const waitForDebugReady = useCallback(async (profileId: string) => {
    const deadline = Date.now() + START_STATUS_POLL_TIMEOUT_MS
    let latestProfile: BrowserProfile | null = null

    while (Date.now() < deadline) {
      await sleep(START_STATUS_POLL_INTERVAL_MS)
      try {
        const status = await getBrowserInstanceStatus(profileId)
        if (status) {
          latestProfile = status
          mergeProfileStatus(status)
        }
        if (status && (!status.running || status.lastError)) return status
        if (status?.debugReady) return status
      } catch {
        // Startup polling is best-effort; the start action itself has already succeeded.
      }
    }

    return latestProfile
  }, [mergeProfileStatus])

  const executeTask = async (task: WorkbenchTask) => {
    updateTask(task.id, { status: 'running', error: '' })
    let feedDetail = task.detail || taskLabel(task.type)
    try {
      switch (task.type) {
        case 'start': {
          const startedProfile = await startBrowserInstance(task.profileId)
          mergeProfileStatus(startedProfile)
          const readyProfile = startedProfile?.debugReady ? startedProfile : await waitForDebugReady(task.profileId)
          if (!readyProfile?.debugReady) {
            feedDetail = '启动完成，等待调试就绪'
          }
          break
        }
        case 'stop': {
          const stoppedProfile = await stopBrowserInstance(task.profileId)
          mergeProfileStatus(stoppedProfile)
          break
        }
        case 'navigate':
          await navigateProfile(task.profileId, task.detail)
          break
        case 'refresh':
          await refreshProfile(task.profileId)
          break
        case 'screenshot':
          throw new Error('截图/预览功能已在实例工作台禁用')
        case 'activate':
          await activateProfileWindow(task.profileId)
          break
        case 'fingerprint-health': {
          setFingerprintHealthCheckingIds((prev) => {
            const next = new Set(prev)
            next.add(task.profileId)
            return next
          })
          try {
            const health = await checkWorkbenchFingerprintHealthProfile(task.profileId)
            const normalizedHealth = {
              ...health,
              profileId: health.profileId || task.profileId,
              profileName: health.profileName || task.profileName,
            }
            setFingerprintHealthById((prev) => ({
              ...prev,
              [normalizedHealth.profileId]: normalizedHealth,
            }))
            setSelectedReportKind('fingerprint_health')
            setSelectedReportProfileId(normalizedHealth.profileId)
            saveWorkbenchDetectionResult(detectionFromFingerprintHealth(normalizedHealth)).catch(() => {
              // Detection persistence failure should not hide the runtime result.
            })
            feedDetail = `指纹体检 ${normalizedHealth.score} 分 · ${fingerprintHealthLevelLabel(normalizedHealth.level)}`
          } finally {
            setFingerprintHealthCheckingIds((prev) => {
              const next = new Set(prev)
              next.delete(task.profileId)
              return next
            })
          }
          break
        }
        default:
          throw new Error(`未知任务: ${task.type}`)
      }

      updateTask(task.id, { status: 'success', detail: feedDetail })
      addActionToFeed({
        id: task.id,
        operation: task.type,
        windowName: task.profileName,
        detail: feedDetail,
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

  const runIdentityReports = async (targets: BrowserProfile[]) => {
    const runnable = targets.filter((profile) => profile.running && profile.debugReady)
    if (runnable.length === 0) {
      toast.warning('请选择正在运行且 CDP 就绪的实例')
      return
    }
    for (const profile of runnable) {
      setIdentityReportCheckingIds((prev) => {
        const next = new Set(prev)
        next.add(profile.profileId)
        return next
      })
      try {
        const report = await checkWorkbenchIdentityReport(profile.profileId)
        const normalizedReport = {
          ...report,
          profileId: report.profileId || profile.profileId,
          profileName: report.profileName || profile.profileName,
        }
        setIdentityReportById((prev) => ({
          ...prev,
          [normalizedReport.profileId]: normalizedReport,
        }))
        setSelectedReportKind('identity_report')
        setSelectedReportProfileId(normalizedReport.profileId)
        saveWorkbenchDetectionResult(detectionFromIdentityReport(normalizedReport)).catch(() => {
          // Detection persistence failure should not block the operator.
        })
        addActionToFeed({
          id: `identity-${profile.profileId}-${Date.now()}`,
          operation: 'identity-report',
          windowName: profile.profileName,
          detail: `身份强度 ${normalizedReport.score} · ${IDENTITY_LEVEL_UI[normalizedReport.level]?.label || normalizedReport.level}`,
          timestamp: new Date().toISOString(),
          status: 'ok',
        })
      } catch (err) {
        const message = normalizeError(err)
        addActionToFeed({
          id: `identity-${profile.profileId}-${Date.now()}`,
          operation: 'identity-report',
          windowName: profile.profileName,
          detail: message,
          timestamp: new Date().toISOString(),
          status: 'error',
        })
        toast.error(message)
      } finally {
        setIdentityReportCheckingIds((prev) => {
          const next = new Set(prev)
          next.delete(profile.profileId)
          return next
        })
      }
    }
  }

  const runDetectorForSelectedProfile = async (site: WorkbenchDetectorSite) => {
    const profile = profiles.find((item) => item.profileId === selectedReportProfileId)
      || selectedProfiles.find((item) => item.running && item.debugReady)
    if (!profile) {
      toast.warning('请先选择一个运行中实例')
      return
    }
    if (!profile.running || !profile.debugReady) {
      toast.warning('第三方检测需要实例运行且 CDP 就绪')
      return
    }
    const runKey = `${profile.profileId}:${site.id}`
    setDetectorRunningIds((prev) => {
      const next = new Set(prev)
      next.add(runKey)
      return next
    })
    try {
      const result = await runWorkbenchDetectorSite(profile.profileId, site.id)
      setDetectorResults((prev) => [result, ...prev.filter((item) => item.id !== result.id)].slice(0, 50))
      setSelectedReportKind('detector_site_run')
      setSelectedReportProfileId(profile.profileId)
      setSelectedReportId(result.id)
      toast.success(`${site.name} 已在真实 profile 新标签打开`)
    } catch (err) {
      toast.error(normalizeError(err))
    } finally {
      setDetectorRunningIds((prev) => {
        const next = new Set(prev)
        next.delete(runKey)
        return next
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

  const activeGroup = groups.find((group) => group.id === activeGroupId) || groups[0]
  const selectedReportProfile = profiles.find((profile) => profile.profileId === selectedReportProfileId)
    || selectedProfiles[0]
  const selectedFingerprintHealth = selectedReportProfile ? fingerprintHealthById[selectedReportProfile.profileId] : undefined
  const selectedIdentityReport = selectedReportProfile ? identityReportById[selectedReportProfile.profileId] : undefined
  const selectedDetectorRuns = selectedReportProfile
    ? detectorResults.filter((result) => result.profileId === selectedReportProfile.profileId)
    : detectorResults.slice(0, 5)
  const selectedProxyQuality = proxyQualityLabel(selectedIdentityReport)

  const toggleExpandedItem = (id: string) => {
    setExpandedItems((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

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
                  <Button size="sm" variant="secondary" className="whitespace-nowrap shrink-0" onClick={() => enqueueTasks('fingerprint-health', selectedRunningProfiles)} disabled={selectedRunningProfiles.length === 0 || taskRunning}>
                    <Fingerprint className="w-3.5 h-3.5" /> 指纹体检
                  </Button>
                  <Button size="sm" variant="secondary" className="whitespace-nowrap shrink-0" onClick={() => runIdentityReports(selectedRunningProfiles)} disabled={selectedRunningProfiles.length === 0 || taskRunning || identityReportCheckingIds.size > 0}>
                    <ShieldCheck className="w-3.5 h-3.5" /> 身份体检
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
                const fingerprintHealth = fingerprintHealthById[profile.profileId]
                const fingerprintHealthChecking = fingerprintHealthCheckingIds.has(profile.profileId)
                const identityReport = identityReportById[profile.profileId]
                const identityReportChecking = identityReportCheckingIds.has(profile.profileId)
                const selected = selectedIds.has(profile.profileId)
                const cardProxyQuality = proxyQualityLabel(identityReport)
                const checkedAt = latestDetectionTime(fingerprintHealth, identityReport)
                const mainRisk = identityReport
                  ? getIdentityReportSummary(identityReport)
                  : fingerprintHealth
                    ? getFingerprintHealthSummary(fingerprintHealth)
                    : '尚未体检'

                return (
                  <div
                    key={profile.profileId}
                    className="absolute left-0 right-0"
                    style={{ transform: `translateY(${index * PROFILE_ROW_HEIGHT}px)` }}
                  >
                    <Card padding="none" className={selected ? 'border-[var(--color-accent)]' : undefined}>
                    <div className="grid grid-cols-[176px_minmax(0,1fr)] min-h-[214px]">
                      <button
                        type="button"
                        className="bg-[var(--color-bg-base)] border-r border-[var(--color-border-muted)] text-left p-3 flex flex-col gap-3"
                        onClick={() => {
                          setSelectedReportProfileId(profile.profileId)
                          setSelectedReportKind(identityReport ? 'identity_report' : fingerprintHealth ? 'fingerprint_health' : '')
                        }}
                      >
                        <div className="flex items-center justify-between gap-2">
                          <span className="text-xs font-medium text-[var(--color-text-muted)]">身份状态</span>
                          {statusBadge(profile, runtime)}
                        </div>
                        <div className="grid grid-cols-2 gap-2">
                          <div className="rounded-md bg-[var(--color-bg-secondary)] px-2 py-2">
                            <div className="text-[10px] text-[var(--color-text-muted)]">指纹分</div>
                            <div className="text-lg font-semibold text-[var(--color-text-primary)]">
                              {fingerprintHealth ? fingerprintHealth.score : '-'}
                            </div>
                          </div>
                          <div className="rounded-md bg-[var(--color-bg-secondary)] px-2 py-2">
                            <div className="text-[10px] text-[var(--color-text-muted)]">身份分</div>
                            <div className="text-lg font-semibold text-[var(--color-text-primary)]">
                              {identityReport ? identityReport.score : '-'}
                            </div>
                          </div>
                        </div>
                        <div className="flex flex-wrap gap-1">
                          {fingerprintHealth && fingerprintHealthBadge(fingerprintHealth)}
                          {identityReport && identityReportBadge(identityReport)}
                          <Badge size="sm" variant={cardProxyQuality.variant}>{cardProxyQuality.label}</Badge>
                        </div>
                        <div className="mt-auto text-[11px] text-[var(--color-text-muted)] line-clamp-2">
                          {checkedAt > 0 ? `最近检测 ${new Date(checkedAt).toLocaleTimeString()}` : '未检测'}
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
                              {fingerprintHealth && fingerprintHealthBadge(fingerprintHealth)}
                              {identityReport && identityReportBadge(identityReport)}
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
                              更新 {new Date(profile.updatedAt).toLocaleString()}
                            </span>
                          </div>
                        </div>

                        <div className="rounded-md border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] px-2.5 py-2 text-xs min-w-0">
                          <div className="flex items-center gap-1.5 min-w-0">
                            <ShieldCheck className="w-3.5 h-3.5 shrink-0 text-[var(--color-text-muted)]" />
                            <span className="font-medium text-[var(--color-text-primary)] shrink-0">主要风险</span>
                            <span className="text-[var(--color-text-muted)] truncate">{mainRisk}</span>
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
                          <Button size="sm" variant="ghost" className="whitespace-nowrap shrink-0" onClick={() => enqueueTasks('fingerprint-health', [profile])} disabled={!profile.running || !profile.debugReady || taskRunning} loading={fingerprintHealthChecking}>
                            <Fingerprint className="w-3.5 h-3.5" /> 体检
                          </Button>
                          <Button size="sm" variant="ghost" className="whitespace-nowrap shrink-0" onClick={() => runIdentityReports([profile])} disabled={!profile.running || !profile.debugReady || taskRunning || identityReportChecking} loading={identityReportChecking}>
                            <ShieldCheck className="w-3.5 h-3.5" /> 身份
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
          <Card title="身份强度报告" subtitle={identityReportResults.length > 0 ? `最近 ${identityReportResults.length} 条` : '等待体检'}>
            {selectedIdentityReport && (
              <div className="mb-3 rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] px-3 py-2">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <p className="text-xs font-medium text-[var(--color-text-primary)] truncate">
                      {selectedIdentityReport.profileName || selectedReportProfile?.profileName || selectedIdentityReport.profileId}
                    </p>
                    <p className="text-[11px] text-[var(--color-text-muted)]">
                      {selectedIdentityReport.source || 'local-cdp'} · {new Date(getIdentityReportTime(selectedIdentityReport)).toLocaleString()}
                    </p>
                  </div>
                  {identityReportBadge(selectedIdentityReport)}
                </div>
                <div className="mt-2 grid grid-cols-2 gap-1.5 text-[11px]">
                  <Badge size="sm" variant="default">指纹 {selectedIdentityReport.subscores.fingerprintVisible}</Badge>
                  <Badge size="sm" variant="default">一致性 {selectedIdentityReport.subscores.consistency}</Badge>
                  <Badge size="sm" variant="default">Profile {selectedIdentityReport.subscores.profilePersistence}</Badge>
                  <Badge size="sm" variant={selectedProxyQuality.variant}>{selectedProxyQuality.label}</Badge>
                  <Badge size="sm" variant="default">行为 {selectedIdentityReport.subscores.behaviorNaturalness}</Badge>
                  <Badge size="sm" variant="default">自动化 {selectedIdentityReport.subscores.automationSafety}</Badge>
                </div>
                <p className="mt-2 text-[11px] text-[var(--color-text-secondary)] line-clamp-2">
                  {getIdentityReportSummary(selectedIdentityReport)}
                </p>
                <div className="mt-2 flex items-center justify-between gap-2">
                  <span className="text-[11px] text-[var(--color-text-muted)]">
                    维度 {selectedIdentityReport.dimensions?.length || 0}
                  </span>
                  <Button size="sm" variant="ghost" onClick={() => toggleExpandedItem('identity-details')}>
                    {expandedItems.has('identity-details') ? '收起明细' : '展开明细'}
                  </Button>
                </div>
                {expandedItems.has('identity-details') && (
                  <div className="mt-2 space-y-1 max-h-56 overflow-y-auto pr-1">
                    {(selectedIdentityReport.dimensions || []).map((dim) => (
                      <div key={`${dim.id}-${dim.category}-${dim.layer}`} className="flex items-start gap-2 text-[11px]">
                        <Badge size="sm" variant={dim.status === 'pass' ? 'success' : dim.status === 'warning' ? 'warning' : dim.status === 'fail' ? 'error' : 'default'}>
                          {dim.status}
                        </Badge>
                        <div className="min-w-0">
                          <div className="text-[var(--color-text-primary)] truncate">{dim.id}</div>
                          <div className="text-[var(--color-text-muted)]">{dim.message}</div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
            {identityReportResults.length === 0 ? (
              <div className="py-10 text-center text-xs text-[var(--color-text-muted)]">
                暂无身份体检结果
              </div>
            ) : (
              <div className="space-y-2 max-h-[320px] overflow-y-auto pr-1">
                {identityReportResults.map((report) => {
                  const abnormal = (report.dimensions || []).filter((dim) => dim.status === 'fail' || dim.status === 'warning')
                  const checkedAt = getIdentityReportTimestamp(report)
                  const profileName = report.profileName || profileNameById.get(report.profileId) || report.profileId

                  return (
                    <div key={report.profileId} className="rounded-lg border border-[var(--color-border-muted)] px-3 py-2">
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <p className="text-xs font-medium text-[var(--color-text-primary)] truncate">
                            {profileName}
                          </p>
                          <p className="text-[11px] text-[var(--color-text-muted)] truncate">
                            {checkedAt > 0 ? new Date(checkedAt).toLocaleTimeString() : '刚刚'} · {report.source || 'local-cdp'}
                          </p>
                        </div>
                        {identityReportBadge(report)}
                      </div>
                      <div className="mt-2 grid grid-cols-3 gap-1.5 text-[11px]">
                        <Badge size="sm" variant="default">指纹 {report.subscores.fingerprintVisible}</Badge>
                        <Badge size="sm" variant="default">一致 {report.subscores.consistency}</Badge>
                        <Badge size="sm" variant="default">Profile {report.subscores.profilePersistence}</Badge>
                      </div>
                      <div className="mt-2 flex flex-wrap items-center gap-1.5">
                        <Badge size="sm" variant="success">维度 {report.dimensions?.length || 0}</Badge>
                        <Badge size="sm" variant={abnormal.length > 0 ? 'warning' : 'default'}>异常 {abnormal.length}</Badge>
                      </div>
                      <p className="mt-2 text-[11px] text-[var(--color-text-secondary)] line-clamp-2">
                        {getIdentityReportSummary(report)}
                      </p>
                    </div>
                  )
                })}
              </div>
            )}
          </Card>

          <Card title="指纹体检结果" subtitle={fingerprintHealthResults.length > 0 ? `最近 ${fingerprintHealthResults.length} 条` : '等待体检'}>
            {selectedFingerprintHealth && (
              <div className="mb-3 rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] px-3 py-2">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <p className="text-xs font-medium text-[var(--color-text-primary)] truncate">
                      {selectedFingerprintHealth.profileName || selectedReportProfile?.profileName || selectedFingerprintHealth.profileId}
                    </p>
                    <p className="text-[11px] text-[var(--color-text-muted)]">
                      {getFingerprintHealthSource(selectedFingerprintHealth) || 'local-cdp'} · {new Date(getFingerprintHealthTime(selectedFingerprintHealth)).toLocaleString()}
                    </p>
                  </div>
                  {fingerprintHealthBadge(selectedFingerprintHealth)}
                </div>
                <p className="mt-2 text-[11px] text-[var(--color-text-secondary)] line-clamp-2">
                  {getFingerprintHealthSummary(selectedFingerprintHealth)}
                </p>
                <div className="mt-2 flex items-center justify-between gap-2">
                  <span className="text-[11px] text-[var(--color-text-muted)]">
                    检查项 {selectedFingerprintHealth.checks?.length || 0}
                  </span>
                  <Button size="sm" variant="ghost" onClick={() => toggleExpandedItem('fingerprint-details')}>
                    {expandedItems.has('fingerprint-details') ? '收起明细' : '展开明细'}
                  </Button>
                </div>
                {expandedItems.has('fingerprint-details') && (
                  <div className="mt-2 space-y-1 max-h-56 overflow-y-auto pr-1">
                    {(selectedFingerprintHealth.checks || []).map((check, index) => {
                      const checkStatus = getFingerprintCheckStatus(check)
                      return (
                        <div key={`${getFingerprintCheckName(check)}-${index}`} className="flex items-start gap-2 text-[11px]">
                          <Badge size="sm" variant={checkStatus === 'passed' ? 'success' : checkStatus === 'warning' ? 'warning' : checkStatus === 'failed' ? 'error' : 'default'}>
                            {checkStatus}
                          </Badge>
                          <div className="min-w-0">
                            <div className="text-[var(--color-text-primary)] truncate">{getFingerprintCheckName(check)}</div>
                            <div className="text-[var(--color-text-muted)]">{getFingerprintCheckMessage(check)}</div>
                          </div>
                        </div>
                      )
                    })}
                  </div>
                )}
              </div>
            )}
            {fingerprintHealthResults.length === 0 ? (
              <div className="py-10 text-center text-xs text-[var(--color-text-muted)]">
                暂无体检结果
              </div>
            ) : (
              <div className="space-y-2 max-h-[320px] overflow-y-auto pr-1">
                {fingerprintHealthResults.map((health) => {
                  const checks = health.checks || []
                  const abnormalChecks = checks.filter((check) => {
                    const status = getFingerprintCheckStatus(check)
                    return status === 'failed' || status === 'warning'
                  })
                  const visibleChecks = (abnormalChecks.length > 0 ? abnormalChecks : checks).slice(0, 2)
                  const checkedAt = getFingerprintHealthTimestamp(health)
                  const source = getFingerprintHealthSource(health)
                  const profileName = health.profileName || profileNameById.get(health.profileId) || health.profileId

                  return (
                    <div key={health.profileId} className="rounded-lg border border-[var(--color-border-muted)] px-3 py-2">
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <p className="text-xs font-medium text-[var(--color-text-primary)] truncate">
                            {profileName}
                          </p>
                          <p className="text-[11px] text-[var(--color-text-muted)] truncate">
                            {checkedAt > 0 ? new Date(checkedAt).toLocaleTimeString() : '刚刚'}
                            {source ? ` · ${source}` : ''}
                          </p>
                        </div>
                        {fingerprintHealthBadge(health)}
                      </div>

                      <div className="mt-2 flex flex-wrap items-center gap-1.5">
                        <Badge size="sm" variant="success">通过 {checks.filter((check) => getFingerprintCheckStatus(check) === 'passed').length}</Badge>
                        <Badge size="sm" variant={abnormalChecks.length > 0 ? 'error' : 'default'}>异常 {abnormalChecks.length}</Badge>
                      </div>

                      <p className="mt-2 text-[11px] text-[var(--color-text-secondary)] line-clamp-2">
                        {getFingerprintHealthSummary(health)}
                      </p>

                      {visibleChecks.length > 0 && (
                        <div className="mt-2 space-y-1">
                          {visibleChecks.map((check, index) => {
                            const checkStatus = getFingerprintCheckStatus(check)
                            return (
                              <div key={`${health.profileId}-${getFingerprintCheckName(check)}-${index}`} className="flex items-center gap-2 text-[11px] min-w-0">
                                <Badge
                                  size="sm"
                                  variant={checkStatus === 'passed' ? 'success' : checkStatus === 'warning' ? 'warning' : checkStatus === 'failed' ? 'error' : 'default'}
                                >
                                  {checkStatus === 'passed' ? '通过' : checkStatus === 'warning' ? '预警' : checkStatus === 'failed' ? '异常' : '未知'}
                                </Badge>
                                <span className="text-[var(--color-text-muted)] truncate">
                                  {getFingerprintCheckName(check)}：{getFingerprintCheckMessage(check)}
                                </span>
                              </div>
                            )
                          })}
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            )}
          </Card>

          <Card title="风险雷达 / 检测站" subtitle={thirdPartyEnabled ? '第三方手动开启' : '默认本地检测'}>
            <div className="space-y-3">
              <label className="flex items-center justify-between gap-3 rounded-lg border border-[var(--color-border-muted)] px-3 py-2 text-xs">
                <span className="text-[var(--color-text-primary)]">允许第三方检测站</span>
                <input
                  type="checkbox"
                  checked={thirdPartyEnabled}
                  onChange={(event) => setThirdPartyEnabled(event.target.checked)}
                  className="w-4 h-4 accent-[var(--color-accent)]"
                />
              </label>
              <p className="text-[11px] text-[var(--color-text-muted)]">
                默认体检只用本地 CDP。第三方检测会用真实 profile 新标签访问，可能留下访问痕迹，不会自动清理 cookie/history。
              </p>
              <div className="grid grid-cols-1 gap-2">
                {detectorSites.map((site) => {
                  const profile = selectedReportProfile
                  const runKey = `${profile?.profileId || ''}:${site.id}`
                  const disabled = !thirdPartyEnabled || !profile?.running || !profile?.debugReady || detectorRunningIds.has(runKey)
                  return (
                    <div key={site.id} className="rounded-lg border border-[var(--color-border-muted)] px-3 py-2">
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <p className="text-xs font-medium text-[var(--color-text-primary)]">{site.name}</p>
                          <p className="text-[11px] text-[var(--color-text-muted)] line-clamp-2">{site.notes}</p>
                        </div>
                        <Badge size="sm" variant="default">{site.gate}</Badge>
                      </div>
                      <Button
                        size="sm"
                        variant="ghost"
                        className="mt-2"
                        onClick={() => runDetectorForSelectedProfile(site)}
                        disabled={disabled}
                        loading={detectorRunningIds.has(runKey)}
                      >
                        <ExternalLink className="w-3.5 h-3.5" /> 打开检测
                      </Button>
                    </div>
                  )
                })}
              </div>
              {selectedDetectorRuns.length > 0 && (
                <div className="space-y-1 max-h-36 overflow-y-auto pr-1">
                  {selectedDetectorRuns.slice(0, 5).map((result) => (
                    <div key={result.id} className="text-[11px] rounded border border-[var(--color-border-muted)] px-2 py-1">
                      <div className="flex items-center justify-between gap-2">
                        <span className="font-medium text-[var(--color-text-primary)]">{result.source}</span>
                        <span className="text-[var(--color-text-muted)]">{new Date(result.createdAt).toLocaleTimeString()}</span>
                      </div>
                      <p className="text-[var(--color-text-muted)] line-clamp-2">{result.summary.join(' / ')}</p>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </Card>

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
