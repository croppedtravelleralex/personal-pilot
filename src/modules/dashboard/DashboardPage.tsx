import { useEffect, useState } from 'react'
import { Monitor, Play, Shield, Cpu, ArrowRight, Globe, Settings, Activity, AlertTriangle, CheckCircle2, Clock, ScanSearch } from 'lucide-react'
import { Link } from 'react-router-dom'
import { Button, Card, toast } from '../../shared/components'
import { collectDesktopWebViewEvidence, fetchDashboardStats, fetchEvidenceReportHistory, fetchReleaseSmokeContract, reloadConfig } from './api'
import type { DashboardStats } from './types'
import type {
  DesktopEvidenceReportHistory,
  DesktopEvidenceReportSummary,
  DesktopReleaseBudgetResult,
  DesktopReleaseSmokeContract,
  DesktopRuntimeAdapterContractItem,
  DesktopValidationBrowserSignal,
} from '../../types/desktop'

interface StatCardProps {
  title: string
  value: string | number
  icon: React.ReactNode
  color: string
}

function StatCard({ title, value, icon, color }: StatCardProps) {
  return (
    <div className="rounded-xl border border-[var(--color-border-default)] bg-[var(--color-bg-card)] p-5">
      <div className="mb-3 flex items-center justify-between">
        <span className="text-sm text-[var(--color-text-muted)]">{title}</span>
        <div className={`flex h-9 w-9 items-center justify-center rounded-lg ${color}`}>
          {icon}
        </div>
      </div>
      <div className="text-2xl font-semibold text-[var(--color-text-primary)]">{value}</div>
    </div>
  )
}

const QUICK_LINKS = [
  { to: '/browser', icon: <Monitor className="h-5 w-5" />, label: '浏览器实例', desc: '管理所有指纹浏览器' },
  { to: '/browser/proxy-pool', icon: <Shield className="h-5 w-5" />, label: '代理池', desc: '配置和测试代理节点' },
  { to: '/browser/cores', icon: <Cpu className="h-5 w-5" />, label: '内核管理', desc: '管理可选内核版本' },
  { to: '/settings', icon: <Settings className="h-5 w-5" />, label: '系统设置', desc: '全局参数配置' },
]

const EVIDENCE_KINDS = [
  { kind: 'm4_acceptance', label: 'M4 Gate' },
  { kind: 'm4_browser_payload_schema', label: 'M4 Payload' },
  { kind: 'm4_dashboard_facade', label: 'M4 Dashboard' },
  { kind: 'm4_settings_logs_facade', label: 'M4 Settings/Logs' },
  { kind: 'm4_profile_facade', label: 'M4 Profile' },
  { kind: 'm4_behavior_preset_facade', label: 'M4 Behavior' },
  { kind: 'm4_automation_facade', label: 'M4 Automation' },
  { kind: 'm5_release_health', label: 'M5 Health' },
  { kind: 'release_performance', label: 'Release 性能' },
  { kind: 'runtime_adapter', label: 'Runtime Adapter' },
  { kind: 'profile_browser_comparison', label: 'Browser 对比' },
  { kind: 'm10_headed_repeatability', label: 'M10 Repeatability' },
  { kind: 'm15_browser_pool', label: 'M15 Pool' },
  { kind: 'remote_proxy_tls', label: '远程代理 TLS' },
  { kind: 'headed_external_smoke', label: 'Headed Browser' },
  { kind: 'camoufox_binary_task', label: 'Camoufox' },
  { kind: 'provider_acceptance', label: 'Provider' },
  { kind: 'session_portability', label: 'SessionBundle' },
  { kind: 'm8_session_handoff', label: 'M8 Handoff' },
  { kind: 'taxonomy_coverage', label: 'Taxonomy 覆盖' },
  { kind: 'taxonomy_audit', label: 'Taxonomy Audit' },
  { kind: 'external_distribution', label: '外部分发' },
]

const UNLIMITED = Number.POSITIVE_INFINITY

