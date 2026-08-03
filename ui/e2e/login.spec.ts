// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test as base, expect } from '@playwright/test'

// Login page is shown when authMethod is "forms" and no valid session exists.

base.describe('Login page', () => {
  base.beforeEach(async ({ page }) => {
    // Catch-all for API requests
    await page.route('**/api/v1/**', (route) =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify([]) }),
    )

    // System status with forms auth enabled
    await page.route('**/api/v1/system/status', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          version: '0.1.0-test',
          instanceName: 'Test NGMS',
          firstBoot: false,
          authMethod: 'forms',
          modules: {
            tvManagement: true, movieManagement: true, torrentEmbedded: false,
            usenetEmbedded: false, torrentExternal: false, usenetExternal: false,
            indexarrSidecar: false, externalIndexers: false, plexIntegration: false,
            notifications: false, streaming: false, remoteAccess: false, stremioAddon: false,
          },
          indexarrAvailable: false,
          startTime: '2026-03-30T00:00:00Z',
        }),
      }),
    )

    // Auth check returns 401 (not logged in)
    await page.route('**/api/v1/auth/me', (route) =>
      route.fulfill({ status: 401, contentType: 'application/json', body: JSON.stringify({ error: 'not authenticated' }) }),
    )
  })

  base.test('renders login form with instance name', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByText('Sign in to continue')).toBeVisible()
    await expect(page.getByRole('heading', { name: /Test NGMS|NGMS/ })).toBeVisible()
  })

  base.test('has username and password fields', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByLabel('Username')).toBeVisible()
    await expect(page.getByLabel('Password')).toBeVisible()
  })

  base.test('has sign in button', async ({ page }) => {
    await page.goto('/')
    await expect(page.getByRole('button', { name: /sign in/i })).toBeVisible()
  })

  base.test('shows error on failed login', async ({ page }) => {
    await page.route('**/api/v1/auth/login', (route) =>
      route.fulfill({ status: 401, contentType: 'application/json', body: JSON.stringify({ error: 'Invalid credentials' }) }),
    )
    await page.goto('/')
    await page.getByLabel('Username').fill('baduser')
    await page.getByLabel('Password').fill('badpass')
    await page.getByRole('button', { name: /sign in/i }).click()
    // Should show some error indication
    await expect(page.locator('[class*="red"]').first()).toBeVisible()
  })

  base.test('button shows loading state during submission', async ({ page }) => {
    // Make the login request hang
    await page.route('**/api/v1/auth/login', () => new Promise(() => {}))
    await page.goto('/')
    await page.getByLabel('Username').fill('admin')
    await page.getByLabel('Password').fill('password')
    await page.getByRole('button', { name: /sign in/i }).click()
    await expect(page.getByRole('button', { name: /signing in/i })).toBeVisible()
    await expect(page.getByRole('button', { name: /signing in/i })).toBeDisabled()
  })
})
