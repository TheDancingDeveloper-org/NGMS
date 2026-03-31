import { useState } from 'react'
import { Loader2, Compass, Plus, X, Check } from 'lucide-react'
import MediaCard from '../components/MediaCard'
import MediaSlider from '../components/MediaSlider'
import {
  useTrending,
  usePopularMovies,
  usePopularTv,
  useUpcomingMovies,
  useUpcomingTv,
  useSystemStatus,
  useAddSeries,
  useAddMovie,
} from '../hooks/useApi'
import { tmdbBackdropUrl, tmdbDisplayTitle, tmdbPosterUrl, tmdbYear } from '../api/types'
import type { TmdbTrendingItem } from '../api/types'

interface AddTarget {
  id: number
  title: string
  year: string
  mediaType: 'movie' | 'tv'
  posterPath: string | null
}

export default function Discover() {
  const { data: status } = useSystemStatus()
  const trending = useTrending('all', 'day')
  const popularMovies = usePopularMovies()
  const popularTv = usePopularTv()
  const upcomingMovies = useUpcomingMovies()
  const upcomingTv = useUpcomingTv()
  const [addTarget, setAddTarget] = useState<AddTarget | null>(null)

  const modules = status?.modules
  const allLoading =
    trending.isLoading && popularMovies.isLoading && popularTv.isLoading
  const allError =
    trending.error && popularMovies.error && popularTv.error

  if (allLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 size={32} className="animate-spin text-blue-500" />
      </div>
    )
  }

  if (allError) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-slate-400">
        <Compass size={48} className="mb-4 text-slate-600" />
        <p className="mb-2">Unable to load discover data</p>
        <p className="text-sm text-slate-500">
          Make sure a TMDB API key is configured in Settings.
        </p>
      </div>
    )
  }

  const handleClick = (item: TmdbTrendingItem) => {
    const mediaType = item.media_type === 'tv' ? 'tv' : 'movie'
    const title = item.title || item.name || 'Unknown'
    const year = tmdbYear(item)
    setAddTarget({ id: item.id, title, year, mediaType, posterPath: item.poster_path ?? null })
  }

  // Featured hero: first trending item with backdrop
  const heroItem = trending.data?.results?.find((r) => r.backdrop_path)

  return (
    <div className="space-y-6">
      {/* Hero Banner */}
      {heroItem && <HeroBanner item={heroItem} onClick={() => handleClick(heroItem)} />}

      {/* Trending */}
      <MediaSlider title="Trending Today" isLoading={trending.isLoading}>
        {trending.data?.results?.map((item) => (
          <MediaCard key={`t-${item.id}-${item.media_type}`} item={item} onClick={() => handleClick(item)} />
        ))}
      </MediaSlider>

      {/* Popular Movies */}
      {(!modules || modules.movieManagement) && (
        <MediaSlider title="Popular Movies" isLoading={popularMovies.isLoading}>
          {popularMovies.data?.results?.map((item) => (
            <MediaCard key={`pm-${item.id}`} item={item} onClick={() => handleClick({ ...item, media_type: 'movie' } as TmdbTrendingItem)} />
          ))}
        </MediaSlider>
      )}

      {/* Popular TV */}
      {(!modules || modules.tvManagement) && (
        <MediaSlider title="Popular TV Shows" isLoading={popularTv.isLoading}>
          {popularTv.data?.results?.map((item) => (
            <MediaCard key={`pt-${item.id}`} item={item} onClick={() => handleClick({ ...item, media_type: 'tv' } as TmdbTrendingItem)} />
          ))}
        </MediaSlider>
      )}

      {/* Upcoming Movies */}
      {(!modules || modules.movieManagement) && (
        <MediaSlider title="Upcoming Movies" isLoading={upcomingMovies.isLoading}>
          {upcomingMovies.data?.results?.map((item) => (
            <MediaCard key={`um-${item.id}`} item={item} onClick={() => handleClick({ ...item, media_type: 'movie' } as TmdbTrendingItem)} />
          ))}
        </MediaSlider>
      )}

      {/* Upcoming TV */}
      {(!modules || modules.tvManagement) && (
        <MediaSlider title="Upcoming TV Shows" isLoading={upcomingTv.isLoading}>
          {upcomingTv.data?.results?.map((item) => (
            <MediaCard key={`ut-${item.id}`} item={item} onClick={() => handleClick({ ...item, media_type: 'tv' } as TmdbTrendingItem)} />
          ))}
        </MediaSlider>
      )}

      {/* Add to library modal */}
      {addTarget && (
        <AddToLibraryModal target={addTarget} onClose={() => setAddTarget(null)} />
      )}
    </div>
  )
}

