// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect, mockApi, mockSearchResults, mockIndexerConfigs } from './fixtures'

test.describe('Search page', () => {
  test('renders page with title', async ({ mockPage: page }) => {
    await page.goto('/search')
    await expect(page.locator('h1, h2').filter({ hasText: 'Search' })).toBeVisible()
  })

  test('has search input and button', async ({ mockPage: page }) => {
    await page.goto('/search')
    await expect(page.getByPlaceholder(/search indexers/i)).toBeVisible()
    await expect(page.getByRole('button', { name: 'Search' })).toBeVisible()
  })

  test('search button is disabled when input is empty', async ({ mockPage: page }) => {
    await page.goto('/search')
    await expect(page.getByRole('button', { name: 'Search' })).toBeDisabled()
  })

  test('has indexer dropdown defaulting to All Indexers', async ({ mockPage: page }) => {
    await page.goto('/search')
    await expect(page.getByText('All Indexers').first()).toBeVisible()
  })

  test('displays search results in table', async ({ mockPage: page }) => {
    await page.goto('/search?q=Breaking+Bad')
    await expect(page.getByText('Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND')).toBeVisible()
    await expect(page.getByText('Breaking.Bad.S01E01.1080p.WEB-DL')).toBeVisible()
  })

  test('shows indexer names on results', async ({ mockPage: page }) => {
    await page.goto('/search?q=Breaking+Bad')
    await expect(page.getByText('NZBGeek')).toBeVisible()
    await expect(page.getByText('1337x')).toBeVisible()
  })

  test('shows protocol badges (Usenet/Torrent)', async ({ mockPage: page }) => {
    await page.goto('/search?q=Breaking+Bad')
    await expect(page.getByText('Usenet').first()).toBeVisible()
    await expect(page.getByText('Torrent').first()).toBeVisible()
  })

  test('shows quality badges', async ({ mockPage: page }) => {
    await page.goto('/search?q=Breaking+Bad')
    await expect(page.getByText('Bluray-720p')).toBeVisible()
    await expect(page.getByText('WEBDL-1080p')).toBeVisible()
  })

  test('shows seeders/leechers for torrent results', async ({ mockPage: page }) => {
    await page.goto('/search?q=Breaking+Bad')
    await expect(page.getByText('42')).toBeVisible() // seeders
  })

  test('has Grab button on results', async ({ mockPage: page }) => {
    await page.goto('/search?q=Breaking+Bad')
    const grabButtons = page.getByRole('button', { name: 'Grab' })
    await expect(grabButtons.first()).toBeVisible()
  })

  test('shows no results message', async ({ page }) => {
    await mockApi(page)
    await page.route('**/api/v1/search**', (route) =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify([]) }),
    )
    await page.goto('/search?q=zzz_nothing_here')
    await expect(page.getByText(/no results found/i)).toBeVisible()
  })

  test('has sortable column headers', async ({ mockPage: page }) => {
    await page.goto('/search?q=Breaking+Bad')
    await expect(page.getByRole('button', { name: 'Title' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Size' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Age' })).toBeVisible()
  })
})
