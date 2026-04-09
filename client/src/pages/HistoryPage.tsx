import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { ChevronLeft, ChevronRight, Download, Upload, Search, Trash2, RotateCcw, Clock } from 'lucide-react'
import type { HistoryEvent } from '../api'
import { useMobile } from '../hooks/useMobile'
import { useHistory } from '../hooks/useApi'

function eventIcon(type: string) {
  switch (type) {
    case 'grabbed': return <Download size={14} />
    case 'downloadFolderImported':
    case 'downloadImported': return <Upload size={14} />
    case 'episodeFileDeleted':
    case 'movieFileDeleted': return <Trash2 size={14} />
    case 'episodeFileRenamed':
    case 'movieFileRenamed': return <RotateCcw size={14} />
    default: return <Clock size={14} />
  }
}

function eventLabel(type: string): string {
  switch (type) {
    case 'grabbed': return 'Grabbed'
    case 'downloadFolderImported':
    case 'downloadImported': return 'Imported'
    case 'episodeFileDeleted':
    case 'movieFileDeleted': return 'Deleted'
    case 'episodeFileRenamed':
    case 'movieFileRenamed': return 'Renamed'
    case 'seriesAdded': return 'Series Added'
    case 'movieAdded': return 'Movie Added'
    default: return type
  }
}

function eventColor(type: string): { bg: string; fg: string } {
  switch (type) {
    case 'grabbed': return { bg: '#1e40af33', fg: '#60a5fa' }
    case 'downloadFolderImported':
    case 'downloadImported': return { bg: '#16653433', fg: '#4ade80' }
    case 'episodeFileDeleted':
    case 'movieFileDeleted': return { bg: '#991b1b33', fg: '#f87171' }
    case 'episodeFileRenamed':
    case 'movieFileRenamed': return { bg: '#854d0e33', fg: '#fbbf24' }
    default: return { bg: '#33415533', fg: '#94a3b8' }
  }
}

function relativeTime(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime()
  const mins = Math.floor(diff / 60_000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 7) return `${days}d ago`
  return new Date(dateStr).toLocaleDateString()
}

function HistoryRow({ event, onNavigate }: { event: HistoryEvent; onNavigate: () => void }) {
  const isMobile = useMobile()
  const ec = eventColor(event.eventType)

  return (
    <div
      onClick={onNavigate}
      style={{
        display: 'flex', alignItems: 'center', gap: isMobile ? 10 : 14,
        padding: '10px 14px', borderRadius: 8,
        background: '#1e293b', border: '1px solid #334155',
        cursor: 'pointer', transition: 'border-color 0.15s',
      }}
      onMouseEnter={(e) => (e.currentTarget.style.borderColor = '#3b82f6')}
      onMouseLeave={(e) => (e.currentTarget.style.borderColor = '#334155')}
    >
      {/* Event type badge */}
      <span style={{
        display: 'flex', alignItems: 'center', gap: 4,
        padding: '3px 8px', borderRadius: 4, fontSize: 11, fontWeight: 600,
        background: ec.bg, color: ec.fg, flexShrink: 0, minWidth: 80, justifyContent: 'center',
      }}>
        {eventIcon(event.eventType)} {eventLabel(event.eventType)}
      </span>

      {/* Title */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{
          fontSize: 13, fontWeight: 500, color: '#f1f5f9',
          overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
        }}>
          {event.sourceTitle}
        </div>
        <div style={{ display: 'flex', gap: 8, fontSize: 11, color: '#64748b', marginTop: 2 }}>
          <span style={{ textTransform: 'capitalize' }}>{event.mediaType}</span>
          {event.indexer && <span>{event.indexer}</span>}
          {event.downloadClient && <span>{event.downloadClient}</span>}
        </div>
      </div>

      {/* Time */}
      <span style={{ fontSize: 11, color: '#64748b', flexShrink: 0 }}>
        {relativeTime(event.date)}
      </span>
    </div>
  )
}

export default function HistoryPage() {
  const navigate = useNavigate()
  const isMobile = useMobile()
  const [page, setPage] = useState(1)
  const pageSize = 25

  const { data, isLoading } = useHistory(page, pageSize)
  const events = data?.records ?? []
  const totalRecords = data?.totalRecords ?? 0
  const totalPages = Math.ceil(totalRecords / pageSize)

  return (
    <div>
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        marginBottom: 20,
      }}>
        <h2 style={{ color: '#f1f5f9', fontSize: 20, fontWeight: 700, margin: 0 }}>
          History
        </h2>
        {totalRecords > 0 && (
          <span style={{ fontSize: 12, color: '#64748b' }}>
            {totalRecords} event{totalRecords !== 1 ? 's' : ''}
          </span>
        )}
      </div>

      {isLoading && (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 60 }}>Loading...</div>
      )}

      {!isLoading && events.length === 0 && (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 60 }}>
          <Search size={48} style={{ marginBottom: 12, opacity: 0.3 }} />
          <p>No history events yet.</p>
        </div>
      )}

      {!isLoading && events.length > 0 && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {events.map((event) => (
            <HistoryRow
              key={event.id}
              event={event}
              onNavigate={() => {
                if (event.mediaType === 'series' && event.seriesId) {
                  navigate(`/series/${event.seriesId}`)
                } else if (event.movieId) {
                  navigate(`/movie/${event.movieId}`)
                }
              }}
            />
          ))}
        </div>
      )}

      {/* Pagination */}
      {totalPages > 1 && (
        <div style={{
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          gap: 12, marginTop: 24,
        }}>
          <button
            onClick={() => setPage((p) => Math.max(1, p - 1))}
            disabled={page === 1}
            style={{
              display: 'flex', padding: 6, borderRadius: 6,
              background: '#1e293b', border: '1px solid #334155',
              color: page === 1 ? '#334155' : '#94a3b8',
              cursor: page === 1 ? 'default' : 'pointer',
            }}
          >
            <ChevronLeft size={18} />
          </button>
          <span style={{ fontSize: 13, color: '#94a3b8', minWidth: isMobile ? 60 : 80, textAlign: 'center' }}>
            {page} / {totalPages}
          </span>
          <button
            onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
            disabled={page === totalPages}
            style={{
              display: 'flex', padding: 6, borderRadius: 6,
              background: '#1e293b', border: '1px solid #334155',
              color: page === totalPages ? '#334155' : '#94a3b8',
              cursor: page === totalPages ? 'default' : 'pointer',
            }}
          >
            <ChevronRight size={18} />
          </button>
        </div>
      )}
    </div>
  )
}
