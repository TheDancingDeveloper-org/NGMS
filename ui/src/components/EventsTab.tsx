import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  Download,
  Upload,
  Trash2,
  ArrowUpCircle,
  XCircle,
  FileText,
  Eye,
  ExternalLink,
  Loader2,
} from 'lucide-react'
import type { HistoryEvent } from '../api/types'
import { qualityName } from '../api/types'
import HistoryDetailModal from './HistoryDetailModal'

function relativeTime(dateStr: string): string {
  const diff = (Date.now() - new Date(dateStr).getTime()) / 1000
  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`
  return new Date(dateStr).toLocaleDateString()
}

function eventIcon(type: string) {
  switch (type) {
    case 'grabbed':
      return <Download size={14} />
    case 'imported':
    case 'downloadImported':
      return <Upload size={14} />
    case 'importStarted':
      return <Loader2 size={14} />
    case 'fileDeleted':
      return <Trash2 size={14} />
    case 'upgraded':
      return <ArrowUpCircle size={14} />
    case 'downloadFailed':
      return <XCircle size={14} />
    case 'fileRenamed':
      return <FileText size={14} />
    case 'downloadIgnored':
      return <Eye size={14} />
    default:
      return <FileText size={14} />
  }
}

function eventStyle(type: string) {
  switch (type) {
    case 'grabbed':
      return { icon: 'bg-blue-900/60 text-blue-400', label: 'Grabbed', labelColor: 'text-blue-400' }
    case 'imported':
    case 'downloadImported':
      return { icon: 'bg-green-900/60 text-green-400', label: 'Imported', labelColor: 'text-green-400' }
    case 'importStarted':
      return { icon: 'bg-slate-700 text-slate-400', label: 'Importing', labelColor: 'text-slate-400' }
    case 'fileDeleted':
      return { icon: 'bg-orange-900/60 text-orange-400', label: 'Upgraded', labelColor: 'text-orange-400' }
    case 'upgraded':
      return { icon: 'bg-cyan-900/60 text-cyan-400', label: 'Upgraded', labelColor: 'text-cyan-400' }
    case 'downloadFailed':
      return { icon: 'bg-red-900/60 text-red-400', label: 'Failed', labelColor: 'text-red-400' }
    case 'fileRenamed':
      return { icon: 'bg-purple-900/60 text-purple-400', label: 'Renamed', labelColor: 'text-purple-400' }
    case 'downloadIgnored':
      return { icon: 'bg-yellow-900/60 text-yellow-400', label: 'Ignored', labelColor: 'text-yellow-400' }
    default:
      return { icon: 'bg-slate-700 text-slate-400', label: type, labelColor: 'text-slate-400' }
  }
}

/** Build a context line from the event's data field for upgrade/delete events. */
function upgradeContext(event: HistoryEvent): string | null {
  if (!event.data) return null
  const d = event.data
  if (d.reason === 'upgrade' && d.replaced_by_quality) {
    const recycled = d.recycled ? 'moved to recycle bin' : 'permanently deleted'
    return `Replaced by ${d.replaced_by_quality} (${recycled})`
  }
  return null
}

/** Extract error message from a failed event's data field. */
function failureReason(event: HistoryEvent): string | null {
  if (event.eventType !== 'downloadFailed' || !event.data) return null
  const d = event.data
  const msg = (d.message ?? d.error ?? d.error_message) as string | undefined
  return msg || null
}

/** Get the media detail page link for an event. */
function mediaLink(event: HistoryEvent): string | null {
  if (event.mediaType === 'series' && event.seriesId) return `/series/${event.seriesId}`
  if (event.mediaType === 'movie' && event.movieId) return `/movies/${event.movieId}`
  return null
}

export default function EventsTab({ events }: { events: HistoryEvent[] }) {
  const navigate = useNavigate()
  const [detailEvent, setDetailEvent] = useState<HistoryEvent | null>(null)

  if (events.length === 0) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-slate-500">
        No recent events
      </div>
    )
  }

  return (
    <>
      <div>
        {events.map((event) => {
          const style = eventStyle(event.eventType)
          const quality = qualityName(event.quality)
          const context = upgradeContext(event)
          const error = failureReason(event)
          const link = mediaLink(event)

          return (
            <div
              key={event.id}
              className="flex gap-3 border-b border-slate-700 px-4 py-2.5 transition-colors hover:bg-slate-700/30 cursor-pointer"
              onClick={() => setDetailEvent(event)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') setDetailEvent(event)
              }}
            >
              {/* Icon */}
              <div
                className={`flex h-7 w-7 flex-shrink-0 items-center justify-center rounded-full ${style.icon}`}
              >
                {eventIcon(event.eventType)}
              </div>

              {/* Content */}
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className={`text-[11px] font-semibold uppercase ${style.labelColor}`}>
                    {style.label}
                  </span>
                  {quality && quality !== 'Unknown' && (
                    <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] font-medium text-blue-400">
                      {quality}
                    </span>
                  )}
                </div>
                <div className="truncate text-[12px] text-slate-300" title={event.sourceTitle}>
                  {event.sourceTitle}
                </div>
                {context && (
                  <div className="truncate text-[11px] text-slate-500">{context}</div>
                )}
                {/* Show truncated error reason for failed events */}
                {error && (
                  <div className="truncate text-[11px] text-red-400/80 mt-0.5" title={error}>
                    {error}
                  </div>
                )}
                <div className="mt-0.5 flex items-center gap-2">
                  <span className="text-[10px] text-slate-600">
                    {relativeTime(event.date)}
                  </span>
                  {/* Quick-nav link to series/movie */}
                  {link && (
                    <button
                      className="flex items-center gap-0.5 text-[10px] text-blue-500 hover:text-blue-400 transition-colors"
                      title={`View ${event.mediaType === 'series' ? 'series' : 'movie'}`}
                      onClick={(e) => {
                        e.stopPropagation()
                        navigate(link)
                      }}
                    >
                      <ExternalLink size={9} />
                      <span>{event.mediaType === 'series' ? 'Series' : 'Movie'}</span>
                    </button>
                  )}
                </div>
              </div>
            </div>
          )
        })}
      </div>

      {/* Detail modal */}
      {detailEvent && (
        <HistoryDetailModal
          event={detailEvent}
          onClose={() => setDetailEvent(null)}
        />
      )}
    </>
  )
}
