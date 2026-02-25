import { test, expect } from '@playwright/test';
import {
  generateUser,
  registerAndLogin,
  login,
  createOrg,
  type TestUser,
} from './helpers';

test.describe('Members', () => {
  let user: TestUser;
  let orgId: string;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    user = await registerAndLogin(page);
    orgId = await createOrg(page, `Members Org ${Date.now()}`);
    await page.close();
  });

  test.beforeEach(async ({ page }) => {
    await login(page, user.email, user.password);
    await page.goto(`/org/${orgId}/members`);
    await page.waitForLoadState('networkidle');
  });

  test('should show members section', async ({ page }) => {
    // The members page should show the Members heading/section
    await expect(
      page.getByText(/members/i).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should show current user in member list', async ({ page }) => {
    // The current user should appear in the member list (display name in title)
    await expect(
      page.getByText(new RegExp(user.displayName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'i')).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should show invite button', async ({ page }) => {
    // The Invite button should be visible in the header
    await expect(
      page.getByRole('button', { name: /invite/i }).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should open invite dialog', async ({ page }) => {
    // Click the Invite button in the header
    await page.getByRole('button', { name: /invite/i }).first().click();

    // Invite dialog should appear with email input and Create Invite submit
    await expect(
      page.getByLabel(/email/i).first()
    ).toBeVisible({ timeout: 5_000 });

    await expect(
      page.getByRole('button', { name: /create invite/i })
    ).toBeVisible();
  });
});
