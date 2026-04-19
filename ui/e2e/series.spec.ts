import { test, expect } from './fixtures'

test.describe('Series list page', () => {
  test('displays series from the library', async ({ mockPage: page }) => {
    await page.goto('/series')
    await expect(page.getByText('Breaking Bad')).toBeVisible()
    await expect(page.getByText('The Office')).toBeVisible()
  })

  test('filters series by search input', async ({ mockPage: page }) => {
    await page.goto('/series')
    await page.getByPlaceholder('Filter series...').fill('Breaking')
    await expect(page.getByText('Breaking Bad')).toBeVisible()
    await expect(page.getByText('The Office')).not.toBeVisible()
  })

  test('shows empty state when filter matches nothing', async ({ mockPage: page }) => {
    await page.goto('/series')
    await page.getByPlaceholder('Filter series...').fill('zzz_no_match')
    await expect(page.getByText('Breaking Bad')).not.toBeVisible()
    await expect(page.getByText('The Office')).not.toBeVisible()
  })

  test('has Add Series button', async ({ mockPage: page }) => {
    await page.goto('/series')
    await expect(page.getByRole('button', { name: /Add Series/i })).toBeVisible()
  })

  test('can switch between library and browse views', async ({ mockPage: page }) => {
    await page.goto('/series')
    // Default is library — filter input visible
    await expect(page.getByPlaceholder('Filter series...')).toBeVisible()

    // Switch to browse
    await page.getByRole('button', { name: 'Browse' }).click()
    // Filter input should be hidden in browse view
    await expect(page.getByPlaceholder('Filter series...')).not.toBeVisible()

    // Switch back
    await page.getByRole('button', { name: 'Library' }).click()
    await expect(page.getByPlaceholder('Filter series...')).toBeVisible()
  })

  test('opens add series modal', async ({ mockPage: page }) => {
    await page.goto('/series')
    await page.getByRole('button', { name: /Add Series/i }).click()
    // Modal should appear with a search input for looking up series
    await expect(page.getByPlaceholder(/search/i)).toBeVisible()
  })
})
