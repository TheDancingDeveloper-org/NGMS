import { useState, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Check, X } from 'lucide-react'
import { api } from '../api'
import RequestCard from '../components/RequestCard'
import { CardListSkeleton } from '../components/Skeleton'
import { useAuth } from '../context/AuthContext'
import { useMobile } from '../hooks/useMobile'

const TAB_STYLE = (active: boolean): React.CSSProperties => ({
  padding: '6px 14px',
  borderRadius: 6,
  border: 'none',
  cursor: 'pointer',
  fontSize: 13,
  fontWeight: 500,
  background: active ? '#1e40af' : '#334155',
  color: active ? '#fff' : '#94a3b8',
  transition: 'background 0.15s',
})

export default function RequestsPage() {
  const { user } = useAuth()
  const isMobile = useMobile()
  const qc = useQueryClient()
  const isAdmin = user?.role === 'admin'

  const [statusFilter, setStatusFilter] = useState<string>('all')
  const [viewMode, setViewMode] = useState<'mine' | 'all'>(isAdmin ? 'all' : 'mine')

  // Fetch either my requests or all requests depending on mode
  const { data: requests = [], isLoading, error } = useQuery({
    queryKey: ['requests', viewMode, statusFilter],
    queryFn: () =>
      viewMode === 'mine'
        ? api.listMyRequests()
        : api.listAllRequests(statusFilter !== 'all' ? statusFilter : undefined),
  })

  const filteredRequests = useMemo(() => {
    if (viewMode === 'all' && statusFilter !== 'all') {
      // Server-side filtering already applied
      return requests
    }
    if (statusFilter === 'all') return requests
    return requests.filter((r) => r.status === statusFilter)
  }, [requests, statusFilter, viewMode])

  const approve = useMutation({
    mutationFn: (id: number) => api.approveRequest(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['requests'] }),
  })

  const decline = useMutation({
    mutationFn: (id: number) => api.declineRequest(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['requests'] }),
  })

  const counts = useMemo(() => {
    const c = { all: requests.length, pending: 0, approved: 0, declined: 0, available: 0 }
    for (const r of requests) {
      if (r.status in c) c[r.status as keyof typeof c]++
    }
    return c
  }, [requests])

  if (isLoading) {
    return <CardListSkeleton count={4} />
  }

  return (
    <div>
      <div style={{
        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
        marginBottom: 16, flexWrap: 'wrap', gap: 12,
      }}>
        <h2 style={{ color: '#f1f5f9', fontSize: 20, fontWeight: 700, margin: 0 }}>
          Requests
        </h2>

        {/* Admin toggle: mine vs all */}
        {isAdmin && (
          <div style={{ display: 'flex', gap: 4 }}>
            <button
              onClick={() => setViewMode('all')}
              style={TAB_STYLE(viewMode === 'all')}
            >
              All Requests
            </button>
            <button
              onClick={() => setViewMode('mine')}
              style={TAB_STYLE(viewMode === 'mine')}
            >
              My Requests
            </button>
          </div>
        )}
      </div>

      {/* Status filter tabs */}
      <div style={{
        display: 'flex', gap: 6, marginBottom: 20,
        flexWrap: 'wrap',
      }}>
        {(['all', 'pending', 'approved', 'declined'] as const).map((status) => (
          <button
            key={status}
            onClick={() => setStatusFilter(status)}
            style={{
              ...TAB_STYLE(statusFilter === status),
              display: 'flex', alignItems: 'center', gap: 4,
            }}
          >
            {status.charAt(0).toUpperCase() + status.slice(1)}
            {viewMode === 'mine' && (
              <span style={{
                fontSize: 10, padding: '0 4px',
                borderRadius: 3,
                background: statusFilter === status ? 'rgba(255,255,255,0.15)' : 'rgba(255,255,255,0.05)',
              }}>
                {counts[status]}
              </span>
            )}
          </button>
        ))}
      </div>

      {error && (
        <div style={{
          padding: 12, background: '#7f1d1d', color: '#fca5a5',
          borderRadius: 8, marginBottom: 16, fontSize: 13,
        }}>
          {error instanceof Error ? error.message : 'Failed to load requests'}
        </div>
      )}

      {filteredRequests.length === 0 ? (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 60 }}>
          <p>No {statusFilter !== 'all' ? statusFilter : ''} requests found.</p>
          <p style={{ fontSize: 13 }}>
            Use the Discover page to search and request movies or TV shows.
          </p>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {filteredRequests.map((req) => (
            <div key={req.id}>
              <RequestCard request={req} />

              {/* Admin actions */}
              {isAdmin && req.status === 'pending' && (
                <div style={{
                  display: 'flex', gap: 8, paddingLeft: isMobile ? 12 : 92,
                  marginTop: 6,
                }}>
                  <button
                    onClick={() => approve.mutate(req.id)}
                    disabled={approve.isPending}
                    style={{
                      display: 'flex', alignItems: 'center', gap: 4,
                      padding: '5px 12px', borderRadius: 6, fontSize: 12, fontWeight: 600,
                      background: '#166534', border: 'none', color: '#4ade80',
                      cursor: 'pointer',
                    }}
                  >
                    <Check size={12} /> Approve
                  </button>
                  <button
                    onClick={() => decline.mutate(req.id)}
                    disabled={decline.isPending}
                    style={{
                      display: 'flex', alignItems: 'center', gap: 4,
                      padding: '5px 12px', borderRadius: 6, fontSize: 12, fontWeight: 600,
                      background: '#991b1b', border: 'none', color: '#f87171',
                      cursor: 'pointer',
                    }}
                  >
                    <X size={12} /> Decline
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
