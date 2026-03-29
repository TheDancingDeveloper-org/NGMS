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

interface NavSection {
  title: string
  items: NavItem[]
}

const navSections: NavSection[] = [
  {
    title: 'Media',
    items: [
      { to: '/series', icon: Tv, label: 'TV', gate: (m) => m.tvManagement },
      { to: '/movies', icon: Film, label: 'Movies', gate: (m) => m.movieManagement },
      { to: '/discover', icon: Compass, label: 'Discover' },
      { to: '/requests', icon: ListChecks, label: 'Requests' },
      { to: '/search', icon: Search, label: 'Search', gate: (m) => m.externalIndexers || m.indexarrSidecar },
      { to: '/calendar', icon: CalendarDays, label: 'Calendar' },
    ],
  },
  {
    title: 'Downloads',
    items: [
      { to: '/queue', icon: Download, label: 'Queue' },
      { to: '/torrents', icon: Magnet, label: 'Torrents', gate: (m) => m.torrentEmbedded },
      { to: '/usenet', icon: HardDrive, label: 'Usenet', gate: (m) => m.usenetEmbedded },
      { to: '/wanted/missing', icon: AlertCircle, label: 'Wanted' },
    ],
  },
  {
    title: 'Streaming',
    items: [
      { to: '/streaming', icon: Play, label: 'Streaming', gate: (m) => m.streaming || m.plexIntegration },
      { to: '/plex/activity', icon: Activity, label: 'Plex Activity', gate: (m) => m.plexIntegration },
      { to: '/watchlist', icon: Bookmark, label: 'Watchlist', gate: (m) => m.plexIntegration },
    ],
  },
]

const bottomItems: NavItem[] = [
  { to: '/history', icon: Clock, label: 'History' },
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

  const filterItems = (items: NavItem[]) =>
    items.filter((item) => !item.gate || !modules || item.gate(modules))

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
          <span className="text-lg font-semibold text-white tracking-tight">NGMS</span>
        )}
      </div>

      {/* Nav links — grouped by section */}
      <nav className="mt-1 flex flex-1 flex-col gap-0.5 px-2 overflow-y-auto">
        {navSections.map((section) => {
          const visible = filterItems(section.items)
          if (visible.length === 0) return null
          return (
            <div key={section.title}>
              {!collapsed && (
                <div className="px-3 pt-3 pb-1">
                  <span className="text-[10px] font-semibold uppercase tracking-widest text-slate-500">
                    {section.title}
                  </span>
                </div>
              )}
              {collapsed && <div className="mt-2" />}
              {visible.map(({ to, icon: Icon, label }) => (
                <NavLink
                  key={to}
                  to={to}
                  title={label}
                  className={({ isActive }) =>
                    `flex items-center gap-3 rounded-lg px-3 py-2 transition-colors ${
                      isActive
                        ? 'bg-blue-600 text-white'
                        : 'text-slate-400 hover:bg-slate-700 hover:text-white'
                    } ${collapsed ? 'justify-center' : ''}`
                  }
                >
                  <Icon size={20} className="shrink-0" />
                  {!collapsed && <span className="text-sm font-medium">{label}</span>}
                  {!collapsed && to === '/requests' && pendingCount && pendingCount.count > 0 && (
                    <span className="ml-auto rounded-full bg-yellow-600 px-1.5 py-0.5 text-[10px] font-semibold text-white">
                      {pendingCount.count}
                    </span>
                  )}
                </NavLink>
              ))}
            </div>
          )
        })}

        {/* Spacer */}
        <div className="flex-1" />

        {/* Bottom items — History, Users, Settings */}
        <div className="border-t border-slate-700 pt-2 mt-2 mb-2 flex flex-col gap-0.5">
          {filterItems(bottomItems).map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              title={label}
              className={({ isActive }) =>
                `flex items-center gap-3 rounded-lg px-3 py-2 transition-colors ${
                  isActive
                    ? 'bg-blue-600 text-white'
                    : 'text-slate-400 hover:bg-slate-700 hover:text-white'
                } ${collapsed ? 'justify-center' : ''}`
              }
            >
              <Icon size={20} className="shrink-0" />
              {!collapsed && <span className="text-sm font-medium">{label}</span>}
            </NavLink>
          ))}
        </div>
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
