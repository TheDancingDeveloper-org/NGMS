import type { UserNotification } from '../api'
import NotificationItem from './NotificationItem'

interface Props {
  notifications: UserNotification[]
  onMarkRead: (id: number) => void
  onMarkAllRead: () => void
  onNavigate?: (path: string) => void
}

export default function NotificationDropdown({
  notifications,
  onMarkRead,
  onMarkAllRead,
  onNavigate,
}: Props) {
  return (
    <div style={{
      position: 'absolute',
      top: '100%',
      right: 0,
      marginTop: 8,
      width: 360,
      maxHeight: 440,
      background: '#1e293b',
      border: '1px solid #334155',
      borderRadius: 10,
      boxShadow: '0 8px 32px rgba(0,0,0,0.4)',
      overflow: 'hidden',
      zIndex: 1000,
    }}>
      {/* Header */}
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        padding: '10px 14px',
        borderBottom: '1px solid #334155',
      }}>
        <span style={{ fontSize: 14, fontWeight: 600, color: '#e2e8f0' }}>
          Notifications
        </span>
        {notifications.some(n => !n.read) && (
          <button
            onClick={(e) => {
              e.stopPropagation()
              onMarkAllRead()
            }}
            style={{
              background: 'none',
              border: 'none',
              color: '#3b82f6',
              fontSize: 12,
              cursor: 'pointer',
              padding: '2px 6px',
            }}
          >
            Mark all read
          </button>
        )}
      </div>

      {/* List */}
      <div style={{ overflowY: 'auto', maxHeight: 380 }}>
        {notifications.length === 0 ? (
          <div style={{
            padding: 24,
            textAlign: 'center',
            color: '#64748b',
            fontSize: 13,
          }}>
            No notifications
          </div>
        ) : (
          notifications.map(n => (
            <NotificationItem
              key={n.id}
              notification={n}
              onMarkRead={onMarkRead}
              onNavigate={onNavigate}
            />
          ))
        )}
      </div>
    </div>
  )
}
