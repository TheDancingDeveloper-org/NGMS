// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect } from './fixtures'

test.describe('App navigation', () => {
  test('redirects / to /discover', async ({ mockPage: page }) => {
    await page.goto('/')
    await expect(page).toHaveURL(/\/discover/)
  })

  test('renders sidebar with expected sections', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await expect(page.getByRole('link', { name: 'TV' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Movies' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Discover' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Queue' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'Settings' })).toBeVisible()
    await expect(page.getByRole('link', { name: 'History' })).toBeVisible()
  })

  test('navigates to series page via sidebar', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await page.getByRole('link', { name: 'TV' }).click()
    await expect(page).toHaveURL(/\/series/)
    await expect(page.getByRole('heading', { name: 'Series' })).toBeVisible()
  })

  test('navigates to movies page via sidebar', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await page.getByRole('link', { name: 'Movies' }).click()
    await expect(page).toHaveURL(/\/movies/)
  })

  test('navigates to queue page', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await page.getByRole('link', { name: 'Queue' }).click()
    await expect(page).toHaveURL(/\/queue/)
  })

  test('navigates to settings page', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await page.getByRole('link', { name: 'Settings' }).click()
    await expect(page).toHaveURL(/\/settings/)
  })

  test('shows NGMS header branding', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await expect(page.getByRole('heading', { name: 'NGMS' })).toBeVisible()
  })

  test('shows version in header', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await expect(page.getByText('v0.1.0-test')).toBeVisible()
  })
})
