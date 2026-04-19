import { test, expect } from './fixtures'

test.describe('Discover page', () => {
  test('renders trending section', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await expect(page.getByText('Trending Today')).toBeVisible()
  })

  test('renders popular movies section', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await expect(page.getByText('Popular Movies')).toBeVisible()
  })

  test('renders popular tv section', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await expect(page.getByText('Popular TV')).toBeVisible()
  })

  test('renders hero banner with trending item', async ({ mockPage: page }) => {
    await page.goto('/discover')
    await expect(page.getByRole('heading', { name: 'Inception' })).toBeVisible()
  })
})
