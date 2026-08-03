// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState } from 'react'
import MediaSlider from './MediaSlider'
import MediaCard from './MediaCard'
import GenreSlider from './GenreSlider'
import AddToLibraryModal from './AddToLibraryModal'
import type { AddTarget } from './AddToLibraryModal'
import {
  useTrending,
  useTopRatedMovies,
  useRecentMovies,
  usePopularMovies,
  useUpcomingMovies,
  useMovieGenres,
} from '../hooks/useApi'
import { tmdbYear } from '../api/types'
import type { TmdbTrendingItem } from '../api/types'

export default function MovieBrowse() {
  const trending = useTrending('movie', 'day')
  const topRated = useTopRatedMovies()
  const recent = useRecentMovies()
  const popular = usePopularMovies()
  const upcoming = useUpcomingMovies()
  const { data: genreData } = useMovieGenres()
  const [addTarget, setAddTarget] = useState<AddTarget | null>(null)

  const handleClick = (item: TmdbTrendingItem) => {
    const title = item.title || item.name || 'Unknown'
    const year = tmdbYear(item)
    setAddTarget({ id: item.id, title, year, mediaType: 'movie', posterPath: item.poster_path ?? null })
  }

  return (
    <div className="space-y-6">
      <MediaSlider title="Trending Movies" isLoading={trending.isLoading}>
        {trending.data?.results?.map((item) => (
          <MediaCard key={`t-${item.id}`} item={item} onClick={() => handleClick(item)} />
        ))}
      </MediaSlider>

      <MediaSlider title="Top Rated" isLoading={topRated.isLoading}>
        {topRated.data?.results?.map((item) => (
          <MediaCard
            key={`tr-${item.id}`}
            item={{ ...item, media_type: 'movie' } as TmdbTrendingItem}
            onClick={() => handleClick({ ...item, media_type: 'movie' } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>

      <MediaSlider title="Recently Released" isLoading={recent.isLoading}>
        {recent.data?.results?.map((item) => (
          <MediaCard
            key={`rr-${item.id}`}
            item={{ ...item, media_type: 'movie' } as TmdbTrendingItem}
            onClick={() => handleClick({ ...item, media_type: 'movie' } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>

      <MediaSlider title="Popular" isLoading={popular.isLoading}>
        {popular.data?.results?.map((item) => (
          <MediaCard
            key={`p-${item.id}`}
            item={{ ...item, media_type: 'movie' } as TmdbTrendingItem}
            onClick={() => handleClick({ ...item, media_type: 'movie' } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>

      <MediaSlider title="Upcoming" isLoading={upcoming.isLoading}>
        {upcoming.data?.results?.map((item) => (
          <MediaCard
            key={`u-${item.id}`}
            item={{ ...item, media_type: 'movie' } as TmdbTrendingItem}
            onClick={() => handleClick({ ...item, media_type: 'movie' } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>

      {genreData?.genres?.map((genre) => (
        <GenreSlider key={genre.id} genre={genre} mediaType="movie" onItemClick={handleClick} />
      ))}

      {addTarget && <AddToLibraryModal target={addTarget} onClose={() => setAddTarget(null)} />}
    </div>
  )
}
