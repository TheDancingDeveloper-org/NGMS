import { useEffect, useMemo, useState } from 'react'
import {
  Check,
  X,
  RefreshCw,
  Film,
  Tv,
  Loader2,
  AlertCircle,
  FolderInput,
} from 'lucide-react'

const API = '/api/v1'

// ---------------------------------------------------------------------------
// Types (match stackarr-core::models::ImportCandidate)
// ---------------------------------------------------------------------------

interface ImportCandidate {
  id: number
  mediaLibraryFolderId: number | null
  mediaType: string
  matchKind: string
  discoveredPath: string
  fileCount: number
  totalSize: number
  parsedTitle: string | null
  parsedYear: number | null
  parsedSeason: number | null
  parsedEpisodes: number[] | null
  suggestedTmdbId: number | null
  suggestedTitle: string | null
  suggestedYear: number | null
  suggestedPoster: string | null
  suggestedOverview: string | null
  confidence: number
  status: string
  targetSeriesId: number | null
  targetMovieId: number | null
  error: string | null
  data: unknown
  discoveredAt: string
  resolvedAt: string | null
}

interface ListResponse {
  items: ImportCandidate[]
  count: number
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatBytes(bytes: number): string {
  if (bytes <= 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`
}

function confidenceColor(c: number): string {
  if (c >= 0.85) return 'text-emerald-400'
  if (c >= 0.65) return 'text-amber-400'
  return 'text-slate-400'
}

function confidenceLabel(c: number): string {
  return `${Math.round(c * 100)}%`
}

// ---------------------------------------------------------------------------
// Main page
// ---------------------------------------------------------------------------

type Filter = 'all' | 'series' | 'movie'

export default function Import() {
  const [candidates, setCandidates] = useState<ImportCandidate[]>([])
  const [loading, setLoading] = useState(true)
  const [filter, setFilter] = useState<Filter>('all')
  const [error, setError] = useState<string | null>(null)
  const [busyIds, setBusyIds] = useState<Set<number>>(new Set())
  const [toast, setToast] = useState<{ msg: string; kind: 'ok' | 'err' } | null>(null)

  const load = async (f: Filter = filter) => {
    setLoading(true)
    setError(null)
    try {
      const q = f === 'all' ? '' : `?mediaType=${f}`
      const res = await fetch(`${API}/import-candidates${q}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data: ListResponse = await res.json()
      setCandidates(data.items)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => {
    void load(filter)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filter])

  useEffect(() => {
    if (!toast) return
    const t = setTimeout(() => setToast(null), 3000)
    return () => clearTimeout(t)
  }, [toast])

  const setBusy = (id: number, busy: boolean) => {
    setBusyIds((prev) => {
      const next = new Set(prev)
      if (busy) next.add(id)
      else next.delete(id)
      return next
    })
  }

  const accept = async (c: ImportCandidate) => {
    setBusy(c.id, true)
    try {
      const res = await fetch(`${API}/import-candidates/${c.id}/accept`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          tmdbId: c.suggestedTmdbId,
          monitored: true,
        }),
      })
      if (!res.ok) {
        const body = await res.json().catch(() => ({}))
        throw new Error(body.error ?? `HTTP ${res.status}`)
      }
      setToast({ msg: `Accepted: ${c.suggestedTitle ?? c.parsedTitle ?? 'candidate'}`, kind: 'ok' })
      setCandidates((prev) => prev.filter((x) => x.id !== c.id))
    } catch (e) {
      setToast({
        msg: `Failed: ${e instanceof Error ? e.message : String(e)}`,
        kind: 'err',
      })
    } finally {
      setBusy(c.id, false)
    }
  }

  const reject = async (c: ImportCandidate) => {
    setBusy(c.id, true)
    try {
      const res = await fetch(`${API}/import-candidates/${c.id}/reject`, {
        method: 'POST',
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      setCandidates((prev) => prev.filter((x) => x.id !== c.id))
    } catch (e) {
      setToast({
        msg: `Reject failed: ${e instanceof Error ? e.message : String(e)}`,
        kind: 'err',
      })
    } finally {
      setBusy(c.id, false)
    }
  }

  const triggerScan = async () => {
    try {
      const res = await fetch(`${API}/system/command`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: 'RescanMediaLibrary' }),
      })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      setToast({ msg: 'Library scan triggered — results will appear here shortly', kind: 'ok' })
    } catch (e) {
      setToast({
        msg: `Scan failed: ${e instanceof Error ? e.message : String(e)}`,
        kind: 'err',
      })
    }
  }

  const visible = useMemo(() => {
    if (filter === 'all') return candidates
    return candidates.filter((c) => c.mediaType === filter)
  }, [candidates, filter])

  return (
    <div className="space-y-4 p-6">
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <FolderInput className="h-6 w-6 text-slate-300" />
          <div>
            <h1 className="text-xl font-semibold text-white">Import Recommendations</h1>
            <p className="text-sm text-slate-400">
              Files your library scanner found on disk that don't match an existing series or
              movie. Review each suggestion and accept to add it, or reject to ignore.
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => load()}
            className="inline-flex items-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-sm text-slate-200 hover:bg-slate-600"
          >
            <RefreshCw className="h-4 w-4" /> Refresh
          </button>
          <button
            onClick={triggerScan}
            className="inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-2 text-sm text-white hover:bg-blue-500"
          >
            <FolderInput className="h-4 w-4" /> Scan library now
          </button>
        </div>
      </header>

      <div className="flex items-center gap-2">
        <FilterChip active={filter === 'all'} onClick={() => setFilter('all')}>
          All ({candidates.length})
        </FilterChip>
        <FilterChip active={filter === 'series'} onClick={() => setFilter('series')}>
          <Tv className="h-3.5 w-3.5" /> Series
        </FilterChip>
        <FilterChip active={filter === 'movie'} onClick={() => setFilter('movie')}>
          <Film className="h-3.5 w-3.5" /> Movies
        </FilterChip>
      </div>

      {error && (
        <div className="rounded-lg border border-red-800 bg-red-950/40 p-3 text-sm text-red-200">
          <AlertCircle className="mr-2 inline h-4 w-4" /> {error}
        </div>
      )}

      {loading ? (
        <div className="flex items-center gap-2 p-6 text-slate-400">
          <Loader2 className="h-4 w-4 animate-spin" /> Loading candidates…
        </div>
      ) : visible.length === 0 ? (
        <EmptyState onScan={triggerScan} />
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {visible.map((c) => (
            <CandidateCard
              key={c.id}
              candidate={c}
              busy={busyIds.has(c.id)}
              onAccept={() => accept(c)}
              onReject={() => reject(c)}
            />
          ))}
        </div>
      )}

      {toast && (
        <div
          className={`fixed bottom-6 right-6 z-50 rounded-lg px-4 py-3 text-sm text-white shadow-lg ${
            toast.kind === 'ok' ? 'bg-emerald-600' : 'bg-red-600'
          }`}
        >
          {toast.msg}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function FilterChip({
  children,
  active,
  onClick,
}: {
  children: React.ReactNode
  active: boolean
  onClick: () => void
}) {
  return (
    <button
      onClick={onClick}
      className={`inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-sm transition-colors ${
        active
          ? 'bg-blue-600 text-white'
          : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
      }`}
    >
      {children}
    </button>
  )
}

function EmptyState({ onScan }: { onScan: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-slate-700 p-12 text-center">
      <FolderInput className="mb-3 h-10 w-10 text-slate-500" />
      <h3 className="text-lg font-medium text-slate-200">Nothing to review</h3>
      <p className="mt-1 max-w-md text-sm text-slate-400">
        Either your library is fully matched, or the scanner hasn't run yet. Trigger a scan
        to pick up any new files on disk.
      </p>
      <button
        onClick={onScan}
        className="mt-4 inline-flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-500"
      >
        <FolderInput className="h-4 w-4" /> Scan library now
      </button>
    </div>
  )
}

function CandidateCard({
  candidate,
  busy,
  onAccept,
  onReject,
}: {
  candidate: ImportCandidate
  busy: boolean
  onAccept: () => void
  onReject: () => void
}) {
  const c = candidate
  const poster = c.suggestedPoster
    ? `https://image.tmdb.org/t/p/w154${c.suggestedPoster}`
    : null
  const hasSuggestion = c.suggestedTmdbId != null
  const kindLabel =
    c.matchKind === 'season'
      ? `Season ${c.parsedSeason ?? ''}`
      : c.matchKind === 'series'
        ? 'Full series'
        : c.matchKind === 'episode'
          ? 'Episode'
          : 'Movie'

  return (
    <div className="flex flex-col rounded-lg border border-slate-700 bg-slate-800 p-3 shadow-sm">
      <div className="flex gap-3">
        {poster ? (
          <img
            src={poster}
            alt=""
            className="h-28 w-[75px] shrink-0 rounded bg-slate-900 object-cover"
          />
        ) : (
          <div className="flex h-28 w-[75px] shrink-0 items-center justify-center rounded bg-slate-900 text-slate-600">
            {c.mediaType === 'movie' ? (
              <Film className="h-6 w-6" />
            ) : (
              <Tv className="h-6 w-6" />
            )}
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-2">
            <h3 className="truncate text-sm font-semibold text-white">
              {c.suggestedTitle ?? c.parsedTitle ?? 'Unknown'}
              {c.suggestedYear && (
                <span className="ml-1 text-slate-400">({c.suggestedYear})</span>
              )}
            </h3>
            <span
              className={`shrink-0 text-xs font-medium ${confidenceColor(c.confidence)}`}
              title="match confidence"
            >
              {confidenceLabel(c.confidence)}
            </span>
          </div>
          <div className="mt-1 flex items-center gap-2 text-xs text-slate-400">
            <span className="rounded bg-slate-700 px-1.5 py-0.5">{kindLabel}</span>
            <span>{c.fileCount} file{c.fileCount === 1 ? '' : 's'}</span>
            <span>·</span>
            <span>{formatBytes(c.totalSize)}</span>
          </div>
          {c.suggestedOverview && (
            <p className="mt-2 line-clamp-3 text-xs text-slate-400">{c.suggestedOverview}</p>
          )}
        </div>
      </div>

      <div className="mt-3 min-h-[20px] truncate text-xs text-slate-500" title={c.discoveredPath}>
        {c.discoveredPath}
      </div>

      <div className="mt-3 flex items-center gap-2">
        <button
          disabled={!hasSuggestion || busy}
          onClick={onAccept}
          className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg bg-emerald-600 px-3 py-2 text-xs font-medium text-white hover:bg-emerald-500 disabled:cursor-not-allowed disabled:bg-slate-700 disabled:text-slate-500"
          title={hasSuggestion ? 'Create the series/movie and link the files' : 'No TMDB suggestion yet'}
        >
          {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
          Accept
        </button>
        <button
          disabled={busy}
          onClick={onReject}
          className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg bg-slate-700 px-3 py-2 text-xs font-medium text-slate-200 hover:bg-slate-600"
        >
          <X className="h-3.5 w-3.5" /> Reject
        </button>
      </div>
      {!hasSuggestion && (
        <p className="mt-2 text-[11px] text-amber-400">
          No TMDB suggestion — match pass hasn't run yet or no confident match was found.
        </p>
      )}
    </div>
  )
}
