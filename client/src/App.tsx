import { useState, useEffect, useCallback } from 'react'
import { Routes, Route, NavLink, useLocation, useNavigate } from 'react-router-dom'
import { Tv, Film, Home, LogOut, User, Search, ListChecks, Bookmark, Settings, Calendar, Download, Clock } from 'lucide-react'
import Browse from './pages/Browse'
import HomePage from './pages/HomePage'
import SeriesView from './pages/SeriesView'
import MovieView from './pages/MovieView'
import Player from './pages/Player'
import DiscoverPage from './pages/DiscoverPage'
import RequestsPage from './pages/RequestsPage'
import WatchlistPage from './pages/WatchlistPage'
import AccountPage from './pages/AccountPage'
import CalendarPage from './pages/CalendarPage'
import QueuePage from './pages/QueuePage'
import HistoryPage from './pages/HistoryPage'
import ServerConnect from './pages/ServerConnect'
import LoginPage from './pages/LoginPage'
import RegisterPage from './pages/RegisterPage'
import { useAuth } from './context/AuthContext'
import { getConnection, clearConnection } from './api'
import NotificationBell from './components/NotificationBell'
import ActivityIndicator from './components/ActivityIndicator'
import ErrorBoundary from './components/ErrorBoundary'
import { useMobile } from './hooks/useMobile'

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

const mobileTabStyle = (active: boolean): React.CSSProperties => ({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 2,
  padding: '6px 0',
  flex: 1,
  fontSize: 10,
  fontWeight: 500,
  textDecoration: 'none',
  color: active ? '#3b82f6' : '#64748b',
  background: 'none',
  border: 'none',
  transition: 'color 0.15s',
})

type ConnState = 'checking' | 'connected' | 'needs_setup'