function statusTone(status: string) {
  const normalized = status.toLowerCase()
  if (normalized.includes('failed')) {
    return {
      icon: <AlertTriangle className="h-4 w-4" />,
      className: 'border-[var(--color-error)]/30 bg-[var(--color-error)]/10 text-[var(--color-error)]',
    }
  }
  if (
    normalized.includes('expected_blocked') ||
    normalized.includes('passed_with_expected_external_blockers') ||
    normalized.includes('passed_with_budget_overrun') ||
    normalized.includes('passed_with_recorded_drift') ||
    normalized.includes('budget_overrun') ||
    normalized.includes('over_budget') ||
    normalized.includes('drift') ||
    normalized.includes('partial') ||
    normalized.includes('warning') ||
    normalized.includes('pending') ||
    normalized.includes('materialized_contract') ||
    normalized.includes('contract') ||
    normalized.includes('handoff_package_ready') ||
    normalized === 'not_run'
  ) {
    return {
      icon: <Clock className="h-4 w-4" />,
      className: 'border-[var(--color-warning)]/30 bg-[var(--color-warning)]/10 text-[var(--color-warning)]',
    }
  }
  if (normalized.includes('blocked') || normalized.includes('failed')) {
    return {
      icon: <AlertTriangle className="h-4 w-4" />,
      className: 'border-[var(--color-error)]/30 bg-[var(--color-error)]/10 text-[var(--color-error)]',
    }
  }
  if (normalized.includes('passed') || normalized === 'ready' || normalized.includes('observed') || normalized.includes('healthy') || normalized === 'within_budget') {
    return {
      icon: <CheckCircle2 className="h-4 w-4" />,
      className: 'border-[var(--color-success)]/30 bg-[var(--color-success)]/10 text-[var(--color-success)]',
    }
  }
  return {
    icon: <Clock className="h-4 w-4" />,
    className: 'border-[var(--color-warning)]/30 bg-[var(--color-warning)]/10 text-[var(--color-warning)]',
  }
}

function latestReportsByKind(history: DesktopEvidenceReportHistory): Record<string, DesktopEvidenceReportSummary> {
  return history.reports.reduce<Record<string, DesktopEvidenceReportSummary>>((acc, report) => {
    const current = acc[report.kind]
    if (!current || report.generatedAt > current.generatedAt) {
      acc[report.kind] = report
    }
    return acc
  }, {})
}

function reportFileName(reportPath: string): string {
  return reportPath.split(/[\\/]/).pop() || reportPath
}

function evidenceDiffFieldLabel(field: string): string {
  switch (field) {
    case 'status':
      return 'status'
    case 'failureReason':
      return 'reason'
    case 'failureReasonCategory':
      return 'category'
    case 'risk':
      return 'risk'
    default:
      return field
  }
}

function runtimeAdapterEvidenceScore(adapter: DesktopRuntimeAdapterContractItem): number {
  const value = [
    adapter.status,
    adapter.profileRuntimeEvidence,
    adapter.fingerprintRuntimeDepth,
    adapter.processLifecycleStatus,
    adapter.cdpAttachStatus,
  ].join(' ').toLowerCase()
  if (value.includes('real_binary_validation_probe') || value.includes('profile_browser_validation_probe_observed')) return 500
  if (value.includes('real_binary_task') || value.includes('minimal_real_binary_task_passed')) return 400
  if (value.includes('source_test') || value.includes('source_contract')) return 300
  if (value.includes('warning_stub')) return 100
  if (value.includes('blocked')) return 0
  return 200
}

function rankedRuntimeAdapters(contract: DesktopReleaseSmokeContract | null): DesktopRuntimeAdapterContractItem[] {
  return [...(contract?.adapterContracts ?? [])].sort((left, right) => (
    runtimeAdapterEvidenceScore(right) - runtimeAdapterEvidenceScore(left)
    || left.adapterId.localeCompare(right.adapterId)
  ))
}

function releaseBudgetMeasurement(metric: DesktopReleaseBudgetResult): string {
  const unit = metric.unit ? ` ${metric.unit}` : ''
  const measured = metric.measured === null ? 'pending' : `${metric.measured}${unit}`
  return `${measured} / ${metric.target}${unit} · drift ${metric.driftTarget}${unit}`
}

