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
import { useState } from 'react'
import {
  useSeriesDetail,
  useEpisodes,
  useDeleteSeries,
  useToggleSeriesMonitor,
  useToggleEpisodeMonitor,
  useSearchEpisode,
  useTvRecommendations,
  useTvSimilar,
  useCurrentUser,
} from '../hooks/useApi'
import type { Episode } from '../api/types'
import { qualityName } from '../api/types'
import MediaCard from '../components/MediaCard'
import MediaSlider from '../components/MediaSlider'
import { formatAirDate } from '../utils/date'

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
  const tmdbId = series?.tmdbId ?? 0
  const recommendations = useTvRecommendations(tmdbId)
  const similar = useTvSimilar(tmdbId)

  const handleDelete = () => {
    if (confirm('Are you sure you want to delete this series?')) {
      deleteMutation.mutate(seriesId, {
        onSuccess: () => navigate('/series'),
      })
    }
  }

  const handleToggleMonitor = () => {
    if (series) {
      toggleMonitor.mutate({ id: series.id, monitored: !series.monitored })
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
          <div className="mt-4 flex gap-6 text-sm">
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
          </div>
        </div>
      </div>

      {/* Season list */}
      <div className="space-y-2">
        {seasons.map((season) => (
          <SeasonAccordion
            key={season}
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
              <MediaCard key={`rec-${item.id}`} item={item} />
            ))}
          </MediaSlider>
        </div>
      )}

      {/* Similar */}
      {similar.data && similar.data.results.length > 0 && (
        <div className="mt-6">
          <MediaSlider title="Similar Series" isLoading={similar.isLoading}>
            {similar.data.results.map((item) => (
              <MediaCard key={`sim-${item.id}`} item={item} />
            ))}
          </MediaSlider>
        </div>
      )}
    </div>
  )
}

function SeasonAccordion({ season, episodes }: { season: number; episodes: Episode[] }) {
  const [expanded, setExpanded] = useState(false)
  const filesCount = episodes.filter((e) => e.hasFile).length

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
            <EpisodeRow key={ep.id} episode={ep} />
          ))}
        </div>
      )}
    </div>
  )
}

function EpisodeRow({ episode }: { episode: Episode }) {
  const navigate = useNavigate()
  const toggleMonitor = useToggleEpisodeMonitor()
  const searchEp = useSearchEpisode()

  return (
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

      {/* Quality badge */}
      {episode.episodeFile && (
        <span className="shrink-0 rounded bg-blue-500/20 px-2 py-0.5 text-xs font-medium text-blue-400">
          {qualityName(episode.episodeFile.quality)}
        </span>
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
        onClick={() => searchEp.mutate(episode.id)}
        disabled={searchEp.isPending}
        title="Search for episode"
        className="shrink-0 text-slate-400 hover:text-blue-400 transition-colors"
      >
        <Search size={14} />
      </button>
    </div>
  )
}
