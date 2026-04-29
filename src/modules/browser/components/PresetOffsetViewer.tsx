import { useState } from 'react'
import { ChevronDown, ChevronUp, MousePointer, Keyboard, ScrollText, Gauge, Repeat, SlidersHorizontal, MoveHorizontal, Timer, Target, Clock, ArrowDownUp, AlertTriangle } from 'lucide-react'
import { BEHAVIOR_PRESETS, type BehaviorPresetFull } from '../data/behaviorPresets'

export function PresetOffsetViewer() {
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const toggle = (id: string) => {
    setExpandedId(prev => (prev === id ? null : id))
  }

  return (
    <div className="space-y-4">
      <h4 className="text-sm font-semibold text-[var(--color-text)] flex items-center gap-2">
        <Gauge className="w-4 h-4" />
        行为模板偏移配置
      </h4>
      <p className="text-xs text-[var(--color-text-muted)]">
        展开模板查看鼠标、键盘、滚动、空闲四个维度的完整偏移参数和相关性
      </p>

      <div className="space-y-2">
        {BEHAVIOR_PRESETS.map(preset => (
          <PresetCard
            key={preset.id}
            preset={preset}
            expanded={expandedId === preset.id}
            onToggle={() => toggle(preset.id)}
          />
        ))}
      </div>

      {/* VariationConfig replay offset visualization */}
      <VariationConfigViewer />

      {/* Humanize offset visualization */}
      <HumanizeOffsetViewer />

      {/* Polling order diagram */}
      <div className="border border-[var(--color-border)] rounded-lg p-4">
        <h5 className="text-xs font-semibold text-[var(--color-text)] mb-3 flex items-center gap-2">
          <Repeat className="w-3.5 h-3.5" />
          行为引擎轮询顺序
        </h5>
        <PollingOrderDiagram />
      </div>
    </div>
  )
}