export default function App() {
  const [connState, setConnState] = useState<ConnState>('checking')
  const [authPage, setAuthPage] = useState<'login' | 'register'>('login')
  const [pendingInviteCode, setPendingInviteCode] = useState<string | null>(null)
  const { user, loading: authLoading, logout } = useAuth()
  const isMobile = useMobile()
  const location = useLocation()
  const [moreMenuOpen, setMoreMenuOpen] = useState(false)

  const globalNavigate = useNavigate()

  // Global keyboard shortcut: / to jump to search
  const handleGlobalKey = useCallback((e: KeyboardEvent) => {
    // Ignore if typing in an input/textarea
    const tag = (e.target as HTMLElement).tagName
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return
    if (e.key === '/' && !e.ctrlKey && !e.metaKey) {
      e.preventDefault()
      globalNavigate('/discover')
    }
  }, [globalNavigate])

  useEffect(() => {
    document.addEventListener('keydown', handleGlobalKey)
    return () => document.removeEventListener('keydown', handleGlobalKey)
  }, [handleGlobalKey])

  // Hide chrome when player is active
  const isPlayerRoute = location.pathname.startsWith('/play/')

  useEffect(() => {
    checkConnection()
  }, [])

  async function checkConnection() {
    const conn = getConnection()
    if (conn) {
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
        // Fall through
      }
      clearConnection()
      setConnState('needs_setup')
      return
    }

    // Try same-origin
    try {
      const res = await fetch('/api/v1/system/status', {
        credentials: 'include',
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

  if (connState === 'checking' || authLoading) {
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
    return <ServerConnect onConnected={(opts) => {
      setConnState('connected')
      if (opts?.claimType === 'invite' && opts?.inviteCode) {
        setPendingInviteCode(opts.inviteCode)
      }
    }} />
  }

  // If connected but no user session, show login/register
  if (!user) {
    if (pendingInviteCode || authPage === 'register') {
      return (
        <RegisterPage
          onSwitchToLogin={() => { setPendingInviteCode(null); setAuthPage('login') }}
          inviteCode={pendingInviteCode}
        />
      )
    }
    return <LoginPage onSwitchToRegister={() => setAuthPage('register')} />
  }

  const conn = getConnection()

  // Mobile layout
  if (isMobile) {
    return (
      <div style={{
        display: 'flex', flexDirection: 'column', minHeight: '100vh',
        paddingBottom: isPlayerRoute ? 0 : 'calc(56px + env(safe-area-inset-bottom, 0px))',
      }}>
        {/* Mobile top bar - hidden during playback */}
        {!isPlayerRoute && (
          <header style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: 'calc(env(safe-area-inset-top, 8px) + 8px) 16px 8px',
            background: '#1e293b',
            borderBottom: '1px solid #334155',
          }}>
            <img src="/app/images/NGMS_Logo.png" alt="NGMS" style={{ height: 24, width: 24 }} />
            <span style={{ fontSize: 16, fontWeight: 700, color: '#3b82f6' }}>NGMS</span>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <ActivityIndicator />
              <NotificationBell />
              <span style={{ color: '#94a3b8', fontSize: 11 }}>
                {user.displayName}
              </span>
              <button
                onClick={async () => {
                  await logout()
                  if (conn) {
                    clearConnection()
                    setConnState('needs_setup')
                  }
                }}
                style={{
                  background: 'none', border: 'none', color: '#64748b',
                  cursor: 'pointer', display: 'flex', padding: 4,
                }}
                title="Sign out"
              >
                <LogOut size={16} />
              </button>
            </div>
          </header>
        )}

        {/* Content */}
        <main style={{
          flex: 1,
          padding: isPlayerRoute ? 0 : 16,
          overflowY: 'auto',
          WebkitOverflowScrolling: 'touch',
        }}>
          <ErrorBoundary>
            <Routes>
              <Route path="/" element={<HomePage />} />
              <Route path="/series" element={<Browse mode="series" />} />
              <Route path="/movies" element={<Browse mode="movies" />} />
              <Route path="/series/:id" element={<SeriesView />} />
              <Route path="/movie/:id" element={<MovieView />} />
              <Route path="/discover" element={<DiscoverPage />} />
              <Route path="/watchlist" element={<WatchlistPage />} />
              <Route path="/requests" element={<RequestsPage />} />
              <Route path="/account" element={<AccountPage />} />
              <Route path="/calendar" element={<CalendarPage />} />
              <Route path="/queue" element={<QueuePage />} />
              <Route path="/history" element={<HistoryPage />} />
              <Route path="/play/:fileId" element={<Player />} />
              <Route path="*" element={
                <div style={{
                  display: 'flex', flexDirection: 'column', alignItems: 'center',
                  justifyContent: 'center', minHeight: '60vh', color: '#e2e8f0',
                  textAlign: 'center', padding: 24,
                }}>
                  <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 8 }}>Page not found</h1>
                  <p style={{ color: '#94a3b8', marginBottom: 16 }}>The page you're looking for doesn't exist.</p>
                  <a href="/" style={{ color: '#3b82f6', textDecoration: 'none', fontWeight: 500 }}>Go home</a>
                </div>
              } />
            </Routes>
          </ErrorBoundary>
        </main>

        {/* Bottom tab bar - hidden during playback */}
        {!isPlayerRoute && (
          <nav style={{
            position: 'fixed',
            bottom: 0,
            left: 0,
            right: 0,
            display: 'flex',
            background: '#1e293b',
            borderTop: '1px solid #334155',
            paddingBottom: 'env(safe-area-inset-bottom, 0px)',
            zIndex: 100,
          }}>
            <NavLink to="/" end style={({ isActive }) => mobileTabStyle(isActive)}>
              <Home size={20} />
              <span>Home</span>
            </NavLink>
            <NavLink to="/series" style={({ isActive }) => mobileTabStyle(isActive)}>
              <Tv size={20} />
              <span>Series</span>
            </NavLink>
            <NavLink to="/movies" style={({ isActive }) => mobileTabStyle(isActive)}>
              <Film size={20} />
              <span>Movies</span>
            </NavLink>
            <NavLink to="/discover" style={({ isActive }) => mobileTabStyle(isActive)}>
              <Search size={20} />
              <span>Discover</span>
            </NavLink>

            {/* More menu (Watchlist + Requests) */}
            <div style={{ flex: 1, position: 'relative' }}>
              <button
                onClick={() => setMoreMenuOpen(!moreMenuOpen)}
                style={{
                  ...mobileTabStyle(location.pathname === '/watchlist' || location.pathname === '/requests'),
                  width: '100%',
                  cursor: 'pointer',
                }}
              >
                <Bookmark size={20} />
                <span>More</span>
              </button>
              {moreMenuOpen && (
                <>
                  <div
                    style={{ position: 'fixed', inset: 0, zIndex: 99 }}
                    onClick={() => setMoreMenuOpen(false)}
                  />
                  <div style={{
                    position: 'absolute',
                    bottom: '100%',
                    right: 0,
                    marginBottom: 8,
                    background: '#1e293b',
                    border: '1px solid #334155',
                    borderRadius: 10,
                    overflow: 'hidden',
                    boxShadow: '0 -4px 20px rgba(0,0,0,0.4)',
                    zIndex: 100,
                    minWidth: 160,
                  }}>
                    <NavLink
                      to="/watchlist"
                      onClick={() => setMoreMenuOpen(false)}
                      style={({ isActive }) => ({
                        display: 'flex', alignItems: 'center', gap: 10,
                        padding: '12px 16px', textDecoration: 'none',
                        color: isActive ? '#3b82f6' : '#e2e8f0', fontSize: 14,
                      })}
                    >
                      <Bookmark size={16} /> Watchlist
                    </NavLink>
                    <NavLink
                      to="/requests"
                      onClick={() => setMoreMenuOpen(false)}
                      style={({ isActive }) => ({
                        display: 'flex', alignItems: 'center', gap: 10,
                        padding: '12px 16px', textDecoration: 'none',
                        color: isActive ? '#3b82f6' : '#e2e8f0', fontSize: 14,
                        borderTop: '1px solid #334155',
                      })}
                    >
                      <ListChecks size={16} /> Requests
                    </NavLink>
                    <NavLink
                      to="/calendar"
                      onClick={() => setMoreMenuOpen(false)}
                      style={({ isActive }) => ({
                        display: 'flex', alignItems: 'center', gap: 10,
                        padding: '12px 16px', textDecoration: 'none',
                        color: isActive ? '#3b82f6' : '#e2e8f0', fontSize: 14,
                        borderTop: '1px solid #334155',
                      })}
                    >
                      <Calendar size={16} /> Calendar
                    </NavLink>
                    <NavLink
                      to="/queue"
                      onClick={() => setMoreMenuOpen(false)}
                      style={({ isActive }) => ({
                        display: 'flex', alignItems: 'center', gap: 10,
                        padding: '12px 16px', textDecoration: 'none',
                        color: isActive ? '#3b82f6' : '#e2e8f0', fontSize: 14,
                        borderTop: '1px solid #334155',
                      })}
                    >
                      <Download size={16} /> Queue
                    </NavLink>
                    <NavLink
                      to="/history"
                      onClick={() => setMoreMenuOpen(false)}
                      style={({ isActive }) => ({
                        display: 'flex', alignItems: 'center', gap: 10,
                        padding: '12px 16px', textDecoration: 'none',
                        color: isActive ? '#3b82f6' : '#e2e8f0', fontSize: 14,
                        borderTop: '1px solid #334155',
                      })}
                    >
                      <Clock size={16} /> History
                    </NavLink>
                    <NavLink
                      to="/account"
                      onClick={() => setMoreMenuOpen(false)}
                      style={({ isActive }) => ({
                        display: 'flex', alignItems: 'center', gap: 10,
                        padding: '12px 16px', textDecoration: 'none',
                        color: isActive ? '#3b82f6' : '#e2e8f0', fontSize: 14,
                        borderTop: '1px solid #334155',
                      })}
                    >
                      <Settings size={16} /> Account
                    </NavLink>
                  </div>
                </>
              )}
            </div>
          </nav>
        )}
      </div>
    )
  }

  // Desktop layout
  return (
    <div style={{ display: 'flex', flexDirection: 'column', minHeight: '100vh' }}>
      {/* Header - hidden during playback */}
      {!isPlayerRoute && (
        <header style={{
          display: 'flex',
          alignItems: 'center',
          gap: 24,
          padding: '12px 24px',
          background: '#1e293b',
          borderBottom: '1px solid #334155',
        }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <img src="/app/images/NGMS_Logo.png" alt="NGMS" style={{ height: 28, width: 28 }} />
            <span style={{ fontSize: 18, fontWeight: 700, color: '#3b82f6' }}>NGMS</span>
          </div>
          <nav style={{ display: 'flex', gap: 8 }}>
            <NavLink to="/" end style={({ isActive }) => navStyle(isActive)}>
              <Home size={16} /> Home
            </NavLink>
            <NavLink to="/series" style={({ isActive }) => navStyle(isActive)}>
              <Tv size={16} /> Series
            </NavLink>
            <NavLink to="/movies" style={({ isActive }) => navStyle(isActive)}>
              <Film size={16} /> Movies
            </NavLink>
            <NavLink to="/discover" style={({ isActive }) => navStyle(isActive)}>
              <Search size={16} /> Discover
            </NavLink>
            <NavLink to="/calendar" style={({ isActive }) => navStyle(isActive)}>
              <Calendar size={16} /> Calendar
            </NavLink>
            <NavLink to="/queue" style={({ isActive }) => navStyle(isActive)}>
              <Download size={16} /> Queue
            </NavLink>
            <NavLink to="/history" style={({ isActive }) => navStyle(isActive)}>
              <Clock size={16} /> History
            </NavLink>
            <NavLink to="/watchlist" style={({ isActive }) => navStyle(isActive)}>
              <Bookmark size={16} /> Watchlist
            </NavLink>
            <NavLink to="/requests" style={({ isActive }) => navStyle(isActive)}>
              <ListChecks size={16} /> Requests
            </NavLink>
          </nav>
          <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 12 }}>
            <ActivityIndicator />
            <NotificationBell />
            <NavLink to="/account" style={({ isActive }) => ({
              color: isActive ? '#3b82f6' : '#94a3b8', fontSize: 12,
              display: 'flex', alignItems: 'center', gap: 4, textDecoration: 'none',
            })}>
              <User size={14} />
              {user.displayName}
            </NavLink>
            {conn && (
              <span style={{ color: '#64748b', fontSize: 12 }}>{conn.serverName}</span>
            )}
            <button
              onClick={async () => {
                await logout()
                if (conn) {
                  clearConnection()
                  setConnState('needs_setup')
                }
              }}
              style={{
                background: 'none', border: 'none', color: '#64748b',
                cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 4,
                fontSize: 12,
              }}
              title="Sign out"
            >
              <LogOut size={14} /> Sign out
            </button>
          </div>
        </header>
      )}

      {/* Content */}
      <main style={{
        flex: 1,
        ...(isPlayerRoute
          ? { padding: 0 }
          : { padding: 24, maxWidth: 1400, width: '100%', margin: '0 auto' }),
      }}>
        <ErrorBoundary>
          <Routes>
            <Route path="/" element={<HomePage />} />
            <Route path="/series" element={<Browse mode="series" />} />
            <Route path="/movies" element={<Browse mode="movies" />} />
            <Route path="/series/:id" element={<SeriesView />} />
            <Route path="/movie/:id" element={<MovieView />} />
            <Route path="/discover" element={<DiscoverPage />} />
            <Route path="/watchlist" element={<WatchlistPage />} />
            <Route path="/requests" element={<RequestsPage />} />
            <Route path="/play/:fileId" element={<Player />} />
            <Route path="*" element={
              <div style={{
                display: 'flex', flexDirection: 'column', alignItems: 'center',
                justifyContent: 'center', minHeight: '60vh', color: '#e2e8f0',
                textAlign: 'center', padding: 24,
              }}>
                <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 8 }}>Page not found</h1>
                <p style={{ color: '#94a3b8', marginBottom: 16 }}>The page you're looking for doesn't exist.</p>
                <a href="/" style={{ color: '#3b82f6', textDecoration: 'none', fontWeight: 500 }}>Go home</a>
              </div>
            } />
          </Routes>
        </ErrorBoundary>
      </main>
    </div>
  )
}
