import { useState, useEffect, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import { Search, Play, Film } from 'lucide-react'
import { imageUrl } from '../api'
import { useMobile } from '../hooks/useMobile'
import { useSeries, useMovies } from '../hooks/useApi'
import { PosterSkeleton } from '../components/Skeleton'

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
      className="poster-card"
      onClick={onClick}
      style={{
        background: '#1e293b',
        border: '1px solid #334155',
        borderRadius: 12,
        overflow: 'hidden',
        cursor: 'pointer',
        textAlign: 'left',
        width: '100%',
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

const selectStyle: React.CSSProperties = {
  padding: '10px 12px',
  background: '#1e293b',
  border: '1px solid #334155',
  borderRadius: 8,
  color: '#f1f5f9',
  fontSize: 14,
}

export default function Browse({ mode }: { mode: 'series' | 'movies' }) {
  const navigate = useNavigate()
  const isMobile = useMobile()
  const { data: series = [], isLoading: seriesLoading } = useSeries()
  const { data: movies = [], isLoading: moviesLoading } = useMovies()
  const [filter, setFilter] = useState('')
  const [yearFilter, setYearFilter] = useState('')
  const [genreFilter, setGenreFilter] = useState('')
  const [statusFilter, setStatusFilter] = useState('')

  const loading = mode === 'series' ? seriesLoading : moviesLoading

  // Reset filters when mode changes
  /* eslint-disable react-hooks/set-state-in-effect -- intentional reset on prop change */
  useEffect(() => {
    setFilter('')
    setYearFilter('')
    setGenreFilter('')
    setStatusFilter('')
  }, [mode])
  /* eslint-enable react-hooks/set-state-in-effect */

  // Scroll restoration: restore position after data loads
  useEffect(() => {
    if (!loading) {
      const saved = sessionStorage.getItem(`browse-scroll-${mode}`)
      if (saved) {
        window.scrollTo(0, parseInt(saved, 10))
      }
    }
  }, [loading, mode])

  // Scroll restoration: debounce-save scroll position
  useEffect(() => {
    let timeout: number
    const handler = () => {
      clearTimeout(timeout)
      timeout = window.setTimeout(() => {
        sessionStorage.setItem(`browse-scroll-${mode}`, String(window.scrollY))
      }, 100)
    }
    window.addEventListener('scroll', handler)
    return () => {
      window.removeEventListener('scroll', handler)
      clearTimeout(timeout)
    }
  }, [mode])

  // Extract unique years and genres from loaded data for filter dropdowns
  const { years, genres, statuses } = useMemo(() => {
    const items: Array<{ year: number | null; genres: string[] | null; status?: string }> =
      mode === 'series'
        ? series.map((s) => ({ year: s.year, genres: s.genres, status: s.status }))
        : movies.map((m) => ({ year: m.year, genres: m.genres }))

    const yearSet = new Set<number>()
    const genreSet = new Set<string>()
    const statusSet = new Set<string>()

    for (const item of items) {
      if (item.year != null) yearSet.add(item.year)
      if (item.genres) {
        for (const g of item.genres) genreSet.add(g)
      }
      if (item.status) statusSet.add(item.status)
    }

    return {
      years: Array.from(yearSet).sort((a, b) => b - a),
      genres: Array.from(genreSet).sort((a, b) => a.localeCompare(b)),
      statuses: Array.from(statusSet).sort(),
    }
  }, [mode, series, movies])

  const lf = filter.toLowerCase()

  const filteredSeries = series.filter((s) => {
    if (!s.title.toLowerCase().includes(lf)) return false
    if (yearFilter && String(s.year) !== yearFilter) return false
    if (genreFilter && !(s.genres ?? []).includes(genreFilter)) return false
    if (statusFilter && s.status !== statusFilter) return false
    return true
  })

  const filteredMovies = movies.filter((m) => {
    if (!m.title.toLowerCase().includes(lf)) return false
    if (yearFilter && String(m.year) !== yearFilter) return false
    if (genreFilter && !(m.genres ?? []).includes(genreFilter)) return false
    return true
  })

  const count = mode === 'series' ? filteredSeries.length : filteredMovies.length

  return (
    <div>
      <style>{`
        .poster-card { transition: transform 0.15s, border-color 0.15s; }
        .poster-card:hover { transform: scale(1.03); border-color: #3b82f6; }
      `}</style>

      <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 12 }}>
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
        {!loading && <span style={{ fontSize: 13, color: '#64748b' }}>{count} {mode}</span>}
      </div>

      <div style={{
        display: 'flex', flexWrap: 'wrap', gap: 8, marginBottom: 24,
      }}>
        <select
          value={yearFilter}
          onChange={(e) => setYearFilter(e.target.value)}
          style={selectStyle}
        >
          <option value="">All Years</option>
          {years.map((y) => (
            <option key={y} value={String(y)}>{y}</option>
          ))}
        </select>

        <select
          value={genreFilter}
          onChange={(e) => setGenreFilter(e.target.value)}
          style={selectStyle}
        >
          <option value="">All Genres</option>
          {genres.map((g) => (
            <option key={g} value={g}>{g}</option>
          ))}
        </select>

        {mode === 'series' && (
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value)}
            style={selectStyle}
          >
            <option value="">All Statuses</option>
            {statuses.map((s) => (
              <option key={s} value={s}>{s.charAt(0).toUpperCase() + s.slice(1)}</option>
            ))}
          </select>
        )}
      </div>

      {loading ? (
        <PosterSkeleton isMobile={isMobile} />
      ) : count === 0 ? (
        <div style={{ textAlign: 'center', padding: 48, color: '#64748b' }}>
          <Film size={48} style={{ marginBottom: 12, opacity: 0.3 }} />
          <p>No {mode} found</p>
        </div>
      ) : (
        <div style={{
          display: 'grid',
          gridTemplateColumns: isMobile
            ? 'repeat(auto-fill, minmax(110px, 1fr))'
            : 'repeat(auto-fill, minmax(160px, 1fr))',
          gap: isMobile ? 10 : 16,
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
