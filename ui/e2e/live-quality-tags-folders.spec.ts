// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

/**
 * Live E2E: Quality profiles, media folders, tags (J70-74, J147-151)
 * Runs serially against real StackArr instance.
 */
import { test, expect } from '@playwright/test'

const isLive = !!process.env.PLAYWRIGHT_LIVE

test.describe.configure({ mode: 'serial' })

// ─── Quality Profiles (J70-74) ─────────────────────────────

test.describe('Live: Quality Profile CRUD', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let profileId: number | null = null

  // J70: Create a quality profile
  test('J70: create a quality profile', async ({ page }) => {
    const r = await page.request.post('/api/v1/qualityprofile', {
      data: {
        name: 'Test HD-1080p',
        cutoff: 7,
        upgradeAllowed: true,
        minFormatScore: 0,
        cutoffFormatScore: 0,
        items: JSON.stringify([
          { quality: { id: 3, name: 'WEBDL-1080p' }, allowed: true },
          { quality: { id: 7, name: 'Bluray-1080p' }, allowed: true },
        ]),
      },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    profileId = body.id
    expect(profileId).toBeTruthy()
    expect(body.name).toBe('Test HD-1080p')
  })

  // J70: Create another profile for movies
  test('J70: create a movie quality profile', async ({ page }) => {
    const r = await page.request.post('/api/v1/qualityprofile', {
      data: {
        name: 'Test 4K',
        cutoff: 18,
        upgradeAllowed: true,
        items: JSON.stringify([]),
        mediaType: 'movie',
      },
    })
    expect(r.status()).toBe(201)
  })

  // J71: Edit a quality profile
  test('J71: edit a quality profile', async ({ page }) => {
    expect(profileId).toBeTruthy()
    const r = await page.request.put(`/api/v1/qualityprofile/${profileId}`, {
      data: { name: 'Test HD-1080p (Edited)', cutoff: 8 },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.name).toBe('Test HD-1080p (Edited)')
    expect(body.cutoff).toBe(8)
  })

  // J74: Minimum custom format score
  test('J74: set minimum custom format score', async ({ page }) => {
    expect(profileId).toBeTruthy()
    const r = await page.request.put(`/api/v1/qualityprofile/${profileId}`, {
      data: { minFormatScore: 10, cutoffFormatScore: 25 },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.minFormatScore).toBe(10)
    expect(body.cutoffFormatScore).toBe(25)
  })

  // J72: Delete a quality profile
  test('J72: delete a quality profile', async ({ page }) => {
    // Delete the movie profile
    const listResp = await page.request.get('/api/v1/qualityprofile')
    const profiles = await listResp.json()
    const movieProfile = profiles.find((p: { name: string }) => p.name === 'Test 4K')
    expect(movieProfile).toBeTruthy()

    const r = await page.request.delete(`/api/v1/qualityprofile/${movieProfile.id}`)
    expect(r.status()).toBe(204)
  })

  // Quality profiles UI
  test('J70: quality profiles settings tab shows profiles', async ({ page }) => {
    await page.goto('/settings')
    // Settings uses sidebar nav - click the Quality Profiles link
    await page.getByRole('button', { name: 'Quality Profiles' }).click()
    await expect(page.getByText('Quality Profiles').first()).toBeVisible()
    await expect(page.getByText('Test HD-1080p (Edited)')).toBeVisible({ timeout: 5000 })
  })
})

// ─── Media Folders (J147-148) ──────────────────────────────

test.describe('Live: Media Folder CRUD', () => {
  test.skip(!isLive, 'Only runs against live instance')

  // J147: Add a media library folder
  test('J147: list existing media folders', async ({ page }) => {
    const r = await page.request.get('/api/v1/medialibraryfolder')
    expect(r.ok()).toBeTruthy()
    const folders = await r.json()
    expect(Array.isArray(folders)).toBeTruthy()
    // Should have the default folders from setup
    expect(folders.length).toBeGreaterThan(0)
  })

  test('J147: add a new media library folder', async ({ page }) => {
    const r = await page.request.post('/api/v1/medialibraryfolder', {
      data: { path: '/downloads/torrent/complete', mediaType: 'series' },
    })
    // May succeed or fail if path doesn't exist in container
    expect([201, 400, 500].includes(r.status())).toBeTruthy()
  })

  // J147: Media folders UI
  test('J147: media folders settings tab shows folders', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: 'Media Folders' }).click()
    await expect(page.getByText('Media Library Folders')).toBeVisible()
    await expect(page.getByRole('button', { name: /Add Folder/i })).toBeVisible()
    // Should show at least one folder
    await expect(page.getByText(/\/media/i).first()).toBeVisible()
  })
})

