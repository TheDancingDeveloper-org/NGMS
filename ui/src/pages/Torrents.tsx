import { useState, useEffect, useCallback, useRef } from 'react'
import {
  Magnet,
  Loader2,
  Pause,
  Play,
  Trash2,
  Plus,
  Search,
  ChevronDown,
  ChevronRight,
  ArrowUpDown,
  X,
  Upload,
} from 'lucide-react'

// ── Types ──────────────────────────────────────────────────────────────────

interface TorrentStats {
  downloadSpeed: number
  uploadSpeed: number
  activeCount: number
  pausedCount: number
}

interface TorrentItem {
  id: string
  name: string
  size: number
  progress: number
  downloadSpeed: number
  uploadSpeed: number
  seeds: number
  peers: number
  eta: number // seconds
  status: string
  category?: string
  files?: TorrentFile[]
  trackers?: string[]
  peerList?: TorrentPeer[]
}

interface TorrentFile {
  name: string
  size: number
  progress: number
}

interface TorrentPeer {
  ip: string
  client: string
  downloadSpeed: number
  uploadSpeed: number
}

type SortField = 'name' | 'size' | 'progress' | 'downloadSpeed' | 'uploadSpeed' | 'seeds' | 'peers' | 'eta' | 'status'
type SortDir = 'asc' | 'desc'

// ── Helpers ────────────────────────────────────────────────────────────────

function formatSpeed(bytesPerSec: number): string {
  if (!bytesPerSec || !isFinite(bytesPerSec) || bytesPerSec <= 0) return '0 KB/s'
  if (bytesPerSec < 1024) return `${bytesPerSec} B/s`
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`
  if (bytesPerSec < 1024 * 1024 * 1024) return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`
  return `${(bytesPerSec / (1024 * 1024 * 1024)).toFixed(1)} GB/s`
}

