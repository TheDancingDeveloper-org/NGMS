import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const __dirname = dirname(fileURLToPath(import.meta.url))
const tauriConf = JSON.parse(
  readFileSync(resolve(__dirname, 'src-tauri/tauri.conf.json'), 'utf-8'),
)
const appVersion: string = tauriConf.version

const isTauri = !!process.env.TAURI_ENV_PLATFORM

export default defineConfig({
  plugins: [react()],
  define: {
    __APP_VERSION__: JSON.stringify(appVersion),
  },
  // Use relative paths for Tauri (Android WebView loads from file://),
  // absolute /app/ for web deployment
  base: isTauri ? '/' : '/app/',
  server: {
    port: 3001,
    // Bind to all interfaces so Tauri mobile dev can reach it
    host: isTauri ? '0.0.0.0' : 'localhost',
    proxy: {
      '/api': {
        target: 'http://192.168.1.75:9111',
        changeOrigin: true,
      },
    },
  },
  build: {
    // Ensure compatibility with Android WebView
    target: isTauri ? 'chrome105' : 'esnext',
  },
})
