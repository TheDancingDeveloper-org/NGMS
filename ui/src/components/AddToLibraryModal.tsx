// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Loader2, Plus, X, Check, FolderOpen } from 'lucide-react'
import { useAddSeries, useAddMovie, useMediaLibraryFolders } from '../hooks/useApi'
import { tmdbPosterUrl } from '../api/types'

export interface AddTarget {
  id: number
  title: string
  year: string
  mediaType: 'movie' | 'tv'
  posterPath: string | null
}

function buildDefaultFolderName(title: string, year: string): string {
  const y = parseInt(year, 10)
  return y > 0 ? `${title} (${y})` : title
}

export default function AddToLibraryModal({ target, onClose }: { target: AddTarget; onClose: () => void }) {
  const navigate = useNavigate()
  const addSeries = useAddSeries()
  const addMovie = useAddMovie()
  const { data: allFolders } = useMediaLibraryFolders()
  const [added, setAdded] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [selectedFolder, setSelectedFolder] = useState<string>('')
  const [folderName, setFolderName] = useState(() => buildDefaultFolderName(target.title, target.year))

  const mediaType = target.mediaType === 'tv' ? 'tv' : 'movie'
  const folders = allFolders?.filter(f => f.mediaType === mediaType) ?? []

  const effectiveFolder = selectedFolder || folders[0]?.path || ''
  const fullPath = effectiveFolder
    ? `${effectiveFolder.replace(/\/$/, '')}/${folderName}`
    : ''

  const posterUrl = tmdbPosterUrl(target.posterPath, 'w342')
  const isPending = addSeries.isPending || addMovie.isPending
  const canAdd = fullPath.length > 0

  const handleAdd = () => {
    if (!canAdd) return
    setError(null)
    const year = parseInt(target.year, 10) || 0

    if (target.mediaType === 'tv') {
      addSeries.mutate(
        { title: target.title, tmdbId: target.id, year, path: fullPath },
        {
          onSuccess: (data) => { setAdded(true); onClose(); navigate(`/series/${data.id}`) },
          onError: (e) => setError(e instanceof Error ? e.message : 'Failed to add'),
        },
      )
    } else {
      addMovie.mutate(
        { title: target.title, tmdbId: target.id, year, path: fullPath },
        {
          onSuccess: (data) => { setAdded(true); onClose(); navigate(`/movies/${data.id}`) },
          onError: (e) => setError(e instanceof Error ? e.message : 'Failed to add'),
        },
      )
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div
        className="w-full max-w-md rounded-xl bg-slate-800 shadow-2xl"
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
              <div className="mt-3 space-y-2">
                {/* Root folder picker */}
                <div>
                  <label className="mb-1 block text-xs font-medium text-slate-400">Root Folder</label>
                  {folders.length === 0 ? (
                    <p className="text-xs text-amber-400">
                      No {mediaType === 'tv' ? 'TV' : 'movie'} media folders configured — add one in Settings → Media Management.
                    </p>
                  ) : (
                    <select
                      value={selectedFolder || folders[0]?.path || ''}
                      onChange={(e) => setSelectedFolder(e.target.value)}
                      className="w-full rounded-lg border border-slate-600 bg-slate-700 px-2 py-1.5 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                    >
                      {folders.map((f) => (
                        <option key={f.id} value={f.path}>{f.path}</option>
                      ))}
                    </select>
                  )}
                </div>

                {/* Subfolder name */}
                <div>
                  <label className="mb-1 block text-xs font-medium text-slate-400">Folder Name</label>
                  <input
                    type="text"
                    value={folderName}
                    onChange={(e) => setFolderName(e.target.value)}
                    className="w-full rounded-lg border border-slate-600 bg-slate-700 px-2 py-1.5 text-sm text-white focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
                  />
                </div>

                {/* Full path preview */}
                {fullPath && (
                  <div className="flex items-center gap-1.5 rounded-md bg-slate-700/50 px-2 py-1.5">
                    <FolderOpen size={13} className="shrink-0 text-slate-400" />
                    <span className="truncate text-xs text-slate-300 font-mono">{fullPath}</span>
                  </div>
                )}

                <div className="flex gap-2 pt-1">
                  <button
                    onClick={handleAdd}
                    disabled={isPending || !canAdd}
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
