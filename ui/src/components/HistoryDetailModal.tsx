// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useNavigate } from 'react-router-dom'
import {
  X,
  Download,
  Upload,
  Trash2,
  ArrowUpCircle,
  XCircle,
  FileText,
  Eye,
  ExternalLink,
} from 'lucide-react'
import type { HistoryEvent } from '../api/types'
import { qualityName } from '../api/types'
import { formatDateTime } from '../utils/date'

interface HistoryDetailModalProps {
  event: HistoryEvent
  onClose: () => void
}

function eventIcon(type: string) {
  switch (type) {
    case 'grabbed':
      return <Download size={18} />
    case 'imported':
      return <Upload size={18} />
    case 'fileDeleted':
      return <Trash2 size={18} />
    case 'upgraded':
      return <ArrowUpCircle size={18} />
    case 'downloadFailed':
      return <XCircle size={18} />
    case 'fileRenamed':
      return <FileText size={18} />
    case 'downloadIgnored':
      return <Eye size={18} />
    default:
      return <FileText size={18} />
  }
}

function eventStyle(type: string) {
  switch (type) {
    case 'grabbed':
      return { bg: 'bg-blue-900/60 text-blue-400', label: 'Grabbed', labelColor: 'text-blue-400' }
    case 'imported':
      return { bg: 'bg-green-900/60 text-green-400', label: 'Imported', labelColor: 'text-green-400' }
    case 'fileDeleted':
      return { bg: 'bg-orange-900/60 text-orange-400', label: 'File Deleted', labelColor: 'text-orange-400' }
    case 'upgraded':
      return { bg: 'bg-cyan-900/60 text-cyan-400', label: 'Upgraded', labelColor: 'text-cyan-400' }
    case 'downloadFailed':
      return { bg: 'bg-red-900/60 text-red-400', label: 'Download Failed', labelColor: 'text-red-400' }
    case 'fileRenamed':
      return { bg: 'bg-purple-900/60 text-purple-400', label: 'Renamed', labelColor: 'text-purple-400' }
    case 'downloadIgnored':
      return { bg: 'bg-yellow-900/60 text-yellow-400', label: 'Ignored', labelColor: 'text-yellow-400' }
    default:
      return { bg: 'bg-slate-700 text-slate-400', label: type, labelColor: 'text-slate-400' }
  }
}

function mediaLink(event: HistoryEvent): string | null {
  if (event.mediaType === 'series' && event.seriesId) return `/series/${event.seriesId}`
  if (event.mediaType === 'movie' && event.movieId) return `/movies/${event.movieId}`
  return null
}

function InfoRow({ label, value }: { label: string; value: React.ReactNode }) {
  if (!value) return null
  return (
    <div className="flex gap-3 py-2 border-b border-slate-700/50 last:border-0">
      <span className="w-32 shrink-0 text-xs font-medium text-slate-500 uppercase tracking-wide pt-0.5">
        {label}
      </span>
      <span className="text-sm text-slate-200 break-all min-w-0">{value}</span>
    </div>
  )
}

