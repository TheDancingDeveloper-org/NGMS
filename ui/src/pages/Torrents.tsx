import { useState, useEffect, useCallback, useRef, memo } from 'react'
import { authHeaders } from '../api/client'
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
  ArrowDown,
  ArrowUp,
  Copy,
  Check,
  Gauge,
  CheckSquare,
  Square,
  MinusSquare,
} from 'lucide-react'

// ── Types ──────────────────────────────────────────────────────────────────

interface SessionStats {
  enabled: boolean
  downloadSpeed: number
  uploadSpeed: number
  sessionUptime: number
  peers: {
    connecting: number
    liveTcp: number
    liveUtp: number
    dead: number
    queued: number
    seen: number
  }
  counters: {
    fetchedBytes: number
    uploadedBytes: number
  }
}

interface LiveTorrentStats {
  snapshot: {
    have_bytes: number
    downloaded_and_checked_bytes: number
    downloaded_and_checked_pieces: number
    fetched_bytes: number
    uploaded_bytes: number
    initially_needed_bytes: number
    remaining_bytes: number
    total_bytes: number
    total_piece_download_ms: number
    peer_stats: {
      queued: number
      connecting: number
      live: number
      seen: number
      dead: number
      not_needed: number
    }
  }
  download_speed: { mbps: number; human_readable: string }
  upload_speed: { mbps: number; human_readable: string }
  time_remaining: { human_readable: string; duration?: { secs: number } } | null
}

interface TorrentStats {
  state: 'initializing' | 'paused' | 'live' | 'error'
  error: string | null
  file_progress: number[]
  progress_bytes: number
  finished: boolean
  total_bytes: number
  live: LiveTorrentStats | null
  ratio?: number
  seeding_time_secs?: number
  queue_state?: 'Active' | 'Queued' | 'ManuallyPaused'
  queue_position?: number
  sequential?: boolean
  super_seeding?: boolean
}

interface TorrentFile {
  name: string
  components: string[]
  length: number
  included: boolean
}

interface TorrentDetails {
  name: string | null
  info_hash: string
  files: TorrentFile[]
  total_pieces?: number
  output_folder: string
  category?: string
}

interface TorrentListItem {
  id: number
  info_hash: string
  name: string | null
  output_folder: string
  total_pieces: number
  stats?: TorrentStats
  category?: string
}

interface ListTorrentsResponse {
  torrents: TorrentListItem[]
  total: number
}

type SortField = 'name' | 'size' | 'progress' | 'downloadSpeed' | 'uploadSpeed' | 'seeds' | 'peers' | 'eta' | 'status'
type SortDir = 'asc' | 'desc'
type DetailTab = 'overview' | 'files' | 'peers' | 'trackers' | 'speed'

interface SpeedPoint {
  timestamp: number
  download: number
  upload: number
}

// ── Helpers ────────────────────────────────────────────────────────────────

function formatSpeed(bytesPerSec: number): string {
  if (!bytesPerSec || !isFinite(bytesPerSec) || bytesPerSec <= 0) return '0 B/s'
  if (bytesPerSec < 1024) return `${bytesPerSec.toFixed(0)} B/s`
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(1)} KB/s`
  if (bytesPerSec < 1024 * 1024 * 1024) return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} MB/s`
  return `${(bytesPerSec / (1024 * 1024 * 1024)).toFixed(2)} GB/s`
}

function formatSize(bytes: number): string {
  if (!bytes || !isFinite(bytes) || bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(i > 0 ? 1 : 0)} ${units[i]}`
}

function formatEta(seconds: number | undefined | null): string {
  if (!seconds || seconds <= 0) return '-'
  if (seconds < 60) return '< 1m'
  const mins = Math.floor(seconds / 60)
  if (mins < 60) return `${mins}m`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ${mins % 60}m`
  return `${Math.floor(hours / 24)}d ${hours % 24}h`
}

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `${mins}m ${secs % 60}s`
  const hours = Math.floor(mins / 60)
  if (hours < 24) return `${hours}h ${mins % 60}m`
  const days = Math.floor(hours / 24)
  return `${days}d ${hours % 24}h`
}

function mibpsToBytes(mibps: number): number {
  return mibps * 1024 * 1024
}

/** Derive a display-friendly status from torrent stats */
function deriveStatus(stats: TorrentStats | undefined): { label: string; key: string } {
  if (!stats) return { label: 'Unknown', key: 'unknown' }
  if (stats.error) return { label: 'Error', key: 'error' }
  if (stats.state === 'initializing') return { label: 'Checking', key: 'checking' }
  if (stats.state === 'paused') return { label: 'Paused', key: 'paused' }
  if (stats.state === 'live' && stats.finished) return { label: 'Seeding', key: 'seeding' }
  if (stats.state === 'live') return { label: 'Downloading', key: 'downloading' }
  if (stats.queue_state === 'Queued') return { label: 'Queued', key: 'queued' }
  return { label: stats.state, key: stats.state }
}

function statusBadgeColor(key: string): string {
  switch (key) {
    case 'downloading': return 'bg-blue-500/20 text-blue-400'
    case 'seeding': return 'bg-green-500/20 text-green-400'
    case 'paused': return 'bg-yellow-500/20 text-yellow-400'
    case 'checking': return 'bg-purple-500/20 text-purple-400'
    case 'error': return 'bg-red-500/20 text-red-400'
    case 'queued': return 'bg-slate-600 text-slate-300'
    default: return 'bg-slate-600 text-slate-300'
  }
}

function progressBarColor(key: string): string {
  switch (key) {
    case 'downloading': return 'bg-blue-500'
    case 'seeding': return 'bg-green-500'
    case 'paused': return 'bg-yellow-500'
    case 'checking': return 'bg-purple-500'
    case 'error': return 'bg-red-500'
    default: return 'bg-slate-500'
  }
}

/** Extract sortable numeric values from a torrent */
function getSortValue(t: TorrentListItem, field: SortField): string | number {
  const s = t.stats
  switch (field) {
    case 'name': return t.name ?? ''
    case 'size': return s?.total_bytes ?? 0
    case 'progress': return s ? (s.total_bytes > 0 ? s.progress_bytes / s.total_bytes : 0) : 0
    case 'downloadSpeed': return s?.live?.download_speed.mbps ?? 0
    case 'uploadSpeed': return s?.live?.upload_speed.mbps ?? 0
    case 'seeds': return s?.live?.snapshot.peer_stats.live ?? 0
    case 'peers': return s?.live?.snapshot.peer_stats.connecting ?? 0
    case 'eta': return s?.live?.time_remaining?.duration?.secs ?? Infinity
    case 'status': return deriveStatus(s).label
  }
}

