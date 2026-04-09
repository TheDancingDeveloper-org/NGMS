import { useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { ArrowLeft, Play, Film } from 'lucide-react'
import WatchlistButton from '../components/WatchlistButton'
import RatingStars from '../components/RatingStars'
import TmdbRow from '../components/TmdbRow'
import type { TmdbDisplayItem } from '../components/TmdbRow'
import { DetailSkeleton } from '../components/Skeleton'
import { useMobile } from '../hooks/useMobile'
import { useMovieDetail, useMovieRecommendations } from '../hooks/useApi'

function formatSize(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}

export default function MovieView() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const isMobile = useMobile()
  const numId = Number(id) || 0
  const { data: movie, isLoading: loading } = useMovieDetail(numId)
  const tmdbId = movie?.tmdbId ?? 0
  const { data: recommendations } = useMovieRecommendations(tmdbId, tmdbId > 0)

  const handleRecommendationClick = useCallback((item: TmdbDisplayItem) => {
    const title = item.title || item.name || ''
    if (title) {
      navigate(`/discover?q=${encodeURIComponent(title)}&type=movie`)
    }
  }, [navigate])

  if (loading) return <DetailSkeleton />
  if (!movie) return <div style={{ color: '#ef4444', padding: 24 }}>Movie not found</div>

  const fanart = movie.fanartUrl
  const poster = movie.posterUrl

  return (
    <div>
      <button
        onClick={() => navigate('/movies')}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, background: 'none', border: 'none',
          color: '#94a3b8', cursor: 'pointer', fontSize: 14, marginBottom: 16, padding: 0,
        }}
      >
        <ArrowLeft size={16} /> Back to movies
      </button>

      {/* Hero */}
      <div style={{
        position: 'relative', borderRadius: 12, overflow: 'hidden',
        marginBottom: 24, background: '#1e293b', minHeight: 300,
      }}>
        {fanart && (
          <img src={fanart} alt="" style={{ width: '100%', height: isMobile ? 250 : 350, objectFit: 'cover', opacity: 0.35 }} />
        )}
        <div style={{
          position: 'absolute', bottom: 0, left: 0, right: 0,
          display: 'flex', gap: isMobile ? 12 : 24, padding: isMobile ? '20px 16px 16px' : '32px 24px 24px',
          background: 'linear-gradient(transparent, rgba(15, 23, 42, 0.97))',
        }}>
          {poster ? (
            <img
              src={poster}
              alt={movie.title}
              style={{ width: isMobile ? 80 : 120, borderRadius: 8, flexShrink: 0, boxShadow: '0 4px 12px rgba(0,0,0,0.5)' }}
            />
          ) : (
            <div style={{
              width: isMobile ? 80 : 120, height: isMobile ? 120 : 180, borderRadius: 8, background: '#0f172a',
              display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
            }}>
              <Film size={isMobile ? 28 : 36} color="#475569" />
            </div>
          )}
          <div style={{ flex: 1, minWidth: 0 }}>
            <h1 style={{ fontSize: isMobile ? 20 : 28, fontWeight: 700, color: '#fff', margin: 0 }}>{movie.title}</h1>
            <div style={{ display: 'flex', gap: 12, marginTop: 8, fontSize: 13, color: '#94a3b8', flexWrap: 'wrap' }}>
              {movie.year && <span>{movie.year}</span>}
              {movie.studio && <span>{movie.studio}</span>}
              {movie.minimumAvailability && (
                <span style={{ textTransform: 'capitalize' }}>{movie.minimumAvailability}</span>
              )}
            </div>
            {/* Genres */}
            {movie.genres && movie.genres.length > 0 && (
              <div style={{ display: 'flex', gap: 6, marginTop: 8, flexWrap: 'wrap' }}>
                {movie.genres.map((g) => (
                  <span key={g} style={{
                    padding: '2px 8px', borderRadius: 4, fontSize: 11,
                    background: '#334155', color: '#94a3b8',
                  }}>
                    {g}
                  </span>
                ))}
              </div>
            )}
            {movie.overview && (
              <p style={{ marginTop: 12, fontSize: 14, color: '#94a3b8', lineHeight: 1.5 }}>
                {movie.overview.length > 400 ? movie.overview.slice(0, 400) + '...' : movie.overview}
              </p>
            )}

            <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginTop: 16 }}>
              <WatchlistButton mediaType="movie" mediaId={movie.id} />
              <RatingStars mediaType="movie" mediaId={movie.id} />
            </div>

            {movie.hasFile && movie.movieFile ? (
              <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginTop: 20 }}>
                <button
                  onClick={() => {
                    navigate(`/play/${movie.movieFile!.id}`)
                  }}
                  style={{
                    display: 'flex', alignItems: 'center', gap: 8,
                    padding: '10px 24px', borderRadius: 8,
                    background: '#2563eb', border: 'none', color: '#fff',
                    cursor: 'pointer', fontSize: 15, fontWeight: 600,
                  }}
                >
                  <Play size={18} fill="#fff" /> Play Movie
                </button>
              </div>
            ) : (
              <div style={{
                marginTop: 20, padding: '10px 16px', borderRadius: 8,
                background: '#7f1d1d33', color: '#fca5a5', fontSize: 14,
              }}>
                No file available
              </div>
            )}
          </div>
        </div>
      </div>

      {/* File & release info */}
      <div style={{
        display: 'flex', gap: isMobile ? 12 : 24, marginBottom: 24, flexWrap: 'wrap',
      }}>
        {/* File info */}
        {movie.movieFile && (
          <div style={{
            background: '#1e293b', borderRadius: 10, border: '1px solid #334155',
            padding: '14px 18px', flex: '1 1 280px',
          }}>
            <h3 style={{ fontSize: 14, fontWeight: 600, color: '#e2e8f0', marginBottom: 10 }}>
              File Info
            </h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, fontSize: 13, color: '#94a3b8' }}>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: '#64748b' }}>Path</span>
                <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', maxWidth: '60%', textAlign: 'right' }}>
                  {movie.movieFile.relativePath}
                </span>
              </div>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <span style={{ color: '#64748b' }}>Size</span>
                <span>{formatSize(movie.movieFile.size)}</span>
              </div>
              {movie.movieFile.releaseGroup && (
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>Release Group</span>
                  <span>{movie.movieFile.releaseGroup}</span>
                </div>
              )}
              {movie.movieFile.edition && (
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>Edition</span>
                  <span>{movie.movieFile.edition}</span>
                </div>
              )}
            </div>
          </div>
        )}

        {/* Release dates */}
        {(movie.inCinemas || movie.physicalRelease || movie.digitalRelease) && (
          <div style={{
            background: '#1e293b', borderRadius: 10, border: '1px solid #334155',
            padding: '14px 18px', flex: '1 1 200px',
          }}>
            <h3 style={{ fontSize: 14, fontWeight: 600, color: '#e2e8f0', marginBottom: 10 }}>
              Release Dates
            </h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6, fontSize: 13, color: '#94a3b8' }}>
              {movie.inCinemas && (
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>In Cinemas</span>
                  <span>{movie.inCinemas}</span>
                </div>
              )}
              {movie.digitalRelease && (
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>Digital</span>
                  <span>{movie.digitalRelease}</span>
                </div>
              )}
              {movie.physicalRelease && (
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span style={{ color: '#64748b' }}>Physical</span>
                  <span>{movie.physicalRelease}</span>
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Recommendations */}
      {recommendations && recommendations.results.length > 0 && (
        <TmdbRow
          title="More Like This"
          items={recommendations.results}
          onItemClick={handleRecommendationClick}
        />
      )}
    </div>
  )
}
