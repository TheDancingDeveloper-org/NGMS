import { useParams, useNavigate } from 'react-router-dom'
import {
  ArrowLeft,
  Eye,
  EyeOff,
  Search,
  ChevronDown,
  ChevronRight,
  CheckCircle,
  XCircle,
  Loader2,
  Trash2,
  Tv,
  Play,
} from 'lucide-react'
import { useState, useRef, useEffect } from 'react'
import {
  useSeriesDetail,
  useEpisodes,
  useDeleteSeries,
  useToggleSeriesMonitor,
  useToggleEpisodeMonitor,
  useSetSeasonMonitor,
  useApplyMonitorStrategy,
  useSeriesMissingSearch,
  useSeriesCutoffSearch,
  useTvRecommendations,
  useTvSimilar,
  useCurrentUser,
  useQualityProfiles,
  useUpdateSeries,
} from '../hooks/useApi'
import type { MonitorStrategy } from '../hooks/useApi'
import type { Episode, TmdbSeries } from '../api/types'
import { qualityName, tmdbYear } from '../api/types'
import MediaCard from '../components/MediaCard'
import MediaSlider from '../components/MediaSlider'
import InteractiveSearchModal from '../components/InteractiveSearchModal'
import AddToLibraryModal from '../components/AddToLibraryModal'
import type { AddTarget } from '../components/AddToLibraryModal'
import { formatAirDate } from '../utils/date'
import MediaFileDetailModal from '../components/MediaFileDetailModal'

