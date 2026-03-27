import { useState, useCallback } from 'react'
import { Search, Film, Tv } from 'lucide-react'
import { api, type DiscoverResult, type DiscoverSearchResults } from '../api'
import DiscoverGrid from '../components/DiscoverGrid'

export default function DiscoverPage() {
  const [query, setQuery] = useState('')
  const [mediaType, setMediaType] = useState<'movie' | 'series'>('movie')
  const [results, setResults] = useState<DiscoverSearchResults | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const doSearch = useCallback(
    async (q: string, type: 'movie' | 'series') => {
      if (q.length < 2) return
      setLoading(true)
      setError(null)
      try {
        const data = await api.discoverSearch(q, type)
        setResults(data)
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Search failed')
      } finally {
        setLoading(false)
      }
    },
    [],
  )

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault()
    doSearch(query, mediaType)
  }

  const handleTypeToggle = (type: 'movie' | 'series') => {
    setMediaType(type)
    if (query.length >= 2) {
      doSearch(query, type)
    }
  }

  const handleRequest = async (item: DiscoverResult) => {
    try {
      await api.createRequest({
        mediaType: item.mediaType,
        tmdbId: item.id,
        title: item.title || item.name || 'Unknown',
        year: parseInt(
          (item.releaseDate || item.firstAirDate || '').substring(0, 4),
        ) || undefined,
        posterUrl: item.posterPath || undefined,
        overview: item.overview || undefined,
      })
      // Re-fetch to update status
      if (query.length >= 2) {
        doSearch(query, mediaType)
      }
    } catch (e) {
      alert(e instanceof Error ? e.message : 'Failed to submit request')
    }
  }

  return (
    <div>
      <h2 style={{ color: '#f1f5f9', fontSize: 20, fontWeight: 700, marginBottom: 16 }}>
        Discover & Request
      </h2>

      <form onSubmit={handleSearch} style={{ display: 'flex', gap: 8, marginBottom: 16 }}>
        <div style={{ display: 'flex', gap: 4 }}>
          <button
            type="button"
            onClick={() => handleTypeToggle('movie')}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              padding: '8px 12px',
              borderRadius: 8,
              border: 'none',
              cursor: 'pointer',
              fontSize: 13,
              fontWeight: 500,
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
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              padding: '8px 12px',
              borderRadius: 8,
              border: 'none',
              cursor: 'pointer',
              fontSize: 13,
              fontWeight: 500,
              background: mediaType === 'series' ? '#1e40af' : '#334155',
              color: mediaType === 'series' ? '#fff' : '#94a3b8',
            }}
          >
            <Tv size={14} /> TV Shows
          </button>
        </div>
        <div style={{ flex: 1, position: 'relative' }}>
          <Search
            size={16}
            style={{
              position: 'absolute',
              left: 12,
              top: '50%',
              transform: 'translateY(-50%)',
              color: '#64748b',
            }}
          />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={`Search ${mediaType === 'series' ? 'TV shows' : 'movies'}...`}
            style={{
              width: '100%',
              padding: '8px 12px 8px 36px',
              borderRadius: 8,
              border: '1px solid #334155',
              background: '#0f172a',
              color: '#f1f5f9',
              fontSize: 14,
              outline: 'none',
            }}
          />
        </div>
        <button
          type="submit"
          disabled={loading || query.length < 2}
          style={{
            padding: '8px 20px',
            borderRadius: 8,
            border: 'none',
            background: '#1e40af',
            color: '#fff',
            fontSize: 14,
            fontWeight: 500,
            cursor: query.length < 2 ? 'not-allowed' : 'pointer',
            opacity: query.length < 2 ? 0.5 : 1,
          }}
        >
          {loading ? 'Searching...' : 'Search'}
        </button>
      </form>

      {error && (
        <div
          style={{
            padding: 12,
            background: '#7f1d1d',
            color: '#fca5a5',
            borderRadius: 8,
            marginBottom: 16,
            fontSize: 13,
          }}
        >
          {error}
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
          <DiscoverGrid
            results={results.results}
            onRequest={handleRequest}
          />
        </>
      )}

      {!results && !loading && (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 60 }}>
          <Search size={48} style={{ marginBottom: 12, opacity: 0.3 }} />
          <p>Search for movies or TV shows to request them.</p>
        </div>
      )}
    </div>
  )
}
