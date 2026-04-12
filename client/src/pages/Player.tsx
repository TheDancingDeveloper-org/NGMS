import { useEffect, useRef, useState, useCallback } from 'react'
import { useParams, useNavigate, useLocation } from 'react-router-dom'
import { ArrowLeft, Maximize, Minimize, RotateCcw, RotateCw, PictureInPicture2, Play, Pause } from 'lucide-react'
import Hls from 'hls.js'
import { api, getConnection, getStreamBase, type StreamInfo, type WatchProgress, type Episode } from '../api'
import ProgressReporter from '../components/ProgressReporter'
import StreamStats from '../components/StreamStats'
import { useMobile } from '../hooks/useMobile'

const DIRECT_CONTAINERS = ['mp4', 'mov', 'webm']
const DIRECT_VIDEO_CODECS = ['h264']
const DIRECT_AUDIO_CODECS = ['aac', 'mp3', 'opus', 'vorbis', 'flac']

function canDirectPlay(info: StreamInfo): boolean {
  const video = info.videoStreams[0]
  const audio = info.audioStreams[0]
  if (!video) return false
  const containerOk = info.container.split(',').some((c) => DIRECT_CONTAINERS.includes(c.trim()))
  if (!containerOk) return false
  if (!DIRECT_VIDEO_CODECS.includes(video.codec)) return false
  if (audio && !DIRECT_AUDIO_CODECS.includes(audio.codec)) return false
  if (video.isHdr) return false
  return true
}

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  const s = Math.floor(secs % 60)
  if (h > 0) return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
  return `${m}:${s.toString().padStart(2, '0')}`
}

