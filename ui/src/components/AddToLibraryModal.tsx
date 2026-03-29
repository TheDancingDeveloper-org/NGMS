import { useState } from 'react'
import { Loader2, Plus, X, Check } from 'lucide-react'
import { useAddSeries, useAddMovie } from '../hooks/useApi'
import { tmdbPosterUrl } from '../api/types'

export interface AddTarget {
  id: number
  title: string
  year: string
  mediaType: 'movie' | 'tv'
  posterPath: string | null
}

export default function AddToLibraryModal({ target, onClose }: { target: AddTarget; onClose: () => void }) {
  const addSeries = useAddSeries()
  const addMovie = useAddMovie()
  const [added, setAdded] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const posterUrl = tmdbPosterUrl(target.posterPath, 'w342')
  const isPending = addSeries.isPending || addMovie.isPending

  const handleAdd = () => {
    setError(null)
    const year = parseInt(target.year, 10) || 0

    if (target.mediaType === 'tv') {
      addSeries.mutate(
        { title: target.title, year },
        {
          onSuccess: () => setAdded(true),
          onError: (e) => setError(e instanceof Error ? e.message : 'Failed to add'),
        },
      )
    } else {
      addMovie.mutate(
        { title: target.title, year },
        {
          onSuccess: () => setAdded(true),
          onError: (e) => setError(e instanceof Error ? e.message : 'Failed to add'),
        },
      )
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="w-full max-w-sm rounded-xl bg-slate-800 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start gap-4 p-6">
          {posterUrl ? (
            <img src={posterUrl} alt={target.title} className="h-28 w-[75px] shrink-0 rounded-md object-cover" />
          ) : (
            <div className="flex h-28 w-[75px] shrink-0 items-center justify-center rounded-md bg-slate-700 text-slate-500 text-xs">
              No poster
            </div>
          )}
          <div className="flex-1 min-w-0">
            <h3 className="text-lg font-semibold text-white truncate">{target.title}</h3>
            <p className="text-sm text-slate-400">
              {target.year} &middot; {target.mediaType === 'tv' ? 'TV Series' : 'Movie'}
            </p>

            {added ? (
              <div className="mt-4 flex items-center gap-2 text-green-400 text-sm font-medium">
                <Check size={16} /> Added to library
              </div>
            ) : (
              <div className="mt-4 flex gap-2">
                <button
                  onClick={handleAdd}
                  disabled={isPending}
                  className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  {isPending ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <Plus size={14} />
                  )}
                  Add to Library
                </button>
                <button
                  onClick={onClose}
                  className="rounded-lg bg-slate-700 px-3 py-2 text-sm text-slate-300 hover:bg-slate-600 transition-colors"
                >
                  <X size={14} />
                </button>
              </div>
            )}

            {error && (
              <p className="mt-2 text-xs text-red-400">{error}</p>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
