import { HardDrive, Check, X, Loader2, Search } from 'lucide-react'
import type { SystemActivity } from '../api/types'

function relativeTime(dateStr: string): string {
  const diff = (Date.now() - new Date(dateStr).getTime()) / 1000
  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`
  return new Date(dateStr).toLocaleDateString()
}

function activityIcon(type: string) {
  switch (type) {
    case 'disk_scan':
      return <HardDrive size={16} />
    case 'episode_search':
    case 'movie_search':
    case 'missing_search':
    case 'cutoff_search':
      return <Search size={16} />
    default:
      return <HardDrive size={16} />
  }
}

function iconColor(status: string) {
  switch (status) {
    case 'running':
      return 'bg-blue-900/60 text-blue-400'
    case 'completed':
      return 'bg-green-900/60 text-green-400'
    case 'failed':
      return 'bg-red-900/60 text-red-400'
    default:
      return 'bg-slate-700 text-slate-400'
  }
}

export default function ActivityTab({ activities }: { activities: SystemActivity[] }) {
  if (activities.length === 0) {
    return (
      <div className="flex items-center justify-center py-12 text-sm text-slate-500">
        No recent activity
      </div>
    )
  }

  return (
    <div>
      {activities.map((a) => {
        const progress = a.progress as Record<string, number> | null
        const foldersTotal = progress?.folders_total ?? 0
        const foldersDone = progress?.folders_done ?? 0
        const filesFound = progress?.files_found ?? 0
        const filesMatched = progress?.files_matched ?? 0
        // For search activities, use searched/total for progress
        const searchTotal = progress?.total ?? 0
        const searchDone = progress?.searched ?? 0
        const grabbed = progress?.grabbed ?? 0

        const isSearch = a.activityType.includes('search')
        const hasProgress = isSearch ? searchTotal > 0 : foldersTotal > 0
        const pct = isSearch
          ? (searchTotal > 0 ? Math.round((searchDone / searchTotal) * 100) : 0)
          : (foldersTotal > 0 ? Math.round((foldersDone / foldersTotal) * 100) : 0)

        return (
          <div
            key={a.id}
            className="flex gap-3 border-b border-slate-700 px-4 py-3 transition-colors hover:bg-slate-700/30"
          >
            {/* Icon */}
            <div className={`relative flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-full ${iconColor(a.status)}`}>
              {activityIcon(a.activityType)}
              {a.status === 'running' && (
                <div className="absolute -inset-[3px] animate-spin rounded-full border-2 border-transparent border-t-blue-500" />
              )}
            </div>

            {/* Content */}
            <div className="min-w-0 flex-1">
              <div className="text-[13px] font-medium text-slate-200">{a.title}</div>
              {a.detail && (
                <div className="truncate text-xs text-slate-500">{a.detail}</div>
              )}

              {/* Running stats */}
              {a.status === 'running' && !isSearch && filesFound > 0 && (
                <div className="mt-0.5 text-[11px] text-slate-500">
                  {filesFound.toLocaleString()} files found, {filesMatched.toLocaleString()} matched
                </div>
              )}
              {a.status === 'running' && isSearch && searchDone > 0 && (
                <div className="mt-0.5 text-[11px] text-slate-500">
                  {searchDone}/{searchTotal} searched, {grabbed} grabbed
                </div>
              )}

              {/* Progress bar */}
              {a.status === 'running' && hasProgress && (
                <div className="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-slate-700">
                  <div
                    className="h-full rounded-full bg-blue-500 transition-all duration-300"
                    style={{ width: `${pct}%` }}
                  />
                </div>
              )}
              {a.status === 'completed' && (
                <div className="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-slate-700">
                  <div className="h-full w-full rounded-full bg-green-400" />
                </div>
              )}
              {a.status === 'failed' && (
                <div className="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-slate-700">
                  <div
                    className="h-full rounded-full bg-red-400"
                    style={{ width: `${pct || 100}%` }}
                  />
                </div>
              )}

              {/* Meta row */}
              <div className="mt-1 flex items-center justify-between">
                <span className="flex items-center gap-1 text-[11px] font-medium">
                  {a.status === 'running' && (
                    <span className="flex items-center gap-1 text-blue-400">
                      <Loader2 size={10} className="animate-spin" />
                      {isSearch
                        ? (searchTotal > 0 ? `${searchDone} / ${searchTotal} items` : 'Running')
                        : (foldersTotal > 0 ? `${foldersDone} / ${foldersTotal} folders` : 'Running')
                      }
                    </span>
                  )}
                  {a.status === 'completed' && (
                    <span className="flex items-center gap-1 text-green-400">
                      <Check size={10} />
                      Completed
                    </span>
                  )}
                  {a.status === 'failed' && (
                    <span className="flex items-center gap-1 text-red-400">
                      <X size={10} />
                      Failed
                    </span>
                  )}
                </span>
                <span className="text-[11px] text-slate-600">
                  {relativeTime(a.startedAt)}
                </span>
              </div>
            </div>
          </div>
        )
      })}
    </div>
  )
}
