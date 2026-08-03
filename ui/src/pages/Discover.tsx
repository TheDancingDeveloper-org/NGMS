// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState } from 'react'
import { Loader2, Compass } from 'lucide-react'
import MediaCard from '../components/MediaCard'
import MediaSlider from '../components/MediaSlider'
import AddToLibraryModal from '../components/AddToLibraryModal'
import type { AddTarget } from '../components/AddToLibraryModal'
import {
  useTrending,
  usePopularMovies,
  usePopularTv,
  useUpcomingMovies,
  useUpcomingTv,
  useSystemStatus,
} from '../hooks/useApi'
import { tmdbBackdropUrl, tmdbDisplayTitle, tmdbYear } from '../api/types'
import type { TmdbTrendingItem } from '../api/types'

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

      {/* Trending — tighter gap after hero */}
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
      className="relative -mx-6 -mt-2 mb-0 block w-[calc(100%+3rem)] overflow-hidden rounded-b-xl text-left hover:brightness-110 transition"
    >
      {backdropUrl && (
        <img
          src={backdropUrl}
          alt={title}
          className="h-[180px] w-full object-cover object-top sm:h-[220px]"
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

