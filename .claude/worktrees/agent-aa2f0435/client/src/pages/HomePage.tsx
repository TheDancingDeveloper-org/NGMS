import { useEffect, useState } from 'react'
import { api, type ContinueWatchingItem, type Series, type Movie, imageUrl } from '../api'
import MediaRow from '../components/MediaRow'

export default function HomePage() {
  const [continueItems, setContinueItems] = useState<ContinueWatchingItem[]>([])
  const [recentSeries, setRecentSeries] = useState<ContinueWatchingItem[]>([])
  const [recentMovies, setRecentMovies] = useState<ContinueWatchingItem[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const load = async () => {
      try {
        const [cont, series, movies] = await Promise.all([
          api.getContinueWatching(20),
          api.listSeries(),
          api.listMovies(),
        ])
        setContinueItems(cont)

        // Build "Recently Added" from series/movies lists (most recent first, limited)
        const recentSeriesItems: ContinueWatchingItem[] = series
          .slice(0, 20)
          .map((s: Series) => ({
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
            posterUrl: imageUrl(s.images, 'poster'),
            backdropUrl: imageUrl(s.images, 'fanart'),
            episodeTitle: null,
            seasonNumber: null,
            episodeNumber: null,
            year: s.year,
          }))
        setRecentSeries(recentSeriesItems)

        const recentMovieItems: ContinueWatchingItem[] = movies
          .slice(0, 20)
          .map((m: Movie) => ({
            id: 0,
            userId: 0,
            mediaFileId: m.movieFileId || 0,
            mediaType: 'movie',
            mediaId: m.id,
            episodeId: null,
            positionSecs: 0,
            durationSecs: 0,
            completed: false,
            updatedAt: '',
            title: m.title,
            posterUrl: imageUrl(m.images, 'poster'),
            backdropUrl: imageUrl(m.images, 'fanart'),
            episodeTitle: null,
            seasonNumber: null,
            episodeNumber: null,
            year: m.year,
          }))
        setRecentMovies(recentMovieItems)
      } catch (e) {
        console.error('Failed to load home page data:', e)
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [])

  if (loading) {
    return (
      <div style={{ color: '#64748b', textAlign: 'center', padding: 48 }}>
        Loading...
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
