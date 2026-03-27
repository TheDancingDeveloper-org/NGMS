import { useState, useEffect } from 'react'
import { Routes, Route, NavLink } from 'react-router-dom'
import { Tv, Film, LogOut } from 'lucide-react'
import Browse from './pages/Browse'
import SeriesView from './pages/SeriesView'
import MovieView from './pages/MovieView'
import Player from './pages/Player'
import ServerConnect from './pages/ServerConnect'
import { getConnection, clearConnection } from './api'

const navStyle = (active: boolean) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  padding: '8px 16px',
  borderRadius: 8,
  fontSize: 14,
  fontWeight: 500,
  textDecoration: 'none',
  color: active ? '#fff' : '#94a3b8',
  background: active ? '#1e40af' : 'transparent',
  transition: 'all 0.15s',
})

type ConnState = 'checking' | 'connected' | 'needs_setup'

export default function App() {
  const [connState, setConnState] = useState<ConnState>('checking')

  useEffect(() => {
    checkConnection()
  }, [])

  async function checkConnection() {
    const conn = getConnection()
    if (conn) {
      // Have a stored connection — verify it's still reachable
      try {
        const res = await fetch(`${conn.serverUrl}/api/v1/system/status`, {
          headers: { Authorization: `Bearer ${conn.clientToken}` },
          signal: AbortSignal.timeout(5000),
        })
        if (res.ok) {
          setConnState('connected')
          return
        }
      } catch {
        // Fall through to needs_setup
      }
      // Stored connection is stale
      clearConnection()
      setConnState('needs_setup')
      return
    }

    // No stored connection — try same-origin (e.g. web deployment behind reverse proxy)
    try {
      const res = await fetch('/api/v1/system/status', {
        signal: AbortSignal.timeout(3000),
      })
      if (res.ok) {
        const ct = res.headers.get('content-type') || ''
        if (ct.includes('application/json')) {
          const data = await res.json()
          if (data && typeof data.version === 'string') {
            setConnState('connected')
            return
          }
        }
      }
    } catch {
      // Not available
    }

    setConnState('needs_setup')
  }

  if (connState === 'checking') {
    return (
      <div style={{
        display: 'flex', justifyContent: 'center', alignItems: 'center',
        minHeight: '100vh', background: '#0f172a', color: '#94a3b8',
      }}>
        Connecting...
      </div>
    )
  }

  if (connState === 'needs_setup') {
    return <ServerConnect onConnected={() => setConnState('connected')} />
  }

  const conn = getConnection()

  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: '100vh' }}>
      {/* Header */}
      <header style={{
        display: 'flex',
        alignItems: 'center',
        gap: 24,
        padding: '12px 24px',
        background: '#1e293b',
        borderBottom: '1px solid #334155',
      }}>
        <span style={{ fontSize: 18, fontWeight: 700, color: '#3b82f6' }}>StackArr Player</span>
        <nav style={{ display: 'flex', gap: 8 }}>
          <NavLink to="/" end style={({ isActive }) => navStyle(isActive)}>
            <Tv size={16} /> Series
          </NavLink>
          <NavLink to="/movies" style={({ isActive }) => navStyle(isActive)}>
            <Film size={16} /> Movies
          </NavLink>
        </nav>
        {conn && (
          <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 12 }}>
            <span style={{ color: '#64748b', fontSize: 12 }}>{conn.serverName}</span>
            <button
              onClick={() => { clearConnection(); setConnState('needs_setup') }}
              style={{
                background: 'none', border: 'none', color: '#64748b',
                cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 4,
                fontSize: 12,
              }}
              title="Disconnect"
            >
              <LogOut size={14} />
            </button>
          </div>
        )}
      </header>

      {/* Content */}
      <main style={{ flex: 1, padding: 24, maxWidth: 1400, width: '100%', margin: '0 auto' }}>
        <Routes>
          <Route path="/" element={<Browse mode="series" />} />
          <Route path="/movies" element={<Browse mode="movies" />} />
          <Route path="/series/:id" element={<SeriesView />} />
          <Route path="/movie/:id" element={<MovieView />} />
          <Route path="/play/:fileId" element={<Player />} />
        </Routes>
      </main>
    </div>
  )
}
