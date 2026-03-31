import { useState } from 'react'
import { useAuth } from '../context/AuthContext'

export default function LoginPage({ onSwitchToRegister }: { onSwitchToRegister: () => void }) {
  const { login } = useAuth()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!username.trim() || !password) {
      setError('Please enter both username and password')
      return
    }
    setError(null)
    setLoading(true)
    try {
      await login(username.trim(), password)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div style={{
      display: 'flex', justifyContent: 'center', alignItems: 'center',
      minHeight: '100vh', background: '#0f172a', padding: 24,
    }}>
      <form onSubmit={handleSubmit} style={{
        background: '#1e293b', borderRadius: 12, padding: 32,
        maxWidth: 400, width: '100%', border: '1px solid #334155',
      }}>
        <h1 style={{ fontSize: 24, fontWeight: 700, color: '#3b82f6', marginBottom: 8 }}>
          StackArr
        </h1>
        <p style={{ color: '#94a3b8', fontSize: 14, marginBottom: 24 }}>
          Sign in to your account
        </p>

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

        <button type="submit" disabled={loading} style={buttonStyle}>
          {loading ? 'Signing in...' : 'Sign In'}
        </button>

        {error && (
          <p style={{ color: '#ef4444', fontSize: 13, marginTop: 12 }}>{error}</p>
        )}

        <p style={{ color: '#64748b', fontSize: 13, marginTop: 16, textAlign: 'center' }}>
          Have an invite code?{' '}
          <button
            type="button"
            onClick={onSwitchToRegister}
            style={{ color: '#3b82f6', background: 'none', border: 'none', cursor: 'pointer', fontSize: 13 }}
          >
            Create account
          </button>
        </p>
      </form>
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
