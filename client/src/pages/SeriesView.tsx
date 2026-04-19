import { useState, useEffect, useCallback } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { ArrowLeft, Play, ChevronDown, ChevronRight } from 'lucide-react'
import WatchlistButton from '../components/WatchlistButton'
import RatingStars from '../components/RatingStars'
import TmdbRow from '../components/TmdbRow'
import type { TmdbDisplayItem } from '../components/TmdbRow'
import { DetailSkeleton } from '../components/Skeleton'
import { useMobile } from '../hooks/useMobile'
import { useSeriesDetail, useEpisodes, useTvRecommendations } from '../hooks/useApi'
import { imageUrl } from '../api'

function formatFileSize(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}

function RecommendationSection({ items, navigate }: {
  items: TmdbDisplayItem[]
  navigate: ReturnType<typeof useNavigate>
}) {
  const handleClick = useCallback((item: TmdbDisplayItem) => {
    const title = item.title || item.name || ''
    if (title) {
      navigate(`/discover?q=${encodeURIComponent(title)}&type=series`)
    }
  }, [navigate])

  return <TmdbRow title="More Like This" items={items} onItemClick={handleClick} />
}

export default function SeriesView() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const isMobile = useMobile()
  const numId = Number(id) || 0
  const { data: series, isLoading: seriesLoading } = useSeriesDetail(numId)
  const { data: episodes = [], isLoading: episodesLoading } = useEpisodes(numId)
  const tmdbId = series?.tmdbId ?? 0
  const { data: recommendations } = useTvRecommendations(tmdbId, tmdbId > 0)
  const loading = seriesLoading || episodesLoading
  const [expandedSeasons, setExpandedSeasons] = useState<Set<number>>(new Set())

  // Auto-expand seasons that have files once episodes load
  /* eslint-disable react-hooks/set-state-in-effect -- one-time init from fetched data */
  useEffect(() => {
    if (episodes.length > 0) {
      const seasonsWithFiles = new Set(
        episodes.filter((e) => e.episodeFile != null).map((e) => e.seasonNumber),
      )
      setExpandedSeasons(seasonsWithFiles)
    }
  }, [episodes])
  /* eslint-enable react-hooks/set-state-in-effect */

  if (loading) return <DetailSkeleton />
  if (!series) return <div style={{ color: '#ef4444', padding: 24 }}>Series not found</div>

  const seasons = [...new Set(episodes.map((e) => e.seasonNumber))].sort((a, b) => a - b)
  const fanart = imageUrl(series.fanartUrl)

  const toggleSeason = (s: number) => {
    const next = new Set(expandedSeasons)
    if (next.has(s)) { next.delete(s) } else { next.add(s) }
    setExpandedSeasons(next)
  }

  return (
    <div>
      <button
        onClick={() => navigate('/series')}
        style={{
          display: 'flex', alignItems: 'center', gap: 6, background: 'none', border: 'none',
          color: '#94a3b8', cursor: 'pointer', fontSize: 14, marginBottom: 16, padding: 0,
        }}
      >
        <ArrowLeft size={16} /> Back to series
      </button>

      {/* Hero banner */}
      <div style={{
        position: 'relative', borderRadius: 12, overflow: 'hidden',
        marginBottom: 24, background: '#1e293b', minHeight: 200,
      }}>
        {fanart && (
          <img src={fanart} alt="" style={{ width: '100%', height: isMobile ? 180 : 280, objectFit: 'cover', opacity: 0.4 }} />
        )}
        <div style={{
          position: 'absolute', bottom: 0, left: 0, right: 0,
          padding: isMobile ? '20px 16px 16px' : '32px 24px 24px',
          background: 'linear-gradient(transparent, rgba(15, 23, 42, 0.95))',
        }}>
          <h1 style={{ fontSize: isMobile ? 20 : 28, fontWeight: 700, color: '#fff', margin: 0 }}>{series.title}</h1>
          <div style={{ display: 'flex', gap: isMobile ? 8 : 12, marginTop: 8, fontSize: isMobile ? 12 : 13, color: '#94a3b8', flexWrap: 'wrap' }}>
            {series.year && <span>{series.year}</span>}
            {series.network && <span>{series.network}</span>}
            {series.runtime != null && <span>{series.runtime}m</span>}
            <span>{series.seasonCount} Seasons</span>
            <span>{series.episodeFileCount} / {series.totalEpisodeCount} Episodes</span>
            <span style={{ textTransform: 'capitalize' }}>{series.status}</span>
          </div>
          {/* Genres */}
          {series.genres && series.genres.length > 0 && (
            <div style={{ display: 'flex', gap: 6, marginTop: 8, flexWrap: 'wrap' }}>
              {series.genres.map((g) => (
                <span key={g} style={{
                  padding: '2px 8px', borderRadius: 4, fontSize: 11,
                  background: '#334155', color: '#94a3b8',
                }}>
                  {g}
                </span>
              ))}
            </div>
          )}
          {series.overview && (
            <p style={{ marginTop: 12, fontSize: 14, color: '#94a3b8', maxWidth: 700, lineHeight: 1.5 }}>
              {series.overview.length > 300 ? series.overview.slice(0, 300) + '...' : series.overview}
            </p>
          )}
          <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginTop: 16 }}>
            <WatchlistButton mediaType="series" mediaId={series.id} />
            <RatingStars mediaType="series" mediaId={series.id} />
          </div>
        </div>
      </div>

      {/* Recommendations */}
      {recommendations && recommendations.results.length > 0 && (
        <RecommendationSection items={recommendations.results} navigate={navigate} />
      )}

      {/* Season accordion */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {seasons.map((seasonNum) => {
          const seasonEps = episodes
            .filter((e) => e.seasonNumber === seasonNum)
            .sort((a, b) => a.episodeNumber - b.episodeNumber)
          const expanded = expandedSeasons.has(seasonNum)
          const fileCount = seasonEps.filter((e) => e.episodeFile != null).length

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
                        gap: 8,
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: 12, flex: 1, minWidth: 0 }}>
                        <span style={{
                          fontSize: 13, color: '#64748b', fontVariantNumeric: 'tabular-nums', minWidth: 30,
                        }}>
                          {ep.episodeNumber}
                        </span>
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <span style={{
                            fontSize: 14, display: 'block',
                            color: ep.episodeFile?.id != null ? '#f1f5f9' : '#64748b',
                            overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                          }}>
                            {ep.title || 'TBA'}
                          </span>
                          <div style={{ display: 'flex', gap: 8, fontSize: 11, color: '#64748b', marginTop: 2 }}>
                            {ep.airDate && <span>{ep.airDate}</span>}
                            {ep.runtime != null && <span>{ep.runtime}m</span>}
                            {ep.episodeFile && (
                              <span style={{ color: '#4ade80' }}>
                                {formatFileSize(ep.episodeFile.size)}
                              </span>
                            )}
                          </div>
                        </div>
                      </div>

                      {ep.episodeFile?.id != null && (
                        <button
                          onClick={() => {
                            navigate(`/play/${ep.episodeFile?.id}`, { state: { seriesId: Number(id), episodeId: ep.id } })
                          }}
                          style={{
                            display: 'flex', alignItems: 'center', gap: 4,
                            padding: '6px 12px', borderRadius: 6,
                            background: '#2563eb', border: 'none', color: '#fff',
                            cursor: 'pointer', fontSize: 13, fontWeight: 500, flexShrink: 0,
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
