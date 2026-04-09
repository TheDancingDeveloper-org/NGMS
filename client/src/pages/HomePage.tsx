import { useMemo } from 'react'
import type { ContinueWatchingItem, Series, Movie } from '../api'
import MediaRow from '../components/MediaRow'
import { RowSkeleton } from '../components/Skeleton'
import { useContinueWatching, useSeries, useMovies } from '../hooks/useApi'

export default function HomePage() {
  const { data: continueItems = [], isLoading: contLoading } = useContinueWatching(20)
  const { data: allSeries = [], isLoading: seriesLoading } = useSeries()
  const { data: allMovies = [], isLoading: moviesLoading } = useMovies()

  const loading = contLoading || seriesLoading || moviesLoading

  const recentSeries = useMemo<ContinueWatchingItem[]>(() => {
    const sorted = [...allSeries].sort((a, b) => {
      const da = a.addedAt ? new Date(a.addedAt).getTime() : 0
      const db = b.addedAt ? new Date(b.addedAt).getTime() : 0
      return db - da
    })
    return sorted.slice(0, 20).map((s: Series) => ({
      id: 0,
      userId: 0,
      mediaFileId: 0,
      mediaType: 'series',
      mediaId: s.id,
      episodeId: null,
      positionSecs: 0,
      durationSecs: 0,
      completed: false,
      updatedAt: '',
      title: s.title,
      posterUrl: s.posterUrl,
      backdropUrl: s.fanartUrl,
      episodeTitle: null,
      seasonNumber: null,
      episodeNumber: null,
      year: s.year,
    }))
  }, [allSeries])

  const recentMovies = useMemo<ContinueWatchingItem[]>(() => {
    const sorted = [...allMovies].sort((a, b) => {
      const da = a.addedAt ? new Date(a.addedAt).getTime() : 0
      const db = b.addedAt ? new Date(b.addedAt).getTime() : 0
      return db - da
    })
    return sorted.slice(0, 20).map((m: Movie) => ({
      id: 0,
      userId: 0,
      mediaFileId: m.movieFile?.id || 0,
      mediaType: 'movie',
      mediaId: m.id,
      episodeId: null,
      positionSecs: 0,
      durationSecs: 0,
      completed: false,
      updatedAt: '',
      title: m.title,
      posterUrl: m.posterUrl,
      backdropUrl: m.fanartUrl,
      episodeTitle: null,
      seasonNumber: null,
      episodeNumber: null,
      year: m.year,
    }))
  }, [allMovies])

  if (loading) {
    return (
      <div>
        <RowSkeleton />
        <RowSkeleton />
        <RowSkeleton />
      </div>
    )
  }

  const hasContent = continueItems.length > 0 || recentSeries.length > 0 || recentMovies.length > 0

  return (
    <div>
      {!hasContent && (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 48 }}>
          No media available. Add some series or movies to get started.
        </div>
      )}

      <MediaRow title="Continue Watching" items={continueItems} />
      <MediaRow title="Series" items={recentSeries} />
      <MediaRow title="Movies" items={recentMovies} />
    </div>
  )
}
