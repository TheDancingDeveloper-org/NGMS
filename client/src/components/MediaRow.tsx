import { useNavigate } from 'react-router-dom'
import type { ContinueWatchingItem } from '../api'
import { useMobile } from '../hooks/useMobile'

interface MediaRowProps {
  title: string
  items: ContinueWatchingItem[]
}

function formatProgress(item: ContinueWatchingItem): string {
  const pct = item.durationSecs > 0
    ? Math.round((item.positionSecs / item.durationSecs) * 100)
    : 0
  return `${pct}%`
}

function formatEpisode(item: ContinueWatchingItem): string {
  if (item.mediaType === 'series' && item.seasonNumber != null && item.episodeNumber != null) {
    const ep = `S${String(item.seasonNumber).padStart(2, '0')}E${String(item.episodeNumber).padStart(2, '0')}`
    return item.episodeTitle ? `${ep} - ${item.episodeTitle}` : ep
  }
  return ''
}

function posterSrc(item: ContinueWatchingItem): string | undefined {
  if (item.posterUrl) return item.posterUrl
  return undefined
}

export default function MediaRow({ title, items }: MediaRowProps) {
  const navigate = useNavigate()
  const isMobile = useMobile()

  if (items.length === 0) return null

  return (
    <div style={{ marginBottom: isMobile ? 20 : 32 }}>
      <style>{`.media-card { transition: transform 0.15s, box-shadow 0.15s; } .media-card:hover { transform: scale(1.04); box-shadow: 0 4px 20px rgba(0,0,0,0.4); }`}</style>
      <h2 style={{
        fontSize: isMobile ? 16 : 18, fontWeight: 600, color: '#e2e8f0',
        marginBottom: isMobile ? 8 : 12, paddingLeft: 4,
      }}>
        {title}
      </h2>
      <div style={{
        display: 'flex',
        gap: isMobile ? 10 : 16,
        overflowX: 'auto',
        paddingBottom: 8,
        scrollbarWidth: 'thin',
        WebkitOverflowScrolling: 'touch',
      }}>
        {items.map((item) => {
          const progressPct = item.durationSecs > 0
            ? (item.positionSecs / item.durationSecs) * 100
            : 0
          const hasProgress = item.positionSecs > 0 || item.durationSecs > 0

          return (
            <div
              key={item.id || `${item.mediaType}-${item.mediaId}`}
              className="media-card"
              onClick={() => {
                if (!item.mediaFileId) {
                  navigate(item.mediaType === 'series' ? `/series/${item.mediaId}` : `/movie/${item.mediaId}`)
                } else {
                  navigate(`/play/${item.mediaFileId}`)
                }
              }}
              style={{
                flex: isMobile ? '0 0 120px' : '0 0 160px',
                cursor: 'pointer',
                borderRadius: 10,
                overflow: 'hidden',
                background: '#1e293b',
              }}
            >
              {/* Poster */}
              <div style={{
                width: isMobile ? 120 : 160,
                height: isMobile ? 180 : 240,
                background: '#334155',
                position: 'relative',
              }}>
                {posterSrc(item) ? (
                  <img
                    src={posterSrc(item)}
                    alt={item.title || ''}
                    style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                    loading="lazy"
                  />
                ) : (
                  <div style={{
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    height: '100%', color: '#64748b', fontSize: 13, padding: 12,
                    textAlign: 'center',
                  }}>
                    {item.title || 'Unknown'}
                  </div>
                )}
                {/* Progress bar overlay — only shown for items with watch progress */}
                {progressPct > 0 && (
                  <div style={{
                    position: 'absolute', bottom: 0, left: 0, right: 0,
                    height: 4, background: 'rgba(0,0,0,0.5)',
                  }}>
                    <div style={{
                      height: '100%',
                      width: `${Math.min(progressPct, 100)}%`,
                      background: '#3b82f6',
                      borderRadius: '0 2px 2px 0',
                      transition: 'width 0.3s',
                    }} />
                  </div>
                )}
              </div>

              {/* Info */}
              <div style={{ padding: '8px 10px' }}>
                <div style={{
                  fontSize: 13, fontWeight: 600, color: '#e2e8f0',
                  whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
                }}>
                  {item.title || 'Unknown'}
                </div>
                {item.mediaType === 'series' && (
                  <div style={{
                    fontSize: 11, color: '#94a3b8', marginTop: 2,
                    whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
                  }}>
                    {formatEpisode(item)}
                  </div>
                )}
                {hasProgress && (
                  <div style={{ fontSize: 11, color: '#64748b', marginTop: 2 }}>
                    {formatProgress(item)} watched
                  </div>
                )}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}
