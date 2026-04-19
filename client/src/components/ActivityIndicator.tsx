import { Loader2 } from 'lucide-react'
import { useRunningActivities } from '../hooks/useApi'

export default function ActivityIndicator() {
  const { data } = useRunningActivities()
  const count = data?.count ?? 0

  if (count === 0) return null

  return (
    <div
      title={`${count} active task${count !== 1 ? 's' : ''}`}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 4,
        padding: '2px 8px',
        borderRadius: 6,
        background: '#1e40af22',
        color: '#60a5fa',
        fontSize: 11,
        fontWeight: 600,
      }}
    >
      <Loader2 size={12} style={{ animation: 'spin 1.5s linear infinite' }} />
      <style>{`@keyframes spin { to { transform: rotate(360deg); } }`}</style>
      {count}
    </div>
  )
}
