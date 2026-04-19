import { test, expect } from '@playwright/test'

test.describe('Connection error state', () => {
  test('shows error UI when API is unreachable', async ({ page }) => {
    // Block all API calls to simulate backend being down
    await page.route('**/api/v1/**', (route) =>
      route.abort('connectionrefused'),
    )
    await page.goto('/')
    await expect(page.getByText(/unable to connect/i)).toBeVisible()
    await expect(page.getByRole('button', { name: /retry/i })).toBeVisible()
    await expect(page.getByRole('button', { name: /connect to server/i })).toBeVisible()
  })
})
