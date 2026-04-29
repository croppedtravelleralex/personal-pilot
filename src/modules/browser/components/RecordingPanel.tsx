import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import {
  ChevronDown, ChevronUp, Play, Square, Trash2, Circle, Zap, Pencil, Search, X,
  Info, WifiOff, Download, Upload, Copy as CopyIcon, RotateCcw,
} from 'lucide-react'
import { Button, FormItem, Input, Select, Textarea, toast } from '../../../shared/components'
import type { PlaybackEventPayload, PlaybackProgressPayload, RecordingSummary, VariationConfig } from '../types'
import {
  startRecording, stopRecording, fetchRecordingSummaries, deleteRecording,
  playRecording, stopPlayback, quickRecord, cleanupRecordingSessions,
  renameRecording, onPlaybackEvents, fetchRecordingStatus, exportRecording,
  importRecording, copyRecording,
} from '../api'
import { RecordingDetailModal } from './RecordingDetailModal'

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

const SEARCH_DEBOUNCE_MS = 300
const LIST_PAGE_SIZES = [25, 50, 100]
const PLAYBACK_OPTION_LIMIT = 200

interface PlaybackRequest {
  profileId: string
  recordingId: string
  variation: VariationConfig
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function RecordingPanel({ profileId, isRunning }: RecordingPanelProps) {
  const [recordings, setRecordings] = useState<RecordingSummary[]>([])
  const [loading, setLoading] = useState(false)
  const [isRecording, setIsRecording] = useState(false)
  const [isPlaying, setIsPlaying] = useState(false)
  const [variation, setVariation] = useState<VariationConfig>(DEFAULT_VARIATION)
  const [advancedOpen, setAdvancedOpen] = useState(false)
  const [playRecordingId, setPlayRecordingId] = useState('')
  const [searchQuery, setSearchQuery] = useState('')
  const [debouncedSearchQuery, setDebouncedSearchQuery] = useState('')
  const [listPage, setListPage] = useState(1)
  const [listPageSize, setListPageSize] = useState(25)
  const [recordingStartTime, setRecordingStartTime] = useState<number | null>(null)
  const [elapsedSec, setElapsedSec] = useState(0)
  const [detailRecordingId, setDetailRecordingId] = useState<string | null>(null)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editName, setEditName] = useState('')
  const [toolLoadingId, setToolLoadingId] = useState<string | null>(null)
  const [exportJson, setExportJson] = useState<{ name: string; text: string } | null>(null)
  const [importOpen, setImportOpen] = useState(false)
  const [importName, setImportName] = useState('')
  const [importPayload, setImportPayload] = useState('')
  const [importing, setImporting] = useState(false)
  const [lastPlaybackRequest, setLastPlaybackRequest] = useState<PlaybackRequest | null>(null)
  const [playbackError, setPlaybackError] = useState('')
  const [playbackProgress, setPlaybackProgress] = useState<PlaybackProgressPayload | null>(null)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Load recordings list
  const loadRecordings = async () => {
    try {
      const list = await fetchRecordingSummaries()
      setRecordings(list || [])
    } catch {
      // ignore errors
    }
  }

  const syncRecordingStatus = useCallback(async () => {
    if (!profileId) {
      setIsRecording(false)
      setRecordingStartTime(null)
      return
    }
    try {
      const status = await fetchRecordingStatus()
      const active = status.profileIds.includes(profileId)
      setIsRecording(active)
      setRecordingStartTime(prev => active ? (prev ?? Date.now()) : null)
    } catch {
      // Status sync is best-effort; direct start/stop errors still surface.
    }
  }, [profileId])

  const isPlaybackPayloadForProfile = useCallback((data: { profileId?: string }) => {
    return data?.profileId === profileId || !data?.profileId
  }, [profileId])

  const handlePlaybackProgress = useCallback((data: PlaybackProgressPayload) => {
    if (!isPlaybackPayloadForProfile(data)) return
    setPlaybackProgress(data)
    if (data.status === 'completed' || data.status === 'failed') {
      setIsPlaying(false)
    } else {
      setIsPlaying(true)
    }
  }, [isPlaybackPayloadForProfile])

