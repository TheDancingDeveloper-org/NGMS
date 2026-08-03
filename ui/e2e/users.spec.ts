// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

import { test, expect, mockApi, mockUsers, mockInvites } from './fixtures'

test.describe('Users page', () => {
  test('renders page with title', async ({ mockPage: page }) => {
    await page.goto('/users')
    await expect(page.locator('h1, h2').filter({ hasText: 'User Management' })).toBeVisible()
  })

  test('has Create Invite and Add User buttons', async ({ mockPage: page }) => {
    await page.goto('/users')
    await expect(page.getByRole('button', { name: /Create Invite/i })).toBeVisible()
    await expect(page.getByRole('button', { name: /Add User/i })).toBeVisible()
  })

  test('displays users and invite sections', async ({ mockPage: page }) => {
    await page.goto('/users')
    await expect(page.getByRole('heading', { name: 'Users' })).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Invite Codes' })).toBeVisible()
  })

  test('shows user display names', async ({ mockPage: page }) => {
    await page.goto('/users')
    // Use exact matching to avoid matching @username too
    await expect(page.locator('.font-medium').filter({ hasText: 'Admin' }).first()).toBeVisible()
    await expect(page.locator('.font-medium').filter({ hasText: 'Viewer' }).first()).toBeVisible()
  })

  test('shows usernames with @ prefix', async ({ mockPage: page }) => {
    await page.goto('/users')
    await expect(page.getByText('@admin')).toBeVisible()
    await expect(page.getByText('@viewer')).toBeVisible()
  })

  test('shows role badges', async ({ mockPage: page }) => {
    await page.goto('/users')
    // Role badge spans contain the role text
    const adminBadge = page.locator('span').filter({ hasText: 'admin' }).first()
    await expect(adminBadge).toBeVisible()
  })

  test('shows active and disabled status', async ({ mockPage: page }) => {
    await page.goto('/users')
    await expect(page.getByText('Active').first()).toBeVisible()
    // 'Disabled' appears both as display name and status - target the status span specifically
    await expect(page.locator('span.text-red-400').filter({ hasText: 'Disabled' })).toBeVisible()
  })

  test('displays invite codes', async ({ mockPage: page }) => {
    await page.goto('/users')
    await expect(page.getByText('INVITE-ABC-123')).toBeVisible()
    await expect(page.getByText('INVITE-DEF-456')).toBeVisible()
  })

  test('shows invite status (Available vs Claimed)', async ({ mockPage: page }) => {
    await page.goto('/users')
    await expect(page.getByText('Available')).toBeVisible()
    await expect(page.getByText('Claimed')).toBeVisible()
  })

  test('opens Create User modal', async ({ mockPage: page }) => {
    await page.goto('/users')
    await page.getByRole('button', { name: /Add User/i }).click()
    await expect(page.getByRole('heading', { name: 'Create User' })).toBeVisible()
    // The modal uses label elements with text, not htmlFor
    await expect(page.locator('label').filter({ hasText: 'Username' })).toBeVisible()
    await expect(page.locator('label').filter({ hasText: 'Password' })).toBeVisible()
  })

  test('Create User modal has role selector', async ({ mockPage: page }) => {
    await page.goto('/users')
    await page.getByRole('button', { name: /Add User/i }).click()
    await expect(page.locator('label').filter({ hasText: 'Role' })).toBeVisible()
    // The modal has a select with user/admin options
    const modal = page.locator('.fixed')
    await expect(modal.locator('option', { hasText: 'User' })).toBeAttached()
    await expect(modal.locator('option', { hasText: 'Admin' })).toBeAttached()
  })

  test('opens Create Invite modal', async ({ mockPage: page }) => {
    await page.goto('/users')
    await page.getByRole('button', { name: /Create Invite/i }).click()
    await expect(page.getByRole('heading', { name: 'Create Invite Code' })).toBeVisible()
    await expect(page.locator('label').filter({ hasText: 'Role' })).toBeVisible()
  })

  test('shows empty state when no users', async ({ page }) => {
    await mockApi(page)
    await page.route('**/api/v1/admin/users', (route) =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify([]) }),
    )
    await page.goto('/users')
    await expect(page.getByText('No users yet')).toBeVisible()
  })

  test('shows empty state when no invites', async ({ page }) => {
    await mockApi(page)
    await page.route('**/api/v1/admin/invites', (route) =>
      route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify([]) }),
    )
    await page.goto('/users')
    await expect(page.getByText('No invite codes')).toBeVisible()
  })
})
