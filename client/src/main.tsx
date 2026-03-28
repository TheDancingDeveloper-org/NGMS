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

// Tauri uses HashRouter (works with custom protocol https://tauri.localhost/)
// Web uses BrowserRouter with /app basename
const isTauri = '__TAURI__' in window
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
