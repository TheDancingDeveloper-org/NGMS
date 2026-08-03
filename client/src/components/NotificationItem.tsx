// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import type { UserNotification } from '../api'

function relativeTime(dateStr: string): string {
  const now = Date.now()
  const then = new Date(dateStr).getTime()
  const diffSec = Math.floor((now - then) / 1000)

  if (diffSec < 60) return 'just now'
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`
  if (diffSec < 604800) return `${Math.floor(diffSec / 86400)}d ago`
  return new Date(dateStr).toLocaleDateString()
}

interface Props {
  notification: UserNotification
  onMarkRead: (id: number) => void
  onNavigate?: (path: string) => void
}

export default function NotificationItem({ notification, onMarkRead, onNavigate }: Props) {
  const handleClick = () => {
    if (!notification.read) {
      onMarkRead(notification.id)
    }
    if (!onNavigate) return

    // Navigate based on notification data or type
    const d = notification.data
    if (d) {
      if (d.seriesId) {
        onNavigate(`/series/${d.seriesId}`)
        return
      }
      if (d.movieId) {
        onNavigate(`/movie/${d.movieId}`)
        return
      }
    }

    // Fallback: navigate by notification type
    const t = notification.notificationType
    if (t.includes('request')) {
      onNavigate('/requests')
    }
  }

  return (
    <div
      onClick={handleClick}
      style={{
        padding: '10px 14px',
        cursor: 'pointer',
        borderBottom: '1px solid #334155',
        background: notification.read ? 'transparent' : 'rgba(59, 130, 246, 0.08)',
        display: 'flex',
        gap: 10,
        alignItems: 'flex-start',
        transition: 'background 0.15s',
      }}
    >
      {/* Unread dot */}
      {!notification.read && (
        <div style={{
          width: 8,
          height: 8,
          borderRadius: '50%',
          background: '#3b82f6',
          marginTop: 6,
          flexShrink: 0,
        }} />
      )}
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{
          fontSize: 13,
          fontWeight: notification.read ? 400 : 600,
          color: '#e2e8f0',
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
        }}>
          {notification.title}
        </div>
        {notification.body && (
          <div style={{
            fontSize: 12,
            color: '#94a3b8',
            marginTop: 2,
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}>
            {notification.body}
          </div>
        )}
        <div style={{ fontSize: 11, color: '#64748b', marginTop: 4 }}>
          {relativeTime(notification.createdAt)}
        </div>
      </div>
    </div>
  )
}
