// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState, useEffect } from 'react'
import { Bookmark } from 'lucide-react'
import { api } from '../api'

interface Props {
  mediaType: 'series' | 'movie'
  mediaId: number
}

export default function WatchlistButton({ mediaType, mediaId }: Props) {
  const [onWatchlist, setOnWatchlist] = useState(false)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    api
      .getWatchlist(mediaType)
      .then((items) => {
        setOnWatchlist(items.some((i) => i.mediaId === mediaId))
      })
      .catch(() => {})
      .finally(() => setLoading(false))
  }, [mediaType, mediaId])

  const toggle = async () => {
    try {
      if (onWatchlist) {
        await api.removeFromWatchlist(mediaType, mediaId)
        setOnWatchlist(false)
      } else {
        await api.addToWatchlist(mediaType, mediaId)
        setOnWatchlist(true)
      }
    } catch (e) {
      console.error('Watchlist toggle failed:', e)
    }
  }

  if (loading) return null

  return (
    <button
      onClick={toggle}
      title={onWatchlist ? 'Remove from watchlist' : 'Add to watchlist'}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 6,
        padding: '8px 16px',
        borderRadius: 8,
        background: onWatchlist ? '#1e40af' : '#334155',
        border: 'none',
        color: onWatchlist ? '#fff' : '#94a3b8',
        cursor: 'pointer',
        fontSize: 13,
        fontWeight: 500,
        transition: 'all 0.15s',
      }}
    >
      <Bookmark size={16} fill={onWatchlist ? '#fff' : 'none'} />
      {onWatchlist ? 'On Watchlist' : 'Watchlist'}
    </button>
  )
}