function PresetCard({ preset, expanded, onToggle }: {
  preset: BehaviorPresetFull
  expanded: boolean
  onToggle: () => void
}) {
  return (
    <div className="border border-[var(--color-border)] rounded-lg overflow-hidden transition-all">
      <button
        type="button"
        className="w-full flex items-center justify-between px-4 py-3 hover:bg-[var(--color-bg-hover)] transition-colors text-left"
        onClick={onToggle}
      >
        <div>
          <div className="text-sm font-medium text-[var(--color-text)]">{preset.name}</div>
          <div className="text-xs text-[var(--color-text-muted)] mt-0.5">{preset.description}</div>
        </div>
        <div className="flex items-center gap-3">
          <div className="hidden sm:flex gap-1.5">
            <MiniStatusBadge enabled={preset.mouse.enabled} label="鼠标" />
            <MiniStatusBadge enabled={preset.keyboard.enabled} label="键盘" />
            <MiniStatusBadge enabled={preset.scroll.enabled} label="滚动" />
          </div>
          {expanded ? <ChevronUp className="w-4 h-4 text-[var(--color-text-muted)]" /> : <ChevronDown className="w-4 h-4 text-[var(--color-text-muted)]" />}
        </div>
      </button>

      {expanded && (
        <div className="px-4 pb-4 border-t border-[var(--color-border)] pt-3 space-y-4">
          {/* Mouse offset */}
          <OffsetSection icon={<MousePointer className="w-3.5 h-3.5" />} title="鼠标偏移" enabled={preset.mouse.enabled}>
            <div className="grid grid-cols-2 gap-3">
              <OffsetMetric label="速度分布" value={`${preset.mouse.speedMean} ± ${preset.mouse.speedStdDev} px/s`} />
              <OffsetMetric label="曲线风格" value={preset.mouse.curveStyle} />
              <OffsetMetric label="位置抖动" value={`${preset.mouse.jitterPx}px`} />
              <OffsetMetric label="空闲间隔" value={preset.mouse.idleMoveInterval} />
              <OffsetMetric label="暂停概率" value={`${(preset.mouse.pauseProb * 100).toFixed(0)}%`} />
              <OffsetMetric label="最大暂停" value={`${preset.mouse.pauseMaxMs}ms`} />
            </div>
            {/* Speed distribution mini chart */}
            <GaussianChart
              mean={preset.mouse.speedMean}
              stddev={preset.mouse.speedStdDev}
              label="鼠标速度分布"
              unit="px/s"
            />
          </OffsetSection>

          {/* Keyboard offset */}
          <OffsetSection icon={<Keyboard className="w-3.5 h-3.5" />} title="键盘偏移" enabled={preset.keyboard.enabled}>
            <div className="grid grid-cols-2 gap-3">
              <OffsetMetric label="基础延迟" value={`${preset.keyboard.baseDelayMs} ± ${preset.keyboard.delayStdDevMs}ms`} />
              <OffsetMetric label="突发概率" value={`${(preset.keyboard.burstProb * 100).toFixed(1)}% (${preset.keyboard.burstKeys}键)`} />
              <OffsetMetric label="误触概率" value={`${(preset.keyboard.typoProb * 100).toFixed(1)}%`} />
              <OffsetMetric label="双字母组延迟" value={preset.keyboard.bigramDelays ? '启用' : '禁用'} />
            </div>
            <GaussianChart
              mean={preset.keyboard.baseDelayMs}
              stddev={preset.keyboard.delayStdDevMs}
              label="键盘延迟分布"
              unit="ms"
            />
          </OffsetSection>

          {/* Scroll offset */}
          <OffsetSection icon={<ScrollText className="w-3.5 h-3.5" />} title="滚动偏移" enabled={preset.scroll.enabled}>
            <div className="grid grid-cols-2 gap-3">
              <OffsetMetric label="步长" value={`${preset.scroll.scrollStepPx} ± ${preset.scroll.scrollStepStdDev}px`} />
              <OffsetMetric label="暂停间隔" value={`${preset.scroll.pauseBetweenMs} ± ${preset.scroll.pauseStdDevMs}ms`} />
              <OffsetMetric label="空闲触发" value={`${(preset.scroll.idleScrollProb * 100).toFixed(0)}%`} />
              <OffsetMetric label="过冲概率" value={`${(preset.scroll.overscrollProb * 100).toFixed(0)}%`} />
              <OffsetMetric label="反向概率" value={`${(preset.scroll.reverseProb * 100).toFixed(0)}%`} />
            </div>
          </OffsetSection>

          {/* Idle behavior */}
          <OffsetSection icon={<Gauge className="w-3.5 h-3.5" />} title="空闲行为" enabled={true}>
            <div className="flex gap-4">
              <IdleToggle label="鼠标游荡" active={preset.idle.mouseWander} />
              <IdleToggle label="标签切换" active={preset.idle.tabSwitch} />
              <IdleToggle label="焦点丢失" active={preset.idle.focusLoss} />
            </div>
          </OffsetSection>
        </div>
      )}
    </div>
  )
}

function MiniStatusBadge({ enabled, label }: { enabled: boolean; label: string }) {
  return (
    <span className={`text-[10px] px-1.5 py-0.5 rounded ${enabled ? 'bg-green-100 text-green-700' : 'bg-gray-100 text-gray-400'}`}>
      {label}
    </span>
  )
}

function OffsetSection({ icon, title, enabled, children }: {
  icon: React.ReactNode
  title: string
  enabled: boolean
  children: React.ReactNode
}) {
  return (
    <div className={`space-y-2 ${!enabled ? 'opacity-40' : ''}`}>
      <div className="flex items-center gap-2">
        <span className="text-[var(--color-text-muted)]">{icon}</span>
        <span className="text-xs font-medium text-[var(--color-text)]">{title}</span>
        {!enabled && <span className="text-[10px] text-gray-400">(已禁用)</span>}
      </div>
      {children}
    </div>
  )
}

function OffsetMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-[var(--color-bg-subtle)] rounded px-2.5 py-1.5">
      <div className="text-[10px] text-[var(--color-text-muted)]">{label}</div>
      <div className="text-xs font-mono font-medium text-[var(--color-text)]">{value}</div>
    </div>
  )
}

function IdleToggle({ label, active }: { label: string; active: boolean }) {
  return (
    <div className={`flex items-center gap-2 px-3 py-2 rounded-lg text-xs ${active ? 'bg-green-50 text-green-700' : 'bg-gray-50 text-gray-400'}`}>
      <span className={`w-2 h-2 rounded-full ${active ? 'bg-green-500' : 'bg-gray-300'}`} />
      {label}
    </div>
  )
}

// SVG Gaussian distribution chart
function GaussianChart({ mean, stddev, label, unit }: {
  mean: number
  stddev: number
  label: string
  unit: string
}) {
  const w = 240
  const h = 70
  const pad = { top: 8, right: 16, bottom: 18, left: 8 }
  const cw = w - pad.left - pad.right
  const ch = h - pad.top - pad.bottom

  const points: [number, number][] = []
  const xMin = Math.max(0, mean - 3.5 * stddev)
  const xMax = mean + 3.5 * stddev
  for (let i = 0; i <= cw; i++) {
    const x = xMin + (xMax - xMin) * (i / cw)
    const z = (x - mean) / stddev
    const y = Math.exp(-0.5 * z * z) / (stddev * Math.sqrt(2 * Math.PI))
    points.push([i, y])
  }
  const yMax = Math.max(...points.map(p => p[1]))

  const pathD = points.map((p, i) => {
    const sx = pad.left + p[0]
    const sy = pad.top + ch - (p[1] / yMax) * ch
    return `${i === 0 ? 'M' : 'L'}${sx.toFixed(1)},${sy.toFixed(1)}`
  }).join(' ')

  // Mean line
  const mx = pad.left + ((mean - xMin) / (xMax - xMin)) * cw

  return (
    <div className="text-xs">
      <div className="text-[10px] text-[var(--color-text-muted)] mb-1">{label} ({unit})</div>
      <svg viewBox={`0 0 ${w} ${h}`} className="w-full max-w-[240px]" style={{ background: 'var(--color-bg-subtle)', borderRadius: 6 }}>
        {/* Grid lines */}
        <line x1={pad.left} y1={pad.top + ch} x2={pad.left + cw} y2={pad.top + ch} stroke="var(--color-border)" strokeWidth={0.5} />
        {/* Distribution curve */}
        <path d={pathD} fill="none" stroke="var(--color-accent)" strokeWidth={1.5} opacity={0.8} />
        {/* Mean line */}
        <line x1={mx} y1={pad.top} x2={mx} y2={pad.top + ch} stroke="var(--color-accent)" strokeWidth={1} strokeDasharray="3 2" opacity={0.6} />
        {/* Labels */}
        <text x={w / 2} y={h - 3} textAnchor="middle" fill="var(--color-text-muted)" fontSize={9}>
          μ={mean.toFixed(0)} σ={stddev.toFixed(0)}
        </text>
      </svg>
    </div>
  )
}

