import { useState, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { Plus, Search, Eye, EyeOff, Tv, X, Loader2, Pencil, Check, SlidersHorizontal } from 'lucide-react'
import { useSeries, useSeriesLookup, useAddSeries, useQualityProfiles, useBulkUpdateSeries } from '../hooks/useApi'
import type { Series } from '../api/types'
import SeriesBrowse from '../components/SeriesBrowse'
import BulkEditBar from '../components/BulkEditBar'

type View = 'library' | 'browse'

interface SeriesFilters {
  status: string
  qualityProfileId: number | ''
  monitored: '' | 'true' | 'false'
}

export default function SeriesList() {
  const navigate = useNavigate()
  const { data: series, isLoading, error } = useSeries()
  const [filter, setFilter] = useState('')
  const [filters, setFilters] = useState<SeriesFilters>({ status: '', qualityProfileId: '', monitored: '' })
  const [showFilters, setShowFilters] = useState(false)
  const [showAddModal, setShowAddModal] = useState(false)
  const [view, setView] = useState<View>('library')
  const [editMode, setEditMode] = useState(false)
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())
  const { data: qualityProfiles } = useQualityProfiles()
  const bulkUpdate = useBulkUpdateSeries()

  const allStatuses = useMemo(
    () => [...new Set((series ?? []).map((s) => s.status).filter(Boolean))].sort(),
    [series],
  )

  const activeFilterCount = [
    filters.status,
    filters.qualityProfileId !== '' ? '1' : '',
    filters.monitored,
  ].filter(Boolean).length

  const clearFilters = () => setFilters({ status: '', qualityProfileId: '', monitored: '' })

  const filtered = useMemo(() => {
    return (series ?? []).filter((s) => {
      if (filter && !s.title.toLowerCase().includes(filter.toLowerCase())) return false
      if (filters.status && s.status.toLowerCase() !== filters.status.toLowerCase()) return false
      if (filters.qualityProfileId !== '' && s.qualityProfileId !== filters.qualityProfileId) return false
      if (filters.monitored !== '') {
        if (s.monitored !== (filters.monitored === 'true')) return false
      }
      return true
    })
  }, [series, filter, filters])

  const toggleSelect = (id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const handleExitEditMode = () => {
    setEditMode(false)
    setSelectedIds(new Set())
  }

  return (
    <div>
      {/* Header */}
      <div className="mb-4 flex flex-wrap items-center justify-between gap-4">
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
              onClick={() => setShowFilters((v) => !v)}
              className={`relative flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-colors ${
                showFilters || activeFilterCount > 0
                  ? 'bg-blue-600/20 text-blue-300 hover:bg-blue-600/30'
                  : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
              }`}
            >
              <SlidersHorizontal size={16} />
              Filters
              {activeFilterCount > 0 && (
                <span className="absolute -right-1.5 -top-1.5 flex h-4 w-4 items-center justify-center rounded-full bg-blue-500 text-[10px] font-bold text-white">
                  {activeFilterCount}
                </span>
              )}
            </button>
            <button
              onClick={() => editMode ? handleExitEditMode() : setEditMode(true)}
              className={`flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-colors ${
                editMode
                  ? 'bg-amber-600 text-white hover:bg-amber-700'
                  : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
              }`}
            >
              <Pencil size={16} /> {editMode ? 'Cancel' : 'Edit'}
            </button>
            {editMode && (
              <>
                <button
                  onClick={() => setSelectedIds(new Set(filtered.map((s) => s.id)))}
                  className="rounded-lg bg-slate-700 px-3 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
                >
                  Select All
                </button>
                {selectedIds.size > 0 && (
                  <button
                    onClick={() => setSelectedIds(new Set())}
                    className="rounded-lg bg-slate-700 px-3 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
                  >
                    Deselect ({selectedIds.size})
                  </button>
                )}
              </>
            )}
            {!editMode && (
              <button
                onClick={() => setShowAddModal(true)}
                className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
              >
                <Plus size={16} /> Add Series
              </button>
            )}
          </div>
        )}
      </div>

      {/* Filter bar */}
      {view === 'library' && showFilters && (
        <div className="mb-4 flex flex-wrap items-center gap-3 rounded-lg border border-slate-700 bg-slate-800/50 px-4 py-3">
          <select
            value={filters.status}
            onChange={(e) => setFilters((f) => ({ ...f, status: e.target.value }))}
            className="rounded-lg border border-slate-600 bg-slate-700 px-3 py-1.5 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          >
            <option value="">All Statuses</option>
            {allStatuses.map((s) => (
              <option key={s} value={s}>{s}</option>
            ))}
          </select>

          <select
            value={filters.qualityProfileId}
            onChange={(e) => setFilters((f) => ({ ...f, qualityProfileId: e.target.value === '' ? '' : Number(e.target.value) }))}
            className="rounded-lg border border-slate-600 bg-slate-700 px-3 py-1.5 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          >
            <option value="">All Profiles</option>
            {(qualityProfiles ?? []).map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>

          <select
            value={filters.monitored}
            onChange={(e) => setFilters((f) => ({ ...f, monitored: e.target.value as '' | 'true' | 'false' }))}
            className="rounded-lg border border-slate-600 bg-slate-700 px-3 py-1.5 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
          >
            <option value="">All Monitored</option>
            <option value="true">Monitored</option>
            <option value="false">Unmonitored</option>
          </select>

          {activeFilterCount > 0 && (
            <button
              onClick={clearFilters}
              className="flex items-center gap-1 text-xs text-slate-400 hover:text-slate-200 transition-colors"
            >
              <X size={12} /> Clear filters
            </button>
          )}

          <span className="ml-auto text-xs text-slate-500">
            {filtered.length} of {series?.length ?? 0} series
          </span>
        </div>
      )}

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
          {!isLoading && !error && filtered.length === 0 && (
            <EmptyState filter={filter} hasFilters={activeFilterCount > 0} onAdd={() => setShowAddModal(true)} onClearFilters={clearFilters} />
          )}

          {/* Grid */}
          {filtered.length > 0 && (
            <div className={`grid grid-cols-4 gap-2 sm:grid-cols-6 md:grid-cols-8 lg:grid-cols-10 xl:grid-cols-12 ${editMode && selectedIds.size > 0 ? 'pb-20' : ''}`}>
              {filtered.map((s) => (
                <SeriesCard
                  key={s.id}
                  series={s}
                  editMode={editMode}
                  selected={selectedIds.has(s.id)}
                  onClick={() => editMode ? toggleSelect(s.id) : navigate(`/series/${s.id}`)}
                />
              ))}
            </div>
          )}
        </>
      )}

      {/* Bulk edit bar */}
      {editMode && selectedIds.size > 0 && (
        <BulkEditBar
          selectedCount={selectedIds.size}
          totalCount={filtered.length}
          qualityProfiles={qualityProfiles ?? []}
          isPending={bulkUpdate.isPending}
          onSelectAll={() => setSelectedIds(new Set(filtered.map((s) => s.id)))}
          onSelectNone={() => setSelectedIds(new Set())}
          onApply={(profileId, monitored) => {
            bulkUpdate.mutate(
              {
                seriesIds: [...selectedIds],
                ...(profileId !== undefined && { qualityProfileId: profileId }),
                ...(monitored !== undefined && { monitored }),
              },
              { onSuccess: () => handleExitEditMode() },
            )
          }}
        />
      )}

      {/* Add modal */}
      {showAddModal && <AddSeriesModal onClose={() => setShowAddModal(false)} />}
    </div>
  )
}

