import { test, expect } from '@playwright/test';
import {
  generateUser,
  registerAndLogin,
  login,
  createOrg,
  createSite,
  type TestUser,
} from './helpers';

test.describe('Sites', () => {
  let user: TestUser;
  let orgId: string;
  let siteId: string;
  const orgName = `Site Test Org ${Date.now()}`;
  const siteDomain = `site-${Date.now()}.example.com`;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    user = await registerAndLogin(page);
    orgId = await createOrg(page, orgName);
    await page.close();
  });

  test.beforeEach(async ({ page }) => {
    await login(page, user.email, user.password);
  });

  test('should create a new site', async ({ page }) => {
    siteId = await createSite(page, orgId, siteDomain);

    // After creation, we're on the dashboard — domain appears in heading
    await expect(page.getByRole('heading', { name: siteDomain })).toBeVisible();
    expect(siteId).toBeTruthy();
  });

  test('should navigate to dashboard for the site', async ({ page }) => {
    if (!siteId) {
      siteId = await createSite(page, orgId, siteDomain);
    }

    await page.goto(`/org/${orgId}/site/${siteId}`);
    await page.waitForLoadState('networkidle');

    await expect(page).toHaveURL(new RegExp(`/org/${orgId}/site/${siteId}`));
    // Dashboard shows the site domain in the heading
    await expect(page.getByRole('heading').first()).toBeVisible({ timeout: 10_000 });
  });

  test('should show tracking code snippet', async ({ page }) => {
    if (!siteId) {
      siteId = await createSite(page, orgId, siteDomain);
    }

    await page.goto(`/org/${orgId}/site/${siteId}/settings`);
    await page.waitForLoadState('networkidle');

    // Look for tracking code section
    await expect(
      page.getByText(/tracking code|tracking/i).first()
    ).toBeVisible({ timeout: 10_000 });

    // Should contain a code block with the script tag
    await expect(
      page.locator('code, pre').first()
    ).toBeVisible();
  });

  test('should show site settings form', async ({ page }) => {
    if (!siteId) {
      siteId = await createSite(page, orgId, siteDomain);
    }

    await page.goto(`/org/${orgId}/site/${siteId}/settings`);
    await page.waitForLoadState('networkidle');

    // Settings form should have site name and domain fields
    await expect(page.getByLabel('Site name')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByLabel('Domain')).toBeVisible();
    await expect(page.getByRole('button', { name: /save/i })).toBeVisible();
  });

  test('should show danger zone with delete option', async ({ page }) => {
    if (!siteId) {
      siteId = await createSite(page, orgId, siteDomain);
    }

    await page.goto(`/org/${orgId}/site/${siteId}/settings`);
    await page.waitForLoadState('networkidle');

    await expect(
      page.getByText(/danger zone/i)
    ).toBeVisible({ timeout: 10_000 });
    await expect(
      page.getByRole('button', { name: /delete site/i })
    ).toBeVisible();
  });
});
