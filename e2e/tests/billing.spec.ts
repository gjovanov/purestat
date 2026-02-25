import { test, expect } from '@playwright/test';
import {
  generateUser,
  registerAndLogin,
  login,
  createOrg,
  type TestUser,
} from './helpers';

test.describe('Billing', () => {
  let user: TestUser;
  let orgId: string;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    user = await registerAndLogin(page);
    orgId = await createOrg(page, `Billing Org ${Date.now()}`);
    await page.close();
  });

  test.beforeEach(async ({ page }) => {
    await login(page, user.email, user.password);
    await page.goto(`/org/${orgId}/billing`);
    await page.waitForLoadState('networkidle');
  });

  test('should show current plan as free', async ({ page }) => {
    await expect(
      page.getByText(/free|starter|current plan/i).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should display plan options', async ({ page }) => {
    // Should show Free, Pro, and Business plan names
    await expect(page.getByText(/free/i).first()).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(/pro/i).first()).toBeVisible({ timeout: 10_000 });
    await expect(page.getByText(/business/i).first()).toBeVisible({ timeout: 10_000 });
  });

  test('should show usage information', async ({ page }) => {
    // Usage section shows pageviews, sites, or team members
    await expect(
      page.getByText(/pageviews|sites|team members|usage/i).first()
    ).toBeVisible({ timeout: 10_000 });
  });
});
