// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState } from 'react'
import ActivityTab from './ActivityTab'
import EventsTab from './EventsTab'
import NotificationTab from './NotificationTab'
import type { SystemActivity, UserNotification, HistoryEvent } from '../api/types'

type Tab = 'events' | 'activity' | 'notifications'

interface Props {
  activities: SystemActivity[]
  events: HistoryEvent[]
  notifications: UserNotification[]
  runningCount: number
  unreadCount: number
  onMarkRead: (id: number) => void
  onMarkAllRead: () => void
}

export default function ActivityNotificationPopup({
  activities,
  events,
  notifications,
  runningCount,
  unreadCount,
  onMarkRead,
  onMarkAllRead,
}: Props) {
  const [tab, setTab] = useState<Tab>('events')

  return (
    <div className="absolute right-0 top-full z-50 mt-2 w-[400px] overflow-hidden rounded-lg border border-slate-700 bg-slate-800 shadow-[0_8px_32px_rgba(0,0,0,0.4)]">
      {/* Tabs */}
      <div className="flex border-b border-slate-700">
        <button
          onClick={() => setTab('events')}
          className={`flex flex-1 items-center justify-center gap-1.5 border-b-2 py-2.5 text-[13px] font-medium transition-colors ${
            tab === 'events'
              ? 'border-blue-500 text-blue-400'
              : 'border-transparent text-slate-500 hover:text-slate-300'
          }`}
        >
          Events
        </button>
        <button
          onClick={() => setTab('activity')}
          className={`flex flex-1 items-center justify-center gap-1.5 border-b-2 py-2.5 text-[13px] font-medium transition-colors ${
            tab === 'activity'
              ? 'border-blue-500 text-blue-400'
              : 'border-transparent text-slate-500 hover:text-slate-300'
          }`}
        >
          Activity
          {runningCount > 0 && (
            <span className="rounded-full bg-blue-500 px-1.5 text-[10px] font-bold text-white">
              {runningCount}
            </span>
          )}
        </button>
        <button
          onClick={() => setTab('notifications')}
          className={`flex flex-1 items-center justify-center gap-1.5 border-b-2 py-2.5 text-[13px] font-medium transition-colors ${
            tab === 'notifications'
              ? 'border-blue-500 text-blue-400'
              : 'border-transparent text-slate-500 hover:text-slate-300'
          }`}
        >
          Notifications
          {unreadCount > 0 && (
            <span className="rounded-full bg-red-500 px-1.5 text-[10px] font-bold text-white">
              {unreadCount > 99 ? '99+' : unreadCount}
            </span>
          )}
        </button>
      </div>

      {/* Tab content */}
      {tab === 'events' && <EventsTab events={events} />}
      {tab === 'activity' && <ActivityTab activities={activities} />}
      {tab === 'notifications' && (
        <NotificationTab
          notifications={notifications}
          onMarkRead={onMarkRead}
          onMarkAllRead={onMarkAllRead}
        />
      )}
    </div>
  )
}
