import { Clock, Check, X, Library } from 'lucide-react'
import type { MediaRequest } from '../api'

const TMDB_IMAGE_BASE = 'https://image.tmdb.org/t/p'

function statusBadge(status: string) {
  const styles: Record<string, { bg: string; color: string; label: string }> = {
    pending: { bg: '#854d0e', color: '#fbbf24', label: 'Pending' },
    approved: { bg: '#166534', color: '#4ade80', label: 'Approved' },
    declined: { bg: '#991b1b', color: '#f87171', label: 'Declined' },
    available: { bg: '#1e40af', color: '#60a5fa', label: 'Available' },
  }
  const s = styles[status] || styles.pending
  return (
    <span
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        gap: 4,
        padding: '2px 8px',
        borderRadius: 6,
        fontSize: 11,
        fontWeight: 600,
        background: s.bg,
        color: s.color,
      }}
    >
      {status === 'pending' && <Clock size={12} />}
      {status === 'approved' && <Check size={12} />}
      {status === 'declined' && <X size={12} />}
      {status === 'available' && <Library size={12} />}
      {s.label}
    </span>
  )
}

export default function RequestCard({ request }: { request: MediaRequest }) {
  const poster = request.posterUrl
    ? `/api/v1/images/${TMDB_IMAGE_BASE}/w342${request.posterUrl}`
    : null

  return (
    <div
      style={{
        display: 'flex',
        gap: 12,
        padding: 12,
        background: '#1e293b',
        borderRadius: 10,
        border: '1px solid #334155',
      }}
    >
      <div
        style={{
          width: 80,
          minHeight: 120,
          borderRadius: 8,
          overflow: 'hidden',
          background: '#0f172a',
          flexShrink: 0,
        }}
      >
        {poster ? (
          <img
            src={poster}
            alt={request.title}
            style={{ width: '100%', height: '100%', objectFit: 'cover' }}
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
              fontSize: 11,
            }}
          >
            No poster
          </div>
        )}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 4 }}>
          <span style={{ fontWeight: 600, color: '#f1f5f9', fontSize: 14 }}>
            {request.title}
          </span>
          {request.year && (
            <span style={{ color: '#94a3b8', fontSize: 12 }}>({request.year})</span>
          )}
        </div>
        <div style={{ display: 'flex', gap: 6, marginBottom: 6 }}>
          {statusBadge(request.status)}
          <span
            style={{
              fontSize: 11,
              color: '#64748b',
              padding: '2px 6px',
              background: '#0f172a',
              borderRadius: 4,
            }}
          >
            {request.mediaType === 'series' ? 'TV' : 'Movie'}
          </span>
        </div>
        {request.overview && (
          <p
            style={{
              color: '#94a3b8',
              fontSize: 12,
              lineHeight: 1.4,
              margin: 0,
              display: '-webkit-box',
              WebkitLineClamp: 2,
              WebkitBoxOrient: 'vertical',
              overflow: 'hidden',
            }}
          >
            {request.overview}
          </p>
        )}
        {request.adminNote && (
          <p style={{ color: '#fbbf24', fontSize: 11, marginTop: 4, margin: '4px 0 0' }}>
            Admin: {request.adminNote}
          </p>
        )}
        <div style={{ color: '#475569', fontSize: 10, marginTop: 4 }}>
          Requested {new Date(request.createdAt).toLocaleDateString()}
        </div>
      </div>
    </div>
  )
}
