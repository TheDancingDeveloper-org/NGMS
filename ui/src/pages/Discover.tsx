import { Loader2, Compass } from 'lucide-react'
import MediaCard from '../components/MediaCard'
import MediaSlider from '../components/MediaSlider'
import {
  useTrending,
  usePopularMovies,
  usePopularTv,
  useUpcomingMovies,
  useUpcomingTv,
  useSystemStatus,
} from '../hooks/useApi'
import { tmdbBackdropUrl, tmdbDisplayTitle } from '../api/types'
import type { TmdbTrendingItem, TmdbMovie, TmdbSeries } from '../api/types'

export default function Discover() {
  const { data: status } = useSystemStatus()
  const trending = useTrending('all', 'day')
  const popularMovies = usePopularMovies()
  const popularTv = usePopularTv()
  const upcomingMovies = useUpcomingMovies()
  const upcomingTv = useUpcomingTv()

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

  // Featured hero: first trending item with backdrop
  const heroItem = trending.data?.results?.find((r) => r.backdrop_path)

  return (
    <div className="space-y-6">
      {/* Hero Banner */}
      {heroItem && <HeroBanner item={heroItem} />}

      {/* Trending */}
      <MediaSlider title="Trending Today" isLoading={trending.isLoading}>
        {trending.data?.results?.map((item) => (
          <MediaCard key={`t-${item.id}-${item.media_type}`} item={item} />
        ))}
      </MediaSlider>

      {/* Popular Movies */}
      {(!modules || modules.movieManagement) && (
        <MediaSlider title="Popular Movies" isLoading={popularMovies.isLoading}>
          {popularMovies.data?.results?.map((item) => (
            <MediaCard key={`pm-${item.id}`} item={item} />
          ))}
        </MediaSlider>
      )}

      {/* Popular TV */}
      {(!modules || modules.tvManagement) && (
        <MediaSlider title="Popular TV Shows" isLoading={popularTv.isLoading}>
          {popularTv.data?.results?.map((item) => (
            <MediaCard key={`pt-${item.id}`} item={item} />
          ))}
        </MediaSlider>
      )}

      {/* Upcoming Movies */}
      {(!modules || modules.movieManagement) && (
        <MediaSlider title="Upcoming Movies" isLoading={upcomingMovies.isLoading}>
          {upcomingMovies.data?.results?.map((item) => (
            <MediaCard key={`um-${item.id}`} item={item} />
          ))}
        </MediaSlider>
      )}

      {/* Upcoming TV */}
      {(!modules || modules.tvManagement) && (
        <MediaSlider title="Upcoming TV Shows" isLoading={upcomingTv.isLoading}>
          {upcomingTv.data?.results?.map((item) => (
            <MediaCard key={`ut-${item.id}`} item={item} />
          ))}
        </MediaSlider>
      )}
    </div>
  )
}

function HeroBanner({ item }: { item: TmdbTrendingItem }) {
  const backdropUrl = tmdbBackdropUrl(item.backdrop_path)
  const title = tmdbDisplayTitle(item)
  const overview = item.overview || ''

  return (
    <div className="relative -mx-4 -mt-4 mb-2 overflow-hidden rounded-b-xl sm:-mx-6 sm:-mt-6">
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
    </div>
  )
}
