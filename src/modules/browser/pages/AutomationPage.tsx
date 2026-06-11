import { useEffect, useState, useCallback } from 'react'
import { Bot, Copy, Rocket, Plus, Play, Trash2, Clock, Repeat, Zap, Pause, RefreshCw, AlertCircle } from 'lucide-react'
import { Button, Card, toast } from '../../../shared/components'
import { messageFromUnknownError } from '../../../shared/errors'
import {
  createAutomationRule,
  createSchedulerTask,
  deleteAutomationRule,
  deleteSchedulerTask,
  fetchAutomationRules,
  fetchLaunchServerInfo,
  fetchSchedulerTasks,
  runSchedulerTaskNow,
  testFireAutomationRule,
  toggleAutomationRule,
  type AutomationRuleInfo,
  type LaunchServerInfo,
  type SchedulerTaskInfo,
} from '../api'

// ─── Types ────────────────────────────────────────────────────────────────────

type TriggerType = 'cron' | 'interval' | 'event'

interface TaskTrigger {
  type: string
  cron?: string
  interval?: string
  event?: string
}

interface TaskAction {
  type: string
  target: string
  value: string
  timeout: number
}

type TaskInfo = SchedulerTaskInfo

interface TaskTemplate {
  id: string
  name: string
  description: string
  icon: React.ReactNode
  trigger: { type: TriggerType; cron?: string; interval?: string; event?: string }
  actions: { type: string; target: string; value: string; timeout: number }[]
}

// ─── Task Templates ───────────────────────────────────────────────────────────

const TASK_TEMPLATES: TaskTemplate[] = [
  {
    id: 'tpl-login',
    name: '账号登录',
    description: '自动登录小红书账号，处理验证码和安全验证',
    icon: <Zap className="h-5 w-5" />,
    trigger: { type: 'event', event: 'account:login:session-expired' },
    actions: [
      { type: 'navigate', target: 'https://www.xiaohongshu.com/login', value: '', timeout: 30000 },
      { type: 'wait', target: '.login-container', value: '', timeout: 10000 },
      { type: 'click', target: '.login-btn', value: 'phone', timeout: 5000 },
    ],
  },
  {
    id: 'tpl-maintain',
    name: '养号任务',
    description: '定时浏览推荐页、点赞互动，模拟真实用户行为',
    icon: <Clock className="h-5 w-5" />,
    trigger: { type: 'interval', interval: '30m' },
    actions: [
      { type: 'navigate', target: 'https://www.xiaohongshu.com/explore', value: '', timeout: 15000 },
      { type: 'wait', target: '.feeds-page', value: '', timeout: 10000 },
      { type: 'extract', target: '.note-item', value: 'notes', timeout: 5000 },
    ],
  },
  {
    id: 'tpl-publish',
    name: '发布笔记',
    description: '按定时计划发布图文或视频笔记',
    icon: <Rocket className="h-5 w-5" />,
    trigger: { type: 'cron', cron: 'daily@09:00' },
    actions: [
      { type: 'navigate', target: 'https://www.xiaohongshu.com/upload', value: '', timeout: 15000 },
      { type: 'click', target: '.upload-btn', value: 'image', timeout: 5000 },
    ],
  },
  {
    id: 'tpl-interact',
    name: '批量互动',
    description: '批量点赞、评论、关注目标用户',
    icon: <Repeat className="h-5 w-5" />,
    trigger: { type: 'interval', interval: '2h' },
    actions: [
      { type: 'navigate', target: 'https://www.xiaohongshu.com/explore', value: '', timeout: 15000 },
      { type: 'click', target: '.note-item:first-child', value: '', timeout: 5000 },
      { type: 'click', target: '.like-btn', value: '', timeout: 3000 },
    ],
  },
]

const TRIGGER_LABELS: Record<TriggerType, string> = {
  cron: '定时',
  interval: '间隔',
  event: '事件驱动',
}

function formatTaskTime(value?: string): string {
  if (!value) return '未运行'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString()
}

// ─── AutomationPage ───────────────────────────────────────────────────────────

const DEFAULT_LAUNCH_BASE_URL = 'http://127.0.0.1:19876'
const DEFAULT_API_AUTH: LaunchServerInfo['apiAuth'] = {
  requested: false, configured: false, enabled: false, header: 'X-Personal-Pilot-Api-Key',
}

type AutomationTabKey = 'tasks' | 'guide' | 'profiles' | 'launch' | 'logs' | 'rules'

const AUTOMATION_TABS: { key: AutomationTabKey; label: string; description: string }[] = [
  { key: 'tasks', label: '任务编排', description: '创建和管理自动化任务，支持定时、间隔和事件驱动触发。' },
  { key: 'guide', label: '接入说明', description: '先理解整体调用方式和推荐流程。' },
  { key: 'profiles', label: '配置管理', description: '集中查看实例创建、配置落库和返回结构。' },
  { key: 'launch', label: '启动调用', description: '集中查看参数化唤起和启动响应。' },
  { key: 'logs', label: '日志排障', description: '集中查看日志查询和后续排障入口。' },
  { key: 'rules', label: '响应规则', description: '配置事件驱动的自动响应规则，条件触发自动执行。' },
]

