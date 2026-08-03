// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect, mockApi } from './fixtures'

test.describe('Calendar page', () => {
  test('renders page with title', async ({ mockPage: page }) => {
    await page.goto('/calendar')
    await expect(page.locator('h1, h2').filter({ hasText: 'Calendar' })).toBeVisible()
  })

  test('displays calendar entry with series title', async ({ mockPage: page }) => {
    await page.goto('/calendar')
    await expect(page.getByText('Breaking Bad')).toBeVisible()
  })

  test('shows episode number', async ({ mockPage: page }) => {
    await page.goto('/calendar')
    await expect(page.getByText(/S01E01/)).toBeVisible()
  })

  test('shows episode title', async ({ mockPage: page }) => {
    await page.goto('/calendar')
    await expect(page.getByText('Pilot')).toBeVisible()
  })

  test('shows empty state when no episodes', async ({ page }) => {
    await mockApi(page, { calendar: [] })
    await page.goto('/calendar')
    await expect(page.getByText(/no upcoming episodes/i)).toBeVisible()
  })

  test('calendar entry is clickable', async ({ mockPage: page }) => {
    await page.goto('/calendar')
    const entry = page.getByText('Breaking Bad')
    await expect(entry).toBeVisible()
    // Each entry is wrapped in a button/link that navigates to series detail
    await entry.click()
    await expect(page).toHaveURL(/\/series\/1/)
  })

  test('shows multiple calendar entries grouped by date', async ({ page }) => {
    const today = new Date().toISOString()
    await mockApi(page, {
      calendar: [
        {
          episodeId: 101, seriesId: 1, seriesTitle: 'Breaking Bad',
          seasonNumber: 1, episodeNumber: 1, episodeTitle: 'Pilot',
          airDateUtc: today, monitored: true, hasFile: false, posterUrl: null,
        },
        {
          episodeId: 102, seriesId: 2, seriesTitle: 'The Office',
          seasonNumber: 1, episodeNumber: 1, episodeTitle: 'Diversity Day',
          airDateUtc: today, monitored: false, hasFile: true, posterUrl: null,
        },
      ],
    })
    await page.goto('/calendar')
    await expect(page.getByText('Breaking Bad')).toBeVisible()
    await expect(page.getByText('The Office')).toBeVisible()
  })

  test('shows Today label for current date entries', async ({ page }) => {
    const today = new Date().toISOString()
    await mockApi(page, {
      calendar: [
        {
          episodeId: 101, seriesId: 1, seriesTitle: 'Breaking Bad',
          seasonNumber: 1, episodeNumber: 1, episodeTitle: 'Pilot',
          airDateUtc: today, monitored: true, hasFile: false, posterUrl: null,
        },
      ],
    })
    await page.goto('/calendar')
    await expect(page.getByText('Today')).toBeVisible()
  })
})