export default function HistoryDetailModal({ event, onClose }: HistoryDetailModalProps) {
  const navigate = useNavigate()
  const style = eventStyle(event.eventType)
  const quality = qualityName(event.quality)
  const link = mediaLink(event)
  const data = event.data ?? {}

  // Extract typed data fields
  const errorMessage = (data.message ?? data.error ?? data.error_message) as string | undefined
  const sourcePath = data.imported_from as string | undefined
  const destPath = data.imported_to as string | undefined
  const reason = data.reason as string | undefined
  const replacedByQuality = data.replaced_by_quality as string | undefined
  const recycled = data.recycled as boolean | undefined
  const nzbName = data.nzb_name as string | undefined
  const releaseGroup = data.release_group as string | undefined
  const downloadUrl = data.download_url as string | undefined

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-16 px-4"
      onClick={onClose}
    >
      <div
        className="flex w-full max-w-lg flex-col rounded-xl bg-slate-800 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-700 px-5 py-4">
          <div className="flex items-center gap-3 min-w-0">
            <div className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-full ${style.bg}`}>
              {eventIcon(event.eventType)}
            </div>
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span className={`text-sm font-semibold ${style.labelColor}`}>{style.label}</span>
                {quality && quality !== 'Unknown' && (
                  <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] font-medium text-blue-400">
                    {quality}
                  </span>
                )}
              </div>
              <p className="text-xs text-slate-400">{formatDateTime(event.date)}</p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white p-1 transition-colors"
          >
            <X size={18} />
          </button>
        </div>

        {/* Error banner for failed events */}
        {event.eventType === 'downloadFailed' && errorMessage && (
          <div className="mx-5 mt-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3">
            <p className="text-xs font-medium text-red-400 uppercase mb-1">Error</p>
            <p className="text-sm text-red-300">{errorMessage}</p>
          </div>
        )}

        {/* Details */}
        <div className="px-5 py-4 space-y-0">
          <InfoRow label="Release" value={event.sourceTitle} />
          <InfoRow label="Event Type" value={style.label} />
          <InfoRow label="Media Type" value={event.mediaType === 'series' ? 'TV Series' : 'Movie'} />
          <InfoRow
            label="Quality"
            value={
              quality && quality !== 'Unknown' ? (
                <span className="rounded bg-blue-500/20 px-2 py-0.5 text-xs font-medium text-blue-400">
                  {quality}
                </span>
              ) : null
            }
          />
          <InfoRow label="Indexer" value={event.indexer || null} />
          <InfoRow label="Download Client" value={event.downloadClient || null} />
          <InfoRow label="Date" value={formatDateTime(event.date)} />

          {/* Grabbed-specific */}
          {event.eventType === 'grabbed' && (
            <>
              <InfoRow label="NZB / Torrent" value={nzbName || null} />
              <InfoRow label="Release Group" value={releaseGroup || null} />
              {downloadUrl && (
                <InfoRow
                  label="Download URL"
                  value={
                    <span className="truncate text-blue-400">{downloadUrl}</span>
                  }
                />
              )}
            </>
          )}

          {/* Imported-specific */}
          {event.eventType === 'imported' && (
            <>
              <InfoRow label="Source Path" value={sourcePath || null} />
              <InfoRow label="Destination" value={destPath || null} />
            </>
          )}

          {/* Deleted/Upgraded-specific */}
          {(event.eventType === 'fileDeleted' || event.eventType === 'upgraded') && (
            <>
              {reason && <InfoRow label="Reason" value={reason} />}
              {replacedByQuality && (
                <InfoRow
                  label="Replaced By"
                  value={
                    <span className="rounded bg-cyan-500/20 px-2 py-0.5 text-xs font-medium text-cyan-400">
                      {replacedByQuality}
                    </span>
                  }
                />
              )}
              {recycled !== undefined && (
                <InfoRow label="Recycled" value={recycled ? 'Moved to recycle bin' : 'Permanently deleted'} />
              )}
            </>
          )}

          {/* Failed-specific (if no message already shown in banner) */}
          {event.eventType === 'downloadFailed' && !errorMessage && (
            <InfoRow label="Error" value={<span className="text-red-400">Unknown error</span>} />
          )}

          {/* Raw data (for any other fields not explicitly shown) */}
          {data && Object.keys(data).length > 0 && (
            <details className="pt-2">
              <summary className="cursor-pointer text-xs text-slate-500 hover:text-slate-400 transition-colors">
                Raw event data
              </summary>
              <pre className="mt-2 rounded-lg bg-slate-900 p-3 text-xs text-slate-400 overflow-x-auto max-h-40 overflow-y-auto">
                {JSON.stringify(data, null, 2)}
              </pre>
            </details>
          )}
        </div>

        {/* Footer with navigation link */}
        <div className="flex items-center justify-end gap-3 border-t border-slate-700 px-5 py-3">
          {link && (
            <button
              onClick={() => {
                onClose()
                navigate(link)
              }}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
            >
              <ExternalLink size={14} />
              View {event.mediaType === 'series' ? 'Series' : 'Movie'}
            </button>
          )}
          <button
            onClick={onClose}
            className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  )
}
