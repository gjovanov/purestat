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

    await expect(page.getByText(siteDomain)).toBeVisible();
    expect(siteId).toBeTruthy();
  });

  test('should navigate to dashboard for the site', async ({ page }) => {
    if (!siteId) {
      siteId = await createSite(page, orgId, siteDomain);
    }

    await page.goto(`/org/${orgId}/site/${siteId}`);
    await page.waitForLoadState('networkidle');

    await expect(page).toHaveURL(new RegExp(`/org/${orgId}/site/${siteId}`));
    // Dashboard should show the site domain or some analytics content
    await expect(
      page.getByText(new RegExp(siteDomain.replace(/\./g, '\\.'), 'i'))
        .or(page.getByText(/dashboard|analytics|visitors/i).first())
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should update site settings', async ({ page }) => {
    if (!siteId) {
      siteId = await createSite(page, orgId, siteDomain);
    }

    await page.goto(`/org/${orgId}/site/${siteId}/settings`);
    await page.waitForLoadState('networkidle');

    // Update the timezone
    const timezoneSelect = page.getByLabel(/timezone/i).first();
    if (await timezoneSelect.isVisible()) {
      await timezoneSelect.click();
      await page.getByRole('option', { name: /utc|europe|america/i }).first().click();
    }

    await page.getByRole('button', { name: /save|update/i }).click();

    await expect(
      page.getByText(/saved|updated|success/i).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should show tracking code snippet', async ({ page }) => {
    if (!siteId) {
      siteId = await createSite(page, orgId, siteDomain);
    }

    await page.goto(`/org/${orgId}/site/${siteId}/settings`);
    await page.waitForLoadState('networkidle');

    // Look for snippet/tracking code section
    await expect(
      page.getByText(/tracking|snippet|script/i).first()
    ).toBeVisible({ timeout: 10_000 });

    // Should contain a code block with script tag
    await expect(
      page.locator('code, pre, [data-testid="tracking-snippet"]').first()
    ).toBeVisible();
  });

  test('should delete site', async ({ page }) => {
    // Create a disposable site
    const disposableDomain = `delete-${Date.now()}.example.com`;
    const disposableId = await createSite(page, orgId, disposableDomain);

    await page.goto(`/org/${orgId}/site/${disposableId}/settings`);
    await page.waitForLoadState('networkidle');

    await page.getByRole('button', { name: /delete/i }).click();

    // Confirm deletion
    const confirmButton = page.getByRole('button', { name: /confirm|delete|yes/i }).last();
    await confirmButton.click();

    // Should redirect to sites list
    await page.waitForURL(new RegExp(`/org/${orgId}/sites`), { timeout: 10_000 });

    // Verify the site is gone
    await expect(page.getByText(disposableDomain)).not.toBeVisible();
  });
});