function buildAuthHeaderLine(apiAuth: LaunchServerInfo['apiAuth']): string {
  if (!apiAuth.enabled) return ''
  return `  -H "${apiAuth.header}: <your-api-key>" \\\n`
}

function buildSampleCreateRequest(baseUrl: string, apiAuth: LaunchServerInfo['apiAuth']): string {
  return `curl -X POST ${baseUrl}/api/profiles \\
  -H "Content-Type: application/json" \\
${buildAuthHeaderLine(apiAuth)}  -d '{
    "profile": {
      "profileName": "buyer-001",
      "userDataDir": "buyers/buyer-001",
      "proxyId": "proxy-us",
      "launchArgs": ["--lang=en-US"],
      "tags": ["电商", "北美"],
      "keywords": ["buyer-001", "amazon"],
      "groupId": "group-sales-us"
    },
    "launchCode": "BUYER_001"
  }'`
}

function buildSampleCreateAndLaunchRequest(baseUrl: string, apiAuth: LaunchServerInfo['apiAuth']): string {
  return `curl -X POST ${baseUrl}/api/profiles \\
  -H "Content-Type: application/json" \\
${buildAuthHeaderLine(apiAuth)}  -d '{
    "profile": {
      "profileName": "buyer-002",
      "userDataDir": "buyers/buyer-002",
      "proxyConfig": "http://user:pass@127.0.0.1:8080",
      "launchArgs": ["--disable-sync"],
      "keywords": ["buyer-002"]
    },
    "autoLaunch": true,
    "start": {
      "launchArgs": ["--window-size=1280,800"],
      "startUrls": ["https://example.com/order"],
      "skipDefaultStartUrls": true
    }
  }'`
}

function buildSampleRequest(baseUrl: string, apiAuth: LaunchServerInfo['apiAuth']): string {
  return `curl -X POST ${baseUrl}/api/launch \\
  -H "Content-Type: application/json" \\
${buildAuthHeaderLine(apiAuth)}  -d '{
    "code": "A3F9K2",
    "launchArgs": ["--window-size=1280,800", "--lang=en-US"],
    "startUrls": ["https://example.com"],
    "skipDefaultStartUrls": true
  }'`
}

const sampleCreateResponse = `{
  "ok": true,
  "created": true,
  "launched": false,
  "profileId": "550e8400-e29b-41d4-a716-446655440000",
  "profileName": "buyer-001",
  "launchCode": "BUYER_001"
}`

const sampleCreateAndLaunchResponse = `{
  "ok": true,
  "created": true,
  "launched": true,
  "profileId": "550e8400-e29b-41d4-a716-446655440001",
  "profileName": "buyer-002",
  "launchCode": "A3F9K2",
  "pid": 12345,
  "debugPort": 9222,
  "cdpUrl": "http://127.0.0.1:19876"
}`

const sampleResponse = `{
  "ok": true,
  "profileId": "550e8400-e29b-41d4-a716-446655440000",
  "profileName": "账号 A",
  "pid": 12345,
  "debugPort": 9222,
  "cdpUrl": "http://127.0.0.1:19876"
}`

function buildSampleLogsRequest(baseUrl: string, apiAuth: LaunchServerInfo['apiAuth']): string {
  if (!apiAuth.enabled) {
    return `curl ${baseUrl}/api/launch/logs?limit=20`
  }
  return `curl ${baseUrl}/api/launch/logs?limit=20 \\
  -H "${apiAuth.header}: <your-api-key>"`
}

function CopyCodeButton({ text }: { text: string }) {
  return (
    <Button size="sm" variant="secondary" onClick={() => { navigator.clipboard.writeText(text).then(() => toast.success('已复制')) }}>
      <Copy className="w-3.5 h-3.5" /> 复制
    </Button>
  )
}

function CodeBlock({ text }: { text: string }) {
  return (
    <pre className="text-xs leading-relaxed font-mono text-[var(--color-text-primary)] bg-[var(--color-bg-secondary)] border border-[var(--color-border-muted)] rounded-lg p-3 overflow-x-auto">
      {text}
    </pre>
  )
}

// ─── Rule Types & Templates ────────────────────────────────────────────────────

type RuleInfo = AutomationRuleInfo

interface RuleTemplate {
  id: string
  name: string
  description: string
  triggerEvent: string
  condition: string
  action: string
  actionParams: Record<string, unknown>
  cooldown: string
}

const ACTION_LABELS: Record<string, string> = {
  emit_event: '发射事件',
  run_task: '执行任务',
  notify: '发送通知',
}

