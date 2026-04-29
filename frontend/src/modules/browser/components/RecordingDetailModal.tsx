import { useEffect, useState } from 'react'
import { X, Trash2, MousePointer, Keyboard, ScrollText, Clock, Monitor, Scissors } from 'lucide-react'
import { Button, Input, Select, toast } from '../../../shared/components'
import type { RecordingDetailPage, RecordingEventStats } from '../types'
import { fetchRecordingDetail, trimRecording } from '../api'

interface RecordingDetailModalProps {
  recordingId: string
  onClose: () => void
  onDelete?: (id: string) => void
  onChanged?: () => void | Promise<void>
}

const EVENT_PAGE_SIZES = [50, 100, 200]

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function RecordingDetailModal({ recordingId, onClose, onDelete, onChanged }: RecordingDetailModalProps) {
  const [detail, setDetail] = useState<RecordingDetailPage | null>(null)
  const [loading, setLoading] = useState(true)
  const [eventPage, setEventPage] = useState(1)
  const [eventPageSize, setEventPageSize] = useState(50)
  const [trimOpen, setTrimOpen] = useState(false)
  const [trimStart, setTrimStart] = useState(1)
  const [trimEnd, setTrimEnd] = useState(1)
  const [trimName, setTrimName] = useState('')
  const [trimming, setTrimming] = useState(false)

  useEffect(() => {
    setEventPage(1)
  }, [recordingId, eventPageSize])

  useEffect(() => {
    let active = true
    const load = async () => {
      setLoading(true)
      try {
        const eventOffset = (eventPage - 1) * eventPageSize
        const nextDetail = await fetchRecordingDetail(recordingId, { eventOffset, eventLimit: eventPageSize })
        if (active) setDetail(nextDetail)
      } catch (e: any) {
        if (active) toast.error(`加载录制详情失败: ${e?.message || e}`)
      } finally {
        if (active) setLoading(false)
      }
    }
    load()
    return () => { active = false }
  }, [recordingId, eventPage, eventPageSize])

  // Event type stats
  const stats: RecordingEventStats = detail?.stats || { total: 0, move: 0, click: 0, key: 0, scroll: 0 }
  const recording = detail?.recording
  const eventTotal = detail?.eventTotal || 0
  const totalEventPages = Math.max(1, Math.ceil(eventTotal / eventPageSize))
  const eventFrom = eventTotal === 0 ? 0 : (eventPage - 1) * eventPageSize + 1
  const eventTo = Math.min(eventPage * eventPageSize, eventTotal)
  const pageEvents = detail?.events || []
  const pageEventOffset = detail?.eventOffset || (eventPage - 1) * eventPageSize

  useEffect(() => {
    if (eventPage > totalEventPages) setEventPage(totalEventPages)
  }, [eventPage, totalEventPages])

  const openTrim = () => {
    const start = eventFrom > 0 ? eventFrom : 1
    const end = eventTo > 0 ? eventTo : Math.max(1, eventTotal)
    setTrimStart(start)
    setTrimEnd(end)
    setTrimName(`${recording?.name || ''} 裁剪`)
    setTrimOpen(true)
  }

  const handleTrim = async () => {
    if (!recording) return
    const start = Math.floor(trimStart)
    const end = Math.floor(trimEnd)
    const name = trimName.trim()
    if (!name) {
      toast.error('请输入裁剪名称')
      return
    }
    if (start < 1 || end < start || end > eventTotal) {
      toast.error(`事件序号需在 1-${eventTotal} 内`)
      return
    }
    setTrimming(true)
    try {
      await trimRecording(recording.id, start, end, name)
      await onChanged?.()
      setTrimOpen(false)
      toast.success('裁剪录制已保存')
    } catch (e) {
      toast.error(`裁剪失败: ${getErrorMessage(e)}`)
    } finally {
      setTrimming(false)
    }
  }

  if (loading) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/20" onClick={onClose}>
        <div className="bg-[var(--color-bg)] border border-[var(--color-border)] rounded-xl p-6 shadow-2xl w-[480px] max-h-[80vh]" onClick={e => e.stopPropagation()}>
          <div className="text-sm text-[var(--color-text-muted)] animate-pulse">加载中...</div>
        </div>
      </div>
    )
  }

  if (!recording) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/20" onClick={onClose}>
        <div className="bg-[var(--color-bg)] border border-[var(--color-border)] rounded-xl p-6 shadow-2xl w-[480px]" onClick={e => e.stopPropagation()}>
          <div className="text-sm text-[var(--color-text-muted)]">录制未找到</div>
          <Button size="sm" className="mt-4" onClick={onClose}>关闭</Button>
        </div>
      </div>
    )
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/20" onClick={onClose}>
      <div className="bg-[var(--color-bg)] border border-[var(--color-border)] rounded-xl p-6 shadow-2xl w-[560px] max-h-[85vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
        {/* Header */}
        <div className="flex items-start justify-between mb-4">
          <div>
            <h3 className="text-base font-semibold text-[var(--color-text)]">{recording.name}</h3>
            {recording.description && (
              <p className="text-xs text-[var(--color-text-muted)] mt-0.5">{recording.description}</p>
            )}
          </div>
          <div className="flex items-center gap-1">
            <button
              className="p-1.5 hover:bg-[var(--color-bg-hover)] rounded transition-colors"
              onClick={openTrim}
              title="裁剪"
            >
              <Scissors className="w-4 h-4 text-[var(--color-text-muted)]" />
            </button>
            {onDelete && (
              <button
                className="p-1.5 hover:bg-red-100 rounded transition-colors"
                onClick={() => onDelete(recording.id)}
                title="删除"
              >
                <Trash2 className="w-4 h-4 text-red-400" />
              </button>
            )}
            <button
              className="p-1.5 hover:bg-[var(--color-bg-hover)] rounded transition-colors"
              onClick={onClose}
            >
              <X className="w-4 h-4 text-[var(--color-text-muted)]" />
            </button>
          </div>
        </div>

        {/* Meta info */}
        <div className="grid grid-cols-4 gap-3 mb-4">
          <div className="bg-[var(--color-bg-subtle)] rounded-lg p-3 text-center">
            <Clock className="w-4 h-4 text-[var(--color-text-muted)] mx-auto mb-1" />
            <div className="text-xs text-[var(--color-text-muted)]">时长</div>
            <div className="text-sm font-semibold text-[var(--color-text)]">{(recording.durationMs / 1000).toFixed(1)}s</div>
          </div>
          <div className="bg-[var(--color-bg-subtle)] rounded-lg p-3 text-center">
            <Monitor className="w-4 h-4 text-[var(--color-text-muted)] mx-auto mb-1" />
            <div className="text-xs text-[var(--color-text-muted)]">视口</div>
            <div className="text-sm font-semibold text-[var(--color-text)]">{recording.viewportW}x{recording.viewportH}</div>
          </div>
          <div className="bg-[var(--color-bg-subtle)] rounded-lg p-3 text-center">
            <div className="w-4 h-4 text-[var(--color-text-muted)] mx-auto mb-1 flex items-center justify-center text-xs font-bold">{stats.total}</div>
            <div className="text-xs text-[var(--color-text-muted)]">事件总数</div>
          </div>
          <div className="bg-[var(--color-bg-subtle)] rounded-lg p-3 text-center">
            <div className="w-4 h-4 text-[var(--color-text-muted)] mx-auto mb-1 text-xs">{new Date(recording.createdAt).toLocaleDateString('zh-CN')}</div>
            <div className="text-xs text-[var(--color-text-muted)]">创建日期</div>
          </div>
        </div>

        {/* Event type breakdown */}
        <div className="mb-4">
          <h5 className="text-xs font-semibold text-[var(--color-text)] mb-2">事件类型分布</h5>
          <div className="flex gap-2">
            <div className="flex-1 bg-blue-50 rounded-lg p-2 text-center">
              <MousePointer className="w-3.5 h-3.5 text-blue-500 mx-auto" />
              <div className="text-xs text-[var(--color-text)] font-semibold">{stats.move}</div>
              <div className="text-[10px] text-[var(--color-text-muted)]">鼠标移动</div>
            </div>
            <div className="flex-1 bg-green-50 rounded-lg p-2 text-center">
              <MousePointer className="w-3.5 h-3.5 text-green-500 mx-auto" />
              <div className="text-xs text-[var(--color-text)] font-semibold">{stats.click}</div>
              <div className="text-[10px] text-[var(--color-text-muted)]">点击</div>
            </div>
            <div className="flex-1 bg-purple-50 rounded-lg p-2 text-center">
              <Keyboard className="w-3.5 h-3.5 text-purple-500 mx-auto" />
              <div className="text-xs text-[var(--color-text)] font-semibold">{stats.key}</div>
              <div className="text-[10px] text-[var(--color-text-muted)]">键盘</div>
            </div>
            <div className="flex-1 bg-orange-50 rounded-lg p-2 text-center">
              <ScrollText className="w-3.5 h-3.5 text-orange-500 mx-auto" />
              <div className="text-xs text-[var(--color-text)] font-semibold">{stats.scroll}</div>
              <div className="text-[10px] text-[var(--color-text-muted)]">滚动</div>
            </div>
          </div>
        </div>

        {/* Event type bar */}
        {stats.total > 0 && (
          <div className="mb-4">
            <div className="flex h-3 rounded-full overflow-hidden">
              {stats.move > 0 && (
                <div className="bg-blue-400 h-full" style={{ width: `${(stats.move / stats.total) * 100}%` }} title={`移动 ${stats.move}`} />
              )}
              {stats.click > 0 && (
                <div className="bg-green-400 h-full" style={{ width: `${(stats.click / stats.total) * 100}%` }} title={`点击 ${stats.click}`} />
              )}
              {stats.key > 0 && (
                <div className="bg-purple-400 h-full" style={{ width: `${(stats.key / stats.total) * 100}%` }} title={`键盘 ${stats.key}`} />
              )}
              {stats.scroll > 0 && (
                <div className="bg-orange-400 h-full" style={{ width: `${(stats.scroll / stats.total) * 100}%` }} title={`滚动 ${stats.scroll}`} />
              )}
            </div>
            <div className="flex gap-3 mt-1.5 text-[10px] text-[var(--color-text-muted)]">
              <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-blue-400 inline-block" />移动 {((stats.move / stats.total) * 100).toFixed(0)}%</span>
              <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-green-400 inline-block" />点击 {((stats.click / stats.total) * 100).toFixed(0)}%</span>
              <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-purple-400 inline-block" />键盘 {((stats.key / stats.total) * 100).toFixed(0)}%</span>
              <span className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-orange-400 inline-block" />滚动 {((stats.scroll / stats.total) * 100).toFixed(0)}%</span>
            </div>
          </div>
        )}

        {trimOpen && (
          <div className="mb-4 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-subtle)] p-3">
            <div className="grid grid-cols-[1fr_80px_80px] gap-2">
              <Input
                className="h-8 text-xs"
                value={trimName}
                onChange={e => setTrimName(e.target.value)}
                placeholder="裁剪后名称"
              />
              <Input
                className="h-8 text-xs"
                type="number"
                min={1}
                max={eventTotal}
                value={trimStart}
                onChange={e => setTrimStart(Number(e.target.value))}
                title="起始事件序号"
              />
              <Input
                className="h-8 text-xs"
                type="number"
                min={1}
                max={eventTotal}
                value={trimEnd}
                onChange={e => setTrimEnd(Number(e.target.value))}
                title="结束事件序号"
              />
            </div>
            <div className="mt-2 flex items-center justify-between gap-2">
              <span className="text-[10px] text-[var(--color-text-muted)]">1-based inclusive · 当前默认 {eventFrom}-{eventTo}</span>
              <div className="flex gap-2">
                <Button size="sm" variant="secondary" className="h-7 px-2" onClick={() => setTrimOpen(false)}>取消</Button>
                <Button size="sm" className="h-7 px-2" onClick={handleTrim} loading={trimming}>保存</Button>
              </div>
            </div>
          </div>
        )}

        {/* Event list */}
        <div>
          <div className="flex items-center justify-between gap-2 mb-2">
            <h5 className="text-xs font-semibold text-[var(--color-text)]">
              事件列表 ({eventFrom}-{eventTo} / 共 {eventTotal} 条)
            </h5>
            <div className="flex items-center gap-1.5 text-xs text-[var(--color-text-muted)]">
              <Select
                className="w-20 h-7 px-2 text-xs"
                value={String(eventPageSize)}
                onChange={e => setEventPageSize(Number(e.target.value))}
                options={EVENT_PAGE_SIZES.map(size => ({ value: String(size), label: `${size}/页` }))}
              />
              <button
                type="button"
                className="px-2 py-1 rounded border border-[var(--color-border)] disabled:opacity-40"
                disabled={eventPage <= 1}
                onClick={() => setEventPage(page => Math.max(1, page - 1))}
              >
                上一页
              </button>
              <span className="tabular-nums">{eventPage}/{totalEventPages}</span>
              <button
                type="button"
                className="px-2 py-1 rounded border border-[var(--color-border)] disabled:opacity-40"
                disabled={eventPage >= totalEventPages}
                onClick={() => setEventPage(page => Math.min(totalEventPages, page + 1))}
              >
                下一页
              </button>
            </div>
          </div>
          <div className="max-h-48 overflow-y-auto space-y-0.5">
            {pageEvents.map((ev, i) => (
              <div key={`${pageEventOffset + i}-${ev.t}-${ev.type}`} className="flex items-center gap-2 px-2 py-1 rounded bg-[var(--color-bg-subtle)] text-xs">
                <span className={`inline-block w-14 text-center rounded px-1 py-0.5 text-[10px] font-medium ${
                  ev.type === 'move' ? 'bg-blue-100 text-blue-700' :
                  ev.type === 'click' || ev.type === 'down' || ev.type === 'up' ? 'bg-green-100 text-green-700' :
                  ev.type === 'key' ? 'bg-purple-100 text-purple-700' :
                  ev.type === 'scroll' ? 'bg-orange-100 text-orange-700' :
                  'bg-gray-100 text-gray-600'
                }`}>
                  {ev.type}
                </span>
                <span className="text-[var(--color-text-muted)] tabular-nums w-8 text-right">#{pageEventOffset + i + 1}</span>
                <span className="text-[var(--color-text-muted)] tabular-nums w-16 text-right">{(ev.t / 1000).toFixed(2)}s</span>
                {ev.type === 'move' && <span className="text-[var(--color-text-muted)]">({ev.x?.toFixed(0)},{ev.y?.toFixed(0)})</span>}
                {(ev.type === 'click' || ev.type === 'down' || ev.type === 'up') && <span className="text-[var(--color-text-muted)]">({ev.x?.toFixed(0)},{ev.y?.toFixed(0)}) btn={ev.btn}</span>}
                {ev.type === 'key' && <span className="text-[var(--color-text-muted)]">{ev.key}{ev.text ? ` (${ev.text})` : ''}</span>}
                {ev.type === 'scroll' && <span className="text-[var(--color-text-muted)]">dx={ev.dx?.toFixed(0)} dy={ev.dy?.toFixed(0)}</span>}
              </div>
            ))}
            {pageEvents.length === 0 && (
              <div className="text-xs text-[var(--color-text-muted)] py-3 text-center border border-dashed border-[var(--color-border)] rounded-lg">
                当前页没有事件
              </div>
            )}
          </div>
        </div>

        <div className="mt-4 pt-3 border-t border-[var(--color-border)] flex justify-end">
          <Button size="sm" variant="secondary" onClick={onClose}>关闭</Button>
        </div>
      </div>
    </div>
  )
}