function desktopWebViewSignal(
  id: string,
  category: string,
  status: string,
  label: string,
  summary: string,
  detail: string,
  durationMs: number,
): DesktopValidationBrowserSignal {
  return {
    id,
    category,
    layer: 'observed',
    status,
    label,
    summary,
    detail: `scope=desktop-webview; target-profile-browser=false; ${detail}`,
    collectorScope: 'desktop-webview',
    runtimeAdapter: 'desktop_webview',
    targetProfileBrowser: false,
    failureReason: status === 'failed' ? summary : null,
    durationMs,
  }
}

function collectDesktopWebViewSignals(): DesktopValidationBrowserSignal[] {
  const started = performance.now()
  const elapsed = () => Math.max(0, Math.round(performance.now() - started))
  const signals: DesktopValidationBrowserSignal[] = []

  try {
    const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || 'unknown'
    signals.push(desktopWebViewSignal(
      'desktop-webview-fingerprint-surface',
      'fingerprint',
      navigator.userAgent ? 'succeeded' : 'warning',
      'Desktop WebView fingerprint surface',
      'Desktop WebView navigator, timezone, and screen surfaces were sampled.',
      `userAgent=${navigator.userAgent}; platform=${navigator.platform}; language=${navigator.language}; timezone=${timezone}; screen=${window.screen.width}x${window.screen.height}; hardwareConcurrency=${navigator.hardwareConcurrency || 0}`,
      elapsed(),
    ))
  } catch (error: any) {
    signals.push(desktopWebViewSignal(
      'desktop-webview-fingerprint-surface',
      'fingerprint',
      'failed',
      'Desktop WebView fingerprint surface',
      error?.message || 'Desktop WebView fingerprint probe failed.',
      `error=${error?.message || String(error)}`,
      elapsed(),
    ))
  }

  try {
    const canvas = document.createElement('canvas')
    canvas.width = 180
    canvas.height = 48
    const ctx = canvas.getContext('2d')
    if (!ctx) {
      signals.push(desktopWebViewSignal(
        'desktop-webview-canvas-render',
        'canvas',
        'warning',
        'Desktop WebView canvas render',
        'Desktop WebView could not create a 2D canvas context.',
        'canvasContext=false',
        elapsed(),
      ))
    } else {
      ctx.fillStyle = '#f2f5f9'
      ctx.fillRect(0, 0, canvas.width, canvas.height)
      ctx.fillStyle = '#273142'
      ctx.font = '16px sans-serif'
      ctx.fillText('PersonaPilot WebView probe', 8, 28)
      const dataUrlLength = canvas.toDataURL('image/png').length
      signals.push(desktopWebViewSignal(
        'desktop-webview-canvas-render',
        'canvas',
        dataUrlLength > 100 ? 'succeeded' : 'warning',
        'Desktop WebView canvas render',
        'Desktop WebView canvas was rendered and sampled.',
        `dataUrlLength=${dataUrlLength}; size=${canvas.width}x${canvas.height}`,
        elapsed(),
      ))
    }
  } catch (error: any) {
    signals.push(desktopWebViewSignal(
      'desktop-webview-canvas-render',
      'canvas',
      'failed',
      'Desktop WebView canvas render',
      error?.message || 'Desktop WebView canvas probe failed.',
      `error=${error?.message || String(error)}`,
      elapsed(),
    ))
  }

  try {
    const AudioCtor = window.AudioContext || (window as any).webkitAudioContext
    if (!AudioCtor) {
      signals.push(desktopWebViewSignal(
        'desktop-webview-audio-context',
        'audio',
        'warning',
        'Desktop WebView audio context',
        'Desktop WebView does not expose AudioContext.',
        'audioContext=false',
        elapsed(),
      ))
    } else {
      const context = new AudioCtor()
      const sampleRate = context.sampleRate || 0
      void context.close?.()
      signals.push(desktopWebViewSignal(
        'desktop-webview-audio-context',
        'audio',
        sampleRate > 0 ? 'succeeded' : 'warning',
        'Desktop WebView audio context',
        'Desktop WebView AudioContext was sampled.',
        `sampleRate=${sampleRate}; state=${context.state || 'unknown'}`,
        elapsed(),
      ))
    }
  } catch (error: any) {
    signals.push(desktopWebViewSignal(
      'desktop-webview-audio-context',
      'audio',
      'failed',
      'Desktop WebView audio context',
      error?.message || 'Desktop WebView audio probe failed.',
      `error=${error?.message || String(error)}`,
      elapsed(),
    ))
  }

  try {
    const hasWebRtc = typeof RTCPeerConnection !== 'undefined'
    signals.push(desktopWebViewSignal(
      'desktop-webview-webrtc-api',
      'webrtc',
      hasWebRtc ? 'warning' : 'warning',
      'Desktop WebView WebRTC API',
      hasWebRtc
        ? 'Desktop WebView exposes RTCPeerConnection; profile-browser leak proof remains separate.'
        : 'Desktop WebView does not expose RTCPeerConnection.',
      `rtcpPeerConnection=${hasWebRtc}`,
      elapsed(),
    ))
  } catch (error: any) {
    signals.push(desktopWebViewSignal(
      'desktop-webview-webrtc-api',
      'webrtc',
      'failed',
      'Desktop WebView WebRTC API',
      error?.message || 'Desktop WebView WebRTC probe failed.',
      `error=${error?.message || String(error)}`,
      elapsed(),
    ))
  }

  try {
    const localStorageAvailable = (() => {
      const key = '__persona_pilot_webview_probe__'
      window.localStorage.setItem(key, '1')
      window.localStorage.removeItem(key)
      return true
    })()
    const sessionStorageAvailable = (() => {
      const key = '__persona_pilot_webview_probe__'
      window.sessionStorage.setItem(key, '1')
      window.sessionStorage.removeItem(key)
      return true
    })()
    signals.push(desktopWebViewSignal(
      'desktop-webview-storage-scope',
      'leak',
      localStorageAvailable || sessionStorageAvailable || navigator.cookieEnabled ? 'succeeded' : 'warning',
      'Desktop WebView storage scope',
      'Desktop WebView storage and cookie surfaces were sampled.',
      `cookieEnabled=${navigator.cookieEnabled}; localStorage=${localStorageAvailable}; sessionStorage=${sessionStorageAvailable}`,
      elapsed(),
    ))
  } catch (error: any) {
    signals.push(desktopWebViewSignal(
      'desktop-webview-storage-scope',
      'leak',
      'failed',
      'Desktop WebView storage scope',
      error?.message || 'Desktop WebView storage probe failed.',
      `error=${error?.message || String(error)}`,
      elapsed(),
    ))
  }

  return signals
}

