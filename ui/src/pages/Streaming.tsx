import { useState, useEffect } from 'react'
import { useStreamSessions, useStopStreamSession, useCreateClaim, useRemoteClients, useDeleteRemoteClient } from '../hooks/useApi'
import { Play, Square, Link, Copy, Trash2, Check } from 'lucide-react'
import { formatTime } from '../utils/date'

function ClaimCodeSection() {
  const createClaim = useCreateClaim()
  const { data: clients, isLoading: clientsLoading } = useRemoteClients()
  const deleteClient = useDeleteRemoteClient()
  const [copied, setCopied] = useState(false)
  const [expiresIn, setExpiresIn] = useState(0)

  // Countdown timer for active claim code
  useEffect(() => {
    if (!createClaim.data || expiresIn <= 0) return
    const timer = setTimeout(() => setExpiresIn((s) => s - 1), 1000)
    return () => clearTimeout(timer)
  }, [createClaim.data, expiresIn])

  function handleGenerate() {
    createClaim.mutate(undefined, {
      onSuccess: (data) => {
        setExpiresIn(data.expiresInSecs)
        setCopied(false)
      },
    })
  }

  function handleCopy(code: string) {
    void navigator.clipboard.writeText(code)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const activeCode = createClaim.data && expiresIn > 0 ? createClaim.data.code : null
  const minutes = Math.floor(expiresIn / 60)
  const seconds = expiresIn % 60

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-lg font-semibold">Link a Client</h3>
        <p className="mt-1 text-sm text-slate-400">
          Generate a claim code to connect a StackArr Player app to this server.
        </p>
      </div>

      <div className="flex items-center gap-3">
        {activeCode ? (
          <div className="flex items-center gap-3">
            <span className="rounded-lg bg-slate-700 px-4 py-2.5 font-mono text-2xl font-bold tracking-widest text-blue-400">
              {activeCode}
            </span>
            <button
              onClick={() => handleCopy(activeCode)}
              className="rounded-lg bg-slate-700 p-2.5 text-slate-300 hover:bg-slate-600 transition-colors"
              title="Copy code"
            >
              {copied ? <Check size={18} className="text-green-400" /> : <Copy size={18} />}
            </button>
            <span className="text-sm text-slate-400">
              Expires in {minutes}:{seconds.toString().padStart(2, '0')}
            </span>
          </div>
        ) : (
          <button
            onClick={handleGenerate}
            disabled={createClaim.isPending}
            className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-500 disabled:opacity-50 transition-colors"
          >
            <Link size={16} />
            {createClaim.isPending ? 'Generating...' : 'Generate Claim Code'}
          </button>
        )}
      </div>

      {createClaim.isError && (
        <p className="text-sm text-red-400">
          {createClaim.error instanceof Error ? createClaim.error.message : 'Failed to generate claim code'}
        </p>
      )}

      {/* Connected clients list */}
      <div className="mt-6">
        <h4 className="mb-2 text-sm font-medium text-slate-300">Connected Clients</h4>
        {clientsLoading && (
          <div className="h-6 w-6 animate-spin rounded-full border-2 border-slate-600 border-t-blue-500" />
        )}
        {!clientsLoading && (!clients || clients.length === 0) && (
          <p className="text-sm text-slate-500">No clients connected yet</p>
        )}
        {clients && clients.length > 0 && (
          <div className="space-y-2">
            {clients.map((client) => (
              <div
                key={client.id}
                className="flex items-center justify-between rounded-lg bg-slate-800 px-4 py-3"
              >
                <div>
                  <span className="text-sm font-medium">
                    {client.clientName || 'Unnamed client'}
                  </span>
                  <span className="ml-3 text-xs text-slate-500">
                    Added {formatTime(client.createdAt)}
                    {client.lastSeen && <> &middot; Last seen {formatTime(client.lastSeen)}</>}
                  </span>
                </div>
                <button
                  onClick={() => deleteClient.mutate(client.id)}
                  disabled={deleteClient.isPending}
                  className="rounded bg-red-600/20 p-2 text-red-400 hover:bg-red-600/40 transition-colors"
                  title="Revoke client"
                >
                  <Trash2 size={14} />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

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

      {/* Divider */}
      <hr className="border-slate-700" />

      {/* Claim Code / Remote Access */}
      <ClaimCodeSection />
    </div>
  )
}
