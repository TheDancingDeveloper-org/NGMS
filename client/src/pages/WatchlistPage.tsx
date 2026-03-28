import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Bookmark, X, Tv, Film } from 'lucide-react'
import { api, type WatchlistItem } from '../api'
import { useMobile } from '../hooks/useMobile'

type FilterTab = 'all' | 'series' | 'movie'

export default function WatchlistPage() {
  const navigate = useNavigate()
  const isMobile = useMobile()
  const [items, setItems] = useState<WatchlistItem[]>([])
  const [loading, setLoading] = useState(true)
  const [filter, setFilter] = useState<FilterTab>('all')

  const load = () => {
    setLoading(true)
    api
      .getWatchlist(filter === 'all' ? undefined : filter)
      .then(setItems)
      .catch(() => {})
      .finally(() => setLoading(false))
  }

  useEffect(() => {
    load()
  }, [filter])

  const remove = async (mediaType: string, mediaId: number) => {
    try {
      await api.removeFromWatchlist(mediaType, mediaId)
      setItems((prev) => prev.filter((i) => !(i.mediaType === mediaType && i.mediaId === mediaId)))
    } catch (e) {
      console.error('Failed to remove from watchlist:', e)
    }
  }

  const tabStyle = (active: boolean) => ({
    padding: '8px 16px',
    borderRadius: 8,
    fontSize: 13,
    fontWeight: 500 as const,
    border: 'none',
    cursor: 'pointer' as const,
    color: active ? '#fff' : '#94a3b8',
    background: active ? '#1e40af' : '#1e293b',
    transition: 'all 0.15s',
  })

  return (
    <div>
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        marginBottom: isMobile ? 16 : 24,
        flexWrap: isMobile ? 'wrap' : 'nowrap', gap: isMobile ? 8 : 0,
      }}>
        <h1 style={{
          fontSize: isMobile ? 18 : 22, fontWeight: 700, color: '#f1f5f9', margin: 0,
          display: 'flex', alignItems: 'center', gap: 8,
        }}>
          <Bookmark size={isMobile ? 18 : 22} /> My Watchlist
        </h1>
        <div style={{ display: 'flex', gap: 8 }}>
          <button style={tabStyle(filter === 'all')} onClick={() => setFilter('all')}>All</button>
          <button style={tabStyle(filter === 'series')} onClick={() => setFilter('series')}>Series</button>
          <button style={tabStyle(filter === 'movie')} onClick={() => setFilter('movie')}>Movies</button>
        </div>
      </div>

      {loading ? (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 48 }}>Loading...</div>
      ) : items.length === 0 ? (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 48 }}>
          Your watchlist is empty. Browse your library and bookmark items to watch later.
        </div>
      ) : (
        <div style={{
          display: 'grid',
          gridTemplateColumns: isMobile
            ? 'repeat(auto-fill, minmax(110px, 1fr))'
            : 'repeat(auto-fill, minmax(160px, 1fr))',
          gap: isMobile ? 10 : 16,
        }}>
          {items.map((item) => (
            <div
              key={`${item.mediaType}-${item.mediaId}`}
              style={{
                position: 'relative',
                borderRadius: 10,
                overflow: 'hidden',
                background: '#1e293b',
                border: '1px solid #334155',
                cursor: 'pointer',
                transition: 'transform 0.15s',
              }}
              onClick={() =>
                navigate(item.mediaType === 'series' ? `/series/${item.mediaId}` : `/movie/${item.mediaId}`)
              }
            >
              {item.posterUrl ? (
                <img
                  src={item.posterUrl}
                  alt={item.title || ''}
                  style={{ width: '100%', aspectRatio: '2/3', objectFit: 'cover', display: 'block' }}
                />
              ) : (
                <div style={{
                  width: '100%',
                  aspectRatio: '2/3',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  background: '#0f172a',
                }}>
                  {item.mediaType === 'series' ? <Tv size={32} color="#475569" /> : <Film size={32} color="#475569" />}
                </div>
              )}

              <div style={{ padding: '8px 10px' }}>
                <div style={{
                  fontSize: 13,
                  fontWeight: 600,
                  color: '#f1f5f9',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                  whiteSpace: 'nowrap',
                }}>
                  {item.title || 'Unknown'}
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, marginTop: 4, fontSize: 11, color: '#64748b' }}>
                  <span style={{ textTransform: 'capitalize' }}>{item.mediaType}</span>
                  {item.year && <span>{item.year}</span>}
                </div>
              </div>

              {/* Remove button */}
              <button
                onClick={(e) => {
                  e.stopPropagation()
                  remove(item.mediaType, item.mediaId)
                }}
                title="Remove from watchlist"
                style={{
                  position: 'absolute',
                  top: 6,
                  right: 6,
                  width: 28,
                  height: 28,
                  borderRadius: '50%',
                  background: 'rgba(0,0,0,0.7)',
                  border: 'none',
                  color: '#fff',
                  cursor: 'pointer',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <X size={14} />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
