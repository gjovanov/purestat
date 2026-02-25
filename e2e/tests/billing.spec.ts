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

  test('should display all three plan cards', async ({ page }) => {
    // Should show Free, Pro, and Business plan cards
    const freeCard = page.getByText(/free/i).first();
    const proCard = page.getByText(/pro/i).first();
    const businessCard = page.getByText(/business|enterprise/i).first();

    await expect(freeCard).toBeVisible({ timeout: 10_000 });
    await expect(proCard).toBeVisible({ timeout: 10_000 });
    await expect(businessCard).toBeVisible({ timeout: 10_000 });

    // Each plan should show pricing or features
    const planCards = page.locator('.v-card, [data-testid*="plan"]');
    const cardCount = await planCards.count();
    expect(cardCount).toBeGreaterThanOrEqual(3);
  });

  test('should show usage stats', async ({ page }) => {
    // Should display usage information (pageviews, sites, etc.)
    await expect(
      page.getByText(/usage|pageviews|events|sites/i).first()
    ).toBeVisible({ timeout: 10_000 });

    // Should show some numeric usage data or progress indicator
    await expect(
      page.locator('.v-progress-linear, [data-testid*="usage"], [role="progressbar"]')
        .or(page.getByText(/\d+.*\/.*\d+|\d+\s*(of|out of)\s*\d+/i).first())
        .or(page.getByText(/0.*pageviews|0.*events/i).first())
    ).toBeVisible({ timeout: 10_000 });
  });
});
