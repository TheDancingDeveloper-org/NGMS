import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { ArrowLeft, Play, Film } from 'lucide-react'
import { api, imageUrl, type Movie } from '../api'
import WatchlistButton from '../components/WatchlistButton'
import RatingStars from '../components/RatingStars'

export default function MovieView() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [movie, setMovie] = useState<Movie | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    if (!id) return
    setLoading(true)
    api.getMovie(Number(id))
      .then(setMovie)
      .catch(() => {})
      .finally(() => setLoading(false))
  }, [id])

  if (loading) return <div style={{ textAlign: 'center', padding: 48, color: '#64748b' }}>Loading...</div>
  if (!movie) return <div style={{ color: '#ef4444', padding: 24 }}>Movie not found</div>

  const fanart = imageUrl(movie.images, 'fanart')
  const poster = imageUrl(movie.images, 'poster')

  return (
    <div>
      <button
        onClick={() => navigate('/movies')}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, background: 'none', border: 'none',
          color: '#94a3b8', cursor: 'pointer', fontSize: 14, marginBottom: 16, padding: 0,
        }}
      >
        <ArrowLeft size={16} /> Back to library
      </button>

      {/* Hero */}
      <div style={{
        position: 'relative', borderRadius: 12, overflow: 'hidden',
        marginBottom: 24, background: '#1e293b', minHeight: 300,
      }}>
        {fanart && (
          <img src={fanart} alt="" style={{ width: '100%', height: 350, objectFit: 'cover', opacity: 0.35 }} />
        )}
        <div style={{
          position: 'absolute', bottom: 0, left: 0, right: 0,
          display: 'flex', gap: 24, padding: '32px 24px 24px',
          background: 'linear-gradient(transparent, rgba(15, 23, 42, 0.97))',
        }}>
          {poster ? (
            <img
              src={poster}
              alt={movie.title}
              style={{ width: 120, borderRadius: 8, flexShrink: 0, boxShadow: '0 4px 12px rgba(0,0,0,0.5)' }}
            />
          ) : (
            <div style={{
              width: 120, height: 180, borderRadius: 8, background: '#0f172a',
              display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
            }}>
              <Film size={36} color="#475569" />
            </div>
          )}
          <div style={{ flex: 1, minWidth: 0 }}>
            <h1 style={{ fontSize: 28, fontWeight: 700, color: '#fff', margin: 0 }}>{movie.title}</h1>
            <div style={{ display: 'flex', gap: 16, marginTop: 8, fontSize: 13, color: '#94a3b8' }}>
              {movie.year && <span>{movie.year}</span>}
              {movie.studio && <span>{movie.studio}</span>}
            </div>
            {movie.overview && (
              <p style={{ marginTop: 12, fontSize: 14, color: '#94a3b8', lineHeight: 1.5 }}>
                {movie.overview.length > 400 ? movie.overview.slice(0, 400) + '...' : movie.overview}
              </p>
            )}

            <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginTop: 16 }}>
              <WatchlistButton mediaType="movie" mediaId={movie.id} />
              <RatingStars mediaType="movie" mediaId={movie.id} />
            </div>

            {movie.movieFileId != null ? (
              <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginTop: 20 }}>
                <button
                  onClick={() => {
                    console.log('[MovieView] Play movie:', { movieId: movie.id, movieFileId: movie.movieFileId, navigateTo: `/play/${movie.movieFileId}` })
                    navigate(`/play/${movie.movieFileId}`)
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
    </div>
  )
}
