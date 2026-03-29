import { useState } from 'react'
import { X, Download, Loader2, CheckCircle, AlertTriangle, ChevronDown, ChevronRight } from 'lucide-react'
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
  if (bytes >= 1073741824) return `${(bytes / 1073741824).toFixed(2)} GB`
  if (bytes >= 1048576) return `${(bytes / 1048576).toFixed(1)} MB`
  return `${(bytes / 1024).toFixed(0)} KB`
}

function formatAge(days: number): string {
  if (days === 0) return 'Today'
  if (days === 1) return '1 day'
  if (days < 365) return `${days}d`
  return `${(days / 365).toFixed(1)}y`
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

  const approvedCount = decisions?.filter((d) => d.approved).length ?? 0
  const rejectedCount = decisions?.filter((d) => !d.approved).length ?? 0

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-12 px-4">
      <div className="flex max-h-[85vh] w-full max-w-5xl flex-col rounded-xl bg-slate-800 shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-slate-700 px-6 py-4">
          <div>
            <h3 className="text-lg font-semibold">Interactive Search</h3>
            <p className="mt-0.5 text-sm text-slate-400 truncate">{title}</p>
          </div>
          <button onClick={onClose} className="text-slate-400 hover:text-white">
            <X size={20} />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto">
          {isLoading && (
            <div className="flex flex-col items-center justify-center py-16">
              <Loader2 size={32} className="animate-spin text-blue-500" />
              <p className="mt-3 text-sm text-slate-400">Searching indexers...</p>
            </div>
          )}

          {error && (
            <div className="m-6 rounded-lg border border-red-500/30 bg-red-500/10 p-4 text-sm text-red-400">
              Search failed: {error.message}
            </div>
          )}

          {decisions && decisions.length === 0 && (
            <div className="flex flex-col items-center justify-center py-16 text-slate-400">
              <AlertTriangle size={32} className="mb-3 text-slate-500" />
              <p>No releases found</p>
            </div>
          )}

          {decisions && decisions.length > 0 && (
            <>
              <div className="flex gap-4 border-b border-slate-700 px-6 py-3 text-xs text-slate-400">
                <span className="text-green-400">{approvedCount} approved</span>
                {rejectedCount > 0 && (
                  <span className="text-yellow-500">{rejectedCount} rejected</span>
                )}
                <span>{decisions.length} total</span>
              </div>

              {/* Approved releases */}
              <div className="divide-y divide-slate-700/50">
                {decisions.filter((d) => d.approved).map((d) => (
                  <ReleaseRow
                    key={d.release.guid}
                    decision={d}
                    onGrab={() => handleGrab(d)}
                    grabbed={grabbedGuids.has(d.release.guid)}
                    grabbing={grabMutation.isPending && grabMutation.variables?.guid === d.release.guid}
                  />
                ))}
              </div>

              {/* Rejected releases (collapsed) */}
              {rejectedCount > 0 && (
                <RejectedSection
                  decisions={decisions.filter((d) => !d.approved)}
                  onGrab={handleGrab}
                  grabbedGuids={grabbedGuids}
                  grabbingGuid={grabMutation.isPending ? grabMutation.variables?.guid : undefined}
                />
              )}
            </>
          )}
        </div>
      </div>
    </div>
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

  return (
    <div className="flex items-center gap-3 px-6 py-2.5 hover:bg-slate-700/30 transition-colors">
      {/* Approval indicator */}
      {decision.approved ? (
        <CheckCircle size={14} className="shrink-0 text-green-500" />
      ) : (
        <AlertTriangle size={14} className="shrink-0 text-yellow-500" />
      )}

      {/* Title + metadata */}
      <div className="min-w-0 flex-1">
        <div className="text-sm text-white truncate" title={r.title}>{r.title}</div>
        <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[11px] text-slate-400">
          <span className="text-blue-400">{r.indexerName}</span>
          <span>{r.protocol === 'torrent' ? 'Torrent' : 'Usenet'}</span>
          <span>{formatSize(r.size)}</span>
          <span>{formatAge(r.ageDays)}</span>
          {r.seeders != null && (
            <span className={r.seeders > 0 ? 'text-green-400' : 'text-red-400'}>
              {r.seeders}S / {r.leechers ?? 0}L
            </span>
          )}
          {decision.customFormatScore !== 0 && (
            <span className={decision.customFormatScore > 0 ? 'text-green-400' : 'text-red-400'}>
              CF: {decision.customFormatScore > 0 ? '+' : ''}{decision.customFormatScore}
            </span>
          )}
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

      {/* Grab button */}
      {grabbed ? (
        <span className="shrink-0 flex items-center gap-1 text-xs text-green-400">
          <CheckCircle size={14} /> Grabbed
        </span>
      ) : (
        <button
          onClick={onGrab}
          disabled={grabbing || !hasUrl}
          className="shrink-0 flex items-center gap-1.5 rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700 disabled:opacity-40 transition-colors"
          title={hasUrl ? 'Download this release' : 'No download URL available'}
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
  )
}

function RejectedSection({
  decisions,
  onGrab,
  grabbedGuids,
  grabbingGuid,
}: {
  decisions: DownloadDecision[]
  onGrab: (d: DownloadDecision) => void
  grabbedGuids: Set<string>
  grabbingGuid?: string
}) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="border-t border-slate-700">
      <button
        onClick={() => setExpanded((e) => !e)}
        className="flex w-full items-center gap-2 px-6 py-2.5 text-xs text-slate-400 hover:text-slate-200 transition-colors"
      >
        {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <span>{decisions.length} rejected release{decisions.length !== 1 ? 's' : ''}</span>
      </button>
      {expanded && (
        <div className="divide-y divide-slate-700/50">
          {decisions.map((d) => (
            <ReleaseRow
              key={d.release.guid}
              decision={d}
              onGrab={() => onGrab(d)}
              grabbed={grabbedGuids.has(d.release.guid)}
              grabbing={grabbingGuid === d.release.guid}
            />
          ))}
        </div>
      )}
    </div>
  )
}
