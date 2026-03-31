import { useEffect, useRef, useState, useCallback } from 'react'
import Hls from 'hls.js'
import { useStreamInfo, useStartTranscode, useStopStreamSession } from '../hooks/useApi'
import type { MediaStreamInfo } from '../api/types'

interface VideoPlayerProps {
  mediaFileId: number
}

type PlayMode = 'detecting' | 'direct' | 'transcode' | 'error'

const DIRECT_PLAY_CODECS: Record<string, string> = {
  h264: 'video/mp4; codecs="avc1.640029"',
  aac: 'audio/mp4; codecs="mp4a.40.2"',
  mp3: 'audio/mpeg',
  opus: 'audio/webm; codecs="opus"',
  vorbis: 'audio/webm; codecs="vorbis"',
  flac: 'audio/flac',
}

const DIRECT_PLAY_CONTAINERS = ['mp4', 'mov', 'webm']

function canDirectPlay(info: MediaStreamInfo): boolean {
  const video = info.videoStreams[0]
  const audio = info.audioStreams[0]
  if (!video) return false

  // Check container
  const containerParts = info.container.split(',')
  const supportedContainer = containerParts.some((c) =>
    DIRECT_PLAY_CONTAINERS.includes(c.trim()),
  )
  if (!supportedContainer) return false

  // Check video codec via MediaSource API
  const videoMime = DIRECT_PLAY_CODECS[video.codec]
  if (!videoMime) return false

  const videoEl = document.createElement('video')
  if (!videoEl.canPlayType(videoMime)) return false

  // Check audio codec (if present)
  if (audio) {
    const audioMime = DIRECT_PLAY_CODECS[audio.codec]
    if (audioMime && !videoEl.canPlayType(audioMime)) return false
  }

  // HDR is generally not supported for direct play in browsers
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

function channelLayout(channels: number): string {
  switch (channels) {
    case 1: return 'Mono'
    case 2: return 'Stereo'
    case 6: return '5.1'
    case 8: return '7.1'
    default: return `${channels}ch`
  }
}

export default function VideoPlayer({ mediaFileId }: VideoPlayerProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const hlsRef = useRef<Hls | null>(null)
  const [mode, setMode] = useState<PlayMode>('detecting')
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [selectedAudio, setSelectedAudio] = useState(0)
  const [selectedSub, setSelectedSub] = useState<number | null>(null)
  const [errorMsg, setErrorMsg] = useState<string | null>(null)

  const { data: streamInfo, isLoading, error: infoError } = useStreamInfo(mediaFileId)
  const startTranscode = useStartTranscode()
  const stopSession = useStopStreamSession()

  // Decide playback mode once stream info is loaded
  useEffect(() => {
    if (!streamInfo) return
    if (mode !== 'detecting') return

    if (canDirectPlay(streamInfo)) {
      setMode('direct')
    } else {
      setMode('transcode')
    }
  }, [streamInfo, mode])

  // Set up direct play
  useEffect(() => {
    if (mode !== 'direct' || !videoRef.current) return
    videoRef.current.src = `/api/v1/stream/${mediaFileId}/direct`
  }, [mode, mediaFileId])

  // Set up transcode + HLS
  useEffect(() => {
    if (mode !== 'transcode' || !streamInfo) return

    startTranscode.mutate(
      {
        mediaFileId,
        videoStreamIndex: 0,
        audioStreamIndex: selectedAudio,
        subtitleStreamIndex: selectedSub ?? undefined,
      },
      {
        onSuccess: (resp) => {
          setSessionId(resp.sessionId)

          if (!videoRef.current) return
          const playlistUrl = `/api/v1${resp.playlistUrl}`

          if (Hls.isSupported()) {
            const hls = new Hls({
              maxBufferLength: 30,
              maxMaxBufferLength: 60,
            })
            hls.loadSource(playlistUrl)
            hls.attachMedia(videoRef.current)
            hls.on(Hls.Events.MANIFEST_PARSED, () => {
              void videoRef.current?.play()
            })
            hls.on(Hls.Events.ERROR, (_event, data) => {
              if (data.fatal) {
                setErrorMsg(`HLS error: ${data.details}`)
                setMode('error')
              }
            })
            hlsRef.current = hls
          } else if (videoRef.current.canPlayType('application/vnd.apple.mpegurl')) {
            // Safari native HLS
            videoRef.current.src = playlistUrl
            void videoRef.current.play()
          } else {
            setErrorMsg('Browser does not support HLS playback')
            setMode('error')
          }
        },
        onError: (err) => {
          setErrorMsg(`Failed to start transcode: ${err.message}`)
          setMode('error')
        },
      },
    )

    return () => {
      if (hlsRef.current) {
        hlsRef.current.destroy()
        hlsRef.current = null
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mode, mediaFileId, selectedAudio, selectedSub])

  // Cleanup session on unmount
  useEffect(() => {
    return () => {
      if (sessionId) {
        stopSession.mutate(sessionId)
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId])

  const handleForceTranscode = useCallback(() => {
    // Stop current direct play and switch to transcode
    if (videoRef.current) {
      videoRef.current.pause()
      videoRef.current.removeAttribute('src')
    }
    setMode('transcode')
  }, [])

  if (isLoading) {
    return (
      <div className="flex items-center justify-center rounded-lg bg-black/50 p-12">
        <div className="h-8 w-8 animate-spin rounded-full border-4 border-slate-600 border-t-blue-500" />
        <span className="ml-3 text-slate-400">Analyzing media...</span>
      </div>
    )
  }

  if (infoError) {
    return (
      <div className="rounded-lg bg-red-900/30 p-6 text-red-400">
        Failed to load media info: {infoError.message}
      </div>
    )
  }

  return (
    <div className="space-y-4">
      {/* Video element */}
      <div className="relative overflow-hidden rounded-lg bg-black">
        {mode === 'error' ? (
          <div className="flex items-center justify-center p-12 text-red-400">
            {errorMsg || 'Playback error'}
          </div>
        ) : (
          <video
            ref={videoRef}
            controls
            className="h-auto w-full"
            playsInline
          >
            {/* Subtitle tracks */}
            {streamInfo?.subtitleStreams
              .filter((s) => !['hdmv_pgs_subtitle', 'pgssub', 'dvb_subtitle', 'dvdsub'].includes(s.codec))
              .map((sub) => (
                <track
                  key={sub.index}
                  kind="subtitles"
                  src={`/api/v1/stream/${mediaFileId}/subtitles/${sub.index}`}
                  srcLang={sub.language}
                  label={sub.title || sub.language}
                  default={sub.isDefault}
                />
              ))}
          </video>
        )}

        {/* Mode badge */}
        {mode !== 'error' && mode !== 'detecting' && (
          <div className="absolute top-3 right-3 rounded bg-black/70 px-2 py-1 text-xs text-slate-300">
            {mode === 'direct' ? 'Direct Play' : 'Transcoding'}
          </div>
        )}
      </div>

      {/* Controls bar */}
      {streamInfo && (
        <div className="flex flex-wrap items-center gap-4 rounded-lg bg-slate-800 p-3">
          {/* Audio track selector */}
          {streamInfo.audioStreams.length > 1 && (
            <label className="flex items-center gap-2 text-sm text-slate-300">
              <span>Audio:</span>
              <select
                value={selectedAudio}
                onChange={(e) => setSelectedAudio(Number(e.target.value))}
                className="rounded bg-slate-700 px-2 py-1 text-sm text-white"
              >
                {streamInfo.audioStreams.map((a) => (
                  <option key={a.index} value={a.index}>
                    {a.title || a.language} ({a.codec.toUpperCase()} {channelLayout(a.channels)})
                  </option>
                ))}
              </select>
            </label>
          )}

          {/* Subtitle selector */}
          {streamInfo.subtitleStreams.length > 0 && (
            <label className="flex items-center gap-2 text-sm text-slate-300">
              <span>Subtitles:</span>
              <select
                value={selectedSub ?? -1}
                onChange={(e) => {
                  const v = Number(e.target.value)
                  setSelectedSub(v >= 0 ? v : null)
                }}
                className="rounded bg-slate-700 px-2 py-1 text-sm text-white"
              >
                <option value={-1}>None</option>
                {streamInfo.subtitleStreams.map((s) => (
                  <option key={s.index} value={s.index}>
                    {s.title || s.language} ({s.codec}) {s.forced ? '[Forced]' : ''}
                  </option>
                ))}
              </select>
            </label>
          )}

          {/* Force transcode button (when in direct play mode) */}
          {mode === 'direct' && (
            <button
              onClick={handleForceTranscode}
              className="rounded bg-slate-700 px-3 py-1 text-sm text-slate-300 hover:bg-slate-600"
            >
              Force Transcode
            </button>
          )}
        </div>
      )}

      {/* Stream info panel */}
      {streamInfo && (
        <div className="rounded-lg bg-slate-800/50 p-4">
          <h3 className="mb-2 text-sm font-medium text-slate-300">Media Info</h3>
          <div className="grid grid-cols-2 gap-x-6 gap-y-1 text-xs text-slate-400 md:grid-cols-4">
            <span>Container: {streamInfo.container}</span>
            <span>Duration: {formatDuration(streamInfo.durationSecs)}</span>
            <span>Bitrate: {formatBitrate(streamInfo.bitrate)}</span>
            {streamInfo.videoStreams[0] && (
              <>
                <span>
                  Video: {streamInfo.videoStreams[0].codec.toUpperCase()}{' '}
                  {streamInfo.videoStreams[0].width}x{streamInfo.videoStreams[0].height}
                  {streamInfo.videoStreams[0].isHdr ? ' HDR' : ''}
                </span>
                <span>
                  Frame Rate: {streamInfo.videoStreams[0].frameRate.toFixed(2)} fps
                </span>
              </>
            )}
            {streamInfo.audioStreams[0] && (
              <span>
                Audio: {streamInfo.audioStreams[0].codec.toUpperCase()}{' '}
                {channelLayout(streamInfo.audioStreams[0].channels)}
              </span>
            )}
          </div>
        </div>
      )}
    </div>
  )
}