const SPEED_LIMIT_PRESETS = [0, 100, 512, 1024, 2048, 5120, 10240] // KB/s, 0 = unlimited
const MAX_HEADER_SPEED_POINTS = 60

// ── Component ──────────────────────────────────────────────────────────────

export default function Torrents() {
  // -- Data state --
  const [sessionStats, setSessionStats] = useState<SessionStats | null>(null)
  const [torrents, setTorrents] = useState<TorrentListItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  // -- UI state --
  const [search, setSearch] = useState('')
  const [sortField, setSortField] = useState<SortField>('name')
  const [sortDir, setSortDir] = useState<SortDir>('asc')
  const [expandedId, setExpandedId] = useState<number | null>(null)
  const [showAddModal, setShowAddModal] = useState(false)
  const [deleteConfirm, setDeleteConfirm] = useState<{ id: number; name: string } | null>(null)
  const [deleteFiles, setDeleteFiles] = useState(false)
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set())
  const [showSpeedLimit, setShowSpeedLimit] = useState(false)
  const [activeTab, setActiveTab] = useState<'torrents' | 'settings'>('torrents')

  // -- Speed graph history for header --
  const [speedHistory, setSpeedHistory] = useState<SpeedPoint[]>([])
  const headerCanvasRef = useRef<HTMLCanvasElement>(null)

  // ── Fetching ──

  const fetchData = useCallback(async () => {
    try {
      const [statsRes, listRes] = await Promise.all([
        fetch('/api/v1/torrent/status'),
        fetch('/api/v1/torrent/list'),
      ])
      if (statsRes.ok) {
        const stats = await statsRes.json() as SessionStats
        setSessionStats(stats)
        if (stats.enabled) {
          setSpeedHistory(prev => {
            const next = [...prev, {
              timestamp: Date.now(),
              download: stats.downloadSpeed,
              upload: stats.uploadSpeed,
            }]
            return next.slice(-MAX_HEADER_SPEED_POINTS)
          })
        }
      }
      if (listRes.ok) {
        const data = await listRes.json() as ListTorrentsResponse
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
    const interval = setInterval(() => void fetchData(), 2000)
    return () => clearInterval(interval)
  }, [fetchData])

  // -- Draw header speed graph --
  useEffect(() => {
    const canvas = headerCanvasRef.current
    if (!canvas || speedHistory.length < 2) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const dpr = window.devicePixelRatio || 1
    const w = canvas.clientWidth
    const h = canvas.clientHeight
    canvas.width = w * dpr
    canvas.height = h * dpr
    ctx.scale(dpr, dpr)
    ctx.clearRect(0, 0, w, h)

    const maxVal = Math.max(
      ...speedHistory.map(p => Math.max(p.download, p.upload)),
      1024,
    )

    const drawLine = (getValue: (p: SpeedPoint) => number, color: string) => {
      ctx.beginPath()
      ctx.strokeStyle = color
      ctx.lineWidth = 1.5
      speedHistory.forEach((point, i) => {
        const x = (i / (MAX_HEADER_SPEED_POINTS - 1)) * w
        const y = h - (getValue(point) / maxVal) * h * 0.9
        if (i === 0) ctx.moveTo(x, y)
        else ctx.lineTo(x, y)
      })
      ctx.stroke()
    }

    drawLine(p => p.download, '#3b82f6')
    drawLine(p => p.upload, '#22c55e')
  }, [speedHistory])

  // ── Actions ──

  const pauseAll = async () => {
    for (const t of torrents) {
      if (t.stats?.state === 'live') {
        await fetch(`/api/v1/torrent/${t.id}/pause`, { method: 'POST' })
      }
    }
    void fetchData()
  }

  const resumeAll = async () => {
    for (const t of torrents) {
      if (t.stats?.state === 'paused') {
        await fetch(`/api/v1/torrent/${t.id}/resume`, { method: 'POST' })
      }
    }
    void fetchData()
  }

  const togglePause = async (id: number, state: string) => {
    const endpoint = state === 'paused' ? 'resume' : 'pause'
    await fetch(`/api/v1/torrent/${id}/${endpoint}`, { method: 'POST' })
    void fetchData()
  }

  const deleteTorrent = async () => {
    if (!deleteConfirm) return
    await fetch(`/api/v1/torrent/${deleteConfirm.id}/delete?deleteFiles=${deleteFiles}`, { method: 'POST' })
    setDeleteConfirm(null)
    setDeleteFiles(false)
    setSelectedIds(prev => {
      const next = new Set(prev)
      next.delete(deleteConfirm.id)
      return next
    })
    void fetchData()
  }

  const bulkPause = async () => {
    for (const id of selectedIds) {
      const t = torrents.find(x => x.id === id)
      if (t?.stats?.state === 'live') {
        await fetch(`/api/v1/torrent/${id}/pause`, { method: 'POST' })
      }
    }
    void fetchData()
  }

  const bulkResume = async () => {
    for (const id of selectedIds) {
      const t = torrents.find(x => x.id === id)
      if (t?.stats?.state === 'paused') {
        await fetch(`/api/v1/torrent/${id}/resume`, { method: 'POST' })
      }
    }
    void fetchData()
  }

  const bulkDelete = async () => {
    for (const id of selectedIds) {
      await fetch(`/api/v1/torrent/${id}/delete?deleteFiles=false`, { method: 'POST' })
    }
    setSelectedIds(new Set())
    void fetchData()
  }

  // ── Sorting / filtering ──

  const handleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir(d => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortField(field)
      setSortDir('asc')
    }
  }

  const filtered = torrents
    .filter(t => (t.name ?? '').toLowerCase().includes(search.toLowerCase()))
    .sort((a, b) => {
      const va = getSortValue(a, sortField)
      const vb = getSortValue(b, sortField)
      const cmp = typeof va === 'string'
        ? (va as string).localeCompare(vb as string)
        : (va as number) - (vb as number)
      return sortDir === 'asc' ? cmp : -cmp
    })

  // -- Derived counts --
  const activeCount = torrents.filter(t => t.stats?.state === 'live').length
  const pausedCount = torrents.filter(t => t.stats?.state === 'paused').length

  // -- Selection helpers --
  const allFilteredSelected = filtered.length > 0 && filtered.every(t => selectedIds.has(t.id))
  const someFilteredSelected = filtered.some(t => selectedIds.has(t.id))

  const toggleSelectAll = () => {
    if (allFilteredSelected) {
      setSelectedIds(new Set())
    } else {
      setSelectedIds(new Set(filtered.map(t => t.id)))
    }
  }

  const toggleSelect = (id: number) => {
    setSelectedIds(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

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
      {/* ── Dashboard Header ── */}
      <div className="mb-4 rounded-lg bg-slate-800 p-4">
        <div className="flex flex-wrap items-center gap-4">
          {/* Speed display */}
          <div className="flex items-center gap-6">
            <div className="flex items-center gap-2">
              <ArrowDown size={16} className="text-blue-400" />
              <div>
                <div className="text-xs text-slate-400">Download</div>
                <div className="text-lg font-semibold text-white">
                  {formatSpeed(sessionStats?.downloadSpeed ?? 0)}
                </div>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <ArrowUp size={16} className="text-green-400" />
              <div>
                <div className="text-xs text-slate-400">Upload</div>
                <div className="text-lg font-semibold text-white">
                  {formatSpeed(sessionStats?.uploadSpeed ?? 0)}
                </div>
              </div>
            </div>
          </div>

          {/* Mini speed graph */}
          <div className="hidden sm:block">
            <canvas
              ref={headerCanvasRef}
              className="h-10 w-32 rounded bg-slate-900/50"
            />
          </div>

          {/* Counts */}
          <div className="flex items-center gap-4 text-sm">
            <span className="text-slate-400">
              Active <span className="font-semibold text-green-400">{activeCount}</span>
            </span>
            <span className="text-slate-400">
              Paused <span className="font-semibold text-yellow-400">{pausedCount}</span>
            </span>
          </div>

          {/* Actions */}
          <div className="ml-auto flex items-center gap-2">
            {/* Speed limit dropdown */}
            <div className="relative">
              <button
                onClick={() => setShowSpeedLimit(!showSpeedLimit)}
                className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm text-slate-300 hover:bg-slate-600 transition-colors"
                title="Speed limit"
              >
                <Gauge size={16} />
              </button>
              {showSpeedLimit && (
                <>
                  <div className="fixed inset-0 z-40" onClick={() => setShowSpeedLimit(false)} />
                  <div className="absolute right-0 top-full z-50 mt-1 w-48 rounded-lg bg-slate-700 py-1 shadow-xl">
                    <div className="px-3 py-1.5 text-xs font-medium text-slate-400 uppercase">Download Limit</div>
                    {SPEED_LIMIT_PRESETS.map(kb => (
                      <button
                        key={kb}
                        onClick={() => setShowSpeedLimit(false)}
                        className="w-full px-3 py-1.5 text-left text-sm text-slate-300 hover:bg-slate-600 transition-colors"
                      >
                        {kb === 0 ? 'Unlimited' : `${kb >= 1024 ? `${(kb / 1024).toFixed(0)} MB/s` : `${kb} KB/s`}`}
                      </button>
                    ))}
                  </div>
                </>
              )}
            </div>

            <button
              onClick={() => void pauseAll()}
              className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
            >
              <Pause size={14} /> Pause All
            </button>
            <button
              onClick={() => void resumeAll()}
              className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
            >
              <Play size={14} /> Resume All
            </button>
            <button
              onClick={() => setShowAddModal(true)}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
            >
              <Plus size={16} /> Add Torrent
            </button>
          </div>
        </div>

        {/* Search */}
        <div className="mt-3">
          <div className="relative">
            <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-slate-400" />
            <input
              type="text"
              placeholder="Filter torrents..."
              value={search}
              onChange={e => setSearch(e.target.value)}
              className="w-full rounded-lg bg-slate-900/50 py-2 pl-9 pr-3 text-sm text-white placeholder-slate-500 outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
            />
            {search && (
              <button
                onClick={() => setSearch('')}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-slate-400 hover:text-white"
              >
                <X size={14} />
              </button>
            )}
          </div>
        </div>
      </div>

      {/* ── Tab Bar ── */}
      <div className="mb-4 flex gap-1 border-b border-slate-700">
        <button
          className={`px-4 py-2 text-sm font-medium transition-colors ${activeTab === 'torrents' ? 'border-b-2 border-blue-500 text-white' : 'text-slate-400 hover:text-white'}`}
          onClick={() => setActiveTab('torrents')}
        >Torrents</button>
        <button
          className={`px-4 py-2 text-sm font-medium transition-colors ${activeTab === 'settings' ? 'border-b-2 border-blue-500 text-white' : 'text-slate-400 hover:text-white'}`}
          onClick={() => setActiveTab('settings')}
        >Settings</button>
      </div>

      {activeTab === 'settings' && <TorrentSettingsTab />}

      {activeTab === 'torrents' && <>
      {/* ── Bulk Action Bar ── */}
      {selectedIds.size > 0 && (
        <div className="mb-3 flex items-center gap-3 rounded-lg bg-blue-500/10 border border-blue-500/20 px-4 py-2">
          <span className="text-sm font-medium text-blue-400">{selectedIds.size} selected</span>
          <button
            onClick={() => void bulkPause()}
            className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-600 transition-colors"
          >
            <Pause size={12} /> Pause
          </button>
          <button
            onClick={() => void bulkResume()}
            className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-1.5 text-xs font-medium text-slate-300 hover:bg-slate-600 transition-colors"
          >
            <Play size={12} /> Resume
          </button>
          <button
            onClick={() => void bulkDelete()}
            className="flex items-center gap-1.5 rounded-lg bg-red-500/20 px-3 py-1.5 text-xs font-medium text-red-400 hover:bg-red-500/30 transition-colors"
          >
            <Trash2 size={12} /> Delete
          </button>
          <button
            onClick={() => setSelectedIds(new Set())}
            className="ml-auto text-xs text-slate-400 hover:text-white transition-colors"
          >
            Clear selection
          </button>
        </div>
      )}

      {/* ── Error ── */}
      {error && (
        <div className="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">
          {error}
        </div>
      )}

      {/* ── Empty state ── */}
      {!error && filtered.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <Magnet size={48} className="mb-4 text-slate-600" />
          <p className="mb-2">{search ? 'No torrents match your filter' : 'No active torrents'}</p>
          {!search && (
            <button
              onClick={() => setShowAddModal(true)}
              className="mt-2 flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
            >
              <Plus size={16} /> Add your first torrent
            </button>
          )}
        </div>
      )}

      {/* ── Torrent Table ── */}
      {filtered.length > 0 && (
        <div className="overflow-x-auto rounded-lg bg-slate-800">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                <th className="w-10 px-2 py-3">
                  <button
                    onClick={toggleSelectAll}
                    className="text-slate-400 hover:text-white transition-colors"
                  >
                    {allFilteredSelected ? (
                      <CheckSquare size={16} />
                    ) : someFilteredSelected ? (
                      <MinusSquare size={16} />
                    ) : (
                      <Square size={16} />
                    )}
                  </button>
                </th>
                <th className="w-8 px-1 py-3" />
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
            <tbody className="divide-y divide-slate-700/50">
              {filtered.map(t => (
                <TorrentRow
                  key={t.id}
                  torrent={t}
                  expanded={expandedId === t.id}
                  selected={selectedIds.has(t.id)}
                  onToggleExpand={() => setExpandedId(expandedId === t.id ? null : t.id)}
                  onToggleSelect={() => toggleSelect(t.id)}
                  onTogglePause={() => void togglePause(t.id, t.stats?.state ?? '')}
                  onDelete={() => setDeleteConfirm({ id: t.id, name: t.name ?? `Torrent #${t.id}` })}
                />
              ))}
            </tbody>
          </table>
        </div>
      )}

      </>}

      {/* ── Add Torrent Modal ── */}
      {showAddModal && (
        <AddTorrentModal onClose={() => setShowAddModal(false)} onAdded={() => void fetchData()} />
      )}

      {/* ── Delete Confirm Dialog ── */}
      {deleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={() => setDeleteConfirm(null)}>
          <div className="w-full max-w-md rounded-lg bg-slate-800 border border-slate-700 p-6 shadow-xl" onClick={e => e.stopPropagation()}>
            <h3 className="mb-2 text-lg font-semibold text-white">Delete Torrent</h3>
            <p className="mb-4 text-sm text-slate-300">
              Are you sure you want to delete <span className="font-medium text-white">{deleteConfirm.name}</span>?
            </p>
            <label className="mb-5 flex items-center gap-2 text-sm text-slate-300 cursor-pointer">
              <input
                type="checkbox"
                checked={deleteFiles}
                onChange={e => setDeleteFiles(e.target.checked)}
                className="rounded border-slate-600 bg-slate-700 text-red-500 focus:ring-red-500"
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
        <ArrowUpDown
          size={12}
          className={
            current === field
              ? `text-blue-400 ${dir === 'desc' ? 'rotate-180' : ''} transition-transform`
              : 'opacity-30'
          }
        />
      </button>
    </th>
  )
}

const TorrentRow = memo(function TorrentRow({
  torrent: t,
  expanded,
  selected,
  onToggleExpand,
  onToggleSelect,
  onTogglePause,
  onDelete,
}: {
  torrent: TorrentListItem
  expanded: boolean
  selected: boolean
  onToggleExpand: () => void
  onToggleSelect: () => void
  onTogglePause: () => void
  onDelete: () => void
}) {
  const stats = t.stats
  const status = deriveStatus(stats)
  const totalBytes = stats?.total_bytes ?? 0
  const progressBytes = stats?.progress_bytes ?? 0
  const progressPct = totalBytes > 0 ? (progressBytes / totalBytes) * 100 : 0
  const dlSpeed = stats?.live ? mibpsToBytes(stats.live.download_speed.mbps) : 0
  const ulSpeed = stats?.live ? mibpsToBytes(stats.live.upload_speed.mbps) : 0
  const seedsLive = stats?.live?.snapshot.peer_stats.live ?? 0
  const peersConnecting = stats?.live?.snapshot.peer_stats.connecting ?? 0
  const etaSecs = stats?.live?.time_remaining?.duration?.secs

  return (
    <>
      <tr className={`hover:bg-slate-700/30 transition-colors ${selected ? 'bg-blue-500/5' : ''}`}>
        <td className="px-2 py-3">
          <button
            onClick={onToggleSelect}
            className="text-slate-400 hover:text-white transition-colors"
          >
            {selected ? <CheckSquare size={16} className="text-blue-400" /> : <Square size={16} />}
          </button>
        </td>
        <td className="px-1 py-3">
          <button onClick={onToggleExpand} className="text-slate-400 hover:text-white transition-colors">
            {expanded ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
          </button>
        </td>
        <td className="px-4 py-3 font-medium text-white max-w-xs truncate" title={t.name ?? ''}>
          {t.name ?? `Torrent #${t.id}`}
        </td>
        <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatSize(totalBytes)}</td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-2">
            <div className="h-2 flex-1 overflow-hidden rounded-full bg-slate-600">
              <div
                className={`h-full rounded-full ${progressBarColor(status.key)} transition-all`}
                style={{ width: `${Math.min(progressPct, 100)}%` }}
              />
            </div>
            <span className="w-12 text-right text-xs text-slate-400">
              {progressPct.toFixed(1)}%
            </span>
          </div>
        </td>
        <td className="px-4 py-3 whitespace-nowrap">
          <span className={dlSpeed > 0 ? 'text-blue-400' : 'text-slate-500'}>
            {formatSpeed(dlSpeed)}
          </span>
        </td>
        <td className="px-4 py-3 whitespace-nowrap">
          <span className={ulSpeed > 0 ? 'text-green-400' : 'text-slate-500'}>
            {formatSpeed(ulSpeed)}
          </span>
        </td>
        <td className="px-4 py-3 text-slate-300">{seedsLive}</td>
        <td className="px-4 py-3 text-slate-300">{peersConnecting}</td>
        <td className="px-4 py-3 text-slate-300 whitespace-nowrap">
          {stats?.finished ? 'Done' : formatEta(etaSecs)}
        </td>
        <td className="px-4 py-3">
          <span className={`rounded-full px-2.5 py-0.5 text-xs font-medium ${statusBadgeColor(status.key)}`}>
            {status.label}
          </span>
        </td>
        <td className="px-4 py-3">
          <div className="flex items-center gap-1">
            <button
              onClick={onTogglePause}
              className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
              title={stats?.state === 'paused' ? 'Resume' : 'Pause'}
            >
              {stats?.state === 'paused' ? <Play size={14} /> : <Pause size={14} />}
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
          <td colSpan={12} className="bg-slate-850 border-t border-slate-700/30">
            <ExpandedDetails torrent={t} />
          </td>
        </tr>
      )}
    </>
  )
})

