import { test, expect, mockApi } from './fixtures'

test.describe('Queue page', () => {
  test('displays active downloads', async ({ mockPage: page }) => {
    await page.goto('/queue')
    await expect(page.getByText('Breaking.Bad.S01E01.720p.BluRay.x264')).toBeVisible()
  })

  test('shows download progress', async ({ mockPage: page }) => {
    await page.goto('/queue')
    // The progress percentage or bar should be present
    await expect(page.getByText(/45/)).toBeVisible()
  })

  test('shows empty queue message when no downloads', async ({ page }) => {
    await mockApi(page, { queue: [] })
    await page.goto('/queue')
    // Should show some kind of empty state
    await expect(page.getByText(/no.*download|queue.*empty|nothing/i)).toBeVisible()
  })
})
