import { test, expect } from './fixtures'

test.describe('Settings page', () => {
  test('renders settings page with General tab', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await expect(page.getByText('General Settings')).toBeVisible()
  })

  test('shows instance name field', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await expect(page.getByText('Instance Name')).toBeVisible()
  })

  test('can navigate to Download Clients tab', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Download Clients/i }).click()
    await expect(page.getByText('Embedded Torrent Client')).toBeVisible()
    await expect(page.getByText('Embedded Usenet Client')).toBeVisible()
  })

  test('embedded clients show BUILT-IN badge', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Download Clients/i }).click()
    await expect(page.getByText('BUILT-IN').first()).toBeVisible()
  })

  test('has Add Client button on download clients tab', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Download Clients/i }).click()
    await expect(page.getByRole('button', { name: /Add Client/i })).toBeVisible()
  })

  test('can navigate to Quality Profiles tab', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Quality/i }).click()
    await expect(page.getByText('HD-1080p')).toBeVisible()
  })

  test('can navigate to Media Folders tab', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Media Folders/i }).click()
    await expect(page.getByText('/tv')).toBeVisible()
    await expect(page.getByText('/movies')).toBeVisible()
  })

  test('can navigate to Naming tab', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Naming/i }).click()
    await expect(page.getByText(/Episode Formats/i)).toBeVisible()
  })

  test('can navigate to Modules tab', async ({ mockPage: page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Modules/i }).click()
    await expect(page.getByText(/TV Management|Movie Management/i)).toBeVisible()
  })
})
