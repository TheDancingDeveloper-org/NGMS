/**
 * Live E2E: Discover page + Requests (J54-69)
 * Discover requires TMDB — tests skip gracefully if unavailable.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe.configure({ mode: 'serial' })

async function hasTmdb(page: import('@playwright/test').Page): Promise<boolean> {
  const r = await page.request.get('/api/v1/discover/trending')
  return r.ok()
}

async function login(page: import('@playwright/test').Page) {
  await page.request.post('/api/v1/auth/login', {
    data: { username: 'admin', password: 'testpass123' },
  })
}

// ─── Discover API (TMDB required) ──────────────────────────

test.describe('Live: Discover API', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('J54: trending endpoint', async ({ page }) => {
    const r = await page.request.get('/api/v1/discover/trending')
    // 200 if TMDB configured, 503 if not
    if (r.status() === 503) { test.skip(); return }
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.results).toBeTruthy()
    expect(body.results.length).toBeGreaterThan(0)
  })

  test('J55: upcoming movies', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    const r = await page.request.get('/api/v1/discover/movies/upcoming')
    expect(r.ok()).toBeTruthy()
  })

  test('J56: upcoming TV shows', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    const r = await page.request.get('/api/v1/discover/tv/upcoming')
    expect(r.ok()).toBeTruthy()
  })

  test('J57: filter by genre', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    // Genre 28 = Action
    const r = await page.request.get('/api/v1/discover/movies/genre/28')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.results).toBeTruthy()
  })

  test('J58: filter by studio', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    // Studio 420 = Marvel
    const r = await page.request.get('/api/v1/discover/movies/studio/420')
    expect(r.ok()).toBeTruthy()
  })

  test('J59: movie recommendations', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    const r = await page.request.get('/api/v1/discover/movies/27205/recommendations')
    expect(r.ok()).toBeTruthy()
  })

  test('J60: TV show similar', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    const r = await page.request.get('/api/v1/discover/tv/1396/similar')
    expect(r.ok()).toBeTruthy()
  })

  test('J61: discover search shows in-library status', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    const r = await page.request.get('/api/v1/discover/search?q=inception&type=movie')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    if (body.results && body.results.length > 0) {
      // The inLibrary field should be enriched
      expect(body.results[0]).toHaveProperty('inLibrary')
    }
  })

  test('genre lists are available', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    const movieGenres = await page.request.get('/api/v1/discover/genres/movie')
    expect(movieGenres.ok()).toBeTruthy()
    const tvGenres = await page.request.get('/api/v1/discover/genres/tv')
    expect(tvGenres.ok()).toBeTruthy()
  })
})

// ─── Discover Sliders (J63) ───────────────────────────────

test.describe('Live: Discover Sliders', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('J63: list discover sliders', async ({ page }) => {
    const r = await page.request.get('/api/v1/discover/sliders')
    expect(r.ok()).toBeTruthy()
    const sliders = await r.json()
    expect(Array.isArray(sliders)).toBeTruthy()
  })

  test('J63: reset sliders to defaults', async ({ page }) => {
    const r = await page.request.post('/api/v1/discover/sliders/reset')
    expect(r.ok()).toBeTruthy()
  })
})

// ─── Discover UI ──────────────────────────────────────────

test.describe('Live: Discover UI', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('J54: discover page loads', async ({ page }) => {
    await page.goto('/discover')
    await page.waitForTimeout(2000)
    // Should show either trending content or TMDB key error
    const hasContent = await page.getByText('Trending Today').isVisible().catch(() => false)
    const hasError = await page.getByText(/TMDB API key/i).isVisible().catch(() => false)
    expect(hasContent || hasError).toBeTruthy()
  })

  test('J54: discover shows trending section (with TMDB)', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    await page.goto('/discover')
    await expect(page.getByText('Trending Today')).toBeVisible({ timeout: 10_000 })
  })

  test('J55-56: discover shows popular and upcoming sections', async ({ page }) => {
    const tmdb = await hasTmdb(page)
    if (!tmdb) { test.skip(); return }
    await page.goto('/discover')
    await page.waitForTimeout(3000)
    await expect(page.getByText('Popular Movies')).toBeVisible()
    await expect(page.getByText('Popular TV Shows')).toBeVisible()
  })
})

// ─── Requests (J64-69) ───────────────────────────────────

test.describe('Live: Requests', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let requestId: number | null = null

  // J64-65: Create a request
  test('J64: create a movie request', async ({ page }) => {
    await login(page)
    const r = await page.request.post('/api/v1/requests', {
      data: {
        mediaType: 'movie',
        tmdbId: 550,
        title: 'Fight Club',
        year: 1999,
      },
    })
    // 201 created, or 409 if already in library/requested
    expect([201, 409].includes(r.status())).toBeTruthy()
    if (r.status() === 201) {
      requestId = (await r.json()).id
    }
  })

  test('J65: create a series request', async ({ page }) => {
    await login(page)
    const r = await page.request.post('/api/v1/requests', {
      data: {
        mediaType: 'series',
        tmdbId: 62560,
        title: 'Mr. Robot',
        year: 2015,
      },
    })
    expect([201, 409].includes(r.status())).toBeTruthy()
  })

  // J66: List pending requests
  test('J66: list pending requests', async ({ page }) => {
    await login(page)
    const r = await page.request.get('/api/v1/requests?status=pending')
    expect(r.ok()).toBeTruthy()
    const requests = await r.json()
    expect(Array.isArray(requests)).toBeTruthy()
  })

  // J66: Pending count
  test('J66: pending request count', async ({ page }) => {
    const r = await page.request.get('/api/v1/requests/pending/count')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('count')
  })

  // J67: Approve a request
  test('J67: approve a request', async ({ page }) => {
    if (!requestId) { test.skip(); return }
    await login(page)
    const r = await page.request.put(`/api/v1/requests/${requestId}/approve`, {
      data: { note: 'Approved by E2E test' },
    })
    expect(r.ok()).toBeTruthy()
  })

  // J68: Decline a request (use the series request)
  test('J68: decline a request', async ({ page }) => {
    await login(page)
    const listResp = await page.request.get('/api/v1/requests?status=pending')
    const pending = await listResp.json()
    if (pending.length === 0) { test.skip(); return }

    const r = await page.request.put(`/api/v1/requests/${pending[0].id}/decline`, {
      data: { note: 'Declined by E2E test' },
    })
    expect(r.ok()).toBeTruthy()
  })

  // J69: View own request history
  test('J69: view request history', async ({ page }) => {
    await login(page)
    const r = await page.request.get('/api/v1/requests')
    expect(r.ok()).toBeTruthy()
    const requests = await r.json()
    expect(requests.length).toBeGreaterThan(0)
  })

  // Requests UI
  test('J66: requests page loads', async ({ page }) => {
    await page.goto('/requests')
    await expect(page.getByRole('heading', { name: /Media Requests/i })).toBeVisible()
    await expect(page.getByRole('button', { name: 'All' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Pending' })).toBeVisible()
  })

  test('J66: request status tabs work', async ({ page }) => {
    await page.goto('/requests')
    await page.getByRole('button', { name: 'Approved' }).click()
    await page.waitForTimeout(500)
    await page.getByRole('button', { name: 'Declined' }).click()
    await page.waitForTimeout(500)
    await page.getByRole('button', { name: 'All' }).click()
  })
})
