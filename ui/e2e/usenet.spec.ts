import { test, expect } from './fixtures'

test.describe('Usenet page', () => {
  test('renders all tabs', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await expect(page.getByRole('button', { name: 'Queue' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'History' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Servers' })).toBeVisible()
    await expect(page.getByRole('button', { name: 'Settings' })).toBeVisible()
  })

  test('displays queue items', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await expect(page.getByText('Breaking.Bad.S01E01')).toBeVisible()
    await expect(page.getByText('Inception.2010.2160p')).toBeVisible()
  })

  test('shows download speed in header', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await expect(page.getByText('MB/s').first()).toBeVisible()
  })

  test('shows queue stats', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await expect(page.getByText(/Queue.*2/)).toBeVisible()
    await expect(page.getByText(/Active.*1/)).toBeVisible()
  })
})

test.describe('Usenet servers tab', () => {
  test('displays server list', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Servers' }).click()
    await expect(page.getByRole('button', { name: /Add Server/i })).toBeVisible()
    await expect(page.getByText('news.example.com').first()).toBeVisible()
    await expect(page.getByText('backup.example.com').first()).toBeVisible()
  })

  test('has Add Server and Test All buttons', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Servers' }).click()
    await expect(page.getByRole('button', { name: /Add Server/i })).toBeVisible()
    await expect(page.getByRole('button', { name: /Test All/i })).toBeVisible()
  })

  test('opens add server form with fields', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Servers' }).click()
    await expect(page.getByRole('button', { name: /Add Server/i })).toBeVisible()
    await page.getByRole('button', { name: /Add Server/i }).click()
    // The modal should show host and connection fields
    await expect(page.locator('label:has-text("Host")').first()).toBeVisible()
    await expect(page.locator('label:has-text("Connections")').first()).toBeVisible()
  })

  test('shows SSL badge on SSL servers', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Servers' }).click()
    await expect(page.getByText('SSL').first()).toBeVisible()
  })

  test('server cards show connection count', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Servers' }).click()
    await expect(page.getByText('20')).toBeVisible()
  })

  test('each server card has edit and delete buttons', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Servers' }).click()
    await expect(page.getByTitle('Edit').first()).toBeVisible()
    await expect(page.getByTitle('Delete').first()).toBeVisible()
  })
})

test.describe('Usenet settings tab', () => {
  test('displays all settings fields', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('Max Active Downloads')).toBeVisible()
    await expect(page.getByText('Speed Limit')).toBeVisible()
    await expect(page.getByText('History Retention')).toBeVisible()
  })

  test('shows directory paths', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('/downloads/usenet/incomplete')).toBeVisible()
    await expect(page.getByText('/downloads/usenet/complete')).toBeVisible()
  })

  test('max active downloads defaults to 1', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Settings' }).click()
    const input = page.locator('input[type="number"]').first()
    await expect(input).toHaveValue('1')
  })

  test('speed limit dropdown has options', async ({ mockPage: page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.locator('select').first()).toBeVisible()
    await expect(page.locator('option', { hasText: 'Unlimited' })).toBeAttached()
    await expect(page.locator('option', { hasText: '100 MB/s' })).toBeAttached()
  })
})
