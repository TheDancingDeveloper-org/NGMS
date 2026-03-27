import { useState, useEffect, useCallback, useRef, type DragEvent } from 'react'
import {
  HardDrive,
  Loader2,
  Pause,
  Play,
  Trash2,
  Plus,
  ChevronUp,
  ChevronDown,
  RefreshCw,
  X,
  Shield,
  Check,
  AlertTriangle,
  RotateCcw,
  Pencil,
  Download,
  Upload,
  Clock,
  Gauge,
  Timer,
  Link,
  FileText,
  BarChart3,
  ScrollText,
  Zap,
} from 'lucide-react'

// ── Types ──────────────────────────────────────────────────────────────────

interface UsenetStats {
  downloadSpeed: number
  queueSize: number
  activeDownloads: number
  paused: boolean
  totalSize?: number
}

interface QueueItem {
  id: string
  name: string
  size: number
  progress: number
  speed: number
  status: string
  eta: number // seconds
  errorMessage?: string
  category?: string
  priority?: string
  totalArticles?: number
  downloadedArticles?: number
  files?: JobFile[]
  logs?: string[]
}

interface HistoryItem {
  id: string
  name: string
  size: number
  status: string
  completedAt: string
  par2Status?: string
  repairStatus?: string
  extractStatus?: string
}

interface NntpServer {
  id: string
  name: string
  host: string
  port: number
  ssl: boolean
  username: string
  password: string
  connections: number
  priority: number
  optional: boolean
  enabled: boolean
  status?: string
}

interface JobFile {
  name: string
  size: number
  status: string
}

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

function formatSpeedShort(bytesPerSec: number): string {
  if (!bytesPerSec || !isFinite(bytesPerSec) || bytesPerSec <= 0) return '0'
  if (bytesPerSec < 1024 * 1024) return `${(bytesPerSec / 1024).toFixed(0)}`
  return `${(bytesPerSec / (1024 * 1024)).toFixed(1)}`
}

function formatSpeedUnit(bytesPerSec: number): string {
  if (!bytesPerSec || !isFinite(bytesPerSec) || bytesPerSec <= 0) return 'KB/s'
  if (bytesPerSec < 1024 * 1024) return 'KB/s'
  return 'MB/s'
}

function queueProgressColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'downloading': return 'bg-blue-500'
    case 'verifying': return 'bg-yellow-500'
    case 'repairing': return 'bg-orange-500'
    case 'extracting': return 'bg-purple-500'
    case 'completed': return 'bg-green-500'
    case 'failed': return 'bg-red-500'
    case 'paused': return 'bg-yellow-500'
    case 'queued': return 'bg-slate-500'
    default: return 'bg-slate-500'
  }
}

function queueBadgeColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'queued': return 'bg-slate-600 text-slate-300'
    case 'downloading': return 'bg-blue-500/20 text-blue-400'
    case 'paused': return 'bg-yellow-500/20 text-yellow-400'
    case 'verifying': return 'bg-yellow-500/20 text-yellow-400'
    case 'repairing': return 'bg-orange-500/20 text-orange-400'
    case 'extracting': return 'bg-purple-500/20 text-purple-400'
    case 'completed': return 'bg-green-500/20 text-green-400'
    case 'failed': return 'bg-red-500/20 text-red-400'
    default: return 'bg-slate-600 text-slate-300'
  }
}

function historyBadgeColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'completed': return 'bg-green-500/20 text-green-400'
    case 'failed': return 'bg-red-500/20 text-red-400'
    default: return 'bg-slate-600 text-slate-300'
  }
}

function stageIcon(result: string | undefined) {
  if (!result) return null
  switch (result.toLowerCase()) {
    case 'success': return <Check size={12} className="text-green-400" />
    case 'failed': return <AlertTriangle size={12} className="text-red-400" />
    default: return <span className="text-xs text-slate-400">{result}</span>
  }
}

// ── Speed Graph ────────────────────────────────────────────────────────────

function SpeedGraph({ dataPoints }: { dataPoints: number[] }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const dpr = window.devicePixelRatio || 1
    const rect = canvas.getBoundingClientRect()
    canvas.width = rect.width * dpr
    canvas.height = rect.height * dpr
    ctx.scale(dpr, dpr)

    const w = rect.width
    const h = rect.height

    ctx.clearRect(0, 0, w, h)

    if (dataPoints.length < 2) return

    const max = Math.max(...dataPoints, 1)
    const step = w / (dataPoints.length - 1)

    // Fill gradient
    const gradient = ctx.createLinearGradient(0, 0, 0, h)
    gradient.addColorStop(0, 'rgba(59, 130, 246, 0.3)')
    gradient.addColorStop(1, 'rgba(59, 130, 246, 0)')

    ctx.beginPath()
    ctx.moveTo(0, h)
    dataPoints.forEach((val, i) => {
      const x = i * step
      const y = h - (val / max) * h * 0.9
      if (i === 0) ctx.lineTo(x, y)
      else ctx.lineTo(x, y)
    })
    ctx.lineTo(w, h)
    ctx.closePath()
    ctx.fillStyle = gradient
    ctx.fill()

    // Line
    ctx.beginPath()
    dataPoints.forEach((val, i) => {
      const x = i * step
      const y = h - (val / max) * h * 0.9
      if (i === 0) ctx.moveTo(x, y)
      else ctx.lineTo(x, y)
    })
    ctx.strokeStyle = '#3b82f6'
    ctx.lineWidth = 1.5
    ctx.stroke()
  }, [dataPoints])

  return (
    <canvas
      ref={canvasRef}
      className="h-full w-full"
      style={{ display: 'block' }}
    />
  )
}

// ── Component ──────────────────────────────────────────────────────────────

