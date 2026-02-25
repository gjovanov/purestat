import { test, expect } from '@playwright/test';
import {
  generateUser,
  registerAndLogin,
  login,
  createOrg,
  createSite,
  type TestUser,
} from './helpers';

test.describe('Goals', () => {
  let user: TestUser;
  let orgId: string;
  let siteId: string;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    user = await registerAndLogin(page);
    orgId = await createOrg(page, `Goals Org ${Date.now()}`);
    siteId = await createSite(page, orgId, `goals-${Date.now()}.example.com`);
    await page.close();
  });

  test.beforeEach(async ({ page }) => {
    await login(page, user.email, user.password);
    await page.goto(`/org/${orgId}/site/${siteId}/goals`);
    await page.waitForLoadState('networkidle');
  });

  test('should show empty goals state', async ({ page }) => {
    await expect(
      page.getByText(/no goals|no .* yet|create.*goal|get started/i).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should show create goal button', async ({ page }) => {
    await expect(
      page.getByRole('button', { name: /create goal/i }).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should open create goal dialog', async ({ page }) => {
    await page.getByRole('button', { name: /create goal/i }).first().click();

    // Dialog should show goal name and type fields
    await expect(
      page.getByLabel(/goal name|name/i).first()
    ).toBeVisible({ timeout: 5_000 });

    // Should show goal type dropdown
    await expect(
      page.getByLabel(/goal type|type/i).first()
    ).toBeVisible();

    // Should have a Create submit button
    await expect(
      page.getByRole('button', { name: /^create$/i })
    ).toBeVisible();
  });
});
