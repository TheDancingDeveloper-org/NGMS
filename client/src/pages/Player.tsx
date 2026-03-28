import { useEffect, useRef, useState, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { ArrowLeft } from 'lucide-react'
import Hls from 'hls.js'
import { api, type StreamInfo, type WatchProgress } from '../api'
import ProgressReporter from '../components/ProgressReporter'

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

export default function Player() {
  const { fileId } = useParams<{ fileId: string }>()
  const navigate = useNavigate()
  const videoRef = useRef<HTMLVideoElement>(null)
  const hlsRef = useRef<Hls | null>(null)

  const [info, setInfo] = useState<StreamInfo | null>(null)
  const [mode, setMode] = useState<Mode>('loading')
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [selectedAudio, setSelectedAudio] = useState(0)
  const [selectedSub, setSelectedSub] = useState<number | null>(null)
  const [savedProgress, setSavedProgress] = useState<WatchProgress | null>(null)
  const [showResume, setShowResume] = useState(false)

  const id = Number(fileId)

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

  // Load stream info
  useEffect(() => {
    console.log('[Player] fileId param:', fileId, 'parsed id:', id, 'isNaN:', isNaN(id))
    if (!id || isNaN(id)) {
      setError(`Invalid media file ID (raw: "${fileId}", parsed: ${id})`)
      setMode('error')
      return
    }
    let cancelled = false
    console.log('[Player] fetching stream info for media file', id)
    api.streamInfo(id)
      .then((data) => {
        if (cancelled) return
        console.log('[Player] stream info loaded:', data.container, data.videoStreams.length, 'video,', data.audioStreams.length, 'audio')
        setInfo(data)
        if (canDirectPlay(data)) {
          setMode('direct')
        } else {
          setMode('transcode')
        }
      })
      .catch((e) => {
        if (cancelled) return
        console.error('[Player] stream info failed:', e)
        setError(`Failed to load media info: ${e.message}`)
        setMode('error')
      })
    return () => { cancelled = true }
  }, [id])

  // Direct play
  useEffect(() => {
    if (mode !== 'direct' || !videoRef.current) return
    videoRef.current.src = api.directPlayUrl(id)
  }, [mode, id])

  // HLS transcode
  useEffect(() => {
    if (mode !== 'transcode' || !info) return
    let cancelled = false

    api.startTranscode(id, {
      videoStreamIndex: 0,
      audioStreamIndex: selectedAudio,
      subtitleStreamIndex: selectedSub ?? undefined,
    })
      .then((resp) => {
        if (cancelled) return
        setSessionId(resp.sessionId)
        if (!videoRef.current) return

        const playlistUrl = `/api/v1${resp.playlistUrl}`

        if (Hls.isSupported()) {
          let retryCount = 0
          const maxRetries = 5

          const hls = new Hls({
            maxBufferLength: 30,
            maxMaxBufferLength: 60,
            manifestLoadingTimeOut: 15000,
            manifestLoadingMaxRetry: maxRetries,
            manifestLoadingRetryDelay: 2000,
          })
          hls.loadSource(playlistUrl)
          hls.attachMedia(videoRef.current)
          hls.on(Hls.Events.MANIFEST_PARSED, () => {
            void videoRef.current?.play()
          })
          hls.on(Hls.Events.ERROR, (_event, data) => {
            if (data.fatal) {
              if (data.type === Hls.ErrorTypes.NETWORK_ERROR && retryCount < maxRetries) {
                retryCount++
                console.log(`[Player] HLS manifest retry ${retryCount}/${maxRetries}`)
                setTimeout(() => hls.loadSource(playlistUrl), 2000)
              } else {
                setError(`HLS error: ${data.details}`)
                setMode('error')
              }
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
  }, [mode, id, selectedAudio, selectedSub])

  // Cleanup session on unmount
  useEffect(() => {
    return () => {
      if (sessionId) {
        fetch(`/api/v1/stream/sessions/${sessionId}`, { method: 'DELETE' }).catch(() => {})
      }
    }
  }, [sessionId])

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
      <div style={{
        background: '#000',
        borderRadius: 12,
        overflow: 'hidden',
        position: 'relative',
        marginBottom: 16,
      }}>
        {mode === 'loading' ? (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: 400, color: '#64748b' }}>
            Analyzing media...
          </div>
        ) : mode === 'error' ? (
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: 400, color: '#fca5a5' }}>
            {error || 'Playback error'}
          </div>
        ) : (
          <>
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

            {/* Mode badge */}
            <div style={{
              position: 'absolute', top: 12, right: 12,
              background: 'rgba(0,0,0,0.7)', borderRadius: 6,
              padding: '4px 10px', fontSize: 12, color: '#cbd5e1',
            }}>
              {mode === 'direct' ? 'Direct Play' : 'Transcoding'}
            </div>

            {/* Resume prompt */}
            {showResume && savedProgress && (
              <div style={{
                position: 'absolute', bottom: 60, left: '50%', transform: 'translateX(-50%)',
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
          </>
        )}
      </div>

      {/* Controls */}
      {info && mode !== 'error' && mode !== 'loading' && (
        <div style={{
          display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 16,
          background: '#1e293b', borderRadius: 10, padding: 12,
          marginBottom: 16,
        }}>
          {info.audioStreams.length > 1 && (
            <label style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13, color: '#94a3b8' }}>
              Audio:
              <select
                value={selectedAudio}
                onChange={(e) => setSelectedAudio(Number(e.target.value))}
                style={{
                  background: '#334155', border: 'none', borderRadius: 6,
                  padding: '4px 8px', color: '#f1f5f9', fontSize: 13,
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
              Subtitles:
              <select
                value={selectedSub ?? -1}
                onChange={(e) => {
                  const v = Number(e.target.value)
                  setSelectedSub(v >= 0 ? v : null)
                }}
                style={{
                  background: '#334155', border: 'none', borderRadius: 6,
                  padding: '4px 8px', color: '#f1f5f9', fontSize: 13,
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

          {mode === 'direct' && (
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
      )}

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