// ── Expanded Detail View ──────────────────────────────────────────────────

function ExpandedDetails({ torrent: t }: { torrent: TorrentListItem }) {
  const [tab, setTab] = useState<DetailTab>('overview')
  const [details, setDetails] = useState<TorrentDetails | null>(null)
  const [loadingDetails, setLoadingDetails] = useState(true)

  // Fetch details when expanded
  useEffect(() => {
    let cancelled = false
    fetch(`/api/v1/torrent/${t.id}`)
      .then(r => r.ok ? r.json() as Promise<TorrentDetails> : null)
      .then(det => {
        if (cancelled) return
        if (det) setDetails(det)
        setLoadingDetails(false)
      })
      .catch(() => {
        if (!cancelled) setLoadingDetails(false)
      })
    return () => { cancelled = true }
  }, [t.id])

  const tabClass = (active: boolean) =>
    `px-3 py-1.5 text-xs font-medium rounded-lg transition-colors ${active ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-white hover:bg-slate-700'}`

  return (
    <div className="px-6 py-4">
      <div className="mb-3 flex gap-1">
        <button className={tabClass(tab === 'overview')} onClick={() => setTab('overview')}>Overview</button>
        <button className={tabClass(tab === 'files')} onClick={() => setTab('files')}>Files</button>
        <button className={tabClass(tab === 'peers')} onClick={() => setTab('peers')}>Peers</button>
        <button className={tabClass(tab === 'trackers')} onClick={() => setTab('trackers')}>Trackers</button>
        <button className={tabClass(tab === 'speed')} onClick={() => setTab('speed')}>Speed</button>
      </div>

      {tab === 'overview' && <OverviewTab torrent={t} details={details} />}
      {tab === 'files' && <FilesTab torrent={t} details={details} loading={loadingDetails} />}
      {tab === 'peers' && <PeersTab torrent={t} />}
      {tab === 'trackers' && <TrackersTab details={details} loading={loadingDetails} />}
      {tab === 'speed' && <SpeedTab key={t.id} torrent={t} />}
    </div>
  )
}

