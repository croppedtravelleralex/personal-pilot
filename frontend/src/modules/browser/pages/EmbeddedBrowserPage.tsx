import { useEffect, useState, useRef, useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'
import { Monitor, Plus, X, Maximize2, Minimize2, Columns } from 'lucide-react'
import { Select, Button, toast } from '../../../shared/components'
import {
  BrowserProfileList, EmbeddedBrowserStart, EmbeddedBrowserStop,
  EmbeddedBrowserFocus,
} from '../../../wailsjs/go/main/App'
import { EventsOn, EventsOff } from '../../../wailsjs/runtime'
import type { browser } from '../../../wailsjs/go/models'

export function EmbeddedBrowserPage() {
  const [profiles, setProfiles] = useState<browser.Profile[]>([])
  const [selectedProfileId, setSelectedProfileId] = useState('')
  const [isLaunching, setIsLaunching] = useState(false)
  const [activeTab, setActiveTab] = useState<string>('')
  const [fullscreen, setFullscreen] = useState(false)
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<Map<string, HTMLCanvasElement>>(new Map())
  const imageRef = useRef<Map<string, HTMLImageElement>>(new Map())

  const [searchParams, setSearchParams] = useSearchParams()

  useEffect(() => {
    BrowserProfileList()
      .then(list => setProfiles(list || []))
      .catch(() => {})
  }, [])

  // Auto-launch when navigated with ?launch=profileId
  useEffect(() => {
    const launchId = searchParams.get('launch')
    if (launchId && profiles.length > 0 && !isLaunching) {
      setSearchParams({}, { replace: true })
      setSelectedProfileId(launchId)
      setTimeout(async () => {
        setIsLaunching(true)
        try {
          await EmbeddedBrowserStart(launchId)
          setActiveTab(launchId)
          toast.success('浏览器已启动')
          const list = await BrowserProfileList()
          setProfiles(list || [])
          // Listen for frames
          registerFrameListener(launchId)
        } catch (e: any) {
          toast.error(`启动失败: ${e?.message || e}`)
        } finally {
          setIsLaunching(false)
        }
      }, 500)
    }
  }, [profiles, searchParams])

  // Listen for frame events
  const registerFrameListener = useCallback((profileId: string) => {
    EventsOff('embedded:frame:' + profileId)
    EventsOn('embedded:frame:' + profileId, (data: string) => {
      if (!data) return
      let img = imageRef.current.get(profileId)
      if (!img) {
        img = new Image()
        img.onload = () => {
          const canvas = canvasRef.current.get(profileId)
          if (!canvas || !img) return
          const ctx = canvas.getContext('2d')
          if (!ctx) return
          canvas.width = img.naturalWidth || 1280
          canvas.height = img.naturalHeight || 720
          ctx.drawImage(img, 0, 0)
        }
        imageRef.current.set(profileId, img)
      }
      img.src = `data:image/jpeg;base64,${data}`
    })
  }, [])

  // Listen for exit events
  useEffect(() => {
    EventsOn('embedded:exited', (data: any) => {
      if (data?.profileId) {
        if (activeTab === data.profileId) setActiveTab('')
        EventsOff('embedded:frame:' + data.profileId)
        toast.warning(`实例已退出`)
      }
    })
    EventsOn('embedded:failed', (data: any) => {
      toast.error(`嵌入失败: ${data?.error || '未知错误'}`)
    })
    return () => {
      EventsOff('embedded:exited')
      EventsOff('embedded:failed')
    }
  }, [activeTab])

  const handleLaunch = async () => {
    if (!selectedProfileId) return
    setIsLaunching(true)
    try {
      await EmbeddedBrowserStart(selectedProfileId)
      setActiveTab(selectedProfileId)
      toast.success('浏览器已启动')
      const list = await BrowserProfileList()
      setProfiles(list || [])
      registerFrameListener(selectedProfileId)
    } catch (e: any) {
      toast.error(`启动失败: ${e?.message || e}`)
    } finally {
      setIsLaunching(false)
    }
  }

  const handleStop = async (profileId: string) => {
    try {
      await EmbeddedBrowserStop(profileId)
      EventsOff('embedded:frame:' + profileId)
      imageRef.current.delete(profileId)
      canvasRef.current.delete(profileId)
      if (activeTab === profileId) setActiveTab('')
      toast.success('实例已停止')
    } catch (e: any) {
      toast.error(`停止失败: ${e?.message || e}`)
    }
  }

  const handleTabClick = (profileId: string) => {
    setActiveTab(profileId)
    EmbeddedBrowserFocus(profileId)
  }

  const runningInstances = profiles.filter(p => p.running)

  return (
    <div className="flex flex-col h-full" style={{ height: 'calc(100vh - 64px)' }}>
      {/* Toolbar */}
      <div className="flex items-center gap-3 px-4 py-2 border-b border-[var(--color-border)] bg-[var(--color-bg)] shrink-0">
        <Monitor className="w-4 h-4 text-[var(--color-text-muted)]" />
        <span className="text-sm font-medium text-[var(--color-text)]">嵌入浏览器</span>
        <div className="flex-1" />
        <Select
          value={selectedProfileId}
          onChange={e => setSelectedProfileId(e.target.value)}
          options={[
            { value: '', label: '选择实例...' },
            ...profiles.filter(p => !p.running).map(p => ({
              value: p.profileId,
              label: p.profileName || p.profileId,
            })),
          ]}
        />
        <Button size="sm" variant="primary" onClick={handleLaunch} loading={isLaunching} disabled={!selectedProfileId}>
          <Plus className="w-3.5 h-3.5" /> 启动
        </Button>
        <Button size="sm" variant="secondary" onClick={() => setFullscreen(!fullscreen)}>
          {fullscreen ? <Minimize2 className="w-3.5 h-3.5" /> : <Maximize2 className="w-3.5 h-3.5" />}
        </Button>
      </div>

      {/* Instance tabs */}
      {runningInstances.length > 0 && (
        <div className="flex items-center gap-1 px-3 py-1.5 border-b border-[var(--color-border)] bg-[var(--color-bg-subtle)] shrink-0 overflow-x-auto">
          {runningInstances.map(p => (
            <button
              key={p.profileId}
              className={`flex items-center gap-1.5 px-3 py-1 rounded text-xs transition-colors ${
                activeTab === p.profileId
                  ? 'bg-[var(--color-bg)] text-[var(--color-text)] font-medium shadow-sm'
                  : 'text-[var(--color-text-muted)] hover:bg-[var(--color-bg-hover)]'
              }`}
              onClick={() => handleTabClick(p.profileId)}
            >
              <span className="w-2 h-2 rounded-full bg-green-400" />
              {p.profileName || p.profileId}
              <button
                className="ml-1 p-0.5 hover:bg-red-100 rounded"
                onClick={e => { e.stopPropagation(); handleStop(p.profileId) }}
              >
                <X className="w-3 h-3" />
              </button>
            </button>
          ))}
        </div>
      )}

      {/* Screencast container */}
      <div ref={containerRef} className="flex-1 relative bg-gray-900">
        {runningInstances.filter(p => p.profileId === activeTab).map(p => (
          <canvas
            key={p.profileId}
            ref={el => { if (el) canvasRef.current.set(p.profileId, el) }}
            className="absolute inset-0 w-full h-full"
            style={{ objectFit: 'contain' }}
          />
        ))}
        {(!activeTab || !runningInstances.find(p => p.profileId === activeTab)) && (
          <div className="absolute inset-0 flex items-center justify-center text-[var(--color-text-muted)]">
            <div className="text-center space-y-3">
              <Columns className="w-10 h-10 mx-auto opacity-20" />
              <p className="text-sm">选择实例并点击"启动"</p>
              <p className="text-xs opacity-60">Chrome 以无头模式运行，通过 CDP 投屏显示画面</p>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
