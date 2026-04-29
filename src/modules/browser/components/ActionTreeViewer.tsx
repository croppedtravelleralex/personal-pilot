import { useState } from 'react'
import { ChevronDown, ChevronRight, Globe, Eye, MousePointerClick, Search, Clock } from 'lucide-react'

export interface ActionNode {
  id: string
  type: 'goto' | 'click' | 'scroll' | 'type' | 'wait'
  description: string
  selector?: string
  url?: string
  text?: string
  durationMs?: number
  category?: string
  status?: 'pending' | 'running' | 'done' | 'error'
  branches?: ActionBranch[]
}

export interface ActionBranch {
  id: string
  name: string
  description: string
  behaviorPreset?: string
  clickRadius?: number
  preDelayMs?: number
  postDelayMs?: number
  overshootRatio?: number
}

interface ActionTreeViewerProps {
  plan: ActionNode[]
  title?: string
}

const typeIcons: Record<string, React.ReactNode> = {
  goto: <Globe className="w-3.5 h-3.5 text-blue-400" />,
  click: <MousePointerClick className="w-3.5 h-3.5 text-green-400" />,
  scroll: <Eye className="w-3.5 h-3.5 text-orange-400" />,
  type: <Search className="w-3.5 h-3.5 text-purple-400" />,
  wait: <Clock className="w-3.5 h-3.5 text-amber-400" />,
}

const typeLabels: Record<string, string> = {
  goto: '导航', click: '点击', scroll: '滚动', type: '输入', wait: '等待',
}

const statusColors: Record<string, string> = {
  pending: 'bg-gray-300', running: 'bg-blue-400 animate-pulse', done: 'bg-green-400', error: 'bg-red-400',
}

export function ActionTreeViewer({ plan, title }: ActionTreeViewerProps) {
  const [expandedNodes, setExpandedNodes] = useState<Set<string>>(new Set())

  const toggleNode = (id: string) => {
    setExpandedNodes(prev => {
      const next = new Set(prev)
      if (next.has(id)) { next.delete(id) } else { next.add(id) }
      return next
    })
  }

  if (plan.length === 0) return null

  return (
    <div className="space-y-2">
      {title && <h5 className="text-xs font-semibold text-[var(--color-text)]">{title}</h5>}

      {plan.map((node, i) => (
        <div key={node.id || i} className="border border-[var(--color-border)] rounded-lg overflow-hidden">
          {/* Root node */}
          <button
            type="button"
            className="w-full flex items-center gap-2 px-3 py-2 hover:bg-[var(--color-bg-hover)] transition-colors text-left"
            onClick={() => toggleNode(node.id)}
          >
            <span className={`w-2 h-2 rounded-full shrink-0 ${statusColors[node.status || 'pending']}`} />
            <span className="text-[var(--color-text-muted)]">{typeIcons[node.type]}</span>
            <span className="text-xs font-medium text-[var(--color-text)] flex-1">{node.description}</span>
            <span className="text-[10px] text-[var(--color-text-muted)] px-1.5 py-0.5 rounded bg-[var(--color-bg-subtle)]">
              {typeLabels[node.type]}
            </span>
            {(node.branches?.length || 0) > 0 && (
              <span className="text-[10px] text-[var(--color-accent)] font-medium">
                {node.branches!.length} 偏移
              </span>
            )}
            {expandedNodes.has(node.id) ? (
              <ChevronDown className="w-3.5 h-3.5 text-[var(--color-text-muted)]" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5 text-[var(--color-text-muted)]" />
            )}
          </button>

          {/* Branches */}
          {expandedNodes.has(node.id) && node.branches && node.branches.length > 0 && (
            <div className="border-t border-[var(--color-border)] bg-[var(--color-bg-subtle)]">
              <div className="text-[10px] text-[var(--color-text-muted)] px-6 py-1.5 border-b border-[var(--color-border)]">
                {node.branches.length} 个偏移方向
              </div>
              <div className="max-h-48 overflow-y-auto">
                {node.branches.map((branch, j) => (
                  <div key={branch.id} className="flex items-center gap-2 px-6 py-1.5 text-xs border-b border-[var(--color-border)] last:border-b-0">
                    <span className="text-[10px] text-[var(--color-text-muted)] w-6 shrink-0">{j + 1}</span>
                    <div className="flex-1 min-w-0">
                      <div className="font-medium text-[var(--color-text)] truncate">{branch.name}</div>
                      <div className="text-[10px] text-[var(--color-text-muted)] truncate">{branch.description}</div>
                    </div>
                    <div className="flex gap-1.5 shrink-0">
                      {branch.preDelayMs != null && (
                        <span className="text-[10px] px-1 py-0.5 rounded bg-blue-50 text-blue-600">±{branch.preDelayMs}ms</span>
                      )}
                      {branch.clickRadius != null && (
                        <span className="text-[10px] px-1 py-0.5 rounded bg-green-50 text-green-600">r={branch.clickRadius}px</span>
                      )}
                      {branch.overshootRatio != null && (
                        <span className="text-[10px] px-1 py-0.5 rounded bg-orange-50 text-orange-600">+{((branch.overshootRatio || 0) * 100).toFixed(0)}%</span>
                      )}
                      {branch.behaviorPreset && (
                        <span className="text-[10px] px-1 py-0.5 rounded bg-purple-50 text-purple-600">{branch.behaviorPreset}</span>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      ))}
    </div>
  )
}