// ── Overview Tab ──────────────────────────────────────────────────────────

function OverviewTab({ torrent: t, details }: { torrent: TorrentListItem; details: TorrentDetails | null }) {
  const [copiedHash, setCopiedHash] = useState(false)
  const stats = t.stats
  const status = deriveStatus(stats)

  const totalBytes = stats?.total_bytes ?? 0
  const progressBytes = stats?.progress_bytes ?? 0
  const progressPct = totalBytes > 0 ? (progressBytes / totalBytes) * 100 : 0

  const dlSpeed = stats?.live?.download_speed.human_readable ?? '-'
  const ulSpeed = stats?.live?.upload_speed.human_readable ?? '-'
  const totalUploaded = stats?.live?.snapshot.uploaded_bytes ?? 0
  const ratio = stats?.ratio ?? 0

  const peersLive = stats?.live?.snapshot.peer_stats.live ?? 0
  const peersConnecting = stats?.live?.snapshot.peer_stats.connecting ?? 0
  const peersQueued = stats?.live?.snapshot.peer_stats.queued ?? 0
  const peersSeen = stats?.live?.snapshot.peer_stats.seen ?? 0
  const peersDead = stats?.live?.snapshot.peer_stats.dead ?? 0

  const etaText = stats?.finished
    ? 'Complete'
    : stats?.live?.time_remaining?.human_readable ?? '-'

  const copyHash = () => {
    void navigator.clipboard.writeText(t.info_hash)
    setCopiedHash(true)
    setTimeout(() => setCopiedHash(false), 2000)
  }

  const labelClass = 'text-slate-500'

  return (
    <div className="space-y-3 text-sm">
      {/* Progress bar */}
      <div>
        <div className="mb-1 flex items-center justify-between text-xs">
          <span className={statusBadgeColor(status.key) + ' rounded-full px-2 py-0.5 font-medium'}>
            {status.label}
          </span>
          <span className="text-slate-400">
            {formatSize(progressBytes)} / {formatSize(totalBytes)} ({progressPct.toFixed(1)}%)
          </span>
        </div>
        <div className="h-2.5 w-full overflow-hidden rounded-full bg-slate-600">
          <div
            className={`h-full rounded-full ${progressBarColor(status.key)} transition-all`}
            style={{ width: `${Math.min(progressPct, 100)}%` }}
          />
        </div>
      </div>

      {/* Stats grid */}
      <div className="grid grid-cols-2 gap-x-8 gap-y-1.5 sm:grid-cols-3 lg:grid-cols-4">
        <div><span className={labelClass}>Download: </span><span className="text-blue-400">{dlSpeed}</span></div>
        <div><span className={labelClass}>Upload: </span><span className="text-green-400">{ulSpeed}</span></div>
        <div><span className={labelClass}>Uploaded: </span><span className="text-slate-300">{formatSize(totalUploaded)}</span></div>
        <div><span className={labelClass}>Ratio: </span><span className="text-slate-300">{ratio.toFixed(2)}</span></div>
        <div><span className={labelClass}>ETA: </span><span className="text-slate-300">{etaText}</span></div>
        {stats?.seeding_time_secs != null && (
          <div><span className={labelClass}>Seeding: </span><span className="text-slate-300">{formatDuration(stats.seeding_time_secs)}</span></div>
        )}
        {stats?.queue_state && (
          <div><span className={labelClass}>Queue: </span><span className="text-slate-300">{stats.queue_state}</span></div>
        )}
        {stats?.sequential !== undefined && (
          <div><span className={labelClass}>Sequential: </span><span className="text-slate-300">{stats.sequential ? 'Yes' : 'No'}</span></div>
        )}
      </div>

      {/* Peers breakdown */}
      {stats?.live && (
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs">
          <span className={labelClass}>Peers:</span>
          <span><span className="text-green-400">{peersLive}</span> <span className={labelClass}>live</span></span>
          <span><span className="text-blue-400">{peersConnecting}</span> <span className={labelClass}>connecting</span></span>
          <span><span className="text-yellow-400">{peersQueued}</span> <span className={labelClass}>queued</span></span>
          <span><span className="text-slate-300">{peersSeen}</span> <span className={labelClass}>seen</span></span>
          <span><span className="text-red-400">{peersDead}</span> <span className={labelClass}>dead</span></span>
        </div>
      )}

      {/* Metadata */}
      <div className="space-y-1 text-xs border-t border-slate-700/50 pt-3">
        <div className="flex items-center gap-2">
          <span className={labelClass}>Hash:</span>
          <code className="text-slate-300 font-mono truncate">{t.info_hash}</code>
          <button
            onClick={copyHash}
            className="shrink-0 rounded p-0.5 text-slate-400 hover:text-white transition-colors"
            title="Copy info hash"
          >
            {copiedHash ? <Check size={12} className="text-green-400" /> : <Copy size={12} />}
          </button>
        </div>
        <div className="truncate">
          <span className={labelClass}>Output: </span>
          <code className="text-slate-300 font-mono">{details?.output_folder ?? t.output_folder}</code>
        </div>
        {(details?.category ?? t.category) && (
          <div>
            <span className={labelClass}>Category: </span>
            <span className="text-slate-300">{details?.category ?? t.category}</span>
          </div>
        )}
        {t.total_pieces > 0 && (
          <div>
            <span className={labelClass}>Pieces: </span>
            <span className="text-slate-300">
              {(stats?.live?.snapshot.downloaded_and_checked_pieces ?? 0).toLocaleString()} / {t.total_pieces.toLocaleString()}
            </span>
          </div>
        )}
      </div>

      {/* Error display */}
      {stats?.error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 p-3 text-sm text-red-400">
          {stats.error}
        </div>
      )}
    </div>
  )
}