function SeriesCard({ series, onClick, editMode, selected }: { series: Series; onClick: () => void; editMode?: boolean; selected?: boolean }) {
  return (
    <button
      onClick={onClick}
      className={`group relative overflow-hidden rounded-md bg-slate-800 text-left transition-transform hover:scale-[1.03] ${
        selected ? 'ring-2 ring-blue-500' : 'hover:ring-2 hover:ring-blue-500'
      }`}
    >
      {/* Selection checkbox */}
      {editMode && (
        <div className={`absolute left-1.5 top-1.5 z-10 flex h-5 w-5 items-center justify-center rounded border-2 ${
          selected ? 'border-blue-500 bg-blue-500' : 'border-slate-400 bg-slate-800/60'
        }`}>
          {selected && <Check size={12} className="text-white" />}
        </div>
      )}

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

function EmptyState({ filter, hasFilters, onAdd, onClearFilters }: { filter: string; hasFilters: boolean; onAdd: () => void; onClearFilters: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center py-20 text-slate-400">
      <Tv size={48} className="mb-4 text-slate-600" />
      {filter || hasFilters ? (
        <>
          <p className="mb-2">No series match the current filters</p>
          <button onClick={onClearFilters} className="text-sm text-blue-400 hover:text-blue-300">
            Clear filters
          </button>
        </>
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
  const navigate = useNavigate()
  const [searchTerm, setSearchTerm] = useState('')
  const { data: results, isLoading } = useSeriesLookup(searchTerm)
  const addMutation = useAddSeries()

  const handleAdd = (result: { title: string; tmdbId: number; year: number }) => {
    addMutation.mutate(
      { title: result.title, tmdbId: result.tmdbId, path: `/tv/${result.title}`, qualityProfileId: 1, monitored: true },
      { onSuccess: (data) => { onClose(); navigate(`/series/${data.id}`) } },
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
