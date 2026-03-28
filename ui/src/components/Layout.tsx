import { useState } from 'react'
import { Outlet } from 'react-router-dom'
import Sidebar from './Sidebar'
import ActivityNotificationBell from './ActivityNotificationBell'
import { useSystemStatus } from '../hooks/useApi'

export default function Layout() {
  const [collapsed, setCollapsed] = useState(false)
  const { data: status } = useSystemStatus()

  return (
    <div className="flex min-h-screen bg-slate-900 text-white">
      <Sidebar collapsed={collapsed} onToggle={() => setCollapsed((c) => !c)} />

      <div
        className={`flex flex-1 flex-col transition-all duration-200 ${
          collapsed ? 'ml-16' : 'ml-56'
        }`}
      >
        {/* Top bar */}
        <header className="sticky top-0 z-20 flex h-14 items-center justify-between border-b border-slate-700/50 bg-slate-900/80 px-6 backdrop-blur-sm">
          <h1 className="text-lg font-semibold tracking-tight">StackArr</h1>
          <div className="flex items-center gap-3 text-sm text-slate-400">
            <ActivityNotificationBell />
            {status?.version && <span>v{status.version}</span>}
          </div>
        </header>

        {/* Content */}
        <main className="flex-1 p-6">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
