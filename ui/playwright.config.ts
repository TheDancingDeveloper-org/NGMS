// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { defineConfig, devices } from '@playwright/test'

/**
 * Two modes:
 *   npm run test:e2e          — mocked API, local Vite dev server
 *   npm run test:e2e:live     — real StackArr at PLAYWRIGHT_BASE_URL
 */
const isLive = !!process.env.PLAYWRIGHT_LIVE

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: process.env.CI ? 'github' : 'html',
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL
      || (isLive ? 'http://node-b:9311' : 'http://localhost:3000'),
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  ...(!isLive && {
    webServer: {
      command: 'npm run dev',
      url: 'http://localhost:3000',
      reuseExistingServer: !process.env.CI,
      timeout: 15_000,
    },
  }),
})
