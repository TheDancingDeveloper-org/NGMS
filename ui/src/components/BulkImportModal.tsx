import { useState, useEffect, useMemo } from 'react'
import {
  X,
  Loader2,
  AlertCircle,
  CheckCircle,
  ChevronDown,
  Tv,
  Film,
  Info,
} from 'lucide-react'
import { apiFetch } from '../api/client'
import type { Episode } from '../api/types'

// ---------------------------------------------------------------------------
// Types (mirror crates/stackarr-web/src/routes/manual_import.rs)
// ---------------------------------------------------------------------------

interface LibraryMatch {
  id: number
  title: string
  year: number | null
  posterUrl: string | null
}

interface AutoMatch {
  mediaType: 'series' | 'movie'
  mediaId: number
  mediaTitle: string
  mediaYear: number | null
  episodeId: number | null
  episodeLabel: string | null
  confidence: 'high' | 'medium' | 'low'
}

interface AnalyzeResponse {
  path: string
  folderName: string
  files: { name: string; path: string; size: number }[]
  parsedTitle: string | null
  parsedYear: number | null
  parsedSeason: number | null
  parsedEpisodes: number[]
  parsedQuality: string
  suggestedMediaType: 'series' | 'movie'
  seriesMatches: LibraryMatch[]
  movieMatches: LibraryMatch[]
  autoMatch: AutoMatch | null
}

interface BulkAnalyzeResponse {
  results: AnalyzeResponse[]
}

interface BulkImportItemResult {
  path: string
  ok: boolean
  imported: number
  skipped: number
  errors: string[]
  error: string | null
}

interface BulkImportResponse {
  results: BulkImportItemResult[]
  totalImported: number
  totalFailed: number
}

type RowStatus = 'pending' | 'ok' | 'failed'

