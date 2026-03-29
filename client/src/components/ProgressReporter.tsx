import { useEffect, useRef, useCallback } from 'react'
import { api } from '../api'

interface ProgressReporterProps {
  videoRef: React.RefObject<HTMLVideoElement | null>
  mediaFileId: number
  intervalMs?: number
}

/**
 * Reports video playback position to the server every `intervalMs` (default 10s),
 * and on pause/ended events.
 */
export default function ProgressReporter({
  videoRef,
  mediaFileId,
  intervalMs = 10_000,
}: ProgressReporterProps) {
  const lastReported = useRef(0)

  const report = useCallback(() => {
    const video = videoRef.current
    if (!video || video.readyState < 1) return
    const pos = video.currentTime
    const dur = video.duration
    if (!dur || isNaN(dur) || dur <= 0) return
    // Avoid spamming identical reports
    if (Math.abs(pos - lastReported.current) < 1) return
    lastReported.current = pos
    api.updateProgress(mediaFileId, pos, dur).catch(() => {})
  }, [videoRef, mediaFileId])

  // Periodic reporting
  useEffect(() => {
    const timer = setInterval(report, intervalMs)
    return () => clearInterval(timer)
  }, [report, intervalMs])

  // Report on pause and ended
  useEffect(() => {
    const video = videoRef.current
    if (!video) return

    const handler = () => report()
    video.addEventListener('pause', handler)
    video.addEventListener('ended', handler)
    return () => {
      video.removeEventListener('pause', handler)
      video.removeEventListener('ended', handler)
    }
  }, [videoRef, report])

  // Report on unmount
  useEffect(() => {
    return () => { report() }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return null
}
