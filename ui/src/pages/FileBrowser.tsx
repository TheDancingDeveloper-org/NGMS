// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState, useEffect, useCallback } from 'react'
import {
  Folder,
  File,
  Trash2,
  ChevronRight,
  ArrowUp,
  Loader2,
  HardDrive,
  RefreshCw,
  AlertTriangle,
  Download,
} from 'lucide-react'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface BrowseEntry {
  name: string
  path: string
  isDir: boolean
  size: number
  modified: number | null
}

interface BrowseResponse {
  path: string
  entries: BrowseEntry[]
  parent: string | null
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatSize(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`
}

function formatDate(ts: number | null): string {
  if (!ts) return '-'
  const d = new Date(ts)
  return d.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function FileBrowser() {
  const [currentPath, setCurrentPath] = useState<string | null>(null)
  const [entries, setEntries] = useState<BrowseEntry[]>([])
  const [parentPath, setParentPath] = useState<string | null>(null)
  const [displayPath, setDisplayPath] = useState('/')
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null)
  const [deleting, setDeleting] = useState<string | null>(null)
  const [selected, setSelected] = useState<Set<string>>(new Set())

  const fetchDir = useCallback(async (path: string | null) => {
    setLoading(true)
    setError(null)
    setSelected(new Set())
    try {
      const url = path
        ? `/api/v1/filebrowser/browse?path=${encodeURIComponent(path)}`
        : '/api/v1/filebrowser/browse'
      const res = await fetch(url)
      if (!res.ok) {
        const body = await res.json().catch(() => ({})) as { error?: string }
        throw new Error(body.error ?? `HTTP ${res.status}`)
      }
      const data = (await res.json()) as BrowseResponse
      setEntries(data.entries)
      setParentPath(data.parent)
      setDisplayPath(data.path)
      setCurrentPath(path)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to browse')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void fetchDir(null)
  }, [fetchDir])

  const navigate = (path: string) => void fetchDir(path)

  const goUp = () => {
    if (parentPath) {
      void fetchDir(parentPath)
    } else {
      void fetchDir(null)
    }
  }

  const handleDelete = async (path: string) => {
    setDeleting(path)
    try {
      const res = await fetch('/api/v1/filebrowser/delete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path }),
      })
      if (!res.ok) {
        const body = (await res.json().catch(() => ({}))) as { error?: string }
        setError(body.error ?? 'Delete failed')
      }
      setDeleteConfirm(null)
      void fetchDir(currentPath)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Delete failed')
    } finally {
      setDeleting(null)
    }
  }

  const handleBulkDelete = async () => {
    for (const path of selected) {
      await handleDelete(path)
    }
    setSelected(new Set())
  }

  const toggleSelect = (path: string) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })
  }

  const toggleAll = () => {
    if (selected.size === entries.length) {
      setSelected(new Set())
    } else {
      setSelected(new Set(entries.map((e) => e.path)))
    }
  }

  const isRoot = currentPath === null

  return (
    <div>
      {/* Header */}
      <div className="mb-6 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <HardDrive size={24} className="text-blue-400" />
          <h2 className="text-2xl font-bold">File Browser</h2>
        </div>
        <div className="flex items-center gap-2">
          {selected.size > 0 && (
            <button
              onClick={() => void handleBulkDelete()}
              className="flex items-center gap-1.5 rounded-lg bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-500 transition-colors"
            >
              <Trash2 size={14} />
              Delete {selected.size} item{selected.size > 1 ? 's' : ''}
            </button>
          )}
          <button
            onClick={() => void fetchDir(currentPath)}
            className="rounded-lg bg-slate-700 p-2 text-slate-400 hover:text-white transition-colors"
            title="Refresh"
          >
            <RefreshCw size={16} />
          </button>
        </div>
      </div>

      {/* Breadcrumb / path bar */}
      <div className="mb-4 flex items-center gap-2 rounded-lg bg-slate-800 px-4 py-2.5 text-sm">
        {!isRoot && (
          <button
            onClick={goUp}
            className="flex items-center gap-1 rounded px-2 py-1 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
            title="Go up"
          >
            <ArrowUp size={14} />
          </button>
        )}
        <span className="text-slate-400 font-mono text-xs truncate">{displayPath}</span>
      </div>

      {/* Error */}
      {error && (
        <div className="mb-4 flex items-center gap-2 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">
          <AlertTriangle size={16} />
          {error}
          <button
            onClick={() => setError(null)}
            className="ml-auto text-red-400 hover:text-red-300"
          >
            &times;
          </button>
        </div>
      )}

      {/* Loading */}
      {loading && (
        <div className="flex items-center justify-center py-20">
          <Loader2 size={32} className="animate-spin text-blue-500" />
        </div>
      )}

      {/* Empty state */}
      {!loading && !error && entries.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <Folder size={48} className="mb-4 text-slate-600" />
          <p className="text-lg font-medium">
            {isRoot ? 'No download directories configured' : 'Empty directory'}
          </p>
        </div>
      )}

      {/* File listing */}
      {!loading && entries.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                <th className="w-10 px-4 py-3">
                  {!isRoot && (
                    <input
                      type="checkbox"
                      checked={selected.size === entries.length && entries.length > 0}
                      onChange={toggleAll}
                      className="rounded border-slate-600 bg-slate-900"
                    />
                  )}
                </th>
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium w-32">Size</th>
                <th className="px-4 py-3 font-medium w-44">Modified</th>
                <th className="px-4 py-3 font-medium w-20">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-700/50">
              {entries.map((entry) => (
                <tr
                  key={entry.path}
                  className={`hover:bg-slate-700/30 transition-colors ${
                    selected.has(entry.path) ? 'bg-slate-700/20' : ''
                  }`}
                >
                  <td className="px-4 py-2.5">
                    {!isRoot && (
                      <input
                        type="checkbox"
                        checked={selected.has(entry.path)}
                        onChange={() => toggleSelect(entry.path)}
                        className="rounded border-slate-600 bg-slate-900"
                      />
                    )}
                  </td>
                  <td className="px-4 py-2.5">
                    {entry.isDir ? (
                      <button
                        onClick={() => navigate(entry.path)}
                        className="flex items-center gap-2 text-white hover:text-blue-400 transition-colors"
                      >
                        <Folder size={16} className="shrink-0 text-yellow-500" />
                        <span className="truncate">{entry.name}</span>
                        <ChevronRight size={14} className="shrink-0 text-slate-500" />
                      </button>
                    ) : (
                      <div className="flex items-center gap-2 text-slate-300">
                        <File size={16} className="shrink-0 text-slate-500" />
                        <span className="truncate">{entry.name}</span>
                      </div>
                    )}
                  </td>
                  <td className="px-4 py-2.5 text-slate-400 tabular-nums">
                    {formatSize(entry.size)}
                  </td>
                  <td className="px-4 py-2.5 text-slate-400 text-xs">
                    {formatDate(entry.modified)}
                  </td>
                  <td className="px-4 py-2.5">
                    {!isRoot && (
                      <div className="flex items-center gap-1">
                        {!entry.isDir && deleteConfirm !== entry.path && (
                          <a
                            href={`/api/v1/filebrowser/download?path=${encodeURIComponent(entry.path)}`}
                            className="rounded p-1 text-slate-400 hover:bg-blue-500/20 hover:text-blue-400 transition-colors"
                            title={`Download ${entry.name}`}
                          >
                            <Download size={14} />
                          </a>
                        )}
                        {deleteConfirm === entry.path ? (
                          <>
                            <button
                              onClick={() => void handleDelete(entry.path)}
                              disabled={deleting === entry.path}
                              className="rounded bg-red-600 px-2 py-0.5 text-xs text-white hover:bg-red-500 disabled:opacity-50"
                            >
                              {deleting === entry.path ? '...' : 'Yes'}
                            </button>
                            <button
                              onClick={() => setDeleteConfirm(null)}
                              className="rounded bg-slate-600 px-2 py-0.5 text-xs text-slate-300 hover:bg-slate-500"
                            >
                              No
                            </button>
                          </>
                        ) : (
                          <button
                            onClick={() => setDeleteConfirm(entry.path)}
                            className="rounded p-1 text-slate-400 hover:bg-red-500/20 hover:text-red-400 transition-colors"
                            title={`Delete ${entry.name}`}
                          >
                            <Trash2 size={14} />
                          </button>
                        )}
                      </div>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
