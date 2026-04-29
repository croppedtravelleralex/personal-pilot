import { useEffect, useState } from 'react'
import { Monitor, Play, Shield, Cpu, ArrowRight, Globe, Settings } from 'lucide-react'
import { Link } from 'react-router-dom'
import { Card } from '../../shared/components'
import { fetchDashboardStats, reloadConfig } from './api'
import type { DashboardStats } from './types'

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

const UNLIMITED = Number.POSITIVE_INFINITY

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

  useEffect(() => {
    void load()
  }, [])

  const load = async () => {
    setLoading(true)
    try {
      await reloadConfig()
      setStats(await fetchDashboardStats())
    } finally {
      setLoading(false)
    }
  }

  const v = (n: number) => (loading ? '-' : n.toString())

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
    </div>
  )
}
