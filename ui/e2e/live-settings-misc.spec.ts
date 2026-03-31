/**
 * Live E2E: Settings, naming, blocklist, history (J103-108, J142-146)
 * Runs serially against real StackArr instance.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe.configure({ mode: 'serial' })

// ─── General Settings (J142-144) ───────────────────────────

test.describe('Live: General Settings', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J142: Change instance name
  test('J142: change instance name via API', async ({ page }) => {
    const r = await page.request.put('/api/v1/config/general', {
      data: { instanceName: 'Test StackArr Renamed' },
    })
    expect(r.ok()).toBeTruthy()

    // Verify
    const getResp = await page.request.get('/api/v1/config/general')
    const config = await getResp.json()
    expect(config.instanceName).toBe('Test StackArr Renamed')

    // Restore
    await page.request.put('/api/v1/config/general', {
      data: { instanceName: 'StackArr-Test' },
    })
  })

  // J142: Instance name visible in settings UI
  test('J142: settings page shows instance name field', async ({ page }) => {
    await page.goto('/settings')
    await expect(page.getByText('Instance Name')).toBeVisible()
  })

  // J142: Change auth method
  test('J142: change auth method via API', async ({ page }) => {
    const r = await page.request.put('/api/v1/config/general', {
      data: { authMethod: 'none' },
    })
    expect(r.ok()).toBeTruthy()
  })

  // J142: Change grab strategy
  test('J142: change grab strategy via API', async ({ page }) => {
    const r = await page.request.put('/api/v1/config/general', {
      data: { grabStrategy: 'indexer_priority' },
    })
    expect(r.ok()).toBeTruthy()

    // Restore
    await page.request.put('/api/v1/config/general', {
      data: { grabStrategy: 'best_quality' },
    })
  })
})

// ─── Naming Templates (J145-146) ──────────────────────────

test.describe('Live: Naming Templates', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J145: Get naming config
  test('J145: get naming config', async ({ page }) => {
    const r = await page.request.get('/api/v1/config/naming')
    expect(r.ok()).toBeTruthy()
    const naming = await r.json()
    expect(naming).toHaveProperty('series')
    expect(naming).toHaveProperty('movie')
  })

  // J145: Update series naming
  test('J145: update series naming template', async ({ page }) => {
    const r = await page.request.put('/api/v1/config/naming', {
      data: {
        series: {
          renameFiles: true,
          standardFormat: '{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]',
          seasonFolderFormat: 'Season {season:00}',
        },
      },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.series).toBeTruthy()
  })

  // J146: Update movie naming
  test('J146: update movie naming template', async ({ page }) => {
    const r = await page.request.put('/api/v1/config/naming', {
      data: {
        movie: {
          renameFiles: true,
          movieFormat: '{Movie Title} ({Release Year}) [{Quality Title}]',
          movieFolderFormat: '{Movie Title} ({Release Year})',
        },
      },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.movie).toBeTruthy()
  })

  // J145: Naming settings UI
  test('J145: naming settings UI shows format fields', async ({ page }) => {
    await page.goto('/settings')
    // Settings sidebar nav
    await page.getByRole('button', { name: 'Naming' }).click()
    await expect(page.getByText(/Episode Formats|Standard Episode|Naming/i).first()).toBeVisible({ timeout: 5000 })
  })
})

// ─── Modules (J10) ────────────────────────────────────────

test.describe('Live: Module Config', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J10: Modules settings UI
  test('J10: modules tab shows all module toggles', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Modules/i }).click()
    await expect(page.getByText('Enabled Modules')).toBeVisible()
    await expect(page.getByText('TV Series Management')).toBeVisible()
    await expect(page.getByText('Movie Management')).toBeVisible()
    await expect(page.getByText('Embedded Torrent Client')).toBeVisible()
    await expect(page.getByText('Embedded Usenet Client')).toBeVisible()
  })
})

// ─── Blocklist (J105-108) ─────────────────────────────────

test.describe('Live: Blocklist', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let blockId: number | null = null

  // Helper: login to get session cookie (blocklist now uses RequireUser)
  async function login(page: import('@playwright/test').Page) {
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
  }

  // J105: List blocklist (initially empty)
  test('J105: list blocklist', async ({ page }) => {
    await login(page)
    const r = await page.request.get('/api/v1/blocklist')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('page')
    expect(body).toHaveProperty('totalRecords')
    expect(body).toHaveProperty('records')
  })

  // J106: Add to blocklist
  test('J106: add release to blocklist', async ({ page }) => {
    await login(page)
    const r = await page.request.post('/api/v1/blocklist', {
      data: {
        mediaType: 'series',
        mediaId: 1,
        sourceTitle: 'Test.Release.S01E01.720p.WEB',
        quality: { quality: { id: 4, name: 'WEBDL-720p' } },
        message: 'test blocklist entry',
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    blockId = body.id
    expect(blockId).toBeTruthy()
  })

  // J105: Verify entry appears
  test('J105: blocklist contains added entry', async ({ page }) => {
    await login(page)
    const r = await page.request.get('/api/v1/blocklist')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.totalRecords).toBeGreaterThan(0)
    expect(body.records[0].sourceTitle).toBe('Test.Release.S01E01.720p.WEB')
  })

  // J107: Remove from blocklist
  test('J107: remove release from blocklist', async ({ page }) => {
    await login(page)
    expect(blockId).toBeTruthy()
    const r = await page.request.delete(`/api/v1/blocklist/${blockId}`)
    expect(r.status()).toBe(204)
  })

  // J108: Bulk delete (add 2, then bulk delete)
  test('J108: bulk delete blocklist entries', async ({ page }) => {
    await login(page)
    // Add two entries
    const r1 = await page.request.post('/api/v1/blocklist', {
      data: { mediaType: 'series', mediaId: 1, sourceTitle: 'Bulk.Test.1', quality: {} },
    })
    const r2 = await page.request.post('/api/v1/blocklist', {
      data: { mediaType: 'series', mediaId: 1, sourceTitle: 'Bulk.Test.2', quality: {} },
    })
    const id1 = (await r1.json()).id
    const id2 = (await r2.json()).id

    const r = await page.request.delete('/api/v1/blocklist/bulk', {
      data: { ids: [id1, id2] },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.deleted).toBe(2)
  })
})

// ─── History (J103-104) ───────────────────────────────────

test.describe('Live: History', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J103: View history
  test('J103: history API returns paginated results', async ({ page }) => {
    const r = await page.request.get('/api/v1/history?page=1&pageSize=20')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('page')
    expect(body).toHaveProperty('pageSize')
    expect(body).toHaveProperty('totalRecords')
    expect(body).toHaveProperty('records')
  })

  // J104: Paginate history
  test('J104: history pagination works', async ({ page }) => {
    const r = await page.request.get('/api/v1/history?page=2&pageSize=5')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.page).toBe(2)
    expect(body.pageSize).toBe(5)
  })

  // J103: History page UI
  test('J103: history page loads', async ({ page }) => {
    await page.goto('/history')
    await expect(page.locator('h1, h2').filter({ hasText: /History/i }).first()).toBeVisible()
  })
})

// ─── Queue & Wanted (J81-86) ──────────────────────────────

test.describe('Live: Queue & Wanted', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J81: View queue
  test('J81: queue API returns data', async ({ page }) => {
    const r = await page.request.get('/api/v1/queue')
    expect(r.ok()).toBeTruthy()
  })

  // J81: Queue page UI
  test('J81: queue page loads', async ({ page }) => {
    await page.goto('/queue')
    await page.waitForTimeout(1000)
    // Should show queue content or empty state
    await expect(page.locator('body')).toBeVisible()
  })
})

// ─── Calendar (J101-102) ──────────────────────────────────

test.describe('Live: Calendar', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J101: Calendar API
  test('J101: calendar API returns data', async ({ page }) => {
    const r = await page.request.get('/api/v1/calendar')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(Array.isArray(body)).toBeTruthy()
  })

  // J101: Calendar page UI
  test('J101: calendar page loads', async ({ page }) => {
    await page.goto('/calendar')
    await page.waitForTimeout(1000)
    await expect(page.locator('body')).toBeVisible()
  })
})

// ─── Filesystem Browser (J160) ────────────────────────────

test.describe('Live: Filesystem Browser', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('J160: browse root directory', async ({ page }) => {
    const r = await page.request.get('/api/v1/filesystem/browse?path=%2F')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('directories')
    expect(body.directories.length).toBeGreaterThan(0)
  })

  test('J160: browse /media directory', async ({ page }) => {
    const r = await page.request.get('/api/v1/filesystem/browse?path=%2Fmedia')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('directories')
  })
})
