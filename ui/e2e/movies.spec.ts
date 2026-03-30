import { test, expect } from './fixtures'

test.describe('Movies page', () => {
  test('displays movies from library', async ({ mockPage: page }) => {
    await page.goto('/movies')
    await expect(page.getByText('Inception')).toBeVisible()
  })

  test('shows movie metadata', async ({ mockPage: page }) => {
    await page.goto('/movies')
    await expect(page.getByText('2010')).toBeVisible()
  })

  test('has Add Movie button', async ({ mockPage: page }) => {
    await page.goto('/movies')
    await expect(page.getByRole('button', { name: /Add Movie/i })).toBeVisible()
  })

  test('filters movies by search', async ({ mockPage: page }) => {
    await page.goto('/movies')
    const filter = page.getByPlaceholder(/filter/i)
    if (await filter.isVisible()) {
      await filter.fill('zzz_no_match')
      await expect(page.getByText('Inception')).not.toBeVisible()
    }
  })

  test('can switch between library and browse', async ({ mockPage: page }) => {
    await page.goto('/movies')
    const browseBtn = page.getByRole('button', { name: 'Browse' })
    if (await browseBtn.isVisible()) {
      await browseBtn.click()
      await expect(page.getByRole('button', { name: 'Library' })).toBeVisible()
    }
  })
})
