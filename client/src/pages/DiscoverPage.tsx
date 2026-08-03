// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState, useCallback, useRef, useEffect } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { Search, Film, Tv } from 'lucide-react'
import { useQuery } from '@tanstack/react-query'
import { type DiscoverResult, type DiscoverSlider, api } from '../api'
import type { TmdbDisplayItem } from '../components/TmdbRow'
import TmdbRow from '../components/TmdbRow'
import DiscoverGrid from '../components/DiscoverGrid'
import { useMobile } from '../hooks/useMobile'
import { useDiscoverSearch, useCreateRequest, useDiscoverSliders } from '../hooks/useApi'

// ── Slider data fetcher ─────────────────────────────────────────────────────

function sliderQueryFn(slider: DiscoverSlider) {
  const t = slider.sliderType
  const customData = slider.customData as Record<string, number | string> | null

  switch (t) {
    case 'trending':
      return () => api.getTrending({ timeWindow: 'day' }).then((r) => r.results)
    case 'popular_movies':
      return () => api.getDiscoverMovies().then((r) => r.results)
    case 'popular_tv':
      return () => api.getDiscoverTv().then((r) => r.results)
    case 'upcoming_movies':
      return () => api.getUpcomingMovies().then((r) => r.results)
    case 'upcoming_tv':
      return () => api.getUpcomingTv().then((r) => r.results)
    case 'tmdb_movie_genre':
      if (customData?.genreId) {
        return () => api.getMoviesByGenre(Number(customData.genreId)).then((r) => r.results)
      }
      return null
    case 'tmdb_tv_genre':
      if (customData?.genreId) {
        return () => api.getTvByGenre(Number(customData.genreId)).then((r) => r.results)
      }
      return null
    case 'tmdb_studio':
      if (customData?.studioId) {
        return () => api.getMoviesByStudio(Number(customData.studioId)).then((r) => r.results)
      }
      return null
    case 'tmdb_network':
      if (customData?.networkId) {
        return () => api.getTvByNetwork(Number(customData.networkId)).then((r) => r.results)
      }
      return null
    default:
      return null
  }
}

function sliderTitle(slider: DiscoverSlider): string {
  if (slider.title) return slider.title
  switch (slider.sliderType) {
    case 'trending': return 'Trending'
    case 'popular_movies': return 'Popular Movies'
    case 'popular_tv': return 'Popular TV Shows'
    case 'upcoming_movies': return 'Upcoming Movies'
    case 'upcoming_tv': return 'Upcoming TV Shows'
    case 'recently_added': return 'Recently Added'
    case 'tmdb_movie_genre': return 'Movies'
    case 'tmdb_tv_genre': return 'TV Shows'
    case 'tmdb_studio': return 'Studio'
    case 'tmdb_network': return 'Network'
    default: return 'Discover'
  }
}

// ── Individual slider row (self-fetching) ───────────────────────────────────

function SliderRow({
  slider,
  onItemClick,
}: {
  slider: DiscoverSlider
  onItemClick: (item: TmdbDisplayItem) => void
}) {
  const qfn = sliderQueryFn(slider)

  const { data: items = [], isLoading } = useQuery({
    queryKey: ['discover', 'slider', slider.id, slider.sliderType],
    queryFn: qfn ?? (() => Promise.resolve([])),
    enabled: qfn !== null,
    staleTime: 5 * 60_000,
  })

  if (!qfn) return null

  return (
    <TmdbRow
      title={sliderTitle(slider)}
      items={items}
      loading={isLoading}
      onItemClick={onItemClick}
    />
  )
}

// ── Fallback sliders when server config unavailable ─────────────────────────