export default function Usenet() {
  const [activeTab, setActiveTab] = useState<'queue' | 'history' | 'servers'>('queue')
  const [stats, setStats] = useState<UsenetStats | null>(null)
  const [speedHistory, setSpeedHistory] = useState<number[]>([])
  const [showAddNzb, setShowAddNzb] = useState(false)
  const [showSpeedLimit, setShowSpeedLimit] = useState(false)
  const [showPauseMenu, setShowPauseMenu] = useState(false)
  const speedLimitRef = useRef<HTMLDivElement>(null)
  const pauseMenuRef = useRef<HTMLDivElement>(null)

  const fetchStats = useCallback(async () => {
    try {
      const res = await fetch('/api/v1/usenet/status')
      if (res.ok) {
        const data = await res.json() as UsenetStats
        setStats(data)
        setSpeedHistory(prev => {
          const next = [...prev, data.downloadSpeed]
          return next.length > 60 ? next.slice(-60) : next
        })
      }
    } catch {
      // ignore
    }
  }, [])

  useEffect(() => {
    void fetchStats()
    const interval = setInterval(() => void fetchStats(), 1000)
    return () => clearInterval(interval)
  }, [fetchStats])

  // Close dropdowns on outside click
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (speedLimitRef.current && !speedLimitRef.current.contains(e.target as Node)) {
        setShowSpeedLimit(false)
      }
      if (pauseMenuRef.current && !pauseMenuRef.current.contains(e.target as Node)) {
        setShowPauseMenu(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [])

  const pauseAll = async () => {
    await fetch('/api/v1/usenet/pause-all', { method: 'POST' })
    void fetchStats()
  }

  const resumeAll = async () => {
    await fetch('/api/v1/usenet/resume-all', { method: 'POST' })
    void fetchStats()
  }

  const timedPause = async (minutes: number) => {
    await fetch('/api/v1/usenet/pause-all', { method: 'POST' })
    setShowPauseMenu(false)
    // Schedule resume after timeout
    setTimeout(() => {
      void fetch('/api/v1/usenet/resume-all', { method: 'POST' })
    }, minutes * 60 * 1000)
    void fetchStats()
  }

  const setSpeedLimit = async (bytesPerSecond: number) => {
    await fetch('/api/v1/usenet/speed-limit', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ bytesPerSecond }),
    })
    setShowSpeedLimit(false)
  }

  const tabClass = (tab: string) =>
    `px-4 py-2 text-sm font-medium rounded-t-lg transition-colors ${
      activeTab === tab
        ? 'bg-slate-800 text-white border-b-2 border-blue-500'
        : 'text-slate-400 hover:text-white hover:bg-slate-800/50'
    }`

  const speedLimits = [
    { label: 'No limit', value: 0 },
    { label: '1 MB/s', value: 1024 * 1024 },
    { label: '5 MB/s', value: 5 * 1024 * 1024 },
    { label: '10 MB/s', value: 10 * 1024 * 1024 },
    { label: '25 MB/s', value: 25 * 1024 * 1024 },
    { label: '50 MB/s', value: 50 * 1024 * 1024 },
    { label: '100 MB/s', value: 100 * 1024 * 1024 },
  ]

  const pauseTimers = [
    { label: '5 minutes', minutes: 5 },
    { label: '10 minutes', minutes: 10 },
    { label: '30 minutes', minutes: 30 },
    { label: '1 hour', minutes: 60 },
  ]

  return (
    <div>
      <h2 className="mb-6 text-2xl font-bold">Usenet</h2>

      {/* ── Dashboard Header ──────────────────────────────────────────── */}
      <div className="mb-4 rounded-lg border border-slate-700 bg-slate-800 p-4">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          {/* Left: Speed + Graph */}
          <div className="flex items-center gap-4">
            {/* Connection status dot */}
            <div className="flex items-center gap-2">
              <div className={`h-2.5 w-2.5 rounded-full ${
                !stats ? 'bg-slate-500' : stats.paused ? 'bg-yellow-400' : 'bg-green-400'
              }`} />
              <span className="text-xs text-slate-400">
                {!stats ? 'Connecting' : stats.paused ? 'Paused' : 'Active'}
              </span>
            </div>

            {/* Speed display */}
            <div className="flex items-center gap-3">
              <Download size={18} className="text-blue-400" />
              <div>
                <div className="flex items-baseline gap-1">
                  <span className="text-2xl font-bold text-white tabular-nums">
                    {formatSpeedShort(stats?.downloadSpeed ?? 0)}
                  </span>
                  <span className="text-sm text-slate-400">
                    {formatSpeedUnit(stats?.downloadSpeed ?? 0)}
                  </span>
                </div>
              </div>
            </div>

            {/* Mini speed graph */}
            <div className="hidden h-10 w-32 sm:block">
              <SpeedGraph dataPoints={speedHistory} />
            </div>
          </div>

          {/* Center: Stats */}
          <div className="flex items-center gap-6 text-sm">
            <div className="flex items-center gap-1.5 text-slate-300">
              <HardDrive size={14} className="text-slate-500" />
              <span className="text-slate-400">Queue:</span>
              <span className="font-medium text-white">{stats?.queueSize ?? 0}</span>
              {stats?.totalSize ? (
                <span className="text-slate-500">({formatSize(stats.totalSize)})</span>
              ) : null}
            </div>
            <div className="flex items-center gap-1.5 text-slate-300">
              <Zap size={14} className="text-slate-500" />
              <span className="text-slate-400">Active:</span>
              <span className="font-medium text-white">{stats?.activeDownloads ?? 0}</span>
            </div>
          </div>

          {/* Right: Controls */}
          <div className="flex items-center gap-2">
            {/* Speed Limit Dropdown */}
            <div className="relative" ref={speedLimitRef}>
              <button
                onClick={() => setShowSpeedLimit(!showSpeedLimit)}
                className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm text-slate-300 hover:bg-slate-600 transition-colors"
                title="Speed Limit"
              >
                <Gauge size={14} />
                <span className="hidden sm:inline">Speed</span>
                <ChevronDown size={12} />
              </button>
              {showSpeedLimit && (
                <div className="absolute right-0 top-full z-30 mt-1 w-40 rounded-lg border border-slate-700 bg-slate-800 py-1 shadow-xl">
                  {speedLimits.map((limit) => (
                    <button
                      key={limit.value}
                      onClick={() => void setSpeedLimit(limit.value)}
                      className="block w-full px-3 py-1.5 text-left text-sm text-slate-300 hover:bg-slate-700 hover:text-white transition-colors"
                    >
                      {limit.label}
                    </button>
                  ))}
                  <div className="border-t border-slate-700 px-3 py-2">
                    <CustomSpeedInput onSubmit={(v) => void setSpeedLimit(v)} />
                  </div>
                </div>
              )}
            </div>

            {/* Pause / Resume */}
            {stats?.paused ? (
              <button
                onClick={() => void resumeAll()}
                className="flex items-center gap-1.5 rounded-lg bg-green-600 px-3 py-2 text-sm font-medium text-white hover:bg-green-500 transition-colors"
              >
                <Play size={14} />
                <span className="hidden sm:inline">Resume</span>
              </button>
            ) : (
              <div className="relative" ref={pauseMenuRef}>
                <div className="flex">
                  <button
                    onClick={() => void pauseAll()}
                    className="flex items-center gap-1.5 rounded-l-lg bg-yellow-600 px-3 py-2 text-sm font-medium text-white hover:bg-yellow-500 transition-colors"
                  >
                    <Pause size={14} />
                    <span className="hidden sm:inline">Pause</span>
                  </button>
                  <button
                    onClick={() => setShowPauseMenu(!showPauseMenu)}
                    className="rounded-r-lg border-l border-yellow-700 bg-yellow-600 px-1.5 py-2 text-white hover:bg-yellow-500 transition-colors"
                  >
                    <ChevronDown size={12} />
                  </button>
                </div>
                {showPauseMenu && (
                  <div className="absolute right-0 top-full z-30 mt-1 w-36 rounded-lg border border-slate-700 bg-slate-800 py-1 shadow-xl">
                    {pauseTimers.map((t) => (
                      <button
                        key={t.minutes}
                        onClick={() => void timedPause(t.minutes)}
                        className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm text-slate-300 hover:bg-slate-700 hover:text-white transition-colors"
                      >
                        <Timer size={12} className="text-slate-500" />
                        {t.label}
                      </button>
                    ))}
                  </div>
                )}
              </div>
            )}

            {/* Add NZB */}
            <button
              onClick={() => setShowAddNzb(true)}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
            >
              <Plus size={14} />
              <span className="hidden sm:inline">Add NZB</span>
            </button>
          </div>
        </div>
      </div>

      {/* ── Tab Bar ───────────────────────────────────────────────────── */}
      <div className="mb-4 flex gap-1 border-b border-slate-700">
        <button className={tabClass('queue')} onClick={() => setActiveTab('queue')}>Queue</button>
        <button className={tabClass('history')} onClick={() => setActiveTab('history')}>History</button>
        <button className={tabClass('servers')} onClick={() => setActiveTab('servers')}>Servers</button>
      </div>

      {activeTab === 'queue' && <QueueTab />}
      {activeTab === 'history' && <HistoryTab />}
      {activeTab === 'servers' && <ServersTab />}

      {/* ── Add NZB Modal ─────────────────────────────────────────────── */}
      {showAddNzb && <AddNzbModal onClose={() => setShowAddNzb(false)} />}
    </div>
  )
}

