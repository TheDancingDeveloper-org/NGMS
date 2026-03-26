import { Routes, Route, Navigate } from 'react-router-dom'
import { useSystemStatus } from './hooks/useApi'
import Layout from './components/Layout'
import FirstBoot from './pages/FirstBoot'
import SeriesList from './pages/SeriesList'
import SeriesDetail from './pages/SeriesDetail'
import MovieList from './pages/MovieList'
import MovieDetail from './pages/MovieDetail'
import Calendar from './pages/Calendar'
import Queue from './pages/Queue'
import History from './pages/History'
import Wanted from './pages/Wanted'
import Settings from './pages/Settings'
import Migrate from './pages/Migrate'
import Torrents from './pages/Torrents'
import Usenet from './pages/Usenet'

export default function App() {
  const { data: status, isLoading, error } = useSystemStatus()

  // Show a minimal loading state while checking system status
  if (isLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-slate-900">
        <div className="flex flex-col items-center gap-3">
          <div className="h-10 w-10 animate-spin rounded-full border-4 border-slate-600 border-t-blue-500" />
          <span className="text-sm text-slate-400">Loading StackArr...</span>
        </div>
      </div>
    )
  }

  // If the API is unreachable, show the app anyway (routes will handle individual errors)
  const firstBoot = !error && status?.firstBoot === true

  return (
    <Routes>
      {/* Setup wizard — no sidebar layout */}
      <Route path="/setup" element={<FirstBoot />} />

      {/* Redirect to setup if first boot */}
      {firstBoot && <Route path="*" element={<Navigate to="/setup" replace />} />}

      {/* Main app layout */}
      <Route element={<Layout />}>
        <Route path="/series" element={<SeriesList />} />
        <Route path="/series/:id" element={<SeriesDetail />} />
        <Route path="/movies" element={<MovieList />} />
        <Route path="/movies/:id" element={<MovieDetail />} />
        <Route path="/calendar" element={<Calendar />} />
        <Route path="/queue" element={<Queue />} />
        <Route path="/torrents" element={<Torrents />} />
        <Route path="/usenet" element={<Usenet />} />
        <Route path="/history" element={<History />} />
        <Route path="/wanted/missing" element={<Wanted />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/migrate" element={<Migrate />} />

        {/* Default redirect */}
        <Route path="/" element={<Navigate to="/series" replace />} />
        <Route path="*" element={<Navigate to="/series" replace />} />
      </Route>
    </Routes>
  )
}
