// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState, useEffect } from 'react'
import { X, Loader2, Download, AlertCircle, CheckCircle, ChevronDown } from 'lucide-react'
import { apiFetch } from '../api/client'
import type { Episode } from '../api/types'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface AnalyzedFile {
  name: string
  path: string
  size: number
}

interface SeriesMatch {
  id: number
  title: string
  year: number | null
  posterUrl: string | null
}

interface AnalyzeResponse {
  path: string
  folderName: string
  files: AnalyzedFile[]
  parsedTitle: string | null
  parsedSeason: number | null
  parsedEpisodes: number[]
  parsedQuality: string
  seriesMatches: SeriesMatch[]
}

interface ImportResult {
  imported: number
  skipped: number
  errors: string[]
  logLines: string[]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatSize(bytes: number): string {
  if (!bytes) return '—'
  const units = ['B', 'KB', 'MB', 'GB']
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export default function ManualImportModal({
  path,
  onClose,
  onImported,
}: {
  path: string
  onClose: () => void
  onImported: () => void
}) {
  const [analysis, setAnalysis] = useState<AnalyzeResponse | null>(null)
  const [analyzing, setAnalyzing] = useState(true)
  const [analyzeError, setAnalyzeError] = useState<string | null>(null)

  const [selectedSeriesId, setSelectedSeriesId] = useState<number>(0)
  const [episodes, setEpisodes] = useState<Episode[]>([])
  const [loadingEpisodes, setLoadingEpisodes] = useState(false)
  const [selectedEpisodeId, setSelectedEpisodeId] = useState<number>(0)

  const [importing, setImporting] = useState(false)
  const [result, setResult] = useState<ImportResult | null>(null)
  const [importError, setImportError] = useState<string | null>(null)

  // Analyze the path on mount
  useEffect(() => {
    setAnalyzing(true)
    setAnalyzeError(null)
    apiFetch<AnalyzeResponse>('/manual-import/analyze', {
      method: 'POST',
      body: JSON.stringify({ path }),
    })
      .then((data) => {
        setAnalysis(data)
        if (data.seriesMatches.length > 0) {
          setSelectedSeriesId(data.seriesMatches[0].id)
        }
      })
      .catch((e) => setAnalyzeError(e instanceof Error ? e.message : String(e)))
      .finally(() => setAnalyzing(false))
  }, [path])

  // Load episodes when series is selected
  useEffect(() => {
    if (!selectedSeriesId) {
      setEpisodes([])
      setSelectedEpisodeId(0)
      return
    }
    setLoadingEpisodes(true)
    apiFetch<Episode[]>(`/series/${selectedSeriesId}/episodes`)
      .then((eps) => {
        setEpisodes(eps)
        // Auto-select the episode matching parsed season + episode
        if (analysis?.parsedSeason != null && analysis.parsedEpisodes.length > 0) {
          const match = eps.find(
            (e) =>
              e.seasonNumber === analysis.parsedSeason &&
              e.episodeNumber === analysis.parsedEpisodes[0],
          )
          if (match) setSelectedEpisodeId(match.id)
        }
      })
      .catch(() => setEpisodes([]))
      .finally(() => setLoadingEpisodes(false))
  }, [selectedSeriesId, analysis?.parsedSeason, analysis?.parsedEpisodes])

  const handleImport = async () => {
    if (!selectedSeriesId) return
    setImporting(true)
    setImportError(null)
    try {
      const body: { path: string; seriesId: number; episodeId?: number } = {
        path,
        seriesId: selectedSeriesId,
      }
      if (selectedEpisodeId) body.episodeId = selectedEpisodeId
      const res = await apiFetch<ImportResult>('/manual-import/import', {
        method: 'POST',
        body: JSON.stringify(body),
      })
      setResult(res)
      if (res.imported > 0) onImported()
    } catch (e) {
      setImportError(e instanceof Error ? e.message : String(e))
    } finally {
      setImporting(false)
    }
  }

  // ── Episode options grouped by season ──────────────────────────────────
  const seasonEpisodes = episodes.filter(
    (e) =>
      analysis?.parsedSeason == null ||
      e.seasonNumber === analysis.parsedSeason,
  )

  const canImport = selectedSeriesId > 0 && !importing && !result

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg rounded-xl bg-slate-800 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-700 px-5 py-4">
          <h2 className="text-base font-semibold text-white">Manual Import</h2>
          <button onClick={onClose} className="text-slate-400 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        <div className="space-y-4 p-5">
          {/* Path */}
          <div>
            <label className="mb-1 block text-xs font-medium text-slate-400">Path</label>
            <p className="truncate rounded bg-slate-900 px-3 py-2 font-mono text-xs text-slate-300">
              {path}
            </p>
          </div>

          {/* Analysis */}
          {analyzing && (
            <div className="flex items-center gap-2 text-sm text-slate-400">
              <Loader2 size={14} className="animate-spin" /> Analysing release…
            </div>
          )}

          {analyzeError && (
            <div className="flex items-center gap-2 rounded-lg bg-red-950/40 p-3 text-sm text-red-300">
              <AlertCircle size={14} className="shrink-0" /> {analyzeError}
            </div>
          )}

          {analysis && !analyzing && (
            <>
              {/* Files found */}
              {analysis.files.length > 0 && (
                <div>
                  <label className="mb-1 block text-xs font-medium text-slate-400">
                    Video files ({analysis.files.length})
                  </label>
                  <div className="max-h-28 overflow-y-auto rounded-lg bg-slate-900">
                    {analysis.files.map((f) => (
                      <div
                        key={f.path}
                        className="flex items-center justify-between border-b border-slate-800 px-3 py-1.5 last:border-0"
                      >
                        <span className="truncate text-xs text-slate-300">{f.name}</span>
                        <span className="ml-2 shrink-0 text-xs text-slate-500">
                          {formatSize(f.size)}
                        </span>
                      </div>
                    ))}
                  </div>
                  {analysis.files.length === 0 && (
                    <p className="text-xs text-amber-400">No video files found in this path.</p>
                  )}
                </div>
              )}

              {/* Parsed info */}
              <div className="flex flex-wrap gap-2 text-xs">
                {analysis.parsedTitle && (
                  <span className="rounded bg-slate-700 px-2 py-1 text-slate-300">
                    <span className="text-slate-500">Title: </span>{analysis.parsedTitle}
                  </span>
                )}
                {analysis.parsedSeason != null && (
                  <span className="rounded bg-slate-700 px-2 py-1 text-slate-300">
                    <span className="text-slate-500">S</span>{String(analysis.parsedSeason).padStart(2, '0')}
                    {analysis.parsedEpisodes.length > 0 && (
                      <><span className="text-slate-500">E</span>{String(analysis.parsedEpisodes[0]).padStart(2, '0')}</>
                    )}
                  </span>
                )}
                {analysis.parsedQuality && analysis.parsedQuality !== 'Unknown' && (
                  <span className="rounded bg-blue-500/20 px-2 py-1 text-blue-400">
                    {analysis.parsedQuality}
                  </span>
                )}
              </div>

              {/* Series picker */}
              <div>
                <label className="mb-1 block text-xs font-medium text-slate-400">
                  Series (your library)
                </label>
                {analysis.seriesMatches.length === 0 ? (
                  <p className="text-xs text-amber-400">
                    No matching series found in your library. Add the series first via Search.
                  </p>
                ) : (
                  <div className="relative">
                    <select
                      value={selectedSeriesId}
                      onChange={(e) => {
                        setSelectedSeriesId(Number(e.target.value))
                        setSelectedEpisodeId(0)
                      }}
                      className="w-full appearance-none rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 pr-8 text-sm text-white focus:border-blue-500 focus:outline-none"
                    >
                      <option value={0}>Select series…</option>
                      {analysis.seriesMatches.map((s) => (
                        <option key={s.id} value={s.id}>
                          {s.title}{s.year ? ` (${s.year})` : ''}
                        </option>
                      ))}
                    </select>
                    <ChevronDown
                      size={14}
                      className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400"
                    />
                  </div>
                )}
              </div>

              {/* Episode picker */}
              {selectedSeriesId > 0 && (
                <div>
                  <label className="mb-1 block text-xs font-medium text-slate-400">
                    Episode
                    {analysis.parsedSeason != null && (
                      <span className="ml-1 text-slate-500">
                        (Season {analysis.parsedSeason})
                      </span>
                    )}
                  </label>
                  {loadingEpisodes ? (
                    <div className="flex items-center gap-2 text-xs text-slate-400">
                      <Loader2 size={12} className="animate-spin" /> Loading…
                    </div>
                  ) : (
                    <div className="relative">
                      <select
                        value={selectedEpisodeId}
                        onChange={(e) => setSelectedEpisodeId(Number(e.target.value))}
                        className="w-full appearance-none rounded-lg border border-slate-600 bg-slate-700 px-3 py-2 pr-8 text-sm text-white focus:border-blue-500 focus:outline-none"
                      >
                        <option value={0}>Auto-detect from filename</option>
                        {seasonEpisodes.map((ep) => (
                          <option key={ep.id} value={ep.id}>
                            S{String(ep.seasonNumber).padStart(2, '0')}E{String(ep.episodeNumber).padStart(2, '0')}
                            {ep.title ? ` — ${ep.title}` : ''}
                          </option>
                        ))}
                      </select>
                      <ChevronDown
                        size={14}
                        className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-slate-400"
                      />
                    </div>
                  )}
                </div>
              )}
            </>
          )}

          {/* Import result */}
          {result && (
            <div
              className={`rounded-lg p-3 text-sm ${
                result.errors.length > 0
                  ? 'border border-amber-700 bg-amber-950/40 text-amber-300'
                  : 'border border-emerald-700 bg-emerald-950/40 text-emerald-300'
              }`}
            >
              <div className="flex items-center gap-2 font-medium">
                <CheckCircle size={14} />
                {result.imported} file{result.imported === 1 ? '' : 's'} imported
                {result.skipped > 0 && `, ${result.skipped} skipped`}
                {result.errors.length > 0 && `, ${result.errors.length} error(s)`}
              </div>
              {result.errors.length > 0 && (
                <ul className="mt-2 space-y-0.5 text-xs">
                  {result.errors.map((e, i) => (
                    <li key={i}>{e}</li>
                  ))}
                </ul>
              )}
            </div>
          )}

          {importError && (
            <div className="flex items-center gap-2 rounded-lg bg-red-950/40 p-3 text-sm text-red-300">
              <AlertCircle size={14} className="shrink-0" /> {importError}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 border-t border-slate-700 px-5 py-4">
          <button
            onClick={onClose}
            className="rounded-lg bg-slate-700 px-4 py-2 text-sm text-slate-300 hover:bg-slate-600 transition-colors"
          >
            {result ? 'Close' : 'Cancel'}
          </button>
          {!result && (
            <button
              onClick={handleImport}
              disabled={!canImport}
              className="flex items-center gap-1.5 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
            >
              {importing ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <Download size={14} />
              )}
              Import
            </button>
          )}
        </div>
      </div>
    </div>
  )
}
