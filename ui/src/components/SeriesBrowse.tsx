import { useState } from 'react'
import MediaSlider from './MediaSlider'
import MediaCard from './MediaCard'
import GenreSlider from './GenreSlider'
import AddToLibraryModal from './AddToLibraryModal'
import type { AddTarget } from './AddToLibraryModal'
import {
  useTrending,
  useTopRatedTv,
  useRecentTv,
  usePopularTv,
  useUpcomingTv,
  useTvGenres,
} from '../hooks/useApi'
import { tmdbYear } from '../api/types'
import type { TmdbTrendingItem } from '../api/types'

export default function SeriesBrowse() {
  const trending = useTrending('tv', 'day')
  const topRated = useTopRatedTv()
  const recent = useRecentTv()
  const popular = usePopularTv()
  const upcoming = useUpcomingTv()
  const { data: genreData } = useTvGenres()
  const [addTarget, setAddTarget] = useState<AddTarget | null>(null)

  const handleClick = (item: TmdbTrendingItem) => {
    const title = item.name || item.title || 'Unknown'
    const year = tmdbYear(item)
    setAddTarget({ id: item.id, title, year, mediaType: 'tv', posterPath: item.poster_path ?? null })
  }

  return (
    <div className="space-y-6">
      <MediaSlider title="Trending TV Shows" isLoading={trending.isLoading}>
        {trending.data?.results?.map((item) => (
          <MediaCard key={`t-${item.id}`} item={item} onClick={() => handleClick(item)} />
        ))}
      </MediaSlider>

      <MediaSlider title="Top Rated" isLoading={topRated.isLoading}>
        {topRated.data?.results?.map((item) => (
          <MediaCard
            key={`tr-${item.id}`}
            item={{ ...item, media_type: 'tv' } as TmdbTrendingItem}
            onClick={() => handleClick({ ...item, media_type: 'tv' } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>

      <MediaSlider title="Recently Aired" isLoading={recent.isLoading}>
        {recent.data?.results?.map((item) => (
          <MediaCard
            key={`rr-${item.id}`}
            item={{ ...item, media_type: 'tv' } as TmdbTrendingItem}
            onClick={() => handleClick({ ...item, media_type: 'tv' } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>

      <MediaSlider title="Popular" isLoading={popular.isLoading}>
        {popular.data?.results?.map((item) => (
          <MediaCard
            key={`p-${item.id}`}
            item={{ ...item, media_type: 'tv' } as TmdbTrendingItem}
            onClick={() => handleClick({ ...item, media_type: 'tv' } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>

      <MediaSlider title="Upcoming" isLoading={upcoming.isLoading}>
        {upcoming.data?.results?.map((item) => (
          <MediaCard
            key={`u-${item.id}`}
            item={{ ...item, media_type: 'tv' } as TmdbTrendingItem}
            onClick={() => handleClick({ ...item, media_type: 'tv' } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>

      {genreData?.genres?.map((genre) => (
        <GenreSlider key={genre.id} genre={genre} mediaType="tv" onItemClick={handleClick} />
      ))}

      {addTarget && <AddToLibraryModal target={addTarget} onClose={() => setAddTarget(null)} />}
    </div>
  )
}