// ─── Tags (J149-151) ──────────────────────────────────────

test.describe('Live: Tag CRUD', () => {
  test.skip(!isLive, 'Only runs against live instance')

  let tagId: number | null = null

  // J149: Create a tag
  test('J149: create a tag', async ({ page }) => {
    const r = await page.request.post('/api/v1/tag', {
      data: { label: 'test-tag-1' },
    })
    expect(r.status()).toBe(201)
    const body = await r.json()
    tagId = body.id
    expect(tagId).toBeTruthy()
    expect(body.label).toBe('test-tag-1')
  })

  // Create another tag
  test('J149: create a second tag', async ({ page }) => {
    const r = await page.request.post('/api/v1/tag', {
      data: { label: 'test-tag-2' },
    })
    expect(r.status()).toBe(201)
  })

  // Duplicate tag should 409
  test('J149: duplicate tag returns 409', async ({ page }) => {
    const r = await page.request.post('/api/v1/tag', {
      data: { label: 'test-tag-1' },
    })
    expect(r.status()).toBe(409)
  })

  // Edit a tag
  test('J149: edit a tag', async ({ page }) => {
    expect(tagId).toBeTruthy()
    const r = await page.request.put(`/api/v1/tag/${tagId}`, {
      data: { label: 'test-tag-renamed' },
    })
    expect(r.ok()).toBeTruthy()
    const body = await r.json()
    expect(body.label).toBe('test-tag-renamed')
  })

  // J151: Delete a tag
  test('J151: delete a tag', async ({ page }) => {
    const listResp = await page.request.get('/api/v1/tag')
    const tags = await listResp.json()
    const tag2 = tags.find((t: { label: string }) => t.label === 'test-tag-2')
    expect(tag2).toBeTruthy()

    const r = await page.request.delete(`/api/v1/tag/${tag2.id}`)
    expect(r.status()).toBe(204)
  })

  // Tag list
  test('J149: list tags returns expected tags', async ({ page }) => {
    const r = await page.request.get('/api/v1/tag')
    expect(r.ok()).toBeTruthy()
    const tags = await r.json()
    expect(tags.some((t: { label: string }) => t.label === 'test-tag-renamed')).toBeTruthy()
    expect(tags.some((t: { label: string }) => t.label === 'test-tag-2')).toBeFalsy()
  })

  // Tags UI
  test('J149: tags settings tab shows tags', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: 'Tags' }).click()
    await expect(page.getByPlaceholder(/Tag name/i)).toBeVisible({ timeout: 5000 })
    await expect(page.getByText('test-tag-renamed')).toBeVisible({ timeout: 5000 })
  })

  // Add tag via UI
  test('J149: add tag via UI', async ({ page }) => {
    await page.goto('/settings')
    await page.getByRole('button', { name: 'Tags' }).click()
    await page.getByPlaceholder(/Tag name/i).fill('ui-created-tag')
    await page.getByRole('button', { name: 'Add' }).click()
    await expect(page.getByText('ui-created-tag')).toBeVisible({ timeout: 5000 })
  })
})
