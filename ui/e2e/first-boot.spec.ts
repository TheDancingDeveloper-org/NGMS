// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect, mockApi, mockStatus } from './fixtures'

test.describe('First boot setup', () => {
  test('redirects to /setup when firstBoot is true', async ({ page }) => {
    await mockApi(page, { status: { firstBoot: true } })
    await page.goto('/')
    await expect(page).toHaveURL(/\/setup/)
  })

  test('does not redirect to /setup when firstBoot is false', async ({ page }) => {
    await mockApi(page)
    await page.goto('/')
    await expect(page).toHaveURL(/\/discover/)
  })
})
