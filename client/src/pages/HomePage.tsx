import { useMemo, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { Play } from 'lucide-react'
import { imageUrl, type ContinueWatchingItem, type Series, type Movie } from '../api'
import type { TmdbDisplayItem } from '../components/TmdbRow'
import MediaRow from '../components/MediaRow'
import TmdbRow from '../components/TmdbRow'
import { RowSkeleton } from '../components/Skeleton'
import { useMobile } from '../hooks/useMobile'
import { useContinueWatching, useSeries, useMovies, useTrending } from '../hooks/useApi'

// ── Hero banner from trending ───────────────────────────────────────────────

function HeroBanner({ item, onPlay }: {
  item: TmdbDisplayItem
  onPlay: () => void
}) {
  const isMobile = useMobile()
  const TMDB_BASE = 'https://image.tmdb.org/t/p'
  const backdrop = item.posterPath
    ? imageUrl(`/api/v1/images/${TMDB_BASE}/w1280${item.posterPath}`)
    : null
  const title = item.title || item.name || ''
  const year = (item.releaseDate || item.firstAirDate || '').substring(0, 4)

  return (
    <div style={{
      position: 'relative', borderRadius: 14, overflow: 'hidden',
      marginBottom: isMobile ? 20 : 28, background: '#1e293b',
      height: isMobile ? 200 : 280,
    }}>
      {backdrop && (
        <img
          src={backdrop}
          alt=""
          style={{ width: '100%', height: '100%', objectFit: 'cover', opacity: 0.4 }}
        />
      )}
      <div style={{
        position: 'absolute', bottom: 0, left: 0, right: 0,
        padding: isMobile ? '16px' : '28px 32px',
        background: 'linear-gradient(transparent, rgba(15, 23, 42, 0.95))',
      }}>
        <div style={{ fontSize: 11, color: '#3b82f6', fontWeight: 600, textTransform: 'uppercase', letterSpacing: 1, marginBottom: 4 }}>
          Trending
        </div>
        <h2 style={{ fontSize: isMobile ? 20 : 28, fontWeight: 700, color: '#fff', margin: 0 }}>
          {title}
        </h2>
        <div style={{ display: 'flex', gap: 12, alignItems: 'center', marginTop: 8 }}>
          {year && <span style={{ fontSize: 13, color: '#94a3b8' }}>{year}</span>}
          {item.voteAverage != null && item.voteAverage > 0 && (
            <span style={{ fontSize: 13, color: '#fbbf24' }}>
              {item.voteAverage.toFixed(1)}
            </span>
          )}
          <button
            onClick={onPlay}
            style={{
              display: 'flex', alignItems: 'center', gap: 6,
              padding: '6px 16px', borderRadius: 8,
              background: '#2563eb', border: 'none', color: '#fff',
              cursor: 'pointer', fontSize: 13, fontWeight: 600,
            }}
          >
            <Play size={14} fill="#fff" /> Discover
          </button>
        </div>
      </div>
    </div>
  )
}

// ── Main page ───────────────────────────────────────────────────────────────

export default function HomePage() {
  const navigate = useNavigate()
  const { data: continueItems = [], isLoading: contLoading } = useContinueWatching(20)
  const { data: allSeries = [], isLoading: seriesLoading } = useSeries()
  const { data: allMovies = [], isLoading: moviesLoading } = useMovies()
  const { data: trending, isLoading: trendingLoading } = useTrending({ timeWindow: 'day' })

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

  const heroItem = trending?.results?.[0] ?? null
  const trendingRow = useMemo(() => trending?.results?.slice(1, 21) ?? [], [trending])

  const handleTrendingClick = useCallback((item: TmdbDisplayItem) => {
    const title = item.title || item.name || ''
    if (title) {
      navigate(`/discover?q=${encodeURIComponent(title)}`)
    }
  }, [navigate])

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
      {/* Hero banner */}
      {heroItem && (
        <HeroBanner
          item={heroItem}
          onPlay={() => {
            const title = heroItem.title || heroItem.name || ''
            navigate(`/discover?q=${encodeURIComponent(title)}`)
          }}
        />
      )}

      {!hasContent && !heroItem && (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 48 }}>
          No media available. Add some series or movies to get started.
        </div>
      )}

      <MediaRow title="Continue Watching" items={continueItems} />
      <MediaRow title="Recently Added Series" items={recentSeries} />
      <MediaRow title="Recently Added Movies" items={recentMovies} />

      {/* Trending row */}
      <TmdbRow
        title="Trending Today"
        items={trendingRow}
        loading={trendingLoading}
        onItemClick={handleTrendingClick}
      />
    </div>
  )
}
