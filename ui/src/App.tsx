import { useState, useEffect, useCallback } from 'react'
import { Routes, Route, Navigate } from 'react-router-dom'
import { useSystemStatus, useCurrentUser } from './hooks/useApi'
import { getConnection, clearConnection, apiLogout } from './api/client'
import Layout from './components/Layout'
import FirstBoot from './pages/FirstBoot'
import Login from './pages/Login'
import SeriesList from './pages/SeriesList'
import SeriesDetail from './pages/SeriesDetail'
import MovieList from './pages/MovieList'
import MovieDetail from './pages/MovieDetail'
import Calendar from './pages/Calendar'
import Queue from './pages/Queue'
import History from './pages/History'
import Wanted from './pages/Wanted'
import Settings from './pages/Settings'
import Torrents from './pages/Torrents'
import Usenet from './pages/Usenet'
import Player from './pages/Player'
import Streaming from './pages/Streaming'
import Discover from './pages/Discover'
import Watchlist from './pages/Watchlist'
import Search from './pages/Search'
import Users from './pages/Users'
import Requests from './pages/Requests'
import PlexActivity from './pages/PlexActivity'
import Logs from './pages/Logs'
import Rss from './pages/Rss'
import FileBrowser from './pages/FileBrowser'
import ServerConnect from './pages/ServerConnect'

export default function App() {
  const [showConnect, setShowConnect] = useState(false)
  const [forceLogin, setForceLogin] = useState(false)
  const { data: status, isLoading, error, refetch } = useSystemStatus()
  const { data: currentUser, isLoading: userLoading, error: userError, refetch: refetchUser } = useCurrentUser()

  // Listen for 401 events from apiFetch
  const handleUnauthorized = useCallback(() => {
    setForceLogin(true)
  }, [])

  useEffect(() => {
    window.addEventListener('stackarr:unauthorized', handleUnauthorized)
    return () => window.removeEventListener('stackarr:unauthorized', handleUnauthorized)
  }, [handleUnauthorized])

  const handleLogout = async () => {
    await apiLogout()
    setForceLogin(true)
  }

  // Show a minimal loading state while checking system status
  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-slate-900">
        <div className="flex flex-col items-center gap-3">
          <div className="h-10 w-10 animate-spin rounded-full border-4 border-slate-600 border-t-blue-500" />
          <span className="text-sm text-slate-400">Loading NGMS...</span>
        </div>
      </div>
    )
  }

  // If the API is unreachable, offer ServerConnect or retry
  if (error || showConnect) {
    if (showConnect) {
      return (
        <ServerConnect
          onConnected={() => {
            setShowConnect(false)
            refetch()
          }}
        />
      )
    }
    return (
      <div className="flex min-h-screen items-center justify-center bg-slate-900">
        <div className="flex flex-col items-center gap-3">
          <span className="text-sm text-slate-400">Unable to connect to NGMS</span>
          <div className="flex gap-2">
            <button
              onClick={() => refetch()}
              className="rounded bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-500"
            >
              Retry
            </button>
            <button
              onClick={() => setShowConnect(true)}
              className="rounded bg-slate-700 px-4 py-2 text-sm text-slate-300 hover:bg-slate-600"
            >
              Connect to Server
            </button>
          </div>
          {getConnection() && (
            <button
              onClick={() => { clearConnection(); refetch() }}
              className="mt-2 text-xs text-slate-500 hover:text-slate-400"
            >
              Clear saved connection
            </button>
          )}
        </div>
      </div>
    )
  }

  const firstBoot = status?.firstBoot === true

  if (firstBoot) {
    return (
      <Routes>
        <Route path="/setup" element={<FirstBoot />} />
        <Route path="*" element={<Navigate to="/setup" replace />} />
      </Routes>
    )
  }

  // For "forms" auth: require login if no valid session
  const authMethod = status?.authMethod ?? 'none'
  if (authMethod === 'forms') {
    // Show loading while checking auth
    if (userLoading) {
      return (
        <div className="flex min-h-screen items-center justify-center bg-slate-900">
          <div className="flex flex-col items-center gap-3">
            <div className="h-10 w-10 animate-spin rounded-full border-4 border-slate-600 border-t-blue-500" />
            <span className="text-sm text-slate-400">Checking authentication...</span>
          </div>
        </div>
      )
    }

    // Show login page if not authenticated
    if (forceLogin || userError || !currentUser) {
      return (
        <Login
          instanceName={status?.instanceName}
          onLoggedIn={() => {
            setForceLogin(false)
            refetchUser()
            refetch()
          }}
        />
      )
    }
  }

  return (
    <Routes>
      {/* Main app layout */}
      <Route element={<Layout onLogout={authMethod !== 'none' ? handleLogout : undefined} />}>
        <Route path="/discover" element={<Discover />} />
        <Route path="/series" element={<SeriesList />} />
        <Route path="/series/:id" element={<SeriesDetail />} />
        <Route path="/movies" element={<MovieList />} />
        <Route path="/movies/:id" element={<MovieDetail />} />
        <Route path="/calendar" element={<Calendar />} />
        <Route path="/search" element={<Search />} />
        <Route path="/queue" element={<Queue />} />
        <Route path="/torrents" element={<Torrents />} />
        <Route path="/usenet" element={<Usenet />} />
        <Route path="/history" element={<History />} />
        <Route path="/logs" element={<Logs />} />
        <Route path="/wanted/missing" element={<Wanted />} />
        <Route path="/rss" element={<Rss />} />
        <Route path="/filebrowser" element={<FileBrowser />} />
        <Route path="/watchlist" element={<Watchlist />} />
        <Route path="/play/:mediaFileId" element={<Player />} />
        <Route path="/streaming" element={<Streaming />} />
        <Route path="/plex/activity" element={<PlexActivity />} />
        <Route path="/requests" element={<Requests />} />
        <Route path="/users" element={<Users />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/migrate" element={<Navigate to="/settings" replace />} />

        {/* Default redirect */}
        <Route path="/" element={<Navigate to="/discover" replace />} />
        <Route path="*" element={<Navigate to="/discover" replace />} />
      </Route>
    </Routes>
  )
}
