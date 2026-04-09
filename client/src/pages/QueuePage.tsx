import { useNavigate } from 'react-router-dom'
import { Download, Trash2, AlertCircle, ArrowDown, ArrowUp } from 'lucide-react'
import type { QueueItem } from '../api'
import { api } from '../api'
import { ListSkeleton } from '../components/Skeleton'
import { useMobile } from '../hooks/useMobile'
import { useQueue } from '../hooks/useApi'
import { useMutation, useQueryClient } from '@tanstack/react-query'

function formatSize(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(0)} MB`
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`
  return `${bytes} B`
}

function formatEta(dateStr: string | null): string {
  if (!dateStr) return ''
  const diff = new Date(dateStr).getTime() - Date.now()
  if (diff <= 0) return 'any moment'
  const mins = Math.floor(diff / 60_000)
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ${mins % 60}m`
  const days = Math.floor(hours / 24)
  return `${days}d ${hours % 24}h`
}

function statusColor(status: string): { bg: string; fg: string } {
  switch (status) {
    case 'downloading': return { bg: '#1e40af33', fg: '#60a5fa' }
    case 'completed': return { bg: '#16653433', fg: '#4ade80' }
    case 'importing': return { bg: '#854d0e33', fg: '#fbbf24' }
    case 'queued': return { bg: '#33415533', fg: '#94a3b8' }
    case 'failed': return { bg: '#991b1b33', fg: '#f87171' }
    case 'paused': return { bg: '#33415533', fg: '#64748b' }
    default: return { bg: '#33415533', fg: '#94a3b8' }
  }
}

function protocolIcon(protocol: string) {
  if (protocol === 'usenet') return <ArrowDown size={14} />
  if (protocol === 'torrent') return <ArrowUp size={14} />
  return <Download size={14} />
}

function QueueEntry({ item, onRemove, onNavigate }: {
  item: QueueItem
  onRemove: () => void
  onNavigate: () => void
}) {
  const isMobile = useMobile()
  const sc = statusColor(item.status)
  const pct = Math.min(Math.max(item.progress, 0), 100)
  const downloaded = item.size - item.sizeLeft

  return (
    <div
      style={{
        background: '#1e293b', borderRadius: 10, border: '1px solid #334155',
        padding: isMobile ? '12px' : '14px 16px',
        cursor: 'pointer', transition: 'border-color 0.15s',
      }}
      onClick={onNavigate}
      onMouseEnter={(e) => (e.currentTarget.style.borderColor = '#3b82f6')}
      onMouseLeave={(e) => (e.currentTarget.style.borderColor = '#334155')}
    >
      {/* Title + status row */}
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8, marginBottom: 8 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, flex: 1, minWidth: 0 }}>
          <span style={{ color: '#64748b', display: 'flex' }}>{protocolIcon(item.protocol)}</span>
          <span style={{
            fontSize: 14, fontWeight: 600, color: '#f1f5f9',
            overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
          }}>
            {item.title}
          </span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}>
          <span style={{
            padding: '2px 8px', borderRadius: 4, fontSize: 11, fontWeight: 600,
            background: sc.bg, color: sc.fg, textTransform: 'capitalize',
          }}>
            {item.status}
          </span>
          <button
            onClick={(e) => { e.stopPropagation(); onRemove() }}
            style={{
              display: 'flex', padding: 4, borderRadius: 4,
              background: 'transparent', border: 'none', color: '#64748b',
              cursor: 'pointer',
            }}
            title="Remove from queue"
          >
            <Trash2 size={14} />
          </button>
        </div>
      </div>

      {/* Progress bar */}
      <div style={{
        height: 6, background: '#0f172a', borderRadius: 3, overflow: 'hidden', marginBottom: 6,
      }}>
        <div style={{
          height: '100%', borderRadius: 3,
          width: `${pct}%`,
          background: item.status === 'failed' ? '#ef4444' : '#3b82f6',
          transition: 'width 0.5s ease',
        }} />
      </div>

      {/* Meta row */}
      <div style={{ display: 'flex', gap: isMobile ? 8 : 16, fontSize: 11, color: '#64748b', flexWrap: 'wrap' }}>
        <span>{pct.toFixed(1)}%</span>
        <span>{formatSize(downloaded)} / {formatSize(item.size)}</span>
        {item.estimatedCompletionTime && item.status === 'downloading' && (
          <span>ETA: {formatEta(item.estimatedCompletionTime)}</span>
        )}
        <span>{item.downloadClient}</span>
        <span style={{ textTransform: 'capitalize' }}>{item.mediaType}</span>
      </div>

      {/* Error */}
      {item.errorMessage && (
        <div style={{
          display: 'flex', alignItems: 'center', gap: 6, marginTop: 6,
          fontSize: 12, color: '#f87171',
        }}>
          <AlertCircle size={12} /> {item.errorMessage}
        </div>
      )}
    </div>
  )
}

export default function QueuePage() {
  const navigate = useNavigate()
  const isMobile = useMobile()
  const { data: items = [], isLoading } = useQueue()
  const qc = useQueryClient()

  const removeItem = useMutation({
    mutationFn: (id: number) => api.removeQueueItem(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['queue'] }),
  })

  const downloading = items.filter((i) => i.status === 'downloading').length
  const totalSize = items.reduce((acc, i) => acc + i.size, 0)

  return (
    <div>
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        marginBottom: 20,
      }}>
        <h2 style={{ color: '#f1f5f9', fontSize: 20, fontWeight: 700, margin: 0 }}>
          Queue
        </h2>
        {items.length > 0 && (
          <div style={{ display: 'flex', gap: isMobile ? 8 : 16, fontSize: 12, color: '#64748b' }}>
            <span>{items.length} item{items.length !== 1 ? 's' : ''}</span>
            {downloading > 0 && <span>{downloading} downloading</span>}
            <span>{formatSize(totalSize)} total</span>
          </div>
        )}
      </div>

      {isLoading && <ListSkeleton count={5} />}

      {!isLoading && items.length === 0 && (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 60 }}>
          <Download size={48} style={{ marginBottom: 12, opacity: 0.3 }} />
          <p>Queue is empty — nothing downloading right now.</p>
        </div>
      )}

      {!isLoading && items.length > 0 && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {items.map((item) => (
            <QueueEntry
              key={item.id}
              item={item}
              onRemove={() => removeItem.mutate(item.id)}
              onNavigate={() => {
                if (item.mediaType === 'series' && item.seriesId) {
                  navigate(`/series/${item.seriesId}`)
                } else if (item.movieId) {
                  navigate(`/movie/${item.movieId}`)
                }
              }}
            />
          ))}
        </div>
      )}
    </div>
  )
}
