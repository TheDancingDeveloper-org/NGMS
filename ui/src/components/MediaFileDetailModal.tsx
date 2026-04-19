import { X, Film, Volume2, Subtitles, FileText, Info } from 'lucide-react'
import type { MediaFile, MediaStreamInfo } from '../api/types'
import { qualityName } from '../api/types'

interface MediaFileDetailModalProps {
  file: MediaFile
  onClose: () => void
}

function formatSize(bytes: number): string {
  if (!bytes || !isFinite(bytes) || bytes <= 0) return '-'
  if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(2)} GB`
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}

function formatBitrate(bps: number): string {
  if (!bps || bps <= 0) return '-'
  if (bps >= 1_000_000) return `${(bps / 1_000_000).toFixed(1)} Mbps`
  if (bps >= 1_000) return `${(bps / 1_000).toFixed(0)} kbps`
  return `${bps} bps`
}

function formatDuration(secs: number): string {
  if (!secs || secs <= 0) return '-'
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  const s = Math.floor(secs % 60)
  if (h > 0) return `${h}h ${m}m ${s}s`
  return `${m}m ${s}s`
}

function formatChannels(channels: number): string {
  switch (channels) {
    case 1: return 'Mono'
    case 2: return 'Stereo'
    case 6: return '5.1'
    case 8: return '7.1'
    default: return `${channels}ch`
  }
}

function formatCodec(codec: string): string {
  const map: Record<string, string> = {
    h264: 'H.264',
    h265: 'H.265 / HEVC',
    hevc: 'H.265 / HEVC',
    av1: 'AV1',
    vp9: 'VP9',
    aac: 'AAC',
    ac3: 'AC3 / Dolby Digital',
    eac3: 'EAC3 / Dolby Digital+',
    truehd: 'TrueHD',
    dts: 'DTS',
    'dts-hd ma': 'DTS-HD MA',
    flac: 'FLAC',
    opus: 'Opus',
    subrip: 'SRT',
    srt: 'SRT',
    ass: 'ASS/SSA',
    hdmv_pgs_subtitle: 'PGS',
    pgssub: 'PGS',
    dvd_subtitle: 'VobSub',
  }
  return map[codec.toLowerCase()] || codec.toUpperCase()
}

function formatLanguages(langs: unknown): string {
  if (!langs) return '-'
  if (Array.isArray(langs)) {
    if (langs.length === 0) return '-'
    return langs.map((l) => {
      if (typeof l === 'string') return l
      if (l && typeof l === 'object' && 'name' in l) return String(l.name)
      if (l && typeof l === 'object' && 'language' in l) return String(l.language)
      return String(l)
    }).join(', ')
  }
  if (typeof langs === 'string') return langs || '-'
  return '-'
}

function formatDate(dateStr: string): string {
  try {
    return new Date(dateStr).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    })
  } catch {
    return dateStr
  }
}

function InfoRow({ label, value }: { label: string; value: string | undefined | null }) {
  if (!value || value === '-') return null
  return (
    <div className="flex justify-between gap-4 py-1.5 border-b border-slate-700/50 last:border-b-0">
      <span className="text-slate-400 text-sm shrink-0">{label}</span>
      <span className="text-sm text-white text-right break-all">{value}</span>
    </div>
  )
}

function SectionHeader({ icon, title }: { icon: React.ReactNode; title: string }) {
  return (
    <div className="flex items-center gap-2 mb-2 mt-4 first:mt-0">
      <span className="text-blue-400">{icon}</span>
      <h4 className="text-sm font-semibold text-slate-200 uppercase tracking-wide">{title}</h4>
    </div>
  )
}

function VideoStreamsSection({ info }: { info: MediaStreamInfo }) {
  if (!info.videoStreams || info.videoStreams.length === 0) return null
  return (
    <div>
      <SectionHeader icon={<Film size={16} />} title="Video" />
      {info.videoStreams.map((vs, i) => (
        <div key={i} className="rounded-lg bg-slate-800/50 px-3 py-2 mb-2">
          <InfoRow label="Codec" value={formatCodec(vs.codec)} />
          <InfoRow label="Resolution" value={`${vs.width} x ${vs.height}`} />
          <InfoRow label="Bitrate" value={formatBitrate(vs.bitrate)} />
          <InfoRow label="Frame Rate" value={vs.frameRate > 0 ? `${vs.frameRate.toFixed(2)} fps` : undefined} />
          <InfoRow label="Profile" value={vs.profile || undefined} />
          <InfoRow label="HDR" value={vs.isHdr ? 'Yes' : undefined} />
        </div>
      ))}
    </div>
  )
}

function AudioStreamsSection({ info }: { info: MediaStreamInfo }) {
  if (!info.audioStreams || info.audioStreams.length === 0) return null
  return (
    <div>
      <SectionHeader icon={<Volume2 size={16} />} title="Audio" />
      {info.audioStreams.map((as_, i) => (
        <div key={i} className="rounded-lg bg-slate-800/50 px-3 py-2 mb-2">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-xs font-medium text-slate-300">
              {as_.language || 'Unknown'}{as_.isDefault ? ' (default)' : ''}
            </span>
            {as_.title && (
              <span className="text-xs text-slate-500">{as_.title}</span>
            )}
          </div>
          <InfoRow label="Codec" value={formatCodec(as_.codec)} />
          <InfoRow label="Channels" value={formatChannels(as_.channels)} />
          <InfoRow label="Bitrate" value={formatBitrate(as_.bitrate)} />
        </div>
      ))}
    </div>
  )
}

function SubtitleStreamsSection({ info }: { info: MediaStreamInfo }) {
  if (!info.subtitleStreams || info.subtitleStreams.length === 0) return null
  return (
    <div>
      <SectionHeader icon={<Subtitles size={16} />} title="Subtitles" />
      <div className="rounded-lg bg-slate-800/50 px-3 py-2">
        {info.subtitleStreams.map((sub, i) => (
          <div key={i} className="flex items-center gap-3 py-1.5 border-b border-slate-700/50 last:border-b-0">
            <span className="text-sm text-white">{sub.language || 'Unknown'}</span>
            <span className="text-xs text-slate-500">{formatCodec(sub.codec)}</span>
            {sub.forced && (
              <span className="rounded bg-yellow-500/20 px-1.5 py-0.5 text-xs text-yellow-400">Forced</span>
            )}
            {sub.isDefault && (
              <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-xs text-blue-400">Default</span>
            )}
            {sub.title && (
              <span className="text-xs text-slate-500 ml-auto truncate max-w-48">{sub.title}</span>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}

export default function MediaFileDetailModal({ file, onClose }: MediaFileDetailModalProps) {
  const info = file.mediaInfo as MediaStreamInfo | null | undefined

  // Check if mediaInfo has the expected streaming shape (videoStreams array)
  const hasStreamInfo = info && Array.isArray(info.videoStreams)

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70" onClick={onClose}>
      <div
        className="relative max-h-[85vh] w-full max-w-2xl overflow-y-auto rounded-xl border border-slate-700 bg-slate-900 p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Close */}
        <button
          onClick={onClose}
          className="absolute right-4 top-4 text-slate-400 hover:text-white transition-colors"
        >
          <X size={20} />
        </button>

        <h3 className="text-lg font-bold text-white pr-8 mb-4">Media File Details</h3>

        {/* File Info */}
        <SectionHeader icon={<FileText size={16} />} title="File" />
        <div className="rounded-lg bg-slate-800/50 px-3 py-2">
          <InfoRow label="Filename" value={file.relativePath?.split('/').pop()} />
          <InfoRow label="Path" value={file.relativePath} />
          <InfoRow label="Size" value={formatSize(file.size)} />
          <InfoRow label="Quality" value={qualityName(file.quality)} />
          <InfoRow label="Languages" value={formatLanguages(file.languages)} />
          <InfoRow label="Release Group" value={file.releaseGroup || undefined} />
          <InfoRow label="Scene Name" value={file.sceneName || undefined} />
          <InfoRow label="Edition" value={file.edition || undefined} />
          <InfoRow label="Date Added" value={file.dateAdded ? formatDate(file.dateAdded) : undefined} />
        </div>

        {/* Media Info from ffprobe */}
        {hasStreamInfo ? (
          <>
            {/* Overview */}
            <SectionHeader icon={<Info size={16} />} title="Media Info" />
            <div className="rounded-lg bg-slate-800/50 px-3 py-2">
              <InfoRow label="Container" value={info.container} />
              <InfoRow label="Duration" value={formatDuration(info.durationSecs)} />
              <InfoRow label="Overall Bitrate" value={formatBitrate(info.bitrate)} />
            </div>

            <VideoStreamsSection info={info} />
            <AudioStreamsSection info={info} />
            <SubtitleStreamsSection info={info} />
          </>
        ) : (
          <div className="mt-4 rounded-lg border border-slate-700/50 bg-slate-800/30 px-4 py-3">
            <p className="text-xs text-slate-500">
              Detailed codec/stream information is not yet available for this file.
              It will be populated automatically when the file is played via the streaming engine.
            </p>
          </div>
        )}
      </div>
    </div>
  )
}
