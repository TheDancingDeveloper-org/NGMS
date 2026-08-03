// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState } from 'react'
import { usePlexEvents, useClearPlexEvents, useCurrentUser } from '../hooks/useApi'
import { Play, Pause, Square, BookPlus, Trash2, Loader2, Activity } from 'lucide-react'

type EventFilter = 'all' | 'plays' | 'library' | 'system'

const EVENT_ICONS: Record<string, typeof Play> = {
  'media.play': Play,
  'media.resume': Play,
  'media.pause': Pause,
  'media.stop': Square,
  'media.scrobble': Activity,
  'library.new': BookPlus,
}

function eventFilterTypes(filter: EventFilter): string | undefined {
  switch (filter) {
    case 'plays': return undefined // we filter client-side for multiple types
    case 'library': return 'library.new'
    case 'system': return undefined
    default: return undefined
  }
}

function matchesFilter(eventType: string, filter: EventFilter): boolean {
  switch (filter) {
    case 'all': return true
    case 'plays': return eventType.startsWith('media.')
    case 'library': return eventType.startsWith('library.')
    case 'system': return !eventType.startsWith('media.') && !eventType.startsWith('library.')
    default: return true
  }
}

function formatEventType(type: string): string {
  const map: Record<string, string> = {
    'media.play': 'Started Playing',
    'media.pause': 'Paused',
    'media.resume': 'Resumed',
    'media.stop': 'Stopped',
    'media.scrobble': 'Watched',
    'library.new': 'Added to Library',
    'library.on.deck': 'On Deck',
    'admin.database.backup': 'Database Backup',
    'admin.database.corrupted': 'Database Corrupted',
  }
  return map[type] ?? type
}

function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'Just now'
  if (mins < 60) return `${mins}m ago`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  return `${days}d ago`
}

export default function PlexActivity() {
  const [filter, setFilter] = useState<EventFilter>('all')
  const { data: events, isLoading } = usePlexEvents(eventFilterTypes(filter))
  const clearEvents = useClearPlexEvents()
  const { data: currentUser } = useCurrentUser()
  const isAdmin = currentUser?.role === 'admin'

  const filtered = (events ?? []).filter((e) => matchesFilter(e.eventType, filter))

  return (
    <div>
      {/* Header */}
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <Activity className="h-6 w-6 text-orange-400" />
          <h1 className="text-2xl font-bold">Plex Activity</h1>
          <span className="rounded-full bg-slate-700 px-2.5 py-0.5 text-xs font-medium text-slate-300">
            {filtered.length}
          </span>
        </div>
        {isAdmin && filtered.length > 0 && (
          <button
            onClick={() => { if (confirm('Clear all Plex events?')) clearEvents.mutate() }}
            disabled={clearEvents.isPending}
            className="flex items-center gap-1.5 rounded-lg bg-red-600/20 px-3 py-2 text-sm font-medium text-red-400 hover:bg-red-600/30 transition-colors"
          >
            <Trash2 size={14} /> Clear
          </button>
        )}
      </div>

      {/* Filter tabs */}
      <div className="mb-6 flex gap-2">
        {(['all', 'plays', 'library', 'system'] as const).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`rounded-full px-4 py-2 text-sm font-medium transition-colors ${
              filter === f
                ? 'bg-orange-600 text-white'
                : 'bg-slate-800 text-slate-300 hover:bg-slate-700'
            }`}
          >
            {f === 'all' ? 'All' : f === 'plays' ? 'Playback' : f === 'library' ? 'Library' : 'System'}
          </button>
        ))}
      </div>

      {/* Loading */}
      {isLoading && (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="h-8 w-8 animate-spin text-blue-500" />
        </div>
      )}

      {/* Empty */}
      {!isLoading && filtered.length === 0 && (
        <div className="rounded-lg border border-slate-700 p-8 text-center text-slate-400">
          <Activity size={32} className="mx-auto mb-3 opacity-50" />
          <p>No Plex events recorded</p>
          <p className="mt-1 text-sm">
            Configure your Plex server's webhook URL in Settings to start receiving events.
          </p>
        </div>
      )}

      {/* Event list */}
      {filtered.length > 0 && (
        <div className="space-y-1">
          {filtered.map((event) => {
            const Icon = EVENT_ICONS[event.eventType] ?? Activity
            const isPlay = event.eventType.startsWith('media.')
            return (
              <div
                key={event.id}
                className="flex items-center gap-3 rounded-lg bg-slate-800 px-4 py-3 transition-colors hover:bg-slate-750"
              >
                <div className={`shrink-0 rounded-lg p-2 ${
                  isPlay ? 'bg-green-500/10 text-green-400' : 'bg-blue-500/10 text-blue-400'
                }`}>
                  <Icon size={16} />
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium text-white truncate">
                      {event.title ?? 'Unknown'}
                    </span>
                    <span className="shrink-0 rounded bg-slate-700 px-1.5 py-0.5 text-[10px] font-medium text-slate-400">
                      {formatEventType(event.eventType)}
                    </span>
                  </div>
                  <div className="mt-0.5 text-xs text-slate-500">
                    {event.userName && <span>{event.userName} &middot; </span>}
                    {timeAgo(event.receivedAt)}
                  </div>
                </div>
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}
