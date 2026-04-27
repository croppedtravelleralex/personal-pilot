import { useEffect, useState, useCallback } from 'react'
import { Monitor, RefreshCw, Send } from 'lucide-react'
import { Card, Button } from '../../shared/components'
import { WindowMatrix } from './components/WindowMatrix'
import { LayoutToolbar } from './components/LayoutToolbar'
import { SynchronizerActionFeed } from './components/SynchronizerActionFeed'
import { SynchronizerFiltersPanel } from './components/SynchronizerFiltersPanel'
import { SynchronizerWorkspaceNav } from './components/SynchronizerWorkspaceNav'
import { useSyncStore } from './store'
import { getMockGroups, arrangeWindows, broadcastRefresh } from './api'
import type { SyncGroup, SyncActionFeedEntry, LayoutPreset } from './types'

export function SynchronizerPage() {
  const {
    groups, activeGroupId,
    setGroups, setActiveGroup, addActionToFeed,
    selectAllInGroup, clearSelection,
    updateWindowStatus,
  } = useSyncStore()

  const [broadcastUrl, setBroadcastUrl] = useState('')
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState('')
  const [loading, setLoading] = useState(false)

  // Load groups (mock data for now)
  useEffect(() => {
    const mockGroups = getMockGroups()
    setGroups(mockGroups)
    if (mockGroups.length > 0 && !activeGroupId) {
      setActiveGroup(mockGroups[0].id)
    }
  }, [setGroups, setActiveGroup, activeGroupId])

  const activeGroup = groups.find((g) => g.id === activeGroupId) || groups[0]

  const logAction = useCallback(
    (operation: SyncActionFeedEntry['operation'], windowName: string, detail: string, status: 'ok' | 'error' = 'ok') => {
      addActionToFeed({
        id: crypto.randomUUID(),
        operation,
        windowName,
        detail,
        timestamp: new Date().toISOString(),
        status,
      })
    },
    [addActionToFeed],
  )

  const handleLayoutChange = useCallback(
    (layout: LayoutPreset) => {
      if (!activeGroup) return
      useSyncStore.getState().setGroupLayout(activeGroup.id, layout)
      arrangeWindows(activeGroup.id, layout).catch(() => {
        // Backend not available, but UI updates immediately
      })
      logAction('arrange', activeGroup.name, `布局切换为 ${layout}`)
    },
    [activeGroup, logAction],
  )

  const handleRefreshAll = useCallback(() => {
    if (!activeGroup) return
    setLoading(true)
    broadcastRefresh(activeGroup.id)
      .then(() => logAction('refresh', activeGroup.name, `全部刷新 ${activeGroup.windows.length} 个窗口`))
      .catch(() => logAction('refresh', activeGroup.name, '刷新失败', 'error'))
      .finally(() => setLoading(false))
  }, [activeGroup, logAction])

  const handleFocus = useCallback(
    (id: string) => {
      const win = activeGroup?.windows.find((w) => w.id === id)
      if (win) {
        updateWindowStatus(id, 'running')
        logAction('focus', win.profileName, `聚焦端口 ${win.debugPort}`)
      }
    },
    [activeGroup, logAction, updateWindowStatus],
  )

  const handleClose = useCallback(
    (id: string) => {
      const win = activeGroup?.windows.find((w) => w.id === id)
      if (win) {
        logAction('close', win.profileName, `关闭端口 ${win.debugPort}`)
      }
    },
    [activeGroup, logAction],
  )

  const handleBroadcastNavigate = useCallback(() => {
    if (!broadcastUrl.trim() || !activeGroup) return
    import('./api').then(({ broadcastNavigate }) =>
      broadcastNavigate(activeGroup.id, broadcastUrl.trim())
        .then(() => logAction('navigate', activeGroup.name, broadcastUrl.trim()))
        .catch(() => logAction('navigate', activeGroup.name, '导航失败', 'error'))
    )
    setBroadcastUrl('')
  }, [broadcastUrl, activeGroup, logAction])

  // Filter windows
  const filteredWindows = activeGroup?.windows.filter((w) => {
    if (searchQuery && !w.profileName.toLowerCase().includes(searchQuery.toLowerCase()) &&
        !w.url.toLowerCase().includes(searchQuery.toLowerCase())) {
      return false
    }
    if (statusFilter && w.status !== statusFilter) return false
    return true
  }) || []

  const filteredGroup: SyncGroup | undefined = activeGroup
    ? { ...activeGroup, windows: filteredWindows }
    : undefined

  return (
    <div className="space-y-4 animate-fade-in">
      {/* Header */}
      <Card>
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="inline-flex items-center gap-2 px-2.5 py-1 rounded-full bg-[var(--color-accent-muted)] text-[var(--color-accent)] text-xs font-medium mb-3">
              <Monitor className="w-3.5 h-3.5" /> Sync Windows
            </div>
            <h1 className="text-xl font-semibold text-[var(--color-text-primary)]">
              多窗口同步操控
            </h1>
            <p className="text-sm text-[var(--color-text-secondary)] mt-2">
              批量管理多个浏览器实例，支持同步导航、布局编排和广播操作。
            </p>
          </div>
        </div>

        {/* Broadcast bar */}
        <div className="flex items-center gap-2 mt-4 pt-4 border-t border-[var(--color-border-muted)]">
          <div className="flex-1 flex items-center gap-2">
            <input
              type="text"
              value={broadcastUrl}
              onChange={(e) => setBroadcastUrl(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleBroadcastNavigate()}
              placeholder="输入 URL 广播到所有窗口..."
              className="flex-1 px-3 py-1.5 text-sm rounded-lg border border-[var(--color-border-default)] bg-[var(--color-bg-secondary)] text-[var(--color-text-primary)] placeholder:text-[var(--color-text-muted)] focus:outline-none focus:border-[var(--color-accent)]"
            />
            <Button size="sm" onClick={handleBroadcastNavigate} disabled={!broadcastUrl.trim()}>
              <Send className="w-3.5 h-3.5" /> 广播导航
            </Button>
          </div>
          <Button size="sm" variant="secondary" onClick={handleRefreshAll} loading={loading}>
            <RefreshCw className="w-3.5 h-3.5" /> 全部刷新
          </Button>
        </div>
      </Card>

      {/* Workspace nav */}
      {groups.length > 0 && (
        <SynchronizerWorkspaceNav
          groups={groups}
          activeGroupId={activeGroupId}
          onSelectGroup={setActiveGroup}
        />
      )}

      {/* Filters */}
      <SynchronizerFiltersPanel
        onSearch={setSearchQuery}
        onFilterStatus={setStatusFilter}
      />

      {/* Layout toolbar */}
      {activeGroup && (
        <LayoutToolbar
          currentLayout={activeGroup.layout}
          selectedCount={useSyncStore.getState().selectedWindowIds.size}
          totalCount={activeGroup.windows.length}
          onLayoutChange={handleLayoutChange}
          onSelectAll={() => selectAllInGroup(activeGroup.id)}
          onClearSelection={clearSelection}
          onRefreshAll={handleRefreshAll}
        />
      )}

      {/* Window matrix + action feed */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <div className="lg:col-span-2">
          {filteredGroup ? (
            <WindowMatrix
              group={filteredGroup}
              onFocus={handleFocus}
              onClose={handleClose}
            />
          ) : (
            <Card>
              <div className="text-center py-16 text-[var(--color-text-muted)]">
                <Monitor className="w-10 h-10 mx-auto mb-3 opacity-30" />
                <p className="text-sm font-medium">暂无同步分组</p>
                <p className="text-xs mt-1">启动多个浏览器实例后，它们会自动出现在这里。</p>
                <p className="text-xs">你可以在实例列表中将它们分配到同一分组。</p>
              </div>
            </Card>
          )}
        </div>

        {/* Action feed sidebar */}
        <Card className="lg:col-span-1">
          <h3 className="text-sm font-semibold text-[var(--color-text-primary)] mb-3">
            操作记录
          </h3>
          <SynchronizerActionFeed />
        </Card>
      </div>
    </div>
  )
}
