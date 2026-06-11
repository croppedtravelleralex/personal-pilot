import { useRef, useState } from 'react'
import { Send, Play, Sparkles } from 'lucide-react'
import { Button, toast, Textarea } from '../../../shared/components'
import { messageFromUnknownError } from '../../../shared/errors'
import { ActionTreeViewer, type ActionNode } from './ActionTreeViewer'
import { executeNaturalLanguageTask, onNaturalLanguageTaskEvents, planNaturalLanguageTask } from '../api'
import type { NaturalLanguageAction, NaturalLanguageTaskEvent } from '../types'

interface NaturalLanguageTaskProps {
  profileId?: string
  isRunning?: boolean
}

export function NaturalLanguageTask({ profileId, isRunning }: NaturalLanguageTaskProps) {
  const [taskDescription, setTaskDescription] = useState('')
  const [plan, setPlan] = useState<ActionNode[]>([])
  const [planning, setPlanning] = useState(false)
  const [executing, setExecuting] = useState(false)
  const [taskState, setTaskState] = useState<'idle' | 'planning' | 'executing' | 'completed' | 'failed' | 'cancelled'>('idle')
  const [currentStep, setCurrentStep] = useState(0)
  const [totalSteps, setTotalSteps] = useState(0)
  const runIdRef = useRef(0)
  const runningStepRef = useRef(0)

  const mapActionsToNodes = (actions: NaturalLanguageAction[]) => actions.map((action, i) => ({
    id: `action-${i}`,
    type: action.type || 'wait',
    description: action.description || action.type,
    selector: action.selector,
    url: action.url,
    text: action.text,
    durationMs: action.durationMs,
    status: 'pending' as const,
    branches: [],
  }))

  // Generate plan preview
  const handlePlan = async () => {
    if (!taskDescription.trim()) return
    setPlanning(true)
    setTaskState('planning')
    try {
      const actions = await planNaturalLanguageTask(taskDescription.trim())
      const nodes: ActionNode[] = mapActionsToNodes(actions)
      setPlan(nodes)
      setTaskState('idle')
      toast.success(`计划生成成功: ${nodes.length} 个步骤`)
    } catch (error: unknown) {
      setTaskState('failed')
      toast.error(`计划生成失败: ${messageFromUnknownError(error, '未知错误')}`)
    } finally {
      setPlanning(false)
    }
  }

  // Execute the plan
  const handleExecute = async () => {
    if (!profileId) { toast.error('请先选择浏览器实例'); return }
    const runId = runIdRef.current + 1
    runIdRef.current = runId
    runningStepRef.current = 0
    let finishedByEvent = false
    setExecuting(true)
    setTaskState('executing')
    setCurrentStep(0)
    setTotalSteps(plan.length)
    setPlan(prev => prev.map(node => ({ ...node, status: 'pending' })))

    const isCurrentRun = () => runIdRef.current === runId
    const matchesProfile = (data: NaturalLanguageTaskEvent) => !data?.profileId || data.profileId === profileId
    const markFailed = () => {
      const failedStep = Math.max(0, runningStepRef.current - 1)
      setPlan(prev => prev.map((node, index) => (
        node.status === 'running' || index === failedStep ? { ...node, status: 'error' } : node
      )))
    }
    const markCompleted = () => {
      setPlan(prev => prev.map(node => (
        node.status === 'error' ? node : { ...node, status: 'done' }
      )))
    }

    // Listen for progress events
    const offTaskEvents = onNaturalLanguageTaskEvents({
      onPlanning: (data) => {
        if (!isCurrentRun() || !matchesProfile(data)) return
        setTaskState('planning')
      },
      onPlanReady: (data) => {
        if (!isCurrentRun() || !matchesProfile(data) || !data.actions) return
        const nodes = mapActionsToNodes(data.actions)
        setPlan(nodes)
        setTotalSteps(nodes.length)
        setTaskState('executing')
      },
      onExecuting: (data) => {
        if (!isCurrentRun() || !matchesProfile(data)) return
        const step = Number(data.step) || 0
        const total = Number(data.total) || plan.length
        runningStepRef.current = step
        setCurrentStep(step)
        setTotalSteps(total)
        setTaskState('executing')
        if (step > 0) {
          setPlan(prev => prev.map((n, i) =>
            i === step - 1 ? { ...n, status: 'running' } : n
          ))
        }
      },
      onStepDone: (data) => {
        if (!isCurrentRun() || !matchesProfile(data)) return
        const step = Number(data.step) || 0
        if (step > 0) {
          setPlan(prev => prev.map((n, i) =>
            i === step - 1 ? { ...n, status: 'done' } : n
          ))
        }
      },
      onComplete: (data) => {
        if (!isCurrentRun() || !matchesProfile(data)) return
        finishedByEvent = true
        markCompleted()
        setTaskState('completed')
        setExecuting(false)
        setCurrentStep(data.total || totalSteps || plan.length)
        if (data.recordingId) {
          toast.success(`任务完成! 录制 ${data.recordingId} (${data.eventCount || 0} 事件)`)
        } else {
          toast.success('任务完成')
        }
      },
      onFailed: (data) => {
        if (!isCurrentRun() || !matchesProfile(data)) return
        finishedByEvent = true
        markFailed()
        setTaskState('failed')
        setExecuting(false)
        toast.error(`执行失败: ${data.error || data.reason || '未知错误'}`)
      },
      onCancelled: (data) => {
        if (!isCurrentRun() || !matchesProfile(data)) return
        finishedByEvent = true
        setTaskState('cancelled')
        setExecuting(false)
        toast.error(`执行已取消${data.reason ? `: ${data.reason}` : ''}`)
      },
    })

    try {
      await executeNaturalLanguageTask(profileId, taskDescription.trim())
      if (isCurrentRun() && !finishedByEvent) {
        markCompleted()
        setTaskState('completed')
        setExecuting(false)
      }
    } catch (error: unknown) {
      if (isCurrentRun() && !finishedByEvent) {
        markFailed()
        setTaskState('failed')
        toast.error(`执行失败: ${messageFromUnknownError(error, '未知错误')}`)
      }
      setExecuting(false)
    } finally {
      offTaskEvents()
    }
  }

  return (
    <div className="space-y-4">
      <h4 className="text-sm font-semibold text-[var(--color-text)] flex items-center gap-2">
        <Sparkles className="w-4 h-4 text-amber-400" />
        自然语言任务
      </h4>

      <div className="space-y-2">
        <Textarea
          placeholder="用自然语言描述你的任务，例如:&#10;打开小红书，浏览首页 5 篇笔记，随机点赞 3 篇有趣的，搜索「美食」并浏览搜索结果"
          value={taskDescription}
          onChange={e => setTaskDescription(e.target.value)}
          rows={4}
          className="text-xs"
        />
        <div className="flex gap-2">
          <Button size="sm" variant="primary" onClick={handlePlan} loading={planning}>
            <Send className="w-3.5 h-3.5" />
            生成计划
          </Button>
          {plan.length > 0 && (
            <Button
              size="sm"
              variant="secondary"
              onClick={handleExecute}
              loading={executing}
              disabled={!isRunning || executing}
              title={!isRunning ? '请先启动浏览器实例' : '执行任务'}
            >
              <Play className="w-3.5 h-3.5" />
              执行任务
            </Button>
          )}
        </div>
      </div>

      {/* Progress bar */}
      {executing && totalSteps > 0 && (
        <div className="space-y-1">
          <div className="flex justify-between text-[10px] text-[var(--color-text-muted)]">
            <span>执行进度</span>
            <span>{currentStep} / {totalSteps}</span>
          </div>
          <div className="h-1.5 bg-gray-200 rounded-full overflow-hidden">
            <div
              className="h-full bg-[var(--color-accent)] rounded-full transition-all duration-300"
              style={{ width: `${(currentStep / totalSteps) * 100}%` }}
            />
          </div>
        </div>
      )}

      {taskState === 'failed' && (
        <div className="text-[10px] text-red-500">执行失败，已停止当前任务状态。</div>
      )}
      {taskState === 'cancelled' && (
        <div className="text-[10px] text-[var(--color-text-muted)]">执行已取消。</div>
      )}

      {/* Action plan tree */}
      {plan.length > 0 && (
        <ActionTreeViewer plan={plan} title={`动作计划 (${plan.length} 步)`} />
      )}
    </div>
  )
}