function EvidenceRow({ label, report }: { label: string; report?: DesktopEvidenceReportSummary }) {
  const status = report?.status ?? 'not_run'
  const tone = statusTone(status)
  return (
    <div className="grid grid-cols-[minmax(92px,120px)_minmax(120px,1fr)] gap-3 border-b border-[var(--color-border-muted)] py-3 last:border-0 sm:grid-cols-[minmax(128px,168px)_minmax(160px,1fr)_minmax(160px,2fr)]">
      <div className="min-w-0 text-sm font-medium text-[var(--color-text-primary)]">{label}</div>
      <div className="min-w-0">
        <span className={`inline-flex max-w-full items-center gap-1 rounded-md border px-2 py-1 text-xs font-medium ${tone.className}`}>
          <span className="shrink-0">{tone.icon}</span>
          <span className="truncate">{status}</span>
        </span>
        {report?.evidenceLevel && (
          <div className="mt-1 truncate text-[11px] text-[var(--color-text-muted)]">{report.evidenceLevel}</div>
        )}
      </div>
      <div className="col-span-2 min-w-0 text-xs text-[var(--color-text-muted)] sm:col-span-1">
        <div className="truncate">{report?.summary ?? 'no local evidence report'}</div>
        {report?.reportPath && (
          <div className="mt-1 truncate text-[var(--color-text-secondary)]" title={report.reportPath}>
            {reportFileName(report.reportPath)}
          </div>
        )}
        {report?.failureReason && <div className="mt-1 truncate text-[var(--color-error)]">{report.failureReason}</div>}
        {report?.failureReasonCategory && report.failureReasonCategory !== 'none' && (
          <div className="mt-1 truncate text-[var(--color-text-secondary)]">{report.failureReasonCategory}</div>
        )}
        {report?.statusTrend && report.statusTrend !== 'new_report_kind' && (
          <div className="mt-1 truncate text-[var(--color-text-secondary)]" title={report.trendSummary}>
            {report.statusTrend} · {report.failureReasonTrend} · {report.failureReasonCategoryTrend} · {report.riskTrend}
          </div>
        )}
        {report?.reportDiffItems?.length ? (
          <div
            className="mt-1 flex min-w-0 flex-wrap gap-x-2 gap-y-1 text-[var(--color-text-secondary)]"
            title={report.reportDiffSummary}
          >
            {report.reportDiffItems.map((item) => (
              <span key={item.field} className="min-w-0 max-w-full truncate">
                {evidenceDiffFieldLabel(item.field)}:{item.previous}-&gt;{item.current}
              </span>
            ))}
          </div>
        ) : null}
        {report?.nextAction && <div className="mt-1 truncate text-[var(--color-text-secondary)]">{report.nextAction}</div>}
      </div>
    </div>
  )
}