// Polling order visualization
function PollingOrderDiagram() {
  const steps = [
    { label: '空闲检测', desc: '检查距上次操作的时间', color: 'bg-slate-400' },
    { label: '鼠标移动', desc: '生成贝塞尔路径 + 位置抖动', color: 'bg-blue-400' },
    { label: '键盘输入', desc: '随机文本 + 延迟变化 + 误触', color: 'bg-purple-400' },
    { label: '滚动行为', desc: '过冲-回正模式 + 暂停', color: 'bg-orange-400' },
    { label: '等待周期', desc: '按画像设定的间隔等待', color: 'bg-gray-400' },
  ]

  return (
    <div className="flex items-center flex-wrap gap-1.5">
      {steps.map((step, i) => (
        <div key={i} className="flex items-center gap-1.5">
          <div className="flex flex-col items-center">
            <div className={`${step.color} text-white text-[10px] font-medium px-2.5 py-1.5 rounded-lg whitespace-nowrap`}>
              {step.label}
            </div>
            <div className="text-[10px] text-[var(--color-text-muted)] mt-0.5 text-center max-w-[80px] leading-tight">
              {step.desc}
            </div>
          </div>
          {i < steps.length - 1 && (
            <div className="text-[var(--color-text-muted)] text-lg font-light shrink-0">→</div>
          )}
        </div>
      ))}
      {/* Loop back arrow */}
      <div className="flex items-center gap-1.5 ml-1">
        <div className="text-[var(--color-text-muted)] text-lg font-light shrink-0">→</div>
        <div className="border border-dashed border-[var(--color-border)] text-[10px] text-[var(--color-text-muted)] px-2 py-1 rounded-lg">
          -- 循环 --
        </div>
      </div>
    </div>
  )
}

// ─── VariationConfig Replay Offset Visualization ──────────────────────


function VariationConfigViewer() {
  return (
    <div className="border border-[var(--color-border)] rounded-lg p-4 space-y-3">
      <h5 className="text-xs font-semibold text-[var(--color-text)] flex items-center gap-2">
        <SlidersHorizontal className="w-3.5 h-3.5" />
        回放偏移配置 (VariationConfig) 效果预览
      </h5>
      <p className="text-xs text-[var(--color-text-muted)]">
        以下展示回放时不同强度等级下的偏移参数影响范围。实际回放使用高斯分布采样，在边界范围内平滑随机。
      </p>

      <div className="grid grid-cols-3 gap-3">
        <VariationCard
          title="低强度 (0.1)"
          intensity={0.1}
          timingMs={20}
          positionPx={1}
          desc="几乎无偏移，高度还原原始录制"
        />
        <VariationCard
          title="默认 (0.3)"
          intensity={0.3}
          timingMs={60}
          positionPx={3}
          desc="适当时序/位置抖动，自然人类操作感"
        />
        <VariationCard
          title="高强度 (0.6)"
          intensity={0.6}
          timingMs={120}
          positionPx={10}
          desc="明显随机偏移，高风控规避场景"
        />
      </div>

      {/* Micro-corrections and extra pauses diagram */}
      <div className="text-xs space-y-2">
        <h6 className="font-medium text-[var(--color-text)]">微修正 & 额外停顿机制</h6>
        <div className="flex gap-3">
          <div className="flex-1 p-3 rounded bg-blue-50 space-y-1.5">
            <div className="flex items-center gap-2">
              <MoveHorizontal className="w-3.5 h-3.5 text-blue-500" />
              <span className="font-medium text-blue-700">微修正 (MicroCorrections)</span>
            </div>
            <div className="text-blue-600 leading-relaxed">
              点击后 50-200ms，30% 概率产生 1-2px 位移 → 返回原位。模拟点击后的微小手指滑动。
            </div>
            <svg viewBox="0 0 200 40" className="w-full max-w-[200px]" style={{ background: 'transparent' }}>
              <line x1={10} y1={20} x2={190} y2={20} stroke="#3b82f6" strokeWidth={1.5} strokeDasharray="4 2" opacity={0.4} />
              <circle cx={100} cy={20} r={3} fill="#3b82f6" />
              <path d="M100,20 Q105,15 102,18 Q98,22 100,20" stroke="#3b82f6" strokeWidth={1} fill="none" />
              <text x={100} y={35} textAnchor="middle" fill="#3b82f6" fontSize={9}>±1-2px 过冲</text>
            </svg>
          </div>
          <div className="flex-1 p-3 rounded bg-amber-50 space-y-1.5">
            <div className="flex items-center gap-2">
              <Timer className="w-3.5 h-3.5 text-amber-500" />
              <span className="font-medium text-amber-700">额外停顿 (ExtraPauses)</span>
            </div>
            <div className="text-amber-600 leading-relaxed">
              事件间隔 &gt;1s 时，20% 概率插入 200-800ms 额外暂停。模拟阅读/思考导致的自然滞留。
            </div>
            <svg viewBox="0 0 200 30" className="w-full max-w-[200px]" style={{ background: 'transparent' }}>
              <line x1={20} y1={15} x2={80} y2={15} stroke="#d97706" strokeWidth={2} />
              <line x1={80} y1={15} x2={90} y2={15} stroke="#d97706" strokeWidth={2} strokeDasharray="2 2" opacity={0.5} />
              <line x1={90} y1={15} x2={120} y2={15} stroke="#d97706" strokeWidth={3} opacity={0.4} />
              <line x1={120} y1={15} x2={180} y2={15} stroke="#d97706" strokeWidth={2} />
              <text x={105} y={28} textAnchor="middle" fill="#d97706" fontSize={9}>暂停 200-800ms</text>
            </svg>
          </div>
        </div>
      </div>
    </div>
  )
}