// ── Files Tab ────────────────────────────────────────────────────────────

function FilesTab({
  torrent: t,
  details,
  loading,
}: {
  torrent: TorrentListItem
  details: TorrentDetails | null
  loading: boolean
}) {
  const fileProgress = t.stats?.file_progress ?? []

  if (loading && !details) {
    return (
      <div className="flex items-center gap-2 py-4 text-sm text-slate-400">
        <Loader2 size={14} className="animate-spin" /> Loading files...
      </div>
    )
  }

  if (!details || !details.files || details.files.length === 0) {
    return <div className="py-4 text-sm text-slate-500">No file information available</div>
  }

  return (
    <div className="space-y-0.5 text-xs">
      {/* Header */}
      <div className="flex items-center gap-2 px-2 py-1.5 text-slate-500 uppercase font-medium">
        <span className="flex-1">File</span>
        <span className="w-20 text-right">Size</span>
        <span className="w-28 text-right">Progress</span>
      </div>
      {details.files.map((f, i) => {
        const fileProg = fileProgress[i] !== undefined
          ? fileProgress[i] * 100
          : (t.stats?.finished ? 100 : 0)
        const isIncluded = f.included

        return (
          <div
            key={i}
            className={`flex items-center gap-2 rounded px-2 py-1.5 ${isIncluded ? 'hover:bg-slate-700/30' : 'opacity-40'} transition-colors`}
          >
            <span className="flex-1 truncate text-slate-300" title={f.name}>
              {f.name}
            </span>
            <span className="w-20 text-right text-slate-400 shrink-0">
              {formatSize(f.length)}
            </span>
            <div className="w-28 shrink-0 flex items-center gap-1.5">
              <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-slate-600">
                <div
                  className="h-full rounded-full bg-blue-500 transition-all"
                  style={{ width: `${Math.min(fileProg, 100)}%` }}
                />
              </div>
              <span className="w-9 text-right text-slate-400">
                {fileProg.toFixed(0)}%
              </span>
            </div>
          </div>
        )
      })}
    </div>
  )
}

