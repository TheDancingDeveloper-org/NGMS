import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Plus, Search, Eye, EyeOff, Tv, X, Loader2 } from 'lucide-react'
import { useSeries, useSeriesLookup, useAddSeries } from '../hooks/useApi'
import type { Series } from '../api/types'
import SeriesBrowse from '../components/SeriesBrowse'

type View = 'library' | 'browse'

export default function SeriesList() {
  const navigate = useNavigate()
  const { data: series, isLoading, error } = useSeries()
  const [filter, setFilter] = useState('')
  const [showAddModal, setShowAddModal] = useState(false)
  const [view, setView] = useState<View>('library')

  const filtered = series?.filter((s) =>
    s.title.toLowerCase().includes(filter.toLowerCase()),
  )

  return (
    <div>
      {/* Header */}
      <div className="mb-6 flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-4">
          <h2 className="text-2xl font-bold">Series</h2>
          <div className="flex rounded-lg bg-slate-800 p-0.5 text-xs">
            <button
              onClick={() => setView('library')}
              className={`rounded-md px-3 py-1.5 font-medium transition-colors ${
                view === 'library' ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white'
              }`}
            >
              Library
            </button>
            <button
              onClick={() => setView('browse')}
              className={`rounded-md px-3 py-1.5 font-medium transition-colors ${
                view === 'browse' ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white'
              }`}
            >
              Browse
            </button>
          </div>
        </div>
        {view === 'library' && (
          <div className="flex items-center gap-3">
            <div className="relative">
              <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
              <input
                type="text"
                placeholder="Filter series..."
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                className="rounded-lg border border-slate-600 bg-slate-800 py-2 pl-9 pr-4 text-sm text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
            </div>
            <button
              onClick={() => setShowAddModal(true)}
              className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
            >
              <Plus size={16} /> Add Series
            </button>
          </div>
        )}
      </div>

      {view === 'browse' ? (
        <SeriesBrowse />
      ) : (
        <>
          {/* Loading / Error / Empty */}
          {isLoading && (
            <div className="flex items-center justify-center py-20">
              <Loader2 size={32} className="animate-spin text-blue-500" />
            </div>
          )}
          {error && (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
              Failed to load series: {error.message}
            </div>
          )}
          {!isLoading && !error && filtered?.length === 0 && (
            <EmptyState filter={filter} onAdd={() => setShowAddModal(true)} />
          )}

          {/* Grid */}
          {filtered && filtered.length > 0 && (
            <div className="grid grid-cols-4 gap-2 sm:grid-cols-6 md:grid-cols-8 lg:grid-cols-10 xl:grid-cols-12">
              {filtered.map((s) => (
                <SeriesCard key={s.id} series={s} onClick={() => navigate(`/series/${s.id}`)} />
              ))}
            </div>
          )}
        </>
      )}

      {/* Add modal */}
      {showAddModal && <AddSeriesModal onClose={() => setShowAddModal(false)} />}
    </div>
  )
}

function SeriesCard({ series, onClick }: { series: Series; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="group relative overflow-hidden rounded-md bg-slate-800 text-left transition-transform hover:scale-[1.03] hover:ring-2 hover:ring-blue-500"
    >
      {/* Poster */}
      {series.posterUrl ? (
        <img
          src={series.posterUrl}
          alt={series.title}
          loading="lazy"
          className="aspect-[2/3] w-full object-cover"
        />
      ) : (
        <div className="flex aspect-[2/3] w-full items-center justify-center bg-slate-700">
          <Tv size={24} className="text-slate-500" />
        </div>
      )}

      {/* Overlay */}
      <div className="absolute inset-0 bg-gradient-to-t from-black/80 via-transparent to-transparent" />

      {/* Monitored badge */}
      <div className="absolute right-1.5 top-1.5">
        {series.monitored ? (
          <Eye size={12} className="text-green-400" />
        ) : (
          <EyeOff size={12} className="text-slate-500" />
        )}
      </div>

      {/* Info */}
      <div className="absolute inset-x-0 bottom-0 p-1.5">
        <div className="text-[11px] font-semibold text-white leading-tight truncate">{series.title}</div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[10px] text-slate-300">
          {series.network && <span className="truncate">{series.network}</span>}
          <span className="shrink-0">
            {series.episodeFileCount}/{series.episodeCount}
          </span>
        </div>
        {/* Progress bar */}
        <div className="mt-1 h-0.5 overflow-hidden rounded-full bg-slate-600">
          <div
            className="h-full rounded-full bg-blue-500"
            style={{
              width: `${series.episodeCount > 0 ? (series.episodeFileCount / series.episodeCount) * 100 : 0}%`,
            }}
          />
        </div>
      </div>
    </button>
  )
}