function VariationCard({ title, intensity, timingMs, positionPx, desc }: {
  title: string; intensity: number; timingMs: number; positionPx: number; desc: string
}) {
  return (
    <div className="p-3 rounded-lg bg-[var(--color-bg-subtle)] space-y-2">
      <div className="text-xs font-semibold text-[var(--color-text)]">{title}</div>
      <div className="space-y-1">
        <div className="flex justify-between text-[10px]">
          <span className="text-[var(--color-text-muted)]">时序抖动</span>
          <span className="font-mono text-[var(--color-text)]">±{timingMs}ms</span>
        </div>
        <RangeBar value={timingMs} max={150} color="bg-blue-400" />
        <div className="flex justify-between text-[10px]">
          <span className="text-[var(--color-text-muted)]">位置抖动</span>
          <span className="font-mono text-[var(--color-text)]">±{positionPx}px</span>
        </div>
        <RangeBar value={positionPx} max={15} color="bg-green-400" />
        <div className="flex justify-between text-[10px]">
          <span className="text-[var(--color-text-muted)]">微修正概率</span>
          <span className="font-mono text-[var(--color-text)]">{Math.round(intensity * 30)}%</span>
        </div>
      </div>
      <div className="text-[10px] text-[var(--color-text-muted)] leading-relaxed">{desc}</div>
    </div>
  )
}

function RangeBar({ value, max, color }: { value: number; max: number; color: string }) {
  const pct = Math.min(100, (value / max) * 100)
  return (
    <div className="h-1.5 bg-gray-200 rounded-full overflow-hidden">
      <div className={`h-full rounded-full ${color}`} style={{ width: `${pct}%` }} />
    </div>
  )
}

// ─── Humanize Offset Visualization ─────────────────────────────────────


