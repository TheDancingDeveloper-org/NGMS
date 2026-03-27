import { useState } from 'react'
import { Check, X, Trash2, Clock, Film, Tv, Loader2, MessageSquare } from 'lucide-react'
import {
  useMediaRequests,
  useApproveRequest,
  useDeclineRequest,
  useDeleteRequest,
} from '../hooks/useApi'
import { tmdbPosterUrl } from '../api/types'
import type { MediaRequest } from '../api/types'

const STATUS_TABS = [
  { key: '', label: 'All' },
  { key: 'pending', label: 'Pending' },
  { key: 'approved', label: 'Approved' },
  { key: 'declined', label: 'Declined' },
  { key: 'available', label: 'Available' },
]

function StatusBadge({ status }: { status: string }) {
  const config: Record<string, { bg: string; text: string }> = {
    pending: { bg: 'bg-yellow-900/50', text: 'text-yellow-400' },
    approved: { bg: 'bg-green-900/50', text: 'text-green-400' },
    declined: { bg: 'bg-red-900/50', text: 'text-red-400' },
    available: { bg: 'bg-blue-900/50', text: 'text-blue-400' },
  }
  const c = config[status] || config.pending
  return (
    <span className={`inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs font-semibold ${c.bg} ${c.text}`}>
      {status === 'pending' && <Clock size={12} />}
      {status === 'approved' && <Check size={12} />}
      {status === 'declined' && <X size={12} />}
      {status.charAt(0).toUpperCase() + status.slice(1)}
    </span>
  )
}

function RequestRow({
  request,
  onApprove,
  onDecline,
  onDelete,
}: {
  request: MediaRequest
  onApprove: (id: number, note?: string) => void
  onDecline: (id: number, note?: string) => void
  onDelete: (id: number) => void
}) {
  const [showNote, setShowNote] = useState(false)
  const [note, setNote] = useState('')

  const poster = tmdbPosterUrl(request.posterUrl, 'w185')

  return (
    <div className="flex gap-3 rounded-lg border border-slate-700 bg-slate-800/50 p-3">
      <div className="h-24 w-16 flex-shrink-0 overflow-hidden rounded-md bg-slate-900">
        {poster ? (
          <img src={poster} alt={request.title} className="h-full w-full object-cover" />
        ) : (
          <div className="flex h-full w-full items-center justify-center text-xs text-slate-600">
            {request.mediaType === 'series' ? <Tv size={20} /> : <Film size={20} />}
          </div>
        )}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate font-semibold text-slate-100">{request.title}</span>
          {request.year && <span className="text-xs text-slate-500">({request.year})</span>}
          <StatusBadge status={request.status} />
          <span className="text-xs text-slate-600">
            {request.mediaType === 'series' ? 'TV' : 'Movie'}
          </span>
        </div>
        {request.overview && (
          <p className="mt-1 line-clamp-1 text-xs text-slate-400">{request.overview}</p>
        )}
        <div className="mt-1 text-xs text-slate-600">
          Requested {new Date(request.createdAt).toLocaleDateString()}
          {request.adminNote && (
            <span className="ml-2 text-yellow-500">Note: {request.adminNote}</span>
          )}
        </div>

        {showNote && (
          <div className="mt-2 flex gap-2">
            <input
              type="text"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder="Optional note..."
              className="flex-1 rounded border border-slate-600 bg-slate-900 px-2 py-1 text-xs text-slate-300 outline-none focus:border-blue-500"
            />
            <button
              onClick={() => {
                onApprove(request.id, note || undefined)
                setShowNote(false)
                setNote('')
              }}
              className="rounded bg-green-600 px-2 py-1 text-xs text-white hover:bg-green-500"
            >
              Approve
            </button>
            <button
              onClick={() => {
                onDecline(request.id, note || undefined)
                setShowNote(false)
                setNote('')
              }}
              className="rounded bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-500"
            >
              Decline
            </button>
            <button
              onClick={() => {
                setShowNote(false)
                setNote('')
              }}
              className="rounded bg-slate-700 px-2 py-1 text-xs text-slate-300 hover:bg-slate-600"
            >
              Cancel
            </button>
          </div>
        )}

        {!showNote && request.status === 'pending' && (
          <div className="mt-2 flex gap-2">
            <button
              onClick={() => onApprove(request.id)}
              className="flex items-center gap-1 rounded bg-green-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-green-500"
              title="Approve"
            >
              <Check size={12} /> Approve
            </button>
            <button
              onClick={() => onDecline(request.id)}
              className="flex items-center gap-1 rounded bg-red-600 px-2.5 py-1 text-xs font-medium text-white hover:bg-red-500"
              title="Decline"
            >
              <X size={12} /> Decline
            </button>
            <button
              onClick={() => setShowNote(true)}
              className="flex items-center gap-1 rounded bg-slate-700 px-2.5 py-1 text-xs font-medium text-slate-300 hover:bg-slate-600"
              title="Add note"
            >
              <MessageSquare size={12} /> Note
            </button>
            <button
              onClick={() => onDelete(request.id)}
              className="flex items-center gap-1 rounded bg-slate-700 px-2.5 py-1 text-xs font-medium text-red-400 hover:bg-slate-600"
              title="Delete"
            >
              <Trash2 size={12} />
            </button>
          </div>
        )}

        {!showNote && request.status !== 'pending' && (
          <div className="mt-2">
            <button
              onClick={() => onDelete(request.id)}
              className="flex items-center gap-1 rounded bg-slate-700 px-2.5 py-1 text-xs font-medium text-red-400 hover:bg-slate-600"
              title="Delete"
            >
              <Trash2 size={12} /> Delete
            </button>
          </div>
        )}
      </div>
    </div>
  )
}

