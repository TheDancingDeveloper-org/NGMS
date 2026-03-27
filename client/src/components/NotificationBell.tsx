import { useState, useEffect, useRef, useCallback } from 'react'
import { Bell } from 'lucide-react'
import { api } from '../api'
import type { UserNotification } from '../api'
import NotificationDropdown from './NotificationDropdown'

export default function NotificationBell() {
  const [open, setOpen] = useState(false)
  const [unread, setUnread] = useState(0)
  const [notifications, setNotifications] = useState<UserNotification[]>([])
  const ref = useRef<HTMLDivElement>(null)

  const fetchUnread = useCallback(async () => {
    try {
      const data = await api.getUnreadCount()
      setUnread(data.count)
    } catch {
      // Silently ignore polling errors
    }
  }, [])

  const fetchNotifications = useCallback(async () => {
    try {
      const data = await api.getNotifications(false, 50, 0)
      setNotifications(data)
    } catch {
      // Silently ignore
    }
  }, [])

  // Poll unread count every 30s
  useEffect(() => {
    fetchUnread()
    const interval = setInterval(fetchUnread, 30_000)
    return () => clearInterval(interval)
  }, [fetchUnread])

  // When dropdown opens, fetch full list
  useEffect(() => {
    if (open) {
      fetchNotifications()
    }
  }, [open, fetchNotifications])

  // Close on click outside
  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    if (open) {
      document.addEventListener('mousedown', handleClick)
      return () => document.removeEventListener('mousedown', handleClick)
    }
  }, [open])

  const handleMarkRead = async (id: number) => {
    try {
      await api.markNotificationRead(id)
      setNotifications(prev =>
        prev.map(n => (n.id === id ? { ...n, read: true } : n)),
      )
      setUnread(prev => Math.max(0, prev - 1))
    } catch {
      // ignore
    }
  }

  const handleMarkAllRead = async () => {
    try {
      await api.markAllNotificationsRead()
      setNotifications(prev => prev.map(n => ({ ...n, read: true })))
      setUnread(0)
    } catch {
      // ignore
    }
  }

  return (
    <div ref={ref} style={{ position: 'relative' }}>
      <button
        onClick={() => setOpen(o => !o)}
        style={{
          background: 'none',
          border: 'none',
          color: '#94a3b8',
          cursor: 'pointer',
          position: 'relative',
          display: 'flex',
          alignItems: 'center',
          padding: 4,
        }}
        title="Notifications"
      >
        <Bell size={18} />
        {unread > 0 && (
          <span style={{
            position: 'absolute',
            top: -2,
            right: -4,
            background: '#ef4444',
            color: '#fff',
            fontSize: 10,
            fontWeight: 700,
            borderRadius: '50%',
            minWidth: 16,
            height: 16,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            padding: '0 4px',
          }}>
            {unread > 99 ? '99+' : unread}
          </span>
        )}
      </button>
      {open && (
        <NotificationDropdown
          notifications={notifications}
          onMarkRead={handleMarkRead}
          onMarkAllRead={handleMarkAllRead}
        />
      )}
    </div>
  )
}
