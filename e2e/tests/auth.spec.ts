import { test, expect } from '@playwright/test';
import { generateUser, registerAndLogin, login, logout, type TestUser } from './helpers';

test.describe('Authentication', () => {
  test('should show landing page', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await expect(page).toHaveURL('/');
    await expect(page.getByText(/purestat|pure.*clean.*honest/i).first()).toBeVisible();
  });

  test('should navigate to register page', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await page.getByRole('link', { name: /get started/i }).first().click();

    await page.waitForURL('**/register');
    await expect(page).toHaveURL(/\/register/);
    await expect(page.getByRole('button', { name: /create account/i })).toBeVisible();
  });

  test('should register a new user', async ({ page }) => {
    await registerAndLogin(page);
    await expect(page).toHaveURL(/\/orgs/);
  });

  test('should login with registered user', async ({ page }) => {
    const user = generateUser();
    await registerAndLogin(page, user);
    await logout(page);
    await login(page, user.email, user.password);
    await expect(page).toHaveURL(/\/orgs/);
  });

  test('should show error for invalid credentials', async ({ page }) => {
    await page.goto('/login');
    await page.waitForLoadState('networkidle');

    await page.getByLabel('Email').fill('nonexistent@purestat.test');
    await page.getByLabel('Password', { exact: true }).fill('WrongPassword123!');

    await page.getByRole('button', { name: /sign in/i }).click();

    // Should show an error message or stay on login page
    await expect(
      page.getByText(/invalid|incorrect|wrong|not found|failed/i).first()
    ).toBeVisible({ timeout: 10_000 });
    await expect(page).toHaveURL(/\/login/);
  });

  test('should logout and redirect to login', async ({ page }) => {
    const user = generateUser();
    await registerAndLogin(page, user);
    await logout(page);
    await expect(page).toHaveURL(/\/login/);
  });
});
