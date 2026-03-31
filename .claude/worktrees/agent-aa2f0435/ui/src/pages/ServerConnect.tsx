import { useState } from 'react'
import { redeemClaimCode, saveConnection, type ServerConnection } from '../api/client'

const DEFAULT_BOOTSTRAP = 'https://streambootstrap.indexarr.net'

export default function ServerConnect({ onConnected }: { onConnected: () => void }) {
  const [mode, setMode] = useState<'claim' | 'direct'>('claim')
  const [code, setCode] = useState('')
  const [clientName, setClientName] = useState('')
  const [bootstrapUrl, setBootstrapUrl] = useState(DEFAULT_BOOTSTRAP)
  const [directUrl, setDirectUrl] = useState('')
  const [directToken, setDirectToken] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleClaim() {
    if (!code.trim() || !clientName.trim()) {
      setError('Please enter both a claim code and your name')
      return
    }
    setError(null)
    setLoading(true)
    try {
      await redeemClaimCode(code.trim(), clientName.trim(), bootstrapUrl)
      onConnected()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to connect')
    } finally {
      setLoading(false)
    }
  }

  async function handleDirect() {
    if (!directUrl.trim() || !directToken.trim()) {
      setError('Please enter both the server URL and token')
      return
    }
    setError(null)
    setLoading(true)
    try {
      const url = directUrl.trim().replace(/\/$/, '')
      const res = await fetch(`${url}/api/v1/system/status`, {
        headers: { Authorization: `Bearer ${directToken.trim()}` },
        signal: AbortSignal.timeout(5000),
      })
      if (!res.ok) throw new Error(`Server returned ${res.status}`)
      const status = await res.json()
      const conn: ServerConnection = {
        serverUrl: url,
        serverName: status.instanceName || 'StackArr',
        serverId: '',
        clientToken: directToken.trim(),
      }
      saveConnection(conn)
      onConnected()
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to connect')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-slate-900 p-6">
      <div className="w-full max-w-md rounded-xl border border-slate-700 bg-slate-800 p-8">
        <h1 className="mb-2 text-2xl font-bold text-blue-500">StackArr</h1>
        <p className="mb-6 text-sm text-slate-400">Connect to your StackArr server</p>

        {/* Mode toggle */}
        <div className="mb-5 flex gap-2">
          {(['claim', 'direct'] as const).map((m) => (
            <button
              key={m}
              onClick={() => { setMode(m); setError(null) }}
              className={`flex-1 rounded-md px-3 py-2 text-sm font-medium ${
                mode === m
                  ? 'bg-blue-600 text-white'
                  : 'bg-slate-700 text-slate-400 hover:text-slate-300'
              }`}
            >
              {m === 'claim' ? 'Claim Code' : 'Direct URL'}
            </button>
          ))}
        </div>

        {mode === 'claim' ? (
          <>
            <label className="mb-1 block text-xs font-medium text-slate-400">Your Name</label>
            <input
              value={clientName}
              onChange={(e) => setClientName(e.target.value)}
              placeholder="e.g. John's iPad"
              className="mb-3 w-full rounded-md border border-slate-600 bg-slate-900 px-3 py-2.5 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
            />
            <label className="mb-1 block text-xs font-medium text-slate-400">Claim Code</label>
            <input
              value={code}
              onChange={(e) => setCode(e.target.value.toUpperCase())}
              placeholder="A7X9"
              maxLength={4}
              className="mb-3 w-full rounded-md border border-slate-600 bg-slate-900 px-3 py-2.5 text-center text-2xl tracking-[0.5em] text-slate-200 focus:border-blue-500 focus:outline-none"
            />
            <details className="mb-4">
              <summary className="cursor-pointer text-xs text-slate-500">Advanced</summary>
              <label className="mb-1 mt-2 block text-xs font-medium text-slate-400">
                Bootstrap URL
              </label>
              <input
                value={bootstrapUrl}
                onChange={(e) => setBootstrapUrl(e.target.value)}
                className="w-full rounded-md border border-slate-600 bg-slate-900 px-3 py-2 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
              />
            </details>
            <button
              onClick={handleClaim}
              disabled={loading}
              className="w-full rounded-lg bg-blue-600 px-4 py-3 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-50"
            >
              {loading ? 'Connecting...' : 'Connect'}
            </button>
          </>
        ) : (
          <>
            <label className="mb-1 block text-xs font-medium text-slate-400">Server URL</label>
            <input
              value={directUrl}
              onChange={(e) => setDirectUrl(e.target.value)}
              placeholder="http://192.168.1.100:8989"
              className="mb-3 w-full rounded-md border border-slate-600 bg-slate-900 px-3 py-2.5 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
            />
            <label className="mb-1 block text-xs font-medium text-slate-400">
              API Key or Client Token
            </label>
            <input
              value={directToken}
              onChange={(e) => setDirectToken(e.target.value)}
              placeholder="Your API key or token"
              type="password"
              className="mb-4 w-full rounded-md border border-slate-600 bg-slate-900 px-3 py-2.5 text-sm text-slate-200 focus:border-blue-500 focus:outline-none"
            />
            <button
              onClick={handleDirect}
              disabled={loading}
              className="w-full rounded-lg bg-blue-600 px-4 py-3 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-50"
            >
              {loading ? 'Connecting...' : 'Connect'}
            </button>
          </>
        )}

        {error && <p className="mt-3 text-sm text-red-400">{error}</p>}
      </div>
    </div>
  )
}
