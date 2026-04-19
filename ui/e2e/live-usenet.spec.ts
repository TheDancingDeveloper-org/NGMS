/**
 * Live E2E: Usenet server CRUD + downloads (J12-25)
 * Runs serially against real StackArr instance.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe.configure({ mode: 'serial' })
test.describe('Live: Usenet Server CRUD', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let addedServerId: number | null = null

  // J12: Add a usenet server
  test('J12: add a usenet server via UI', async ({ page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Servers' }).click()
    await page.getByRole('button', { name: /Add Server/i }).click()

    // Fill server form (uses placeholder text, not <label>)
    await page.getByPlaceholder('e.g. Eweka, Newshosting').fill('Test Frugal')
    await page.getByPlaceholder('news.example.com').fill('aunews.frugalusenet.com')
    // Port should default to 563, SSL should be checked

    await page.getByRole('button', { name: /Save/i }).click()
    await page.waitForTimeout(2000)

    // Server should appear in the list
    await expect(page.getByText('Test Frugal').first()).toBeVisible({ timeout: 5000 })
  })

  // J12: Also add via API and capture the ID
  test('J12: add a second server via API', async ({ page }) => {
    const r = await page.request.post('/api/v1/usenet/servers', {
      data: {
        name: 'Test Backup',
        host: 'news.example.com',
        port: 563,
        ssl: true,
        username: 'testuser',
        password: 'testpass',
        connections: 4,
        priority: 5,
        enabled: true,
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    addedServerId = body.dbId
    expect(addedServerId).toBeTruthy()
  })

  // J13: Test server connection
  test('J13: test a usenet server connection', async ({ page }) => {
    const r = await page.request.post('/api/v1/usenet/servers/test', {
      data: {
        host: 'aunews.frugalusenet.com',
        port: 563,
        ssl: true,
        username: 'admin',
        password: '3MemP7tRt',
        connections: 2,
      },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('success')
  })

  // J14: Edit a usenet server
  test('J14: edit a usenet server via API', async ({ page }) => {
    expect(addedServerId).toBeTruthy()
    const r = await page.request.put(`/api/v1/usenet/servers/${addedServerId}`, {
      data: { name: 'Test Backup (Edited)', connections: 8 },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.name).toBe('Test Backup (Edited)')
  })

  // J15: Disable/re-enable a usenet server
  test('J15: disable and re-enable a server', async ({ page }) => {
    expect(addedServerId).toBeTruthy()
    // Disable
    let r = await page.request.put(`/api/v1/usenet/servers/${addedServerId}`, {
      data: { enabled: false },
    })
    expect(r.ok()).toBeTruthy()
    let body = await r.json()
    expect(body.enabled).toBe(false)

    // Re-enable
    r = await page.request.put(`/api/v1/usenet/servers/${addedServerId}`, {
      data: { enabled: true },
    })
    expect(r.ok()).toBeTruthy()
    body = await r.json()
    expect(body.enabled).toBe(true)
  })

  // J16: Delete a usenet server
  test('J16: delete a usenet server', async ({ page }) => {
    expect(addedServerId).toBeTruthy()
    const r = await page.request.delete(`/api/v1/usenet/servers/${addedServerId}`)
    expect(r.status()).toBe(204)

    // Verify it's gone
    const listResp = await page.request.get('/api/v1/usenet/servers')
    const list = await listResp.json()
    const found = list.servers?.find((s: { dbId: number }) => s.dbId === addedServerId)
    expect(found).toBeUndefined()
  })

  // J25: Usenet engine settings
  test('J25: get and update usenet engine settings', async ({ page }) => {
    // GET
    const getResp = await page.request.get('/api/v1/usenet/settings')
    expect(getResp.ok()).toBeTruthy()
    const settings = await getResp.json()
    expect(settings).toHaveProperty('maxActiveDownloads')
    expect(settings).toHaveProperty('speedLimit')

    // PUT
    const putResp = await page.request.put('/api/v1/usenet/settings', {
      data: { maxActiveDownloads: 5 },
    })
    expect(putResp.ok()).toBeTruthy()
    const updated = await putResp.json()
    expect(updated.maxActiveDownloads).toBe(5)

    // Restore
    await page.request.put('/api/v1/usenet/settings', {
      data: { maxActiveDownloads: 3 },
    })
  })

  // J25: Settings UI
  test('J25: usenet settings UI renders fields', async ({ page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: 'Settings' }).click()
    await expect(page.getByText('Max Active Downloads')).toBeVisible()
    await expect(page.getByText('Speed Limit')).toBeVisible()
    await expect(page.getByText('History Retention')).toBeVisible()
    await expect(page.getByRole('heading', { name: 'Directories' })).toBeVisible()
  })
})

test.describe('Live: Usenet Downloads', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J22: Pause all / J23: Resume all
  test('J22-23: pause and resume all downloads', async ({ page }) => {
    const pauseResp = await page.request.post('/api/v1/usenet/pause-all', {
      data: {},
    })
    expect(pauseResp.ok()).toBeTruthy()

    // Check status shows paused
    const statusResp = await page.request.get('/api/v1/usenet/status')
    const status = await statusResp.json()
    expect(status.paused).toBe(true)

    // Resume
    const resumeResp = await page.request.post('/api/v1/usenet/resume-all')
    expect(resumeResp.ok()).toBeTruthy()
  })

  // J24: Set speed limit
  test('J24: set usenet speed limit', async ({ page }) => {
    const r = await page.request.post('/api/v1/usenet/speed-limit', {
      data: { bytesPerSecond: 10_485_760 }, // 10 MB/s
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.success).toBe(true)

    // Remove limit
    await page.request.post('/api/v1/usenet/speed-limit', {
      data: { bytesPerSecond: 0 },
    })
  })

  // J17: Add NZB by URL (smoke test — URL will fail but API should accept)
  test('J17: add NZB by URL API accepts request', async ({ page }) => {
    const r = await page.request.post('/api/v1/usenet/add', {
      data: {
        url: 'https://example.com/test.nzb',
        name: 'test-download',
        category: '',
      },
    })
    // URL is fake so fetch may fail upstream. Accept any outcome that means
    // the endpoint is wired up: 200 (queued), 4xx/5xx (bad url / upstream error).
    expect([200, 400, 500, 502].includes(r.status())).toBeTruthy()
  })

  // J17: Add NZB UI modal
  test('J17: add NZB modal has correct fields', async ({ page }) => {
    await page.goto('/usenet')
    await page.getByRole('button', { name: /Add NZB/i }).click()
    await expect(page.getByText('Add NZB').first()).toBeVisible()
    await expect(page.getByPlaceholder(/example\.com.*nzb|nzb.*url/i)).toBeVisible()
    await expect(page.getByRole('button', { name: /Cancel/i })).toBeVisible()
  })
})
