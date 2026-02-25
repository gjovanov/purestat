import { test, expect } from '@playwright/test';
import {
  generateUser,
  registerAndLogin,
  login,
  createOrg,
  createSite,
  type TestUser,
} from './helpers';

test.describe('API Keys', () => {
  let user: TestUser;
  let orgId: string;
  let siteId: string;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    user = await registerAndLogin(page);
    orgId = await createOrg(page, `API Keys Org ${Date.now()}`);
    siteId = await createSite(page, orgId, `apikeys-${Date.now()}.example.com`);
    await page.close();
  });

  test.beforeEach(async ({ page }) => {
    await login(page, user.email, user.password);
    await page.goto(`/org/${orgId}/site/${siteId}/api-keys`);
    await page.waitForLoadState('networkidle');
  });

  test('should show empty API keys state', async ({ page }) => {
    await expect(
      page.getByText(/no api keys|no .* yet|create.*key|get started/i).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should show create API key button', async ({ page }) => {
    await expect(
      page.getByRole('button', { name: /create api key/i }).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should open create API key dialog', async ({ page }) => {
    await page.getByRole('button', { name: /create api key/i }).first().click();

    // Dialog should show key name field
    await expect(
      page.getByLabel(/key name/i).first()
    ).toBeVisible({ timeout: 5_000 });

    // Should have a Create submit button
    await expect(
      page.getByRole('button', { name: /^create$/i })
    ).toBeVisible();
  });
});
