// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

/**
 * Live E2E tests — run against real StackArr on Node B.
 * Only executed when PLAYWRIGHT_LIVE=1 (npm run test:e2e:live).
 * These tests skip when running in mocked mode.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe('Live: StackArr on Node B', () => {
  test.skip(!isLive, 'Only runs against live instance')

  test('health endpoint responds', async ({ page }) => {
    const response = await page.request.get('/health')
    expect(response.ok()).toBeTruthy()
    const body = await response.json()
    expect(body.status).toBe('ok')
  })

  test('system status returns valid response', async ({ page }) => {
    const response = await page.request.get('/api/v1/system/status')
    expect(response.ok()).toBeTruthy()
    const status = await response.json()
    expect(status.version).toBeTruthy()
    expect(status).toHaveProperty('firstBoot')
    expect(status).toHaveProperty('modules')
  })

  test('app loads and shows UI', async ({ page }) => {
    await page.goto('/')
    // Should show either the main app, first-boot setup, or connection page
    await expect(
      page.getByText(/NGMS|Setup|Connect/i).first(),
    ).toBeVisible({ timeout: 10_000 })
  })

  test('first boot shows setup page', async ({ page }) => {
    const response = await page.request.get('/api/v1/system/status')
    const status = await response.json()

    if (status.firstBoot) {
      await page.goto('/')
      await expect(page).toHaveURL(/\/setup/, { timeout: 10_000 })
    } else {
      test.skip()
    }
  })

  test('after setup, navigates to discover', async ({ page }) => {
    const response = await page.request.get('/api/v1/system/status')
    const status = await response.json()

    if (!status.firstBoot) {
      await page.goto('/')
      await expect(page).toHaveURL(/\/discover/, { timeout: 10_000 })
    } else {
      test.skip()
    }
  })
})
