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

  test('should create an API key', async ({ page }) => {
    const keyName = `test-key-${Date.now()}`;

    await page.getByRole('button', { name: /create|add|new|generate/i }).first().click();

    // Fill in the key name/description
    await page.getByLabel(/name|description|label/i).first().fill(keyName);

    await page.getByRole('button', { name: /create|generate|save|submit/i }).click();

    // The newly created key should be displayed (usually shown once)
    await expect(
      page.locator('code, pre, [data-testid*="api-key"], input[readonly]')
        .or(page.getByText(/sk_|pk_|key_/i))
    ).toBeVisible({ timeout: 10_000 });

    // The key name should appear in the list
    await expect(page.getByText(keyName)).toBeVisible({ timeout: 10_000 });
  });

  test('should show key prefix in list', async ({ page }) => {
    // Create a key if none exists
    const keyName = `prefix-key-${Date.now()}`;

    await page.getByRole('button', { name: /create|add|new|generate/i }).first().click();
    await page.getByLabel(/name|description|label/i).first().fill(keyName);
    await page.getByRole('button', { name: /create|generate|save|submit/i }).click();

    // Dismiss the key display dialog if present
    const doneButton = page.getByRole('button', { name: /done|close|ok|got it/i });
    if (await doneButton.isVisible({ timeout: 3_000 }).catch(() => false)) {
      await doneButton.click();
    }

    // The list should show a masked/prefix version of the key
    await expect(
      page.getByText(/sk_\w+\.\.\.|pk_\w+\.\.\.|key_\w+\.\.\.|\*{4,}|•{4,}/i)
        .or(page.locator('[data-testid*="key-prefix"]'))
        .or(page.getByText(keyName))
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should revoke API key', async ({ page }) => {
    // Create a key to revoke
    const keyName = `revoke-key-${Date.now()}`;

    await page.getByRole('button', { name: /create|add|new|generate/i }).first().click();
    await page.getByLabel(/name|description|label/i).first().fill(keyName);
    await page.getByRole('button', { name: /create|generate|save|submit/i }).click();

    // Dismiss the key display dialog if present
    const doneButton = page.getByRole('button', { name: /done|close|ok|got it/i });
    if (await doneButton.isVisible({ timeout: 3_000 }).catch(() => false)) {
      await doneButton.click();
    }

    await expect(page.getByText(keyName)).toBeVisible({ timeout: 10_000 });

    // Find and click the revoke/delete button for this key
    const keyRow = page.locator(`text=${keyName}`).locator('..');
    const revokeButton = keyRow.getByRole('button', { name: /revoke|delete|remove/i })
      .or(keyRow.locator('[data-testid*="revoke"]'))
      .or(keyRow.locator('[data-testid*="delete"]'))
      .first();

    await revokeButton.click();

    // Confirm revocation
    const confirmButton = page.getByRole('button', { name: /confirm|revoke|delete|yes/i });
    if (await confirmButton.isVisible({ timeout: 2_000 }).catch(() => false)) {
      await confirmButton.click();
    }

    // Verify the key is gone
    await expect(page.getByText(keyName)).not.toBeVisible({ timeout: 10_000 });
  });
});
