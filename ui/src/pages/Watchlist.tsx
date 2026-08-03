// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { Loader2, RefreshCw, Bookmark, Film, Tv } from 'lucide-react'
import { useWatchlist, useSyncWatchlist, useSystemStatus } from '../hooks/useApi'
import type { WatchlistItem } from '../api/types'
import { formatDate } from '../utils/date'

export default function Watchlist() {
  const { data: status } = useSystemStatus()
  const { data: items, isLoading, error } = useWatchlist()
  const syncMutation = useSyncWatchlist()

  const plexEnabled = status?.modules?.plexIntegration

  if (!plexEnabled) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-slate-400">
        <Bookmark size={48} className="mb-4 text-slate-600" />
        <p className="mb-2">Plex integration is not enabled</p>
        <p className="text-sm text-slate-500">
          Enable Plex in Settings to use the watchlist feature.
        </p>
      </div>
    )
  }

  return (
    <div>
      {/* Header */}
      <div className="mb-6 flex flex-wrap items-center justify-between gap-4">
        <h2 className="text-2xl font-bold">Plex Watchlist</h2>
        <button
          onClick={() => syncMutation.mutate()}
          disabled={syncMutation.isPending}
          className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
        >
          <RefreshCw size={16} className={syncMutation.isPending ? 'animate-spin' : ''} />
          {syncMutation.isPending ? 'Syncing...' : 'Sync Now'}
        </button>
      </div>

      {/* Loading */}
      {isLoading && (
        <div className="flex items-center justify-center py-20">
          <Loader2 size={32} className="animate-spin text-blue-500" />
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
          Failed to load watchlist: {error.message}
        </div>
      )}

      {/* Empty state */}
      {!isLoading && !error && items?.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <Bookmark size={48} className="mb-4 text-slate-600" />
          <p className="mb-2">Your Plex watchlist is empty</p>
          <p className="text-sm text-slate-500">
            Add items to your Plex watchlist and sync to see them here.
          </p>
        </div>
      )}

      {/* Watchlist grid */}
      {items && items.length > 0 && (
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {items.map((item) => (
            <WatchlistCard key={item.id} item={item} />
          ))}
        </div>
      )}
    </div>
  )
}

function WatchlistCard({ item }: { item: WatchlistItem }) {
  const isMovie = item.media_type === 'movie'

  return (
    <div className="flex items-center gap-3 rounded-lg bg-slate-800 p-3">
      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-slate-700">
        {isMovie ? (
          <Film size={18} className="text-blue-400" />
        ) : (
          <Tv size={18} className="text-purple-400" />
        )}
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-sm font-medium text-white">
          TMDB #{item.tmdb_id}
        </div>
        <div className="flex items-center gap-2 text-xs text-slate-400">
          <span className="rounded bg-slate-700 px-1.5 py-0.5 text-[10px] font-medium uppercase">
            {item.media_type}
          </span>
          {item.auto_requested && (
            <span className="rounded bg-green-500/20 px-1.5 py-0.5 text-[10px] font-medium text-green-400">
              Requested
            </span>
          )}
          <span>{formatDate(item.created_at)}</span>
        </div>
      </div>
    </div>
  )
}
