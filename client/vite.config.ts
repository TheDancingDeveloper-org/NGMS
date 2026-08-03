// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

const isTauri = !!process.env.TAURI_ENV_PLATFORM
// Set by the Tauri CLI to the public network address it chose for the dev
// server (e.g. the machine's LAN/Tailscale IP). On mobile the WebView at
// `tauri.localhost` cannot proxy the HMR WebSocket — point it at that host
// directly so HMR actually connects.
const tauriDevHost = process.env.TAURI_DEV_HOST

export default defineConfig({
  plugins: [react()],
  // Use relative paths for Tauri (Android WebView loads from file://),
  // absolute /app/ for web deployment
  base: isTauri ? '/' : '/app/',
  server: {
    port: 3001,
    strictPort: true,
    // Bind to all interfaces so Tauri mobile dev can reach it
    host: isTauri ? '0.0.0.0' : 'localhost',
    hmr: tauriDevHost
      ? { protocol: 'ws', host: tauriDevHost, port: 3002 }
      : undefined,
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
