import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const isTauri = !!process.env.TAURI_ENV_PLATFORM

export default defineConfig({
  plugins: [react()],
  // Use relative paths for Tauri (Android WebView loads from file://),
  // absolute /app/ for web deployment
  base: isTauri ? '/' : '/app/',
  server: {
    port: 3001,
    // Bind to all interfaces so Tauri mobile dev can reach it
    host: isTauri ? '0.0.0.0' : 'localhost',
    proxy: {
      '/api': {
        target: 'http://localhost:9111',
        changeOrigin: true,
      },
    },
  },
  build: {
    // Ensure compatibility with Android WebView
    target: isTauri ? 'chrome105' : 'esnext',
  },
})
