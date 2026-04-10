/* eslint-disable react-refresh/only-export-components -- entry point, no HMR exports needed */
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter, HashRouter } from 'react-router-dom'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { AuthProvider } from './context/AuthContext'
import App from './App'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
})

// Tauri uses HashRouter (works with custom protocol http://tauri.localhost/).
// Web uses BrowserRouter with /app basename.
// Build-time discriminator: vite.config.ts sets `base: '/'` for Tauri and
// `'/app/'` for web, so BASE_URL is the authoritative signal — runtime checks
// like `'__TAURI__' in window` are unreliable on Tauri v2 mobile.
const isTauri = import.meta.env.BASE_URL === '/'
const Router = isTauri
  ? ({ children }: { children: React.ReactNode }) => <HashRouter>{children}</HashRouter>
  : ({ children }: { children: React.ReactNode }) => <BrowserRouter basename="/app">{children}</BrowserRouter>

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <Router>
        <AuthProvider>
          <App />
        </AuthProvider>
      </Router>
    </QueryClientProvider>
  </StrictMode>,
)

// Register service worker (web only, not Tauri)
if ('serviceWorker' in navigator && !isTauri) {
  window.addEventListener('load', () => {
    navigator.serviceWorker
      .register('/app/sw.js', { scope: '/app/' })
      .then((reg) => {
        console.log('SW registered:', reg.scope)
      })
      .catch((err) => {
        console.warn('SW registration failed:', err)
      })
  })
}
