import { useState } from 'react'
import { redeemClaimCode, saveConnection, type ServerConnection } from '../api'

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
    <div style={{
      display: 'flex', justifyContent: 'center', alignItems: 'center',
      minHeight: '100vh', background: '#0f172a', padding: 24,
    }}>
      <div style={{
        background: '#1e293b', borderRadius: 12, padding: 32,
        maxWidth: 420, width: '100%', border: '1px solid #334155',
      }}>
        <h1 style={{ fontSize: 24, fontWeight: 700, color: '#3b82f6', marginBottom: 8 }}>
          StackArr Player
        </h1>
        <p style={{ color: '#94a3b8', fontSize: 14, marginBottom: 24 }}>
          Connect to your StackArr server
        </p>

        {/* Mode toggle */}
        <div style={{ display: 'flex', gap: 8, marginBottom: 20 }}>
          {(['claim', 'direct'] as const).map((m) => (
            <button
              key={m}
              onClick={() => { setMode(m); setError(null) }}
              style={{
                flex: 1, padding: '8px 12px', borderRadius: 6, border: 'none',
                cursor: 'pointer', fontSize: 13, fontWeight: 500,
                background: mode === m ? '#3b82f6' : '#334155',
                color: mode === m ? '#fff' : '#94a3b8',
              }}
            >
              {m === 'claim' ? 'Claim Code' : 'Direct URL'}
            </button>
          ))}
        </div>

        {mode === 'claim' ? (
          <>
            <label style={labelStyle}>Your Name</label>
            <input
              value={clientName}
              onChange={(e) => setClientName(e.target.value)}
              placeholder="e.g. John's iPad"
              style={inputStyle}
            />
            <label style={labelStyle}>Claim Code</label>
            <input
              value={code}
              onChange={(e) => setCode(e.target.value.toUpperCase())}
              placeholder="A7X9"
              maxLength={4}
              style={{ ...inputStyle, fontSize: 24, letterSpacing: 8, textAlign: 'center' }}
            />
            <details style={{ marginBottom: 16 }}>
              <summary style={{ color: '#64748b', fontSize: 12, cursor: 'pointer' }}>
                Advanced
              </summary>
              <label style={{ ...labelStyle, marginTop: 8 }}>Bootstrap URL</label>
              <input
                value={bootstrapUrl}
                onChange={(e) => setBootstrapUrl(e.target.value)}
                style={inputStyle}
              />
            </details>
            <button onClick={handleClaim} disabled={loading} style={buttonStyle}>
              {loading ? 'Connecting...' : 'Connect'}
            </button>
          </>
        ) : (
          <>
            <label style={labelStyle}>Server URL</label>
            <input
              value={directUrl}
              onChange={(e) => setDirectUrl(e.target.value)}
              placeholder="http://192.168.1.100:8989"
              style={inputStyle}
            />
            <label style={labelStyle}>API Key or Client Token</label>
            <input
              value={directToken}
              onChange={(e) => setDirectToken(e.target.value)}
              placeholder="Your API key or token"
              type="password"
              style={inputStyle}
            />
            <button onClick={handleDirect} disabled={loading} style={buttonStyle}>
              {loading ? 'Connecting...' : 'Connect'}
            </button>
          </>
        )}

        {error && (
          <p style={{ color: '#ef4444', fontSize: 13, marginTop: 12 }}>{error}</p>
        )}
      </div>
    </div>
  )
}

const labelStyle: React.CSSProperties = {
  display: 'block', color: '#94a3b8', fontSize: 12,
  fontWeight: 500, marginBottom: 4,
}

const inputStyle: React.CSSProperties = {
  width: '100%', padding: '10px 12px', borderRadius: 6,
  border: '1px solid #475569', background: '#0f172a',
  color: '#e2e8f0', fontSize: 14, marginBottom: 12,
  boxSizing: 'border-box',
}

const buttonStyle: React.CSSProperties = {
  width: '100%', padding: '12px 16px', borderRadius: 8,
  border: 'none', background: '#3b82f6', color: '#fff',
  fontSize: 15, fontWeight: 600, cursor: 'pointer',
}
