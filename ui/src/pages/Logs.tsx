import { useState, useEffect, useRef, useCallback } from 'react'
import { ScrollText, Pause, Play, ArrowDown, Loader2, AlertCircle } from 'lucide-react'
import { apiFetch } from '../api/client'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface LogEntry {
  timestamp: string
  level: string
  target: string
  message: string
  seq: number
}

interface LogResponse {
  entries: LogEntry[]
  latestSeq: number
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const LEVEL_STYLES: Record<string, { badge: string; text: string }> = {
  ERROR: { badge: 'bg-red-500/20 text-red-400', text: 'text-red-300' },
  WARN: { badge: 'bg-yellow-500/20 text-yellow-400', text: 'text-yellow-200' },
  INFO: { badge: 'bg-blue-500/20 text-blue-400', text: 'text-slate-300' },
  DEBUG: { badge: 'bg-slate-500/20 text-slate-400', text: 'text-slate-400' },
  TRACE: { badge: 'bg-slate-600/20 text-slate-500', text: 'text-slate-500' },
}

function levelStyle(level: string) {
  return LEVEL_STYLES[level.toUpperCase()] ?? LEVEL_STYLES.INFO
}

function formatTimestamp(ts: string) {
  try {
    const d = new Date(ts)
    return d.toLocaleTimeString('en-GB', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })
      + '.' + String(d.getMilliseconds()).padStart(3, '0')
  } catch {
    return ts
  }
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

const MAX_ENTRIES = 5000
const POLL_INTERVAL = 3000

export default function Logs() {
  const [entries, setEntries] = useState<LogEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [paused, setPaused] = useState(false)
  const [level, setLevel] = useState('')
  const [targetFilter, setTargetFilter] = useState('')
  const [searchText, setSearchText] = useState('')
  const [atBottom, setAtBottom] = useState(true)

  const latestSeqRef = useRef<number | undefined>(undefined)
  const scrollRef = useRef<HTMLDivElement>(null)
  const pollTimerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  const fetchLogs = useCallback(async (incremental: boolean) => {
    try {
      const params = new URLSearchParams()
      if (incremental && latestSeqRef.current !== undefined) {
        params.set('afterSeq', String(latestSeqRef.current))
      }
      if (!incremental) {
        params.set('limit', '500')
      }
      const data = await apiFetch<LogResponse>(`/log?${params}`)

      if (incremental && data.entries.length > 0) {
        setEntries(prev => {
          const merged = [...prev, ...data.entries]
          return merged.length > MAX_ENTRIES ? merged.slice(-MAX_ENTRIES) : merged
        })
      } else if (!incremental) {
        setEntries(data.entries)
      }

      latestSeqRef.current = data.latestSeq
    } catch {
      // Silently handle fetch errors — will retry on next poll
    } finally {
      setLoading(false)
    }
  }, [])

  // Initial fetch
  useEffect(() => {
    setLoading(true)
    latestSeqRef.current = undefined
    setEntries([])
    fetchLogs(false)
  }, [fetchLogs])

  // Polling
  useEffect(() => {
    if (paused) {
      if (pollTimerRef.current) clearInterval(pollTimerRef.current)
      pollTimerRef.current = null
      return
    }
    pollTimerRef.current = setInterval(() => fetchLogs(true), POLL_INTERVAL)
    return () => {
      if (pollTimerRef.current) clearInterval(pollTimerRef.current)
    }
  }, [paused, fetchLogs])

  // Auto-scroll
  useEffect(() => {
    if (atBottom && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [entries, atBottom])

  const handleScroll = () => {
    if (!scrollRef.current) return
    const { scrollTop, scrollHeight, clientHeight } = scrollRef.current
    setAtBottom(scrollHeight - scrollTop - clientHeight < 50)
  }

  const scrollToBottom = () => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
      setAtBottom(true)
    }
  }

  // Client-side filtering
  const filtered = entries.filter(e => {
    if (level && e.level.toUpperCase() !== level) return false
    if (targetFilter && !e.target.toLowerCase().includes(targetFilter.toLowerCase())) return false
    if (searchText && !e.message.toLowerCase().includes(searchText.toLowerCase())) return false
    return true
  })

  return (
    <div className="flex h-[calc(100vh-3.5rem)] flex-col">
      {/* Header */}
      <div className="mb-4 flex flex-wrap items-center gap-3">
        <div className="flex items-center gap-3">
          <ScrollText className="h-6 w-6 text-blue-400" />
          <h1 className="text-2xl font-bold text-white">Logs</h1>
          <span className="rounded-full bg-slate-700 px-2.5 py-0.5 text-xs font-medium text-slate-300">
            {filtered.length}
          </span>
        </div>

        <div className="ml-auto flex flex-wrap items-center gap-2">
          {/* Level filter */}
          <select
            value={level}
            onChange={e => setLevel(e.target.value)}
            className="rounded-lg bg-slate-700 px-3 py-1.5 text-sm text-slate-300 outline-none focus:ring-1 focus:ring-blue-500"
          >
            <option value="">All Levels</option>
            <option value="ERROR">Error</option>
            <option value="WARN">Warn</option>
            <option value="INFO">Info</option>
            <option value="DEBUG">Debug</option>
            <option value="TRACE">Trace</option>
          </select>

          {/* Target filter */}
          <input
            type="text"
            value={targetFilter}
            onChange={e => setTargetFilter(e.target.value)}
            placeholder="Filter target..."
            className="w-40 rounded-lg bg-slate-700 px-3 py-1.5 text-sm text-slate-300 placeholder-slate-500 outline-none focus:ring-1 focus:ring-blue-500"
          />

          {/* Search */}
          <input
            type="text"
            value={searchText}
            onChange={e => setSearchText(e.target.value)}
            placeholder="Search messages..."
            className="w-48 rounded-lg bg-slate-700 px-3 py-1.5 text-sm text-slate-300 placeholder-slate-500 outline-none focus:ring-1 focus:ring-blue-500"
          />

          {/* Pause / Resume */}
          <button
            onClick={() => setPaused(p => !p)}
            className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition-colors ${
              paused
                ? 'bg-green-600 text-white hover:bg-green-700'
                : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
            }`}
          >
            {paused ? <Play className="h-3.5 w-3.5" /> : <Pause className="h-3.5 w-3.5" />}
            {paused ? 'Resume' : 'Pause'}
          </button>
        </div>
      </div>

      {/* Log entries */}
      {loading ? (
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="h-8 w-8 animate-spin text-blue-400" />
        </div>
      ) : filtered.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 text-slate-500">
          <AlertCircle className="h-12 w-12" />
          <p className="text-sm">No log entries{level || targetFilter || searchText ? ' match your filters' : ''}</p>
        </div>
      ) : (
        <div className="relative flex-1 overflow-hidden rounded-lg bg-slate-800">
          <div
            ref={scrollRef}
            onScroll={handleScroll}
            className="h-full overflow-y-auto p-2 font-mono text-xs leading-relaxed"
          >
            {filtered.map(entry => {
              const style = levelStyle(entry.level)
              return (
                <div key={entry.seq} className="flex gap-2 px-2 py-0.5 hover:bg-slate-700/50">
                  <span className="shrink-0 text-slate-500">{formatTimestamp(entry.timestamp)}</span>
                  <span className={`shrink-0 w-12 rounded px-1 text-center font-semibold ${style.badge}`}>
                    {entry.level}
                  </span>
                  <span className="shrink-0 max-w-48 truncate text-slate-500" title={entry.target}>
                    {entry.target}
                  </span>
                  <span className={style.text}>{entry.message}</span>
                </div>
              )
            })}
          </div>

          {/* Scroll to bottom FAB */}
          {!atBottom && (
            <button
              onClick={scrollToBottom}
              className="absolute bottom-4 right-4 flex items-center gap-1.5 rounded-full bg-blue-600 px-3 py-1.5 text-xs font-medium text-white shadow-lg hover:bg-blue-700 transition-colors"
            >
              <ArrowDown className="h-3.5 w-3.5" />
              Latest
            </button>
          )}
        </div>
      )}
    </div>
  )
}