function HumanizeOffsetViewer() {
  return (
    <div className="border border-[var(--color-border)] rounded-lg p-4 space-y-4">
      <h5 className="text-xs font-semibold text-[var(--color-text)] flex items-center gap-2">
        <Target className="w-3.5 h-3.5" />
        Humanize 偏移系统（自动化指令人类化）
      </h5>
      <p className="text-xs text-[var(--color-text-muted)]">
        humanize 系统在自动化指令执行前注入人类化变异。以下展示各偏移子系统的工作机制。
      </p>

      {/* ClickOffset */}
      <div className="space-y-2">
        <h6 className="text-xs font-medium text-[var(--color-text)] flex items-center gap-2">
          <Target className="w-3 h-3 text-red-400" />
          点击偏移 (ClickOffset)
        </h6>
        <div className="flex gap-4">
          <div className="flex-1 space-y-2">
            <div className="text-[10px] text-[var(--color-text-muted)] leading-relaxed">
              从元素中心取随机角度，在 <strong>RadiusPx</strong> 半径内随机偏移点击位置。
              可配置 <strong>BiasDirection</strong> 象限偏向（如偏向右上角模拟右撇子习惯）。
            </div>
            <div className="grid grid-cols-2 gap-2 text-[10px]">
              <OffsetBadge label="RadiusPx" value="3-12px" />
              <OffsetBadge label="角度范围" value="0-360°" />
              <OffsetBadge label="悬停前延迟" value="0-300ms" />
              <OffsetBadge label="轨迹路径" value="hover→target" />
            </div>
          </div>
          {/* Polar offset diagram */}
          <svg viewBox="0 0 100 100" className="w-24 h-24 shrink-0">
            <circle cx={50} cy={50} r={40} fill="none" stroke="var(--color-border)" strokeWidth={0.5} />
            <circle cx={50} cy={50} r={15} fill="none" stroke="var(--color-border)" strokeWidth={0.5} strokeDasharray="3 2" />
            <circle cx={50} cy={50} r={3} fill="#ef4444" />
            {/* Random offset points */}
            {[30, 60, 120, 200, 270, 340].map(deg => {
              const r = 18 + Math.random() * 20
              const rad = (deg * Math.PI) / 180
              return <circle key={deg} cx={50 + r * Math.cos(rad)} cy={50 + r * Math.sin(rad)} r={2} fill="#3b82f6" opacity={0.7} />
            })}
            <line x1={50} y1={10} x2={50} y2={90} stroke="var(--color-border)" strokeWidth={0.3} />
            <line x1={10} y1={50} x2={90} y2={50} stroke="var(--color-border)" strokeWidth={0.3} />
            <text x={50} y={98} textAnchor="middle" fill="var(--color-text-muted)" fontSize={8}>极坐标偏移</text>
          </svg>
        </div>
      </div>

      {/* TimingDistribution */}
      <div className="space-y-2">
        <h6 className="text-xs font-medium text-[var(--color-text)] flex items-center gap-2">
          <Clock className="w-3 h-3 text-amber-400" />
          时序分布 (TimingDistribution)
        </h6>
        <div className="text-[10px] text-[var(--color-text-muted)] leading-relaxed mb-2">
          支持三种延迟分布类型，适配不同人类行为模式：
        </div>
        <div className="grid grid-cols-3 gap-2">
          <TimingDistCard
            type="均匀分布"
            desc="固定范围内等概率"
            diagram="uniform"
            color="bg-blue-500"
          />
          <TimingDistCard
            type="正态分布"
            desc="均值附近集中，两端递减"
            diagram="normal"
            color="bg-green-500"
          />
          <TimingDistCard
            type="右偏分布"
            desc="快速响应为主，少量长延迟"
            diagram="skewed"
            color="bg-purple-500"
          />
        </div>
      </div>

      {/* ScrollPlan */}
      <div className="space-y-2">
        <h6 className="text-xs font-medium text-[var(--color-text)] flex items-center gap-2">
          <ArrowDownUp className="w-3 h-3 text-orange-400" />
          滚动计划 (ScrollPlan) 过冲-返回模式
        </h6>
        <div className="text-[10px] text-[var(--color-text-muted)] leading-relaxed mb-2">
          滚动操作采用 4 阶段过冲-返回模式，模拟人类滚动惯性和视觉确认：
        </div>
        <svg viewBox="0 0 300 50" className="w-full max-w-[300px]" style={{ background: 'var(--color-bg-subtle)', borderRadius: 6 }}>
          {/* Baseline */}
          <line x1={20} y1={35} x2={280} y2={35} stroke="var(--color-border)" strokeWidth={0.5} />
          {/* Scroll curve */}
          <path d="M20,35 Q50,15 80,10 Q100,8 110,15 Q120,22 125,25 Q130,28 145,28 Q160,28 170,35 Q180,40 190,42 Q200,43 215,38 Q230,35 240,34 Q260,34 280,35"
            fill="none" stroke="#f97316" strokeWidth={1.5} />
          {/* Labels */}
          <text x={50} y={30} textAnchor="middle" fill="var(--color-text-muted)" fontSize={7}>1.过冲</text>
          <text x={100} y={28} textAnchor="middle" fill="var(--color-text-muted)" fontSize={7}>2.暂停</text>
          <text x={155} y={30} textAnchor="middle" fill="var(--color-text-muted)" fontSize={7}>3.返回</text>
          <text x={215} y={32} textAnchor="middle" fill="var(--color-text-muted)" fontSize={7}>4.微调</text>
        </svg>
      </div>

      {/* FailureStyle */}
      <div className="space-y-2">
        <h6 className="text-xs font-medium text-[var(--color-text)] flex items-center gap-2">
          <AlertTriangle className="w-3 h-3 text-red-400" />
          故障恢复 (FailureStyle)
        </h6>
        <div className="text-[10px] text-[var(--color-text-muted)] leading-relaxed">
          支持 <strong>即时重试</strong> 和 <strong>人类式延迟重试</strong> 两种模式。人类式模式下，失败后等待随机间隔 (MinWaitMs-MaxWaitMs)，
          最多重试 MaxRetries 次，有 GiveUpChance 概率提前放弃。
        </div>
      </div>
    </div>
  )
}