function EmptyState({ filter, onAdd }: { filter: string; onAdd: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-slate-400">
      <Tv size={48} className="mb-4 text-slate-600" />
      {filter ? (
        <p>No series matching "{filter}"</p>
      ) : (
        <>
          <p className="mb-4">No series yet. Add your first one!</p>
          <button
            onClick={onAdd}
            className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
          >
            <Plus size={16} /> Add Series
          </button>
        </>
      )}
    </div>
  )
}

function AddSeriesModal({ onClose }: { onClose: () => void }) {
  const [searchTerm, setSearchTerm] = useState('')
  const { data: results, isLoading } = useSeriesLookup(searchTerm)
  const addMutation = useAddSeries()

  const handleAdd = (result: { title: string; tmdbId: number; year: number }) => {
    addMutation.mutate(
      { title: result.title, tmdbId: result.tmdbId, path: `/tv/${result.title}`, qualityProfileId: 1, monitored: true },
      { onSuccess: () => onClose() },
    )
  }

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-20">
      <div className="w-full max-w-xl rounded-xl bg-slate-800 shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-700 px-6 py-4">
          <h3 className="text-lg font-semibold">Add Series</h3>
          <button onClick={onClose} className="text-slate-400 hover:text-white">
            <X size={20} />
          </button>
        </div>

        {/* Search */}
        <div className="border-b border-slate-700 px-6 py-4">
          <div className="relative">
            <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
            <input
              type="text"
              autoFocus
              placeholder="Search for a series..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              className="w-full rounded-lg border border-slate-600 bg-slate-700 py-2.5 pl-9 pr-4 text-sm text-white placeholder-slate-400 focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
            />
          </div>
        </div>

        {/* Results */}
        <div className="max-h-80 overflow-y-auto px-6 py-4">
          {isLoading && (
            <div className="flex justify-center py-8">
              <Loader2 size={24} className="animate-spin text-blue-500" />
            </div>
          )}
          {results && results.length === 0 && (
            <p className="py-8 text-center text-slate-400">No results found</p>
          )}
          {results?.map((r) => (
            <div
              key={r.tmdbId}
              className="flex items-center gap-4 rounded-lg p-3 hover:bg-slate-700"
            >
              {r.posterUrl ? (
                <img src={r.posterUrl} alt={r.title} className="h-16 w-11 rounded object-cover" />
              ) : (
                <div className="flex h-16 w-11 items-center justify-center rounded bg-slate-600">
                  <Tv size={16} className="text-slate-400" />
                </div>
              )}
              <div className="flex-1 min-w-0">
                <div className="font-medium text-white truncate">{r.title}</div>
                <div className="text-xs text-slate-400">
                  {r.year > 0 && <>{r.year} &middot; </>}
                  {r.network && <>{r.network} &middot; </>}
                  {r.seasonCount > 0 ? `${r.seasonCount} seasons` : 'TV Series'}
                </div>
              </div>
              <button
                onClick={() => handleAdd(r)}
                disabled={addMutation.isPending}
                className="shrink-0 rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
              >
                {addMutation.isPending ? 'Adding...' : 'Add'}
              </button>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
