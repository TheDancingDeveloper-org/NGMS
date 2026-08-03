// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

/**
 * Live E2E: Movie CRUD (J47-53)
 * Runs serially against real StackArr instance.
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

// ─── Movie CRUD via API ────────────────────────────────────

test.describe('Live: Movie CRUD (API)', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let movieId: number | null = null

  // J47: Create a movie
  test('J47: create a movie via API', async ({ page }) => {
    const r = await page.request.post('/api/v1/movies', {
      data: {
        title: 'Inception',
        year: 2010,
        tmdbId: 27205,
        monitored: true,
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    movieId = body.id
    expect(movieId).toBeTruthy()
    expect(body.title).toBe('Inception')
  })

  // J47: Create another movie
  test('J47: create a second movie', async ({ page }) => {
    const r = await page.request.post('/api/v1/movies', {
      data: {
        title: 'The Dark Knight',
        year: 2008,
        tmdbId: 155,
        monitored: false,
      },
    })
    expect(r.status()).toBe(201)
  })

  // J48: List movies
  test('J48: list movies', async ({ page }) => {
    const r = await page.request.get('/api/v1/movies')
    expect(r.ok()).toBeTruthy()
    const movies = await r.json()
    expect(movies.length).toBeGreaterThanOrEqual(2)
    const titles = movies.map((m: { title: string }) => m.title)
    expect(titles).toContain('Inception')
    expect(titles).toContain('The Dark Knight')
  })

  // J49: Get movie detail
  test('J49: get movie detail', async ({ page }) => {
    expect(movieId).toBeTruthy()
    const r = await page.request.get(`/api/v1/movies/${movieId}`)
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.title).toBe('Inception')
    expect(body).toHaveProperty('hasFile')
  })

  // J52: Edit movie
  test('J52: edit movie settings', async ({ page }) => {
    expect(movieId).toBeTruthy()
    const r = await page.request.put(`/api/v1/movies/${movieId}`, {
      data: { monitored: false },
    })
    expect(r.ok()).toBeTruthy()
    expect((await r.json()).monitored).toBe(false)
  })

  // Toggle monitoring back
  test('J52: toggle movie monitoring', async ({ page }) => {
    expect(movieId).toBeTruthy()
    const r = await page.request.put(`/api/v1/movies/${movieId}`, {
      data: { monitored: true },
    })
    expect(r.ok()).toBeTruthy()
    expect((await r.json()).monitored).toBe(true)
  })

  // J53: Delete movie (requires admin)
  test('J53: delete a movie', async ({ page }) => {
    await login(page)
    // Delete The Dark Knight, keep Inception
    const listResp = await page.request.get('/api/v1/movies')
    const movies = await listResp.json()
    const dk = movies.find((m: { title: string }) => m.title === 'The Dark Knight')
    expect(dk).toBeTruthy()

    const r = await page.request.delete(`/api/v1/movies/${dk.id}`)
    expect(r.status()).toBe(204)

    // Verify deleted
    const getResp = await page.request.get(`/api/v1/movies/${dk.id}`)
    expect(getResp.status()).toBe(404)
  })
})

// ─── Movie Lookup (TMDB) ──────────────────────────────────

test.describe('Live: Movie Lookup (TMDB)', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('J47: lookup movie via TMDB', async ({ page }) => {
    const tmdbAvailable = await hasTmdb(page)
    if (!tmdbAvailable) { test.skip(); return }

    const r = await page.request.get('/api/v1/movies/lookup?term=inception')
    expect(r.ok()).toBeTruthy()
    const results = await r.json()
    expect(results.length).toBeGreaterThan(0)
    expect(results[0]).toHaveProperty('tmdbId')
  })
})

// ─── Movie UI ──────────────────────────────────────────────

test.describe('Live: Movie UI', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('J48: movie list page shows movies', async ({ page }) => {
    await page.goto('/movies')
    await expect(page.getByRole('heading', { name: 'Movies' })).toBeVisible()
    await expect(page.getByText('Inception')).toBeVisible({ timeout: 5000 })
  })

  test('J48: movie list has filter and add button', async ({ page }) => {
    await page.goto('/movies')
    await expect(page.getByPlaceholder('Filter movies...')).toBeVisible()
    await expect(page.getByRole('button', { name: /Add Movie/i }).first()).toBeVisible()
  })

  test('J48: filter narrows results', async ({ page }) => {
    await page.goto('/movies')
    await page.getByPlaceholder('Filter movies...').fill('Inception')
    await expect(page.getByText('Inception')).toBeVisible()
  })

  test('J48: library/browse toggle works', async ({ page }) => {
    await page.goto('/movies')
    await page.getByRole('button', { name: 'Browse' }).click()
    await expect(page.getByPlaceholder('Filter movies...')).not.toBeVisible()
    await page.getByRole('button', { name: 'Library' }).click()
    await expect(page.getByPlaceholder('Filter movies...')).toBeVisible()
  })

  test('J47: add movie modal opens with search', async ({ page }) => {
    await page.goto('/movies')
    await page.getByRole('button', { name: /Add Movie/i }).first().click()
    await expect(page.getByRole('heading', { name: 'Add Movie' })).toBeVisible()
    await expect(page.getByPlaceholder(/Search for a movie/i)).toBeVisible()
  })

  test('J49: clicking movie navigates to detail', async ({ page }) => {
    await page.goto('/movies')
    await page.getByText('Inception').click()
    await page.waitForTimeout(1000)
    await expect(page).toHaveURL(/\/movies\/\d+/)
    await expect(page.getByText('Inception')).toBeVisible()
  })

  test('J49: movie detail shows file status and action buttons', async ({ page }) => {
    await page.goto('/movies')
    await page.getByText('Inception').click()
    await page.waitForTimeout(1000)
    // Should show file status and action buttons
    await expect(page.getByRole('button', { name: /Back to Movies/i })).toBeVisible()
    await expect(
      page.getByText('File Available').or(page.getByText('No file available'))
    ).toBeVisible()
  })
})
