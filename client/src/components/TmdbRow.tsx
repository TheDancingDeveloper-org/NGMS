import { Film } from 'lucide-react'
import { imageUrl } from '../api'
import { useMobile } from '../hooks/useMobile'

const TMDB_IMAGE_BASE = 'https://image.tmdb.org/t/p'

export interface TmdbDisplayItem {
  id: number
  posterPath?: string | null
  title?: string
  name?: string
  releaseDate?: string
  firstAirDate?: string
  voteAverage?: number
  mediaType?: string
}

function getTitle(item: TmdbDisplayItem): string {
  return item.title || item.name || 'Unknown'
}

function getYear(item: TmdbDisplayItem): string {
  const date = item.releaseDate || item.firstAirDate
  return date?.substring(0, 4) || ''
}

function posterSrc(item: TmdbDisplayItem): string | undefined {
  if (!item.posterPath) return undefined
  return imageUrl(`/api/v1/images/${TMDB_IMAGE_BASE}/w342${item.posterPath}`)
}

export default function TmdbRow({
  title,
  items,
  onItemClick,
  loading,
}: {
  title: string
  items: TmdbDisplayItem[]
  onItemClick?: (item: TmdbDisplayItem) => void
  loading?: boolean
}) {
  const isMobile = useMobile()

  if (loading) {
    return (
      <div style={{ marginBottom: isMobile ? 20 : 32 }}>
        <h3 style={{
          fontSize: isMobile ? 15 : 17, fontWeight: 600, color: '#e2e8f0',
          marginBottom: isMobile ? 8 : 12, paddingLeft: 4,
        }}>
          {title}
        </h3>
        <div style={{ display: 'flex', gap: isMobile ? 10 : 14 }}>
          {Array.from({ length: 8 }).map((_, i) => (
            <div key={i} style={{
              flex: isMobile ? '0 0 110px' : '0 0 150px',
              borderRadius: 10, overflow: 'hidden', background: '#1e293b',
            }}>
              <div style={{
                width: isMobile ? 110 : 150, height: isMobile ? 165 : 225,
                background: '#334155', animation: 'shimmer 1.5s infinite',
              }} />
              <div style={{ padding: '8px 10px' }}>
                <div style={{ height: 14, width: '70%', background: '#334155', borderRadius: 4 }} />
                <div style={{ height: 10, width: '40%', background: '#334155', borderRadius: 4, marginTop: 6 }} />
              </div>
            </div>
          ))}
        </div>
      </div>
    )
  }

  if (items.length === 0) return null

  return (
    <div style={{ marginBottom: isMobile ? 20 : 32 }}>
      <style>{`
        .tmdb-card { transition: transform 0.15s, box-shadow 0.15s; }
        .tmdb-card:hover { transform: scale(1.04); box-shadow: 0 4px 20px rgba(0,0,0,0.4); }
        @keyframes shimmer {
          0% { opacity: 0.5; }
          50% { opacity: 0.8; }
          100% { opacity: 0.5; }
        }
      `}</style>
      <h3 style={{
        fontSize: isMobile ? 15 : 17, fontWeight: 600, color: '#e2e8f0',
        marginBottom: isMobile ? 8 : 12, paddingLeft: 4,
      }}>
        {title}
      </h3>
      <div style={{
        display: 'flex',
        gap: isMobile ? 10 : 14,
        overflowX: 'auto',
        paddingBottom: 8,
        scrollbarWidth: 'thin',
        WebkitOverflowScrolling: 'touch',
      }}>
        {items.map((item) => (
          <div
            key={`${item.mediaType}-${item.id}`}
            className="tmdb-card"
            onClick={() => onItemClick?.(item)}
            style={{
              flex: isMobile ? '0 0 110px' : '0 0 150px',
              cursor: onItemClick ? 'pointer' : 'default',
              borderRadius: 10,
              overflow: 'hidden',
              background: '#1e293b',
            }}
          >
            <div style={{
              width: isMobile ? 110 : 150,
              height: isMobile ? 165 : 225,
              background: '#334155',
              position: 'relative',
            }}>
              {posterSrc(item) ? (
                <img
                  src={posterSrc(item)}
                  alt={getTitle(item)}
                  style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                  loading="lazy"
                />
              ) : (
                <div style={{
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                  height: '100%', color: '#64748b',
                }}>
                  <Film size={28} />
                </div>
              )}
              {item.voteAverage != null && item.voteAverage > 0 && (
                <span style={{
                  position: 'absolute', top: 6, right: 6,
                  background: 'rgba(0,0,0,0.75)', color: '#fbbf24',
                  fontSize: 11, fontWeight: 600, padding: '2px 6px', borderRadius: 4,
                }}>
                  {item.voteAverage.toFixed(1)}
                </span>
              )}
            </div>
            <div style={{ padding: '8px 10px' }}>
              <div style={{
                fontSize: 13, fontWeight: 600, color: '#e2e8f0',
                whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
              }}>
                {getTitle(item)}
              </div>
              <div style={{ fontSize: 11, color: '#94a3b8', marginTop: 2 }}>
                {getYear(item)}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
