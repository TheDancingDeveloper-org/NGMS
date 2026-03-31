/**
 * Live E2E: TV Series CRUD (J36-46)
 * Runs serially against real StackArr instance.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe.configure({ mode: 'serial' })

// ─── Helpers ───────────────────────────────────────────────

async function hasTmdb(page: import('@playwright/test').Page): Promise<boolean> {
  const r = await page.request.get('/api/v1/discover/trending')
  return r.ok()
}

async function login(page: import('@playwright/test').Page) {
  await page.request.post('/api/v1/auth/login', {
    data: { username: 'admin', password: 'testpass123' },
  })
}

// ─── Series CRUD via API ───────────────────────────────────

test.describe('Live: Series CRUD (API)', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let seriesId: number | null = null

  // J36: Create a series (no TMDB — just title + profile)
  test('J36: create a series via API', async ({ page }) => {
    const r = await page.request.post('/api/v1/series', {
      data: {
        title: 'Test Series',
        monitored: true,
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    seriesId = body.id
    expect(seriesId).toBeTruthy()
    expect(body.title).toBe('Test Series')
  })

  // J36: Create series with TMDB ID (populates episodes if TMDB key present)
  test('J36: create series with TMDB ID', async ({ page }) => {
    const r = await page.request.post('/api/v1/series', {
      data: {
        title: 'Breaking Bad',
        tmdbId: 1396,
        monitored: true,
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    expect(body.title).toBe('Breaking Bad')
  })

  // J37: List all series
  test('J37: list series', async ({ page }) => {
    const r = await page.request.get('/api/v1/series')
    expect(r.ok()).toBeTruthy()
    const series = await r.json()
    expect(Array.isArray(series)).toBeTruthy()
    expect(series.length).toBeGreaterThanOrEqual(2)
  })

  // J38: Get series detail
  test('J38: get series detail', async ({ page }) => {
    expect(seriesId).toBeTruthy()
    const r = await page.request.get(`/api/v1/series/${seriesId}`)
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.title).toBe('Test Series')
  })

  // J44: Edit series
  test('J44: edit series settings', async ({ page }) => {
    expect(seriesId).toBeTruthy()
    const r = await page.request.put(`/api/v1/series/${seriesId}`, {
      data: { title: 'Test Series (Edited)', monitored: false },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.title).toBe('Test Series (Edited)')
    expect(body.monitored).toBe(false)
  })

  // J39: Monitor/unmonitor (toggle back)
  test('J39: toggle series monitoring', async ({ page }) => {
    expect(seriesId).toBeTruthy()
    let r = await page.request.put(`/api/v1/series/${seriesId}`, {
      data: { monitored: true },
    })
    expect(r.ok()).toBeTruthy()
    expect((await r.json()).monitored).toBe(true)
  })

  // J45: Delete series (requires admin)
  test('J45: delete a series', async ({ page }) => {
    await login(page)
    expect(seriesId).toBeTruthy()
    const r = await page.request.delete(`/api/v1/series/${seriesId}`)
    expect(r.status()).toBe(204)

    // Verify deleted
    const getResp = await page.request.get(`/api/v1/series/${seriesId}`)
    expect(getResp.status()).toBe(404)
  })
})

// ─── Series with TMDB (episodes) ──────────────────────────

test.describe('Live: Series Episodes', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // Find Breaking Bad series (created above)
  test('J38: list episodes for series', async ({ page }) => {
    const listResp = await page.request.get('/api/v1/series')
    const series = await listResp.json()
    const bb = series.find((s: { title: string }) => s.title === 'Breaking Bad')
    if (!bb) { test.skip(); return }

    const r = await page.request.get(`/api/v1/series/${bb.id}/episodes`)
    expect(r.ok()).toBeTruthy()
    const episodes = await r.json()
    expect(Array.isArray(episodes)).toBeTruthy()
    // If TMDB populated episodes, there should be many
    if (episodes.length > 0) {
      expect(episodes[0]).toHaveProperty('seasonNumber')
      expect(episodes[0]).toHaveProperty('episodeNumber')
      expect(episodes[0]).toHaveProperty('title')
    }
  })

  // J39: Monitor/unmonitor specific episode
  test('J39: monitor/unmonitor episode', async ({ page }) => {
    const listResp = await page.request.get('/api/v1/series')
    const series = await listResp.json()
    const bb = series.find((s: { title: string }) => s.title === 'Breaking Bad')
    if (!bb) { test.skip(); return }

    const epResp = await page.request.get(`/api/v1/series/${bb.id}/episodes`)
    const episodes = await epResp.json()
    if (episodes.length === 0) { test.skip(); return }

    const ep = episodes[0]
    // Unmonitor
    let r = await page.request.put(`/api/v1/episode/${ep.id}`, {
      data: { monitored: false },
    })
    expect(r.ok()).toBeTruthy()

    // Re-monitor
    r = await page.request.put(`/api/v1/episode/${ep.id}`, {
      data: { monitored: true },
    })
    expect(r.ok()).toBeTruthy()
  })

  // J40: Bulk monitor season
  test('J40: monitor/unmonitor entire season', async ({ page }) => {
    const listResp = await page.request.get('/api/v1/series')
    const series = await listResp.json()
    const bb = series.find((s: { title: string }) => s.title === 'Breaking Bad')
    if (!bb) { test.skip(); return }

    const r = await page.request.put(`/api/v1/series/${bb.id}/seasons/1/monitor`, {
      data: { monitored: false },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.monitored).toBe(false)

    // Re-monitor
    await page.request.put(`/api/v1/series/${bb.id}/seasons/1/monitor`, {
      data: { monitored: true },
    })
  })

  // J39: Bulk monitor episodes
  test('J39: bulk monitor episodes', async ({ page }) => {
    const listResp = await page.request.get('/api/v1/series')
    const series = await listResp.json()
    const bb = series.find((s: { title: string }) => s.title === 'Breaking Bad')
    if (!bb) { test.skip(); return }

    const epResp = await page.request.get(`/api/v1/series/${bb.id}/episodes`)
    const episodes = await epResp.json()
    if (episodes.length < 2) { test.skip(); return }

    const ids = episodes.slice(0, 3).map((e: { id: number }) => e.id)
    const r = await page.request.put('/api/v1/episode/monitor', {
      data: { episodeIds: ids, monitored: false },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.updated).toBe(ids.length)
  })
})

// ─── Series Lookup (requires TMDB) ────────────────────────

test.describe('Live: Series Lookup (TMDB)', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('J36: lookup series via TMDB', async ({ page }) => {
    const tmdbAvailable = await hasTmdb(page)
    if (!tmdbAvailable) { test.skip(); return }

    const r = await page.request.get('/api/v1/series/lookup?term=breaking+bad')
    expect(r.ok()).toBeTruthy()
    const results = await r.json()
    expect(Array.isArray(results)).toBeTruthy()
    expect(results.length).toBeGreaterThan(0)
    expect(results[0]).toHaveProperty('tmdbId')
    expect(results[0]).toHaveProperty('title')
  })

  test('J36: lookup returns 503 without TMDB key', async ({ page }) => {
    const tmdbAvailable = await hasTmdb(page)
    if (tmdbAvailable) { test.skip(); return }

    const r = await page.request.get('/api/v1/series/lookup?term=test')
    expect(r.status()).toBe(503)
  })
})

// ─── Series UI ─────────────────────────────────────────────

test.describe('Live: Series UI', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('J37: series list page shows library', async ({ page }) => {
    await page.goto('/series')
    await expect(page.getByRole('heading', { name: 'Series' })).toBeVisible()
    // Should show at least Breaking Bad
    await expect(page.getByText('Breaking Bad')).toBeVisible({ timeout: 5000 })
  })

  test('J37: series list has filter and add button', async ({ page }) => {
    await page.goto('/series')
    await expect(page.getByPlaceholder('Filter series...')).toBeVisible()
    await expect(page.getByRole('button', { name: /Add Series/i })).toBeVisible()
  })

  test('J37: filter narrows results', async ({ page }) => {
    await page.goto('/series')
    await page.getByPlaceholder('Filter series...').fill('Breaking')
    await expect(page.getByText('Breaking Bad')).toBeVisible()
  })

  test('J37: filter with no match shows empty', async ({ page }) => {
    await page.goto('/series')
    await page.getByPlaceholder('Filter series...').fill('zzz_no_match_zzz')
    await expect(page.getByText('Breaking Bad')).not.toBeVisible()
  })

  test('J37: library/browse toggle works', async ({ page }) => {
    await page.goto('/series')
    await page.getByRole('button', { name: 'Browse' }).click()
    // Browse view shouldn't have filter input
    await expect(page.getByPlaceholder('Filter series...')).not.toBeVisible()
    await page.getByRole('button', { name: 'Library' }).click()
    await expect(page.getByPlaceholder('Filter series...')).toBeVisible()
  })

  test('J36: add series modal opens with search', async ({ page }) => {
    await page.goto('/series')
    await page.getByRole('button', { name: /Add Series/i }).click()
    await expect(page.getByRole('heading', { name: 'Add Series' })).toBeVisible()
    await expect(page.getByPlaceholder(/Search for a series/i)).toBeVisible()
  })

  test('J38: clicking series navigates to detail', async ({ page }) => {
    await page.goto('/series')
    await page.getByText('Breaking Bad').click()
    await page.waitForTimeout(1000)
    await expect(page).toHaveURL(/\/series\/\d+/)
    await expect(page.getByText('Breaking Bad')).toBeVisible()
  })

  test('J38: series detail shows action buttons', async ({ page }) => {
    await page.goto('/series')
    await page.getByText('Breaking Bad').click()
    await page.waitForTimeout(1000)
    // Should have monitor toggle, search, and back button
    await expect(page.getByRole('button', { name: /Back to Series/i })).toBeVisible()
    await expect(
      page.getByRole('button', { name: /Monitored|Unmonitored/i }).first()
    ).toBeVisible()
  })
})

// ─── Release Search ────────────────────────────────────────

test.describe('Live: Release Search', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J41: Search for a specific episode
  test('J41: search releases for episode', async ({ page }) => {
    const r = await page.request.get('/api/v1/release?term=breaking+bad+s01e01&mediaType=series')
    expect(r.ok()).toBeTruthy()
    const results = await r.json()
    expect(Array.isArray(results)).toBeTruthy()
  })

  // J50: Search for movie release
  test('J50: search releases for movie', async ({ page }) => {
    const r = await page.request.get('/api/v1/release?term=inception+2010&mediaType=movie')
    expect(r.ok()).toBeTruthy()
    const results = await r.json()
    expect(Array.isArray(results)).toBeTruthy()
  })
})

// ─── Wanted ────────────────────────────────────────────────

test.describe('Live: Wanted', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J83: Missing episodes
  test('J83: wanted/missing API returns paginated data', async ({ page }) => {
    const r = await page.request.get('/api/v1/wanted/missing?page=1&page_size=10')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('page')
    expect(body).toHaveProperty('totalRecords')
    expect(body).toHaveProperty('records')
  })

  // J84: Cutoff unmet
  test('J84: wanted/cutoff API returns paginated data', async ({ page }) => {
    const r = await page.request.get('/api/v1/wanted/cutoff?page=1&page_size=10')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('records')
  })

  // Wanted UI
  test('J83: wanted page loads with tabs', async ({ page }) => {
    await page.goto('/wanted/missing')
    await expect(page.getByRole('heading', { name: /Wanted/i })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Missing' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Cutoff Unmet' })).toBeVisible()
  })

  test('J84: cutoff unmet tab switches', async ({ page }) => {
    await page.goto('/wanted/missing')
    await page.getByRole('button', { name: 'Cutoff Unmet' }).click()
    await expect(page.getByRole('button', { name: /Search All Cutoff/i })).toBeVisible()
  })
})
