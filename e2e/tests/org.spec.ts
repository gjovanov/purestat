import { test, expect } from '@playwright/test';
import { generateUser, registerAndLogin, createOrg, type TestUser } from './helpers';

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
    const { login } = await import('./helpers');
    await login(page, user.email, user.password);
  });

  test('should create a new organization', async ({ page }) => {
    orgId = await createOrg(page, orgName);

    await expect(page.getByText(orgName)).toBeVisible();
    expect(orgId).toBeTruthy();
  });

  test('should navigate to org sites page', async ({ page }) => {
    // Create org if not yet created
    if (!orgId) {
      orgId = await createOrg(page, orgName);
    }

    await page.goto(`/org/${orgId}/sites`);
    await page.waitForLoadState('networkidle');

    await expect(page).toHaveURL(new RegExp(`/org/${orgId}/sites`));
    await expect(page.getByText(/sites/i).first()).toBeVisible();
  });

  test('should update org settings', async ({ page }) => {
    if (!orgId) {
      orgId = await createOrg(page, orgName);
    }

    await page.goto(`/org/${orgId}/settings`);
    await page.waitForLoadState('networkidle');

    const updatedName = `${orgName} Updated`;
    const nameInput = page.getByLabel(/name/i).first();
    await nameInput.clear();
    await nameInput.fill(updatedName);

    await page.getByRole('button', { name: /save|update/i }).click();

    // Verify the update was successful
    await expect(
      page.getByText(/saved|updated|success/i).first()
    ).toBeVisible({ timeout: 10_000 });
  });

  test('should delete org', async ({ page }) => {
    // Create a disposable org for deletion
    const disposableName = `Delete Me ${Date.now()}`;
    const disposableId = await createOrg(page, disposableName);

    await page.goto(`/org/${disposableId}/settings`);
    await page.waitForLoadState('networkidle');

    // Click delete button
    await page.getByRole('button', { name: /delete/i }).click();

    // Confirm deletion in dialog
    const confirmButton = page.getByRole('button', { name: /confirm|delete|yes/i }).last();
    await confirmButton.click();

    // Should redirect to orgs list
    await page.waitForURL('**/orgs', { timeout: 10_000 });

    // Verify the org is no longer in the list
    await expect(page.getByText(disposableName)).not.toBeVisible();
  });
});
