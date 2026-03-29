import { useEffect, useState, useRef } from 'react'
import Hls from 'hls.js'

interface StreamStatsProps {
  hls: Hls | null
  videoRef: React.RefObject<HTMLVideoElement | null>
  encoder: string | null
  visible: boolean
}

interface Stats {
  bandwidth: number
  bufferAhead: number
  droppedFrames: number
  totalFrames: number
  currentLevel: number
  levelHeight: number
  levelBitrate: number
  segmentLoadTime: number
  autoLevel: boolean
}

export default function StreamStats({ hls, videoRef, encoder, visible }: StreamStatsProps) {
  const [stats, setStats] = useState<Stats | null>(null)
  const lastFragLoadRef = useRef(0)

  useEffect(() => {
    if (!hls || !visible) return

    // Track segment load times via FRAG_LOADED event
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const onFragLoaded = (...args: any[]) => {
      try {
        const data = args[1]
        if (data?.frag?.stats?.loading) {
          const loadMs = data.frag.stats.loading.end - data.frag.stats.loading.start
          lastFragLoadRef.current = loadMs
        }
      } catch { /* ignore */ }
    }
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    ;(hls as any).on('hlsFragLoaded', onFragLoaded)

    const interval = setInterval(() => {
      if (!hls || !videoRef.current) return

      const video = videoRef.current
      const buffered = video.buffered
      let bufferAhead = 0
      if (buffered.length > 0) {
        bufferAhead = buffered.end(buffered.length - 1) - video.currentTime
      }

      const quality = (video as HTMLVideoElement & { getVideoPlaybackQuality?: () => { droppedVideoFrames: number; totalVideoFrames: number } }).getVideoPlaybackQuality?.()
      const level = hls.levels[hls.currentLevel]

      setStats({
        bandwidth: hls.bandwidthEstimate || 0,
        bufferAhead: Math.max(0, bufferAhead),
        droppedFrames: quality?.droppedVideoFrames ?? 0,
        totalFrames: quality?.totalVideoFrames ?? 0,
        currentLevel: hls.currentLevel,
        levelHeight: level?.height ?? 0,
        levelBitrate: level?.bitrate ?? 0,
        segmentLoadTime: lastFragLoadRef.current,
        autoLevel: hls.autoLevelEnabled,
      })
    }, 2000)

    return () => {
      clearInterval(interval)
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ;(hls as any).off('hlsFragLoaded', onFragLoaded)
    }
  }, [hls, videoRef, visible])

  if (!visible || !stats) return null

  const fmt = (bps: number) => bps >= 1_000_000
    ? `${(bps / 1_000_000).toFixed(1)} Mbps`
    : `${(bps / 1_000).toFixed(0)} kbps`

  return (
    <div style={{
      position: 'absolute', top: 12, left: 12, zIndex: 10,
      background: 'rgba(0, 0, 0, 0.8)', borderRadius: 8,
      padding: '8px 12px', fontSize: 11, color: '#94a3b8',
      fontFamily: 'monospace', lineHeight: 1.6, pointerEvents: 'none',
    }}>
      <div style={{ color: '#3b82f6', fontWeight: 600, marginBottom: 2 }}>Stream Stats</div>
      <div>Quality: {stats.levelHeight}p {stats.autoLevel ? '(Auto)' : '(Manual)'}</div>
      <div>Bitrate: {fmt(stats.levelBitrate)}</div>
      <div>Bandwidth: {fmt(stats.bandwidth)}</div>
      <div>Buffer: {stats.bufferAhead.toFixed(1)}s</div>
      <div>Segment load: {stats.segmentLoadTime.toFixed(0)}ms</div>
      {stats.droppedFrames > 0 && (
        <div style={{ color: '#fbbf24' }}>
          Dropped: {stats.droppedFrames}/{stats.totalFrames}
        </div>
      )}
      {encoder && <div>Encoder: {encoder}</div>}
    </div>
  )
}
