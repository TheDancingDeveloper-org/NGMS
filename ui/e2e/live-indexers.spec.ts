// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

/**
 * Live E2E: Indexer CRUD + Search (J26-35)
 * Runs serially against real StackArr instance.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe.configure({ mode: 'serial' })
test.describe('Live: Indexer CRUD', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let indexerId: number | null = null

  // J26: Add a Newznab indexer
  test('J26: add a Newznab indexer via API', async ({ page }) => {
    const r = await page.request.post('/api/v1/indexer', {
      data: {
        name: 'Test Newznab',
        indexerType: 'Newznab',
        baseUrl: 'https://nzb.indexarr.net/api/v1',
        apiKey: '3bdec035-6fae-40c4-b3b7-fc8e8251ba5e',
        protocol: 'usenet',
        enabled: true,
        priority: 10,
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    indexerId = body.id
    expect(indexerId).toBeTruthy()
    expect(body.name).toBe('Test Newznab')
  })

  // J27: Add a Torznab indexer
  test('J27: add a Torznab indexer via API', async ({ page }) => {
    const r = await page.request.post('/api/v1/indexer', {
      data: {
        name: 'Test Torznab',
        indexerType: 'Torznab',
        baseUrl: 'https://torrent.example.com/api',
        apiKey: 'fake-key',
        protocol: 'torrent',
        enabled: true,
        priority: 20,
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    expect(body.name).toBe('Test Torznab')
  })

  // J29: Browse Cardigann catalog
  test('J29: browse available Cardigann indexer catalog', async ({ page }) => {
    const r = await page.request.get('/api/v1/indexer/available')
    expect(r.ok()).toBeTruthy()
    const catalog = await r.json()
    expect(Array.isArray(catalog)).toBeTruthy()
    // Should have at least some definitions loaded
    if (catalog.length > 0) {
      expect(catalog[0]).toHaveProperty('id')
      expect(catalog[0]).toHaveProperty('name')
      expect(catalog[0]).toHaveProperty('protocol')
    }
  })

  // J30: Test an indexer connection
  test('J30: test indexer connection', async ({ page }) => {
    expect(indexerId).toBeTruthy()
    const r = await page.request.post(`/api/v1/indexer/${indexerId}/test`)
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.success).toBe(true)
  })

  // J31: Edit an indexer
  test('J31: edit an indexer', async ({ page }) => {
    expect(indexerId).toBeTruthy()
    const r = await page.request.put(`/api/v1/indexer/${indexerId}`, {
      data: { name: 'Test Newznab (Edited)', priority: 5 },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.name).toBe('Test Newznab (Edited)')
    expect(body.priority).toBe(5)
  })

  // J31: Disable/re-enable
  test('J31: disable and re-enable an indexer', async ({ page }) => {
    expect(indexerId).toBeTruthy()
    let r = await page.request.put(`/api/v1/indexer/${indexerId}`, {
      data: { enabled: false },
    })
    expect(r.ok()).toBeTruthy()
    expect((await r.json()).enabled).toBe(false)

    r = await page.request.put(`/api/v1/indexer/${indexerId}`, {
      data: { enabled: true },
    })
    expect(r.ok()).toBeTruthy()
    expect((await r.json()).enabled).toBe(true)
  })

  // J32: Delete an indexer
  test('J32: delete an indexer', async ({ page }) => {
    // Delete the torznab one (keep newznab for search tests)
    const listResp = await page.request.get('/api/v1/indexer')
    const indexers = await listResp.json()
    const torznab = indexers.find((i: { name: string }) => i.name === 'Test Torznab')
    expect(torznab).toBeTruthy()

    const r = await page.request.delete(`/api/v1/indexer/${torznab.id}`)
    expect(r.status()).toBe(204)

    // Verify gone
    const after = await page.request.get('/api/v1/indexer')
    const remaining = await after.json()
    expect(remaining.find((i: { name: string }) => i.name === 'Test Torznab')).toBeUndefined()
  })

  // J33: Freehand search
  test('J33: freehand search across indexers', async ({ page }) => {
    const r = await page.request.get('/api/v1/search?query=test')
    expect(r.ok()).toBeTruthy()
    const results = await r.json()
    expect(Array.isArray(results)).toBeTruthy()
  })

  // J34: Search with category filters
  test('J34: search with category filter', async ({ page }) => {
    const r = await page.request.get('/api/v1/search?query=test&categories=2000,5000')
    expect(r.ok()).toBeTruthy()
    const results = await r.json()
    expect(Array.isArray(results)).toBeTruthy()
  })

  // J35: Search with Indexarr-only flag
  test('J35: search with Indexarr-only flag', async ({ page }) => {
    const r = await page.request.get('/api/v1/search?query=test&indexarrOnly=true')
    // May return 200 (results), 502/503 if Indexarr sidecar is down
    expect([200, 502, 503].includes(r.status())).toBeTruthy()
  })
})

test.describe('Live: Indexer Settings UI', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('indexers settings tab shows configured indexers', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: 'Indexers' }).click()
    await expect(page.getByText('Indexers').first()).toBeVisible()
    // Should show our added indexer(s)
    await expect(page.getByText(/Test Newznab|Newznab/i).first()).toBeVisible({ timeout: 5000 })
  })

  test('indexer catalog is browsable', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: 'Indexers' }).click()
    await page.getByRole('button', { name: /Browse Indexers/i }).click()
    await expect(page.getByText('Available Indexers')).toBeVisible({ timeout: 5000 })
    await expect(page.getByPlaceholder(/Search indexers/i)).toBeVisible()
  })

  test('manual add indexer form opens', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Indexers/i }).click()
    await page.getByRole('button', { name: /Manual/i }).click()
    await expect(page.getByText(/Add Indexer/i)).toBeVisible()
    await expect(page.getByLabel('Name')).toBeVisible()
    await expect(page.getByLabel(/URL/i)).toBeVisible()
  })
})
