import { NavLink } from 'react-router-dom'
import {
  Tv,
  Film,
  CalendarDays,
  Download,
  Clock,
  AlertCircle,
  Settings,
  ChevronLeft,
  ChevronRight,
  Magnet,
  HardDrive,
  Play,
  Compass,
  Bookmark,
  Search,
  Users,
  ListChecks,
  Activity,
} from 'lucide-react'
import { useSystemStatus, usePendingRequestCount } from '../hooks/useApi'
import type { EnabledModules } from '../api/types'
import type { LucideIcon } from 'lucide-react'

interface NavItem {
  to: string
  icon: LucideIcon
  label: string
  gate?: (m: EnabledModules) => boolean
}

const navItems: NavItem[] = [
  { to: '/discover', icon: Compass, label: 'Discover' },
  { to: '/series', icon: Tv, label: 'Series', gate: (m) => m.tvManagement },
  { to: '/movies', icon: Film, label: 'Movies', gate: (m) => m.movieManagement },
  { to: '/search', icon: Search, label: 'Search', gate: (m) => m.externalIndexers || m.indexarrSidecar },
  { to: '/calendar', icon: CalendarDays, label: 'Calendar' },
  { to: '/queue', icon: Download, label: 'Queue' },
  { to: '/torrents', icon: Magnet, label: 'Torrents', gate: (m) => m.torrentEmbedded },
  { to: '/usenet', icon: HardDrive, label: 'Usenet', gate: (m) => m.usenetEmbedded },
  { to: '/streaming', icon: Play, label: 'Streaming', gate: (m) => m.streaming || m.plexIntegration },
  { to: '/plex/activity', icon: Activity, label: 'Plex Activity', gate: (m) => m.plexIntegration },
  { to: '/history', icon: Clock, label: 'History' },
  { to: '/wanted/missing', icon: AlertCircle, label: 'Wanted' },
  { to: '/watchlist', icon: Bookmark, label: 'Watchlist', gate: (m) => m.plexIntegration },
  { to: '/requests', icon: ListChecks, label: 'Requests' },
  { to: '/users', icon: Users, label: 'Users' },
  { to: '/settings', icon: Settings, label: 'Settings' },
]

interface SidebarProps {
  collapsed: boolean
  onToggle: () => void
}

export default function Sidebar({ collapsed, onToggle }: SidebarProps) {
  const { data: status } = useSystemStatus()
  const { data: pendingCount } = usePendingRequestCount()
  const modules = status?.modules

  const visibleItems = navItems.filter((item) => !item.gate || !modules || item.gate(modules))

  return (
    <aside
      className={`fixed top-0 left-0 z-30 flex h-full flex-col bg-slate-800 transition-all duration-200 ${
        collapsed ? 'w-16' : 'w-56'
      }`}
    >
      {/* Brand */}
      <div className="flex h-14 items-center gap-2 border-b border-slate-700 px-4">
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-blue-600 font-bold text-white text-sm">
          S
        </div>
        {!collapsed && (
          <span className="text-lg font-semibold text-white tracking-tight">StackArr</span>
        )}
      </div>

      {/* Nav links */}
      <nav className="mt-2 flex flex-1 flex-col gap-1 px-2">
        {visibleItems.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            title={label}
            className={({ isActive }) =>
              `flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors ${
                isActive
                  ? 'bg-blue-600 text-white'
                  : 'text-slate-400 hover:bg-slate-700 hover:text-white'
              } ${collapsed ? 'justify-center' : ''}`
            }
          >
            <Icon size={20} className="shrink-0" />
            {!collapsed && <span className="text-sm font-medium">{label}</span>}
            {!collapsed && to === '/requests' && pendingCount && pendingCount.count > 0 && (
              <span className="ml-auto rounded-full bg-yellow-600 px-1.5 py-0.5 text-xs font-semibold text-white">
                {pendingCount.count}
              </span>
            )}
          </NavLink>
        ))}
      </nav>

      {/* Collapse toggle */}
      <button
        onClick={onToggle}
        className="flex items-center justify-center border-t border-slate-700 py-3 text-slate-400 hover:text-white transition-colors"
        title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      >
        {collapsed ? <ChevronRight size={18} /> : <ChevronLeft size={18} />}
      </button>
    </aside>
  )
}
