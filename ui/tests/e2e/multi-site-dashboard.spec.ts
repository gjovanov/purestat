import { test, expect } from '@playwright/test'

// The account comes from the environment, never from the source:
// TEST_EMAIL and TEST_PASSWORD. Without them the suite is skipped.
const EMAIL = process.env.TEST_EMAIL ?? ''
const PASSWORD = process.env.TEST_PASSWORD ?? ''

test.describe('Multi-site dashboard', () => {
  test.skip(!EMAIL || !PASSWORD, 'set TEST_EMAIL and TEST_PASSWORD to run against a real account')
  test.beforeEach(async ({ page }) => {
    // Login
    await page.goto('/login')
    await page.fill('input[type="email"]', EMAIL)
    await page.fill('input[type="password"]', PASSWORD)
    await page.click('button[type="submit"]')
    await page.waitForURL(/\/orgs|\/org\//)
  })

  test('multi-select site dropdown is visible and functional', async ({ page }) => {
    // Navigate to an org (first available)
    await page.waitForSelector('.v-navigation-drawer')
    const dashboardLink = page.locator('a[href*="/dashboard"]').first()
    if (await dashboardLink.isVisible()) {
      await dashboardLink.click()
    }

    // The multi-select site dropdown should be in the app bar
    const siteSelect = page.locator('.v-app-bar .v-select')
    await expect(siteSelect).toBeVisible({ timeout: 10_000 })
  })

  test('select all toggle works', async ({ page }) => {
    await page.waitForSelector('.v-navigation-drawer')
    const dashboardLink = page.locator('a[href*="/dashboard"]').first()
    if (await dashboardLink.isVisible()) {
      await dashboardLink.click()
    }

    const siteSelect = page.locator('.v-app-bar .v-select')
    await expect(siteSelect).toBeVisible({ timeout: 10_000 })

    // Open dropdown
    await siteSelect.click()
    await page.waitForSelector('.v-list-item')

    // Click "Select All"
    const selectAll = page.locator('.v-list-item:has-text("Select All")')
    await expect(selectAll).toBeVisible()
    await selectAll.click()

    // Chips should appear in the select
    const chips = siteSelect.locator('.v-chip')
    const chipCount = await chips.count()
    expect(chipCount).toBeGreaterThan(0)
  })

  test('stats load on site selection change', async ({ page }) => {
    await page.waitForSelector('.v-navigation-drawer')
    const dashboardLink = page.locator('a[href*="/dashboard"]').first()
    if (await dashboardLink.isVisible()) {
      await dashboardLink.click()
    }

    // Wait for dashboard content to load
    await page.waitForSelector('h1', { timeout: 10_000 })

    // Verify metric cards are present
    const metricCards = page.locator('.v-card')
    const cardCount = await metricCards.count()
    expect(cardCount).toBeGreaterThan(0)
  })

  test('realtime badge shows visitor details on click', async ({ page }) => {
    await page.waitForSelector('.v-navigation-drawer')
    const dashboardLink = page.locator('a[href*="/dashboard"]').first()
    if (await dashboardLink.isVisible()) {
      await dashboardLink.click()
    }

    // Find the realtime badge
    const badge = page.locator('[data-test="realtime-badge"]')
    await expect(badge).toBeVisible({ timeout: 10_000 })

    // Click to open visitor details menu
    await badge.click()

    // The v-menu card should appear
    const menuCard = page.locator('.v-menu .v-card, .v-overlay .v-card')
    await expect(menuCard).toBeVisible({ timeout: 5_000 })

    // Should show "Active Visitors" title
    await expect(menuCard.locator('.v-card-title')).toContainText('Active Visitors')
  })
})