// ── Custom Speed Input ─────────────────────────────────────────────────────

function CustomSpeedInput({ onSubmit }: { onSubmit: (bytes: number) => void }) {
  const [value, setValue] = useState('')

  const handleSubmit = () => {
    const num = parseFloat(value)
    if (num > 0) {
      onSubmit(num * 1024 * 1024)
    }
  }

  return (
    <div className="flex items-center gap-1">
      <input
        type="number"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={(e) => { if (e.key === 'Enter') handleSubmit() }}
        placeholder="Custom"
        className="w-16 rounded bg-slate-900 px-2 py-1 text-xs text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500"
        min={0}
        step={0.1}
      />
      <span className="text-xs text-slate-500">MB/s</span>
      <button
        onClick={handleSubmit}
        className="rounded bg-blue-600 px-1.5 py-1 text-xs text-white hover:bg-blue-500"
      >
        Set
      </button>
    </div>
  )
}

// ── Add NZB Modal ──────────────────────────────────────────────────────────

function AddNzbModal({ onClose }: { onClose: () => void }) {
  const [url, setUrl] = useState('')
  const [category, setCategory] = useState('')
  const [priority, setPriority] = useState('normal')
  const [file, setFile] = useState<File | null>(null)
  const [dragActive, setDragActive] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleDrag = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault()
    e.stopPropagation()
    if (e.type === 'dragenter' || e.type === 'dragover') {
      setDragActive(true)
    } else if (e.type === 'dragleave') {
      setDragActive(false)
    }
  }

  const handleDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault()
    e.stopPropagation()
    setDragActive(false)
    if (e.dataTransfer.files?.[0]) {
      setFile(e.dataTransfer.files[0])
      setUrl('')
    }
  }

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files?.[0]) {
      setFile(e.target.files[0])
      setUrl('')
    }
  }

  const handleSubmit = async () => {
    if (!url && !file) return
    setSubmitting(true)
    setError(null)

    try {
      if (file) {
        const formData = new FormData()
        formData.append('file', file)
        if (category) formData.append('category', category)
        formData.append('priority', priority)
        const res = await fetch('/api/v1/usenet/add', {
          method: 'POST',
          body: formData,
        })
        if (!res.ok) throw new Error(`Upload failed (${res.status})`)
      } else {
        const res = await fetch('/api/v1/usenet/add', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ url, category, priority }),
        })
        if (!res.ok) throw new Error(`Add failed (${res.status})`)
      }
      onClose()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to add NZB')
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="w-full max-w-lg rounded-lg bg-slate-800 p-6" onClick={(e) => e.stopPropagation()}>
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-lg font-semibold text-white">Add NZB</h3>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition-colors">
            <X size={20} />
          </button>
        </div>

        <div className="space-y-4">
          {/* URL Input */}
          <FormField label="NZB URL">
            <div className="flex items-center gap-2">
              <Link size={14} className="text-slate-500" />
              <input
                type="text"
                value={url}
                onChange={(e) => { setUrl(e.target.value); setFile(null) }}
                placeholder="https://example.com/file.nzb"
                className="flex-1 rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
                disabled={!!file}
              />
            </div>
          </FormField>

          {/* Divider */}
          <div className="flex items-center gap-3">
            <div className="h-px flex-1 bg-slate-700" />
            <span className="text-xs text-slate-500">OR</span>
            <div className="h-px flex-1 bg-slate-700" />
          </div>

          {/* File Upload */}
          <div
            onDragEnter={handleDrag}
            onDragLeave={handleDrag}
            onDragOver={handleDrag}
            onDrop={handleDrop}
            onClick={() => fileInputRef.current?.click()}
            className={`cursor-pointer rounded-lg border-2 border-dashed p-6 text-center transition-colors ${
              dragActive
                ? 'border-blue-500 bg-blue-500/10'
                : file
                  ? 'border-green-500/50 bg-green-500/5'
                  : 'border-slate-600 hover:border-slate-500 hover:bg-slate-700/30'
            }`}
          >
            <input
              ref={fileInputRef}
              type="file"
              accept=".nzb,.nzb.gz"
              onChange={handleFileChange}
              className="hidden"
            />
            {file ? (
              <div className="flex items-center justify-center gap-2 text-sm text-green-400">
                <Check size={16} />
                <span>{file.name}</span>
                <button
                  onClick={(e) => { e.stopPropagation(); setFile(null) }}
                  className="ml-2 rounded p-0.5 hover:bg-slate-700 text-slate-400 hover:text-white"
                >
                  <X size={14} />
                </button>
              </div>
            ) : (
              <>
                <Upload size={24} className="mx-auto mb-2 text-slate-500" />
                <p className="text-sm text-slate-400">
                  Drop NZB file here or <span className="text-blue-400">browse</span>
                </p>
                <p className="mt-1 text-xs text-slate-500">.nzb or .nzb.gz</p>
              </>
            )}
          </div>

          {/* Category + Priority */}
          <div className="grid grid-cols-2 gap-3">
            <FormField label="Category">
              <select
                value={category}
                onChange={(e) => setCategory(e.target.value)}
                className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
              >
                <option value="">None</option>
                <option value="movies">Movies</option>
                <option value="tv">TV</option>
                <option value="music">Music</option>
                <option value="software">Software</option>
                <option value="other">Other</option>
              </select>
            </FormField>
            <FormField label="Priority">
              <select
                value={priority}
                onChange={(e) => setPriority(e.target.value)}
                className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
              >
                <option value="low">Low</option>
                <option value="normal">Normal</option>
                <option value="high">High</option>
                <option value="force">Force</option>
              </select>
            </FormField>
          </div>

          {error && (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">
              {error}
            </div>
          )}
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
            disabled={submitting || (!url && !file)}
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

