import { useState, useRef, useEffect } from 'react'
import { Bell } from 'lucide-react'
import ActivityNotificationPopup from './ActivityNotificationPopup'
import {
  useActivities,
  useRunningActivityCount,
  useEventStream,
  useNotifications,
  useUnreadNotificationCount,
  useMarkNotificationRead,
  useMarkAllNotificationsRead,
} from '../hooks/useApi'

export default function ActivityNotificationBell() {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  // Data — activities poll at 5s when open, notifications fetched on open
  const { data: activities = [] } = useActivities(open)
  const { data: events = [] } = useEventStream(open)
  const { data: runningData } = useRunningActivityCount()
  const { data: notifications = [] } = useNotifications(open)
  const { data: unreadData } = useUnreadNotificationCount()
  const markRead = useMarkNotificationRead()
  const markAllRead = useMarkAllNotificationsRead()

  const runningCount = runningData?.count ?? 0
  const unreadCount = unreadData?.count ?? 0
  const totalBadge = runningCount + unreadCount

  // Click outside to close
  useEffect(() => {
    if (!open) return
    function handler(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [open])

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex h-8 w-8 items-center justify-center rounded-md border border-slate-700 text-slate-400 transition-colors hover:bg-slate-700 hover:text-slate-200"
        title="Activity & Notifications"
      >
        <Bell size={16} />
        {totalBadge > 0 && (
          <span className="absolute -right-1 -top-1 flex h-4 min-w-4 items-center justify-center rounded-full border-2 border-slate-900 bg-red-500 px-0.5 text-[10px] font-bold text-white">
            {totalBadge > 99 ? '99+' : totalBadge}
          </span>
        )}
      </button>

      {open && (
        <ActivityNotificationPopup
          activities={activities}
          events={events}
          notifications={notifications}
          runningCount={runningCount}
          unreadCount={unreadCount}
          onMarkRead={(id) => markRead.mutate(id)}
          onMarkAllRead={() => markAllRead.mutate()}
        />
      )}
    </div>
  )
}
