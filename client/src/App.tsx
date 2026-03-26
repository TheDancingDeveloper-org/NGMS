import { Routes, Route, NavLink } from 'react-router-dom'
import { Tv, Film } from 'lucide-react'
import Browse from './pages/Browse'
import SeriesView from './pages/SeriesView'
import MovieView from './pages/MovieView'
import Player from './pages/Player'

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

export default function App() {
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
