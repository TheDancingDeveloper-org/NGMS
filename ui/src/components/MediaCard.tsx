import { memo } from 'react'
import { Film, Tv, Star, Plus } from 'lucide-react'
import { tmdbPosterUrl, tmdbDisplayTitle, tmdbYear } from '../api/types'
import type { TmdbTrendingItem, TmdbMovie, TmdbSeries } from '../api/types'

type MediaItem = TmdbTrendingItem | TmdbMovie | TmdbSeries

function getTitle(item: MediaItem): string {
  if ('media_type' in item && (item as TmdbTrendingItem).media_type) {
    return tmdbDisplayTitle(item as TmdbTrendingItem)
  }
  if ('title' in item && typeof item.title === 'string') return item.title
  if ('name' in item && typeof item.name === 'string') return item.name
  return 'Unknown'
}

function getYear(item: MediaItem): string {
  return tmdbYear(item as TmdbTrendingItem)
}

function getMediaType(item: MediaItem): 'movie' | 'tv' | undefined {
  if ('media_type' in item) return (item as TmdbTrendingItem).media_type as 'movie' | 'tv'
  if ('title' in item && typeof item.title === 'string') return 'movie'
  if ('name' in item && typeof item.name === 'string') return 'tv'
  return undefined
}

interface MediaCardProps {
  item: MediaItem
  onClick?: () => void
  onAdd?: () => void
}

export default memo(function MediaCard({ item, onClick, onAdd }: MediaCardProps) {
  const posterUrl = tmdbPosterUrl(item.poster_path, 'w342')
  const title = getTitle(item)
  const year = getYear(item)
  const mediaType = getMediaType(item)
  const rating = item.vote_average

  return (
    <div className="group w-[120px] shrink-0 sm:w-[130px]">
      <button
        onClick={onClick}
        className="relative aspect-[2/3] w-full overflow-hidden rounded-md bg-slate-800 transition-transform hover:scale-[1.05] hover:ring-2 hover:ring-blue-500"
      >
        {posterUrl ? (
          <img src={posterUrl} alt={title} className="h-full w-full object-cover" loading="lazy" />
        ) : (
          <div className="flex h-full w-full items-center justify-center bg-slate-700">
            {mediaType === 'tv' ? (
              <Tv size={20} className="text-slate-500" />
            ) : (
              <Film size={20} className="text-slate-500" />
            )}
          </div>
        )}

        {/* Hover overlay */}
        <div className="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity" />

        {/* Rating badge */}
        {rating > 0 && (
          <div className="absolute top-1 left-1 flex items-center gap-0.5 rounded bg-black/70 px-1 py-0.5">
            <Star size={9} className="fill-yellow-400 text-yellow-400" />
            <span className="text-[9px] font-semibold text-yellow-400">{rating.toFixed(1)}</span>
          </div>
        )}

        {/* Media type badge */}
        {mediaType && (
          <div className="absolute top-1 right-1 rounded bg-black/70 px-1 py-0.5 text-[9px] font-semibold uppercase text-slate-300">
            {mediaType}
          </div>
        )}

        {/* Add button on hover */}
        {onAdd && (
          <button
            onClick={(e) => { e.stopPropagation(); onAdd() }}
            className="absolute bottom-1.5 right-1.5 rounded-full bg-blue-600 p-1 opacity-0 group-hover:opacity-100 transition-opacity hover:bg-blue-500"
            title="Add to library"
          >
            <Plus size={12} className="text-white" />
          </button>
        )}
      </button>

      {/* Title and year below poster */}
      <div className="mt-1 px-0.5">
        <div className="text-[11px] font-medium text-white truncate leading-tight">{title}</div>
        {year && <div className="text-[10px] text-slate-400">{year}</div>}
      </div>
    </div>
  )
})