const RULE_TEMPLATES: RuleTemplate[] = [
  {
    id: 'rpl-node-banned',
    name: '节点被封 → 自动通知',
    description: '当检测到代理节点被封时，自动发送通知提醒',
    triggerEvent: 'risk:node:banned',
    condition: '',
    action: 'notify',
    actionParams: { message: '代理节点已被封禁，请及时处理', severity: 'warning' },
    cooldown: '5m',
  },
  {
    id: 'rpl-captcha',
    name: '验证码出现 → 暂停任务',
    description: '当检测到验证码时，暂停该实例所有自动化任务',
    triggerEvent: 'risk:captcha:detected',
    condition: '',
    action: 'emit_event',
    actionParams: { event: 'automation:pause-profile-tasks' },
    cooldown: '2m',
  },
  {
    id: 'rpl-crash',
    name: '浏览器崩溃 → 自动重启',
    description: '浏览器崩溃后自动尝试重启（冷却期内最多1次）',
    triggerEvent: 'browser:instance:crashed',
    condition: '',
    action: 'emit_event',
    actionParams: { event: 'automation:request-restart' },
    cooldown: '3m',
  },
  {
    id: 'rpl-high-latency',
    name: '高延迟 → 切换节点通知',
    description: '代理延迟过高时发送通知，建议切换节点',
    triggerEvent: 'risk:proxy:high-latency',
    condition: 'latencyMs>=3000',
    action: 'notify',
    actionParams: { message: '代理延迟过高(>3s)，建议切换节点', severity: 'warning' },
    cooldown: '5m',
  },
]

// ─── Rules Tab ─────────────────────────────────────────────────────────────────

