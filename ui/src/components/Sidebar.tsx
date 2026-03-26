import { NavLink } from 'react-router-dom'
import {
  Tv,
  Film,
  CalendarDays,
  Download,
  Clock,
  AlertCircle,
  Settings,
  Database,
  ChevronLeft,
  ChevronRight,
  Magnet,
  HardDrive,
} from 'lucide-react'

const navItems = [
  { to: '/series', icon: Tv, label: 'Series' },
  { to: '/movies', icon: Film, label: 'Movies' },
  { to: '/calendar', icon: CalendarDays, label: 'Calendar' },
  { to: '/queue', icon: Download, label: 'Queue' },
  { to: '/torrents', icon: Magnet, label: 'Torrents' },
  { to: '/usenet', icon: HardDrive, label: 'Usenet' },
  { to: '/history', icon: Clock, label: 'History' },
  { to: '/wanted/missing', icon: AlertCircle, label: 'Wanted' },
  { to: '/settings', icon: Settings, label: 'Settings' },
  { to: '/migrate', icon: Database, label: 'Migration' },
] as const

interface SidebarProps {
  collapsed: boolean
  onToggle: () => void
}

export default function Sidebar({ collapsed, onToggle }: SidebarProps) {
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
        {navItems.map(({ to, icon: Icon, label }) => (
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
