import { useState, useEffect } from 'react'
import { Loader2 } from 'lucide-react'
import { api, type MediaRequest } from '../api'
import RequestCard from '../components/RequestCard'

export default function RequestsPage() {
  const [requests, setRequests] = useState<MediaRequest[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    loadRequests()
  }, [])

  async function loadRequests() {
    setLoading(true)
    setError(null)
    try {
      const data = await api.listMyRequests()
      setRequests(data)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load requests')
    } finally {
      setLoading(false)
    }
  }

  if (loading) {
    return (
      <div
        style={{
          display: 'flex',
          justifyContent: 'center',
          alignItems: 'center',
          padding: 60,
          color: '#94a3b8',
        }}
      >
        <Loader2 size={24} style={{ animation: 'spin 1s linear infinite' }} />
        <style>{`@keyframes spin { to { transform: rotate(360deg); } }`}</style>
      </div>
    )
  }

  return (
    <div>
      <h2 style={{ color: '#f1f5f9', fontSize: 20, fontWeight: 700, marginBottom: 16 }}>
        My Requests
      </h2>

      {error && (
        <div
          style={{
            padding: 12,
            background: '#7f1d1d',
            color: '#fca5a5',
            borderRadius: 8,
            marginBottom: 16,
            fontSize: 13,
          }}
        >
          {error}
        </div>
      )}

      {requests.length === 0 ? (
        <div style={{ color: '#64748b', textAlign: 'center', padding: 60 }}>
          <p>You have no requests yet.</p>
          <p style={{ fontSize: 13 }}>
            Use the Discover page to search and request movies or TV shows.
          </p>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {requests.map((req) => (
            <RequestCard key={req.id} request={req} />
          ))}
        </div>
      )}
    </div>
  )
}