export default function Requests() {
  const [statusFilter, setStatusFilter] = useState('')
  const { data: requests, isLoading, error } = useMediaRequests(statusFilter || undefined)
  const approveRequest = useApproveRequest()
  const declineRequest = useDeclineRequest()
  const deleteRequest = useDeleteRequest()

  const handleApprove = (id: number, note?: string) => {
    approveRequest.mutate({ id, note })
  }

  const handleDecline = (id: number, note?: string) => {
    declineRequest.mutate({ id, note })
  }

  const handleDelete = (id: number) => {
    if (confirm('Delete this request?')) {
      deleteRequest.mutate(id)
    }
  }

  const pendingCount = requests?.filter((r) => r.status === 'pending').length ?? 0

  return (
    <div className="mx-auto max-w-4xl">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-xl font-bold text-white">
          Media Requests
          {pendingCount > 0 && statusFilter !== 'pending' && (
            <span className="ml-2 rounded-full bg-yellow-600 px-2 py-0.5 text-xs font-medium text-white">
              {pendingCount} pending
            </span>
          )}
        </h1>
      </div>

      <div className="mb-4 flex gap-1">
        {STATUS_TABS.map((tab) => (
          <button
            key={tab.key}
            onClick={() => setStatusFilter(tab.key)}
            className={`rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
              statusFilter === tab.key
                ? 'bg-blue-600 text-white'
                : 'bg-slate-800 text-slate-400 hover:bg-slate-700 hover:text-white'
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {isLoading && (
        <div className="flex justify-center py-12">
          <Loader2 size={24} className="animate-spin text-slate-500" />
        </div>
      )}

      {error && (
        <div className="rounded-lg bg-red-900/30 px-4 py-3 text-sm text-red-300">
          Failed to load requests
        </div>
      )}

      {requests && requests.length === 0 && (
        <div className="py-12 text-center text-sm text-slate-500">
          No {statusFilter || ''} requests found.
        </div>
      )}

      {requests && requests.length > 0 && (
        <div className="flex flex-col gap-2">
          {requests.map((req) => (
            <RequestRow
              key={req.id}
              request={req}
              onApprove={handleApprove}
              onDecline={handleDecline}
              onDelete={handleDelete}
            />
          ))}
        </div>
      )}
    </div>
  )
}
