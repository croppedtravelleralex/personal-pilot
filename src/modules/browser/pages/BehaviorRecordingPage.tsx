import { useEffect, useState } from 'react'
import { Monitor } from 'lucide-react'
import { Card, Select } from '../../../shared/components'
import { RecordingPanel } from '../components/RecordingPanel'
import { PresetOffsetViewer } from '../components/PresetOffsetViewer'
import { fetchBrowserProfiles } from '../api'
import type { BrowserProfile } from '../types'

export function BehaviorRecordingPage() {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([])
  const [selectedProfileId, setSelectedProfileId] = useState('')

  useEffect(() => {
    fetchBrowserProfiles()
      .then(list => setProfiles(list || []))
      .catch(() => {})
  }, [])

  const selectedProfile = profiles.find(p => p.profileId === selectedProfileId)

  return (
    <div className="max-w-3xl mx-auto space-y-6 p-6">
      <div>
        <h1 className="text-lg font-bold text-[var(--color-text)]">行为录制与回放</h1>
        <p className="text-sm text-[var(--color-text-muted)] mt-1">
          录制真人在浏览器中的操作（点击、输入、滚动），回放时自动添加随机偏移，规避平台风控。
        </p>
      </div>

      <Card>
        <div className="space-y-4">
          <div className="flex items-center gap-3">
            <Monitor className="w-4 h-4 text-[var(--color-text-muted)]" />
            <span className="text-sm font-medium text-[var(--color-text)]">选择浏览器实例</span>
          </div>

          <Select
            value={selectedProfileId}
            onChange={e => setSelectedProfileId(e.target.value)}
            options={[
              { value: '', label: '请选择实例...' },
              ...profiles.map(p => ({
                value: p.profileId,
                label: `${p.profileName || p.profileId}${p.running ? ' (运行中)' : ' (已停止)'}`,
              })),
            ]}
          />

          {selectedProfileId && !selectedProfile?.running && (
            <div className="text-sm text-amber-600 bg-amber-50 border border-amber-200 rounded-lg px-4 py-3">
              该实例尚未启动，请先启动实例后才能录制或回放。
            </div>
          )}
        </div>
      </Card>

      {selectedProfileId && (
        <Card>
          <RecordingPanel
            profileId={selectedProfileId}
            isRunning={selectedProfile?.running ?? false}
          />
        </Card>
      )}

      {/* Behavior template offset viewer - always visible for inspection */}
      <Card>
        <PresetOffsetViewer />
      </Card>
    </div>
  )
}