// ── Queue Tab ──────────────────────────────────────────────────────────────

function QueueTab() {
  const [items, setItems] = useState<QueueItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [detailItem, setDetailItem] = useState<QueueItem | null>(null)

  const fetchQueue = useCallback(async () => {
    try {
      const res = await fetch('/api/v1/usenet/queue')
      if (res.ok) {
        const data = await res.json() as { jobs?: QueueItem[] }
        setItems(data.jobs ?? [])
        setError(null)
      } else {
        setError(`Failed to fetch queue (${res.status})`)
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Network error')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void fetchQueue()
    const interval = setInterval(() => void fetchQueue(), 3000)
    return () => clearInterval(interval)
  }, [fetchQueue])

  const pauseItem = async (id: string) => {
    await fetch(`/api/v1/usenet/queue/${id}/pause`, { method: 'POST' })
    void fetchQueue()
  }

  const resumeItem = async (id: string) => {
    await fetch(`/api/v1/usenet/queue/${id}/resume`, { method: 'POST' })
    void fetchQueue()
  }

  const deleteItem = async (id: string) => {
    await fetch(`/api/v1/usenet/queue/${id}/delete`, { method: 'POST' })
    setSelected(prev => { const next = new Set(prev); next.delete(id); return next })
    void fetchQueue()
  }

  const priorityUp = async (id: string) => {
    const idx = items.findIndex((i) => i.id === id)
    if (idx > 0) {
      const reordered = [...items]
      const temp = reordered[idx - 1]
      reordered[idx - 1] = reordered[idx]
      reordered[idx] = temp
      setItems(reordered)
    }
    await fetch(`/api/v1/usenet/queue/${id}/priority`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ direction: 'up' }),
    })
    void fetchQueue()
  }

  const priorityDown = async (id: string) => {
    const idx = items.findIndex((i) => i.id === id)
    if (idx < items.length - 1) {
      const reordered = [...items]
      const temp = reordered[idx + 1]
      reordered[idx + 1] = reordered[idx]
      reordered[idx] = temp
      setItems(reordered)
    }
    await fetch(`/api/v1/usenet/queue/${id}/priority`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ direction: 'down' }),
    })
    void fetchQueue()
  }

  // ── Bulk actions ──
  const toggleSelect = (id: string) => {
    setSelected(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  const toggleSelectAll = () => {
    if (selected.size === items.length) {
      setSelected(new Set())
    } else {
      setSelected(new Set(items.map(i => i.id)))
    }
  }

  const bulkPause = async () => {
    await Promise.all([...selected].map(id =>
      fetch(`/api/v1/usenet/queue/${id}/pause`, { method: 'POST' })
    ))
    void fetchQueue()
  }

  const bulkResume = async () => {
    await Promise.all([...selected].map(id =>
      fetch(`/api/v1/usenet/queue/${id}/resume`, { method: 'POST' })
    ))
    void fetchQueue()
  }

  const bulkDelete = async () => {
    await Promise.all([...selected].map(id =>
      fetch(`/api/v1/usenet/queue/${id}/delete`, { method: 'POST' })
    ))
    setSelected(new Set())
    void fetchQueue()
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 size={32} className="animate-spin text-blue-500" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">{error}</div>
    )
  }

  if (items.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-slate-400">
        <HardDrive size={48} className="mb-4 text-slate-600" />
        <p className="text-lg font-medium">Queue is empty</p>
        <p className="mt-1 text-sm text-slate-500">Add an NZB to start downloading</p>
      </div>
    )
  }

  return (
    <div>
      {/* Bulk action bar */}
      {selected.size > 0 && (
        <div className="mb-3 flex items-center gap-3 rounded-lg border border-blue-500/30 bg-blue-500/10 px-4 py-2">
          <span className="text-sm text-blue-400">{selected.size} selected</span>
          <div className="flex items-center gap-1">
            <button
              onClick={() => void bulkPause()}
              className="flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-300 hover:bg-slate-700 transition-colors"
            >
              <Pause size={12} /> Pause
            </button>
            <button
              onClick={() => void bulkResume()}
              className="flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-300 hover:bg-slate-700 transition-colors"
            >
              <Play size={12} /> Resume
            </button>
            <button
              onClick={() => void bulkDelete()}
              className="flex items-center gap-1 rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/20 transition-colors"
            >
              <Trash2 size={12} /> Delete
            </button>
          </div>
          <button
            onClick={() => setSelected(new Set())}
            className="ml-auto text-xs text-slate-400 hover:text-white transition-colors"
          >
            Clear selection
          </button>
        </div>
      )}

      <div className="overflow-x-auto rounded-lg bg-slate-800">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
              <th className="px-3 py-3 font-medium w-8">
                <input
                  type="checkbox"
                  checked={items.length > 0 && selected.size === items.length}
                  onChange={toggleSelectAll}
                  className="rounded border-slate-600 bg-slate-700"
                />
              </th>
              <th className="px-4 py-3 font-medium">Name</th>
              <th className="px-4 py-3 font-medium">Status</th>
              <th className="px-4 py-3 font-medium w-48">Progress</th>
              <th className="px-4 py-3 font-medium">Speed</th>
              <th className="px-4 py-3 font-medium">Size</th>
              <th className="px-4 py-3 font-medium">ETA</th>
              <th className="px-4 py-3 font-medium">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-700">
            {items.map((item) => (
              <tr key={item.id} className={`transition-colors ${
                selected.has(item.id) ? 'bg-blue-500/5' : 'hover:bg-slate-700/30'
              }`}>
                <td className="px-3 py-3">
                  <input
                    type="checkbox"
                    checked={selected.has(item.id)}
                    onChange={() => toggleSelect(item.id)}
                    className="rounded border-slate-600 bg-slate-700"
                  />
                </td>
                <td className="px-4 py-3">
                  <button
                    onClick={() => setDetailItem(item)}
                    className="text-left font-medium text-white hover:text-blue-400 transition-colors max-w-xs truncate block"
                    title={item.name}
                  >
                    {item.name}
                  </button>
                  {item.status.toLowerCase() === 'failed' && item.errorMessage && (
                    <div className="mt-1 text-xs text-red-400">{item.errorMessage}</div>
                  )}
                </td>
                <td className="px-4 py-3">
                  <span className={`rounded-full px-2 py-0.5 text-xs font-medium capitalize ${queueBadgeColor(item.status)}`}>
                    {item.status}
                  </span>
                </td>
                <td className="px-4 py-3">
                  <div className="flex items-center gap-2">
                    <div className="h-2 flex-1 overflow-hidden rounded-full bg-slate-600">
                      <div
                        className={`h-full rounded-full ${queueProgressColor(item.status)} transition-all`}
                        style={{ width: `${item.progress}%` }}
                      />
                    </div>
                    <span className="w-10 text-right text-xs tabular-nums text-slate-400">{Math.round(item.progress)}%</span>
                  </div>
                </td>
                <td className="px-4 py-3 text-slate-300 whitespace-nowrap tabular-nums">{formatSpeed(item.speed)}</td>
                <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatSize(item.size)}</td>
                <td className="px-4 py-3 text-slate-300 whitespace-nowrap tabular-nums">{formatEta(item.eta)}</td>
                <td className="px-4 py-3">
                  <div className="flex items-center gap-1">
                    {item.status.toLowerCase() === 'paused' ? (
                      <button
                        onClick={() => void resumeItem(item.id)}
                        className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
                        title="Resume"
                      >
                        <Play size={14} />
                      </button>
                    ) : (
                      <button
                        onClick={() => void pauseItem(item.id)}
                        className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
                        title="Pause"
                      >
                        <Pause size={14} />
                      </button>
                    )}
                    <button
                      onClick={() => void deleteItem(item.id)}
                      className="rounded p-1 text-slate-400 hover:bg-red-500/20 hover:text-red-400 transition-colors"
                      title="Delete"
                    >
                      <Trash2 size={14} />
                    </button>
                    <button
                      onClick={() => void priorityUp(item.id)}
                      className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
                      title="Priority up"
                    >
                      <ChevronUp size={14} />
                    </button>
                    <button
                      onClick={() => void priorityDown(item.id)}
                      className="rounded p-1 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
                      title="Priority down"
                    >
                      <ChevronDown size={14} />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Job Details Modal */}
      {detailItem && (
        <JobDetailModal item={detailItem} onClose={() => setDetailItem(null)} />
      )}
    </div>
  )
}

