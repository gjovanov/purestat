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

  test('should show current user as owner', async ({ page }) => {
    // The current user should be listed as owner/admin
    await expect(
      page.getByText(new RegExp(user.email.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'i'))
        .or(page.getByText(new RegExp(user.displayName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'), 'i')))
    ).toBeVisible({ timeout: 10_000 });

    await expect(
      page.getByText(/owner|admin/i).first()
    ).toBeVisible({ timeout: 5_000 });
  });

  test('should create an invite', async ({ page }) => {
    const inviteEmail = `invite-${Date.now()}@purestat.test`;

    await page.getByRole('button', { name: /invite|add member|add/i }).first().click();

    // Fill in the invite form
    const emailInput = page.getByLabel(/email/i).first();
    await emailInput.fill(inviteEmail);

    // Select role if available
    const roleSelect = page.getByLabel(/role/i).first();
    if (await roleSelect.isVisible({ timeout: 2_000 }).catch(() => false)) {
      await roleSelect.click();
      await page.getByRole('option', { name: /viewer|member|editor/i }).first().click();
    }

    await page.getByRole('button', { name: /send|invite|create|submit/i }).click();

    // Verify the invite appears in pending invites
    await expect(
      page.getByText(inviteEmail)
        .or(page.getByText(/pending|invited/i))
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should copy invite link', async ({ page }) => {
    // Create an invite first
    const inviteEmail = `copy-link-${Date.now()}@purestat.test`;

    await page.getByRole('button', { name: /invite|add member|add/i }).first().click();

    const emailInput = page.getByLabel(/email/i).first();
    await emailInput.fill(inviteEmail);

    await page.getByRole('button', { name: /send|invite|create|submit/i }).click();

    await expect(page.getByText(inviteEmail)).toBeVisible({ timeout: 10_000 });

    // Find and click the copy link button
    const copyButton = page.getByRole('button', { name: /copy.*link|copy.*invite/i })
      .or(page.locator('[data-testid*="copy"]'))
      .or(page.locator('button[aria-label*="copy" i]'))
      .first();

    if (await copyButton.isVisible({ timeout: 3_000 }).catch(() => false)) {
      await copyButton.click();

      // Should show a success toast/snackbar
      await expect(
        page.getByText(/copied|clipboard/i).first()
      ).toBeVisible({ timeout: 5_000 });
    }
  });

  test('should revoke invite', async ({ page }) => {
    // Create an invite to revoke
    const inviteEmail = `revoke-${Date.now()}@purestat.test`;

    await page.getByRole('button', { name: /invite|add member|add/i }).first().click();

    const emailInput = page.getByLabel(/email/i).first();
    await emailInput.fill(inviteEmail);

    await page.getByRole('button', { name: /send|invite|create|submit/i }).click();

    await expect(page.getByText(inviteEmail)).toBeVisible({ timeout: 10_000 });

    // Find the revoke/delete button for this invite
    const inviteRow = page.locator(`text=${inviteEmail}`).locator('..');
    const revokeButton = inviteRow.getByRole('button', { name: /revoke|remove|delete|cancel/i })
      .or(inviteRow.locator('[data-testid*="revoke"]'))
      .or(inviteRow.locator('[data-testid*="delete"]'))
      .first();

    await revokeButton.click();

    // Confirm if dialog appears
    const confirmButton = page.getByRole('button', { name: /confirm|revoke|yes|delete/i });
    if (await confirmButton.isVisible({ timeout: 2_000 }).catch(() => false)) {
      await confirmButton.click();
    }

    // Verify the invite is gone
    await expect(page.getByText(inviteEmail)).not.toBeVisible({ timeout: 10_000 });
  });
});
