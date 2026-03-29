import { useState } from 'react'
import { useAuth } from '../context/AuthContext'
import { labelStyle, inputStyle, buttonStyleDisabled } from '../styles/forms'

export default function RegisterPage({
  onSwitchToLogin,
  inviteCode: prefilledInviteCode,
}: {
  onSwitchToLogin: () => void
  inviteCode?: string | null
}) {
  const { register } = useAuth()
  const [username, setUsername] = useState('')
  const [displayName, setDisplayName] = useState('')
  const [password, setPassword] = useState('')
  const [inviteCode, setInviteCode] = useState(prefilledInviteCode ?? '')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!username.trim() || !password || !inviteCode.trim()) {
      setError('Please fill in all required fields')
      return
    }
    if (password.length < 6) {
      setError('Password must be at least 6 characters')
      return
    }
    setError(null)
    setLoading(true)
    try {
      await register(
        username.trim(),
        password,
        displayName.trim() || username.trim(),
        inviteCode.trim(),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Registration failed')
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
        <img src="/app/images/NGMS_Banner.png" alt="NGMS" style={{ height: 48, marginBottom: 8 }} />
        <p style={{ color: '#94a3b8', fontSize: 14, marginBottom: 24 }}>
          Create your account
        </p>

        <label style={labelStyle}>Invite Code *</label>
        {prefilledInviteCode ? (
          <>
            <input
              value={inviteCode}
              readOnly
              style={{ ...inputStyle, letterSpacing: 2, textAlign: 'center', opacity: 0.7 }}
            />
            <p style={{ color: '#22c55e', fontSize: 12, marginTop: -8, marginBottom: 12 }}>
              Your invite code has been verified via server discovery
            </p>
          </>
        ) : (
          <input
            value={inviteCode}
            onChange={(e) => setInviteCode(e.target.value.toUpperCase())}
            placeholder="ABCD1234"
            maxLength={8}
            style={{ ...inputStyle, letterSpacing: 2, textAlign: 'center' }}
          />
        )}

        <label style={labelStyle}>Username *</label>
        <input
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          placeholder="username"
          autoComplete="username"
          style={inputStyle}
        />

        <label style={labelStyle}>Display Name</label>
        <input
          value={displayName}
          onChange={(e) => setDisplayName(e.target.value)}
          placeholder="Your name (optional)"
          style={inputStyle}
        />

        <label style={labelStyle}>Password *</label>
        <input
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          placeholder="minimum 6 characters"
          autoComplete="new-password"
          style={inputStyle}
        />

        <button type="submit" disabled={loading} style={buttonStyleDisabled(loading)}>
          {loading ? 'Creating account...' : 'Create Account'}
        </button>

        {error && (
          <p style={{ color: '#ef4444', fontSize: 13, marginTop: 12 }}>{error}</p>
        )}

        <p style={{ color: '#64748b', fontSize: 13, marginTop: 16, textAlign: 'center' }}>
          Already have an account?{' '}
          <button
            type="button"
            onClick={onSwitchToLogin}
            style={{ color: '#3b82f6', background: 'none', border: 'none', cursor: 'pointer', fontSize: 13 }}
          >
            Sign in
          </button>
        </p>
      </form>
    </div>
  )
}
