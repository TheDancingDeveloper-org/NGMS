import { Download, Loader2, RefreshCw } from 'lucide-react'
import { Link } from 'react-router-dom'
import { useQueue } from '../hooks/useApi'
import { useQueryClient } from '@tanstack/react-query'
import { formatTime } from '../utils/date'

export default function Queue() {
  const { data: queue, isLoading, error, dataUpdatedAt } = useQueue()
  const qc = useQueryClient()

  return (
    <div>
      <div className="mb-6 flex items-center justify-between">
        <h2 className="text-2xl font-bold">Queue</h2>
        <div className="flex items-center gap-3">
          <span className="text-xs text-slate-500">
            Auto-refreshes every 5s
            {dataUpdatedAt > 0 && ` \u00B7 Updated ${formatTime(dataUpdatedAt)}`}
          </span>
          <button
            onClick={() => void qc.invalidateQueries({ queryKey: ['queue'] })}
            className="rounded-lg bg-slate-700 p-2 text-slate-400 hover:text-white transition-colors"
            title="Refresh now"
          >
            <RefreshCw size={16} />
          </button>
        </div>
      </div>

      {isLoading && (
        <div className="flex items-center justify-center py-20">
          <Loader2 size={32} className="animate-spin text-blue-500" />
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
          Failed to load queue: {error.message}
        </div>
      )}

      {!isLoading && !error && queue?.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <Download size={48} className="mb-4 text-slate-600" />
          <p>Queue is empty</p>
        </div>
      )}

      {queue && queue.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                <th className="px-4 py-3 font-medium">Title</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium w-48">Progress</th>
                <th className="px-4 py-3 font-medium">Size</th>
                <th className="px-4 py-3 font-medium">ETA</th>
                <th className="px-4 py-3 font-medium">Client</th>
              </tr>
            </thead>
            <tbody>
              {queue.map((item) => (
                <tr key={item.id} className="border-b border-slate-700/50 hover:bg-slate-700/30 transition-colors">
                  <td className="px-4 py-3 font-medium text-white">
                    {item.mediaType === 'series' && item.seriesId ? (
                      <Link to={`/series/${item.seriesId}`} className="hover:text-blue-400 transition-colors">
                        {item.title}
                      </Link>
                    ) : item.mediaType === 'movie' && item.movieId ? (
                      <Link to={`/movies/${item.movieId}`} className="hover:text-blue-400 transition-colors">
                        {item.title}
                      </Link>
                    ) : (
                      item.title
                    )}
                  </td>
                  <td className="px-4 py-3">
                    <StatusBadge status={item.status} />
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex items-center gap-2">
                      <div className="h-2 flex-1 overflow-hidden rounded-full bg-slate-600">
                        <div
                          className="h-full rounded-full bg-blue-500 transition-all"
                          style={{ width: `${item.progress}%` }}
                        />
                      </div>
                      <span className="w-10 text-right text-xs text-slate-400">
                        {Math.round(item.progress)}%
                      </span>
                    </div>
                  </td>
                  <td className="px-4 py-3 text-slate-300">
                    {formatSize(item.size)}
                  </td>
                  <td className="px-4 py-3 text-slate-300">
                    {item.estimatedCompletionTime
                      ? formatEta(item.estimatedCompletionTime)
                      : '-'}
                  </td>
                  <td className="px-4 py-3 text-slate-400">
                    <div className="flex items-center gap-2">
                      {item.protocol && (
                        <span className={`rounded px-1.5 py-0.5 text-xs font-medium ${
                          item.protocol === 'usenet'
                            ? 'bg-emerald-500/20 text-emerald-400'
                            : 'bg-orange-500/20 text-orange-400'
                        }`}>
                          {item.protocol === 'usenet' ? 'NZB' : 'Torrent'}
                        </span>
                      )}
                      {item.downloadClient}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    downloading: 'bg-blue-500/20 text-blue-400',
    paused: 'bg-yellow-500/20 text-yellow-400',
    queued: 'bg-slate-600 text-slate-300',
    completed: 'bg-green-500/20 text-green-400',
    importing: 'bg-purple-500/20 text-purple-400',
    postProcessing: 'bg-cyan-500/20 text-cyan-400',
    post_processing: 'bg-cyan-500/20 text-cyan-400',
    failed: 'bg-red-500/20 text-red-400',
    warning: 'bg-orange-500/20 text-orange-400',
  }
  const labels: Record<string, string> = {
    postProcessing: 'Processing',
    post_processing: 'Processing',
    importing: 'Importing',
  }
  return (
    <span className={`rounded-full px-2 py-0.5 text-xs font-medium capitalize ${colors[status] ?? 'bg-slate-600 text-slate-300'}`}>
      {labels[status] ?? status}
    </span>
  )
}

function formatSize(bytes: number): string {
  if (!bytes || !isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function formatEta(isoDate: string): string {
  const diff = new Date(isoDate).getTime() - Date.now()
  if (diff <= 0) return 'Done'
  const mins = Math.floor(diff / 60000)
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ${mins % 60}m`
  return `${Math.floor(hours / 24)}d ${hours % 24}h`
}
