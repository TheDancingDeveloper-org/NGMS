// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { useState } from 'react'
import { useAuth } from '../hooks/useAuth'
import { labelStyle, inputStyle, buttonStyleDisabled } from '../styles/forms'
import { assetUrl, getConnection } from '../api'

export default function LoginPage({
  onSwitchToRegister,
  onSwitchServer,
}: {
  onSwitchToRegister: () => void
  onSwitchServer: () => void
}) {
  const { login } = useAuth()
  const conn = getConnection()
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
        <img src={assetUrl('images/NGMS_Banner.png')} alt="NGMS" style={{ height: 48, marginBottom: 8 }} />
        <p style={{ color: '#94a3b8', fontSize: 14, marginBottom: 4 }}>
          Signing in to{' '}
          <strong style={{ color: '#e2e8f0' }}>
            {conn?.serverName || 'this server'}
          </strong>
        </p>
        <p style={{ fontSize: 12, marginBottom: 24 }}>
          <button
            type="button"
            onClick={onSwitchServer}
            style={{
              color: '#3b82f6', background: 'none', border: 'none',
              cursor: 'pointer', fontSize: 12, padding: 0,
            }}
          >
            Switch server
          </button>
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

        <button type="submit" disabled={loading} style={buttonStyleDisabled(loading)}>
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