export function DashboardPage() {
  const [stats, setStats] = useState<DashboardStats>({
    totalInstances: 0,
    runningInstances: 0,
    proxyCount: 0,
    coreCount: 0,
    memUsedMB: 0,
    maxProfileLimit: UNLIMITED,
    appVersion: 'unknown',
  })
  const [loading, setLoading] = useState(true)
  const [collectingEvidence, setCollectingEvidence] = useState(false)
  const [evidenceHistory, setEvidenceHistory] = useState<DesktopEvidenceReportHistory>({
    generatedAt: '',
    reportCount: 0,
    reports: [],
    summary: '',
  })
  const [releaseContract, setReleaseContract] = useState<DesktopReleaseSmokeContract | null>(null)

  useEffect(() => {
    void load()
  }, [])

  const load = async () => {
    setLoading(true)
    try {
      await reloadConfig()
      const [nextStats, nextEvidenceHistory, nextReleaseContract] = await Promise.all([
        fetchDashboardStats(),
        fetchEvidenceReportHistory(),
        fetchReleaseSmokeContract(),
      ])
      setStats(nextStats)
      setEvidenceHistory(nextEvidenceHistory)
      setReleaseContract(nextReleaseContract)
    } finally {
      setLoading(false)
    }
  }

  const handleCollectDesktopWebViewEvidence = async () => {
    setCollectingEvidence(true)
    try {
      const report = await collectDesktopWebViewEvidence(collectDesktopWebViewSignals())
      toast.success(`WebView evidence saved: ${report.signals.length} signals`)
      await load()
    } catch (error: any) {
      toast.error(error?.message || 'WebView evidence collection failed')
    } finally {
      setCollectingEvidence(false)
    }
  }

  const v = (n: number) => (loading ? '-' : n.toString())
  const evidenceByKind = latestReportsByKind(evidenceHistory)
  const runtimeAdapters = rankedRuntimeAdapters(releaseContract)

  return (
    <div className="animate-fade-in space-y-6">
      <div>
        <h1 className="text-xl font-semibold text-[var(--color-text-primary)]">控制台</h1>
        <p className="mt-1 text-sm text-[var(--color-text-muted)]">personal-pilot 本地指纹浏览器工作台概览</p>
      </div>

      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
        <StatCard
          title="实例总数"
          value={v(stats.totalInstances)}
          icon={<Monitor className="h-4 w-4 text-blue-500" />}
          color="bg-blue-50 dark:bg-blue-900/20"
        />
        <StatCard
          title="运行中"
          value={v(stats.runningInstances)}
          icon={<Play className="h-4 w-4 text-green-500" />}
          color="bg-green-50 dark:bg-green-900/20"
        />
        <StatCard
          title="代理节点"
          value={v(stats.proxyCount)}
          icon={<Globe className="h-4 w-4 text-purple-500" />}
          color="bg-purple-50 dark:bg-purple-900/20"
        />
        <StatCard
          title="内核版本"
          value={v(stats.coreCount)}
          icon={<Cpu className="h-4 w-4 text-orange-500" />}
          color="bg-orange-50 dark:bg-orange-900/20"
        />
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <Card title="快捷操作">
          <div className="grid grid-cols-2 gap-3">
            {QUICK_LINKS.map((link) => (
              <Link
                key={link.to}
                to={link.to}
                className="group flex items-center gap-3 rounded-xl border border-[var(--color-border-default)] bg-[var(--color-bg-subtle)] p-4 transition-all duration-150 hover:border-[var(--color-border-strong)] hover:bg-[var(--color-bg-muted)]"
              >
                <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-[var(--color-accent-muted)] text-[var(--color-text-secondary)] transition-colors group-hover:bg-[var(--color-accent)] group-hover:text-[var(--color-text-inverse)]">
                  {link.icon}
                </div>
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium text-[var(--color-text-primary)]">{link.label}</p>
                  <p className="truncate text-xs text-[var(--color-text-muted)]">{link.desc}</p>
                </div>
                <ArrowRight className="h-4 w-4 shrink-0 -translate-x-2 text-[var(--color-text-muted)] opacity-0 transition-all group-hover:translate-x-0 group-hover:opacity-100" />
              </Link>
            ))}
          </div>
        </Card>

        <Card title="系统信息">
          <div className="space-y-1">
            {[
              { label: '系统版本', value: loading ? '-' : stats.appVersion },
              { label: '运行环境', value: 'Wails v2 + React' },
              { label: '数据存储', value: 'SQLite + YAML' },
              { label: '内存占用', value: loading ? '-' : `${stats.memUsedMB} MB` },
              { label: '实例运行', value: loading ? '-' : `${stats.runningInstances} / ${stats.totalInstances}` },
            ].map((item) => (
              <div
                key={item.label}
                className="flex items-center justify-between border-b border-[var(--color-border-muted)] py-3 last:border-0"
              >
                <span className="text-sm text-[var(--color-text-muted)]">{item.label}</span>
                <span className="text-sm font-medium text-[var(--color-text-primary)]">{item.value}</span>
              </div>
            ))}
          </div>

          <div className="mt-6 border-t border-[var(--color-border-muted)] pt-6">
            <h3 className="mb-3 text-sm font-medium text-[var(--color-text-primary)]">实例策略</h3>
            <div className="rounded-lg border border-[var(--color-success)]/30 bg-[var(--color-success)]/10 p-4">
              <p className="text-sm font-medium text-[var(--color-text-primary)]">本地无限实例</p>
              <p className="mt-1 text-xs text-[var(--color-text-muted)]">运行策略为本地优先，无需外部授权。</p>
            </div>
          </div>
        </Card>
      </div>

      <Card
        title="验收证据"
        subtitle={loading ? 'loading reports' : `${evidenceHistory.reportCount} local reports`}
        actions={(
          <div className="flex items-center gap-2">
            <Button
              variant="secondary"
              size="sm"
              onClick={handleCollectDesktopWebViewEvidence}
              loading={collectingEvidence}
            >
              <ScanSearch className="h-4 w-4" />
              采集 WebView
            </Button>
            <Activity className="h-4 w-4 text-[var(--color-text-muted)]" />
          </div>
        )}
      >
        <div className="divide-y divide-[var(--color-border-muted)]">
          {EVIDENCE_KINDS.map((item) => (
            <EvidenceRow key={item.kind} label={item.label} report={evidenceByKind[item.kind]} />
          ))}
        </div>
      </Card>

      <Card
        title="Runtime Adapter"
        subtitle={releaseContract
          ? `${releaseContract.measurementStatus} · ${releaseContract.budgetStatus} · ${runtimeAdapters.length} adapters`
          : 'release contract unavailable'}
      >
        {releaseContract && (
          <div className="border-b border-[var(--color-border-muted)] pb-3">
            <div className="flex flex-wrap items-start gap-2">
              <span className={`inline-flex max-w-full items-center gap-1 rounded-md border px-2 py-1 text-xs font-medium ${statusTone(releaseContract.releaseHealthStatus).className}`}>
                <span className="shrink-0">{statusTone(releaseContract.releaseHealthStatus).icon}</span>
                <span className="truncate">{releaseContract.releaseHealthStatus}</span>
              </span>
              <div className="min-w-[180px] flex-1 text-xs text-[var(--color-text-muted)]">
                {releaseContract.releaseHealthSummary}
              </div>
            </div>
            <div className="mt-3 grid gap-2 md:grid-cols-3">
              {releaseContract.budgetResults.map(metric => {
                const tone = statusTone(metric.status)
                return (
                  <div key={metric.id} className="min-w-0 rounded-md border border-[var(--color-border-muted)] px-3 py-2">
                    <div className="flex items-center justify-between gap-2">
                      <span className="truncate text-xs font-medium text-[var(--color-text-primary)]">{metric.label}</span>
                      <span className={`inline-flex shrink-0 items-center gap-1 rounded-md border px-1.5 py-0.5 text-[11px] ${tone.className}`}>
                        {tone.icon}
                        <span>{metric.status}</span>
                      </span>
                    </div>
                    <div className="mt-1 truncate text-[11px] text-[var(--color-text-muted)]">
                      {releaseBudgetMeasurement(metric)}
                    </div>
                  </div>
                )
              })}
            </div>
            {releaseContract.mitigationHints[0] && (
              <div className="mt-2 truncate text-xs text-[var(--color-warning)]">
                {releaseContract.mitigationHints[0]}
              </div>
            )}
          </div>
        )}
        <div className="divide-y divide-[var(--color-border-muted)]">
          {runtimeAdapters.length === 0 && (
            <div className="py-3 text-sm text-[var(--color-text-muted)]">no runtime adapter contract</div>
          )}
          {runtimeAdapters.map(adapter => {
            const tone = statusTone(adapter.status)
            const blockers = adapter.blockers.slice(0, 2)
            return (
              <div key={adapter.adapterId} className="grid grid-cols-[minmax(112px,160px)_minmax(160px,1fr)] gap-3 py-3 lg:grid-cols-[minmax(128px,180px)_minmax(180px,1fr)_minmax(200px,2fr)]">
                <div className="min-w-0">
                  <div className="truncate text-sm font-medium text-[var(--color-text-primary)]">{adapter.adapterId}</div>
                  <div className="mt-1 truncate text-[11px] text-[var(--color-text-muted)]">{adapter.runnerKind}</div>
                </div>
                <div className="min-w-0">
                  <span className={`inline-flex max-w-full items-center gap-1 rounded-md border px-2 py-1 text-xs font-medium ${tone.className}`}>
                    <span className="shrink-0">{tone.icon}</span>
                    <span className="truncate">{adapter.status}</span>
                  </span>
                  <div className="mt-1 truncate text-[11px] text-[var(--color-text-muted)]">
                    score={runtimeAdapterEvidenceScore(adapter)}
                  </div>
                </div>
                <div className="col-span-2 min-w-0 text-xs text-[var(--color-text-muted)] lg:col-span-1">
                  <div className="truncate">{adapter.profileRuntimeEvidence}</div>
                  <div className="mt-1 truncate text-[var(--color-text-secondary)]">{adapter.fingerprintRuntimeDepth}</div>
                  {blockers.map(blocker => (
                    <div key={blocker} className="mt-1 truncate text-[var(--color-warning)]">{blocker}</div>
                  ))}
                </div>
              </div>
            )
          })}
        </div>
      </Card>
    </div>
  )
}
