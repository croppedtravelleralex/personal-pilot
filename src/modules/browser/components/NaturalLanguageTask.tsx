import { useState } from 'react'
import { Sparkles } from 'lucide-react'
import { Button, toast, Textarea } from '../../../shared/components'
import type { NaturalLanguageAction } from '../types'
import { ActionTreeViewer, type ActionNode } from './ActionTreeViewer'

interface NaturalLanguageTaskProps {
  profileId?: string
  isRunning?: boolean
}

function previewActionType(text: string): NaturalLanguageAction['type'] {
  const normalized = text.trim().toLowerCase()
  if (normalized.includes('浏览') || normalized.includes('打开')) return 'goto'
  if (normalized.includes('点击')) return 'click'
  if (normalized.includes('输入') || normalized.includes('搜索')) return 'type'
  if (normalized.includes('滚动')) return 'scroll'
  return 'wait'
}

function toNodes(actions: NaturalLanguageAction[]): ActionNode[] {
  return actions.map((action, index) => ({
    id: `natural-language-${index}`,
    type: action.type,
    description: action.description || action.type,
    selector: action.selector,
    url: action.url,
    text: action.text,
    durationMs: action.durationMs,
    status: 'pending',
    branches: [],
  }))
}

export function NaturalLanguageTask({ profileId, isRunning }: NaturalLanguageTaskProps) {
  const [taskDescription, setTaskDescription] = useState('')
  const [plan, setPlan] = useState<ActionNode[]>([])
  const [planning, setPlanning] = useState(false)

  const handlePlan = () => {
    if (!taskDescription.trim()) return
    setPlanning(true)
    try {
      const actionType = previewActionType(taskDescription)
      const nodes = toNodes([{ type: actionType, description: taskDescription.trim() }])
      setPlan(nodes)
      toast.info('当前版本只生成预览，不执行自然语言任务')
    } finally {
      setPlanning(false)
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
          placeholder="输入一句话描述，你只能得到步骤预览，不会执行。"
          value={taskDescription}
          onChange={e => setTaskDescription(e.target.value)}
          rows={4}
          className="text-xs"
        />
        <div className="flex gap-2">
          <Button size="sm" variant="primary" onClick={handlePlan} loading={planning}>
            生成预览
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => toast.error('当前版本不支持自然语言任务执行')}
            disabled={!isRunning || !profileId}
          >
            执行任务
          </Button>
        </div>
      </div>

      {plan.length > 0 && (
        <ActionTreeViewer plan={plan} title={`动作预览 (${plan.length} 步)`} />
      )}
    </div>
  )
}
