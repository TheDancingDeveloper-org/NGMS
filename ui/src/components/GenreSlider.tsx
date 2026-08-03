// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState, useEffect, useRef } from 'react'
import MediaSlider from './MediaSlider'
import MediaCard from './MediaCard'
import { useMoviesByGenre, useTvByGenre } from '../hooks/useApi'
import type { TmdbGenre, TmdbTrendingItem } from '../api/types'

interface GenreSliderProps {
  genre: TmdbGenre
  mediaType: 'movie' | 'tv'
  onItemClick: (item: TmdbTrendingItem) => void
}

export default function GenreSlider({ genre, mediaType, onItemClick }: GenreSliderProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [isVisible, setIsVisible] = useState(false)

  useEffect(() => {
    const el = containerRef.current
    if (!el) return
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setIsVisible(true)
          observer.disconnect()
        }
      },
      { rootMargin: '200px' },
    )
    observer.observe(el)
    return () => observer.disconnect()
  }, [])

  const movieQuery = useMoviesByGenre(isVisible && mediaType === 'movie' ? genre.id : 0)
  const tvQuery = useTvByGenre(isVisible && mediaType === 'tv' ? genre.id : 0)
  const query = mediaType === 'movie' ? movieQuery : tvQuery
  const results = query.data?.results

  // Don't render empty genre rows
  if (isVisible && !query.isLoading && (!results || results.length === 0)) return null

  return (
    <div ref={containerRef}>
      <MediaSlider title={genre.name} isLoading={isVisible && query.isLoading}>
        {results?.map((item) => (
          <MediaCard
            key={item.id}
            item={{ ...item, media_type: mediaType } as TmdbTrendingItem}
            onClick={() => onItemClick({ ...item, media_type: mediaType } as TmdbTrendingItem)}
          />
        ))}
      </MediaSlider>
    </div>
  )
}
