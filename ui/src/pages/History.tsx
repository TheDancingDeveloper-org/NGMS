import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Clock, Loader2, ExternalLink } from 'lucide-react'
import { useHistory } from '../hooks/useApi'
import { qualityName } from '../api/types'
import type { HistoryEvent } from '../api/types'
import { formatDate, formatTime } from '../utils/date'
import HistoryDetailModal from '../components/HistoryDetailModal'

/** Get the media detail page link for an event. */
function mediaLink(event: HistoryEvent): string | null {
  if (event.mediaType === 'series' && event.seriesId) return `/series/${event.seriesId}`
  if (event.mediaType === 'movie' && event.movieId) return `/movies/${event.movieId}`
  return null
}

/** Extract error summary from a failed event's data field. */
function failureSummary(event: HistoryEvent): string | null {
  if (event.eventType !== 'downloadFailed' || !event.data) return null
  const d = event.data
  const msg = (d.message ?? d.error ?? d.error_message) as string | undefined
  return msg || null
}

export default function History() {
  const [page, setPage] = useState(1)
  const { data, isLoading, error, refetch } = useHistory(page)
  const navigate = useNavigate()
  const [detailEvent, setDetailEvent] = useState<HistoryEvent | null>(null)
  const [clearing, setClearing] = useState(false)

  const clearHistory = async () => {
    if (!data || data.totalRecords === 0) return
    if (!confirm(`Delete all ${data.totalRecords} history events?`)) return
    setClearing(true)
    await fetch('/api/v1/history', { method: 'DELETE' })
    setClearing(false)
    setPage(1)
    void refetch()
  }

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h2 className="text-2xl font-bold">History</h2>
        {data && data.totalRecords > 0 && (
          <button
            onClick={clearHistory}
            disabled={clearing}
            className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 disabled:opacity-50 transition-colors"
          >
            Clear All
          </button>
        )}
      </div>

      {isLoading && page === 1 && (
        <div className="flex items-center justify-center py-20">
          <Loader2 size={32} className="animate-spin text-blue-500" />
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
          Failed to load history: {error.message}
        </div>
      )}

      {!isLoading && !error && data && data.records.length === 0 && page === 1 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <Clock size={48} className="mb-4 text-slate-600" />
          <p>No history events yet</p>
        </div>
      )}

      {data && data.records.length > 0 && (
        <>
          <div className="overflow-x-auto rounded-lg bg-slate-800">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                  <th className="px-4 py-3 font-medium">Date</th>
                  <th className="px-4 py-3 font-medium">Event</th>
                  <th className="px-4 py-3 font-medium">Title</th>
                  <th className="px-4 py-3 font-medium">Quality</th>
                  <th className="px-4 py-3 font-medium">Indexer</th>
                  <th className="px-4 py-3 font-medium">Media</th>
                </tr>
              </thead>
              <tbody>
                {data.records.map((event) => {
                  const link = mediaLink(event)
                  const errorMsg = failureSummary(event)

                  return (
                    <tr
                      key={event.id}
                      className="border-b border-slate-700/50 hover:bg-slate-700/30 transition-colors cursor-pointer"
                      onClick={() => setDetailEvent(event)}
                      role="button"
                      tabIndex={0}
                      onKeyDown={(e) => {
                        if (e.key === 'Enter' || e.key === ' ') setDetailEvent(event)
                      }}
                    >
                      <td className="px-4 py-3 text-slate-300 whitespace-nowrap">
                        {formatDate(event.date)}{' '}
                        <span className="text-slate-500">
                          {formatTime(event.date)}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <EventBadge type={event.eventType} />
                      </td>
                      <td className="px-4 py-3 max-w-xs">
                        <div className="font-medium text-white truncate">{event.sourceTitle}</div>
                        {/* Show error reason inline for failed events */}
                        {errorMsg && (
                          <div className="truncate text-xs text-red-400/80 mt-0.5" title={errorMsg}>
                            {errorMsg}
                          </div>
                        )}
                      </td>
                      <td className="px-4 py-3">
                        {event.quality && (
                          <span className="rounded bg-blue-500/20 px-2 py-0.5 text-xs font-medium text-blue-400">
                            {qualityName(event.quality)}
                          </span>
                        )}
                      </td>
                      <td className="px-4 py-3 text-slate-400">{event.indexer || '-'}</td>
                      <td className="px-4 py-3">
                        {link && (
                          <button
                            className="flex items-center gap-1 text-xs text-blue-500 hover:text-blue-400 transition-colors"
                            title={`View ${event.mediaType === 'series' ? 'series' : 'movie'}`}
                            onClick={(e) => {
                              e.stopPropagation()
                              navigate(link)
                            }}
                          >
                            <ExternalLink size={12} />
                            <span>{event.mediaType === 'series' ? 'Series' : 'Movie'}</span>
                          </button>
                        )}
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>

          {/* Pagination */}
          <div className="mt-4 flex items-center justify-between">
            <span className="text-sm text-slate-400">
              Showing {(page - 1) * 20 + 1}-{Math.min(page * 20, data.totalRecords)} of{' '}
              {data.totalRecords}
            </span>
            <div className="flex gap-2">
              <button
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                disabled={page === 1}
                className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 disabled:opacity-50 transition-colors"
              >
                Previous
              </button>
              <button
                onClick={() => setPage((p) => p + 1)}
                disabled={page * 20 >= data.totalRecords}
                className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm font-medium text-slate-300 hover:bg-slate-600 disabled:opacity-50 transition-colors"
              >
                Next
              </button>
            </div>
          </div>
        </>
      )}

      {/* Detail modal */}
      {detailEvent && (
        <HistoryDetailModal
          event={detailEvent}
          onClose={() => setDetailEvent(null)}
        />
      )}
    </div>
  )
}

function EventBadge({ type }: { type: string }) {
  const styles: Record<string, string> = {
    grabbed: 'bg-blue-500/20 text-blue-400',
    downloaded: 'bg-green-500/20 text-green-400',
    downloadFailed: 'bg-red-500/20 text-red-400',
    deleted: 'bg-red-500/20 text-red-400',
    renamed: 'bg-purple-500/20 text-purple-400',
    upgraded: 'bg-cyan-500/20 text-cyan-400',
    imported: 'bg-green-500/20 text-green-400',
    ignored: 'bg-yellow-500/20 text-yellow-400',
  }
  return (
    <span
      className={`rounded-full px-2 py-0.5 text-xs font-medium capitalize ${
        styles[type] ?? 'bg-slate-600 text-slate-300'
      }`}
    >
      {type.replace(/([A-Z])/g, ' $1').trim()}
    </span>
  )
}