function HeroBanner({ item, onClick }: { item: TmdbTrendingItem; onClick: () => void }) {
  const backdropUrl = tmdbBackdropUrl(item.backdrop_path)
  const title = tmdbDisplayTitle(item)
  const overview = item.overview || ''

  return (
    <button
      onClick={onClick}
      className="relative -mx-4 -mt-4 mb-2 block w-[calc(100%+2rem)] overflow-hidden rounded-b-xl text-left sm:-mx-6 sm:-mt-6 sm:w-[calc(100%+3rem)] hover:brightness-110 transition"
    >
      {backdropUrl && (
        <img
          src={backdropUrl}
          alt={title}
          className="h-[220px] w-full object-cover object-top sm:h-[280px]"
        />
      )}
      <div className="absolute inset-0 bg-gradient-to-t from-slate-900 via-slate-900/60 to-transparent" />
      <div className="absolute inset-x-0 bottom-0 p-4 sm:p-6">
        <span className="mb-1 inline-block rounded bg-blue-600/80 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-white">
          {item.media_type === 'tv' ? 'TV Series' : 'Movie'}
        </span>
        <h2 className="text-xl font-bold text-white sm:text-2xl">{title}</h2>
        {overview && (
          <p className="mt-1 line-clamp-2 max-w-2xl text-xs text-slate-300 sm:text-sm">
            {overview}
          </p>
        )}
      </div>
    </button>
  )
}

function AddToLibraryModal({ target, onClose }: { target: AddTarget; onClose: () => void }) {
  const addSeries = useAddSeries()
  const addMovie = useAddMovie()
  const [added, setAdded] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const posterUrl = tmdbPosterUrl(target.posterPath, 'w342')
  const isPending = addSeries.isPending || addMovie.isPending

  const handleAdd = () => {
    setError(null)
    const year = parseInt(target.year, 10) || 0

    if (target.mediaType === 'tv') {
      addSeries.mutate(
        { title: target.title, year },
        {
          onSuccess: () => setAdded(true),
          onError: (e) => setError(e instanceof Error ? e.message : 'Failed to add'),
        },
      )
    } else {
      addMovie.mutate(
        { title: target.title, year },
        {
          onSuccess: () => setAdded(true),
          onError: (e) => setError(e instanceof Error ? e.message : 'Failed to add'),
        },
      )
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="w-full max-w-sm rounded-xl bg-slate-800 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-4 p-6">
          {posterUrl ? (
            <img src={posterUrl} alt={target.title} className="h-28 w-[75px] shrink-0 rounded-md object-cover" />
          ) : (
            <div className="flex h-28 w-[75px] shrink-0 items-center justify-center rounded-md bg-slate-700 text-slate-500 text-xs">
              No poster
            </div>
          )}
          <div className="flex-1 min-w-0">
            <h3 className="text-lg font-semibold text-white truncate">{target.title}</h3>
            <p className="text-sm text-slate-400">
              {target.year} &middot; {target.mediaType === 'tv' ? 'TV Series' : 'Movie'}
            </p>

            {added ? (
              <div className="mt-4 flex items-center gap-2 text-green-400 text-sm font-medium">
                <Check size={16} /> Added to library
              </div>
            ) : (
              <div className="mt-4 flex gap-2">
                <button
                  onClick={handleAdd}
                  disabled={isPending}
                  className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  {isPending ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <Plus size={14} />
                  )}
                  Add to Library
                </button>
                <button
                  onClick={onClose}
                  className="rounded-lg bg-slate-700 px-3 py-2 text-sm text-slate-300 hover:bg-slate-600 transition-colors"
                >
                  <X size={14} />
                </button>
              </div>
            )}

            {error && (
              <p className="mt-2 text-xs text-red-400">{error}</p>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
