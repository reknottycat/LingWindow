import { getCurrentWindow } from '@tauri-apps/api/window'
import { StrictMode, useEffect, useRef, useState } from 'react'
import { createRoot } from 'react-dom/client'
import '@/shared/theme.css'
import { syncTrackingFromPrefs } from '@/shared/analytics'
import {
  EVENT,
  formatCmdError,
  type HotkeyInfo,
  mainWindowShow,
  overlayConfigGet,
  overlayConfigPatch,
  overlayHide,
  overlaySetMode,
  sessionPreviewRendered,
  sessionStatus,
  shortcutsGet,
} from '@/shared/ipc'
import { initLocale } from '@/shared/locale'
import { type OverlayConfig, OverlayDisplayMode, OverlayMode, TaskStatus, TranslationState } from '@/shared/types'
import { useBroadcastSnapshot } from '@/shared/useBroadcastSnapshot'
import { useTranscriptStream } from '@/shared/useTranscriptStream'
import { OverlayToolbar, ResizeHandle } from './OverlayToolbar'
import { StreamView } from './StreamView'
import { DirectionControls } from './DirectionControls'

const DEFAULT_CONFIG: OverlayConfig = {
  enable: true,
  mode: OverlayMode.Edit,
  displayMode: OverlayDisplayMode.Bilingual,
  alwaysOnTop: true,
  opacity: 60,
  fontSize: 22,
  fontColor: '#ffffff',
  bgColor: '#101018',
  maxLines: 6,
  glow: false,
  glowSpeed: 0,
  glowWidth: 2,
}

void initLocale()
void syncTrackingFromPrefs()

function useCapturing(): boolean {
  const [capturing, setCapturing] = useState(false)

  useBroadcastSnapshot<TaskStatus>(
    EVENT.taskState,
    () => sessionStatus().then((info) => info.status),
    (status) => setCapturing(status === TaskStatus.Capturing),
  )

  return capturing
}

function OverlayApp() {
  const { segments } = useTranscriptStream()
  const capturing = useCapturing()
  const acknowledged = useRef<string | null>(null)
  const preview = segments
    .filter((segment) => segment.id.startsWith('preview:') && segment.translationState === TranslationState.Done)
    .at(-1)
  useEffect(() => {
    if (!preview || acknowledged.current === preview.id) return
    let cancelled = false
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => {
        void getCurrentWindow()
          .isVisible()
          .then((visible) => {
            if (!visible || cancelled) return
            acknowledged.current = preview.id
            void sessionPreviewRendered(preview.id)
          })
          .catch(() => {})
      })
    })
    return () => {
      cancelled = true
      cancelAnimationFrame(frame)
    }
  }, [preview])
  const [config, setConfig] = useState<OverlayConfig>(DEFAULT_CONFIG)
  const [shortcuts, setShortcuts] = useState<HotkeyInfo[]>([])
  const [busy, setBusy] = useState(false)
  const pending = useRef(false)
  const [error, setError] = useState<string | null>(null)

  useBroadcastSnapshot<OverlayConfig>(EVENT.overlayState, overlayConfigGet, setConfig)
  useBroadcastSnapshot<HotkeyInfo[]>(EVENT.overlayShortcuts, shortcutsGet, setShortcuts)

  async function perform(action: () => Promise<unknown>) {
    if (pending.current) return
    pending.current = true
    setBusy(true)
    setError(null)
    try {
      await action()
    } catch (err) {
      setError(formatCmdError(err))
    } finally {
      pending.current = false
      setBusy(false)
    }
  }

  return (
    <div className="fixed inset-0 flex flex-col overflow-hidden rounded-[var(--radius-panel)]">
      <OverlayToolbar
        directionControls={<DirectionControls onError={setError} />}
        config={config}
        shortcuts={shortcuts}
        busy={busy}
        onShowMain={() => void perform(mainWindowShow)}
        onHide={() => void perform(overlayHide)}
        onLock={() => void perform(() => overlaySetMode(OverlayMode.Locked))}
        onDisplayMode={(displayMode) => void perform(() => overlayConfigPatch({ displayMode }))}
        onError={setError}
      />
      {error && (
        <div role="alert" className="shrink-0 bg-red-950 px-3 py-1 text-xs text-white">
          {error}
        </div>
      )}
      <div
        className="min-h-0 flex-1"
        onDoubleClick={() => {
          if (config.mode === OverlayMode.Edit) void perform(mainWindowShow)
        }}
      >
        <StreamView
          segments={segments}
          displayMode={config.displayMode}
          fontSize={config.fontSize}
          fontColor={config.fontColor}
          opacity={config.opacity}
          bgColor={config.bgColor}
          glow={config.glow}
          glowSpeed={config.glowSpeed}
          glowWidth={config.glowWidth}
          streaming={capturing}
        />
      </div>
      {config.mode === OverlayMode.Edit && <ResizeHandle onError={setError} />}
    </div>
  )
}

const root = document.getElementById('overlay-root')
if (!root) throw new Error('#overlay-root 未找到')

createRoot(root).render(
  <StrictMode>
    <OverlayApp />
  </StrictMode>,
)