function OffsetBadge({ label, value }: { label: string; value: string }) {
  return (
    <div className="px-2 py-1 rounded bg-[var(--color-bg-subtle)]">
      <span className="text-[var(--color-text-muted)]">{label}: </span>
      <span className="font-mono font-medium text-[var(--color-text)]">{value}</span>
    </div>
  )
}

function TimingDistCard({ type, desc, diagram, color }: {
  type: string; desc: string; diagram: 'uniform' | 'normal' | 'skewed'; color: string
}) {
  const h = 40; const w = 80
  const pad = { top: 5, bottom: 12, left: 5, right: 5 }
  const cw = w - pad.left - pad.right; const ch = h - pad.top - pad.bottom

  // Build SVG path based on distribution type
  let pathD = ''
  if (diagram === 'uniform') {
    pathD = `M${pad.left},${pad.top} L${pad.left},${pad.top + ch} L${pad.left + cw},${pad.top + ch} L${pad.left + cw},${pad.top}`
  } else if (diagram === 'normal') {
    const amp = ch
    const pts: string[] = []
    for (let i = 0; i <= 20; i++) {
      const t = i / 20
      const x = pad.left + t * cw
      const z = (t - 0.5) * 4
      const y = pad.top + ch - amp * Math.exp(-z * z / 2)
      pts.push(`${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`)
    }
    pathD = pts.join(' ')
  } else { // skewed
    const pts: string[] = []
    for (let i = 0; i <= 20; i++) {
      const t = i / 20
      const x = pad.left + t * cw
      // Right-skewed: quick rise, slow fall
      const z = t * 5
      const y = t < 0.2 ? pad.top + ch - ch * Math.exp(-(z - 1) * (z - 1) / 0.5) : pad.top + ch * (1 - Math.exp(-z * 1.2))
      pts.push(`${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${Math.max(pad.top, parseFloat(y.toFixed(1)))}`)
    }
    pathD = pts.join(' ')
  }

  return (
    <div className="p-2.5 rounded-lg bg-[var(--color-bg-subtle)] space-y-1.5">
      <div className={`w-2 h-2 rounded-full ${color}`} />
      <div className="text-xs font-medium text-[var(--color-text)]">{type}</div>
      <div className="text-[10px] text-[var(--color-text-muted)]">{desc}</div>
      <svg viewBox={`0 0 ${w} ${h}`} className="w-full" style={{ background: 'transparent' }}>
        <line x1={pad.left} y1={pad.top + ch} x2={pad.left + cw} y2={pad.top + ch} stroke="var(--color-border)" strokeWidth={0.5} />
        <path d={pathD} fill="none" stroke="var(--color-accent)" strokeWidth={1.5} opacity={0.7} />
      </svg>
    </div>
  )
}
