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

  test('should create a custom event goal', async ({ page }) => {
    const goalName = `signup-click-${Date.now()}`;

    await page.getByRole('button', { name: /create|add|new/i }).first().click();

    // Select custom event type
    await page.getByText(/custom event/i).first().click();

    // Fill in the event name
    await page.getByLabel(/event name|name/i).first().fill(goalName);

    await page.getByRole('button', { name: /create|save|submit/i }).click();

    // Verify the goal appears in the list
    await expect(page.getByText(goalName)).toBeVisible({ timeout: 10_000 });
  });

  test('should create a pageview goal', async ({ page }) => {
    const pagePath = `/thank-you-${Date.now()}`;

    await page.getByRole('button', { name: /create|add|new/i }).first().click();

    // Select pageview type
    await page.getByText(/pageview|page visit/i).first().click();

    // Fill in the page path
    await page.getByLabel(/path|page|url/i).first().fill(pagePath);

    await page.getByRole('button', { name: /create|save|submit/i }).click();

    // Verify the goal appears in the list
    await expect(page.getByText(pagePath)).toBeVisible({ timeout: 10_000 });
  });

  test('should delete a goal', async ({ page }) => {
    // Create a goal to delete
    const goalName = `delete-me-${Date.now()}`;

    await page.getByRole('button', { name: /create|add|new/i }).first().click();
    await page.getByText(/custom event/i).first().click();
    await page.getByLabel(/event name|name/i).first().fill(goalName);
    await page.getByRole('button', { name: /create|save|submit/i }).click();

    await expect(page.getByText(goalName)).toBeVisible({ timeout: 10_000 });

    // Find the delete button for this goal
    const goalRow = page.locator(`text=${goalName}`).locator('..');
    const deleteButton = goalRow.getByRole('button', { name: /delete|remove/i })
      .or(goalRow.locator('[data-testid*="delete"]'))
      .or(goalRow.locator('button').last());

    await deleteButton.click();

    // Confirm deletion if dialog appears
    const confirmButton = page.getByRole('button', { name: /confirm|delete|yes/i });
    if (await confirmButton.isVisible({ timeout: 2_000 }).catch(() => false)) {
      await confirmButton.click();
    }

    // Verify the goal is gone
    await expect(page.getByText(goalName)).not.toBeVisible({ timeout: 10_000 });
  });
});
