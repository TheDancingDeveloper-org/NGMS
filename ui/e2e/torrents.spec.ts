import { test, expect } from './fixtures'

test.describe('Torrents page', () => {
  test('renders dashboard header with speed labels', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await expect(page.getByText('Download').first()).toBeVisible()
    await expect(page.getByText('Upload').first()).toBeVisible()
  })

  test('displays torrent in list', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await expect(page.getByText('Ubuntu.24.04.iso')).toBeVisible()
  })

  test('has Add Torrent button', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await expect(page.getByRole('button', { name: /Add Torrent/i })).toBeVisible()
  })

  test('has filter input', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await expect(page.getByPlaceholder(/filter/i)).toBeVisible()
  })

  test('filter hides non-matching torrents', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await page.getByPlaceholder(/filter/i).fill('zzz_no_match')
    await expect(page.getByText('Ubuntu.24.04.iso')).not.toBeVisible()
  })

  test('has Torrents and Settings tabs', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await expect(page.getByRole('button', { name: 'Torrents' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Settings' })).toBeVisible()
  })

  test('opens add torrent modal', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await page.getByRole('button', { name: /Add Torrent/i }).click()
    // Modal should have a URL/magnet input
    await expect(page.locator('input[placeholder*="magnet"],input[placeholder*="Magnet"],input[placeholder*="URL"],input[placeholder*="url"],input[placeholder*="http"]')).toBeVisible()
  })
})

test.describe('Torrent settings tab', () => {
  test('displays speed limit section', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('Speed Limits')).toBeVisible()
    await expect(page.getByText('Download Limit')).toBeVisible()
    await expect(page.getByText('Upload Limit')).toBeVisible()
  })

  test('displays connection settings', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('Connection Settings')).toBeVisible()
    await expect(page.getByText('Max Peers per Torrent')).toBeVisible()
  })

  test('shows DHT status', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('DHT')).toBeVisible()
    await expect(page.getByText('Enabled')).toBeVisible()
  })

  test('shows directories section', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await page.getByRole('button', { name: 'Settings' }).click()
    const dirHeader = page.getByText('Directories')
    await dirHeader.scrollIntoViewIfNeeded()
    await expect(dirHeader).toBeVisible()
    await expect(page.getByText('Download Directory')).toBeVisible()
  })

  test('switching tabs preserves content', async ({ mockPage: page }) => {
    await page.goto('/torrents')
    await expect(page.getByText('Ubuntu.24.04.iso')).toBeVisible()
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('Speed Limits')).toBeVisible()
    await expect(page.getByText('Ubuntu.24.04.iso')).not.toBeVisible()
    await page.getByRole('button', { name: 'Torrents' }).click()
    await expect(page.getByText('Ubuntu.24.04.iso')).toBeVisible()
  })
})
