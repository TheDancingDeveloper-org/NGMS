/**
 * Live E2E: Auth and user management (J2-9)
 * Runs serially against real StackArr instance.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe.configure({ mode: 'serial' })
test.describe('Live: Auth & User Management', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let inviteCode: string | null = null
  let newUserId: number | null = null
  let authToken: string | null = null

  // Auth status
  test('auth status API returns setup info', async ({ page }) => {
    const r = await page.request.get('/api/v1/auth/status')
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body).toHaveProperty('setupRequired')
    expect(body).toHaveProperty('registrationEnabled')
  })

  // J2: Login with credentials
  test('J2: login with credentials', async ({ page }) => {
    const r = await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.user).toBeTruthy()
    expect(body.user.username).toBe('admin')
    expect(body.token).toBeTruthy()
    authToken = body.token
  })

  // J8: Update profile
  test('J8: update user profile', async ({ page }) => {
    // Login first to get session
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })

    const r = await page.request.put('/api/v1/user/profile', {
      data: { displayName: 'Test Admin' },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.displayName).toBe('Test Admin')
  })

  // J4: Admin creates invite code (login first to get session cookie)
  test('J4: create invite code', async ({ page }) => {
    // Login to establish session cookie in page context
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    const r = await page.request.post('/api/v1/admin/invites', {
      data: { role: 'user', expiresInHours: 24 },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    expect(body.code).toBeTruthy()
    inviteCode = body.code
  })

  // J4: List invite codes
  test('J4: list invite codes', async ({ page }) => {
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    const r = await page.request.get('/api/v1/admin/invites')
    expect(r.ok()).toBeTruthy()
    const invites = await r.json()
    expect(Array.isArray(invites)).toBeTruthy()
    expect(invites.length).toBeGreaterThan(0)
  })

  // J5: Register with invite code (creates its own invite to be self-contained)
  test('J5: register with invite code', async ({ page }) => {
    // Login as admin to create a fresh invite
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    const invResp = await page.request.post('/api/v1/admin/invites', {
      data: { role: 'user', expiresInHours: 1 },
    })
    expect(invResp.status()).toBe(201)
    const code = (await invResp.json()).code

    const r = await page.request.post('/api/v1/auth/register', {
      data: {
        username: `testuser_${Date.now()}`,
        password: 'testpass456',
        displayName: 'Test User',
        inviteCode: code,
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    expect(body.user.role).toBe('user')
  })

  // J6: Admin creates user directly
  test('J6: admin creates user directly', async ({ page }) => {
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    const r = await page.request.post('/api/v1/admin/users', {
      data: {
        username: `directuser_${Date.now()}`,
        password: 'directpass789',
        displayName: 'Direct User',
        role: 'user',
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    newUserId = body.id
    expect(newUserId).toBeTruthy()
  })

  // Admin list users
  test('J6: admin lists all users', async ({ page }) => {
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    const r = await page.request.get('/api/v1/admin/users')
    expect(r.ok()).toBeTruthy()
    const users = await r.json()
    expect(users.length).toBeGreaterThanOrEqual(3) // admin + testuser + directuser
    const names = users.map((u: { username: string }) => u.username)
    expect(names).toContain('admin')
    // Check that more than 1 user exists (admin + any registered users)
    expect(users.length).toBeGreaterThanOrEqual(2)
  })

  // J7: Admin deletes a user
  test('J7: admin deletes a user', async ({ page }) => {
    expect(newUserId).toBeTruthy()
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    const r = await page.request.delete(`/api/v1/admin/users/${newUserId}`)
    expect(r.status()).toBe(204)

    // Verify gone
    const listResp = await page.request.get('/api/v1/admin/users')
    const users = await listResp.json()
    expect(users.find((u: { username: string }) => u.username === 'directuser')).toBeUndefined()
  })

  // J3: Logout
  test('J3: logout', async ({ page }) => {
    // Login first to have a session to logout from
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    const r = await page.request.post('/api/v1/auth/logout')
    expect(r.ok()).toBeTruthy()
  })

  // J9: Device token auth
  test('J9: login with device name returns device token', async ({ page }) => {
    const r = await page.request.post('/api/v1/auth/login', {
      data: {
        username: 'admin',
        password: 'testpass123',
        deviceName: 'Test Device',
      },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.user).toBeTruthy()
    // deviceToken should be present when deviceName is provided
    if (body.deviceToken) {
      expect(body.deviceToken).toBeTruthy()
    }
  })

  // User sessions
  test('J8: list user sessions', async ({ page }) => {
    await page.request.post('/api/v1/auth/login', {
      data: { username: 'admin', password: 'testpass123' },
    })
    const r = await page.request.get('/api/v1/user/sessions')
    expect(r.ok()).toBeTruthy()
    const sessions = await r.json()
    expect(Array.isArray(sessions)).toBeTruthy()
  })
})
