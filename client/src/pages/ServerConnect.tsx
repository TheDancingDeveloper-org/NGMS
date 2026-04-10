import { useState } from 'react'
import { redeemClaimCode, saveConnection, type ServerConnection } from '../api'
import { labelStyle, inputStyle, buttonStyleDisabled } from '../styles/forms'

const DEFAULT_BOOTSTRAP = 'https://streambootstrap.indexarr.net'

interface ConnectedOpts {
  claimType?: string
  inviteCode?: string
}

export default function ServerConnect({ onConnected }: { onConnected: (opts?: ConnectedOpts) => void }) {
  const [mode, setMode] = useState<'claim' | 'login' | 'direct'>('claim')
  const [code, setCode] = useState('')
  const [bootstrapUrl, setBootstrapUrl] = useState(DEFAULT_BOOTSTRAP)
  const [directUrl, setDirectUrl] = useState('')
  const [directToken, setDirectToken] = useState('')
  const [serverName, setServerName] = useState('')
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleClaim() {
    if (!code.trim()) {
      setError('Please enter an invite code')
      return
    }
    setError(null)
    setLoading(true)
    try {
      const result = await redeemClaimCode(code.trim(), bootstrapUrl)
      onConnected({ claimType: result.claimType, inviteCode: result.inviteCode })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to connect')
    } finally {
      setLoading(false)
    }
  }

  async function handleLogin() {
    if (!serverName.trim() || !username.trim() || !password) {
      setError('Please enter server name, username, and password')
      return
    }
    setError(null)
    setLoading(true)
    try {
      // Resolve server name via bootstrap
      const lookupRes = await fetch(
        `${bootstrapUrl}/api/v1/servers/by-name/${encodeURIComponent(serverName.trim())}`,
        { signal: AbortSignal.timeout(5000) },
      )
      if (lookupRes.status === 404) throw new Error('Server not found')
      if (lookupRes.status === 503) throw new Error('Server is currently offline')
      if (!lookupRes.ok) throw new Error(`Bootstrap error: ${lookupRes.status}`)
      const data = await lookupRes.json()

      // Probe order: LAN IPs, then public IP, then relay URL (HTTPS fallback)
      const urls = [
        ...data.localIps.map((ip: string) => `http://${ip}:${data.port}`),
        `http://${data.publicIp}:${data.port}`,
        ...(data.relayUrl ? [data.relayUrl] : []),
      ]

      let serverUrl: string | null = null
      for (const url of urls) {
        try {
          const probe = await fetch(`${url}/api/v1/system/status`, {
            signal: AbortSignal.timeout(5000),
          })
          if (probe.ok) {
            serverUrl = url
            break
          }
        } catch { /* try next */ }
      }

      if (!serverUrl) throw new Error('Server found but unreachable. Check your network/firewall.')

      // Direct HTTPS streaming URL (wildcard cert), if bootstrap provided one
      const streamUrl = data.tlsDomain
        ? `https://${data.tlsDomain}:9443`
        : undefined

      // Authenticate with the server
      const loginRes = await fetch(`${serverUrl}/api/v1/auth/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        credentials: 'include',
        body: JSON.stringify({ username: username.trim(), password }),
      })
      if (loginRes.status === 401) throw new Error('Invalid username or password')
      if (!loginRes.ok) throw new Error(`Login failed: ${loginRes.status}`)

      const conn: ServerConnection = {
        serverUrl,
        streamUrl,
        serverName: data.serverName || serverName.trim(),
        serverId: data.serverId || '',
      }
      saveConnection(conn)
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
        serverName: status.instanceName || 'NGMS',
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
        <img src="/app/images/NGMS_Banner.png" alt="NGMS" style={{ height: 48, marginBottom: 8 }} />
        <p style={{ color: '#94a3b8', fontSize: 14, marginBottom: 24 }}>
          Connect to your NGMS server
        </p>

        {/* Mode toggle */}
        <div style={{ display: 'flex', gap: 8, marginBottom: 20 }}>
          {(['claim', 'login', 'direct'] as const).map((m) => (
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
              {m === 'claim' ? 'Invite Code' : m === 'login' ? 'Sign In' : 'Direct URL'}
            </button>
          ))}
        </div>

        {mode === 'claim' ? (
          <>
            <label style={labelStyle}>Invite Code</label>
            <input
              value={code}
              onChange={(e) => setCode(e.target.value.toUpperCase())}
              placeholder="ABC12DEF"
              maxLength={8}
              style={{ ...inputStyle, fontSize: 24, letterSpacing: 4, textAlign: 'center' }}
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
            <button onClick={handleClaim} disabled={loading} style={buttonStyleDisabled(loading)}>
              {loading ? 'Connecting...' : 'Connect'}
            </button>
          </>
        ) : mode === 'login' ? (
          <>
            <label style={labelStyle}>Server Name</label>
            <input
              value={serverName}
              onChange={(e) => setServerName(e.target.value)}
              placeholder="e.g. MyNGMS"
              style={inputStyle}
            />
            <label style={labelStyle}>Username</label>
            <input
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="username"
              autoComplete="username"
              style={inputStyle}
            />
            <label style={labelStyle}>Password</label>
            <input
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="password"
              autoComplete="current-password"
              style={inputStyle}
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
            <button onClick={handleLogin} disabled={loading} style={buttonStyleDisabled(loading)}>
              {loading ? 'Connecting...' : 'Sign In'}
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
            <button onClick={handleDirect} disabled={loading} style={buttonStyleDisabled(loading)}>
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
