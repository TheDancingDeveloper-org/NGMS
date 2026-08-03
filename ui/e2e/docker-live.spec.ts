// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

/**
 * Live E2E tests for Docker Compose test environment.
 *
 * Runs against a real StackArr instance (PLAYWRIGHT_LIVE=1).
 * Tests actual user journeys from docs/userjourneys.md.
 *
 * IMPORTANT: These tests run serially because they share state
 * (first-boot setup must complete before other tests).
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

// Run all live tests serially — they share a single StackArr instance
test.describe.configure({ mode: 'serial' })

// Track whether setup has been completed so each test can check
let setupDone = false

// ─── First Boot & Setup ────────────────────────────────────

test.describe('Live: First Boot', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('health endpoint responds', async ({ page }) => {
    const response = await page.request.get('/health')
    expect(response.ok()).toBeTruthy()
    const body = await response.json()
    expect(body.status).toBe('ok')
  })

  test('system status shows firstBoot', async ({ page }) => {
    const response = await page.request.get('/api/v1/system/status')
    expect(response.ok()).toBeTruthy()
    const status = await response.json()
    expect(status.version).toBeTruthy()
    expect(status).toHaveProperty('firstBoot')
    expect(status).toHaveProperty('modules')
  })

  test('redirects to /setup on first boot', async ({ page }) => {
    const response = await page.request.get('/api/v1/system/status')
    const status = await response.json()
    if (!status.firstBoot) { test.skip(); return }

    await page.goto('/')
    await expect(page).toHaveURL(/\/setup/, { timeout: 10_000 })
    await expect(page.getByRole('heading', { name: /Create Admin Account/i })).toBeVisible()
  })

  test('J1: complete first-boot setup via API', async ({ page }) => {
    const response = await page.request.get('/api/v1/system/status')
    const status = await response.json()
    if (!status.firstBoot) { setupDone = true; return }

    // Step 1: Create admin account via API
    const setupResp = await page.request.post('/api/v1/auth/setup', {
      data: { username: 'admin', password: 'testpass123' },
    })
    expect(setupResp.ok()).toBeTruthy()

    // Step 2: Initialize with modules and folders via API
    const initResp = await page.request.post('/api/v1/setup/init', {
      data: {
        modules: {
          tvManagement: true,
          movieManagement: true,
          torrentEmbedded: true,
          usenetEmbedded: true,
          indexarrSidecar: false,
          plexIntegration: false,
          streaming: false,
          stremioAddon: false,
        },
        mediaLibraryFolders: [
          { path: '/media/TV', mediaType: 'tv' },
          { path: '/media/Movies', mediaType: 'movie' },
        ],
      },
    })
    expect(initResp.ok()).toBeTruthy()

    // Verify firstBoot is now false
    const verifyResp = await page.request.get('/api/v1/system/status')
    const newStatus = await verifyResp.json()
    expect(newStatus.firstBoot).toBe(false)

    // Verify app loads normally
    await page.goto('/')
    await expect(page).not.toHaveURL(/\/setup/, { timeout: 10_000 })
    setupDone = true
  })

  test('after setup, firstBoot is false', async ({ page }) => {
    const response = await page.request.get('/api/v1/system/status')
    const status = await response.json()
    expect(status.firstBoot).toBe(false)
    setupDone = true
  })
})

// ─── Post-Setup Tests ──────────────────────────────────────
// These all require setup to be complete.

test.describe('Live: Navigation', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('app loads discover page', async ({ page }) => {
    await page.goto('/')
    await expect(page).toHaveURL(/\/(discover|series)/, { timeout: 10_000 })
  })

  test('sidebar has core navigation links', async ({ page }) => {
    await page.goto('/')
    await page.waitForTimeout(1000)
    await expect(page.getByRole('link', { name: 'Discover' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'TV' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Movies' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Queue' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Settings' })).toBeVisible()
  })

  test('navigates to TV series page', async ({ page }) => {
    await page.goto('/')
    await page.waitForTimeout(500)
    await page.getByRole('link', { name: 'TV' }).click()
    await expect(page).toHaveURL(/\/series/)
  })

  test('navigates to Movies page', async ({ page }) => {
    await page.goto('/')
    await page.waitForTimeout(500)
    await page.getByRole('link', { name: 'Movies' }).click()
    await expect(page).toHaveURL(/\/movies/)
  })

  test('navigates to Queue page', async ({ page }) => {
    await page.goto('/')
    await page.waitForTimeout(500)
    await page.getByRole('link', { name: 'Queue' }).click()
    await expect(page).toHaveURL(/\/queue/)
  })

  test('navigates to Settings page', async ({ page }) => {
    await page.goto('/')
    await page.waitForTimeout(500)
    await page.getByRole('link', { name: 'Settings' }).click()
    await expect(page).toHaveURL(/\/settings/)
  })

  test('navigates to History page', async ({ page }) => {
    await page.goto('/')
    await page.waitForTimeout(500)
    await page.getByRole('link', { name: 'History' }).click()
    await expect(page).toHaveURL(/\/history/)
  })
})

test.describe('Live: Settings', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('General settings page loads', async ({ page }) => {
    await page.goto('/settings')
    await expect(page.getByText('General Settings')).toBeVisible()
    await expect(page.getByText('Instance Name')).toBeVisible()
  })

  test('download clients tab loads', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Download Clients/i }).click()
    // Should show the Add Client button
    await expect(page.getByRole('button', { name: /Add Client/i })).toBeVisible({ timeout: 5000 })
  })

  test('modules tab is accessible', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Modules/i }).click()
    await expect(page.getByText(/TV Management|Movie Management/i)).toBeVisible()
  })

  test('media folders tab shows configured paths', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Media Folders/i }).click()
    await expect(page.getByText(/\/media/i).first()).toBeVisible()
  })
})

test.describe('Live: Usenet', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('usenet page loads with tabs', async ({ page }) => {
    await page.goto('/usenet')
    await expect(page.getByRole('button', { name: 'Queue' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Servers' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Settings' })).toBeVisible()
  })

  test('servers tab loads with Add Server button', async ({ page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Servers' }).click()
    await expect(page.getByRole('button', { name: /Add Server/i })).toBeVisible({ timeout: 5000 })
  })

  test('usenet settings loads', async ({ page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('Max Active Downloads')).toBeVisible()
  })
})

test.describe('Live: Torrent Engine', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('torrent page loads', async ({ page }) => {
    await page.goto('/torrents')
    await expect(page.getByRole('button', { name: 'Torrents' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Settings' })).toBeVisible()
  })

  test('torrent settings accessible', async ({ page }) => {
    await page.goto('/torrents')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('Speed Limits')).toBeVisible()
  })
})

test.describe('Live: TV Series', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('series page loads', async ({ page }) => {
    await page.goto('/series')
    await expect(page.getByRole('heading', { name: /Series/i })).toBeVisible()
  })

  test('add series button exists', async ({ page }) => {
    await page.goto('/series')
    await expect(page.getByRole('button', { name: /Add Series/i })).toBeVisible()
  })
})

test.describe('Live: Movies', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('movies page loads with Add Movie button', async ({ page }) => {
    await page.goto('/movies')
    await page.waitForTimeout(1000)
    await expect(page.getByRole('button', { name: /Add Movie/i }).first()).toBeVisible()
  })
})

test.describe('Live: Queue', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('queue page loads', async ({ page }) => {
    await page.goto('/queue')
    await expect(page.getByText(/queue|download|nothing|empty/i).first()).toBeVisible({ timeout: 5000 })
  })
})

test.describe('Live: History', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('history page loads', async ({ page }) => {
    await page.goto('/history')
    await expect(page.locator('h1, h2').filter({ hasText: /History/i }).first()).toBeVisible()
  })
})

test.describe('Live: API endpoints', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('GET /api/v1/series', async ({ page }) => {
    const r = await page.request.get('/api/v1/series')
    expect(r.ok()).toBeTruthy()
  })

  test('GET /api/v1/movies', async ({ page }) => {
    const r = await page.request.get('/api/v1/movies')
    expect(r.ok()).toBeTruthy()
  })

  test('GET /api/v1/queue', async ({ page }) => {
    const r = await page.request.get('/api/v1/queue')
    expect(r.ok()).toBeTruthy()
  })

  test('GET /api/v1/qualityprofile', async ({ page }) => {
    const r = await page.request.get('/api/v1/qualityprofile')
    expect(r.ok()).toBeTruthy()
  })

  test('GET /api/v1/usenet/status', async ({ page }) => {
    const r = await page.request.get('/api/v1/usenet/status')
    expect(r.ok()).toBeTruthy()
    const data = await r.json()
    expect(data).toHaveProperty('enabled')
  })

  test('GET /api/v1/torrent/status', async ({ page }) => {
    const r = await page.request.get('/api/v1/torrent/status')
    expect(r.ok()).toBeTruthy()
    const data = await r.json()
    expect(data).toHaveProperty('enabled')
  })

  test('GET /api/v1/filesystem/browse', async ({ page }) => {
    const r = await page.request.get('/api/v1/filesystem/browse?path=%2F')
    expect(r.ok()).toBeTruthy()
    const data = await r.json()
    expect(data).toHaveProperty('directories')
  })

  test('POST /api/v1/indexer — add Newznab indexer', async ({ page }) => {
    const r = await page.request.post('/api/v1/indexer', {
      data: {
        name: 'Indexarr NZB',
        baseUrl: 'https://nzb.indexarr.net/api/v1',
        apiKey: '3bdec035-6fae-40c4-b3b7-fc8e8251ba5e',
        protocol: 'usenet',
        indexerType: 'newznab',
        enabled: true,
        priority: 0,
      },
    })
    expect([200, 201, 409].includes(r.status())).toBeTruthy()
  })
})
