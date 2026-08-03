// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState, useMemo } from 'react'
import {
  X,
  Download,
  Loader2,
  CheckCircle,
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  ExternalLink,
  ArrowUpDown,
} from 'lucide-react'
import { useInteractiveSearch, useGrabRelease } from '../hooks/useApi'
import type { DownloadDecision } from '../api/types'

interface InteractiveSearchModalProps {
  title: string
  term: string
  mediaType: 'series' | 'movie'
  qualityProfileId?: number
  seriesId?: number
  movieId?: number
  episodeId?: number
  onClose: () => void
}

function formatSize(bytes: number): string {
  if (!bytes || !isFinite(bytes) || bytes <= 0) return '-'
  if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(2)} GB`
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}

function formatAge(days: number): string {
  if (days <= 0) return 'Today'
  if (days === 1) return '1 day'
  if (days < 30) return `${days}d`
  if (days < 365) return `${Math.floor(days / 30)}mo`
  return `${(days / 365).toFixed(1)}y`
}

type SortField = 'title' | 'indexer' | 'protocol' | 'size' | 'age' | 'seeders' | 'score'
type SortDir = 'asc' | 'desc'

function sortDecisions(decisions: DownloadDecision[], field: SortField, dir: SortDir): DownloadDecision[] {
  const sorted = [...decisions]
  const m = dir === 'asc' ? 1 : -1
  sorted.sort((a, b) => {
    switch (field) {
      case 'title': return m * a.release.title.localeCompare(b.release.title)
      case 'indexer': return m * a.release.indexerName.localeCompare(b.release.indexerName)
      case 'protocol': return m * a.release.protocol.localeCompare(b.release.protocol)
      case 'size': return m * (a.release.size - b.release.size)
      case 'age': return m * (a.release.ageDays - b.release.ageDays)
      case 'seeders': return m * ((a.release.seeders ?? -1) - (b.release.seeders ?? -1))
      case 'score': return m * (a.customFormatScore - b.customFormatScore)
    }
  })
  return sorted
}

export default function InteractiveSearchModal({
  title,
  term,
  mediaType,
  qualityProfileId,
  seriesId,
  movieId,
  episodeId,
  onClose,
}: InteractiveSearchModalProps) {
  const { data: decisions, isLoading, error } = useInteractiveSearch({
    term,
    mediaType,
    qualityProfileId,
    seriesId,
    movieId,
    episodeId,
  })
  const grabMutation = useGrabRelease()
  const [grabbedGuids, setGrabbedGuids] = useState<Set<string>>(new Set())
  const [sortField, setSortField] = useState<SortField>('size')
  const [sortDir, setSortDir] = useState<SortDir>('desc')
  const [showRejected, setShowRejected] = useState(false)

  const handleGrab = (d: DownloadDecision) => {
    if (!d.release.downloadUrl) return
    grabMutation.mutate(
      {
        guid: d.release.guid,
        indexerId: d.release.indexerId,
        title: d.release.title,
        downloadUrl: d.release.downloadUrl,
        protocol: d.release.protocol,
        size: d.release.size,
        mediaId: mediaType === 'movie' ? movieId : seriesId,
        mediaType,
        episodeId,
      },
      {
        onSuccess: () => {
          setGrabbedGuids((prev) => new Set(prev).add(d.release.guid))
        },
      },
    )
  }

  const toggleSort = (field: SortField) => {
    if (sortField === field) {
      setSortDir((d) => (d === 'asc' ? 'desc' : 'asc'))
    } else {
      setSortField(field)
      setSortDir(field === 'title' || field === 'indexer' ? 'asc' : 'desc')
    }
  }

  const approved = useMemo(
    () => sortDecisions(decisions?.filter((d) => d.approved) ?? [], sortField, sortDir),
    [decisions, sortField, sortDir],
  )
  const rejected = useMemo(
    () => sortDecisions(decisions?.filter((d) => !d.approved) ?? [], sortField, sortDir),
    [decisions, sortField, sortDir],
  )

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-8 px-4" onClick={onClose}>
      <div
        className="flex max-h-[90vh] w-full max-w-6xl flex-col rounded-xl bg-slate-800 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-700 px-5 py-3">
          <div className="min-w-0">
            <h3 className="text-base font-semibold">Interactive Search</h3>
            <p className="text-xs text-slate-400 truncate">{title}</p>
          </div>
          <div className="flex items-center gap-4 shrink-0">
            {decisions && decisions.length > 0 && (
              <div className="flex items-center gap-3 text-xs">
                <span className="text-green-400">{approved.length} approved</span>
                {rejected.length > 0 && (
                  <span className="text-yellow-500">{rejected.length} rejected</span>
                )}
              </div>
            )}
            <button onClick={onClose} className="text-slate-400 hover:text-white p-1">
              <X size={18} />
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-auto">
          {isLoading && (
            <div className="flex flex-col items-center justify-center py-16">
              <Loader2 size={28} className="animate-spin text-blue-500" />
              <p className="mt-3 text-sm text-slate-400">Searching indexers...</p>
            </div>
          )}

          {error && (
            <div className="m-4 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-400">
              Search failed: {error.message}
            </div>
          )}

          {decisions && decisions.length === 0 && (
            <div className="flex flex-col items-center justify-center py-16 text-slate-400">
              <AlertTriangle size={28} className="mb-2 text-slate-500" />
              <p className="text-sm">No releases found</p>
            </div>
          )}

          {decisions && decisions.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead className="sticky top-0 z-10 bg-slate-800">
                  <tr className="border-b border-slate-700 text-left text-[11px] uppercase tracking-wide text-slate-500">
                    <SortHeader field="title" label="Release" current={sortField} dir={sortDir} onClick={toggleSort} className="pl-5 w-[40%]" />
                    <SortHeader field="indexer" label="Indexer" current={sortField} dir={sortDir} onClick={toggleSort} />
                    <SortHeader field="protocol" label="Type" current={sortField} dir={sortDir} onClick={toggleSort} />
                    <SortHeader field="size" label="Size" current={sortField} dir={sortDir} onClick={toggleSort} />
                    <SortHeader field="age" label="Age" current={sortField} dir={sortDir} onClick={toggleSort} />
                    <SortHeader field="seeders" label="Peers" current={sortField} dir={sortDir} onClick={toggleSort} />
                    <SortHeader field="score" label="Score" current={sortField} dir={sortDir} onClick={toggleSort} />
                    <th className="px-3 py-2.5 font-medium text-right pr-5">Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {approved.map((d) => (
                    <ReleaseRow
                      key={d.release.guid}
                      decision={d}
                      onGrab={() => handleGrab(d)}
                      grabbed={grabbedGuids.has(d.release.guid)}
                      grabbing={grabMutation.isPending && grabMutation.variables?.guid === d.release.guid}
                    />
                  ))}
                </tbody>
              </table>

              {/* Rejected section */}
              {rejected.length > 0 && (
                <div className="border-t border-slate-700">
                  <button
                    onClick={() => setShowRejected((s) => !s)}
                    className="flex w-full items-center gap-2 px-5 py-2 text-xs text-slate-400 hover:text-slate-200 transition-colors"
                  >
                    {showRejected ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                    {rejected.length} rejected release{rejected.length !== 1 ? 's' : ''}
                  </button>
                  {showRejected && (
                    <table className="w-full text-sm">
                      <tbody>
                        {rejected.map((d) => (
                          <ReleaseRow
                            key={d.release.guid}
                            decision={d}
                            onGrab={() => handleGrab(d)}
                            grabbed={grabbedGuids.has(d.release.guid)}
                            grabbing={grabMutation.isPending && grabMutation.variables?.guid === d.release.guid}
                          />
                        ))}
                      </tbody>
                    </table>
                  )}
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

function SortHeader({
  field,
  label,
  current,
  dir,
  onClick,
  className = '',
}: {
  field: SortField
  label: string
  current: SortField
  dir: SortDir
  onClick: (f: SortField) => void
  className?: string
}) {
  const active = current === field
  return (
    <th className={`px-3 py-2.5 font-medium ${className}`}>
      <button
        onClick={() => onClick(field)}
        className={`inline-flex items-center gap-1 hover:text-slate-300 transition-colors ${active ? 'text-blue-400' : ''}`}
      >
        {label}
        {active ? (
          dir === 'asc' ? <ChevronUp size={12} /> : <ChevronDown size={12} />
        ) : (
          <ArrowUpDown size={10} className="opacity-40" />
        )}
      </button>
    </th>
  )
}

function ReleaseRow({
  decision,
  onGrab,
  grabbed,
  grabbing,
}: {
  decision: DownloadDecision
  onGrab: () => void
  grabbed: boolean
  grabbing: boolean
}) {
  const r = decision.release
  const hasUrl = !!r.downloadUrl
  const isTorrent = r.protocol === 'torrent'

  return (
    <tr className={`border-b border-slate-700/40 transition-colors ${
      decision.approved ? 'hover:bg-slate-700/30' : 'hover:bg-slate-700/20 opacity-70'
    }`}>
      {/* Title */}
      <td className="py-2 pl-5 pr-3">
        <div className="flex items-start gap-2">
          {decision.approved ? (
            <CheckCircle size={14} className="shrink-0 mt-0.5 text-green-500" />
          ) : (
            <AlertTriangle size={14} className="shrink-0 mt-0.5 text-yellow-500" />
          )}
          <div className="min-w-0">
            <div className="text-xs text-white leading-snug break-all line-clamp-2" title={r.title}>
              {r.title}
            </div>
            {decision.rejections.length > 0 && (
              <div className="mt-1 flex flex-wrap gap-1">
                {decision.rejections.map((rej, i) => (
                  <span key={i} className="rounded bg-yellow-500/15 px-1.5 py-px text-[10px] text-yellow-400">
                    {rej.reason}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      </td>

      {/* Indexer */}
      <td className="px-3 py-2 text-xs text-blue-400 whitespace-nowrap">{r.indexerName}</td>

      {/* Protocol */}
      <td className="px-3 py-2">
        <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
          isTorrent ? 'bg-orange-500/20 text-orange-400' : 'bg-purple-500/20 text-purple-400'
        }`}>
          {isTorrent ? 'Torrent' : 'Usenet'}
        </span>
      </td>

      {/* Size */}
      <td className="px-3 py-2 text-xs text-slate-300 whitespace-nowrap">{formatSize(r.size)}</td>

      {/* Age */}
      <td className="px-3 py-2 text-xs text-slate-400 whitespace-nowrap">{formatAge(r.ageDays)}</td>

      {/* Peers */}
      <td className="px-3 py-2 text-xs whitespace-nowrap">
        {isTorrent && r.seeders != null ? (
          <span>
            <span className={r.seeders > 0 ? 'text-green-400' : 'text-red-400'}>{r.seeders}</span>
            <span className="text-slate-600"> / </span>
            <span className="text-slate-400">{r.leechers ?? 0}</span>
          </span>
        ) : (
          <span className="text-slate-600">-</span>
        )}
      </td>

      {/* Custom Format Score */}
      <td className="px-3 py-2 text-xs">
        <div className="whitespace-nowrap">
          {decision.customFormatScore !== 0 ? (
            <span className={decision.customFormatScore > 0 ? 'text-green-400' : 'text-red-400'}>
              {decision.customFormatScore > 0 ? '+' : ''}{decision.customFormatScore}
            </span>
          ) : (
            <span className="text-slate-600">0</span>
          )}
        </div>
        {decision.matchedFormats?.length > 0 && (
          <div className="mt-1 flex flex-wrap gap-0.5 max-w-[200px]">
            {decision.matchedFormats.map(mf => (
              <span
                key={mf.formatId}
                className={`rounded px-1 py-px text-[9px] leading-tight ${
                  mf.score > 0
                    ? 'bg-green-500/15 text-green-400'
                    : mf.score < 0
                      ? 'bg-red-500/15 text-red-400'
                      : 'bg-slate-500/15 text-slate-400'
                }`}
                title={`${mf.formatName}: ${mf.score > 0 ? '+' : ''}${mf.score}`}
              >
                {mf.formatName}
              </span>
            ))}
          </div>
        )}
      </td>

      {/* Actions */}
      <td className="px-3 py-2 pr-5">
        <div className="flex items-center justify-end gap-1.5">
          {r.infoUrl && (
            <a
              href={r.infoUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="rounded p-1 text-slate-500 hover:text-blue-400 hover:bg-slate-700 transition-colors"
              title="Info page"
            >
              <ExternalLink size={13} />
            </a>
          )}
          {grabbed ? (
            <span className="flex items-center gap-1 rounded-md bg-green-500/15 px-2 py-1 text-[11px] text-green-400">
              <CheckCircle size={12} /> Grabbed
            </span>
          ) : (
            <button
              onClick={onGrab}
              disabled={grabbing || !hasUrl}
              className="flex items-center gap-1 rounded-md bg-blue-600 px-2.5 py-1 text-[11px] font-medium text-white hover:bg-blue-500 disabled:opacity-40 transition-colors"
              title={hasUrl ? 'Download this release' : 'No download URL'}
            >
              {grabbing ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <Download size={12} />
              )}
              Grab
            </button>
          )}
        </div>
      </td>
    </tr>
  )
}