function formatBitrate(bps: number): string {
  if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} Mbps`
  if (bps >= 1_000) return `${(bps / 1_000).toFixed(0)} kbps`
  return `${bps} bps`
}

function channelLayout(ch: number): string {
  switch (ch) {
    case 1: return 'Mono'
    case 2: return 'Stereo'
    case 6: return '5.1'
    case 8: return '7.1'
    default: return `${ch}ch`
  }
}

type Mode = 'loading' | 'direct' | 'transcode' | 'error'

interface PlayerLocationState {
  seriesId?: number
  episodeId?: number
}

export default function Player() {
  const { fileId } = useParams<{ fileId: string }>()
  const navigate = useNavigate()
  const location = useLocation()
  const videoRef = useRef<HTMLVideoElement>(null)
  const hlsRef = useRef<Hls | null>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const isMobile = useMobile()

  const [info, setInfo] = useState<StreamInfo | null>(null)
  const [mode, setMode] = useState<Mode>('loading')
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [selectedAudio, setSelectedAudio] = useState(0)
  const [selectedSub, setSelectedSub] = useState<number | null>(null)
  const [savedProgress, setSavedProgress] = useState<WatchProgress | null>(null)
  const [showResume, setShowResume] = useState(false)
  const [isFullscreen, setIsFullscreen] = useState(false)
  const [isPip, setIsPip] = useState(false)
  const [isPlaying, setIsPlaying] = useState(false)

  // Next episode state
  const locationState = (location.state as PlayerLocationState) || {}
  const [nextEpisode, setNextEpisode] = useState<Episode | null>(null)
  const [showNextEpisode, setShowNextEpisode] = useState(false)
  const [nextEpisodeCountdown, setNextEpisodeCountdown] = useState(10)

  const id = Number(fileId)

  // Track fullscreen state
  useEffect(() => {
    const handler = () => setIsFullscreen(!!document.fullscreenElement)
    document.addEventListener('fullscreenchange', handler)
    return () => document.removeEventListener('fullscreenchange', handler)
  }, [])

  // Track PiP state
  useEffect(() => {
    const video = videoRef.current
    if (!video) return
    const onEnterPip = () => setIsPip(true)
    const onLeavePip = () => setIsPip(false)
    video.addEventListener('enterpictureinpicture', onEnterPip)
    video.addEventListener('leavepictureinpicture', onLeavePip)
    return () => {
      video.removeEventListener('enterpictureinpicture', onEnterPip)
      video.removeEventListener('leavepictureinpicture', onLeavePip)
    }
  }, [mode])

  // Track play/pause state
  useEffect(() => {
    const video = videoRef.current
    if (!video) return
    const onPlay = () => setIsPlaying(true)
    const onPause = () => setIsPlaying(false)
    video.addEventListener('play', onPlay)
    video.addEventListener('pause', onPause)
    return () => {
      video.removeEventListener('play', onPlay)
      video.removeEventListener('pause', onPause)
    }
  }, [mode])

  // Request landscape orientation on mobile when playing
  useEffect(() => {
    if (!isMobile || mode === 'loading' || mode === 'error') return
    const orientation = screen.orientation as ScreenOrientation & {
      lock?: (o: string) => Promise<void>
      unlock?: () => void
    }
    try {
      orientation.lock?.('landscape').catch(() => {})
    } catch { /* not supported */ }
    return () => {
      try {
        orientation.unlock?.()
      } catch { /* ignore */ }
    }
  }, [isMobile, mode])

  // Keep screen awake during playback via Wake Lock API
  useEffect(() => {
    if (mode === 'loading' || mode === 'error') return
    let wakeLock: WakeLockSentinel | null = null
    navigator.wakeLock?.request?.('screen')
      .then((wl) => { wakeLock = wl })
      .catch(() => {})
    return () => { wakeLock?.release?.() }
  }, [mode])

  const toggleFullscreen = useCallback(() => {
    if (!containerRef.current) return
    if (document.fullscreenElement) {
      document.exitFullscreen().catch(() => {})
    } else {
      containerRef.current.requestFullscreen().catch(() => {
        // Fallback: try video element directly (some mobile browsers)
        videoRef.current?.requestFullscreen?.().catch(() => {})
      })
    }
  }, [])

  const togglePip = useCallback(async () => {
    try {
      if (document.pictureInPictureElement) {
        await document.exitPictureInPicture()
      } else if (videoRef.current) {
        await videoRef.current.requestPictureInPicture()
      }
    } catch { /* PiP not supported or failed */ }
  }, [])

  const togglePlayPause = useCallback(() => {
    const video = videoRef.current
    if (!video) return
    if (video.paused) {
      void video.play()
    } else {
      video.pause()
    }
  }, [])

  const skipBack = useCallback(() => {
    if (videoRef.current) {
      videoRef.current.currentTime = Math.max(0, videoRef.current.currentTime - 10)
    }
  }, [])

  const skipForward = useCallback(() => {
    if (videoRef.current) {
      videoRef.current.currentTime = Math.min(
        videoRef.current.duration || Infinity,
        videoRef.current.currentTime + 30,
      )
    }
  }, [])

  // Fetch saved progress
  useEffect(() => {
    if (!id || isNaN(id)) return
    api.getProgressSafe(id).then((p) => {
      if (p && p.positionSecs > 5 && !p.completed) {
        setSavedProgress(p)
        setShowResume(true)
      }
    })
  }, [id])

  const [measuredBandwidth, setMeasuredBandwidth] = useState<number | null>(null)
  const [selectedTier, setSelectedTier] = useState<import('../api').QualityTier | null>(null)

  // Load stream info + bandwidth test + quality tiers
  useEffect(() => {
    if (!id || isNaN(id)) {
      setError(`Invalid media file ID (raw: "${fileId}", parsed: ${id})`)
      setMode('error')
      return
    }
    let cancelled = false

    async function init() {
      try {
        const [data, bandwidth] = await Promise.all([
          api.streamInfo(id),
          api.bandwidthTest().catch(() => null),
        ])
        if (cancelled) return

        setInfo(data)
        if (bandwidth) {
          setMeasuredBandwidth(bandwidth)
        }

        if (canDirectPlay(data)) {
          setMode('direct')
        } else {
          if (bandwidth) {
            try {
              const tiers = await api.qualityTiers(id)
              if (!cancelled && tiers.length > 0) {
                const transcodeTiers = tiers.filter(t => t.videoBitrate > 0)
                const affordable = transcodeTiers.filter(t => t.videoBitrate < bandwidth * 0.8)
                const best = affordable.length > 0 ? affordable[0] : transcodeTiers[transcodeTiers.length - 1]
                if (best) {
                  setSelectedTier(best)
                }
              }
            } catch { /* non-critical, use defaults */ }
          }
          setMode('transcode')
        }
      } catch (e) {
        if (!cancelled) {
          console.error('[Player] init failed:', e)
          setError(`Failed to load media info: ${e instanceof Error ? e.message : String(e)}`)
          setMode('error')
        }
      }
    }

    init()
    return () => { cancelled = true }
  // eslint-disable-next-line react-hooks/exhaustive-deps -- id is derived from fileId; init runs once per media file
  }, [id])

  // Direct play
  useEffect(() => {
    if (mode !== 'direct' || !videoRef.current) return
    videoRef.current.src = api.directPlayUrl(id)
  }, [mode, id])

  // HLS transcode
  const [preparing, setPreparing] = useState(false)
  const [encoder, setEncoder] = useState<string | null>(null)
  const [currentLevel, setCurrentLevel] = useState<number>(-1)
  const [qualityLevels, setQualityLevels] = useState<Array<{ width: number; height: number; bitrate: number }>>([])
  const [autoQuality, setAutoQuality] = useState(true)
  const [showStats, setShowStats] = useState(false)
  const [qualityToast, setQualityToast] = useState<string | null>(null)

  useEffect(() => {
    if (mode !== 'transcode' || !info) return
    let cancelled = false
    setPreparing(true)

    async function waitForPlaylist(url: string, timeoutMs: number): Promise<boolean> {
      const conn = getConnection()
      const headers: Record<string, string> = {}
      if (conn?.clientToken) headers['Authorization'] = `Bearer ${conn.clientToken}`

      const start = Date.now()
      while (Date.now() - start < timeoutMs) {
        if (cancelled) return false
        try {
          const res = await fetch(url, { headers, credentials: 'include' })
          if (res.ok) return true
        } catch {
          // fetch error, retry
        }
        await new Promise((r) => setTimeout(r, 2000))
      }
      return false
    }

    api.startTranscode(id, {
      videoStreamIndex: 0,
      audioStreamIndex: selectedAudio,
      subtitleStreamIndex: selectedSub ?? undefined,
      ...(selectedTier ? {
        maxWidth: selectedTier.maxWidth,
        maxHeight: selectedTier.maxHeight,
        videoBitrate: selectedTier.videoBitrate,
      } : {}),
    })
      .then(async (resp) => {
        if (cancelled) return
        setSessionId(resp.sessionId)
        setEncoder(resp.encoder)

        // Server returns `/api/v1/stream/…/master.m3u8`; prepend the stream
        // base (absolute URL against the backend / wildcard-TLS host).
        // A relative path would resolve against the Tauri WebView origin
        // and hls.js would fail with `manifestParsingError`.
        const playlistPath = resp.playlistUrl.startsWith('/api/v1')
          ? resp.playlistUrl.slice('/api/v1'.length)
          : resp.playlistUrl
        const playlistUrl = `${getStreamBase()}${playlistPath}`

        const ready = await waitForPlaylist(playlistUrl, 60000)
        if (cancelled) return
        setPreparing(false)

        if (!ready) {
          setError('Transcode timed out — the server may still be encoding. Try again in a moment.')
          setMode('error')
          return
        }
        if (!videoRef.current) return

        if (Hls.isSupported()) {
          const hls = new Hls({
            maxBufferLength: 30,
            maxMaxBufferLength: 60,
            manifestLoadingTimeOut: 30000,
            manifestLoadingMaxRetry: 3,
            manifestLoadingRetryDelay: 2000,
            startLevel: -1,
            ...(measuredBandwidth ? {
              abrEwmaDefaultEstimate: measuredBandwidth,
            } : {}),
            abrBandWidthFactor: 0.9,
            abrBandWidthUpFactor: 0.7,
            xhrSetup: (xhr) => {
              const conn = getConnection()
              if (conn?.clientToken) {
                xhr.setRequestHeader('Authorization', `Bearer ${conn.clientToken}`)
              }
            },
          })
          hls.loadSource(playlistUrl)
          hls.attachMedia(videoRef.current)
          hls.on(Hls.Events.MANIFEST_PARSED, () => {
            const levels = hls.levels.map(l => ({
              width: l.width,
              height: l.height,
              bitrate: l.bitrate,
            }))
            setQualityLevels(levels)
            void videoRef.current?.play()
          })
          hls.on(Hls.Events.LEVEL_SWITCHED, (_event, data) => {
            setCurrentLevel(data.level)
          })
          hls.on(Hls.Events.ERROR, (_event, data) => {
            if (data.fatal) {
              setError(`HLS error: ${data.details}`)
              setMode('error')
            }
          })
          hlsRef.current = hls
        } else if (videoRef.current.canPlayType('application/vnd.apple.mpegurl')) {
          videoRef.current.src = playlistUrl
          void videoRef.current.play()
        } else {
          setError('Browser does not support HLS playback')
          setMode('error')
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setPreparing(false)
          setError(`Failed to start transcode: ${e.message}`)
          setMode('error')
        }
      })

    return () => {
      cancelled = true
      if (hlsRef.current) {
        hlsRef.current.destroy()
        hlsRef.current = null
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, id, selectedAudio])

  // Cleanup session on unmount
  useEffect(() => {
    return () => {
      if (sessionId) {
        api.stopStreamSession(sessionId).catch(() => {})
      }
    }
  }, [sessionId])

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const tag = document.activeElement?.tagName
      if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') return

      switch (e.key) {
        case ' ':
        case 'k':
        case 'K':
          e.preventDefault()
          togglePlayPause()
          break
        case 'ArrowLeft':
          e.preventDefault()
          skipBack()
          break
        case 'ArrowRight':
          e.preventDefault()
          skipForward()
          break
        case 'f':
        case 'F':
          e.preventDefault()
          toggleFullscreen()
          break
        case 'm':
        case 'M':
          e.preventDefault()
          if (videoRef.current) {
            videoRef.current.muted = !videoRef.current.muted
          }
          break
        case 'ArrowUp':
          e.preventDefault()
          if (videoRef.current) {
            videoRef.current.volume = Math.min(1, videoRef.current.volume + 0.1)
          }
          break
        case 'ArrowDown':
          e.preventDefault()
          if (videoRef.current) {
            videoRef.current.volume = Math.max(0, videoRef.current.volume - 0.1)
          }
          break
        case 's':
        case 'S':
          setShowStats(prev => !prev)
          break
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [togglePlayPause, skipBack, skipForward, toggleFullscreen])

  // Next episode: detect end of video and find next episode
  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    const onEnded = async () => {
      const { seriesId, episodeId } = locationState
      if (!seriesId || !episodeId) return

      try {
        const episodes = await api.getEpisodes(seriesId)
        const sorted = episodes
          .sort((a, b) => a.seasonNumber - b.seasonNumber || a.episodeNumber - b.episodeNumber)
        const currentIdx = sorted.findIndex(ep => ep.id === episodeId)
        if (currentIdx === -1) return

        // Find next episode with a file
        for (let i = currentIdx + 1; i < sorted.length; i++) {
          if (sorted[i].episodeFile?.id) {
            setNextEpisode(sorted[i])
            setShowNextEpisode(true)
            setNextEpisodeCountdown(10)
            return
          }
        }
      } catch {
        // Failed to fetch episodes, silently ignore
      }
    }

    video.addEventListener('ended', onEnded)
    return () => video.removeEventListener('ended', onEnded)
  // eslint-disable-next-line react-hooks/exhaustive-deps -- destructured locationState fields are stable for this media session
  }, [mode, locationState.seriesId, locationState.episodeId])

  // Next episode countdown timer
  useEffect(() => {
    if (!showNextEpisode || !nextEpisode) return
    if (nextEpisodeCountdown <= 0) {
      // Auto-play next episode
      navigate(`/play/${nextEpisode.episodeFile!.id}`, {
        state: { seriesId: locationState.seriesId, episodeId: nextEpisode.id },
      })
      return
    }
    const timer = setTimeout(() => setNextEpisodeCountdown(prev => prev - 1), 1000)
    return () => clearTimeout(timer)
  }, [showNextEpisode, nextEpisode, nextEpisodeCountdown, navigate, locationState.seriesId])

  // Buffer safety net
  useEffect(() => {
    if (!hlsRef.current || qualityLevels.length <= 1) return

    let cooldownUntil = 0
    const interval = setInterval(() => {
      const hls = hlsRef.current
      const video = videoRef.current
      if (!hls || !video || !video.buffered.length) return

      const bufferAhead = video.buffered.end(video.buffered.length - 1) - video.currentTime
      const now = Date.now()

      if (bufferAhead < 2 && now > cooldownUntil && hls.currentLevel > 0) {
        const newLevel = Math.max(0, hls.currentLevel - 1)
        hls.nextLevel = newLevel
        setAutoQuality(false)
        cooldownUntil = now + 30000
        const level = hls.levels[newLevel]
        const msg = `Quality reduced to ${level?.height || '?'}p (buffering)`
        setQualityToast(msg)
        setTimeout(() => setQualityToast(null), 4000)
      } else if (bufferAhead > 10 && now > cooldownUntil && !autoQuality) {
        hls.currentLevel = -1
        setAutoQuality(true)
      }
    }, 2000)

    return () => clearInterval(interval)
  }, [qualityLevels, autoQuality])

  const forceTranscode = useCallback(() => {
    if (videoRef.current) {
      videoRef.current.pause()
      videoRef.current.removeAttribute('src')
    }
    setMode('transcode')
  }, [])

  const handleResume = useCallback(() => {
    if (savedProgress && videoRef.current) {
      videoRef.current.currentTime = savedProgress.positionSecs
    }
    setShowResume(false)
  }, [savedProgress])

  const handleStartOver = useCallback(() => {
    setShowResume(false)
  }, [])

  const pipSupported = typeof document !== 'undefined' && document.pictureInPictureEnabled

  // Next episode overlay component
  const nextEpisodeOverlay = showNextEpisode && nextEpisode && (
    <div style={{
      position: 'absolute', inset: 0, zIndex: 20,
      display: 'flex', alignItems: 'center', justifyContent: 'center',
      background: 'rgba(0, 0, 0, 0.85)',
    }}>
      <div style={{
        display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 16,
        maxWidth: 400, textAlign: 'center', padding: 24,
      }}>
        <span style={{ fontSize: 13, color: '#64748b', textTransform: 'uppercase', letterSpacing: 1 }}>
          Up Next
        </span>
        <span style={{ fontSize: 18, fontWeight: 600, color: '#f1f5f9' }}>
          S{String(nextEpisode.seasonNumber).padStart(2, '0')}E{String(nextEpisode.episodeNumber).padStart(2, '0')}
        </span>
        <span style={{ fontSize: 15, color: '#cbd5e1' }}>
          {nextEpisode.title || 'TBA'}
        </span>
        <div style={{
          width: 48, height: 48, borderRadius: '50%',
          border: '3px solid #3b82f6',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          fontSize: 18, fontWeight: 700, color: '#3b82f6',
          marginTop: 4,
        }}>
          {nextEpisodeCountdown}
        </div>
        <div style={{ display: 'flex', gap: 12, marginTop: 4 }}>
          <button
            onClick={() => {
              navigate(`/play/${nextEpisode.episodeFile!.id}`, {
                state: { seriesId: locationState.seriesId, episodeId: nextEpisode.id },
              })
            }}
            style={{
              background: '#3b82f6', border: 'none', borderRadius: 8,
              padding: '10px 24px', color: '#fff', fontSize: 14,
              cursor: 'pointer', fontWeight: 600,
            }}
          >
            Play Now
          </button>
          <button
            onClick={() => {
              setShowNextEpisode(false)
              setNextEpisode(null)
            }}
            style={{
              background: '#334155', border: 'none', borderRadius: 8,
              padding: '10px 24px', color: '#94a3b8', fontSize: 14,
              cursor: 'pointer',
            }}
          >
            Cancel
          </button>
        </div>
      </div>
    </div>
  )

  // Skip button style helper
  const skipButtonStyle: React.CSSProperties = {
    background: 'rgba(0, 0, 0, 0.5)',
    border: 'none',
    borderRadius: '50%',
    width: 48,
    height: 48,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    color: '#fff',
    cursor: 'pointer',
    flexDirection: 'column',
    gap: 0,
  }

  // Controls section (shared between mobile and desktop, placed below video on mobile)
  const controlsSection = info && mode !== 'error' && mode !== 'loading' && (
    <div style={{
      display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: isMobile ? 8 : 16,
      background: '#1e293b', borderRadius: isMobile ? 0 : 10,
      padding: isMobile ? '10px 12px' : 12,
      marginBottom: isMobile ? 0 : 16,
    }}>
      {info.audioStreams.length > 1 && (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: '#94a3b8' }}>
          Audio:
          <select
            value={selectedAudio}
            onChange={(e) => setSelectedAudio(Number(e.target.value))}
            style={{
              background: '#334155', border: 'none', borderRadius: 6,
              padding: '6px 8px', color: '#f1f5f9', fontSize: 13,
            }}
          >
            {info.audioStreams.map((a) => (
              <option key={a.index} value={a.index}>
                {a.title || a.language} ({a.codec.toUpperCase()} {channelLayout(a.channels)})
              </option>
            ))}
          </select>
        </label>
      )}

      {info.subtitleStreams.length > 0 && (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: '#94a3b8' }}>
          Subs:
          <select
            value={selectedSub ?? -1}
            onChange={(e) => {
              const v = Number(e.target.value)
              setSelectedSub(v >= 0 ? v : null)
              if (videoRef.current) {
                const tracks = videoRef.current.textTracks
                for (let i = 0; i < tracks.length; i++) {
                  tracks[i].mode = i === v ? 'showing' : 'hidden'
                }
              }
            }}
            style={{
              background: '#334155', border: 'none', borderRadius: 6,
              padding: '6px 8px', color: '#f1f5f9', fontSize: 13,
            }}
          >
            <option value={-1}>None</option>
            {info.subtitleStreams.map((s) => (
              <option key={s.index} value={s.index}>
                {s.title || s.language} ({s.codec}) {s.forced ? '[Forced]' : ''}
              </option>
            ))}
          </select>
        </label>
      )}

      {mode === 'transcode' && qualityLevels.length > 1 && (
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: '#94a3b8' }}>
          Quality:
          <select
            value={autoQuality ? -1 : currentLevel}
            onChange={(e) => {
              const v = Number(e.target.value)
              if (v === -1) {
                setAutoQuality(true)
                if (hlsRef.current) hlsRef.current.currentLevel = -1
              } else {
                setAutoQuality(false)
                if (hlsRef.current) hlsRef.current.currentLevel = v
              }
            }}
            style={{
              background: '#334155', border: 'none', borderRadius: 6,
              padding: '6px 8px', color: '#f1f5f9', fontSize: 13,
            }}
          >
            <option value={-1}>Auto</option>
            {qualityLevels.map((level, i) => (
              <option key={i} value={i}>
                {level.height}p ({(level.bitrate / 1_000_000).toFixed(1)} Mbps)
              </option>
            ))}
          </select>
        </label>
      )}

      {mode === 'direct' && !isMobile && (
        <button
          onClick={forceTranscode}
          style={{
            background: '#334155', border: 'none', borderRadius: 6,
            padding: '6px 12px', color: '#94a3b8', fontSize: 13, cursor: 'pointer',
          }}
        >
          Force Transcode
        </button>
      )}
    </div>
  )

  // Mobile layout
  if (isMobile) {
    return (
      <div ref={containerRef} style={{
        display: 'flex', flexDirection: 'column',
        background: '#000',
        minHeight: isFullscreen ? '100vh' : undefined,
      }}>
        {/* Video area */}
        <div style={{ position: 'relative', width: '100%' }}>
          {mode === 'loading' ? (
            <div style={{
              display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
              height: '56.25vw', color: '#64748b', gap: 12,
            }}>
              <div style={{ width: 32, height: 32, border: '3px solid #334155', borderTopColor: '#3b82f6', borderRadius: '50%', animation: 'spin 1s linear infinite' }} />
              Analyzing media...
            </div>
          ) : mode === 'error' ? (
            <div style={{
              display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
              height: '56.25vw', color: '#fca5a5', padding: 24, textAlign: 'center', fontSize: 14,
            }}>
              {error || 'Playback error'}
              <button
                onClick={() => navigate(-1)}
                style={{
                  marginTop: 16, padding: '10px 24px', borderRadius: 8,
                  background: '#334155', border: 'none', color: '#e2e8f0',
                  fontSize: 14, cursor: 'pointer',
                }}
              >
                Go Back
              </button>
            </div>
          ) : (
            <>
              {preparing && (
                <div style={{
                  position: 'absolute', inset: 0, zIndex: 5,
                  display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
                  background: 'rgba(0,0,0,0.85)', color: '#64748b', gap: 12,
                }}>
                  <div style={{ width: 32, height: 32, border: '3px solid #334155', borderTopColor: '#3b82f6', borderRadius: '50%', animation: 'spin 1s linear infinite' }} />
                  <span style={{ fontSize: 13, textAlign: 'center', padding: '0 24px' }}>
                    Preparing stream{selectedTier ? ` — ${selectedTier.name}` : ''}{encoder ? ` (${encoder})` : ''}...
                  </span>
                </div>
              )}

              <video
                ref={videoRef}
                controls
                playsInline
                autoPlay
                style={{
                  width: '100%',
                  display: 'block',
                  maxHeight: isFullscreen ? '100vh' : undefined,
                }}
              >
                {info?.subtitleStreams
                  .filter((s) => !['hdmv_pgs_subtitle', 'pgssub', 'dvb_subtitle', 'dvdsub'].includes(s.codec))
                  .map((sub) => (
                    <track
                      key={sub.index}
                      kind="subtitles"
                      src={api.subtitleUrl(id, sub.index)}
                      srcLang={sub.language}
                      label={sub.title || sub.language}
                      default={sub.isDefault}
                    />
                  ))}
              </video>

              {/* Overlay buttons - top row */}
              <button
                onClick={() => navigate(-1)}
                style={{
                  position: 'absolute', top: 12, left: 12, zIndex: 10,
                  background: 'rgba(0,0,0,0.6)', border: 'none', borderRadius: '50%',
                  width: 36, height: 36, display: 'flex', alignItems: 'center', justifyContent: 'center',
                  color: '#fff', cursor: 'pointer',
                }}
              >
                <ArrowLeft size={18} />
              </button>

              {pipSupported && (
                <button
                  onClick={togglePip}
                  style={{
                    position: 'absolute', top: 12, right: 96, zIndex: 10,
                    background: 'rgba(0,0,0,0.6)', border: 'none', borderRadius: '50%',
                    width: 36, height: 36, display: 'flex', alignItems: 'center', justifyContent: 'center',
                    color: isPip ? '#3b82f6' : '#fff', cursor: 'pointer',
                  }}
                >
                  <PictureInPicture2 size={16} />
                </button>
              )}

              <button
                onClick={toggleFullscreen}
                style={{
                  position: 'absolute', top: 12, right: 48, zIndex: 10,
                  background: 'rgba(0,0,0,0.6)', border: 'none', borderRadius: '50%',
                  width: 36, height: 36, display: 'flex', alignItems: 'center', justifyContent: 'center',
                  color: '#fff', cursor: 'pointer',
                }}
              >
                {isFullscreen ? <Minimize size={16} /> : <Maximize size={16} />}
              </button>

              {/* Mode badge */}
              <div style={{
                position: 'absolute', top: 12, right: 12,
                background: 'rgba(0,0,0,0.7)', borderRadius: 6,
                padding: '4px 8px', fontSize: 11, color: '#cbd5e1',
              }}>
                {mode === 'direct' ? 'Direct' : (() => {
                  const level = qualityLevels[currentLevel]
                  return level ? `${level.height}p` : 'HLS'
                })()}
              </div>

              {/* Center skip buttons overlay */}
              <div style={{
                position: 'absolute', top: '50%', left: '50%',
                transform: 'translate(-50%, -50%)',
                display: 'flex', alignItems: 'center', gap: 32,
                zIndex: 8, pointerEvents: 'none',
              }}>
                <button
                  onClick={skipBack}
                  style={{ ...skipButtonStyle, pointerEvents: 'auto' }}
                  title="Skip back 10s"
                >
                  <RotateCcw size={20} />
                  <span style={{ fontSize: 9, marginTop: -2 }}>10</span>
                </button>
                <button
                  onClick={togglePlayPause}
                  style={{ ...skipButtonStyle, width: 56, height: 56, pointerEvents: 'auto' }}
                  title={isPlaying ? 'Pause' : 'Play'}
                >
                  {isPlaying ? <Pause size={24} /> : <Play size={24} style={{ marginLeft: 2 }} />}
                </button>
                <button
                  onClick={skipForward}
                  style={{ ...skipButtonStyle, pointerEvents: 'auto' }}
                  title="Skip forward 30s"
                >
                  <RotateCw size={20} />
                  <span style={{ fontSize: 9, marginTop: -2 }}>30</span>
                </button>
              </div>

              {/* Resume prompt - bottom 80px */}
              {showResume && savedProgress && (
                <div style={{
                  position: 'absolute', bottom: 80, left: '50%', transform: 'translateX(-50%)',
                  background: 'rgba(15, 23, 42, 0.95)', borderRadius: 10,
                  padding: '10px 16px', display: 'flex', alignItems: 'center', gap: 12,
                  boxShadow: '0 4px 20px rgba(0,0,0,0.5)', zIndex: 10,
                  whiteSpace: 'nowrap',
                }}>
                  <span style={{ color: '#cbd5e1', fontSize: 13 }}>
                    Resume from {formatDuration(savedProgress.positionSecs)}?
                  </span>
                  <button
                    onClick={handleResume}
                    style={{
                      background: '#3b82f6', border: 'none', borderRadius: 6,
                      padding: '8px 16px', color: '#fff', fontSize: 13,
                      cursor: 'pointer', fontWeight: 500,
                    }}
                  >
                    Resume
                  </button>
                  <button
                    onClick={handleStartOver}
                    style={{
                      background: '#334155', border: 'none', borderRadius: 6,
                      padding: '8px 16px', color: '#94a3b8', fontSize: 13,
                      cursor: 'pointer',
                    }}
                  >
                    Start Over
                  </button>
                </div>
              )}

              <ProgressReporter videoRef={videoRef} mediaFileId={id} />
              <StreamStats hls={hlsRef.current} videoRef={videoRef} encoder={encoder} visible={showStats} />

              {/* Quality toast - bottom 40px */}
              {qualityToast && (
                <div style={{
                  position: 'absolute', bottom: 40, left: '50%', transform: 'translateX(-50%)',
                  background: 'rgba(15, 23, 42, 0.95)', borderRadius: 8,
                  padding: '8px 16px', fontSize: 13, color: '#fbbf24',
                  boxShadow: '0 4px 12px rgba(0,0,0,0.5)', zIndex: 10,
                  whiteSpace: 'nowrap',
                }}>
                  {qualityToast}
                </div>
              )}

              {nextEpisodeOverlay}
            </>
          )}
        </div>

        {/* Controls below video */}
        {!isFullscreen && controlsSection}

        {/* Compact media info on mobile */}
        {!isFullscreen && info && (
          <div style={{
            background: '#0f172a', padding: '12px 16px',
            fontSize: 12, color: '#64748b',
            display: 'flex', flexWrap: 'wrap', gap: '4px 16px',
          }}>
            <span>{info.container} · {formatDuration(info.durationSecs)}</span>
            {info.videoStreams[0] && (
              <span>
                {info.videoStreams[0].codec.toUpperCase()} {info.videoStreams[0].width}x{info.videoStreams[0].height}
                {info.videoStreams[0].isHdr ? ' HDR' : ''}
              </span>
            )}
            <span>{formatBitrate(info.bitrate)}</span>
          </div>
        )}
      </div>
    )
  }

  // Desktop layout
  return (
    <div>
      <button
        onClick={() => navigate(-1)}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, background: 'none', border: 'none',
          color: '#94a3b8', cursor: 'pointer', fontSize: 14, marginBottom: 16, padding: 0,
        }}
      >
        <ArrowLeft size={16} /> Back
      </button>

      {/* Player */}
      <div ref={containerRef} style={{
        background: '#000',
        borderRadius: 12,
        overflow: 'hidden',
        position: 'relative',
        marginBottom: 16,
      }}>
        {mode === 'loading' ? (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: 400, color: '#64748b', gap: 12 }}>
            <div style={{ width: 32, height: 32, border: '3px solid #334155', borderTopColor: '#3b82f6', borderRadius: '50%', animation: 'spin 1s linear infinite' }} />
            Analyzing media...
          </div>
        ) : mode === 'error' ? (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: 400, color: '#fca5a5' }}>
            {error || 'Playback error'}
          </div>
        ) : (
          <>
            {preparing && (
              <div style={{
                position: 'absolute', inset: 0, zIndex: 5,
                display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center',
                background: 'rgba(0,0,0,0.85)', color: '#64748b', gap: 12,
              }}>
                <div style={{ width: 32, height: 32, border: '3px solid #334155', borderTopColor: '#3b82f6', borderRadius: '50%', animation: 'spin 1s linear infinite' }} />
                Preparing stream{selectedTier ? ` — ${selectedTier.name}` : ''}{encoder ? ` (${encoder})` : ''}...
              </div>
            )}

            <video
              ref={videoRef}
              controls
              playsInline
              style={{ width: '100%', display: 'block' }}
            >
              {info?.subtitleStreams
                .filter((s) => !['hdmv_pgs_subtitle', 'pgssub', 'dvb_subtitle', 'dvdsub'].includes(s.codec))
                .map((sub) => (
                  <track
                    key={sub.index}
                    kind="subtitles"
                    src={api.subtitleUrl(id, sub.index)}
                    srcLang={sub.language}
                    label={sub.title || sub.language}
                    default={sub.isDefault}
                  />
                ))}
            </video>

            {/* Top-right controls row */}
            {pipSupported && (
              <button
                onClick={togglePip}
                style={{
                  position: 'absolute', top: 12, right: 110, zIndex: 10,
                  background: 'rgba(0,0,0,0.6)', border: 'none', borderRadius: 6,
                  padding: '4px 8px', display: 'flex', alignItems: 'center',
                  color: isPip ? '#3b82f6' : '#cbd5e1', cursor: 'pointer', fontSize: 12, gap: 4,
                }}
              >
                <PictureInPicture2 size={14} />
              </button>
            )}

            {/* Fullscreen toggle */}
            <button
              onClick={toggleFullscreen}
              style={{
                position: 'absolute', top: 12, right: 60, zIndex: 10,
                background: 'rgba(0,0,0,0.6)', border: 'none', borderRadius: 6,
                padding: '4px 8px', display: 'flex', alignItems: 'center',
                color: '#cbd5e1', cursor: 'pointer', fontSize: 12, gap: 4,
              }}
            >
              {isFullscreen ? <Minimize size={14} /> : <Maximize size={14} />}
            </button>

            {/* Mode badge */}
            <div style={{
              position: 'absolute', top: 12, right: 12,
              background: 'rgba(0,0,0,0.7)', borderRadius: 6,
              padding: '4px 10px', fontSize: 12, color: '#cbd5e1',
            }}>
              {mode === 'direct' ? 'Direct Play' : (() => {
                const level = qualityLevels[currentLevel]
                const qualityLabel = level ? `${level.height}p` : ''
                const autoLabel = autoQuality ? 'Auto' : ''
                const parts = [qualityLabel, autoLabel].filter(Boolean).join(' ')
                return `Transcoding${parts ? ` — ${parts}` : ''}`
              })()}
            </div>

            {/* Center skip buttons overlay (desktop) */}
            <div style={{
              position: 'absolute', top: '50%', left: '50%',
              transform: 'translate(-50%, -50%)',
              display: 'flex', alignItems: 'center', gap: 40,
              zIndex: 8, pointerEvents: 'none',
            }}>
              <button
                onClick={skipBack}
                style={{ ...skipButtonStyle, pointerEvents: 'auto', opacity: 0.8 }}
                title="Skip back 10s (Left Arrow)"
              >
                <RotateCcw size={20} />
                <span style={{ fontSize: 9, marginTop: -2 }}>10</span>
              </button>
              <button
                onClick={togglePlayPause}
                style={{ ...skipButtonStyle, width: 56, height: 56, pointerEvents: 'auto', opacity: 0.8 }}
                title={isPlaying ? 'Pause (Space)' : 'Play (Space)'}
              >
                {isPlaying ? <Pause size={26} /> : <Play size={26} style={{ marginLeft: 3 }} />}
              </button>
              <button
                onClick={skipForward}
                style={{ ...skipButtonStyle, pointerEvents: 'auto', opacity: 0.8 }}
                title="Skip forward 30s (Right Arrow)"
              >
                <RotateCw size={20} />
                <span style={{ fontSize: 9, marginTop: -2 }}>30</span>
              </button>
            </div>

            {/* Resume prompt - bottom 80px */}
            {showResume && savedProgress && (
              <div style={{
                position: 'absolute', bottom: 80, left: '50%', transform: 'translateX(-50%)',
                background: 'rgba(15, 23, 42, 0.95)', borderRadius: 10,
                padding: '12px 20px', display: 'flex', alignItems: 'center', gap: 16,
                boxShadow: '0 4px 20px rgba(0,0,0,0.5)', zIndex: 10,
              }}>
              <span style={{ color: '#cbd5e1', fontSize: 14 }}>
                  Resume from {formatDuration(savedProgress.positionSecs)}?
                </span>
                <button
                  onClick={handleResume}
                  style={{
                    background: '#3b82f6', border: 'none', borderRadius: 6,
                    padding: '6px 14px', color: '#fff', fontSize: 13,
                    cursor: 'pointer', fontWeight: 500,
                  }}
                >
                  Resume
                </button>
                <button
                  onClick={handleStartOver}
                  style={{
                    background: '#334155', border: 'none', borderRadius: 6,
                    padding: '6px 14px', color: '#94a3b8', fontSize: 13,
                    cursor: 'pointer',
                  }}
                >
                  Start Over
                </button>
              </div>
            )}

            <ProgressReporter videoRef={videoRef} mediaFileId={id} />
            <StreamStats hls={hlsRef.current} videoRef={videoRef} encoder={encoder} visible={showStats} />

            {/* Quality toast - bottom 40px */}
            {qualityToast && (
              <div style={{
                position: 'absolute', bottom: 40, left: '50%', transform: 'translateX(-50%)',
                background: 'rgba(15, 23, 42, 0.95)', borderRadius: 8,
                padding: '8px 16px', fontSize: 13, color: '#fbbf24',
                boxShadow: '0 4px 12px rgba(0,0,0,0.5)', zIndex: 10,
                whiteSpace: 'nowrap',
              }}>
                {qualityToast}
              </div>
            )}

            {nextEpisodeOverlay}
          </>
        )}
      </div>

      {/* Controls */}
      {controlsSection}

      {/* Media info */}
      {info && (
        <div style={{
          background: '#1e293b50', borderRadius: 10, padding: 16,
          border: '1px solid #334155',
        }}>
          <h3 style={{ fontSize: 14, fontWeight: 600, color: '#94a3b8', marginBottom: 10 }}>
            Media Info
          </h3>
          <div style={{
            display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(200px, 1fr))',
            gap: '6px 24px', fontSize: 13, color: '#64748b',
          }}>
            <span>Container: {info.container}</span>
            <span>Duration: {formatDuration(info.durationSecs)}</span>
            <span>Bitrate: {formatBitrate(info.bitrate)}</span>
            {info.videoStreams[0] && (
              <>
                <span>
                  Video: {info.videoStreams[0].codec.toUpperCase()}{' '}
                  {info.videoStreams[0].width}x{info.videoStreams[0].height}
                  {info.videoStreams[0].isHdr ? ' HDR' : ''}
                </span>
                <span>Frame Rate: {info.videoStreams[0].frameRate.toFixed(2)} fps</span>
              </>
            )}
            {info.audioStreams[0] && (
              <span>
                Audio: {info.audioStreams[0].codec.toUpperCase()}{' '}
                {channelLayout(info.audioStreams[0].channels)}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
