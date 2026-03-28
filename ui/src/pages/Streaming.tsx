import { useStreamSessions, useStopStreamSession } from '../hooks/useApi'
import { Play, Square } from 'lucide-react'
import { formatTime } from '../utils/date'

export default function Streaming() {
  const { data: sessions, isLoading } = useStreamSessions()
  const stopSession = useStopStreamSession()

  return (
    <div className="space-y-8">
      {/* Active Streams */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-xl font-semibold">Active Streams</h2>
          <span className="text-sm text-slate-400">
            {sessions?.length ?? 0} active
          </span>
        </div>

        {isLoading && (
          <div className="flex items-center justify-center py-12">
            <div className="h-8 w-8 animate-spin rounded-full border-4 border-slate-600 border-t-blue-500" />
          </div>
        )}

        {!isLoading && (!sessions || sessions.length === 0) && (
          <div className="rounded-lg border border-slate-700 p-8 text-center text-slate-400">
            <Play size={32} className="mx-auto mb-3 opacity-50" />
            <p>No active streams</p>
            <p className="mt-1 text-sm">
              Start playing media from the Series or Movies pages
            </p>
          </div>
        )}

        {sessions && sessions.length > 0 && (
          <div className="space-y-2">
            {sessions.map((session) => (
              <div
                key={session.sessionId}
                className="flex items-center justify-between rounded-lg bg-slate-800 p-4"
              >
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span
                      className={`inline-block h-2 w-2 rounded-full ${
                        session.status === 'active'
                          ? 'bg-green-500'
                          : 'bg-yellow-500'
                      }`}
                    />
                    <span className="text-sm font-medium capitalize">
                      {session.sessionType}
                    </span>
                    <span className="text-xs text-slate-500">
                      File #{session.mediaFileId}
                    </span>
                  </div>
                  <div className="text-xs text-slate-400">
                    Started {formatTime(session.startedAt)}
                    {session.transcodeProgress != null && (
                      <span className="ml-2">
                        Progress: {(session.transcodeProgress * 100).toFixed(0)}%
                      </span>
                    )}
                  </div>
                </div>
                <button
                  onClick={() => stopSession.mutate(session.sessionId)}
                  disabled={stopSession.isPending}
                  className="rounded bg-red-600/20 p-2 text-red-400 hover:bg-red-600/40 transition-colors"
                  title="Stop stream"
                >
                  <Square size={16} />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

    </div>
  )
}
