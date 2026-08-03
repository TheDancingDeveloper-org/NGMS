// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect, mockApi } from './fixtures'

test.describe('History page', () => {
  test('renders history page with title', async ({ mockPage: page }) => {
    await page.goto('/history')
    await expect(page.locator('h1, h2').filter({ hasText: 'History' })).toBeVisible()
  })

  test('shows empty state when no history', async ({ page }) => {
    await mockApi(page, { history: { page: 1, pageSize: 20, totalRecords: 0, records: [] } })
    await page.goto('/history')
    await expect(page.getByText('No history events yet')).toBeVisible()
  })

  test('renders history with records', async ({ page }) => {
    await mockApi(page, {
      history: {
        page: 1,
        pageSize: 20,
        totalRecords: 1,
        records: [
          {
            id: 1,
            date: '2026-03-30T01:00:00Z',
            eventType: 'grabbed',
            sourceTitle: 'Breaking.Bad.S01E01.720p.BluRay',
            quality: { quality: { name: 'Bluray-720p' } },
            mediaType: 'series',
            seriesId: 1,
            movieId: null,
            episodeId: 101,
            indexer: 'TestIndexer',
          },
        ],
      },
    })
    await page.goto('/history')
    await expect(page.getByText('Breaking.Bad.S01E01')).toBeVisible()
  })
})
