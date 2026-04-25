import { useEffect, useState } from 'react'
import { ChevronDown, ChevronUp, Play, Square, Trash2, Circle } from 'lucide-react'
import { Button, FormItem, Input, Select, toast } from '../../../shared/components'
import type { Recording, VariationConfig } from '../types'
import {
  BehaviorRecordingList,
  BehaviorRecordingDelete,
  BehaviorPlayRecording,
  BehaviorStopPlayback,
  BehaviorStopRecording,
  BehaviorStartRecording,
} from '../../../wailsjs/go/main/App'

interface RecordingPanelProps {
  profileId?: string
  isRunning?: boolean
}

const DEFAULT_VARIATION: VariationConfig = {
  intensity: 0.3,
  timingJitter: 200,
  positionJitter: 5,
  speedVariation: 0.2,
  microCorrections: true,
  extraPauses: true,
}

export function RecordingPanel({ profileId, isRunning }: RecordingPanelProps) {
  const [recordings, setRecordings] = useState<Recording[]>([])
  const [loading, setLoading] = useState(false)
  const [isRecording, setIsRecording] = useState(false)
  const [isPlaying, setIsPlaying] = useState(false)
  const [variation, setVariation] = useState<VariationConfig>(DEFAULT_VARIATION)
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [playRecordingId, setPlayRecordingId] = useState('')

  // Load recordings list
  const loadRecordings = async () => {
    try {
      const list = await BehaviorRecordingList()
      setRecordings(list || [])
    } catch {
      // ignore errors (e.g. store not initialized yet)
    }
  }

  useEffect(() => {
    loadRecordings()
  }, [])

  // Start recording
  const handleStartRecording = async () => {
    if (!profileId) return
    setLoading(true)
    try {
      await BehaviorStartRecording(profileId)
      setIsRecording(true)
      toast.success('录制已开始 - 请在浏览器中操作')
    } catch (e: any) {
      toast.error(`开始录制失败: ${e?.message || e}`)
    } finally {
      setLoading(false)
    }
  }

  // Stop recording
  const handleStopRecording = async () => {
    if (!profileId) return
    setLoading(true)
    try {
      const name = `录制 ${new Date().toLocaleString('zh-CN')}`
      await BehaviorStopRecording(profileId, name)
      setIsRecording(false)
      toast.success('录制已保存')
      await loadRecordings()
    } catch (e: any) {
      toast.error(`停止录制失败: ${e?.message || e}`)
    } finally {
      setLoading(false)
    }
  }

  // Delete recording
  const handleDelete = async (id: string) => {
    try {
      await BehaviorRecordingDelete(id)
      toast.success('录制已删除')
      await loadRecordings()
      if (playRecordingId === id) setPlayRecordingId('')
    } catch (e: any) {
      toast.error(`删除失败: ${e?.message || e}`)
    }
  }

  // Start playback
  const handlePlay = async () => {
    if (!profileId || !playRecordingId) return
    setLoading(true)
    try {
      await BehaviorPlayRecording(profileId, playRecordingId, variation)
      setIsPlaying(true)
      toast.success('回放已开始')
    } catch (e: any) {
      toast.error(`回放失败: ${e?.message || e}`)
    } finally {
      setLoading(false)
    }
  }

  // Stop playback
  const handleStopPlayback = async () => {
    if (!profileId) return
    try {
      await BehaviorStopPlayback(profileId)
      setIsPlaying(false)
      toast.success('回放已停止')
    } catch (e: any) {
      toast.error(`停止回放失败: ${e?.message || e}`)
    }
  }

  const hasRunningProfile = !!profileId && isRunning

  return (
    <div className="space-y-4">
      <h4 className="text-sm font-semibold text-[var(--color-text)]">行为录制与回放</h4>

      {/* Recording controls */}
      <div className="flex items-center gap-3">
        {!isRecording ? (
          <Button
            size="sm"
            variant="primary"
            onClick={handleStartRecording}
            loading={loading}
            disabled={!hasRunningProfile}
            title={!hasRunningProfile ? '请先启动浏览器实例' : '开始录制'}
          >
            <Circle className="w-3.5 h-3.5 fill-red-500 text-red-500" />
            录制
          </Button>
        ) : (
          <Button size="sm" variant="secondary" onClick={handleStopRecording} loading={loading}>
            <Square className="w-3.5 h-3.5" />
            停止录制
          </Button>
        )}
        {isPlaying && (
          <Button size="sm" variant="secondary" onClick={handleStopPlayback}>
            <Square className="w-3.5 h-3.5" />
            停止回放
          </Button>
        )}
        {isRecording && (
          <span className="text-xs text-red-500 animate-pulse">录制中...</span>
        )}
        {isPlaying && (
          <span className="text-xs text-[var(--color-accent)] animate-pulse">回放中...</span>
        )}
      </div>

      {/* Playback section */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Select
            value={playRecordingId}
            onChange={e => setPlayRecordingId(e.target.value)}
            options={[
              { value: '', label: '选择录制...' },
              ...recordings.map(r => ({
                value: r.id,
                label: `${r.name} (${(r.durationMs / 1000).toFixed(1)}s, ${r.events?.length || 0} 事件)`,
              })),
            ]}
          />
          <Button
            size="sm"
            onClick={handlePlay}
            loading={loading}
            disabled={!hasRunningProfile || !playRecordingId}
            title={!hasRunningProfile ? '请先启动浏览器实例' : '开始回放'}
          >
            <Play className="w-3.5 h-3.5" />
          </Button>
        </div>

        {/* Variation config */}
        <button
          type="button"
          className="w-full flex items-center justify-between px-3 py-1.5 text-xs text-[var(--color-text-muted)] hover:bg-[var(--color-bg-hover)] rounded transition-colors"
          onClick={() => setAdvancedOpen(v => !v)}
        >
          <span>偏移配置 (强度: {Math.round(variation.intensity * 100)}%)</span>
          {advancedOpen ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
        </button>
        {advancedOpen && (
          <div className="space-y-3 px-3 py-3 border border-[var(--color-border)] rounded-lg bg-[var(--color-bg-subtle)]">
            <FormItem label={`强度: ${Math.round(variation.intensity * 100)}%`}>
              <Input
                type="range"
                min={0}
                max={100}
                step={5}
                value={Math.round(variation.intensity * 100)}
                onChange={e => setVariation({ ...variation, intensity: Number(e.target.value) / 100 })}
              />
            </FormItem>
            <FormItem label={`时序抖动: ${Math.round(variation.timingJitter)}ms`}>
              <Input
                type="range"
                min={0}
                max={1000}
                step={50}
                value={Math.round(variation.timingJitter)}
                onChange={e => setVariation({ ...variation, timingJitter: Number(e.target.value) })}
              />
            </FormItem>
            <FormItem label={`位置抖动: ${Math.round(variation.positionJitter)}px`}>
              <Input
                type="range"
                min={0}
                max={30}
                step={1}
                value={Math.round(variation.positionJitter)}
                onChange={e => setVariation({ ...variation, positionJitter: Number(e.target.value) })}
              />
            </FormItem>
            <FormItem label={`速度变化: ${Math.round(variation.speedVariation * 100)}%`}>
              <Input
                type="range"
                min={0}
                max={100}
                step={5}
                value={Math.round(variation.speedVariation * 100)}
                onChange={e => setVariation({ ...variation, speedVariation: Number(e.target.value) / 100 })}
              />
            </FormItem>
            <div className="flex gap-4">
              <label className="flex items-center gap-2 text-xs text-[var(--color-text-muted)]">
                <input
                  type="checkbox"
                  checked={variation.microCorrections}
                  onChange={e => setVariation({ ...variation, microCorrections: e.target.checked })}
                />
                微修正
              </label>
              <label className="flex items-center gap-2 text-xs text-[var(--color-text-muted)]">
                <input
                  type="checkbox"
                  checked={variation.extraPauses}
                  onChange={e => setVariation({ ...variation, extraPauses: e.target.checked })}
                />
                额外停顿
              </label>
            </div>
          </div>
        )}
      </div>

      {/* Recordings list */}
      {recordings.length > 0 && (
        <div className="space-y-1.5 max-h-48 overflow-y-auto">
          <div className="text-xs text-[var(--color-text-muted)] font-medium">已保存录制 ({recordings.length})</div>
          {recordings.map(r => (
            <div
              key={r.id}
              className="flex items-center justify-between px-2.5 py-1.5 rounded bg-[var(--color-bg-subtle)] text-xs"
            >
              <div className="flex-1 min-w-0">
                <div className="font-medium text-[var(--color-text)] truncate">{r.name}</div>
                <div className="text-[var(--color-text-muted)]">
                  {(r.durationMs / 1000).toFixed(1)}s · {r.events?.length || 0} 事件 · {r.viewportW}x{r.viewportH}
                </div>
              </div>
              <div className="flex items-center gap-1 ml-2 shrink-0">
                <button
                  className="p-1 hover:bg-[var(--color-bg-hover)] rounded"
                  onClick={() => setPlayRecordingId(r.id)}
                  title="选择播放"
                >
                  <Play className="w-3 h-3 text-[var(--color-text-muted)]" />
                </button>
                <button
                  className="p-1 hover:bg-red-100 rounded"
                  onClick={() => handleDelete(r.id)}
                  title="删除"
                >
                  <Trash2 className="w-3 h-3 text-red-400" />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {recordings.length === 0 && !isRecording && (
        <div className="text-xs text-[var(--color-text-muted)] py-2 text-center border border-dashed border-[var(--color-border)] rounded-lg">
          暂无录制。启动实例后点击"录制"开始录制操作。
        </div>
      )}
    </div>
  )
}