  const handlePlaybackCompleted = useCallback((data: PlaybackEventPayload) => {
    if (!isPlaybackPayloadForProfile(data)) return
    setIsPlaying(false)
    setPlaybackError('')
    setPlaybackProgress(prev => prev ? { ...prev, percent: 100, status: 'completed' } : prev)
  }, [isPlaybackPayloadForProfile])

  const handlePlaybackFailed = useCallback((data: PlaybackEventPayload) => {
    if (!isPlaybackPayloadForProfile(data)) return
    const error = data?.error || 'Playback failed'
    setIsPlaying(false)
    setPlaybackError(error)
    setPlaybackProgress(prev => prev ? { ...prev, status: 'failed' } : prev)
    toast.error(`鍥炴斁澶辫触: ${error}`)
  }, [isPlaybackPayloadForProfile])

  useEffect(() => {
    loadRecordings()
  }, [])

  useEffect(() => {
    void syncRecordingStatus()
    const timer = setInterval(() => { void syncRecordingStatus() }, 5000)
    return () => clearInterval(timer)
  }, [syncRecordingStatus])

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearchQuery(searchQuery.trim()), SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [searchQuery])

  // Recording timer
  useEffect(() => {
    if (isRecording && recordingStartTime) {
      timerRef.current = setInterval(() => {
        setElapsedSec(Math.floor((Date.now() - recordingStartTime) / 1000))
      }, 200)
    } else {
      if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null }
      if (!isRecording) setElapsedSec(0)
    }
    return () => { if (timerRef.current) { clearInterval(timerRef.current); timerRef.current = null } }
  }, [isRecording, recordingStartTime])

  useEffect(() => {
    return onPlaybackEvents({
      onProgress: handlePlaybackProgress,
      onCompleted: handlePlaybackCompleted,
      onFailed: handlePlaybackFailed,
    })
  }, [handlePlaybackCompleted, handlePlaybackFailed, handlePlaybackProgress])

  // Start recording
  const handleStartRecording = async () => {
    if (!profileId) return
    setLoading(true)
    try {
      await startRecording(profileId)
      setIsRecording(true)
      setRecordingStartTime(Date.now())
      setElapsedSec(0)
      await syncRecordingStatus()
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
      await stopRecording(profileId, name)
      setIsRecording(false)
      setRecordingStartTime(null)
      await syncRecordingStatus()
      toast.success('录制已保存')
      await loadRecordings()
    } catch (e: any) {
      toast.error(`停止录制失败: ${e?.message || e}`)
    } finally {
      setLoading(false)
    }
  }

  // Quick record (auto-nurturing)
  const handleQuickRecord = async () => {
    if (!profileId) return
    setLoading(true)
    try {
      toast.success('养号录制已开始 (~60秒)')
      const rec = await quickRecord(profileId)
      if (rec) toast.success(`养号录制完成: ${rec.name}`)
      await loadRecordings()
    } catch (e: any) {
      toast.error(`养号录制失败: ${e?.message || e}`)
    } finally {
      setLoading(false)
    }
  }

  // Delete recording
  const handleDelete = async (id: string) => {
    try {
      await deleteRecording(id)
      toast.success('录制已删除')
      await loadRecordings()
      if (playRecordingId === id) setPlayRecordingId('')
      if (detailRecordingId === id) setDetailRecordingId(null)
    } catch (e: any) {
      toast.error(`删除失败: ${e?.message || e}`)
    }
  }

  const handleExportJson = async (recording: RecordingSummary) => {
    setToolLoadingId(`export:${recording.id}`)
    try {
      const bundle = await exportRecording(recording.id)
      if (!bundle) throw new Error('导出接口不可用')
      setExportJson({
        name: recording.name,
        text: JSON.stringify(bundle, null, 2),
      })
      toast.success('导出 JSON 已生成')
    } catch (e) {
      toast.error(`导出失败: ${getErrorMessage(e)}`)
    } finally {
      setToolLoadingId(null)
    }
  }

  const handleCopyTemplate = async (recording: RecordingSummary) => {
    const name = `${recording.name} 副本`
    setToolLoadingId(`copy:${recording.id}`)
    try {
      const copied = await copyRecording(recording.id, name)
      await loadRecordings()
      if (copied?.id) setPlayRecordingId(copied.id)
      toast.success('模板副本已创建')
    } catch (e) {
      toast.error(`复制失败: ${getErrorMessage(e)}`)
    } finally {
      setToolLoadingId(null)
    }
  }

  const handleCopyExportText = async () => {
    if (!exportJson) return
    try {
      await navigator.clipboard.writeText(exportJson.text)
      toast.success('JSON 已复制')
    } catch (e) {
      toast.error(`复制失败: ${getErrorMessage(e)}`)
    }
  }

  const handleImportJson = async () => {
    const payload = importPayload.trim()
    const name = importName.trim()
    if (!name) {
      toast.error('请输入导入名称')
      return
    }
    if (!payload) {
      toast.error('请输入 JSON')
      return
    }
    try {
      JSON.parse(payload)
    } catch {
      toast.error('JSON 格式无效')
      return
    }

    setImporting(true)
    try {
      const recording = await importRecording(payload, name)
      await loadRecordings()
      if (recording?.id) setPlayRecordingId(recording.id)
      setImportOpen(false)
      setImportName('')
      setImportPayload('')
      toast.success('录制已导入')
    } catch (e) {
      toast.error(`导入失败: ${getErrorMessage(e)}`)
    } finally {
      setImporting(false)
    }
  }

  // Rename recording
  const handleStartRename = (r: RecordingSummary) => {
    setEditingId(r.id)
    setEditName(r.name)
  }
  const handleSaveRename = async () => {
    if (!editingId || !editName.trim()) return
    try {
      await renameRecording(editingId, editName.trim())
      setRecordings(prev => prev.map(r => r.id === editingId ? { ...r, name: editName.trim() } : r))
      toast.success('名称已更新')
    } catch (e: any) {
      toast.error(`重命名失败: ${e?.message || e}`)
    }
    setEditingId(null)
  }
  const handleCancelRename = () => {
    setEditingId(null)
    setEditName('')
  }

  const runPlayback = async (request: PlaybackRequest) => {
    setLoading(true)
    setPlaybackError('')
    setLastPlaybackRequest(request)
    const selected = recordings.find(r => r.id === request.recordingId)
    setPlaybackProgress({
      profileId: request.profileId,
      recordingId: request.recordingId,
      eventIndex: 0,
      eventTotal: selected ? getEventCount(selected) : 0,
      percent: 0,
      elapsedMs: 0,
      status: 'starting',
    })
    try {
      await playRecording(request.profileId, request.recordingId, request.variation)
      setIsPlaying(true)
      toast.success('回放已开始')
    } catch (e) {
      const error = getErrorMessage(e)
      setIsPlaying(false)
      setPlaybackError(error)
      setPlaybackProgress(prev => prev ? { ...prev, status: 'failed' } : prev)
      toast.error(`回放失败: ${error}`)
    } finally {
      setLoading(false)
    }
  }

  // Start playback
  const handlePlay = async () => {
    if (!profileId || !playRecordingId) return
    await runPlayback({
      profileId,
      recordingId: playRecordingId,
      variation: { ...variation },
    })
  }

  const handleRetryPlayback = async () => {
    if (!lastPlaybackRequest) return
    setPlayRecordingId(lastPlaybackRequest.recordingId)
    await runPlayback(lastPlaybackRequest)
  }

  // Stop playback
  const handleStopPlayback = async () => {
    if (!profileId) return
    try {
      await stopPlayback(profileId)
      setIsPlaying(false)
      toast.success('回放已停止')
    } catch (e: any) {
      toast.error(`停止回放失败: ${e?.message || e}`)
    }
  }

  // Cleanup stale sessions
  const handleCleanup = async () => {
    try {
      await cleanupRecordingSessions()
      toast.success('卡死会话已清理')
      await loadRecordings()
    } catch (e: any) {
      toast.error(`清理失败: ${e?.message || e}`)
    }
  }

  const hasRunningProfile = !!profileId && isRunning
  const getEventCount = (recording: RecordingSummary) => recording.eventCount ?? recording.events?.length ?? 0

  // Filter recordings by search query
  const filteredRecordings = useMemo(() => {
    if (!debouncedSearchQuery) return recordings
    const q = debouncedSearchQuery.toLowerCase()
    return recordings.filter(r =>
      r.name.toLowerCase().includes(q) ||
      r.description?.toLowerCase().includes(q) ||
      r.id.toLowerCase().includes(q)
    )
  }, [recordings, debouncedSearchQuery])

  const totalListPages = Math.max(1, Math.ceil(filteredRecordings.length / listPageSize))
  const pagedRecordings = useMemo(() => {
    const start = (listPage - 1) * listPageSize
    return filteredRecordings.slice(start, start + listPageSize)
  }, [filteredRecordings, listPage, listPageSize])
  const playbackRecordings = useMemo(() => {
    const limited = filteredRecordings.slice(0, PLAYBACK_OPTION_LIMIT)
    if (!playRecordingId || limited.some(r => r.id === playRecordingId)) return limited
    const selected = recordings.find(r => r.id === playRecordingId)
    return selected ? [selected, ...limited] : limited
  }, [filteredRecordings, playRecordingId, recordings])

  useEffect(() => {
    setListPage(1)
  }, [debouncedSearchQuery, listPageSize])

  useEffect(() => {
    if (listPage > totalListPages) setListPage(totalListPages)
  }, [listPage, totalListPages])

  // Format elapsed time
  const formatElapsed = (sec: number) => {
    const m = Math.floor(sec / 60)
    const s = sec % 60
    return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
  }
  const formatElapsedMs = (ms: number) => `${(Math.max(0, ms) / 1000).toFixed(1)}s`
  const playbackPercent = playbackProgress
    ? Math.max(0, Math.min(100, Math.round(playbackProgress.percent || 0)))
    : 0

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h4 className="text-sm font-semibold text-[var(--color-text)]">行为录制与回放</h4>
        <div className="flex items-center gap-2">
          <Button
            size="sm"
            variant="secondary"
            className="h-7 px-2"
            onClick={() => setImportOpen(true)}
            title="导入录制 JSON"
          >
            <Upload className="w-3 h-3" />
            导入
          </Button>
          <button
            className="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-accent)] transition-colors flex items-center gap-1"
            onClick={handleCleanup}
            title="清理卡死的录制会话"
          >
            <WifiOff className="w-3 h-3" />
            清理会话
          </button>
        </div>
      </div>

      {/* Recording controls */}
      <div className="flex items-center gap-2 flex-wrap">
        {!isRecording ? (
          <>
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
            <Button
              size="sm"
              variant="secondary"
              onClick={handleQuickRecord}
              loading={loading}
              disabled={!hasRunningProfile}
              title={!hasRunningProfile ? '请先启动浏览器实例' : '自动养号录制 (~60秒)'}
            >
              <Zap className="w-3.5 h-3.5" />
              养号
            </Button>
          </>
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
          <span className="text-xs text-red-500 animate-pulse flex items-center gap-1.5">
            <span className="inline-block w-2 h-2 rounded-full bg-red-500" />
            录制中 {formatElapsed(elapsedSec)}
          </span>
        )}
        {isPlaying && (
          <span className="text-xs text-[var(--color-accent)] animate-pulse">回放中...</span>
        )}
      </div>

      {/* Search bar */}
      {recordings.length > 0 && !isRecording && (
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-[var(--color-text-muted)]" />
          <Input
            className="pl-8 text-xs h-8"
            placeholder="搜索录制..."
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
          />
          {searchQuery && (
            <button
              className="absolute right-2 top-1/2 -translate-y-1/2 text-[var(--color-text-muted)]"
              onClick={() => setSearchQuery('')}
            >
              <X className="w-3 h-3" />
            </button>
          )}
        </div>
      )}

      {/* Playback section */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <Select
            value={playRecordingId}
            onChange={e => setPlayRecordingId(e.target.value)}
            options={[
              { value: '', label: '选择录制...' },
              ...playbackRecordings.map(r => ({
                value: r.id,
                label: `${r.name} (${(r.durationMs / 1000).toFixed(1)}s, ${getEventCount(r)} 事件)`,
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

        {(playbackProgress || playbackError) && (
          <div className="space-y-1.5 rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-subtle)] px-3 py-2 text-xs">
            {playbackProgress && (
              <>
                <div className="flex items-center justify-between gap-2 text-[var(--color-text-muted)]">
                  <span>
                    回放进度 {playbackProgress.eventIndex}/{playbackProgress.eventTotal || 0}
                    {playbackProgress.status ? ` · ${playbackProgress.status}` : ''}
                  </span>
                  <span className="tabular-nums">{playbackPercent}%</span>
                </div>
                <div className="h-1.5 rounded-full bg-[var(--color-border)] overflow-hidden">
                  <div
                    className="h-full bg-[var(--color-accent)] transition-all"
                    style={{ width: `${playbackPercent}%` }}
                  />
                </div>
                <div className="text-[10px] text-[var(--color-text-muted)]">
                  elapsed {formatElapsedMs(playbackProgress.elapsedMs)}
                </div>
              </>
            )}
            {playbackError && lastPlaybackRequest && (
              <div className="flex items-center justify-between gap-2">
                <span className="text-red-500 truncate">失败: {playbackError}</span>
                <Button
                  size="sm"
                  variant="secondary"
                  className="h-7 px-2 shrink-0"
                  onClick={handleRetryPlayback}
                  loading={loading}
                  disabled={!hasRunningProfile}
                >
                  <RotateCcw className="w-3 h-3" />
                  重试
                </Button>
              </div>
            )}
          </div>
        )}

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
      {filteredRecordings.length > 0 && (
        <div className="space-y-1.5">
          <div className="flex items-center justify-between gap-2 text-xs text-[var(--color-text-muted)]">
            <span className="font-medium">
              已保存录制 ({filteredRecordings.length}{filteredRecordings.length !== recordings.length ? ` / 共 ${recordings.length}` : ''})
            </span>
            <div className="flex items-center gap-1.5 shrink-0">
              <Select
                className="w-20 h-7 px-2 text-xs"
                value={String(listPageSize)}
                onChange={e => setListPageSize(Number(e.target.value))}
                options={LIST_PAGE_SIZES.map(size => ({ value: String(size), label: `${size}/页` }))}
              />
              <button
                type="button"
                className="px-2 py-1 rounded border border-[var(--color-border)] disabled:opacity-40"
                disabled={listPage <= 1}
                onClick={() => setListPage(page => Math.max(1, page - 1))}
              >
                上一页
              </button>
              <span className="tabular-nums">{listPage}/{totalListPages}</span>
              <button
                type="button"
                className="px-2 py-1 rounded border border-[var(--color-border)] disabled:opacity-40"
                disabled={listPage >= totalListPages}
                onClick={() => setListPage(page => Math.min(totalListPages, page + 1))}
              >
                下一页
              </button>
            </div>
          </div>
          <div className="max-h-64 overflow-y-auto space-y-1.5">
            {pagedRecordings.map(r => (
              <div
                key={r.id}
                className="flex items-center justify-between px-2.5 py-1.5 rounded bg-[var(--color-bg-subtle)] text-xs group"
              >
                <div className="flex-1 min-w-0">
                  {editingId === r.id ? (
                    <div className="flex items-center gap-1.5">
                      <Input
                        className="h-7 text-xs flex-1"
                        value={editName}
                        onChange={e => setEditName(e.target.value)}
                        onKeyDown={e => { if (e.key === 'Enter') handleSaveRename(); if (e.key === 'Escape') handleCancelRename() }}
                        autoFocus
                      />
                      <button className="text-xs text-[var(--color-accent)] hover:underline shrink-0" onClick={handleSaveRename}>保存</button>
                      <button className="text-xs text-[var(--color-text-muted)] hover:underline shrink-0" onClick={handleCancelRename}>取消</button>
                    </div>
                  ) : (
                    <>
                      <div className="font-medium text-[var(--color-text)] truncate">{r.name}</div>
                      <div className="text-[var(--color-text-muted)]">
                        {(r.durationMs / 1000).toFixed(1)}s · {getEventCount(r)} 事件 · {r.viewportW}x{r.viewportH}
                      </div>
                    </>
                  )}
                </div>
                <div className="flex items-center gap-0.5 ml-2 shrink-0">
                  <button
                    className="p-1 hover:bg-[var(--color-bg-hover)] rounded opacity-60 group-hover:opacity-100 transition-opacity"
                    onClick={() => setDetailRecordingId(r.id)}
                    title="查看详情"
                  >
                    <Info className="w-3 h-3 text-[var(--color-text-muted)]" />
                  </button>
                  <button
                    className="p-1 hover:bg-[var(--color-bg-hover)] rounded opacity-60 group-hover:opacity-100 transition-opacity disabled:opacity-40"
                    onClick={() => handleExportJson(r)}
                    disabled={toolLoadingId === `export:${r.id}`}
                    title="导出 JSON"
                  >
                    <Download className="w-3 h-3 text-[var(--color-text-muted)]" />
                  </button>
                  <button
                    className="p-1 hover:bg-[var(--color-bg-hover)] rounded opacity-60 group-hover:opacity-100 transition-opacity disabled:opacity-40"
                    onClick={() => handleCopyTemplate(r)}
                    disabled={toolLoadingId === `copy:${r.id}`}
                    title="复制模板"
                  >
                    <CopyIcon className="w-3 h-3 text-[var(--color-text-muted)]" />
                  </button>
                  <button
                    className="p-1 hover:bg-[var(--color-bg-hover)] rounded opacity-60 group-hover:opacity-100 transition-opacity"
                    onClick={() => handleStartRename(r)}
                    title="重命名"
                  >
                    <Pencil className="w-3 h-3 text-[var(--color-text-muted)]" />
                  </button>
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
        </div>
      )}

      {recordings.length === 0 && !isRecording && (
        <div className="text-xs text-[var(--color-text-muted)] py-2 text-center border border-dashed border-[var(--color-border)] rounded-lg">
          暂无录制。启动实例后点击"录制"开始录制操作，或点击"养号"自动生成浏览录制。
        </div>
      )}

      {exportJson && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/20" onClick={() => setExportJson(null)}>
          <div className="bg-[var(--color-bg)] border border-[var(--color-border)] rounded-xl p-5 shadow-2xl w-[560px] max-w-[92vw]" onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-semibold text-[var(--color-text)]">导出 JSON: {exportJson.name}</h3>
              <button className="p-1.5 hover:bg-[var(--color-bg-hover)] rounded" onClick={() => setExportJson(null)}>
                <X className="w-4 h-4 text-[var(--color-text-muted)]" />
              </button>
            </div>
            <Textarea
              className="h-72 font-mono text-xs"
              value={exportJson.text}
              readOnly
            />
            <div className="mt-3 flex justify-end gap-2">
              <Button size="sm" variant="secondary" onClick={() => setExportJson(null)}>关闭</Button>
              <Button size="sm" onClick={handleCopyExportText}>
                <CopyIcon className="w-3.5 h-3.5" />
                复制
              </Button>
            </div>
          </div>
        </div>
      )}

      {importOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/20" onClick={() => setImportOpen(false)}>
          <div className="bg-[var(--color-bg)] border border-[var(--color-border)] rounded-xl p-5 shadow-2xl w-[560px] max-w-[92vw]" onClick={e => e.stopPropagation()}>
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-semibold text-[var(--color-text)]">导入录制 JSON</h3>
              <button className="p-1.5 hover:bg-[var(--color-bg-hover)] rounded" onClick={() => setImportOpen(false)}>
                <X className="w-4 h-4 text-[var(--color-text-muted)]" />
              </button>
            </div>
            <div className="space-y-3">
              <FormItem label="名称">
                <Input
                  value={importName}
                  onChange={e => setImportName(e.target.value)}
                  placeholder="导入后的录制名称"
                />
              </FormItem>
              <FormItem label="JSON">
                <Textarea
                  className="h-72 font-mono text-xs"
                  value={importPayload}
                  onChange={e => setImportPayload(e.target.value)}
                  placeholder="粘贴导出的 JSON"
                />
              </FormItem>
            </div>
            <div className="mt-3 flex justify-end gap-2">
              <Button size="sm" variant="secondary" onClick={() => setImportOpen(false)}>取消</Button>
              <Button size="sm" onClick={handleImportJson} loading={importing}>
                <Upload className="w-3.5 h-3.5" />
                导入
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Recording detail modal */}
      {detailRecordingId && (
        <RecordingDetailModal
          recordingId={detailRecordingId}
          onClose={() => setDetailRecordingId(null)}
          onDelete={(id) => { handleDelete(id); setDetailRecordingId(null) }}
          onChanged={loadRecordings}
        />
      )}
    </div>
  )
}
