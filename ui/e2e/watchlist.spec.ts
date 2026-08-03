// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect, mockApi } from './fixtures'

test.describe('Watchlist page', () => {
  test('shows disabled message when Plex is not enabled', async ({ mockPage: page }) => {
    // Default mock has plexIntegration: false
    await page.goto('/watchlist')
    await expect(page.getByText(/plex integration is not enabled/i)).toBeVisible()
    await expect(page.getByText(/enable plex in settings/i)).toBeVisible()
  })

  test('renders watchlist when Plex is enabled', async ({ page }) => {
    await mockApi(page, { status: { modules: {
      tvManagement: true, movieManagement: true, torrentEmbedded: true, usenetEmbedded: true,
      torrentExternal: false, usenetExternal: false, indexarrSidecar: true, externalIndexers: true,
      plexIntegration: true, notifications: true, streaming: false, remoteAccess: false, stremioAddon: false,
    }}})
    await page.goto('/watchlist')
    await expect(page.locator('h1, h2').filter({ hasText: 'Plex Watchlist' })).toBeVisible()
  })

  test('has Sync Now button when enabled', async ({ page }) => {
    await mockApi(page, { status: { modules: {
      tvManagement: true, movieManagement: true, torrentEmbedded: true, usenetEmbedded: true,
      torrentExternal: false, usenetExternal: false, indexarrSidecar: true, externalIndexers: true,
      plexIntegration: true, notifications: true, streaming: false, remoteAccess: false, stremioAddon: false,
    }}})
    await page.goto('/watchlist')
    await expect(page.getByRole('button', { name: /Sync Now/i })).toBeVisible()
  })

  test('displays watchlist items with media type', async ({ page }) => {
    await mockApi(page, { status: { modules: {
      tvManagement: true, movieManagement: true, torrentEmbedded: true, usenetEmbedded: true,
      torrentExternal: false, usenetExternal: false, indexarrSidecar: true, externalIndexers: true,
      plexIntegration: true, notifications: true, streaming: false, remoteAccess: false, stremioAddon: false,
    }}})
    await page.goto('/watchlist')
    await expect(page.getByText('TMDB #27205')).toBeVisible()
    await expect(page.getByText('TMDB #1396')).toBeVisible()
    await expect(page.getByText('movie').first()).toBeVisible()
    await expect(page.getByText('tv').first()).toBeVisible()
  })

  test('shows auto-requested badge', async ({ page }) => {
    await mockApi(page, { status: { modules: {
      tvManagement: true, movieManagement: true, torrentEmbedded: true, usenetEmbedded: true,
      torrentExternal: false, usenetExternal: false, indexarrSidecar: true, externalIndexers: true,
      plexIntegration: true, notifications: true, streaming: false, remoteAccess: false, stremioAddon: false,
    }}})
    await page.goto('/watchlist')
    await expect(page.getByText('Requested')).toBeVisible()
  })

  test('shows empty state when watchlist is empty', async ({ page }) => {
    await mockApi(page, { status: { modules: {
      tvManagement: true, movieManagement: true, torrentEmbedded: true, usenetEmbedded: true,
      torrentExternal: false, usenetExternal: false, indexarrSidecar: true, externalIndexers: true,
      plexIntegration: true, notifications: true, streaming: false, remoteAccess: false, stremioAddon: false,
    }}})
    await page.route('**/api/v1/plex/watchlist', (route) =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify([]) }),
    )
    await page.goto('/watchlist')
    await expect(page.getByText(/watchlist is empty/i)).toBeVisible()
  })
})
