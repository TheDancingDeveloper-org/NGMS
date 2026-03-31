/**
 * Live E2E: Download client CRUD (J75-80)
 * Runs serially against real StackArr instance.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe.configure({ mode: 'serial' })
test.describe('Live: Download Client CRUD', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let clientId: number | null = null

  // J79: Embedded engine status
  test('J79: list download clients', async ({ page }) => {
    const r = await page.request.get('/api/v1/downloadclient')
    expect(r.ok()).toBeTruthy()
    const clients = await r.json()
    expect(Array.isArray(clients)).toBeTruthy()
    // Embedded clients (id=-1, -2) may or may not appear depending on engine init
  })

  // J75: Add an external download client
  test('J75: add external download client', async ({ page }) => {
    const r = await page.request.post('/api/v1/downloadclient', {
      data: {
        name: 'Test qBittorrent',
        clientType: 'qbittorrent',
        protocol: 'torrent',
        config: { host: 'localhost', port: 8080, username: 'admin', password: 'admin' },
        enabled: true,
        priority: 1,
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    clientId = body.id
    expect(clientId).toBeTruthy()
    expect(body.name).toBe('Test qBittorrent')
  })

  // J76: Test download client connection
  test('J76: test download client connection', async ({ page }) => {
    expect(clientId).toBeTruthy()
    const r = await page.request.post(`/api/v1/downloadclient/${clientId}/test`)
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.success).toBe(true)
  })

  // J77: Edit download client
  test('J77: edit download client', async ({ page }) => {
    expect(clientId).toBeTruthy()
    const r = await page.request.put(`/api/v1/downloadclient/${clientId}`, {
      data: { name: 'Test qBittorrent (Edited)', priority: 5 },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.name).toBe('Test qBittorrent (Edited)')
    expect(body.priority).toBe(5)
  })

  // J80: Set download client priority
  test('J80: set download client priority', async ({ page }) => {
    expect(clientId).toBeTruthy()
    const r = await page.request.put(`/api/v1/downloadclient/${clientId}`, {
      data: { priority: 10 },
    })
    expect(r.ok()).toBeTruthy()
    expect((await r.json()).priority).toBe(10)
  })

  // J78: Delete download client
  test('J78: delete download client', async ({ page }) => {
    expect(clientId).toBeTruthy()
    const r = await page.request.delete(`/api/v1/downloadclient/${clientId}`)
    expect(r.status()).toBe(204)
  })

  // Download clients UI
  test('J75: download clients settings UI has Add Client button', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Download Clients/i }).click()
    await expect(page.getByRole('button', { name: /Add Client/i })).toBeVisible()
  })

  test('J75: add client form opens with correct fields', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: /Download Clients/i }).click()
    await page.getByRole('button', { name: /Add Client/i }).click()
    await expect(page.getByText(/Add Download Client/i)).toBeVisible()
    await expect(page.getByLabel('Name')).toBeVisible()
    await expect(page.getByLabel('Type')).toBeVisible()
  })
})