function RulesTab({ rules, loading, onAddRule, onDeleteRule, onToggleRule, onTestFire, onRefresh }: {
  rules: RuleInfo[]
  loading: boolean
  onAddRule: (tpl: RuleTemplate) => void
  onDeleteRule: (id: string) => void
  onToggleRule: (id: string, enabled: boolean) => void
  onTestFire: (id: string) => void
  onRefresh: () => void
}) {
  const [activeSub, setActiveSub] = useState<'templates' | 'list'>('templates')

  return (
    <div className="space-y-5">
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">规则模板库</h3>
            <p className="text-xs text-[var(--color-text-muted)] mt-1">一键创建常用事件响应规则，支持条件匹配和冷却控制</p>
          </div>
          <div className="flex items-center gap-2">
            <div className="flex rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-secondary)] p-0.5">
              <button
                onClick={() => setActiveSub('templates')}
                className={`rounded px-3 py-1 text-xs font-medium transition-colors ${
                  activeSub === 'templates'
                    ? 'bg-[var(--color-bg-card)] text-[var(--color-text-primary)] shadow-sm'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]'
                }`}
              >
                模板
              </button>
              <button
                onClick={() => setActiveSub('list')}
                className={`rounded px-3 py-1 text-xs font-medium transition-colors ${
                  activeSub === 'list'
                    ? 'bg-[var(--color-bg-card)] text-[var(--color-text-primary)] shadow-sm'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]'
                }`}
              >
                已创建 ({rules.length})
              </button>
            </div>
            <Button size="sm" variant="ghost" onClick={onRefresh} loading={loading}>
              <RefreshCw className="w-3.5 h-3.5" />
            </Button>
          </div>
        </div>

        {activeSub === 'templates' && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {RULE_TEMPLATES.map(tpl => (
              <div
                key={tpl.id}
                className="rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-4 hover:border-[var(--color-primary)] transition-colors cursor-pointer"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex items-start gap-3">
                    <div className="rounded-lg bg-purple-100 p-2 text-purple-600">
                      <Zap className="h-4 w-4" />
                    </div>
                    <div>
                      <h4 className="font-medium text-sm text-[var(--color-text-primary)]">{tpl.name}</h4>
                      <p className="text-xs text-[var(--color-text-muted)] mt-1">{tpl.description}</p>
                      <div className="flex items-center gap-2 mt-2">
                        <span className="inline-flex items-center rounded-full border border-[var(--color-border-muted)] px-2 py-0.5 text-[10px] text-[var(--color-text-muted)] font-mono">
                          {tpl.triggerEvent}
                        </span>
                        <span className="text-xs text-[var(--color-text-muted)]">
                          动作: {ACTION_LABELS[tpl.action] || tpl.action}
                        </span>
                        <span className="text-xs text-[var(--color-text-muted)]">
                          冷却: {tpl.cooldown}
                        </span>
                      </div>
                    </div>
                  </div>
                  <Button size="sm" onClick={() => onAddRule(tpl)}>
                    <Plus className="w-3.5 h-3.5" /> 创建
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}

        {activeSub === 'list' && (
          <>
            {rules.length === 0 ? (
              <div className="text-center py-12 text-[var(--color-text-muted)]">
                <Zap className="h-8 w-8 mx-auto mb-2 opacity-40" />
                <p className="text-sm">暂无规则</p>
                <p className="text-xs mt-1">从模板库创建你的第一个自动响应规则</p>
              </div>
            ) : (
              <div className="space-y-2">
                {rules.map(rule => (
                  <div
                    key={rule.id}
                    className="flex items-center justify-between rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-3"
                  >
                    <div className="flex items-center gap-3">
                      <div className={`flex h-8 w-8 items-center justify-center rounded-lg ${
                        rule.enabled ? 'bg-purple-100 text-purple-600' : 'bg-gray-100 text-gray-400'
                      }`}>
                        <Zap className="h-4 w-4" />
                      </div>
                      <div>
                        <h4 className="text-sm font-medium text-[var(--color-text-primary)]">{rule.name}</h4>
                        <div className="flex items-center gap-2 mt-0.5">
                          <span className="inline-flex items-center rounded-full bg-purple-100 px-1.5 py-0.5 text-[10px] font-medium text-purple-700 font-mono">
                            {rule.triggerEvent}
                          </span>
                          <span className="text-xs text-[var(--color-text-muted)]">
                            {ACTION_LABELS[rule.action] || rule.action}
                            {rule.condition && ` · ${rule.condition}`}
                            {' · '}冷却 {rule.cooldown}
                          </span>
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-1">
                      <button
                        onClick={() => onToggleRule(rule.id, !rule.enabled)}
                        className={`rounded px-2 py-1 text-xs font-medium transition-colors ${
                          rule.enabled
                            ? 'bg-purple-100 text-purple-700 hover:bg-purple-200'
                            : 'bg-gray-100 text-gray-500 hover:bg-gray-200'
                        }`}
                      >
                        {rule.enabled ? '已启用' : '已禁用'}
                      </button>
                      <Button size="sm" variant="ghost" onClick={() => onTestFire(rule.id)} title="测试触发">
                        <Play className="w-3.5 h-3.5 text-blue-500" />
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => onDeleteRule(rule.id)}>
                        <Trash2 className="w-3.5 h-3.5 text-red-500" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      </Card>
    </div>
  )
}

// ─── Tasks Tab ────────────────────────────────────────────────────────────────

function TasksTab({ tasks, loading, onAddTask, onDeleteTask, onRunNow, onRefresh }: {
  tasks: TaskInfo[]
  loading: boolean
  onAddTask: (tpl: TaskTemplate) => void
  onDeleteTask: (id: string) => void
  onRunNow: (id: string) => void
  onRefresh: () => void
}) {
  const [activeSub, setActiveSub] = useState<'templates' | 'list'>('templates')

  return (
    <div className="space-y-5">
      {/* Template library */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <h3 className="text-sm font-semibold text-[var(--color-text-primary)]">任务模板库</h3>
            <p className="text-xs text-[var(--color-text-muted)] mt-1">一键创建常用自动化任务，支持小红书账号运营全流程</p>
          </div>
          <div className="flex items-center gap-2">
            <div className="flex rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-secondary)] p-0.5">
              <button
                onClick={() => setActiveSub('templates')}
                className={`rounded px-3 py-1 text-xs font-medium transition-colors ${
                  activeSub === 'templates'
                    ? 'bg-[var(--color-bg-card)] text-[var(--color-text-primary)] shadow-sm'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]'
                }`}
              >
                模板
              </button>
              <button
                onClick={() => setActiveSub('list')}
                className={`rounded px-3 py-1 text-xs font-medium transition-colors ${
                  activeSub === 'list'
                    ? 'bg-[var(--color-bg-card)] text-[var(--color-text-primary)] shadow-sm'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]'
                }`}
              >
                已创建 ({tasks.length})
              </button>
            </div>
            <Button size="sm" variant="ghost" onClick={onRefresh} loading={loading}>
              <RefreshCw className="w-3.5 h-3.5" />
            </Button>
          </div>
        </div>

        {activeSub === 'templates' && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {TASK_TEMPLATES.map(tpl => (
              <div
                key={tpl.id}
                className="rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-4 hover:border-[var(--color-primary)] transition-colors cursor-pointer"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex items-start gap-3">
                    <div className="rounded-lg bg-[var(--color-accent-muted)] p-2 text-[var(--color-accent)]">
                      {tpl.icon}
                    </div>
                    <div>
                      <h4 className="font-medium text-sm text-[var(--color-text-primary)]">{tpl.name}</h4>
                      <p className="text-xs text-[var(--color-text-muted)] mt-1">{tpl.description}</p>
                      <div className="flex items-center gap-2 mt-2">
                        <span className="inline-flex items-center rounded-full border border-[var(--color-border-muted)] px-2 py-0.5 text-xs text-[var(--color-text-muted)]">
                          {TRIGGER_LABELS[tpl.trigger.type]}
                        </span>
                        <span className="text-xs text-[var(--color-text-muted)]">
                          {tpl.trigger.interval && `每 ${tpl.trigger.interval}`}
                          {tpl.trigger.cron && tpl.trigger.cron}
                          {tpl.trigger.event && `监听: ${tpl.trigger.event}`}
                        </span>
                      </div>
                    </div>
                  </div>
                  <Button size="sm" onClick={() => onAddTask(tpl)}>
                    <Plus className="w-3.5 h-3.5" /> 创建
                  </Button>
                </div>
              </div>
            ))}
          </div>
        )}

        {activeSub === 'list' && (
          <>
            {tasks.length === 0 ? (
              <div className="text-center py-12 text-[var(--color-text-muted)]">
                <Bot className="h-8 w-8 mx-auto mb-2 opacity-40" />
                <p className="text-sm">暂无任务</p>
                <p className="text-xs mt-1">从模板库创建你的第一个自动化任务</p>
              </div>
            ) : (
              <div className="space-y-2">
                {tasks.map(task => (
                  <div
                    key={task.id}
                    className="flex items-center justify-between rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-3"
                  >
                    <div className="flex items-center gap-3">
                      <div className={`flex h-8 w-8 items-center justify-center rounded-lg ${
                        task.status === 'running' ? 'bg-blue-100 text-blue-600 animate-pulse' :
                        task.status === 'failed' ? 'bg-red-100 text-red-600' :
                        task.status === 'done' ? 'bg-emerald-100 text-emerald-600' :
                        task.enabled ? 'bg-emerald-100 text-emerald-600' : 'bg-gray-100 text-gray-400'
                      }`}>
                        {task.status === 'running' ? <RefreshCw className="h-4 w-4" /> :
                         task.status === 'failed' ? <AlertCircle className="h-4 w-4" /> :
                         task.enabled ? <Play className="h-4 w-4" /> : <Pause className="h-4 w-4" />}
                      </div>
                      <div>
                        <h4 className="text-sm font-medium text-[var(--color-text-primary)]">{task.name}</h4>
                        <div className="flex items-center gap-2 mt-0.5">
                          <span className={`inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] font-medium ${
                            task.status === 'running' ? 'bg-blue-100 text-blue-700' :
                            task.status === 'failed' ? 'bg-red-100 text-red-700' :
                            task.status === 'done' ? 'bg-emerald-100 text-emerald-700' :
                            'bg-gray-100 text-gray-600'
                          }`}>
                            {task.status || 'idle'}
                          </span>
                          <span className="text-xs text-[var(--color-text-muted)]">
                            {TRIGGER_LABELS[task.trigger.type as TriggerType]}
                            {task.trigger.interval && ` · 每${task.trigger.interval}`}
                            {task.trigger.event && ` · ${task.trigger.event}`}
                            {` · ${task.actions.length} steps`}
                          </span>
                          {task.profileId && (
                            <span className="text-xs text-[var(--color-text-muted)]">
                              · {task.profileId.slice(0, 8)}...
                            </span>
                          )}
                        </div>
                        <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-[var(--color-text-muted)]">
                          <span>上次运行: {formatTaskTime(task.lastRunAt)}</span>
                          {task.retryCount > 0 && <span>重试: {task.retryCount}/{task.maxRetries}</span>}
                          {task.lastError && <span className="max-w-[360px] truncate text-[var(--color-error)]">错误: {task.lastError}</span>}
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-1">
                      <Button size="sm" variant="ghost" onClick={() => onRunNow(task.id)} title="立即执行">
                        <Play className="w-3.5 h-3.5 text-blue-500" />
                      </Button>
                      <Button size="sm" variant="ghost" onClick={() => onDeleteTask(task.id)}>
                        <Trash2 className="w-3.5 h-3.5 text-red-500" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      </Card>

    </div>
  )
}

// ─── Main Component ───────────────────────────────────────────────────────────

export function AutomationPage() {
  const [launchBaseUrl, setLaunchBaseUrl] = useState(DEFAULT_LAUNCH_BASE_URL)
  const [launchServerReady, setLaunchServerReady] = useState(false)
  const [apiAuth, setApiAuth] = useState<LaunchServerInfo['apiAuth']>(DEFAULT_API_AUTH)
  const [activeTab, setActiveTab] = useState<AutomationTabKey>('tasks')
  const [tasks, setTasks] = useState<TaskInfo[]>([])
  const [tasksLoading, setTasksLoading] = useState(false)
  const [rules, setRules] = useState<RuleInfo[]>([])
  const [rulesLoading, setRulesLoading] = useState(false)

  useEffect(() => {
    let disposed = false
    void fetchLaunchServerInfo()
      .then((info) => {
        if (disposed) return
        if (info.baseUrl) setLaunchBaseUrl(info.baseUrl)
        setLaunchServerReady(info.ready)
        setApiAuth(info.apiAuth)
      })
      .catch(() => {})
    return () => { disposed = true }
  }, [])

  const refreshTasks = useCallback(async () => {
    setTasksLoading(true)
    try {
      const list = await fetchSchedulerTasks()
      setTasks(list || [])
    } catch {
      // backend may not be ready
    } finally {
      setTasksLoading(false)
    }
  }, [])

  useEffect(() => {
    void refreshTasks()
  }, [refreshTasks])

  const handleAddTask = async (tpl: TaskTemplate) => {
    try {
      await createSchedulerTask({
        name: tpl.name,
        trigger: {
          type: tpl.trigger.type,
          cron: tpl.trigger.cron || '',
          interval: tpl.trigger.interval || '',
          event: tpl.trigger.event || '',
        },
        actions: tpl.actions.map(a => ({
          type: a.type,
          target: a.target,
          value: a.value,
          timeout: a.timeout,
        })),
        maxRetries: 3,
        retryDelay: '10s',
        dependsOn: [],
        profileId: '',
        enabled: true,
      })
      toast.success(`任务「${tpl.name}」已创建`)
      await refreshTasks()
    } catch (error: unknown) {
      toast.error(messageFromUnknownError(error, '创建任务失败'))
    }
  }

  const handleDeleteTask = async (id: string) => {
    try {
      await deleteSchedulerTask(id)
      toast.success('任务已删除')
      await refreshTasks()
    } catch (error: unknown) {
      toast.error(messageFromUnknownError(error, '删除任务失败'))
    }
  }

  const handleRunNow = async (id: string) => {
    try {
      await runSchedulerTaskNow(id)
      toast.success('任务已触发执行')
      await refreshTasks()
    } catch (error: unknown) {
      toast.error(messageFromUnknownError(error, '任务触发失败'))
    }
  }

  // ─── Rules handlers ─────────────────────────────────────────────────────────

  const refreshRules = useCallback(async () => {
    setRulesLoading(true)
    try {
      const list = await fetchAutomationRules()
      setRules(list || [])
    } catch {
      // backend may not be ready
    } finally {
      setRulesLoading(false)
    }
  }, [])

  useEffect(() => {
    void refreshRules()
  }, [refreshRules])

  const handleAddRule = async (tpl: RuleTemplate) => {
    try {
      await createAutomationRule({
        name: tpl.name,
        triggerEvent: tpl.triggerEvent,
        condition: tpl.condition,
        action: tpl.action,
        actionParams: tpl.actionParams,
        cooldown: tpl.cooldown,
        enabled: true,
      })
      toast.success(`规则「${tpl.name}」已创建`)
      await refreshRules()
    } catch (error: unknown) {
      toast.error(messageFromUnknownError(error, '创建规则失败'))
    }
  }

  const handleDeleteRule = async (id: string) => {
    try {
      await deleteAutomationRule(id)
      toast.success('规则已删除')
      await refreshRules()
    } catch (error: unknown) {
      toast.error(messageFromUnknownError(error, '删除规则失败'))
    }
  }

  const handleToggleRule = async (id: string, enabled: boolean) => {
    try {
      await toggleAutomationRule(id, enabled)
      await refreshRules()
    } catch (error: unknown) {
      toast.error(messageFromUnknownError(error, `${enabled ? '启用' : '禁用'}规则失败`))
    }
  }

  const handleTestFire = async (id: string) => {
    try {
      await testFireAutomationRule(id)
      toast.success('测试事件已发送')
    } catch (error: unknown) {
      toast.error(messageFromUnknownError(error, '测试触发失败'))
    }
  }

  const sampleCreateRequest = buildSampleCreateRequest(launchBaseUrl, apiAuth)
  const sampleCreateAndLaunchRequest = buildSampleCreateAndLaunchRequest(launchBaseUrl, apiAuth)
  const sampleRequest = buildSampleRequest(launchBaseUrl, apiAuth)
  const sampleLogsRequest = buildSampleLogsRequest(launchBaseUrl, apiAuth)
  const activeTabMeta = AUTOMATION_TABS.find(tab => tab.key === activeTab) || AUTOMATION_TABS[0]

  return (
    <div className="space-y-5 animate-fade-in">
      <Card>
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="inline-flex items-center gap-2 px-2.5 py-1 rounded-full bg-[var(--color-accent-muted)] text-[var(--color-accent)] text-xs font-medium mb-3">
              <Bot className="w-3.5 h-3.5" /> 自动化接口（实验）
            </div>
            <h1 className="text-xl font-semibold text-[var(--color-text-primary)]">
              {activeTab === 'tasks' ? '任务编排与自动化' : '外部脚本配置与唤起接口'}
            </h1>
            <p className="text-sm text-[var(--color-text-secondary)] mt-2">
              {activeTab === 'tasks'
                ? '创建定时、间隔和事件驱动的自动化任务，管理小红书多账号运营全流程。'
                : '已支持通过本地 HTTP + JSON 协议管理实例配置并唤起实例，并通过同一个固定端口暴露 CDP 入口。'}
            </p>
            {activeTab !== 'tasks' && (
              <p className="text-xs text-[var(--color-text-muted)] mt-2">
                当前 Launch 地址：<code>{launchBaseUrl}</code>
                {!launchServerReady ? '（服务启动后会自动刷新）' : ''}
              </p>
            )}
          </div>
        </div>
      </Card>

      <div className="space-y-3">
        <div className="overflow-x-auto">
          <div className="flex min-w-max border-b border-[var(--color-border)]">
            {AUTOMATION_TABS.map(tab => (
              <button
                key={tab.key}
                onClick={() => setActiveTab(tab.key)}
                className={[
                  'px-4 py-2 text-sm font-medium transition-colors whitespace-nowrap',
                  activeTab === tab.key
                    ? 'border-b-2 border-[var(--color-primary)] text-[var(--color-primary)]'
                    : 'text-[var(--color-text-muted)] hover:text-[var(--color-text-secondary)]',
                ].join(' ')}
              >
                {tab.label}
              </button>
            ))}
          </div>
        </div>

        <Card className="bg-[var(--color-bg-surface)]/70">
          <div className="flex items-start gap-3">
            <div className="rounded-lg bg-[var(--color-accent-muted)] p-2 text-[var(--color-accent)]">
              <Bot className="w-4 h-4" />
            </div>
            <div>
              <p className="text-sm font-medium text-[var(--color-text-primary)]">{activeTabMeta.label}</p>
              <p className="text-sm text-[var(--color-text-secondary)] mt-1">{activeTabMeta.description}</p>
            </div>
          </div>
        </Card>
      </div>

      {activeTab === 'tasks' && (
        <TasksTab tasks={tasks} loading={tasksLoading} onAddTask={handleAddTask} onDeleteTask={handleDeleteTask} onRunNow={handleRunNow} onRefresh={refreshTasks} />
      )}

      {activeTab === 'rules' && (
        <RulesTab rules={rules} loading={rulesLoading} onAddRule={handleAddRule} onDeleteRule={handleDeleteRule} onToggleRule={handleToggleRule} onTestFire={handleTestFire} onRefresh={refreshRules} />
      )}

      {activeTab === 'guide' && (
        <div className="space-y-5">
          <Card title="推荐接入顺序" subtitle="稳定性优先时，建议把创建、启动、接管拆开处理">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3 text-sm">
              <div className="rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-4">
                <p className="text-xs uppercase tracking-[0.14em] text-[var(--color-text-muted)]">Step 1</p>
                <p className="mt-2 font-medium text-[var(--color-text-primary)]">先创建配置</p>
                <p className="mt-1 text-[var(--color-text-secondary)]">先拿到 <code>profileId</code> 和 <code>launchCode</code>，把落库和启动拆开。</p>
              </div>
              <div className="rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-4">
                <p className="text-xs uppercase tracking-[0.14em] text-[var(--color-text-muted)]">Step 2</p>
                <p className="mt-2 font-medium text-[var(--color-text-primary)]">再调用启动</p>
                <p className="mt-1 text-[var(--color-text-secondary)]">启动失败时更容易单独重试，也更容易记录调度结果。</p>
              </div>
              <div className="rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-4">
                <p className="text-xs uppercase tracking-[0.14em] text-[var(--color-text-muted)]">Step 3</p>
                <p className="mt-2 font-medium text-[var(--color-text-primary)]">最后接 CDP</p>
                <p className="mt-1 text-[var(--color-text-secondary)]">统一使用响应里的 <code>cdpUrl</code>，不要自己拼内部调试端口。</p>
              </div>
            </div>
          </Card>

          <Card title="触发创建的方式" subtitle="推荐按用途选择 /api/profiles 的三种调用模式">
            <div className="text-sm text-[var(--color-text-secondary)] space-y-2">
              <p><code>仅创建配置</code>: 传 <code>profile</code>，不传 <code>autoLaunch</code>，接口只落库不启动浏览器。</p>
              <p><code>创建并立即启动</code>: 传 <code>profile</code> + <code>autoLaunch=true</code>，可再用 <code>start</code> 追加本次启动参数。</p>
              <p><code>先创建后单独唤起</code>: 先调用 <code>POST /api/profiles</code> 取得 <code>profileId</code> / <code>launchCode</code>，再调用 <code>POST /api/launch</code> 或 <code>GET /api/launch/{'{code}'}</code>。</p>
              <p>稳定性优先时，推荐默认走"先创建后单独唤起"，这样创建和启动失败可以分开处理、分开重试。</p>
            </div>
          </Card>
        </div>
      )}

      {activeTab === 'profiles' && (
        <div className="space-y-5">
          <Card title="仅创建实例配置" subtitle="POST /api/profiles" actions={<CopyCodeButton text={sampleCreateRequest} />}>
            <CodeBlock text={sampleCreateRequest} />
            <div className="mt-3 text-sm text-[var(--color-text-secondary)] space-y-1">
              <p><code>profile</code>: 持久化的实例配置，支持实例名、代理、标签、关键字、分组、默认启动参数等字段。</p>
              <p><code>launchCode</code>: 可选的自定义启动码；如果不传，系统会自动生成。</p>
              <p><code>autoLaunch</code> + <code>start</code>: 可选，表示创建后立即启动，并附带一次性启动参数。</p>
              <p>同一资源还支持 <code>GET /api/profiles</code>、<code>GET/PUT/DELETE /api/profiles/{'{profileId}'}</code>，用于后续查询、更新、删除。</p>
            </div>
          </Card>
          <Card title="创建响应" subtitle="创建成功后返回 profileId + launchCode" actions={<CopyCodeButton text={sampleCreateResponse} />}>
            <CodeBlock text={sampleCreateResponse} />
          </Card>
          <Card title="创建并立即启动" subtitle="POST /api/profiles + autoLaunch=true" actions={<CopyCodeButton text={sampleCreateAndLaunchRequest} />}>
            <CodeBlock text={sampleCreateAndLaunchRequest} />
            <div className="mt-3 text-sm text-[var(--color-text-secondary)] space-y-1">
              <p><code>autoLaunch=true</code>: 当前请求在创建完成后会直接启动实例。</p>
              <p><code>start</code>: 只作用于本次启动，不会写回实例持久化配置。</p>
              <p>如果创建已经成功但自动启动失败，响应里仍会标出 <code>created=true</code>，便于脚本分支处理。</p>
            </div>
          </Card>
          <Card title="创建并启动响应" subtitle="返回 created + launched + cdpUrl" actions={<CopyCodeButton text={sampleCreateAndLaunchResponse} />}>
            <CodeBlock text={sampleCreateAndLaunchResponse} />
          </Card>
        </div>
      )}

      {activeTab === 'launch' && (
        <div className="space-y-5">
          <Card title="启动接口使用建议" subtitle="把选择目标、附加参数和页面打开策略放在一次请求里">
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm text-[var(--color-text-secondary)]">
              <div className="rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-4">
                <p className="font-medium text-[var(--color-text-primary)]">目标匹配</p>
                <p className="mt-1"><code>code</code> 用于精确唤起；<code>key</code> 适合关键字检索和批量调度。</p>
              </div>
              <div className="rounded-lg border border-[var(--color-border-muted)] bg-[var(--color-bg-secondary)] p-4">
                <p className="font-medium text-[var(--color-text-primary)]">接管方式</p>
                <p className="mt-1">外部统一使用固定 <code>cdpUrl</code> 连接，不直接依赖内部实际 <code>debugPort</code>。</p>
              </div>
            </div>
          </Card>
          <Card title="参数化唤起接口" subtitle="POST /api/launch" actions={<CopyCodeButton text={sampleRequest} />}>
            <CodeBlock text={sampleRequest} />
            <div className="mt-3 text-sm text-[var(--color-text-secondary)] space-y-1">
              <p><code>code</code> / <code>key</code>: 二选一即可；<code>code</code> 按 LaunchCode 精确匹配，<code>key</code> 按实例关键字优先精确、未命中时再模糊匹配。</p>
              <p><code>matchMode</code>: 多命中时的行为控制，支持 <code>unique</code> / <code>first</code> / <code>all</code>；传 <code>key</code> 时默认 <code>first</code>。</p>
              <p><code>launchArgs</code>: 仅本次启动附加的 Chrome 启动参数。</p>
              <p><code>startUrls</code>: 启动后打开的页面列表。</p>
              <p><code>skipDefaultStartUrls</code>: 设为 <code>true</code> 时不追加系统默认起始页。</p>
            </div>
          </Card>
          <Card title="启动响应" subtitle="成功返回 pid + cdpUrl；外部统一使用固定端口接 CDP" actions={<CopyCodeButton text={sampleResponse} />}>
            <CodeBlock text={sampleResponse} />
          </Card>
        </div>
      )}

      {activeTab === 'logs' && (
        <div className="space-y-5">
          <Card title="调用记录" subtitle="GET /api/launch/logs?limit=20" actions={<CopyCodeButton text={sampleLogsRequest} />}>
            <CodeBlock text={sampleLogsRequest} />
            <p className="mt-3 text-sm text-[var(--color-text-secondary)]">
              可查询最近接口调用记录（默认 50 条，最大 200 条），用于排查自动化脚本调用问题。
            </p>
          </Card>
          <Card title="排障提示" subtitle="先看最近调用，再看实例是否已经完成后台接管">
            <div className="text-sm text-[var(--color-text-secondary)] space-y-2">
              <p>如果返回里已经有 <code>pid</code>，但 <code>debugReady=false</code>，说明窗口已拉起，只是 CDP 还在后台附着。</p>
              <p>如果接口直接返回错误，优先查看最近日志和实例最近错误，再决定是否重试。</p>
              <p>排查自动化脚本时，建议把请求参数、响应体和调用日志一起保存，方便复现。</p>
            </div>
          </Card>
          <Card>
            <div className="flex items-start gap-2 text-sm text-[var(--color-text-secondary)]">
              <Rocket className="w-4 h-4 mt-0.5 text-[var(--color-accent)]" />
              <p>当前这部分接口已经可用，后续会继续补充自动化任务编排、模板脚本、连接状态监控等增强能力。</p>
            </div>
          </Card>
        </div>
      )}
    </div>
  )
}
