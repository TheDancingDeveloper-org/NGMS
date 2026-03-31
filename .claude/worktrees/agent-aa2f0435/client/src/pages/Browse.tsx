import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Search, Play, Film } from 'lucide-react'
import { api, imageUrl, type Series, type Movie } from '../api'

function PosterCard({
  title,
  year,
  poster,
  hasFile,
  onClick,
}: {
  title: string
  year: number | null
  poster: string | null
  hasFile: boolean
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      style={{
        background: '#1e293b',
        border: '1px solid #334155',
        borderRadius: 12,
        overflow: 'hidden',
        cursor: 'pointer',
        textAlign: 'left',
        transition: 'transform 0.15s, border-color 0.15s',
        width: '100%',
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.transform = 'scale(1.03)'
        e.currentTarget.style.borderColor = '#3b82f6'
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.transform = 'scale(1)'
        e.currentTarget.style.borderColor = '#334155'
      }}
    >
      <div style={{ aspectRatio: '2/3', background: '#0f172a', position: 'relative' }}>
        {poster ? (
          <img
            src={poster}
            alt={title}
            style={{ width: '100%', height: '100%', objectFit: 'cover' }}
            loading="lazy"
          />
        ) : (
          <div style={{
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            height: '100%', color: '#475569',
          }}>
            <Film size={48} />
          </div>
        )}
        {hasFile && (
          <div style={{
            position: 'absolute', top: 8, right: 8,
            background: 'rgba(34, 197, 94, 0.9)', borderRadius: '50%', padding: 4,
          }}>
            <Play size={14} color="#fff" fill="#fff" />
          </div>
        )}
      </div>
      <div style={{ padding: '10px 12px' }}>
        <div style={{
          fontSize: 13, fontWeight: 600, color: '#f1f5f9',
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }}>
          {title}
        </div>
        {year && <div style={{ fontSize: 12, color: '#64748b', marginTop: 2 }}>{year}</div>}
      </div>
    </button>
  )
}

export default function Browse({ mode }: { mode: 'series' | 'movies' }) {
  const navigate = useNavigate()
  const [series, setSeries] = useState<Series[]>([])
  const [movies, setMovies] = useState<Movie[]>([])
  const [loading, setLoading] = useState(true)
  const [filter, setFilter] = useState('')

  useEffect(() => {
    setLoading(true)
    setFilter('')
    if (mode === 'series') {
      api.listSeries().then(setSeries).catch(() => setSeries([])).finally(() => setLoading(false))
    } else {
      api.listMovies().then(setMovies).catch(() => setMovies([])).finally(() => setLoading(false))
    }
  }, [mode])

  const lf = filter.toLowerCase()

  const filteredSeries = series.filter((s) => s.title.toLowerCase().includes(lf))
  const filteredMovies = movies.filter((m) => m.title.toLowerCase().includes(lf))
  const count = mode === 'series' ? filteredSeries.length : filteredMovies.length

  return (
    <div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 24 }}>
        <div style={{ display: 'flex', alignItems: 'center', position: 'relative', flex: 1, maxWidth: 400 }}>
          <Search size={16} style={{ position: 'absolute', left: 12, color: '#64748b' }} />
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder={`Search ${mode}...`}
            style={{
              width: '100%', padding: '10px 12px 10px 36px',
              background: '#1e293b', border: '1px solid #334155', borderRadius: 8,
              color: '#f1f5f9', fontSize: 14, outline: 'none',
            }}
          />
        </div>
        <span style={{ fontSize: 13, color: '#64748b' }}>{count} {mode}</span>
      </div>

      {loading ? (
        <div style={{ textAlign: 'center', padding: 48, color: '#64748b' }}>Loading...</div>
      ) : count === 0 ? (
        <div style={{ textAlign: 'center', padding: 48, color: '#64748b' }}>
          <Film size={48} style={{ marginBottom: 12, opacity: 0.3 }} />
          <p>No {mode} found</p>
        </div>
      ) : (
        <div style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fill, minmax(160px, 1fr))',
          gap: 16,
        }}>
          {mode === 'series'
            ? filteredSeries.map((s) => (
                <PosterCard
                  key={s.id}
                  title={s.title}
                  year={s.year}
                  poster={imageUrl(s.images, 'poster')}
                  hasFile={false}
                  onClick={() => navigate(`/series/${s.id}`)}
                />
              ))
            : filteredMovies.map((m) => (
                <PosterCard
                  key={m.id}
                  title={m.title}
                  year={m.year}
                  poster={imageUrl(m.images, 'poster')}
                  hasFile={m.movieFileId != null}
                  onClick={() => navigate(`/movie/${m.id}`)}
                />
              ))}
        </div>
      )}
    </div>
  )
}