// ── Peers Tab ────────────────────────────────────────────────────────────

function PeersTab({ torrent: t }: { torrent: TorrentListItem }) {
  const peers = t.stats?.live?.snapshot.peer_stats

  if (!peers) {
    return <div className="py-4 text-sm text-slate-500">No peer information available. Torrent may not be active.</div>
  }

  return (
    <div className="text-xs">
      {/* Peer summary */}
      <div className="mb-3 flex flex-wrap gap-x-4 gap-y-1 text-sm">
        <span><span className="text-green-400 font-medium">{peers.live}</span> <span className="text-slate-500">live</span></span>
        <span><span className="text-blue-400 font-medium">{peers.connecting}</span> <span className="text-slate-500">connecting</span></span>
        <span><span className="text-yellow-400 font-medium">{peers.queued}</span> <span className="text-slate-500">queued</span></span>
        <span><span className="text-slate-300 font-medium">{peers.seen}</span> <span className="text-slate-500">seen</span></span>
        <span><span className="text-red-400 font-medium">{peers.dead}</span> <span className="text-slate-500">dead</span></span>
      </div>

      {/* Aggregate stats */}
      <div className="rounded bg-slate-700/30 px-3 py-2 text-slate-400">
        Aggregate peer statistics shown above. Total connected: {peers.live + peers.connecting}
      </div>
    </div>
  )
}

// ── Trackers Tab ──────────────────────────────────────────────────────────

function TrackersTab({ details, loading }: { details: TorrentDetails | null; loading: boolean }) {
  if (loading && !details) {
    return (
      <div className="flex items-center gap-2 py-4 text-sm text-slate-400">
        <Loader2 size={14} className="animate-spin" /> Loading trackers...
      </div>
    )
  }

  // Details from the API might not include trackers in the current schema,
  // but we display what we have
  return (
    <div className="py-2 text-sm text-slate-400">
      <div className="space-y-1.5">
        <div className="flex items-center gap-2 text-xs text-slate-500 uppercase font-medium px-2">
          <span>Tracker URL</span>
        </div>
        {/* The backend detail response includes info_hash and output_folder,
            tracker URLs would come from the torrent metadata */}
        <div className="px-2 py-2 text-slate-500">
          Tracker information is available through the torrent metadata.
          Info hash: <code className="text-slate-400 font-mono">{details?.info_hash ?? 'N/A'}</code>
        </div>
      </div>
    </div>
  )
}

// ── Speed Tab ─────────────────────────────────────────────────────────────

