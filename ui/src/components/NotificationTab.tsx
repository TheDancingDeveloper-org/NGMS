// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import type { UserNotification } from '../api/types'

function relativeTime(dateStr: string): string {
  const diff = (Date.now() - new Date(dateStr).getTime()) / 1000
  if (diff < 60) return 'just now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  if (diff < 604800) return `${Math.floor(diff / 86400)}d ago`
  return new Date(dateStr).toLocaleDateString()
}

interface Props {
  notifications: UserNotification[]
  onMarkRead: (id: number) => void
  onMarkAllRead: () => void
}

export default function NotificationTab({ notifications, onMarkRead, onMarkAllRead }: Props) {
  const hasUnread = notifications.some((n) => !n.read)

  return (
    <>
      {/* Header with mark-all-read */}
      {hasUnread && (
        <div className="flex items-center justify-end border-b border-slate-700 px-4 py-2">
          <button
            onClick={onMarkAllRead}
            className="text-xs text-blue-400 hover:text-blue-300"
          >
            Mark all read
          </button>
        </div>
      )}

      <div>
        {notifications.length === 0 ? (
          <div className="flex items-center justify-center py-12 text-sm text-slate-500">
            No notifications
          </div>
        ) : (
          notifications.map((n) => (
            <div
              key={n.id}
              onClick={() => !n.read && onMarkRead(n.id)}
              className={`flex cursor-pointer gap-3 border-b border-slate-700 px-4 py-3 transition-colors hover:bg-slate-700/30 ${
                !n.read ? 'bg-blue-500/[0.06]' : ''
              }`}
            >
              {/* Unread dot */}
              <div className="mt-1.5 flex-shrink-0">
                {!n.read ? (
                  <div className="h-2 w-2 rounded-full bg-blue-500" />
                ) : (
                  <div className="h-2 w-2" />
                )}
              </div>

              {/* Content */}
              <div className="min-w-0 flex-1">
                <div
                  className={`truncate text-[13px] ${
                    !n.read ? 'font-semibold text-slate-100' : 'font-medium text-slate-300'
                  }`}
                >
                  {n.title}
                </div>
                {n.body && (
                  <div className="truncate text-xs text-slate-500">{n.body}</div>
                )}
                <div className="mt-1 text-[11px] text-slate-600">
                  {relativeTime(n.createdAt)}
                </div>
              </div>
            </div>
          ))
        )}
      </div>
    </>
  )
}
