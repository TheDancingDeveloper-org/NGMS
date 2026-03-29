import { useState } from 'react'
import { useUnifiedSessions, useStopStreamSession, useSystemStatus } from '../hooks/useApi'
import type { UnifiedSession } from '../api/types'
import { Play, Square, Monitor, Tv, ChevronDown, ChevronUp } from 'lucide-react'

type SourceFilter = 'all' | 'ngms' | 'plex'

export default function Streaming() {
  const { data: sessions, isLoading } = useUnifiedSessions()
  const { data: status } = useSystemStatus()
  const stopSession = useStopStreamSession()
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>('all')
  const [advanced, setAdvanced] = useState(false)

  const plexEnabled = status?.modules?.plexIntegration ?? false
  const streamingEnabled = status?.modules?.streaming ?? false

  const filtered = (sessions ?? []).filter(
    (s) => sourceFilter === 'all' || s.source === sourceFilter,
  )

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <h2 className="text-xl font-semibold">Active Streams</h2>
          <span className="rounded-full bg-slate-700 px-2.5 py-0.5 text-xs font-medium text-slate-300">
            {filtered.length}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {/* Source filter */}
          {plexEnabled && streamingEnabled && (
            <div className="flex rounded-lg bg-slate-800 p-0.5 text-xs">
              {(['all', 'ngms', 'plex'] as const).map((f) => (
                <button
                  key={f}
                  onClick={() => setSourceFilter(f)}
                  className={`rounded-md px-3 py-1.5 font-medium transition-colors ${
                    sourceFilter === f
                      ? 'bg-blue-600 text-white'
                      : 'text-slate-400 hover:text-white'
                  }`}
                >
                  {f === 'all' ? 'All' : f === 'ngms' ? 'NGMS' : 'Plex'}
                </button>
              ))}
            </div>
          )}
          {/* Advanced toggle */}
          <button
            onClick={() => setAdvanced(!advanced)}
            className="flex items-center gap-1 rounded-lg bg-slate-800 px-3 py-1.5 text-xs font-medium text-slate-400 hover:text-white transition-colors"
          >
            {advanced ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
            Advanced
          </button>
        </div>
      </div>

      {/* Loading */}
      {isLoading && (
        <div className="flex items-center justify-center py-12">
          <div className="h-8 w-8 animate-spin rounded-full border-4 border-slate-600 border-t-blue-500" />
        </div>
      )}

      {/* Empty */}
      {!isLoading && filtered.length === 0 && (
        <div className="rounded-lg border border-slate-700 p-8 text-center text-slate-400">
          <Play size={32} className="mx-auto mb-3 opacity-50" />
          <p>No active streams</p>
          <p className="mt-1 text-sm">
            {plexEnabled && streamingEnabled
              ? 'Start playing media in Plex or from the library'
              : plexEnabled
                ? 'Start playing media in Plex'
                : 'Start playing media from the Series or Movies pages'}
          </p>
        </div>
      )}

      {/* Session list */}
      {filtered.length > 0 && (
        <div className="space-y-2">
          {filtered.map((session) => (
            <SessionCard
              key={`${session.source}-${session.id}`}
              session={session}
              advanced={advanced}
              onStop={session.source === 'ngms' ? () => stopSession.mutate(session.id) : undefined}
              stopPending={stopSession.isPending}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function SessionCard({
  session,
  advanced,
  onStop,
  stopPending,
}: {
  session: UnifiedSession
  advanced: boolean
  onStop?: () => void
  stopPending: boolean
}) {
  const stateColor =
    session.state === 'playing' ? 'bg-green-500' :
    session.state === 'paused' ? 'bg-yellow-500' :
    session.state === 'buffering' ? 'bg-orange-500' :
    'bg-green-500'

  const sourceBadge = session.source === 'plex' ? (
    <span className="rounded bg-orange-500/20 px-1.5 py-0.5 text-[10px] font-semibold text-orange-400">Plex</span>
  ) : (
    <span className="rounded bg-blue-500/20 px-1.5 py-0.5 text-[10px] font-semibold text-blue-400">NGMS</span>
  )

  const typeBadge = session.sessionType === 'transcode' ? (
    <span className="rounded bg-purple-500/20 px-1.5 py-0.5 text-[10px] font-medium text-purple-400">Transcode</span>
  ) : (
    <span className="rounded bg-green-500/20 px-1.5 py-0.5 text-[10px] font-medium text-green-400">Direct Play</span>
  )

  return (
    <div className="rounded-lg bg-slate-800 p-4">
      {/* Main row */}
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1 min-w-0 space-y-1.5">
          {/* Title + badges */}
          <div className="flex items-center gap-2 flex-wrap">
            <span className={`inline-block h-2 w-2 shrink-0 rounded-full ${stateColor}`} />
            {sourceBadge}
            {typeBadge}
            <span className="text-sm font-medium text-white truncate">
              {session.title ?? `File #${session.id}`}
            </span>
          </div>

          {/* User / player / state */}
          <div className="flex items-center gap-3 text-xs text-slate-400">
            {session.user && (
              <span className="flex items-center gap-1">
                <Monitor size={10} /> {session.user}
              </span>
            )}
            {session.player && (
              <span className="flex items-center gap-1">
                <Tv size={10} /> {session.player}
                {session.platform && ` (${session.platform})`}
              </span>
            )}
            {session.isLocal != null && (
              <span className={session.isLocal ? 'text-green-400' : 'text-slate-500'}>
                {session.isLocal ? 'Local' : 'Remote'}
              </span>
            )}
            <span className="capitalize">{session.state}</span>
          </div>

          {/* Progress bar */}
          {session.progressPercent != null && (
            <div className="flex items-center gap-2">
              <div className="h-1 flex-1 overflow-hidden rounded-full bg-slate-600">
                <div
                  className="h-full rounded-full bg-blue-500 transition-all"
                  style={{ width: `${Math.min(session.progressPercent, 100)}%` }}
                />
              </div>
              <span className="text-[10px] text-slate-500">
                {session.progressPercent.toFixed(0)}%
              </span>
            </div>
          )}

          {/* Advanced details */}
          {advanced && (
            <div className="mt-1 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-slate-500">
              {session.resolution && <span>Resolution: <b className="text-slate-300">{session.resolution}p</b></span>}
              {session.videoCodec && <span>Video: <b className="text-slate-300">{session.videoCodec}</b></span>}
              {session.audioCodec && <span>Audio: <b className="text-slate-300">{session.audioCodec}</b></span>}
              {session.bitrate != null && <span>Bitrate: <b className="text-slate-300">{(session.bitrate / 1000).toFixed(1)} Mbps</b></span>}
              {session.videoDecision && <span>Video: <b className="text-slate-300">{session.videoDecision}</b></span>}
              {session.audioDecision && <span>Audio: <b className="text-slate-300">{session.audioDecision}</b></span>}
              {session.transcodeSpeed != null && <span>Speed: <b className="text-slate-300">{session.transcodeSpeed.toFixed(1)}x</b></span>}
            </div>
          )}
        </div>

        {/* Stop button (NGMS only) */}
        {onStop && (
          <button
            onClick={onStop}
            disabled={stopPending}
            className="shrink-0 rounded bg-red-600/20 p-2 text-red-400 hover:bg-red-600/40 transition-colors disabled:opacity-50"
            title="Stop stream"
          >
            <Square size={16} />
          </button>
        )}
      </div>
    </div>
  )
}
