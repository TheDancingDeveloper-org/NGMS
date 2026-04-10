import { useMemo, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ChevronLeft, ChevronRight, Check, Clock } from 'lucide-react'
import { imageUrl, type CalendarEpisode } from '../api'
import { ListSkeleton } from '../components/Skeleton'
import { useMobile } from '../hooks/useMobile'
import { useCalendar } from '../hooks/useApi'

function formatDate(dateStr: string): string {
  const d = new Date(dateStr)
  return d.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' })
}

function formatTime(dateStr: string): string {
  const d = new Date(dateStr)
  return d.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
}

function dateKey(dateStr: string): string {
  return new Date(dateStr).toISOString().substring(0, 10)
}

function isToday(key: string): boolean {
  return key === new Date().toISOString().substring(0, 10)
}

function startOfWeek(date: Date): Date {
  const d = new Date(date)
  const day = d.getDay()
  d.setDate(d.getDate() - day)
  d.setHours(0, 0, 0, 0)
  return d
}

function formatRange(start: Date): string {
  const end = new Date(start)
  end.setDate(end.getDate() + 13)
  const fmt = (d: Date) => d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
  return `${fmt(start)} – ${fmt(end)}`
}

export default function CalendarPage() {
  const navigate = useNavigate()
  const isMobile = useMobile()
  const [weekOffset, setWeekOffset] = useState(0)

  const baseDate = useMemo(() => {
    const d = startOfWeek(new Date())
    d.setDate(d.getDate() + weekOffset * 7)
    return d
  }, [weekOffset])

  const start = baseDate.toISOString().substring(0, 10)
  const endDate = new Date(baseDate)
  endDate.setDate(endDate.getDate() + 13)
  const end = endDate.toISOString().substring(0, 10)

  const { data: episodes = [], isLoading } = useCalendar(start, end)

  // Group by date
  const grouped = useMemo(() => {
    const map = new Map<string, CalendarEpisode[]>()
    for (const ep of episodes) {
      if (!ep.airDateUtc) continue
      const key = dateKey(ep.airDateUtc)
      if (!map.has(key)) map.set(key, [])
      map.get(key)!.push(ep)
    }
    // Sort each group by time
    for (const [, eps] of map) {
      eps.sort((a, b) => {
        const ta = a.airDateUtc ? new Date(a.airDateUtc).getTime() : 0
        const tb = b.airDateUtc ? new Date(b.airDateUtc).getTime() : 0
        return ta - tb
      })
    }
    // Return sorted keys
    const keys = [...map.keys()].sort()
    return keys.map((k) => ({ date: k, episodes: map.get(k)! }))
  }, [episodes])

  return (
    <div>
      {/* Header with navigation */}
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        marginBottom: 24,
      }}>
        <h2 style={{ color: '#f1f5f9', fontSize: 20, fontWeight: 700, margin: 0 }}>
          Calendar
        </h2>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <button
            onClick={() => setWeekOffset((w) => w - 1)}
            style={{
              display: 'flex', padding: 6, borderRadius: 6, background: '#1e293b',
              border: '1px solid #334155', color: '#94a3b8', cursor: 'pointer',
            }}
          >
            <ChevronLeft size={18} />
          </button>
          <span style={{ fontSize: 13, color: '#94a3b8', minWidth: isMobile ? 130 : 160, textAlign: 'center' }}>
            {formatRange(baseDate)}
          </span>
          <button
            onClick={() => setWeekOffset((w) => w + 1)}
            style={{
              display: 'flex', padding: 6, borderRadius: 6, background: '#1e293b',
              border: '1px solid #334155', color: '#94a3b8', cursor: 'pointer',
            }}
          >
            <ChevronRight size={18} />
          </button>
          {weekOffset !== 0 && (
            <button
              onClick={() => setWeekOffset(0)}
              style={{
                padding: '4px 10px', borderRadius: 6, fontSize: 12,
                background: '#334155', border: 'none', color: '#94a3b8', cursor: 'pointer',
              }}
            >
              Today
            </button>
          )}
        </div>
      </div>

      {isLoading && <ListSkeleton count={8} />}

      {!isLoading && grouped.length === 0 && (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 60 }}>
          <Clock size={48} style={{ marginBottom: 12, opacity: 0.3 }} />
          <p>No upcoming episodes in this period.</p>
        </div>
      )}

      {/* Day groups */}
      {grouped.map(({ date, episodes: dayEps }) => (
        <div key={date} style={{ marginBottom: 20 }}>
          <div style={{
            fontSize: 14, fontWeight: 600,
            color: isToday(date) ? '#3b82f6' : '#e2e8f0',
            marginBottom: 8, paddingLeft: 4,
            display: 'flex', alignItems: 'center', gap: 8,
          }}>
            {formatDate(date + 'T00:00:00Z')}
            {isToday(date) && (
              <span style={{
                fontSize: 10, padding: '1px 6px', borderRadius: 4,
                background: '#1e40af', color: '#93c5fd', fontWeight: 700,
              }}>
                TODAY
              </span>
            )}
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {dayEps.map((ep) => (
              <div
                key={ep.episodeId}
                onClick={() => navigate(`/series/${ep.seriesId}`)}
                style={{
                  display: 'flex', alignItems: 'center', gap: 12,
                  padding: '10px 14px', borderRadius: 10,
                  background: '#1e293b', border: '1px solid #334155',
                  cursor: 'pointer',
                  transition: 'border-color 0.15s',
                }}
                onMouseEnter={(e) => (e.currentTarget.style.borderColor = '#3b82f6')}
                onMouseLeave={(e) => (e.currentTarget.style.borderColor = '#334155')}
              >
                {/* Poster thumbnail */}
                {ep.posterUrl ? (
                  <img
                    src={imageUrl(ep.posterUrl)}
                    alt=""
                    style={{ width: 36, height: 54, borderRadius: 4, objectFit: 'cover', flexShrink: 0 }}
                    loading="lazy"
                  />
                ) : (
                  <div style={{
                    width: 36, height: 54, borderRadius: 4, background: '#334155', flexShrink: 0,
                  }} />
                )}

                {/* Info */}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{
                    fontSize: 14, fontWeight: 600, color: '#f1f5f9',
                    overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                  }}>
                    {ep.seriesTitle}
                  </div>
                  <div style={{ fontSize: 12, color: '#94a3b8', marginTop: 2 }}>
                    S{String(ep.seasonNumber).padStart(2, '0')}E{String(ep.episodeNumber).padStart(2, '0')}
                    {ep.episodeTitle && ` · ${ep.episodeTitle}`}
                  </div>
                  {ep.airDateUtc && (
                    <div style={{ fontSize: 11, color: '#64748b', marginTop: 2 }}>
                      {formatTime(ep.airDateUtc)}
                    </div>
                  )}
                </div>

                {/* Status */}
                {ep.hasFile ? (
                  <span style={{
                    display: 'flex', alignItems: 'center', gap: 4,
                    padding: '3px 8px', borderRadius: 4, fontSize: 11, fontWeight: 600,
                    background: '#166534', color: '#4ade80', flexShrink: 0,
                  }}>
                    <Check size={12} /> Downloaded
                  </span>
                ) : ep.monitored ? (
                  <span style={{
                    display: 'flex', alignItems: 'center', gap: 4,
                    padding: '3px 8px', borderRadius: 4, fontSize: 11, fontWeight: 600,
                    background: '#1e40af33', color: '#60a5fa', flexShrink: 0,
                  }}>
                    <Clock size={12} /> Monitored
                  </span>
                ) : (
                  <span style={{
                    padding: '3px 8px', borderRadius: 4, fontSize: 11,
                    color: '#64748b', flexShrink: 0,
                  }}>
                    Unmonitored
                  </span>
                )}
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  )
}
