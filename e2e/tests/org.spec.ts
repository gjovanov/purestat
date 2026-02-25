import { test, expect } from '@playwright/test';
import { generateUser, registerAndLogin, createOrg, login, type TestUser } from './helpers';

test.describe('Organizations', () => {
  let user: TestUser;
  let orgId: string;
  const orgName = `Test Org ${Date.now()}`;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    user = await registerAndLogin(page);
    await page.close();
  });

  test.beforeEach(async ({ page }) => {
    await login(page, user.email, user.password);
  });

  test('should create a new organization', async ({ page }) => {
    orgId = await createOrg(page, orgName);

    // After creation, we're redirected to /org/:orgId/sites
    await expect(page).toHaveURL(/\/org\/[^/]+\/sites/);
    expect(orgId).toBeTruthy();
  });

  test('should navigate to org sites page', async ({ page }) => {
    if (!orgId) {
      orgId = await createOrg(page, orgName);
    }

    await page.goto(`/org/${orgId}/sites`);
    await page.waitForLoadState('networkidle');

    await expect(page).toHaveURL(new RegExp(`/org/${orgId}/sites`));
    await expect(page.getByText(/sites/i).first()).toBeVisible();
  });

  test('should show org settings page', async ({ page }) => {
    if (!orgId) {
      orgId = await createOrg(page, orgName);
    }

    await page.goto(`/org/${orgId}/settings`);
    await page.waitForLoadState('networkidle');

    // Should show the org name field
    await expect(page.getByLabel('Organization name')).toBeVisible({ timeout: 10_000 });
    await expect(page.getByRole('button', { name: /save/i })).toBeVisible();
  });

  test('should show danger zone in org settings', async ({ page }) => {
    if (!orgId) {
      orgId = await createOrg(page, orgName);
    }

    await page.goto(`/org/${orgId}/settings`);
    await page.waitForLoadState('networkidle');

    await expect(page.getByText(/danger zone/i)).toBeVisible({ timeout: 10_000 });
    await expect(
      page.getByRole('button', { name: /delete organization/i })
    ).toBeVisible();
  });
});
