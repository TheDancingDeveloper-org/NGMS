import { useParams, useNavigate } from 'react-router-dom'
import {
  ArrowLeft,
  Search,
  Trash2,
  CheckCircle,
  XCircle,
  Film,
  Loader2,
} from 'lucide-react'
import { useMovieDetail, useDeleteMovie, useSearchMovie } from '../hooks/useApi'

export default function MovieDetail() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const movieId = Number(id) || 0
  const { data: movie, isLoading, error } = useMovieDetail(movieId)
  const deleteMutation = useDeleteMovie()
  const searchMutation = useSearchMovie()

  const handleDelete = () => {
    if (confirm('Are you sure you want to delete this movie?')) {
      deleteMutation.mutate(movieId, {
        onSuccess: () => navigate('/movies'),
      })
    }
  }

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 size={32} className="animate-spin text-blue-500" />
      </div>
    )
  }

  if (error || !movie) {
    return (
      <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
        {error ? `Failed to load movie: ${error.message}` : 'Movie not found'}
      </div>
    )
  }

  return (
    <div>
      {/* Back */}
      <button
        onClick={() => navigate('/movies')}
        className="mb-4 flex items-center gap-1 text-sm text-slate-400 hover:text-white transition-colors"
      >
        <ArrowLeft size={16} /> Back to Movies
      </button>

      {/* Header */}
      <div className="flex flex-col gap-6 md:flex-row">
        {/* Poster */}
        {movie.posterUrl ? (
          <img
            src={movie.posterUrl}
            alt={movie.title}
            className="h-80 w-56 shrink-0 rounded-lg object-cover"
          />
        ) : (
          <div className="flex h-80 w-56 shrink-0 items-center justify-center rounded-lg bg-slate-800">
            <Film size={48} className="text-slate-600" />
          </div>
        )}

        <div className="flex-1">
          <div className="flex flex-wrap items-start gap-3">
            <h2 className="text-3xl font-bold">{movie.title}</h2>
            <span className="mt-1 rounded-full bg-slate-700 px-2.5 py-0.5 text-xs font-medium text-slate-300">
              {movie.year}
            </span>
          </div>

          {movie.studio && (
            <div className="mt-1 text-sm text-slate-400">{movie.studio}</div>
          )}

          {movie.overview && (
            <p className="mt-3 text-sm text-slate-300 leading-relaxed">{movie.overview}</p>
          )}

          {/* File status */}
          <div className="mt-6 rounded-lg bg-slate-800 p-4">
            <div className="flex items-center gap-3">
              {movie.hasFile ? (
                <>
                  <CheckCircle size={20} className="text-green-500" />
                  <div>
                    <div className="font-medium text-green-400">File Available</div>
                    {movie.movieFile && (
                      <div className="mt-0.5 text-sm text-slate-400">
                        {movie.movieFile.relativePath} &middot;{' '}
                        {(movie.movieFile.size / 1073741824).toFixed(2)} GB &middot;{' '}
                        <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-xs font-medium text-blue-400">
                          {movie.movieFile.quality}
                        </span>
                      </div>
                    )}
                  </div>
                </>
              ) : (
                <>
                  <XCircle size={20} className="text-red-500" />
                  <div className="font-medium text-red-400">No file available</div>
                </>
              )}
            </div>
          </div>

          {/* Actions */}
          <div className="mt-4 flex flex-wrap gap-2">
            <button
              onClick={() => searchMutation.mutate(movieId)}
              disabled={searchMutation.isPending}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50 transition-colors"
            >
              <Search size={16} />
              {searchMutation.isPending ? 'Searching...' : 'Search'}
            </button>
            <button
              onClick={handleDelete}
              disabled={deleteMutation.isPending}
              className="flex items-center gap-1.5 rounded-lg bg-red-600/20 px-4 py-2 text-sm font-medium text-red-400 hover:bg-red-600/30 transition-colors"
            >
              <Trash2 size={16} /> Delete
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
