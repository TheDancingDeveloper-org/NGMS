import { useState } from 'react'
import { Clock, Loader2 } from 'lucide-react'
import { useHistory } from '../hooks/useApi'
import { qualityName } from '../api/types'
import { formatDate, formatTime } from '../utils/date'

export default function History() {
  const [page, setPage] = useState(1)
  const { data, isLoading, error } = useHistory(page)

  return (
    <div>
      <h2 className="mb-6 text-2xl font-bold">History</h2>

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
                </tr>
              </thead>
              <tbody>
                {data.records.map((event) => (
                  <tr
                    key={event.id}
                    className="border-b border-slate-700/50 hover:bg-slate-700/30 transition-colors"
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
                    <td className="px-4 py-3 font-medium text-white max-w-xs truncate">
                      {event.sourceTitle}
                    </td>
                    <td className="px-4 py-3">
                      {event.quality && (
                        <span className="rounded bg-blue-500/20 px-2 py-0.5 text-xs font-medium text-blue-400">
                          {qualityName(event.quality)}
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-3 text-slate-400">{event.indexer || '-'}</td>
                  </tr>
                ))}
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