const FALLBACK_SLIDERS: DiscoverSlider[] = [
  { id: -1, sliderType: 'trending', displayOrder: 0, isBuiltIn: true, enabled: true, title: 'Trending Now', customData: null, createdAt: '', updatedAt: '' },
  { id: -2, sliderType: 'popular_movies', displayOrder: 1, isBuiltIn: true, enabled: true, title: 'Popular Movies', customData: null, createdAt: '', updatedAt: '' },
  { id: -3, sliderType: 'popular_tv', displayOrder: 2, isBuiltIn: true, enabled: true, title: 'Popular TV Shows', customData: null, createdAt: '', updatedAt: '' },
  { id: -4, sliderType: 'upcoming_movies', displayOrder: 3, isBuiltIn: true, enabled: true, title: 'Upcoming Movies', customData: null, createdAt: '', updatedAt: '' },
  { id: -5, sliderType: 'upcoming_tv', displayOrder: 4, isBuiltIn: true, enabled: true, title: 'Upcoming TV Shows', customData: null, createdAt: '', updatedAt: '' },
]

// ── Main page ───────────────────────────────────────────────────────────────

export default function DiscoverPage() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const isMobile = useMobile()

  // Initialise from URL params (recommendation clicks: /discover?q=...&type=...)
  const urlQ = searchParams.get('q') ?? ''
  const urlType = searchParams.get('type')
  const initType = (urlType === 'series' || urlType === 'movie') ? urlType : 'movie'

  const searchRef = useRef<HTMLInputElement>(null)
  const [query, setQuery] = useState(urlQ)
  const [submittedQuery, setSubmittedQuery] = useState(urlQ.length >= 2 ? urlQ : '')
  const [mediaType, setMediaType] = useState<'movie' | 'series'>(initType)

  // Auto-focus search input when arriving without a query (e.g. via / shortcut)
  useEffect(() => {
    if (!urlQ && searchRef.current) {
      searchRef.current.focus()
    }
  }, [urlQ])

  const isSearchActive = submittedQuery.length >= 2

  const { data: results, isLoading: searchLoading, error } = useDiscoverSearch(
    submittedQuery,
    mediaType,
    isSearchActive,
  )

  const { data: serverSliders } = useDiscoverSliders()
  const createRequest = useCreateRequest()

  const sliders = serverSliders && serverSliders.length > 0
    ? serverSliders.filter((s) => s.enabled).sort((a, b) => a.displayOrder - b.displayOrder)
    : FALLBACK_SLIDERS

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault()
    setSubmittedQuery(query)
  }

  const clearSearch = () => {
    setQuery('')
    setSubmittedQuery('')
  }

  const handleTypeToggle = (type: 'movie' | 'series') => {
    setMediaType(type)
  }

  const handleRequest = async (item: DiscoverResult) => {
    try {
      await createRequest.mutateAsync({
        mediaType: item.mediaType,
        tmdbId: item.id,
        title: item.title || item.name || 'Unknown',
        year: parseInt(
          (item.releaseDate || item.firstAirDate || '').substring(0, 4),
        ) || undefined,
        posterUrl: item.posterPath || undefined,
        overview: item.overview || undefined,
      })
    } catch (e) {
      alert(e instanceof Error ? e.message : 'Failed to submit request')
    }
  }

  const handleSliderItemClick = useCallback((item: TmdbDisplayItem) => {
    const title = item.title || item.name || ''
    if (title) {
      const type = item.mediaType === 'tv' ? 'series' : (item.mediaType || 'movie')
      setMediaType(type as 'movie' | 'series')
      setQuery(title)
      setSubmittedQuery(title)
      window.scrollTo({ top: 0, behavior: 'smooth' })
    }
  }, [])

  // Click handler for recommendation items that navigates to library if possible
  const handleRecommendationClick = useCallback((item: TmdbDisplayItem) => {
    // For items with a known media type, navigate to discover search
    const title = item.title || item.name || ''
    if (title) {
      const type = item.mediaType === 'tv' ? 'series' : 'movie'
      navigate(`/discover?q=${encodeURIComponent(title)}&type=${type}`)
      setMediaType(type as 'movie' | 'series')
      setQuery(title)
      setSubmittedQuery(title)
    }
  }, [navigate])

  // Use the simpler click handler since we're already on the discover page
  void handleRecommendationClick

  return (
    <div>
      <h2 style={{ color: '#f1f5f9', fontSize: 20, fontWeight: 700, marginBottom: 16 }}>
        Discover & Request
      </h2>

      {/* Search bar */}
      <form onSubmit={handleSearch} style={{
        display: 'flex', gap: 8, marginBottom: 20,
        flexWrap: isMobile ? 'wrap' : 'nowrap',
      }}>
        <div style={{ display: 'flex', gap: 4 }}>
          <button
            type="button"
            onClick={() => handleTypeToggle('movie')}
            style={{
              display: 'flex', alignItems: 'center', gap: 4,
              padding: '8px 12px', borderRadius: 8, border: 'none', cursor: 'pointer',
              fontSize: 13, fontWeight: 500,
              background: mediaType === 'movie' ? '#1e40af' : '#334155',
              color: mediaType === 'movie' ? '#fff' : '#94a3b8',
            }}
          >
            <Film size={14} /> Movies
          </button>
          <button
            type="button"
            onClick={() => handleTypeToggle('series')}
            style={{
              display: 'flex', alignItems: 'center', gap: 4,
              padding: '8px 12px', borderRadius: 8, border: 'none', cursor: 'pointer',
              fontSize: 13, fontWeight: 500,
              background: mediaType === 'series' ? '#1e40af' : '#334155',
              color: mediaType === 'series' ? '#fff' : '#94a3b8',
            }}
          >
            <Tv size={14} /> TV Shows
          </button>
        </div>
        <div style={{ flex: 1, position: 'relative' }}>
          <Search size={16} style={{
            position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', color: '#64748b',
          }} />
          <input
            ref={searchRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Search ${mediaType === 'series' ? 'TV shows' : 'movies'}...`}
            style={{
              width: '100%', padding: '8px 12px 8px 36px', borderRadius: 8,
              border: '1px solid #334155', background: '#0f172a',
              color: '#f1f5f9', fontSize: 14, outline: 'none',
            }}
          />
        </div>
        {isSearchActive ? (
          <button
            type="button"
            onClick={clearSearch}
            style={{
              padding: '8px 16px', borderRadius: 8, border: '1px solid #334155',
              background: 'transparent', color: '#94a3b8', fontSize: 14,
              fontWeight: 500, cursor: 'pointer',
            }}
          >
            Clear
          </button>
        ) : (
          <button
            type="submit"
            disabled={searchLoading || query.length < 2}
            style={{
              padding: '8px 20px', borderRadius: 8, border: 'none',
              background: '#1e40af', color: '#fff', fontSize: 14,
              fontWeight: 500,
              cursor: query.length < 2 ? 'not-allowed' : 'pointer',
              opacity: query.length < 2 ? 0.5 : 1,
            }}
          >
            {searchLoading ? 'Searching...' : 'Search'}
          </button>
        )}
      </form>

      {/* Search results mode */}
      {isSearchActive && (
        <div>
          {error && (
            <div style={{
              padding: 12, background: '#7f1d1d', color: '#fca5a5',
              borderRadius: 8, marginBottom: 16, fontSize: 13,
            }}>
              {error instanceof Error ? error.message : 'Search failed'}
            </div>
          )}

          {searchLoading && (
            <div style={{ color: '#94a3b8', textAlign: 'center', padding: 40 }}>
              Searching...
            </div>
          )}

          {results && results.results.length === 0 && (
            <div style={{ color: '#94a3b8', textAlign: 'center', padding: 40 }}>
              No results found. Try a different search term.
            </div>
          )}

          {results && results.results.length > 0 && (
            <>
              <div style={{ color: '#64748b', fontSize: 12, marginBottom: 12 }}>
                {results.totalResults} result{results.totalResults !== 1 ? 's' : ''} found
              </div>
              <DiscoverGrid results={results.results} onRequest={handleRequest} />
            </>
          )}
        </div>
      )}

      {/* Browse mode — slider rows */}
      {!isSearchActive && (
        <div>
          {sliders.map((slider) => (
            <SliderRow
              key={slider.id}
              slider={slider}
              onItemClick={handleSliderItemClick}
            />
          ))}

          {sliders.length === 0 && (
            <div style={{ color: '#64748b', textAlign: 'center', padding: 60 }}>
              <Search size={48} style={{ marginBottom: 12, opacity: 0.3 }} />
              <p>Search for movies or TV shows to request them.</p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
