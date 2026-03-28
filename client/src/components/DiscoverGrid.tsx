import { Check, Clock, Library, Plus, X } from 'lucide-react'
import type { DiscoverResult } from '../api'
import { useMobile } from '../hooks/useMobile'

const TMDB_IMAGE_BASE = 'https://image.tmdb.org/t/p'

function getTitle(item: DiscoverResult): string {
  return item.title || item.name || 'Unknown'
}

function getYear(item: DiscoverResult): string {
  const date = item.releaseDate || item.firstAirDate
  return date?.substring(0, 4) || ''
}

function statusButton(item: DiscoverResult, onRequest: (item: DiscoverResult) => void) {
  if (item.inLibrary) {
    return (
      <span
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 4,
          padding: '4px 10px',
          borderRadius: 6,
          fontSize: 11,
          fontWeight: 600,
          background: '#166534',
          color: '#4ade80',
        }}
      >
        <Library size={12} /> In Library
      </span>
    )
  }
  if (item.requestStatus === 'pending') {
    return (
      <span
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 4,
          padding: '4px 10px',
          borderRadius: 6,
          fontSize: 11,
          fontWeight: 600,
          background: '#854d0e',
          color: '#fbbf24',
        }}
      >
        <Clock size={12} /> Requested
      </span>
    )
  }
  if (item.requestStatus === 'approved') {
    return (
      <span
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 4,
          padding: '4px 10px',
          borderRadius: 6,
          fontSize: 11,
          fontWeight: 600,
          background: '#1e40af',
          color: '#60a5fa',
        }}
      >
        <Check size={12} /> Approved
      </span>
    )
  }
  if (item.requestStatus === 'declined') {
    return (
      <span
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 4,
          padding: '4px 10px',
          borderRadius: 6,
          fontSize: 11,
          fontWeight: 600,
          background: '#991b1b',
          color: '#f87171',
        }}
      >
        <X size={12} /> Declined
      </span>
    )
  }
  return (
    <button
      onClick={(e) => {
        e.stopPropagation()
        onRequest(item)
      }}
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
        padding: '4px 10px',
        borderRadius: 6,
        fontSize: 11,
        fontWeight: 600,
        background: '#1e40af',
        color: '#fff',
        border: 'none',
        cursor: 'pointer',
      }}
    >
      <Plus size={12} /> Request
    </button>
  )
}

export default function DiscoverGrid({
  results,
  onRequest,
}: {
  results: DiscoverResult[]
  onRequest: (item: DiscoverResult) => void
}) {
  const isMobile = useMobile()

  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: isMobile
          ? 'repeat(auto-fill, minmax(110px, 1fr))'
          : 'repeat(auto-fill, minmax(160px, 1fr))',
        gap: isMobile ? 10 : 16,
      }}
    >
      {results.map((item) => (
        <div
          key={`${item.mediaType}-${item.id}`}
          style={{
            background: '#1e293b',
            border: '1px solid #334155',
            borderRadius: 12,
            overflow: 'hidden',
            transition: 'transform 0.15s',
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.transform = 'scale(1.03)'
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.transform = 'scale(1)'
          }}
        >
          <div
            style={{
              aspectRatio: '2/3',
              background: '#0f172a',
              position: 'relative',
            }}
          >
            {item.posterPath ? (
              <img
                src={`/api/v1/images/${TMDB_IMAGE_BASE}/w342${item.posterPath}`}
                alt={getTitle(item)}
                style={{ width: '100%', height: '100%', objectFit: 'cover' }}
                loading="lazy"
              />
            ) : (
              <div
                style={{
                  width: '100%',
                  height: '100%',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  color: '#475569',
                  fontSize: 12,
                }}
              >
                No poster
              </div>
            )}
            {item.voteAverage > 0 && (
              <span
                style={{
                  position: 'absolute',
                  top: 6,
                  right: 6,
                  background: 'rgba(0,0,0,0.7)',
                  color: '#fbbf24',
                  fontSize: 11,
                  fontWeight: 600,
                  padding: '2px 6px',
                  borderRadius: 4,
                }}
              >
                {item.voteAverage.toFixed(1)}
              </span>
            )}
          </div>
          <div style={{ padding: '8px 10px' }}>
            <div
              style={{
                fontWeight: 600,
                color: '#f1f5f9',
                fontSize: 13,
                whiteSpace: 'nowrap',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
              }}
            >
              {getTitle(item)}
            </div>
            <div
              style={{
                color: '#94a3b8',
                fontSize: 11,
                marginBottom: 6,
              }}
            >
              {getYear(item)} · {item.mediaType === 'series' ? 'TV' : 'Movie'}
            </div>
            {statusButton(item, onRequest)}
          </div>
        </div>
      ))}
    </div>
  )
}