function SpeedTab({ torrent: t }: { torrent: TorrentListItem }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [history, setHistory] = useState<SpeedPoint[]>([])
  const MAX_POINTS = 300 // 5 minutes at 1s resolution

  // Poll speed data every second
  useEffect(() => {
    if (t.stats?.state !== 'live') return
    let cancelled = false

    const poll = async () => {
      try {
        const res = await fetch(`/api/v1/torrent/${t.id}/stats`)
        if (!res.ok || cancelled) return
        const stats = await res.json() as TorrentStats
        if (stats.live && !cancelled) {
          setHistory(prev => {
            const next = [...prev, {
              timestamp: Date.now(),
              download: mibpsToBytes(stats.live!.download_speed.mbps),
              upload: mibpsToBytes(stats.live!.upload_speed.mbps),
            }]
            return next.slice(-MAX_POINTS)
          })
        }
      } catch {
        // ignore
      }
    }

    void poll()
    const interval = setInterval(() => void poll(), 1000)
    return () => {
      cancelled = true
      clearInterval(interval)
    }
  }, [t.id, t.stats?.state])

  // Draw chart
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || history.length < 2) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const dpr = window.devicePixelRatio || 1
    const w = canvas.clientWidth
    const h = canvas.clientHeight
    canvas.width = w * dpr
    canvas.height = h * dpr
    ctx.scale(dpr, dpr)
    ctx.clearRect(0, 0, w, h)

    const maxVal = Math.max(
      ...history.map(p => Math.max(p.download, p.upload)),
      1024,
    )

    // Grid lines
    ctx.strokeStyle = 'rgba(255,255,255,0.06)'
    ctx.lineWidth = 1
    for (let i = 0; i <= 4; i++) {
      const y = (i / 4) * h
      ctx.beginPath()
      ctx.moveTo(0, y)
      ctx.lineTo(w, y)
      ctx.stroke()
    }

    // Draw filled area + line
    const drawSeries = (getValue: (p: SpeedPoint) => number, lineColor: string, fillColor: string) => {
      // Fill
      ctx.beginPath()
      ctx.fillStyle = fillColor
      history.forEach((point, i) => {
        const x = (i / (MAX_POINTS - 1)) * w
        const y = h - (getValue(point) / maxVal) * h * 0.9
        if (i === 0) {
          ctx.moveTo(x, h)
          ctx.lineTo(x, y)
        } else {
          ctx.lineTo(x, y)
        }
      })
      ctx.lineTo(((history.length - 1) / (MAX_POINTS - 1)) * w, h)
      ctx.closePath()
      ctx.fill()

      // Line
      ctx.beginPath()
      ctx.strokeStyle = lineColor
      ctx.lineWidth = 2
      history.forEach((point, i) => {
        const x = (i / (MAX_POINTS - 1)) * w
        const y = h - (getValue(point) / maxVal) * h * 0.9
        if (i === 0) ctx.moveTo(x, y)
        else ctx.lineTo(x, y)
      })
      ctx.stroke()
    }

    // Download (green fill + line)
    drawSeries(p => p.download, '#4ade80', 'rgba(74, 222, 128, 0.1)')
    // Upload (blue fill + line)
    drawSeries(p => p.upload, '#60a5fa', 'rgba(96, 165, 250, 0.1)')

    // Y-axis labels
    ctx.fillStyle = 'rgba(255,255,255,0.4)'
    ctx.font = '10px monospace'
    ctx.textAlign = 'right'
    for (let i = 0; i <= 4; i++) {
      const value = maxVal * (1 - i / 4)
      const y = (i / 4) * h + 11
      ctx.fillText(`${formatSpeed(value)}`, w - 4, y)
    }
  }, [history])

  if (t.stats?.state !== 'live') {
    return (
      <div className="py-4 text-sm text-slate-500">
        Speed chart is only available for active torrents.
      </div>
    )
  }

  const currentDl = t.stats?.live?.download_speed.human_readable ?? '-'
  const currentUl = t.stats?.live?.upload_speed.human_readable ?? '-'

  return (
    <div className="space-y-2">
      {/* Legend */}
      <div className="flex items-center gap-6 text-xs">
        <div className="flex items-center gap-1.5">
          <span className="inline-block h-0.5 w-4 rounded bg-green-400" />
          <span className="text-slate-400">Download</span>
          <span className="font-medium text-green-400">{currentDl}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="inline-block h-0.5 w-4 rounded bg-blue-400" />
          <span className="text-slate-400">Upload</span>
          <span className="font-medium text-blue-400">{currentUl}</span>
        </div>
        <span className="text-slate-500 ml-auto">
          {history.length > 0 ? `${Math.round(history.length)} points (${Math.round(history.length)}s)` : 'Collecting data...'}
        </span>
      </div>

      {/* Canvas */}
      <canvas
        ref={canvasRef}
        className="w-full rounded-lg bg-slate-900/50"
        style={{ height: '160px' }}
      />
    </div>
  )
}

// ── Add Torrent Modal ─────────────────────────────────────────────────────

function AddTorrentModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => void }) {
  const [magnetUrl, setMagnetUrl] = useState('')
  const [category, setCategory] = useState('')
  const [startPaused, setStartPaused] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [errorMsg, setErrorMsg] = useState('')
  const [dragActive, setDragActive] = useState(false)
  const [fileName, setFileName] = useState('')
  const fileRef = useRef<HTMLInputElement>(null)

  const handleSubmit = async () => {
    setSubmitting(true)
    setErrorMsg('')
    try {
      const fileInput = fileRef.current
      const hasFile = fileInput?.files?.[0]

      if (!magnetUrl && !hasFile) {
        setErrorMsg('Provide a magnet URL or .torrent file')
        setSubmitting(false)
        return
      }

      // Use JSON body for magnet URL (matches backend AddTorrentRequest)
      if (magnetUrl) {
        const res = await fetch('/api/v1/torrent/add', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ url: magnetUrl }),
        })
        if (!res.ok) {
          const body = await res.text()
          throw new Error(body || `HTTP ${res.status}`)
        }
      } else if (hasFile) {
        // File upload — send as multipart form data
        const formData = new FormData()
        formData.append('file', hasFile)
        const res = await fetch('/api/v1/torrent/add/upload', {
          method: 'POST',
          headers: authHeaders(),
          body: formData,
        })
        if (!res.ok) {
          const body = await res.text()
          throw new Error(body || `HTTP ${res.status}`)
        }
      }

      onAdded()
      onClose()
    } catch (e) {
      setErrorMsg(e instanceof Error ? e.message : 'Failed to add torrent')
    } finally {
      setSubmitting(false)
    }
  }

  const handleDrag = (e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setDragActive(true)
    } else if (e.type === 'dragleave') {
      setDragActive(false)
    }
  }

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setDragActive(false)
    if (e.dataTransfer.files?.[0]) {
      const file = e.dataTransfer.files[0]
      if (file.name.endsWith('.torrent')) {
        if (fileRef.current) {
          const dt = new DataTransfer()
          dt.items.add(file)
          fileRef.current.files = dt.files
          setFileName(file.name)
        }
      }
    }
  }

  const handleFileChange = () => {
    const file = fileRef.current?.files?.[0]
    setFileName(file?.name ?? '')
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div className="w-full max-w-lg rounded-lg bg-slate-800 border border-slate-700 p-6 shadow-xl" onClick={e => e.stopPropagation()}>
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-lg font-semibold text-white">Add Torrent</h3>
          <button onClick={onClose} className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors">
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
              onChange={e => setMagnetUrl(e.target.value)}
              placeholder="magnet:?xt=urn:btih:..."
              className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white placeholder-slate-500 outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
            />
          </div>

          {/* Drag & drop zone */}
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">.torrent File</label>
            <div
              onDragEnter={handleDrag}
              onDragLeave={handleDrag}
              onDragOver={handleDrag}
              onDrop={handleDrop}
              onClick={() => fileRef.current?.click()}
              className={`flex cursor-pointer flex-col items-center justify-center rounded-lg border-2 border-dashed p-6 transition-colors ${
                dragActive
                  ? 'border-blue-500 bg-blue-500/10'
                  : 'border-slate-600 bg-slate-900/50 hover:border-slate-500'
              }`}
            >
              <Upload size={24} className={`mb-2 ${dragActive ? 'text-blue-400' : 'text-slate-500'}`} />
              {fileName ? (
                <span className="text-sm text-white">{fileName}</span>
              ) : (
                <>
                  <span className="text-sm text-slate-400">Drop .torrent file here or click to browse</span>
                  <span className="mt-1 text-xs text-slate-500">Supports .torrent files</span>
                </>
              )}
            </div>
            <input
              ref={fileRef}
              type="file"
              accept=".torrent"
              className="hidden"
              onChange={handleFileChange}
            />
          </div>

          {/* Category */}
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Category</label>
            <input
              type="text"
              value={category}
              onChange={e => setCategory(e.target.value)}
              placeholder="Optional category"
              className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white placeholder-slate-500 outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
            />
          </div>

          {/* Start paused */}
          <label className="flex items-center gap-2 text-sm text-slate-300 cursor-pointer">
            <input
              type="checkbox"
              checked={startPaused}
              onChange={e => setStartPaused(e.target.checked)}
              className="rounded border-slate-600 bg-slate-700 text-blue-500 focus:ring-blue-500"
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
            Add Torrent
          </button>
        </div>
      </div>
    </div>
  )
}

