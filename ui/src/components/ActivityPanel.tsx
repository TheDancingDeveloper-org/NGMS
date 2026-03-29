import { useState } from 'react'
import { PanelRightClose } from 'lucide-react'
import EventsTab from './EventsTab'
import ActivityTab from './ActivityTab'
import NotificationTab from './NotificationTab'
import {
  useActivities,
  useRunningActivityCount,
  useEventStream,
  useNotifications,
  useUnreadNotificationCount,
  useMarkNotificationRead,
  useMarkAllNotificationsRead,
} from '../hooks/useApi'

type Tab = 'events' | 'activity' | 'notifications'

export default function ActivityPanel({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>('events')

  // Always fetch — panel is always visible
  const { data: activities = [] } = useActivities(true)
  const { data: events = [] } = useEventStream(true)
  const { data: runningData } = useRunningActivityCount()
  const { data: notifications = [] } = useNotifications(true)
  const { data: unreadData } = useUnreadNotificationCount()
  const markRead = useMarkNotificationRead()
  const markAllRead = useMarkAllNotificationsRead()

  const runningCount = runningData?.count ?? 0
  const unreadCount = unreadData?.count ?? 0

  return (
    <aside className="fixed top-0 right-0 z-30 flex h-full w-80 flex-col border-l border-slate-700 bg-slate-800">
      {/* Header */}
      <div className="flex h-14 items-center justify-between border-b border-slate-700 px-4">
        <span className="text-sm font-semibold text-white">Activity</span>
        <div className="flex items-center gap-2">
          {runningCount > 0 && (
            <span className="flex items-center gap-1.5 text-[11px] text-green-400">
              <span className="flex h-1.5 w-1.5 rounded-full bg-green-400 animate-pulse" />
              {runningCount} active
            </span>
          )}
          <button
            onClick={onClose}
            className="flex h-7 w-7 items-center justify-center rounded-md text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
            title="Minimize activity panel"
          >
            <PanelRightClose size={15} />
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-slate-700">
        {(['events', 'activity', 'notifications'] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`flex flex-1 items-center justify-center gap-1.5 border-b-2 py-2.5 text-[13px] font-medium transition-colors ${
              tab === t
                ? 'border-blue-500 text-blue-400'
                : 'border-transparent text-slate-500 hover:text-slate-300'
            }`}
          >
            {t === 'events' ? 'Events' : t === 'activity' ? 'Tasks' : 'Alerts'}
            {t === 'activity' && runningCount > 0 && (
              <span className="rounded-full bg-blue-500 px-1.5 text-[10px] font-bold text-white">
                {runningCount}
              </span>
            )}
            {t === 'notifications' && unreadCount > 0 && (
              <span className="rounded-full bg-red-500 px-1.5 text-[10px] font-bold text-white">
                {unreadCount > 99 ? '99+' : unreadCount}
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Tab content — fills remaining height, scrolls internally */}
      <div className="flex-1 overflow-y-auto">
        {tab === 'events' && <EventsTab events={events} />}
        {tab === 'activity' && <ActivityTab activities={activities} />}
        {tab === 'notifications' && (
          <NotificationTab
            notifications={notifications}
            onMarkRead={(id) => markRead.mutate(id)}
            onMarkAllRead={() => markAllRead.mutate()}
          />
        )}
      </div>
    </aside>
  )
}
