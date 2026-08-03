// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect, mockApi, mockLogResponse } from './fixtures'

test.describe('Logs page', () => {
  test('renders page with title and entry count', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await expect(page.locator('h1, h2').filter({ hasText: 'Logs' })).toBeVisible()
    // Entry count badge is inside the header area
    const badge = page.locator('.rounded-full').filter({ hasText: String(mockLogResponse.entries.length) })
    await expect(badge).toBeVisible()
  })

  test('displays log entries with levels', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await expect(page.getByText('Server started on port 8989')).toBeVisible()
    await expect(page.getByText('Connection refused')).toBeVisible()
  })

  test('shows level badges', async ({ mockPage: page }) => {
    await page.goto('/logs')
    // Each level badge is a span with the level text
    await expect(page.locator('span').filter({ hasText: 'INFO' }).first()).toBeVisible()
    await expect(page.locator('span').filter({ hasText: 'WARN' }).first()).toBeVisible()
    await expect(page.locator('span').filter({ hasText: 'ERROR' }).first()).toBeVisible()
  })

  test('shows target column', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await expect(page.getByTitle('stackarr_web')).toBeVisible()
    await expect(page.getByTitle('stackarr_indexer')).toBeVisible()
  })

  test('has level filter dropdown', async ({ mockPage: page }) => {
    await page.goto('/logs')
    const select = page.locator('select')
    await expect(select).toBeVisible()
    await expect(select.locator('option', { hasText: 'All Levels' })).toBeAttached()
    await expect(select.locator('option', { hasText: 'Error' })).toBeAttached()
    await expect(select.locator('option', { hasText: 'Warn' })).toBeAttached()
    await expect(select.locator('option', { hasText: 'Info' })).toBeAttached()
    await expect(select.locator('option', { hasText: 'Debug' })).toBeAttached()
    await expect(select.locator('option', { hasText: 'Trace' })).toBeAttached()
  })

  test('level filter narrows entries', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await page.locator('select').selectOption('ERROR')
    await expect(page.getByText('Connection refused')).toBeVisible()
    await expect(page.getByText('Server started on port 8989')).not.toBeVisible()
  })

  test('has target filter input', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await expect(page.getByPlaceholder(/filter target/i)).toBeVisible()
  })

  test('target filter narrows entries', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await page.getByPlaceholder(/filter target/i).fill('stackarr_web')
    await expect(page.getByText('Server started on port 8989')).toBeVisible()
    await expect(page.getByText('Indexer NZBGeek')).not.toBeVisible()
  })

  test('has search input', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await expect(page.getByPlaceholder(/search messages/i)).toBeVisible()
  })

  test('search filters by message text', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await page.getByPlaceholder(/search messages/i).fill('Connection')
    await expect(page.getByText('Connection refused')).toBeVisible()
    await expect(page.getByText('Server started')).not.toBeVisible()
  })

  test('has pause/resume button', async ({ mockPage: page }) => {
    await page.goto('/logs')
    const pauseBtn = page.getByRole('button', { name: 'Pause' })
    await expect(pauseBtn).toBeVisible()
    await pauseBtn.click()
    await expect(page.getByRole('button', { name: 'Resume' })).toBeVisible()
  })

  test('shows empty state when no entries match filter', async ({ mockPage: page }) => {
    await page.goto('/logs')
    await page.getByPlaceholder(/search messages/i).fill('zzz_no_match_ever')
    await expect(page.getByText(/no log entries/i)).toBeVisible()
  })

  test('shows empty state with no entries', async ({ page }) => {
    await mockApi(page)
    await page.route('**/api/v1/log**', (route) =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ entries: [], latestSeq: 0 }) }),
    )
    await page.goto('/logs')
    await expect(page.getByText(/no log entries/i)).toBeVisible()
  })
})