// ── Torrent Settings Tab ──────────────────────────────────────────────────

interface TorrentSettings {
  downloadFolder: string
  completedFolder: string | null
  uploadLimitBps: number
  downloadLimitBps: number
  peerLimit: number
  concurrentInitLimit: number
  dhtEnabled: boolean
}

function TorrentSettingsTab() {
  const [settings, setSettings] = useState<TorrentSettings | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [saved, setSaved] = useState(false)

  const load = useCallback(async () => {
    try {
      const res = await fetch('/api/v1/torrent/settings')
      if (res.ok) setSettings(await res.json() as TorrentSettings)
    } catch { /* empty */ }
    finally { setLoading(false) }
  }, [])

  useEffect(() => { void load() }, [load])

  const save = async (patch: Record<string, unknown>) => {
    setSaving(true)
    try {
      const res = await fetch('/api/v1/torrent/settings', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(patch),
      })
      if (res.ok) {
        setSettings(await res.json() as TorrentSettings)
        setSaved(true)
        setTimeout(() => setSaved(false), 2000)
      }
    } catch { /* empty */ }
    finally { setSaving(false) }
  }

  if (loading) return <div className="text-slate-400">Loading settings...</div>
  if (!settings) return <div className="text-slate-400">Torrent engine not initialized.</div>

  const kbpsToBytes = (kbps: number) => kbps * 1024
  const bytesToKbps = (bps: number) => Math.round(bps / 1024)

  return (
    <div className="space-y-6">
      <div className="rounded-lg border border-slate-700 bg-slate-800 p-5">
        <h3 className="mb-4 text-sm font-semibold uppercase tracking-wider text-slate-400">Speed Limits</h3>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Download Limit (KB/s)</label>
            <input
              type="number" min={0} step={100}
              value={bytesToKbps(settings.downloadLimitBps)}
              onChange={(e) => {
                const bps = kbpsToBytes(Math.max(0, Number(e.target.value) || 0))
                setSettings({ ...settings, downloadLimitBps: bps })
              }}
              onBlur={() => void save({ downloadLimitBps: settings.downloadLimitBps })}
              className="w-full rounded bg-slate-700 border border-slate-600 px-3 py-2 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
            />
            <p className="mt-1 text-xs text-slate-500">0 = unlimited</p>
          </div>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Upload Limit (KB/s)</label>
            <input
              type="number" min={0} step={100}
              value={bytesToKbps(settings.uploadLimitBps)}
              onChange={(e) => {
                const bps = kbpsToBytes(Math.max(0, Number(e.target.value) || 0))
                setSettings({ ...settings, uploadLimitBps: bps })
              }}
              onBlur={() => void save({ uploadLimitBps: settings.uploadLimitBps })}
              className="w-full rounded bg-slate-700 border border-slate-600 px-3 py-2 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
            />
            <p className="mt-1 text-xs text-slate-500">0 = unlimited</p>
          </div>
        </div>
      </div>

      <div className="rounded-lg border border-slate-700 bg-slate-800 p-5">
        <h3 className="mb-4 text-sm font-semibold uppercase tracking-wider text-slate-400">Connection Settings</h3>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Max Peers per Torrent</label>
            <input
              type="number" min={1} max={1000}
              value={settings.peerLimit}
              onChange={(e) => {
                const v = Math.max(1, Math.min(1000, Number(e.target.value) || 50))
                setSettings({ ...settings, peerLimit: v })
              }}
              onBlur={() => void save({ peerLimit: settings.peerLimit })}
              className="w-full rounded bg-slate-700 border border-slate-600 px-3 py-2 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
            />
          </div>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Concurrent Init Limit</label>
            <input
              type="number" min={1} max={20}
              value={settings.concurrentInitLimit}
              onChange={(e) => {
                const v = Math.max(1, Math.min(20, Number(e.target.value) || 3))
                setSettings({ ...settings, concurrentInitLimit: v })
              }}
              onBlur={() => void save({ concurrentInitLimit: settings.concurrentInitLimit })}
              className="w-full rounded bg-slate-700 border border-slate-600 px-3 py-2 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
            />
            <p className="mt-1 text-xs text-slate-500">Max torrents checking/initializing at once</p>
          </div>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">DHT</label>
            <div className={`rounded px-3 py-2 text-sm font-medium ${settings.dhtEnabled ? 'bg-green-500/10 text-green-400' : 'bg-slate-700/50 text-slate-500'}`}>
              {settings.dhtEnabled ? 'Enabled' : 'Disabled'}
            </div>
            <p className="mt-1 text-xs text-slate-500">Configured at startup via config file</p>
          </div>
        </div>
      </div>

      <div className="rounded-lg border border-slate-700 bg-slate-800 p-5">
        <h3 className="mb-4 text-sm font-semibold uppercase tracking-wider text-slate-400">Directories</h3>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Download Directory</label>
            <input
              type="text"
              value={settings.downloadFolder}
              onChange={(e) => setSettings({ ...settings, downloadFolder: e.target.value })}
              onBlur={() => void save({ downloadFolder: settings.downloadFolder })}
              className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
            />
          </div>
          <div>
            <label className="mb-1 block text-sm font-medium text-slate-300">Completed Directory</label>
            <input
              type="text"
              value={settings.completedFolder ?? ''}
              onChange={(e) => setSettings({ ...settings, completedFolder: e.target.value || null })}
              onBlur={() => void save({ completedFolder: settings.completedFolder })}
              placeholder="Not set (stays in download dir)"
              className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors placeholder:text-slate-600"
            />
          </div>
        </div>
      </div>

      {(saving || saved) && (
        <div className={`text-sm ${saved ? 'text-green-400' : 'text-slate-400'}`}>
          {saved ? 'Settings saved.' : 'Saving...'}
        </div>
      )}
    </div>
  )
}