interface Row {
  path: string
  analysis: AnalyzeResponse
  included: boolean
  mediaType: 'series' | 'movie'
  targetId: number // series or movie id; 0 = none selected
  episodeId: number // 0 = auto-detect
  status: RowStatus
  resultMsg: string | null
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function confidenceClasses(c: AutoMatch['confidence'] | undefined): string {
  switch (c) {
    case 'high':
      return 'bg-emerald-500/20 text-emerald-300'
    case 'medium':
      return 'bg-amber-500/20 text-amber-300'
    case 'low':
      return 'bg-slate-500/20 text-slate-300'
    default:
      return 'bg-slate-700 text-slate-400'
  }
}

function shortName(p: string): string {
  const parts = p.split('/').filter(Boolean)
  return parts[parts.length - 1] ?? p
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function BulkImportModal({
  paths,
  onClose,
  onImported,
}: {
  paths: string[]
  onClose: () => void
  onImported: (importedCount: number) => void
}) {
  const [rows, setRows] = useState<Row[]>([])
  const [analyzing, setAnalyzing] = useState(true)
  const [analyzeError, setAnalyzeError] = useState<string | null>(null)

  const [importing, setImporting] = useState(false)
  const [importSummary, setImportSummary] = useState<BulkImportResponse | null>(null)

  // Episodes cache keyed by seriesId → Episode[]
  const [episodesBySeries, setEpisodesBySeries] = useState<Record<number, Episode[]>>({})
  const [loadingSeries, setLoadingSeries] = useState<Set<number>>(new Set())

  // ── Initial bulk analyze ────────────────────────────────────────────────
  useEffect(() => {
    if (paths.length === 0) {
      setAnalyzing(false)
      return
    }
    setAnalyzing(true)
    setAnalyzeError(null)
    apiFetch<BulkAnalyzeResponse>('/manual-import/bulk-analyze', {
      method: 'POST',
      body: JSON.stringify({ paths }),
    })
      .then((data) => {
        const initial: Row[] = data.results.map((a) => {
          const auto = a.autoMatch
          const mediaType: 'series' | 'movie' = auto
            ? auto.mediaType
            : a.suggestedMediaType
          const targetId = auto?.mediaId ?? 0
          const episodeId = auto?.episodeId ?? 0
          // Pre-check when we have a target AND confidence is high.
          const included = !!auto && auto.confidence === 'high'
          return {
            path: a.path,
            analysis: a,
            included,
            mediaType,
            targetId,
            episodeId,
            status: 'pending',
            resultMsg: null,
          }
        })
        setRows(initial)
      })
      .catch((e) => setAnalyzeError(e instanceof Error ? e.message : String(e)))
      .finally(() => setAnalyzing(false))
  }, [paths])

  // ── Episode loader for any series that is selected but not cached ───────
  useEffect(() => {
    const wanted = new Set<number>()
    rows.forEach((r) => {
      if (r.mediaType === 'series' && r.targetId > 0) wanted.add(r.targetId)
    })
    wanted.forEach((seriesId) => {
      if (episodesBySeries[seriesId] !== undefined) return
      if (loadingSeries.has(seriesId)) return
      setLoadingSeries((prev) => new Set(prev).add(seriesId))
      apiFetch<Episode[]>(`/series/${seriesId}/episodes`)
        .then((eps) => setEpisodesBySeries((prev) => ({ ...prev, [seriesId]: eps })))
        .catch(() => setEpisodesBySeries((prev) => ({ ...prev, [seriesId]: [] })))
        .finally(() =>
          setLoadingSeries((prev) => {
            const next = new Set(prev)
            next.delete(seriesId)
            return next
          }),
        )
    })
  }, [rows, episodesBySeries, loadingSeries])

  // ── Row updaters ────────────────────────────────────────────────────────
  const updateRow = (idx: number, patch: Partial<Row>) => {
    setRows((prev) => prev.map((r, i) => (i === idx ? { ...r, ...patch } : r)))
  }

  const toggleAll = (checked: boolean) => {
    setRows((prev) => prev.map((r) => ({ ...r, included: checked })))
  }

  // ── Submit ──────────────────────────────────────────────────────────────
  const checkedCount = useMemo(
    () => rows.filter((r) => r.included && r.targetId > 0).length,
    [rows],
  )
  const untargetedChecked = useMemo(
    () => rows.some((r) => r.included && r.targetId <= 0),
    [rows],
  )

  const handleImport = async () => {
    setImporting(true)
    setImportSummary(null)
    const items = rows
      .filter((r) => r.included && r.targetId > 0)
      .map((r) => ({
        path: r.path,
        mediaType: r.mediaType,
        mediaId: r.targetId,
        episodeId: r.mediaType === 'series' && r.episodeId > 0 ? r.episodeId : null,
      }))
    try {
      const res = await apiFetch<BulkImportResponse>('/manual-import/bulk-import', {
        method: 'POST',
        body: JSON.stringify({ items }),
      })
      setImportSummary(res)
      // Reflect per-row status
      const byPath = new Map(res.results.map((r) => [r.path, r]))
      setRows((prev) =>
        prev.map((r) => {
          const out = byPath.get(r.path)
          if (!out) return r
          if (out.ok) {
            return { ...r, status: 'ok', resultMsg: `${out.imported} file(s) imported` }
          }
          const msg = out.error ?? out.errors.join('; ') ?? 'failed'
          return { ...r, status: 'failed', resultMsg: msg }
        }),
      )
      if (res.totalImported > 0) onImported(res.totalImported)
    } catch (e) {
      setImportSummary({
        results: [],
        totalImported: 0,
        totalFailed: items.length,
      })
      setRows((prev) =>
        prev.map((r) =>
          r.included
            ? {
                ...r,
                status: 'failed',
                resultMsg: e instanceof Error ? e.message : String(e),
              }
            : r,
        ),
      )
    } finally {
      setImporting(false)
    }
  }

  const allChecked = rows.length > 0 && rows.every((r) => r.included)

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
      onClick={onClose}
    >
      <div
        className="flex max-h-[90vh] w-full max-w-5xl flex-col rounded-xl bg-slate-800 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-700 px-5 py-4">
          <div>
            <h2 className="text-base font-semibold text-white">Bulk Import</h2>
            <p className="mt-0.5 text-xs text-slate-400">
              {paths.length} selected · review auto-matches, untick items you don't trust, then import.
            </p>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-5">
          {analyzing && (
            <div className="flex items-center gap-2 py-8 text-sm text-slate-400">
              <Loader2 size={14} className="animate-spin" /> Analyzing {paths.length} item(s)…
            </div>
          )}

          {analyzeError && (
            <div className="flex items-center gap-2 rounded-lg bg-red-950/40 p-3 text-sm text-red-300">
              <AlertCircle size={14} className="shrink-0" /> {analyzeError}
            </div>
          )}

          {!analyzing && !analyzeError && rows.length === 0 && (
            <div className="rounded-lg bg-slate-900 p-8 text-center text-sm text-slate-400">
              Nothing to import.
            </div>
          )}

          {!analyzing && rows.length > 0 && (
            <div className="space-y-2">
              {/* Table header */}
              <div className="flex items-center gap-3 px-3 py-2 text-xs text-slate-500">
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={allChecked}
                    onChange={(e) => toggleAll(e.target.checked)}
                    className="h-4 w-4 rounded border-slate-600 bg-slate-900 accent-blue-500"
                  />
                  <span>{allChecked ? 'Uncheck all' : 'Check all'}</span>
                </label>
                <span className="ml-auto">
                  {checkedCount} of {rows.length} selected
                </span>
              </div>

              {rows.map((row, idx) => (
                <BulkRow
                  key={row.path + idx}
                  row={row}
                  episodes={episodesBySeries[row.targetId] ?? []}
                  episodesLoading={
                    row.mediaType === 'series' &&
                    row.targetId > 0 &&
                    episodesBySeries[row.targetId] === undefined
                  }
                  onChange={(patch) => updateRow(idx, patch)}
                />
              ))}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between gap-3 border-t border-slate-700 px-5 py-4">
          <div className="text-xs text-slate-400">
            {importSummary ? (
              <span className="inline-flex items-center gap-2">
                <CheckCircle size={14} className="text-emerald-400" />
                {importSummary.totalImported} file(s) imported
                {importSummary.totalFailed > 0 &&
                  `, ${importSummary.totalFailed} item(s) failed`}
              </span>
            ) : untargetedChecked ? (
              <span className="inline-flex items-center gap-1 text-amber-300">
                <Info size={12} /> Some checked rows have no target — they'll be skipped.
              </span>
            ) : (
              <span>Only checked rows with a target will be imported.</span>
            )}
          </div>
          <div className="flex gap-2">
            <button
              onClick={onClose}
              className="rounded-lg bg-slate-700 px-4 py-2 text-sm text-slate-300 hover:bg-slate-600 transition-colors"
            >
              {importSummary ? 'Close' : 'Cancel'}
            </button>
            {!importSummary && (
              <button
                onClick={handleImport}
                disabled={analyzing || importing || checkedCount === 0}
                className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
              >
                {importing ? (
                  <Loader2 size={14} className="animate-spin" />
                ) : (
                  <CheckCircle size={14} />
                )}
                Import {checkedCount} item{checkedCount === 1 ? '' : 's'}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Row
// ---------------------------------------------------------------------------

function BulkRow({
  row,
  episodes,
  episodesLoading,
  onChange,
}: {
  row: Row
  episodes: Episode[]
  episodesLoading: boolean
  onChange: (patch: Partial<Row>) => void
}) {
  const a = row.analysis
  const auto = a.autoMatch
  const library = row.mediaType === 'series' ? a.seriesMatches : a.movieMatches

  // Ensure the auto-matched target is present in the options even if it's not
  // in the initial library list (edge case when series/movie list is trimmed).
  const options = useMemo(() => {
    if (
      auto &&
      auto.mediaType === row.mediaType &&
      !library.some((m) => m.id === auto.mediaId)
    ) {
      return [
        ...library,
        {
          id: auto.mediaId,
          title: auto.mediaTitle,
          year: auto.mediaYear,
          posterUrl: null,
        },
      ]
    }
    return library
  }, [auto, row.mediaType, library])

  const seasonEpisodes = useMemo(() => {
    if (a.parsedSeason == null) return episodes
    return episodes.filter((e) => e.seasonNumber === a.parsedSeason)
  }, [episodes, a.parsedSeason])

  const statusBadge = (() => {
    if (row.status === 'ok') {
      return (
        <span className="inline-flex items-center gap-1 rounded bg-emerald-500/20 px-2 py-0.5 text-xs text-emerald-300">
          <CheckCircle size={12} /> Imported
        </span>
      )
    }
    if (row.status === 'failed') {
      return (
        <span
          className="inline-flex items-center gap-1 rounded bg-red-500/20 px-2 py-0.5 text-xs text-red-300"
          title={row.resultMsg ?? ''}
        >
          <AlertCircle size={12} /> Failed
        </span>
      )
    }
    return null
  })()

  const confBadge = auto ? (
    <span
      className={`rounded px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide ${confidenceClasses(
        auto.confidence,
      )}`}
      title="auto-match confidence"
    >
      {auto.confidence}
    </span>
  ) : (
    <span className="rounded bg-slate-700 px-1.5 py-0.5 text-[10px] text-slate-400">
      no match
    </span>
  )

  return (
    <div
      className={`rounded-lg border p-3 transition-colors ${
        row.status === 'ok'
          ? 'border-emerald-700/60 bg-emerald-950/20'
          : row.status === 'failed'
            ? 'border-red-800/60 bg-red-950/20'
            : row.included
              ? 'border-slate-600 bg-slate-900/60'
              : 'border-slate-700 bg-slate-900/30 opacity-80'
      }`}
    >
      <div className="flex items-start gap-3">
        <input
          type="checkbox"
          checked={row.included}
          onChange={(e) => onChange({ included: e.target.checked })}
          disabled={row.status === 'ok'}
          className="mt-1 h-4 w-4 shrink-0 rounded border-slate-600 bg-slate-900 accent-blue-500"
        />

        <div className="min-w-0 flex-1">
          {/* Path + parsed summary */}
          <div className="flex flex-wrap items-center gap-2">
            <span className="truncate text-sm font-medium text-white" title={row.path}>
              {shortName(row.path)}
            </span>
            {a.parsedTitle && (
              <span className="rounded bg-slate-700 px-1.5 py-0.5 text-[11px] text-slate-300">
                {a.parsedTitle}
              </span>
            )}
            {a.parsedSeason != null && (
              <span className="rounded bg-slate-700 px-1.5 py-0.5 text-[11px] text-slate-300">
                S{String(a.parsedSeason).padStart(2, '0')}
                {a.parsedEpisodes[0] != null &&
                  `E${String(a.parsedEpisodes[0]).padStart(2, '0')}`}
              </span>
            )}
            {a.parsedYear && (
              <span className="rounded bg-slate-700 px-1.5 py-0.5 text-[11px] text-slate-300">
                {a.parsedYear}
              </span>
            )}
            {a.parsedQuality && a.parsedQuality !== 'Unknown' && (
              <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[11px] text-blue-300">
                {a.parsedQuality}
              </span>
            )}
            <span className="text-[11px] text-slate-500">
              {a.files.length} file{a.files.length === 1 ? '' : 's'}
            </span>
            <div className="ml-auto flex items-center gap-2">
              {statusBadge}
              {!statusBadge && confBadge}
            </div>
          </div>

          <div className="mt-1 truncate font-mono text-[11px] text-slate-500" title={row.path}>
            {row.path}
          </div>

          {/* Controls */}
          <div className="mt-2 flex flex-wrap items-center gap-2">
            {/* Type toggle */}
            <div className="flex overflow-hidden rounded-md border border-slate-700">
              <TypeToggleButton
                active={row.mediaType === 'series'}
                onClick={() =>
                  onChange({
                    mediaType: 'series',
                    targetId:
                      auto && auto.mediaType === 'series'
                        ? auto.mediaId
                        : (a.seriesMatches[0]?.id ?? 0),
                    episodeId:
                      auto && auto.mediaType === 'series' ? (auto.episodeId ?? 0) : 0,
                  })
                }
              >
                <Tv size={12} /> Series
              </TypeToggleButton>
              <TypeToggleButton
                active={row.mediaType === 'movie'}
                onClick={() =>
                  onChange({
                    mediaType: 'movie',
                    targetId:
                      auto && auto.mediaType === 'movie'
                        ? auto.mediaId
                        : (a.movieMatches[0]?.id ?? 0),
                    episodeId: 0,
                  })
                }
              >
                <Film size={12} /> Movie
              </TypeToggleButton>
            </div>

            {/* Target dropdown */}
            <div className="relative min-w-[200px] flex-1">
              <select
                value={row.targetId}
                onChange={(e) => onChange({ targetId: Number(e.target.value), episodeId: 0 })}
                disabled={row.status === 'ok'}
                className="w-full appearance-none rounded border border-slate-600 bg-slate-700 px-2 py-1 pr-6 text-xs text-white focus:border-blue-500 focus:outline-none disabled:opacity-60"
              >
                <option value={0}>
                  {options.length === 0
                    ? `No ${row.mediaType} in library`
                    : `Select ${row.mediaType}…`}
                </option>
                {options.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.title}
                    {m.year ? ` (${m.year})` : ''}
                  </option>
                ))}
              </select>
              <ChevronDown
                size={12}
                className="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 text-slate-400"
              />
            </div>

            {/* Episode dropdown (series only) */}
            {row.mediaType === 'series' && row.targetId > 0 && (
              <div className="relative min-w-[180px]">
                {episodesLoading ? (
                  <div className="flex items-center gap-1 rounded border border-slate-700 bg-slate-700 px-2 py-1 text-xs text-slate-400">
                    <Loader2 size={12} className="animate-spin" /> episodes…
                  </div>
                ) : (
                  <>
                    <select
                      value={row.episodeId}
                      onChange={(e) => onChange({ episodeId: Number(e.target.value) })}
                      disabled={row.status === 'ok'}
                      className="w-full appearance-none rounded border border-slate-600 bg-slate-700 px-2 py-1 pr-6 text-xs text-white focus:border-blue-500 focus:outline-none disabled:opacity-60"
                    >
                      <option value={0}>Auto-detect episode</option>
                      {seasonEpisodes.map((ep) => (
                        <option key={ep.id} value={ep.id}>
                          S{String(ep.seasonNumber).padStart(2, '0')}E
                          {String(ep.episodeNumber).padStart(2, '0')}
                          {ep.title ? ` — ${ep.title}` : ''}
                        </option>
                      ))}
                    </select>
                    <ChevronDown
                      size={12}
                      className="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 text-slate-400"
                    />
                  </>
                )}
              </div>
            )}
          </div>

          {row.resultMsg && row.status === 'failed' && (
            <div className="mt-2 text-xs text-red-300">{row.resultMsg}</div>
          )}
        </div>
      </div>
    </div>
  )
}

function TypeToggleButton({
  active,
  onClick,
  children,
}: {
  active: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      onClick={onClick}
      className={`inline-flex items-center gap-1 px-2 py-1 text-xs transition-colors ${
        active
          ? 'bg-blue-600 text-white'
          : 'bg-slate-700 text-slate-300 hover:bg-slate-600'
      }`}
    >
      {children}
    </button>
  )
}
