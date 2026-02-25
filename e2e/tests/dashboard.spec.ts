import { test, expect } from '@playwright/test';
import {
  generateUser,
  registerAndLogin,
  login,
  createOrg,
  createSite,
  type TestUser,
} from './helpers';

test.describe('Dashboard', () => {
  let user: TestUser;
  let orgId: string;
  let siteId: string;

  test.beforeAll(async ({ browser }) => {
    const page = await browser.newPage();
    user = await registerAndLogin(page);
    orgId = await createOrg(page, `Dashboard Org ${Date.now()}`);
    siteId = await createSite(page, orgId, `dash-${Date.now()}.example.com`);
    await page.close();
  });

  test.beforeEach(async ({ page }) => {
    await login(page, user.email, user.password);
    await page.goto(`/org/${orgId}/site/${siteId}`);
    await page.waitForLoadState('networkidle');
  });

  test('should show dashboard with metric cards', async ({ page }) => {
    // Dashboard should display metric cards for key stats
    const metricCards = page.locator(
      '.v-card, [data-testid*="metric"], [data-testid*="stat"]'
    );

    // Expect at least some metric cards (visitors, pageviews, bounce rate, etc.)
    await expect(metricCards.first()).toBeVisible({ timeout: 10_000 });

    // Check for common analytics metric labels
    const metricLabels = page.getByText(
      /visitors|pageviews|bounce rate|visit duration|unique visitors/i
    );
    await expect(metricLabels.first()).toBeVisible({ timeout: 10_000 });
  });

  test('should show date picker with ranges', async ({ page }) => {
    // Look for the date range picker
    const datePicker = page.locator(
      '[data-testid="date-picker"], [data-testid="date-range"], button:has-text("Today"), button:has-text("Last 7")'
    ).first();

    await expect(datePicker).toBeVisible({ timeout: 10_000 });

    // Click to open date range options
    await datePicker.click();

    // Should show predefined ranges
    await expect(
      page.getByText(/today|yesterday|last 7 days|last 30 days|this month/i).first()
    ).toBeVisible({ timeout: 5_000 });
  });

  test('should show empty state for sources/pages/devices', async ({ page }) => {
    // With a fresh site and no tracking data, sections should show empty states
    const sections = ['sources', 'pages', 'devices', 'countries', 'browsers'];

    for (const section of sections) {
      const sectionEl = page.getByText(new RegExp(section, 'i')).first();
      if (await sectionEl.isVisible({ timeout: 2_000 }).catch(() => false)) {
        // Section exists, check for empty state or zero values
        const emptyOrZero = page.getByText(/no data|no .* yet|0|empty/i);
        // At least one empty indicator should be present somewhere on the page
        await expect(emptyOrZero.first()).toBeVisible({ timeout: 5_000 });
        break;
      }
    }
  });

  test('should switch date ranges', async ({ page }) => {
    // Find and click the date picker
    const datePicker = page.locator(
      '[data-testid="date-picker"], [data-testid="date-range"], button:has-text("Today"), button:has-text("Last 7"), button:has-text("Last 30")'
    ).first();

    await expect(datePicker).toBeVisible({ timeout: 10_000 });
    await datePicker.click();

    // Select "Last 30 days" range
    const last30 = page.getByText(/last 30 days/i).first();
    if (await last30.isVisible({ timeout: 3_000 }).catch(() => false)) {
      await last30.click();

      // Wait for dashboard to refresh
      await page.waitForLoadState('networkidle');

      // Verify the date range changed (the picker label should update)
      await expect(
        page.getByText(/last 30 days|30d/i).first()
      ).toBeVisible({ timeout: 5_000 });
    }

    // Try switching to "Today"
    await datePicker.click();
    const today = page.getByText(/^today$/i).first();
    if (await today.isVisible({ timeout: 3_000 }).catch(() => false)) {
      await today.click();
      await page.waitForLoadState('networkidle');
    }
  });
});
