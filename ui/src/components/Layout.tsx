import { useState } from 'react'
import { Outlet } from 'react-router-dom'
import { PanelRight, LogOut } from 'lucide-react'
import Sidebar from './Sidebar'
import ActivityPanel from './ActivityPanel'
import { useSystemStatus, useRunningActivityCount, useUnreadNotificationCount, useCurrentUser } from '../hooks/useApi'

interface LayoutProps {
  onLogout?: () => void
}

export default function Layout({ onLogout }: LayoutProps) {
  const [collapsed, setCollapsed] = useState(false)
  const [activityOpen, setActivityOpen] = useState(true)
  const { data: status } = useSystemStatus()
  const { data: currentUser } = useCurrentUser()
  const { data: runningData } = useRunningActivityCount()
  const { data: unreadData } = useUnreadNotificationCount()

  const badgeCount = (runningData?.count ?? 0) + (unreadData?.count ?? 0)

  return (
    <div className="flex min-h-screen bg-slate-900 text-white">
      <Sidebar collapsed={collapsed} onToggle={() => setCollapsed((c) => !c)} />

      <div
        className={`flex min-w-0 flex-1 flex-col overflow-x-hidden transition-all duration-200 ${
          collapsed ? 'ml-16' : 'ml-56'
        } ${activityOpen ? 'mr-80' : 'mr-0'}`}
      >
        {/* Top bar */}
        <header className="sticky top-0 z-20 flex h-14 items-center justify-between border-b border-slate-700/50 bg-slate-900/80 px-6 backdrop-blur-sm">
          <div className="flex items-center gap-2">
            <img src="/images/NGMS_Logo.png" alt="NGMS" className="h-7 w-7" />
            <h1 className="text-lg font-semibold tracking-tight">NGMS</h1>
          </div>
          <div className="flex items-center gap-3 text-sm text-slate-400">
            <button
              onClick={() => setActivityOpen((o) => !o)}
              className={`relative flex h-8 w-8 items-center justify-center rounded-md border transition-colors ${
                activityOpen
                  ? 'border-blue-500/50 bg-blue-600/10 text-blue-400'
                  : 'border-slate-700 text-slate-400 hover:bg-slate-700 hover:text-slate-200'
              }`}
              title={activityOpen ? 'Hide activity panel' : 'Show activity panel'}
            >
              <PanelRight size={16} />
              {!activityOpen && badgeCount > 0 && (
                <span className="absolute -right-1 -top-1 flex h-4 min-w-4 items-center justify-center rounded-full border-2 border-slate-900 bg-red-500 px-0.5 text-[10px] font-bold text-white">
                  {badgeCount > 99 ? '99+' : badgeCount}
                </span>
              )}
            </button>
            {status?.version && <span>v{status.version}</span>}
            {onLogout && (
              <>
                {currentUser && (
                  <span className="text-slate-500">{currentUser.displayName || currentUser.username}</span>
                )}
                <button
                  onClick={onLogout}
                  className="flex h-8 w-8 items-center justify-center rounded-md border border-slate-700 text-slate-400 hover:bg-slate-700 hover:text-white transition-colors"
                  title="Sign out"
                >
                  <LogOut size={16} />
                </button>
              </>
            )}
          </div>
        </header>

        {/* Content */}
        <main className="flex-1 px-6 pb-6 pt-2">
          <Outlet />
        </main>
      </div>

      {/* Right activity panel */}
      {activityOpen && <ActivityPanel onClose={() => setActivityOpen(false)} />}
    </div>
  )
}
