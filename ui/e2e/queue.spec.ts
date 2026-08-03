// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect, mockApi } from './fixtures'

test.describe('Queue page', () => {
  test('renders page with title', async ({ mockPage: page }) => {
    await page.goto('/queue')
    await expect(page.locator('h1, h2').filter({ hasText: 'Queue' })).toBeVisible()
  })

  test('displays active downloads', async ({ mockPage: page }) => {
    await page.goto('/queue')
    await expect(page.getByText('Breaking.Bad.S01E01.720p.BluRay.x264')).toBeVisible()
  })

  test('shows download progress', async ({ mockPage: page }) => {
    await page.goto('/queue')
    // The progress percentage or bar should be present
    await expect(page.getByText(/45%/)).toBeVisible()
  })

  test('shows status badge', async ({ mockPage: page }) => {
    await page.goto('/queue')
    await expect(page.getByText('downloading')).toBeVisible()
  })

  test('shows auto-refresh note', async ({ mockPage: page }) => {
    await page.goto('/queue')
    await expect(page.getByText(/auto-refreshes every 5s/i)).toBeVisible()
  })

  test('has refresh button', async ({ mockPage: page }) => {
    await page.goto('/queue')
    await expect(page.getByTitle(/refresh now/i)).toBeVisible()
  })

  test('shows table headers', async ({ mockPage: page }) => {
    await page.goto('/queue')
    await expect(page.getByText('Title').first()).toBeVisible()
    await expect(page.getByText('Status').first()).toBeVisible()
    await expect(page.getByText('Progress').first()).toBeVisible()
    await expect(page.getByText('Size').first()).toBeVisible()
    await expect(page.getByText('ETA').first()).toBeVisible()
    await expect(page.getByText('Client').first()).toBeVisible()
  })

  test('shows download client name', async ({ mockPage: page }) => {
    await page.goto('/queue')
    await expect(page.getByText('librtbit')).toBeVisible()
  })

  test('shows empty queue message when no downloads', async ({ page }) => {
    await mockApi(page, { queue: [] })
    await page.goto('/queue')
    // Should show some kind of empty state
    await expect(page.getByText(/no.*download|queue.*empty|nothing/i)).toBeVisible()
  })

  test('renders multiple queue items', async ({ page }) => {
    await mockApi(page, {
      queue: [
        {
          id: 1, title: 'Breaking.Bad.S01E01.720p', status: 'downloading', progress: 45,
          size: 1_500_000_000, sizeLeft: 822_000_000, estimatedCompletionTime: '2026-03-30T02:00:00Z',
          downloadClient: 'librtbit', mediaType: 'series', seriesId: 1, quality: 'Bluray-720p',
        },
        {
          id: 2, title: 'Inception.2010.1080p', status: 'queued', progress: 0,
          size: 4_000_000_000, sizeLeft: 4_000_000_000, estimatedCompletionTime: null,
          downloadClient: 'nzb-engine', mediaType: 'movie', movieId: 1, quality: 'Bluray-1080p',
        },
      ],
    })
    await page.goto('/queue')
    await expect(page.getByText('Breaking.Bad.S01E01.720p')).toBeVisible()
    await expect(page.getByText('Inception.2010.1080p')).toBeVisible()
  })

  test('shows failed status with error message', async ({ page }) => {
    await mockApi(page, {
      queue: [
        {
          id: 1, title: 'Failed.Download', status: 'failed', progress: 0,
          size: 1_000_000, sizeLeft: 1_000_000, estimatedCompletionTime: null,
          downloadClient: 'librtbit', mediaType: 'series', seriesId: 1, quality: 'HDTV-720p',
          errorMessage: 'Tracker returned error: not registered',
        },
      ],
    })
    await page.goto('/queue')
    // Status badge uses capitalize CSS class, so "failed" renders as "Failed"
    await expect(page.getByText(/failed/i).first()).toBeVisible()
    await expect(page.getByText('Tracker returned error')).toBeVisible()
  })
})
