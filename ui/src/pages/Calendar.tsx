import { useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { CalendarDays, Loader2, Eye, EyeOff, CheckCircle, Circle, Tv } from 'lucide-react'
import { useCalendar } from '../hooks/useApi'
import type { CalendarEntry } from '../api/types'

export default function Calendar() {
  const navigate = useNavigate()
  const today = new Date()
  const start = new Date(today)
  start.setDate(start.getDate() - 7)
  const end = new Date(today)
  end.setDate(end.getDate() + 30)

  const startStr = start.toISOString().split('T')[0]
  const endStr = end.toISOString().split('T')[0]

  const { data: entries, isLoading, error } = useCalendar(startStr, endStr)

  // Group by date
  const grouped = useMemo(() => {
    if (!entries) return new Map<string, CalendarEntry[]>()
    const map = new Map<string, CalendarEntry[]>()
    for (const entry of entries) {
      if (!entry.airDateUtc) continue // skip entries with no air date
      const date = entry.airDateUtc.split('T')[0]
      if (!map.has(date)) map.set(date, [])
      map.get(date)!.push(entry)
    }
    return new Map([...map.entries()].sort(([a], [b]) => a.localeCompare(b)))
  }, [entries])

  const todayStr = today.toISOString().split('T')[0]

  return (
    <div>
      <h2 className="mb-6 text-2xl font-bold">Calendar</h2>

      {isLoading && (
        <div className="flex items-center justify-center py-20">
          <Loader2 size={32} className="animate-spin text-blue-500" />
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
          Failed to load calendar: {error.message}
        </div>
      )}

      {!isLoading && !error && grouped.size === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <CalendarDays size={48} className="mb-4 text-slate-600" />
          <p>No upcoming episodes</p>
        </div>
      )}

      {grouped.size > 0 && (
        <div className="space-y-4">
          {[...grouped.entries()].map(([date, items]) => {
            const dateObj = new Date(date + 'T00:00:00')
            const isToday = date === todayStr
            const isPast = date < todayStr

            return (
              <div key={date}>
                {/* Date header */}
                <div
                  className={`mb-2 flex items-center gap-2 text-sm font-medium ${
                    isToday
                      ? 'text-blue-400'
                      : isPast
                        ? 'text-slate-500'
                        : 'text-slate-300'
                  }`}
                >
                  <CalendarDays size={14} />
                  {isToday
                    ? 'Today'
                    : dateObj.toLocaleDateString(undefined, {
                        weekday: 'long',
                        month: 'short',
                        day: 'numeric',
                      })}
                </div>

                {/* Episodes */}
                <div className="space-y-1">
                  {items.map((entry) => (
                    <button
                      key={entry.episodeId}
                      onClick={() => navigate(`/series/${entry.seriesId}`)}
                      className="flex w-full items-center gap-3 rounded-lg bg-slate-800 px-4 py-2.5 text-left transition-colors hover:bg-slate-700 hover:ring-1 hover:ring-blue-500/50"
                    >
                      {/* Poster thumbnail */}
                      {entry.posterUrl ? (
                        <img
                          src={entry.posterUrl}
                          alt={entry.seriesTitle}
                          className="h-12 w-8 shrink-0 rounded object-cover"
                        />
                      ) : (
                        <div className="flex h-12 w-8 shrink-0 items-center justify-center rounded bg-slate-700">
                          <Tv size={14} className="text-slate-500" />
                        </div>
                      )}

                      {/* File status */}
                      {entry.hasFile ? (
                        <CheckCircle size={14} className="shrink-0 text-green-500" />
                      ) : (
                        <Circle size={14} className="shrink-0 text-slate-500" />
                      )}

                      {/* Info */}
                      <div className="flex-1 min-w-0">
                        <div className="text-sm font-medium text-white">{entry.seriesTitle}</div>
                        <div className="text-xs text-slate-400">
                          S{String(entry.seasonNumber).padStart(2, '0')}E
                          {String(entry.episodeNumber).padStart(2, '0')} &middot; {entry.episodeTitle ?? 'TBA'}
                        </div>
                      </div>

                      {/* Monitored */}
                      {entry.monitored ? (
                        <Eye size={14} className="shrink-0 text-green-400" />
                      ) : (
                        <EyeOff size={14} className="shrink-0 text-slate-500" />
                      )}
                    </button>
                  ))}
                </div>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}