// ── Job Detail Modal ───────────────────────────────────────────────────────

function JobDetailModal({ item, onClose }: { item: QueueItem; onClose: () => void }) {
  const [activeTab, setActiveTab] = useState<'files' | 'stats' | 'logs'>('files')
  const [files, setFiles] = useState<JobFile[]>(item.files ?? [])
  const [logs, setLogs] = useState<string[]>(item.logs ?? [])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const fetchDetails = async () => {
      try {
        const res = await fetch(`/api/v1/usenet/queue/${item.id}`)
        if (res.ok) {
          const data = await res.json() as QueueItem
          setFiles(data.files ?? [])
          setLogs(data.logs ?? [])
        }
      } catch {
        // ignore
      } finally {
        setLoading(false)
      }
    }
    void fetchDetails()
  }, [item.id])

  const detailTabClass = (tab: string) =>
    `px-3 py-1.5 text-xs font-medium rounded transition-colors ${
      activeTab === tab
        ? 'bg-slate-700 text-white'
        : 'text-slate-400 hover:text-white hover:bg-slate-700/50'
    }`

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="w-full max-w-2xl rounded-lg bg-slate-800 p-6" onClick={(e) => e.stopPropagation()}>
        <div className="mb-4 flex items-center justify-between">
          <div className="min-w-0 flex-1 pr-4">
            <h3 className="truncate text-lg font-semibold text-white" title={item.name}>{item.name}</h3>
            <div className="mt-1 flex items-center gap-3 text-xs text-slate-400">
              <span>{formatSize(item.size)}</span>
              <span className={`rounded-full px-2 py-0.5 font-medium capitalize ${queueBadgeColor(item.status)}`}>
                {item.status}
              </span>
              <span>{Math.round(item.progress)}%</span>
            </div>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition-colors">
            <X size={20} />
          </button>
        </div>

        {/* Sub-tabs */}
        <div className="mb-4 flex gap-1">
          <button className={detailTabClass('files')} onClick={() => setActiveTab('files')}>
            <span className="flex items-center gap-1"><FileText size={12} /> Files</span>
          </button>
          <button className={detailTabClass('stats')} onClick={() => setActiveTab('stats')}>
            <span className="flex items-center gap-1"><BarChart3 size={12} /> Stats</span>
          </button>
          <button className={detailTabClass('logs')} onClick={() => setActiveTab('logs')}>
            <span className="flex items-center gap-1"><ScrollText size={12} /> Logs</span>
          </button>
        </div>

        {loading ? (
          <div className="flex items-center justify-center py-12">
            <Loader2 size={24} className="animate-spin text-blue-500" />
          </div>
        ) : (
          <>
            {activeTab === 'files' && (
              <div className="max-h-80 overflow-y-auto rounded-lg border border-slate-700">
                {files.length === 0 ? (
                  <div className="py-8 text-center text-sm text-slate-500">No file data available</div>
                ) : (
                  <table className="w-full text-xs">
                    <thead className="sticky top-0 bg-slate-800">
                      <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
                        <th className="px-3 py-2 font-medium">File</th>
                        <th className="px-3 py-2 font-medium w-20">Size</th>
                        <th className="px-3 py-2 font-medium w-20">Status</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-700/50">
                      {files.map((f, i) => (
                        <tr key={i} className="hover:bg-slate-700/20">
                          <td className="px-3 py-1.5 text-slate-300 truncate max-w-md" title={f.name}>{f.name}</td>
                          <td className="px-3 py-1.5 text-slate-400 whitespace-nowrap">{formatSize(f.size)}</td>
                          <td className="px-3 py-1.5">
                            <span className={`capitalize text-xs ${
                              f.status === 'completed' ? 'text-green-400' :
                              f.status === 'downloading' ? 'text-blue-400' :
                              f.status === 'failed' ? 'text-red-400' :
                              'text-slate-400'
                            }`}>
                              {f.status}
                            </span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </div>
            )}

            {activeTab === 'stats' && (
              <div className="grid grid-cols-2 gap-3">
                <div className="rounded-lg bg-slate-900 p-3">
                  <div className="text-xs text-slate-400">Total Size</div>
                  <div className="text-sm font-medium text-white">{formatSize(item.size)}</div>
                </div>
                <div className="rounded-lg bg-slate-900 p-3">
                  <div className="text-xs text-slate-400">Downloaded</div>
                  <div className="text-sm font-medium text-white">{formatSize(item.size * (item.progress / 100))}</div>
                </div>
                <div className="rounded-lg bg-slate-900 p-3">
                  <div className="text-xs text-slate-400">Speed</div>
                  <div className="text-sm font-medium text-white">{formatSpeed(item.speed)}</div>
                </div>
                <div className="rounded-lg bg-slate-900 p-3">
                  <div className="text-xs text-slate-400">ETA</div>
                  <div className="text-sm font-medium text-white">{formatEta(item.eta)}</div>
                </div>
                <div className="rounded-lg bg-slate-900 p-3">
                  <div className="text-xs text-slate-400">Articles</div>
                  <div className="text-sm font-medium text-white">
                    {item.downloadedArticles !== undefined && item.totalArticles !== undefined
                      ? `${item.downloadedArticles.toLocaleString()} / ${item.totalArticles.toLocaleString()}`
                      : '-'}
                  </div>
                </div>
                <div className="rounded-lg bg-slate-900 p-3">
                  <div className="text-xs text-slate-400">Files</div>
                  <div className="text-sm font-medium text-white">{files.length || '-'}</div>
                </div>
              </div>
            )}

            {activeTab === 'logs' && (
              <div className="max-h-80 overflow-y-auto rounded-lg border border-slate-700 bg-slate-900 p-3">
                {logs.length === 0 ? (
                  <div className="py-8 text-center text-sm text-slate-500">No logs available</div>
                ) : (
                  <div className="space-y-0.5 font-mono text-xs text-slate-300">
                    {logs.map((line, i) => (
                      <div key={i} className="whitespace-pre-wrap break-all">
                        <span className="mr-2 text-slate-600 select-none">{String(i + 1).padStart(3)}</span>
                        {line}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}

// ── History Tab ────────────────────────────────────────────────────────────

function HistoryTab() {
  const [items, setItems] = useState<HistoryItem[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const fetchHistory = useCallback(async () => {
    try {
      const res = await fetch('/api/v1/usenet/history')
      if (res.ok) {
        const data = await res.json() as { records?: HistoryItem[] }
        setItems(data.records ?? [])
        setError(null)
      } else {
        setError(`Failed to fetch history (${res.status})`)
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Network error')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void fetchHistory()
  }, [fetchHistory])

  const retryItem = async (id: string) => {
    await fetch(`/api/v1/usenet/history/${id}/retry`, { method: 'POST' })
    void fetchHistory()
  }

  const clearItem = async (id: string) => {
    await fetch(`/api/v1/usenet/history/${id}`, { method: 'DELETE' })
    setItems((prev) => prev.filter((i) => i.id !== id))
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 size={32} className="animate-spin text-blue-500" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">{error}</div>
    )
  }

  if (items.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20 text-slate-400">
        <Clock size={48} className="mb-4 text-slate-600" />
        <p className="text-lg font-medium">No history</p>
        <p className="mt-1 text-sm text-slate-500">Completed and failed downloads will appear here</p>
      </div>
    )
  }

  return (
    <div className="overflow-x-auto rounded-lg bg-slate-800">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-slate-700 text-left text-xs uppercase text-slate-400">
            <th className="px-4 py-3 font-medium">Name</th>
            <th className="px-4 py-3 font-medium">Size</th>
            <th className="px-4 py-3 font-medium">Status</th>
            <th className="px-4 py-3 font-medium">Completed</th>
            <th className="px-4 py-3 font-medium">Stages</th>
            <th className="px-4 py-3 font-medium">Actions</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-slate-700">
          {items.map((item) => (
            <tr key={item.id} className="hover:bg-slate-700/30 transition-colors">
              <td className="px-4 py-3 font-medium text-white max-w-xs truncate" title={item.name}>{item.name}</td>
              <td className="px-4 py-3 text-slate-300 whitespace-nowrap">{formatSize(item.size)}</td>
              <td className="px-4 py-3">
                <span className={`rounded-full px-2 py-0.5 text-xs font-medium capitalize ${historyBadgeColor(item.status)}`}>
                  {item.status}
                </span>
              </td>
              <td className="px-4 py-3 text-slate-300 whitespace-nowrap">
                {new Date(item.completedAt).toLocaleDateString()}{' '}
                <span className="text-slate-500">
                  {new Date(item.completedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
                </span>
              </td>
              <td className="px-4 py-3">
                <div className="flex items-center gap-3 text-xs">
                  {item.par2Status && (
                    <span className="flex items-center gap-1" title="Par2 verify">
                      {stageIcon(item.par2Status)} Par2
                    </span>
                  )}
                  {item.repairStatus && (
                    <span className="flex items-center gap-1" title="Repair">
                      {stageIcon(item.repairStatus)} Repair
                    </span>
                  )}
                  {item.extractStatus && (
                    <span className="flex items-center gap-1" title="Extract">
                      {stageIcon(item.extractStatus)} Extract
                    </span>
                  )}
                  {!item.par2Status && !item.repairStatus && !item.extractStatus && (
                    <span className="text-slate-500">-</span>
                  )}
                </div>
              </td>
              <td className="px-4 py-3">
                <div className="flex items-center gap-1">
                  {item.status.toLowerCase() === 'failed' && (
                    <button
                      onClick={() => void retryItem(item.id)}
                      className="rounded p-1 text-slate-400 hover:bg-blue-500/20 hover:text-blue-400 transition-colors"
                      title="Retry"
                    >
                      <RotateCcw size={14} />
                    </button>
                  )}
                  <button
                    onClick={() => void clearItem(item.id)}
                    className="rounded p-1 text-slate-400 hover:bg-red-500/20 hover:text-red-400 transition-colors"
                    title="Clear from history"
                  >
                    <Trash2 size={14} />
                  </button>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

// ── Servers Tab ────────────────────────────────────────────────────────────

const emptyServer: Omit<NntpServer, 'id'> = {
  name: '',
  host: '',
  port: 563,
  ssl: true,
  username: '',
  password: '',
  connections: 10,
  priority: 0,
  optional: false,
  enabled: true,
}

function ServersTab() {
  const [servers, setServers] = useState<NntpServer[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [showForm, setShowForm] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [formData, setFormData] = useState<Omit<NntpServer, 'id'>>(emptyServer)
  const [submitting, setSubmitting] = useState(false)
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null)
  const [testingId, setTestingId] = useState<string | null>(null)
  const [inlineTestResult, setInlineTestResult] = useState<Record<string, { ok: boolean; message: string }>>({})
  const [deleteConfirm, setDeleteConfirm] = useState<string | null>(null)

  const fetchServers = useCallback(async () => {
    try {
      const res = await fetch('/api/v1/usenet/servers')
      if (res.ok) {
        const data = await res.json() as { servers?: NntpServer[] }
        setServers(data.servers ?? [])
        setError(null)
      } else {
        setError(`Failed to fetch servers (${res.status})`)
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Network error')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void fetchServers()
  }, [fetchServers])

  const openAdd = () => {
    setEditingId(null)
    setFormData({ ...emptyServer })
    setTestResult(null)
    setShowForm(true)
  }

  const openEdit = (server: NntpServer) => {
    setEditingId(server.id)
    setFormData({
      name: server.name,
      host: server.host,
      port: server.port,
      ssl: server.ssl,
      username: server.username,
      password: server.password,
      connections: server.connections,
      priority: server.priority,
      optional: server.optional,
      enabled: server.enabled,
    })
    setTestResult(null)
    setShowForm(true)
  }

  const closeForm = () => {
    setShowForm(false)
    setEditingId(null)
    setTestResult(null)
  }

  const handleSave = async () => {
    setSubmitting(true)
    try {
      const url = editingId ? `/api/v1/usenet/servers/${editingId}` : '/api/v1/usenet/servers'
      const method = editingId ? 'PUT' : 'POST'
      const res = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(formData),
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      closeForm()
      void fetchServers()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save server')
    } finally {
      setSubmitting(false)
    }
  }

  const handleTest = async () => {
    setTestResult(null)
    try {
      const url = editingId
        ? `/api/v1/usenet/servers/${editingId}/test`
        : '/api/v1/usenet/servers/test'
      const res = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(formData),
      })
      if (res.ok) {
        const data = await res.json() as { success?: boolean; message?: string }
        setTestResult({ ok: data.success !== false, message: data.message ?? 'Connection successful' })
      } else {
        const body = await res.text()
        setTestResult({ ok: false, message: body || `Test failed (${res.status})` })
      }
    } catch (e) {
      setTestResult({ ok: false, message: e instanceof Error ? e.message : 'Test failed' })
    }
  }

  const handleInlineTest = async (server: NntpServer) => {
    setTestingId(server.id)
    setInlineTestResult(prev => {
      const next = { ...prev }
      delete next[server.id]
      return next
    })
    try {
      const res = await fetch(`/api/v1/usenet/servers/${server.id}/test`, { method: 'POST' })
      if (res.ok) {
        const data = await res.json() as { success?: boolean; message?: string }
        setInlineTestResult(prev => ({
          ...prev,
          [server.id]: { ok: data.success !== false, message: data.message ?? 'OK' },
        }))
      } else {
        setInlineTestResult(prev => ({
          ...prev,
          [server.id]: { ok: false, message: `Failed (${res.status})` },
        }))
      }
    } catch (e) {
      setInlineTestResult(prev => ({
        ...prev,
        [server.id]: { ok: false, message: e instanceof Error ? e.message : 'Failed' },
      }))
    } finally {
      setTestingId(null)
    }
  }

  const toggleEnabled = async (server: NntpServer) => {
    await fetch(`/api/v1/usenet/servers/${server.id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ...server, enabled: !server.enabled }),
    })
    void fetchServers()
  }

  const deleteServer = async (id: string) => {
    await fetch(`/api/v1/usenet/servers/${id}`, { method: 'DELETE' })
    setDeleteConfirm(null)
    void fetchServers()
  }

  const updateField = <K extends keyof Omit<NntpServer, 'id'>>(key: K, value: Omit<NntpServer, 'id'>[K]) => {
    setFormData((prev) => ({ ...prev, [key]: value }))
  }

  if (loading) {
    return (
      <div className="flex items-center justify-center py-20">
        <Loader2 size={32} className="animate-spin text-blue-500" />
      </div>
    )
  }

  return (
    <div>
      {error && (
        <div className="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-red-400">{error}</div>
      )}

      <div className="mb-4">
        <button
          onClick={openAdd}
          className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm font-medium text-white hover:bg-blue-500 transition-colors"
        >
          <Plus size={16} /> Add Server
        </button>
      </div>

      {servers.length === 0 && !showForm && (
        <div className="flex flex-col items-center justify-center py-20 text-slate-400">
          <HardDrive size={48} className="mb-4 text-slate-600" />
          <p className="text-lg font-medium">No NNTP servers configured</p>
          <p className="mt-1 text-sm text-slate-500">Add a server to start downloading</p>
        </div>
      )}

      {/* Server cards */}
      {servers.length > 0 && (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {servers.map((server) => (
            <div
              key={server.id}
              className={`rounded-lg border p-4 transition-colors ${
                server.enabled
                  ? 'border-slate-700 bg-slate-800'
                  : 'border-slate-700/50 bg-slate-800/50 opacity-60'
              }`}
            >
              {/* Card header */}
              <div className="mb-3 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className={`h-2 w-2 rounded-full ${
                    !server.enabled ? 'bg-slate-500' :
                    server.status === 'connected' ? 'bg-green-400' :
                    server.status === 'error' ? 'bg-red-400' :
                    'bg-green-400'
                  }`} />
                  <h4 className="font-medium text-white">{server.name || 'Unnamed Server'}</h4>
                </div>
                {server.ssl && (
                  <span className="flex items-center gap-1 rounded bg-green-500/10 px-1.5 py-0.5 text-xs text-green-400">
                    <Shield size={10} /> SSL
                  </span>
                )}
              </div>

              {/* Card body */}
              <div className="mb-3 space-y-1.5 text-sm">
                <div className="flex items-center justify-between">
                  <span className="text-slate-400">Host</span>
                  <span className="text-slate-200">{server.host}:{server.port}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-slate-400">Connections</span>
                  <span className="text-slate-200">{server.connections}</span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-slate-400">Priority</span>
                  <span className="text-slate-200">{server.priority}</span>
                </div>
                {server.optional && (
                  <div className="flex items-center justify-between">
                    <span className="text-slate-400">Type</span>
                    <span className="text-xs text-yellow-400">Fill server</span>
                  </div>
                )}
              </div>

              {/* Inline test result */}
              {inlineTestResult[server.id] && (
                <div className={`mb-3 rounded px-2 py-1 text-xs ${
                  inlineTestResult[server.id].ok
                    ? 'bg-green-500/10 text-green-400'
                    : 'bg-red-500/10 text-red-400'
                }`}>
                  {inlineTestResult[server.id].ok ? <Check size={10} className="inline mr-1" /> : <AlertTriangle size={10} className="inline mr-1" />}
                  {inlineTestResult[server.id].message}
                </div>
              )}

              {/* Card actions */}
              <div className="flex items-center justify-between border-t border-slate-700 pt-3">
                <div className="flex items-center gap-1">
                  <button
                    onClick={() => void handleInlineTest(server)}
                    disabled={testingId === server.id}
                    className="flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-400 hover:bg-slate-700 hover:text-white transition-colors disabled:opacity-50"
                    title="Test connection"
                  >
                    {testingId === server.id ? (
                      <Loader2 size={12} className="animate-spin" />
                    ) : (
                      <RefreshCw size={12} />
                    )}
                    Test
                  </button>
                  <button
                    onClick={() => openEdit(server)}
                    className="flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
                    title="Edit"
                  >
                    <Pencil size={12} /> Edit
                  </button>
                  <button
                    onClick={() => setDeleteConfirm(server.id)}
                    className="flex items-center gap-1 rounded px-2 py-1 text-xs text-slate-400 hover:bg-red-500/20 hover:text-red-400 transition-colors"
                    title="Delete"
                  >
                    <Trash2 size={12} /> Delete
                  </button>
                </div>
                <button
                  onClick={() => void toggleEnabled(server)}
                  className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                    server.enabled ? 'bg-blue-600' : 'bg-slate-600'
                  }`}
                  title={server.enabled ? 'Disable' : 'Enable'}
                >
                  <span
                    className={`inline-block h-3.5 w-3.5 rounded-full bg-white transition-transform ${
                      server.enabled ? 'translate-x-4.5' : 'translate-x-0.5'
                    }`}
                  />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Server Form Modal */}
      {showForm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={closeForm}>
          <div className="w-full max-w-lg rounded-lg bg-slate-800 p-6" onClick={(e) => e.stopPropagation()}>
            <div className="mb-4 flex items-center justify-between">
              <h3 className="text-lg font-semibold text-white">{editingId ? 'Edit Server' : 'Add Server'}</h3>
              <button onClick={closeForm} className="text-slate-400 hover:text-white transition-colors">
                <X size={20} />
              </button>
            </div>

            <div className="space-y-3">
              <FormField label="Name">
                <input
                  type="text"
                  value={formData.name}
                  onChange={(e) => updateField('name', e.target.value)}
                  placeholder="e.g. Eweka, Newshosting"
                  className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
                />
              </FormField>

              <div className="grid grid-cols-3 gap-3">
                <div className="col-span-2">
                  <FormField label="Host">
                    <input
                      type="text"
                      value={formData.host}
                      onChange={(e) => updateField('host', e.target.value)}
                      placeholder="news.example.com"
                      className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
                    />
                  </FormField>
                </div>
                <FormField label="Port">
                  <input
                    type="number"
                    value={formData.port}
                    onChange={(e) => updateField('port', Number(e.target.value))}
                    className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
                  />
                </FormField>
              </div>

              <label className="flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={formData.ssl}
                  onChange={(e) => {
                    updateField('ssl', e.target.checked)
                    // Auto-adjust port
                    if (e.target.checked && formData.port === 119) {
                      updateField('port', 563)
                    } else if (!e.target.checked && formData.port === 563) {
                      updateField('port', 119)
                    }
                  }}
                  className="rounded border-slate-600 bg-slate-700"
                />
                Use SSL
              </label>

              <div className="grid grid-cols-2 gap-3">
                <FormField label="Username">
                  <input
                    type="text"
                    value={formData.username}
                    onChange={(e) => updateField('username', e.target.value)}
                    className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
                  />
                </FormField>
                <FormField label="Password">
                  <input
                    type="password"
                    value={formData.password}
                    onChange={(e) => updateField('password', e.target.value)}
                    className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
                  />
                </FormField>
              </div>

              <div className="grid grid-cols-2 gap-3">
                <FormField label="Connections (1-50)">
                  <input
                    type="number"
                    value={formData.connections}
                    onChange={(e) => updateField('connections', Math.min(50, Math.max(1, Number(e.target.value))))}
                    min={1}
                    max={50}
                    className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
                  />
                </FormField>
                <FormField label="Priority">
                  <input
                    type="number"
                    value={formData.priority}
                    onChange={(e) => updateField('priority', Number(e.target.value))}
                    min={0}
                    className="w-full rounded-lg bg-slate-900 px-3 py-2 text-sm text-white outline-none ring-1 ring-slate-700 focus:ring-blue-500 transition-colors"
                  />
                </FormField>
              </div>

              <div className="flex items-center gap-6">
                <label className="flex items-center gap-2 text-sm text-slate-300">
                  <input
                    type="checkbox"
                    checked={formData.optional}
                    onChange={(e) => updateField('optional', e.target.checked)}
                    className="rounded border-slate-600 bg-slate-700"
                  />
                  Optional (fill server)
                </label>
                <label className="flex items-center gap-2 text-sm text-slate-300">
                  <input
                    type="checkbox"
                    checked={formData.enabled}
                    onChange={(e) => updateField('enabled', e.target.checked)}
                    className="rounded border-slate-600 bg-slate-700"
                  />
                  Enabled
                </label>
              </div>

              {testResult && (
                <div className={`rounded-lg border p-3 text-sm ${
                  testResult.ok
                    ? 'border-green-500/30 bg-green-500/10 text-green-400'
                    : 'border-red-500/30 bg-red-500/10 text-red-400'
                }`}>
                  {testResult.ok ? <Check size={14} className="inline mr-1" /> : <AlertTriangle size={14} className="inline mr-1" />}
                  {testResult.message}
                </div>
              )}
            </div>

            <div className="mt-6 flex justify-between">
              <button
                onClick={() => void handleTest()}
                className="flex items-center gap-1.5 rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
              >
                <RefreshCw size={14} /> Test Connection
              </button>
              <div className="flex gap-2">
                <button
                  onClick={closeForm}
                  className="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={() => void handleSave()}
                  disabled={submitting || !formData.name || !formData.host}
                  className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
                >
                  {submitting && <Loader2 size={14} className="animate-spin" />}
                  Save
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Delete Confirm Dialog */}
      {deleteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={() => setDeleteConfirm(null)}>
          <div className="w-full max-w-md rounded-lg bg-slate-800 p-6" onClick={(e) => e.stopPropagation()}>
            <div className="mb-2 flex items-center gap-2">
              <AlertTriangle size={20} className="text-red-400" />
              <h3 className="text-lg font-semibold text-white">Delete Server</h3>
            </div>
            <p className="mb-4 text-sm text-slate-300">
              Are you sure you want to delete <strong className="text-white">{servers.find(s => s.id === deleteConfirm)?.name ?? 'this server'}</strong>? This action cannot be undone.
            </p>
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setDeleteConfirm(null)}
                className="rounded-lg bg-slate-700 px-4 py-2 text-sm font-medium text-slate-300 hover:bg-slate-600 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => void deleteServer(deleteConfirm)}
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

// ── Shared sub-components ──────────────────────────────────────────────────

function FormField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <label className="mb-1 block text-sm font-medium text-slate-300">{label}</label>
      {children}
    </div>
  )
}
