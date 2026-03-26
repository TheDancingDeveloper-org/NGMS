import { useState, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { ArrowLeft, Play, ChevronDown, ChevronRight } from 'lucide-react'
import { api, imageUrl, type Series, type Episode } from '../api'

export default function SeriesView() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const [series, setSeries] = useState<Series | null>(null)
  const [episodes, setEpisodes] = useState<Episode[]>([])
  const [loading, setLoading] = useState(true)
  const [expandedSeasons, setExpandedSeasons] = useState<Set<number>>(new Set())

  useEffect(() => {
    if (!id) return
    setLoading(true)
    Promise.all([api.getSeries(Number(id)), api.getEpisodes(Number(id))])
      .then(([s, eps]) => {
        setSeries(s)
        setEpisodes(eps)
        const seasonsWithFiles = new Set(
          eps.filter((e) => e.episodeFileId != null).map((e) => e.seasonNumber),
        )
        setExpandedSeasons(seasonsWithFiles)
      })
      .catch(() => {})
      .finally(() => setLoading(false))
  }, [id])

  if (loading) return <div style={{ textAlign: 'center', padding: 48, color: '#64748b' }}>Loading...</div>
  if (!series) return <div style={{ color: '#ef4444', padding: 24 }}>Series not found</div>

  const seasons = [...new Set(episodes.map((e) => e.seasonNumber))].sort((a, b) => a - b)
  const fanart = imageUrl(series.images, 'fanart')

  const toggleSeason = (s: number) => {
    const next = new Set(expandedSeasons)
    next.has(s) ? next.delete(s) : next.add(s)
    setExpandedSeasons(next)
  }

  return (
    <div>
      <button
        onClick={() => navigate('/')}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, background: 'none', border: 'none',
          color: '#94a3b8', cursor: 'pointer', fontSize: 14, marginBottom: 16, padding: 0,
        }}
      >
        <ArrowLeft size={16} /> Back to library
      </button>

      {/* Hero banner */}
      <div style={{
        position: 'relative', borderRadius: 12, overflow: 'hidden',
        marginBottom: 24, background: '#1e293b', minHeight: 200,
      }}>
        {fanart && (
          <img src={fanart} alt="" style={{ width: '100%', height: 280, objectFit: 'cover', opacity: 0.4 }} />
        )}
        <div style={{
          position: 'absolute', bottom: 0, left: 0, right: 0,
          padding: '32px 24px 24px',
          background: 'linear-gradient(transparent, rgba(15, 23, 42, 0.95))',
        }}>
          <h1 style={{ fontSize: 28, fontWeight: 700, color: '#fff', margin: 0 }}>{series.title}</h1>
          <div style={{ display: 'flex', gap: 16, marginTop: 8, fontSize: 13, color: '#94a3b8' }}>
            {series.year && <span>{series.year}</span>}
            <span>{seasons.length} Seasons</span>
            <span>{episodes.filter((e) => e.episodeFileId != null).length} / {episodes.length} Episodes</span>
            <span style={{ textTransform: 'capitalize' }}>{series.status}</span>
          </div>
          {series.overview && (
            <p style={{ marginTop: 12, fontSize: 14, color: '#94a3b8', maxWidth: 700, lineHeight: 1.5 }}>
              {series.overview.length > 300 ? series.overview.slice(0, 300) + '...' : series.overview}
            </p>
          )}
        </div>
      </div>

      {/* Season accordion */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {seasons.map((seasonNum) => {
          const seasonEps = episodes
            .filter((e) => e.seasonNumber === seasonNum)
            .sort((a, b) => a.episodeNumber - b.episodeNumber)
          const expanded = expandedSeasons.has(seasonNum)
          const fileCount = seasonEps.filter((e) => e.episodeFileId != null).length

          return (
            <div key={seasonNum} style={{ background: '#1e293b', borderRadius: 10, border: '1px solid #334155' }}>
              <button
                onClick={() => toggleSeason(seasonNum)}
                style={{
                  display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                  width: '100%', padding: '14px 16px', background: 'none', border: 'none',
                  color: '#f1f5f9', cursor: 'pointer', fontSize: 15, fontWeight: 600,
                }}
              >
                <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                  {expanded ? <ChevronDown size={18} /> : <ChevronRight size={18} />}
                  Season {seasonNum}
                </span>
                <span style={{ fontSize: 12, color: '#64748b', fontWeight: 400 }}>
                  {fileCount} / {seasonEps.length} episodes
                </span>
              </button>

              {expanded && (
                <div style={{ borderTop: '1px solid #334155' }}>
                  {seasonEps.map((ep) => (
                    <div
                      key={ep.id}
                      style={{
                        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                        padding: '10px 16px', borderBottom: '1px solid #1e293b',
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: 12, flex: 1, minWidth: 0 }}>
                        <span style={{
                          fontSize: 13, color: '#64748b', fontVariantNumeric: 'tabular-nums', minWidth: 30,
                        }}>
                          {ep.episodeNumber}
                        </span>
                        <span style={{
                          fontSize: 14,
                          color: ep.episodeFileId != null ? '#f1f5f9' : '#64748b',
                          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                        }}>
                          {ep.title || 'TBA'}
                        </span>
                      </div>

                      {ep.episodeFileId != null && (
                        <button
                          onClick={() => navigate(`/play/${ep.episodeFileId}`)}
                          style={{
                            display: 'flex', alignItems: 'center', gap: 4,
                            padding: '6px 12px', borderRadius: 6,
                            background: '#2563eb', border: 'none', color: '#fff',
                            cursor: 'pointer', fontSize: 13, fontWeight: 500,
                          }}
                        >
                          <Play size={14} fill="#fff" /> Play
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