export default function SeriesDetail() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const seriesId = Number(id) || 0
  const { data: series, isLoading, error } = useSeriesDetail(seriesId)
  const { data: episodes } = useEpisodes(seriesId)
  const deleteMutation = useDeleteSeries()
  const toggleMonitor = useToggleSeriesMonitor()
  const { data: currentUser } = useCurrentUser()
  const isAdmin = currentUser?.role === 'admin'
  const { data: qualityProfiles } = useQualityProfiles()
  const updateSeries = useUpdateSeries()
  const tmdbId = series?.tmdbId ?? 0
  const recommendations = useTvRecommendations(tmdbId)
  const similar = useTvSimilar(tmdbId)
  const [addTarget, setAddTarget] = useState<AddTarget | null>(null)

  const handleRecClick = (item: TmdbSeries) => {
    setAddTarget({
      id: item.id,
      title: item.name,
      year: tmdbYear(item),
      mediaType: 'tv',
      posterPath: item.poster_path ?? null,
    })
  }

  const handleDelete = () => {
    if (confirm('Are you sure you want to delete this series?')) {
      deleteMutation.mutate(seriesId, {
        onSuccess: () => navigate('/series'),
      })
    }
  }

  const applyStrategy = useApplyMonitorStrategy()
  const missingSearch = useSeriesMissingSearch()
  const cutoffSearch = useSeriesCutoffSearch()
  const [strategyOpen, setStrategyOpen] = useState(false)
  const [searchOpen, setSearchOpen] = useState(false)
  const [searchToast, setSearchToast] = useState<string | null>(null)
  const strategyRef = useRef<HTMLDivElement>(null)
  const searchRef = useRef<HTMLDivElement>(null)

  // Close dropdowns when clicking outside
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (strategyRef.current && !strategyRef.current.contains(e.target as Node)) {
        setStrategyOpen(false)
      }
      if (searchRef.current && !searchRef.current.contains(e.target as Node)) {
        setSearchOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  const showSearchToast = (msg: string) => {
    setSearchToast(msg)
    setTimeout(() => setSearchToast(null), 4000)
  }

  const handleSearchMissing = () => {
    setSearchOpen(false)
    missingSearch.mutate(seriesId, {
      onSuccess: () => showSearchToast('Missing episode search started'),
      onError: (e) => {
        if (e instanceof Error && e.message.includes('409')) {
          showSearchToast('A search is already running')
        } else {
          showSearchToast('Failed to start search')
        }
      },
    })
  }

  const handleSearchCutoff = () => {
    setSearchOpen(false)
    cutoffSearch.mutate(seriesId, {
      onSuccess: () => showSearchToast('Upgrade search started'),
      onError: (e) => {
        if (e instanceof Error && e.message.includes('409')) {
          showSearchToast('A search is already running')
        } else {
          showSearchToast('Failed to start search')
        }
      },
    })
  }

  const handleToggleMonitor = () => {
    if (series) {
      toggleMonitor.mutate({ id: series.id, monitored: !series.monitored })
    }
  }

  const handleStrategy = (strategy: MonitorStrategy) => {
    if (series) {
      applyStrategy.mutate({ seriesId: series.id, monitorStrategy: strategy })
      setStrategyOpen(false)
    }
  }

  // Group episodes by season
  const seasons = episodes
    ? [...new Set(episodes.map((e) => e.seasonNumber))].sort((a, b) => a - b)
    : []

  const episodesBySeason = (season: number) =>
    episodes?.filter((e) => e.seasonNumber === season).sort((a, b) => a.episodeNumber - b.episodeNumber) ?? []

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 size={32} className="animate-spin text-blue-500" />
      </div>
    )
  }

  if (error || !series) {
    return (
      <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
        {error ? `Failed to load series: ${error.message}` : 'Series not found'}
      </div>
    )
  }

  return (
    <div>
      {searchToast && (
        <div className="fixed top-4 right-4 z-50 flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-3 text-sm font-medium text-white shadow-lg animate-in fade-in">
          <Search className="h-4 w-4 shrink-0" />
          {searchToast}
        </div>
      )}
      {/* Back button */}
      <button
        onClick={() => navigate('/series')}
        className="mb-4 flex items-center gap-1 text-sm text-slate-400 hover:text-white transition-colors"
      >
        <ArrowLeft size={16} /> Back to Series
      </button>

      {/* Header */}
      <div className="mb-6 flex flex-col gap-6 md:flex-row">
        {/* Poster */}
        {series.posterUrl ? (
          <img
            src={series.posterUrl}
            alt={series.title}
            className="h-64 w-44 shrink-0 rounded-lg object-cover"
          />
        ) : (
          <div className="flex h-64 w-44 shrink-0 items-center justify-center rounded-lg bg-slate-800">
            <Tv size={48} className="text-slate-600" />
          </div>
        )}

        <div className="flex-1">
          <div className="flex flex-wrap items-start gap-3">
            <h2 className="text-3xl font-bold">{series.title}</h2>
            <span className="mt-1 rounded-full bg-slate-700 px-2.5 py-0.5 text-xs font-medium text-slate-300">
              {series.year}
            </span>
            <span className={`mt-1 rounded-full px-2.5 py-0.5 text-xs font-medium ${
              series.status === 'continuing' ? 'bg-green-500/20 text-green-400' : 'bg-slate-700 text-slate-300'
            }`}>
              {series.status}
            </span>
          </div>

          {series.network && (
            <div className="mt-1 text-sm text-slate-400">{series.network}</div>
          )}

          {series.overview && (
            <p className="mt-3 text-sm text-slate-300 leading-relaxed line-clamp-4">
              {series.overview}
            </p>
          )}

          <div className="mt-4 flex flex-wrap gap-2">
            <button
              onClick={handleToggleMonitor}
              className={`flex items-center gap-1.5 rounded-lg px-3 py-2 text-sm font-medium transition-colors ${
                series.monitored
                  ? 'bg-green-600 text-white hover:bg-green-700'
                  : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
              }`}
            >
              {series.monitored ? <Eye size={16} /> : <EyeOff size={16} />}
              {series.monitored ? 'Monitored' : 'Unmonitored'}
            </button>

            {/* Monitor strategy dropdown */}
            <div className="relative" ref={strategyRef}>
              <button
                onClick={() => setStrategyOpen((o) => !o)}
                className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
              >
                Monitor
                <ChevronDown size={14} />
              </button>
              {strategyOpen && (
                <div className="absolute left-0 top-full z-20 mt-1 w-52 rounded-lg border border-slate-600 bg-slate-800 py-1 shadow-xl">
                  {([
                    ['all', 'Monitor All'],
                    ['latestSeason', 'Latest Season Only'],
                    ['firstSeason', 'First Season Only'],
                    ['upcoming', 'Upcoming Only'],
                    ['none', 'Unmonitor All'],
                  ] as [MonitorStrategy, string][]).map(([key, label]) => (
                    <button
                      key={key}
                      onClick={() => handleStrategy(key)}
                      className="flex w-full items-center px-3 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 hover:text-white transition-colors"
                    >
                      {label}
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Search missing/upgrades dropdown */}
            <div className="relative" ref={searchRef}>
              <button
                onClick={() => setSearchOpen((o) => !o)}
                disabled={missingSearch.isPending || cutoffSearch.isPending}
                className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors disabled:opacity-50"
              >
                {missingSearch.isPending || cutoffSearch.isPending ? (
                  <Loader2 size={16} className="animate-spin" />
                ) : (
                  <Search size={16} />
                )}
                Search
                <ChevronDown size={14} />
              </button>
              {searchOpen && (
                <div className="absolute left-0 top-full z-20 mt-1 w-52 rounded-lg border border-slate-600 bg-slate-800 py-1 shadow-xl">
                  <button
                    onClick={handleSearchMissing}
                    className="flex w-full items-center px-3 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 hover:text-white transition-colors"
                  >
                    Search Missing
                  </button>
                  <button
                    onClick={handleSearchCutoff}
                    className="flex w-full items-center px-3 py-2 text-left text-sm text-slate-300 hover:bg-slate-700 hover:text-white transition-colors"
                  >
                    Search Upgrades
                  </button>
                </div>
              )}
            </div>

            {isAdmin && (
              <button
                onClick={handleDelete}
                disabled={deleteMutation.isPending}
                className="flex items-center gap-1.5 rounded-lg bg-red-600/20 px-3 py-2 text-sm font-medium text-red-400 hover:bg-red-600/30 transition-colors"
              >
                <Trash2 size={16} /> Delete
              </button>
            )}
          </div>

          {/* Stats */}
          <div className="mt-4 flex flex-wrap items-center gap-6 text-sm">
            <div>
              <span className="text-slate-400">Seasons:</span>{' '}
              <span className="font-medium">{series.seasonCount}</span>
            </div>
            <div>
              <span className="text-slate-400">Episodes:</span>{' '}
              <span className="font-medium">
                {series.episodeFileCount}/{series.episodeCount}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-slate-400">Quality Profile:</span>
              <select
                value={series.qualityProfileId}
                onChange={(e) => updateSeries.mutate({ id: series.id, qualityProfileId: Number(e.target.value) })}
                disabled={updateSeries.isPending}
                className="rounded-lg border border-slate-600 bg-slate-700 px-2 py-1 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              >
                {qualityProfiles?.map((p) => (
                  <option key={p.id} value={p.id}>{p.name}</option>
                ))}
              </select>
            </div>
          </div>
        </div>
      </div>

      {/* Season list */}
      <div className="space-y-2">
        {seasons.map((season) => (
          <SeasonAccordion
            key={season}
            seriesId={seriesId}
            seriesTitle={series.title}
            qualityProfileId={series.qualityProfileId}
            season={season}
            episodes={episodesBySeason(season)}
          />
        ))}
        {seasons.length === 0 && (
          <div className="rounded-lg bg-slate-800 p-8 text-center text-slate-400">
            No episode data available
          </div>
        )}
      </div>

      {/* Recommendations */}
      {recommendations.data && recommendations.data.results.length > 0 && (
        <div className="mt-8">
          <MediaSlider title="Recommended" isLoading={recommendations.isLoading}>
            {recommendations.data.results.map((item) => (
              <MediaCard key={`rec-${item.id}`} item={item} onClick={() => handleRecClick(item)} />
            ))}
          </MediaSlider>
        </div>
      )}

      {/* Similar */}
      {similar.data && similar.data.results.length > 0 && (
        <div className="mt-6">
          <MediaSlider title="Similar Series" isLoading={similar.isLoading}>
            {similar.data.results.map((item) => (
              <MediaCard key={`sim-${item.id}`} item={item} onClick={() => handleRecClick(item)} />
            ))}
          </MediaSlider>
        </div>
      )}

      {addTarget && (
        <AddToLibraryModal target={addTarget} onClose={() => setAddTarget(null)} />
      )}
    </div>
  )
}

function SeasonAccordion({ seriesId, seriesTitle, qualityProfileId, season, episodes }: { seriesId: number; seriesTitle: string; qualityProfileId: number; season: number; episodes: Episode[] }) {
  const [expanded, setExpanded] = useState(false)
  const setSeasonMonitor = useSetSeasonMonitor()
  const filesCount = episodes.filter((e) => e.hasFile).length
  const monitoredCount = episodes.filter((e) => e.monitored).length
  const allMonitored = monitoredCount === episodes.length && episodes.length > 0
  const noneMonitored = monitoredCount === 0

  const handleSeasonMonitor = (e: React.MouseEvent) => {
    e.stopPropagation()
    // Toggle: if all are monitored, unmonitor; otherwise monitor all
    setSeasonMonitor.mutate({
      seriesId,
      seasonNumber: season,
      monitored: !allMonitored,
    })
  }

  return (
    <div className="overflow-hidden rounded-lg bg-slate-800">
      <button
        onClick={() => setExpanded((e) => !e)}
        className="flex w-full items-center gap-3 px-4 py-3 text-left hover:bg-slate-750 transition-colors"
      >
        {expanded ? (
          <ChevronDown size={16} className="text-slate-400" />
        ) : (
          <ChevronRight size={16} className="text-slate-400" />
        )}
        <span className="font-medium">
          {season === 0 ? 'Specials' : `Season ${season}`}
        </span>

        {/* Season monitor toggle */}
        <span
          onClick={handleSeasonMonitor}
          title={allMonitored ? 'Unmonitor season' : 'Monitor season'}
          className={`shrink-0 transition-colors ${
            noneMonitored
              ? 'text-slate-500 hover:text-white'
              : allMonitored
                ? 'text-green-500 hover:text-green-400'
                : 'text-yellow-500 hover:text-yellow-400'
          }`}
        >
          {noneMonitored ? <EyeOff size={15} /> : <Eye size={15} />}
        </span>

        <span className="ml-auto text-sm text-slate-400">
          {filesCount}/{episodes.length} episodes
        </span>
        {/* Mini progress */}
        <div className="h-1.5 w-20 overflow-hidden rounded-full bg-slate-600">
          <div
            className="h-full rounded-full bg-blue-500"
            style={{
              width: `${episodes.length > 0 ? (filesCount / episodes.length) * 100 : 0}%`,
            }}
          />
        </div>
      </button>

      {expanded && (
        <div className="border-t border-slate-700">
          {episodes.map((ep) => (
            <EpisodeRow key={ep.id} episode={ep} seriesId={seriesId} seriesTitle={seriesTitle} qualityProfileId={qualityProfileId} />
          ))}
        </div>
      )}
    </div>
  )
}

function EpisodeRow({ episode, seriesId, seriesTitle, qualityProfileId }: { episode: Episode; seriesId: number; seriesTitle: string; qualityProfileId: number }) {
  const navigate = useNavigate()
  const toggleMonitor = useToggleEpisodeMonitor()
  const [showSearch, setShowSearch] = useState(false)
  const [showFileDetail, setShowFileDetail] = useState(false)

  const searchTerm = `${seriesTitle} S${String(episode.seasonNumber).padStart(2, '0')}E${String(episode.episodeNumber).padStart(2, '0')}`

  return (
    <>
      <div className="flex items-center gap-3 border-b border-slate-700/50 px-4 py-2.5 last:border-b-0 hover:bg-slate-700/30 transition-colors">
        {/* Episode number */}
        <span className="w-10 shrink-0 text-sm font-mono text-slate-500">
          {String(episode.episodeNumber).padStart(2, '0')}
        </span>

        {/* File status */}
        {episode.hasFile ? (
          <CheckCircle size={16} className="shrink-0 text-green-500" />
        ) : (
          <XCircle size={16} className="shrink-0 text-red-500" />
        )}

        {/* Title */}
        <div className="flex-1 min-w-0">
          <span className="text-sm text-white truncate block">{episode.title || 'TBA'}</span>
        </div>

        {/* Air date */}
        <span className="shrink-0 text-xs text-slate-400">
          {episode.airDate ? formatAirDate(episode.airDate) : '-'}
        </span>

        {/* Quality badge — clickable for file detail */}
        {episode.episodeFile && (
          <button
            onClick={() => setShowFileDetail(true)}
            title="View file details"
            className="shrink-0 rounded bg-blue-500/20 px-2 py-0.5 text-xs font-medium text-blue-400 hover:bg-blue-500/30 hover:text-blue-300 transition-colors cursor-pointer"
          >
            {qualityName(episode.episodeFile.quality)}
          </button>
        )}

        {/* Monitor toggle */}
        <button
          onClick={() => toggleMonitor.mutate({ id: episode.id, monitored: !episode.monitored })}
          title={episode.monitored ? 'Unmonitor' : 'Monitor'}
          className="shrink-0 text-slate-400 hover:text-white transition-colors"
        >
          {episode.monitored ? <Eye size={14} /> : <EyeOff size={14} />}
        </button>

        {/* Play */}
        {episode.hasFile && episode.episodeFile && (
          <button
            onClick={() => navigate(`/play/${episode.episodeFile!.id}`)}
            title="Play episode"
            className="shrink-0 text-slate-400 hover:text-green-400 transition-colors"
          >
            <Play size={14} />
          </button>
        )}

        {/* Search */}
        <button
          onClick={() => setShowSearch(true)}
          title="Search for episode"
          className="shrink-0 text-slate-400 hover:text-blue-400 transition-colors"
        >
          <Search size={14} />
        </button>
      </div>

      {showSearch && (
        <InteractiveSearchModal
          title={searchTerm}
          term={searchTerm}
          mediaType="series"
          qualityProfileId={qualityProfileId}
          seriesId={seriesId}
          episodeId={episode.id}
          onClose={() => setShowSearch(false)}
        />
      )}

      {showFileDetail && episode.episodeFile && (
        <MediaFileDetailModal
          file={episode.episodeFile}
          onClose={() => setShowFileDetail(false)}
        />
      )}
    </>
  )
}