function formatSize(bytes: number): string {
  if (!bytes || !isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

function formatEta(seconds: number): string {
  if (seconds <= 0) return '-'
  if (seconds < 60) return '< 1m'
  const mins = Math.floor(seconds / 60)
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ${mins % 60}m`
  return `${Math.floor(hours / 24)}d ${hours % 24}h`
}

function statusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'downloading': return 'bg-blue-500'
    case 'seeding': return 'bg-green-500'
    case 'paused': return 'bg-yellow-500'
    case 'checking': return 'bg-purple-500'
    case 'error': return 'bg-red-500'
    default: return 'bg-slate-500'
  }
}

function statusBadgeColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'downloading': return 'bg-blue-500/20 text-blue-400'
    case 'seeding': return 'bg-green-500/20 text-green-400'
    case 'paused': return 'bg-yellow-500/20 text-yellow-400'
    case 'checking': return 'bg-purple-500/20 text-purple-400'
    case 'error': return 'bg-red-500/20 text-red-400'
    default: return 'bg-slate-600 text-slate-300'
  }
}

// ── Component ──────────────────────────────────────────────────────────────

export default function Torrents() {
  const [stats, setStats] = useState<TorrentStats | null>(null)
  const [torrents, setTorrents] = useState<TorrentItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [search, setSearch] = useState('')
  const [sortField, setSortField] = useState<SortField>('name')
  const [sortDir, setSortDir] = useState<SortDir>('asc')
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [showAddModal, setShowAddModal] = useState(false)
  const [deleteConfirm, setDeleteConfirm] = useState<{ id: string; name: string } | null>(null)
  const [deleteFiles, setDeleteFiles] = useState(false)

  // ── Fetching ──

  const fetchData = useCallback(async () => {
    try {
      const [statsRes, listRes] = await Promise.all([
        fetch('/api/v1/torrent/status'),
        fetch('/api/v1/torrent/list'),
      ])
      if (statsRes.ok) setStats(await statsRes.json() as TorrentStats)
      if (listRes.ok) {
        const data = await listRes.json() as { torrents?: TorrentItem[] }
        setTorrents(data.torrents ?? [])
        setError(null)
      } else {
        setError(`Failed to fetch torrents (${listRes.status})`)
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Network error')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void fetchData()
    const interval = setInterval(() => void fetchData(), 3000)
    return () => clearInterval(interval)
  }, [fetchData])

  // ── Actions ──

  const pauseAll = async () => {
    for (const t of torrents) {
      if (t.status.toLowerCase() !== 'paused') {
        await fetch(`/api/v1/torrent/${t.id}/pause`, { method: 'POST' })
      }
    }
    void fetchData()
  }

  const resumeAll = async () => {
    for (const t of torrents) {
      if (t.status.toLowerCase() === 'paused') {
        await fetch(`/api/v1/torrent/${t.id}/resume`, { method: 'POST' })
      }
    }
    void fetchData()
  }

  const togglePause = async (id: string, status: string) => {
    const endpoint = status.toLowerCase() === 'paused' ? 'resume' : 'pause'
    await fetch(`/api/v1/torrent/${id}/${endpoint}`, { method: 'POST' })
    void fetchData()
  }

  const deleteTorrent = async () => {
    if (!deleteConfirm) return
    await fetch(`/api/v1/torrent/${deleteConfirm.id}/delete?deleteFiles=${deleteFiles}`, { method: 'POST' })
    setDeleteConfirm(null)
    setDeleteFiles(false)
    void fetchData()
  }

  // ── Sorting / filtering ──

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortField(field)
      setSortDir('asc')
    }
  }

  const filtered = torrents
    .filter((t) => t.name.toLowerCase().includes(search.toLowerCase()))
    .sort((a, b) => {
      const va = a[sortField]
      const vb = b[sortField]
      const cmp = typeof va === 'string' ? (va as string).localeCompare(vb as string) : (va as number) - (vb as number)
      return sortDir === 'asc' ? cmp : -cmp
    })

  // ── Render ───

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 size={32} className="animate-spin text-blue-500" />
      </div>
    )
  }

  return (
    <div>
      {/* Header */}
      <h2 className="mb-6 text-2xl font-bold">Torrents</h2>

      {/* Stats bar */}
      {stats && (
        <div className="mb-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
          <StatCard label="Download" value={formatSpeed(stats.downloadSpeed)} />
          <StatCard label="Upload" value={formatSpeed(stats.uploadSpeed)} />
          <StatCard label="Active" value={String(stats.activeCount)} />
          <StatCard label="Paused" value={String(stats.pausedCount)} />
        </div>
      )}

      {/* Toolbar */}
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <button
          onClick={() => setShowAddModal(true)}
          className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
        >
          <Plus size={16} /> Add Torrent
        </button>
        <button
          onClick={() => void pauseAll()}
          className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
        >
          <Pause size={16} /> Pause All
        </button>
        <button
          onClick={() => void resumeAll()}
          className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
        >
          <Play size={16} /> Resume All
        </button>
        <div className="relative ml-auto">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
          <input
            type="text"
            placeholder="Filter torrents..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="rounded-lg bg-slate-800 py-2 pl-9 pr-3 text-sm text-white placeholder-slate-500 outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors w-64"
          />
        </div>
      </div>

      {/* Error */}
      {error && (
        <div className="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
          {error}
        </div>
      )}

      {/* Empty state */}
      {!error && filtered.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <Magnet size={48} className="mb-4 text-slate-600" />
          <p>{search ? 'No torrents match your filter' : 'No torrents'}</p>
        </div>
      )}

      {/* Torrent table */}
      {filtered.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                <th className="w-8 px-2 py-3" />
                <SortHeader field="name" label="Name" current={sortField} dir={sortDir} onSort={handleSort} />
                <SortHeader field="size" label="Size" current={sortField} dir={sortDir} onSort={handleSort} />
                <SortHeader field="progress" label="Progress" current={sortField} dir={sortDir} onSort={handleSort} className="w-44" />
                <SortHeader field="downloadSpeed" label="Down" current={sortField} dir={sortDir} onSort={handleSort} />
                <SortHeader field="uploadSpeed" label="Up" current={sortField} dir={sortDir} onSort={handleSort} />
                <SortHeader field="seeds" label="Seeds" current={sortField} dir={sortDir} onSort={handleSort} />
                <SortHeader field="peers" label="Peers" current={sortField} dir={sortDir} onSort={handleSort} />
                <SortHeader field="eta" label="ETA" current={sortField} dir={sortDir} onSort={handleSort} />
                <SortHeader field="status" label="Status" current={sortField} dir={sortDir} onSort={handleSort} />
                <th className="px-4 py-3 font-medium">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-700">
              {filtered.map((t) => (
                <TorrentRow
                  key={t.id}
                  torrent={t}
                  expanded={expandedId === t.id}
                  onToggleExpand={() => setExpandedId(expandedId === t.id ? null : t.id)}
                  onTogglePause={() => void togglePause(t.id, t.status)}
                  onDelete={() => setDeleteConfirm({ id: t.id, name: t.name })}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Add Torrent Modal */}
      {showAddModal && (
        <AddTorrentModal onClose={() => setShowAddModal(false)} onAdded={() => void fetchData()} />
      )}

      {/* Delete Confirm Dialog */}
      {deleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={() => setDeleteConfirm(null)}>
          <div className="w-full max-w-md rounded-lg bg-slate-800 p-6" onClick={(e) => e.stopPropagation()}>
            <h3 className="mb-2 text-lg font-semibold text-white">Delete Torrent</h3>
            <p className="mb-4 text-sm text-slate-300">
              Are you sure you want to delete <span className="font-medium text-white">{deleteConfirm.name}</span>?
            </p>
            <label className="mb-4 flex items-center gap-2 text-sm text-slate-300">
              <input
                type="checkbox"
                checked={deleteFiles}
                onChange={(e) => setDeleteFiles(e.target.checked)}
                className="rounded border-slate-600 bg-slate-700"
              />
              Also delete downloaded files
            </label>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setDeleteConfirm(null)}
                className="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => void deleteTorrent()}
                className="rounded-lg bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-500 transition-colors"
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

// ── Sub-components ─────────────────────────────────────────────────────────

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg bg-slate-800 px-4 py-3">
      <div className="text-xs text-slate-400">{label}</div>
      <div className="text-lg font-semibold text-white">{value}</div>
    </div>
  )
}

function SortHeader({
  field,
  label,
  current,
  dir,
  onSort,
  className,
}: {
  field: SortField
  label: string
  current: SortField
  dir: SortDir
  onSort: (f: SortField) => void
  className?: string
}) {
  return (
    <th className={`px-4 py-3 font-medium ${className ?? ''}`}>
      <button
        onClick={() => onSort(field)}
        className="flex items-center gap-1 hover:text-white transition-colors"
      >
        {label}
        <ArrowUpDown size={12} className={current === field ? (dir === 'asc' ? 'text-blue-400' : 'text-blue-400 rotate-180') : 'opacity-30'} />
      </button>
    </th>
  )
}

function TorrentRow({
  torrent: t,
  expanded,
  onToggleExpand,
  onTogglePause,
  onDelete,
}: {
  torrent: TorrentItem
  expanded: boolean
  onToggleExpand: () => void
  onTogglePause: () => void
  onDelete: () => void
}) {
  return (
    <>
      <tr className="hover:bg-slate-700/30 transition-colors">
        <td className="px-2 py-3">
          <button onClick={onToggleExpand} className="text-slate-400 hover:text-white transition-colors">
            {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          </button>
        </td>
        <td className="px-4 py-3 font-medium text-white max-w-xs truncate" title={t.name}>{t.name}</td>
        <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatSize(t.size)}</td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-2">
            <div className="h-2 flex-1 overflow-hidden rounded-full bg-slate-600">
              <div
                className={`h-full rounded-full ${statusColor(t.status)} transition-all`}
                style={{ width: `${t.progress}%` }}
              />
            </div>
            <span className="w-10 text-right text-xs text-slate-400">{Math.round(t.progress)}%</span>
          </div>
        </td>
        <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatSpeed(t.downloadSpeed)}</td>
        <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatSpeed(t.uploadSpeed)}</td>
        <td className="px-4 py-3 text-slate-300">{t.seeds}</td>
        <td className="px-4 py-3 text-slate-300">{t.peers}</td>
        <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatEta(t.eta)}</td>
        <td className="px-4 py-3">
          <span className={`rounded-full px-2 py-0.5 text-xs font-medium capitalize ${statusBadgeColor(t.status)}`}>
            {t.status}
          </span>
        </td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-1">
            <button
              onClick={onTogglePause}
              className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
              title={t.status.toLowerCase() === 'paused' ? 'Resume' : 'Pause'}
            >
              {t.status.toLowerCase() === 'paused' ? <Play size={14} /> : <Pause size={14} />}
            </button>
            <button
              onClick={onDelete}
              className="rounded p-1 text-slate-400 hover:bg-red-500/20 hover:text-red-400 transition-colors"
              title="Delete"
            >
              <Trash2 size={14} />
            </button>
          </div>
        </td>
      </tr>
      {expanded && (
        <tr>
          <td colSpan={11} className="bg-slate-800/50 px-6 py-4">
            <ExpandedDetails torrent={t} />
          </td>
        </tr>
      )}
    </>
  )
}

function ExpandedDetails({ torrent: t }: { torrent: TorrentItem }) {
  const [tab, setTab] = useState<'files' | 'trackers' | 'peers'>('files')
  const tabClass = (active: boolean) =>
    `px-3 py-1.5 text-xs font-medium rounded-lg transition-colors ${active ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700'}`

  return (
    <div>
      <div className="mb-3 flex gap-1">
        <button className={tabClass(tab === 'files')} onClick={() => setTab('files')}>Files</button>
        <button className={tabClass(tab === 'trackers')} onClick={() => setTab('trackers')}>Trackers</button>
        <button className={tabClass(tab === 'peers')} onClick={() => setTab('peers')}>Peers</button>
      </div>
      {tab === 'files' && (
        <div className="space-y-1 text-xs text-slate-300">
          {t.files && t.files.length > 0 ? (
            t.files.map((f, i) => (
              <div key={i} className="flex items-center justify-between gap-4">
                <span className="truncate">{f.name}</span>
                <span className="shrink-0 text-slate-400">{formatSize(f.size)} ({Math.round(f.progress)}%)</span>
              </div>
            ))
          ) : (
            <span className="text-slate-500">No file information available</span>
          )}
        </div>
      )}
      {tab === 'trackers' && (
        <div className="space-y-1 text-xs text-slate-300">
          {t.trackers && t.trackers.length > 0 ? (
            t.trackers.map((tr, i) => <div key={i} className="truncate">{tr}</div>)
          ) : (
            <span className="text-slate-500">No tracker information available</span>
          )}
        </div>
      )}
      {tab === 'peers' && (
        <div className="space-y-1 text-xs text-slate-300">
          {t.peerList && t.peerList.length > 0 ? (
            <table className="w-full">
              <thead>
                <tr className="text-left text-slate-500">
                  <th className="py-1 pr-4">IP</th>
                  <th className="py-1 pr-4">Client</th>
                  <th className="py-1 pr-4">Down</th>
                  <th className="py-1">Up</th>
                </tr>
              </thead>
              <tbody>
                {t.peerList.map((p, i) => (
                  <tr key={i}>
                    <td className="py-0.5 pr-4">{p.ip}</td>
                    <td className="py-0.5 pr-4">{p.client}</td>
                    <td className="py-0.5 pr-4">{formatSpeed(p.downloadSpeed)}</td>
                    <td className="py-0.5">{formatSpeed(p.uploadSpeed)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <span className="text-slate-500">No peer information available</span>
          )}
        </div>
      )}
    </div>
  )
}

function AddTorrentModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => void }) {
  const [magnetUrl, setMagnetUrl] = useState('')
  const [category, setCategory] = useState('')
  const [startPaused, setStartPaused] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [errorMsg, setErrorMsg] = useState('')
  const fileRef = useRef<HTMLInputElement>(null)

  const handleSubmit = async () => {
    setSubmitting(true)
    setErrorMsg('')
    try {
      const formData = new FormData()
      if (magnetUrl) {
        formData.append('magnetUrl', magnetUrl)
      }
      const fileInput = fileRef.current
      if (fileInput?.files?.[0]) {
        formData.append('torrentFile', fileInput.files[0])
      }
      if (!magnetUrl && !fileInput?.files?.[0]) {
        setErrorMsg('Provide a magnet URL or .torrent file')
        setSubmitting(false)
        return
      }
      if (category) formData.append('category', category)
      formData.append('startPaused', String(startPaused))

      const res = await fetch('/api/v1/torrent/add', { method: 'POST', body: formData })
      if (!res.ok) {
        const body = await res.text()
        throw new Error(body || `HTTP ${res.status}`)
      }
      onAdded()
      onClose()
    } catch (e) {
      setErrorMsg(e instanceof Error ? e.message : 'Failed to add torrent')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="w-full max-w-lg rounded-lg bg-slate-800 p-6" onClick={(e) => e.stopPropagation()}>
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-lg font-semibold text-white">Add Torrent</h3>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition-colors">
            <X size={20} />
          </button>
        </div>

        {errorMsg && (
          <div className="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">
            {errorMsg}
          </div>
        )}

        <div className="space-y-4">
          {/* Magnet URL */}
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Magnet URL</label>
            <input
              type="text"
              value={magnetUrl}
              onChange={(e) => setMagnetUrl(e.target.value)}
              placeholder="magnet:?xt=urn:btih:..."
              className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white placeholder-slate-500 outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
            />
          </div>

          {/* File upload */}
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">.torrent File</label>
            <div className="flex items-center gap-2">
              <button
                onClick={() => fileRef.current?.click()}
                className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm text-slate-300 hover:bg-slate-600 transition-colors"
              >
                <Upload size={14} /> Choose File
              </button>
              <span className="text-sm text-slate-400 truncate">
                {fileRef.current?.files?.[0]?.name ?? 'No file selected'}
              </span>
            </div>
            <input ref={fileRef} type="file" accept=".torrent" className="hidden" onChange={() => { /* trigger re-render is not needed, submit reads ref */ }} />
          </div>

          {/* Category */}
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Category</label>
            <input
              type="text"
              value={category}
              onChange={(e) => setCategory(e.target.value)}
              placeholder="Optional category"
              className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white placeholder-slate-500 outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
            />
          </div>

          {/* Start paused */}
          <label className="flex items-center gap-2 text-sm text-slate-300">
            <input
              type="checkbox"
              checked={startPaused}
              onChange={(e) => setStartPaused(e.target.checked)}
              className="rounded border-slate-600 bg-slate-700"
            />
            Start paused
          </label>
        </div>

        <div className="mt-6 flex justify-end gap-2">
          <button
            onClick={onClose}
            className="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={() => void handleSubmit()}
            disabled={submitting}
            className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
          >
            {submitting && <Loader2 size={14} className="animate-spin" />}
            Add
          </button>
        </div>
      </div>
    </div>
  )
}
