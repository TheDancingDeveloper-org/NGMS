// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState } from 'react'
import {
  Search,
  FolderOpen,
  Clock,
  Play,
  Trash2,
  Copy,
  File,
  Folder,
  AlertTriangle,
  Loader2,
  Radio,
} from 'lucide-react'
import {
  useDavItems,
  useDavStatus,
  useDavHistory,
  useDavStream,
  useDavDeleteItem,
  useSearchReleases,
} from '../hooks/useApi'
import type { DavItem, FreehandSearchResult } from '../api/types'

type Tab = 'search' | 'browse' | 'history'

export default function LiveStream() {
  const [tab, setTab] = useState<Tab>('search')

  return (
    <div className="mx-auto max-w-6xl p-6">
      <div className="mb-6 flex items-center gap-3">
        <Radio className="h-6 w-6 text-sky-400" />
        <h1 className="text-2xl font-bold text-white">Live Stream</h1>
        <StatusPill />
      </div>

      {/* Tab bar */}
      <div className="mb-6 flex gap-1 rounded-lg bg-slate-800 p-1">
        {([
          { id: 'search' as Tab, icon: Search, label: 'Search' },
          { id: 'browse' as Tab, icon: FolderOpen, label: 'Browse' },
          { id: 'history' as Tab, icon: Clock, label: 'History' },
        ]).map(({ id, icon: Icon, label }) => (
          <button
            key={id}
            onClick={() => setTab(id)}
            className={`flex flex-1 items-center justify-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition ${
              tab === id
                ? 'bg-slate-700 text-white'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            <Icon size={16} />
            {label}
          </button>
        ))}
      </div>

      {tab === 'search' && <SearchTab />}
      {tab === 'browse' && <BrowseTab />}
      {tab === 'history' && <HistoryTab />}
    </div>
  )
}

// ── Status pill ────────────────────────────────────────────────────────────

function StatusPill() {
  const { data } = useDavStatus()
  if (!data?.enabled) {
    return (
      <span className="rounded-full bg-slate-700 px-2 py-0.5 text-xs text-slate-400">
        Disabled
      </span>
    )
  }
  return (
    <span className="rounded-full bg-sky-900/50 px-2 py-0.5 text-xs text-sky-300">
      {data.providerConnections} connections &middot; {data.itemsCount} items
    </span>
  )
}

// ── Search tab ─────────────────────────────────────────────────────────────

function SearchTab() {
  const [query, setQuery] = useState('')
  const [submitted, setSubmitted] = useState('')
  const { data: results, isLoading } = useSearchReleases(submitted)
  const streamMutation = useDavStream()

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault()
    if (query.trim()) setSubmitted(query.trim())
  }

  const handleStream = (result: FreehandSearchResult) => {
    const url = result.nzbUrl || result.downloadUrl || ''
    if (!url) return
    streamMutation.mutate({
      nzbUrl: url,
      name: result.title,
    })
  }

  return (
    <div>
      <form onSubmit={handleSearch} className="mb-4 flex gap-2">
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search indexers for streamable content..."
          className="flex-1 rounded-lg border border-slate-700 bg-slate-800 px-4 py-2 text-white placeholder-slate-500 focus:border-sky-500 focus:outline-none"
        />
        <button
          type="submit"
          disabled={isLoading || !query.trim()}
          className="rounded-lg bg-sky-600 px-6 py-2 text-sm font-medium text-white hover:bg-sky-500 disabled:opacity-50"
        >
          {isLoading ? <Loader2 size={16} className="animate-spin" /> : 'Search'}
        </button>
      </form>

      {streamMutation.isPending && (
        <div className="mb-4 flex items-center gap-2 rounded-lg bg-sky-900/30 px-4 py-3 text-sky-300">
          <Loader2 size={16} className="animate-spin" />
          Processing NZB... This may take a few seconds.
        </div>
      )}

      {streamMutation.isSuccess && (
        <div className="mb-4 rounded-lg bg-green-900/30 px-4 py-3 text-green-300">
          Stream ready at <code className="text-green-200">{streamMutation.data.davPath}</code>
          &mdash; {streamMutation.data.itemsCreated} files available
        </div>
      )}

      {streamMutation.isError && (
        <div className="mb-4 rounded-lg bg-red-900/30 px-4 py-3 text-red-300">
          Failed: {streamMutation.error.message}
        </div>
      )}

      {results && results.length > 0 && (
        <div className="overflow-hidden rounded-lg border border-slate-700">
          <table className="w-full text-sm">
            <thead className="bg-slate-800 text-slate-400">
              <tr>
                <th className="px-4 py-2 text-left">Title</th>
                <th className="px-4 py-2 text-left">Size</th>
                <th className="px-4 py-2 text-left">Indexer</th>
                <th className="px-4 py-2 text-right">Action</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-700">
              {results.map((r, i) => (
                <tr key={i} className="hover:bg-slate-800/50">
                  <td className="px-4 py-2 text-white">{r.title}</td>
                  <td className="px-4 py-2 text-slate-400">
                    {r.size ? formatBytes(r.size) : '-'}
                  </td>
                  <td className="px-4 py-2 text-slate-400">{r.indexerName}</td>
                  <td className="px-4 py-2 text-right">
                    {(r.nzbUrl || r.downloadUrl) && r.protocol === 'usenet' ? (
                      <button
                        onClick={() => handleStream(r)}
                        disabled={streamMutation.isPending}
                        className="inline-flex items-center gap-1 rounded bg-sky-600 px-3 py-1 text-xs font-medium text-white hover:bg-sky-500 disabled:opacity-50"
                      >
                        <Play size={12} />
                        Stream
                      </button>
                    ) : (
                      <span className="text-xs text-slate-600">
                        {r.protocol === 'torrent' ? 'Torrent' : 'N/A'}
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {submitted && results && results.length === 0 && !isLoading && (
        <p className="text-center text-slate-500">No results found.</p>
      )}
    </div>
  )
}

// ── Browse tab ─────────────────────────────────────────────────────────────

function BrowseTab() {
  const [path, setPath] = useState('/content/')
  const { data: items, isLoading } = useDavItems(path)
  const deleteMutation = useDavDeleteItem()

  const navigate = (item: DavItem) => {
    if (item.isDirectory) {
      setPath(item.path.endsWith('/') ? item.path : `${item.path}/`)
    }
  }

  const goUp = () => {
    const parts = path.replace(/\/$/, '').split('/')
    if (parts.length > 2) {
      parts.pop()
      setPath(parts.join('/') + '/')
    }
  }

  const copyDavUrl = (item: DavItem) => {
    const url = `${window.location.origin}/dav${item.path}`
    navigator.clipboard.writeText(url)
  }

  const hoursUntilExpiry = (createdAt: string) => {
    const created = new Date(createdAt)
    const expiresAt = new Date(created.getTime() + 24 * 60 * 60 * 1000)
    const hours = (expiresAt.getTime() - Date.now()) / (1000 * 60 * 60)
    return Math.max(0, hours)
  }

  return (
    <div>
      <div className="mb-4 rounded-lg bg-slate-800 px-4 py-2 text-sm text-slate-400">
        Streamed content is automatically removed after 24 hours.
      </div>

      <div className="mb-3 flex items-center gap-2 text-sm text-slate-400">
        <button onClick={goUp} className="hover:text-white" disabled={path === '/content/'}>
          ..
        </button>
        <span className="text-slate-600">/</span>
        <span className="text-white">{path}</span>
      </div>

      {isLoading ? (
        <div className="flex justify-center py-8">
          <Loader2 size={24} className="animate-spin text-slate-500" />
        </div>
      ) : items && items.length > 0 ? (
        <div className="overflow-hidden rounded-lg border border-slate-700">
          <table className="w-full text-sm">
            <thead className="bg-slate-800 text-slate-400">
              <tr>
                <th className="px-4 py-2 text-left">Name</th>
                <th className="px-4 py-2 text-left">Size</th>
                <th className="px-4 py-2 text-left">Expires</th>
                <th className="px-4 py-2 text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-700">
              {items.map((item) => {
                const hours = hoursUntilExpiry(item.createdAt)
                return (
                  <tr key={item.id} className="hover:bg-slate-800/50">
                    <td className="px-4 py-2">
                      <button
                        onClick={() => navigate(item)}
                        className="flex items-center gap-2 text-white hover:text-sky-400"
                      >
                        {item.isDirectory ? (
                          <Folder size={16} className="text-amber-400" />
                        ) : (
                          <File size={16} className="text-slate-400" />
                        )}
                        {item.name}
                      </button>
                    </td>
                    <td className="px-4 py-2 text-slate-400">
                      {item.fileSize ? formatBytes(item.fileSize) : '-'}
                    </td>
                    <td className="px-4 py-2">
                      {!item.isDirectory && (
                        <span
                          className={`rounded-full px-2 py-0.5 text-xs ${
                            hours < 2
                              ? 'bg-amber-900/50 text-amber-300'
                              : 'text-slate-500'
                          }`}
                        >
                          {hours < 2 && <AlertTriangle size={10} className="mr-1 inline" />}
                          {hours.toFixed(0)}h
                        </span>
                      )}
                    </td>
                    <td className="px-4 py-2 text-right">
                      <div className="flex justify-end gap-1">
                        {!item.isDirectory && (
                          <button
                            onClick={() => copyDavUrl(item)}
                            className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-white"
                            title="Copy DAV URL"
                          >
                            <Copy size={14} />
                          </button>
                        )}
                        <button
                          onClick={() => deleteMutation.mutate(item.id)}
                          className="rounded p-1 text-slate-400 hover:bg-red-900/50 hover:text-red-400"
                          title="Delete"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="py-8 text-center text-slate-500">No items in this directory.</p>
      )}
    </div>
  )
}

// ── History tab ─────────────────────────────────────────────────────────────

function HistoryTab() {
  const { data: history, isLoading } = useDavHistory()

  return (
    <div>
      {isLoading ? (
        <div className="flex justify-center py-8">
          <Loader2 size={24} className="animate-spin text-slate-500" />
        </div>
      ) : history && history.length > 0 ? (
        <div className="overflow-hidden rounded-lg border border-slate-700">
          <table className="w-full text-sm">
            <thead className="bg-slate-800 text-slate-400">
              <tr>
                <th className="px-4 py-2 text-left">Name</th>
                <th className="px-4 py-2 text-left">Status</th>
                <th className="px-4 py-2 text-left">Size</th>
                <th className="px-4 py-2 text-left">Time</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-700">
              {history.map((h) => (
                <tr key={h.id} className="hover:bg-slate-800/50">
                  <td className="px-4 py-2 text-white">{h.jobName}</td>
                  <td className="px-4 py-2">
                    <span
                      className={`rounded-full px-2 py-0.5 text-xs ${
                        h.downloadStatus === 1
                          ? 'bg-green-900/50 text-green-300'
                          : 'bg-red-900/50 text-red-300'
                      }`}
                    >
                      {h.downloadStatus === 1 ? 'Completed' : 'Failed'}
                    </span>
                  </td>
                  <td className="px-4 py-2 text-slate-400">
                    {formatBytes(h.totalSegmentBytes)}
                  </td>
                  <td className="px-4 py-2 text-slate-400">
                    {new Date(h.createdAt).toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <p className="py-8 text-center text-slate-500">No streaming history yet.</p>
      )}
    </div>
  )
}

// ── Helpers ─────────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B'
  const k = 1024
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`
}
