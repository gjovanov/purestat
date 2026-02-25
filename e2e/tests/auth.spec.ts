import { test, expect } from '@playwright/test';
import { generateUser, registerAndLogin, login, logout, type TestUser } from './helpers';

test.describe('Authentication', () => {
  let registeredUser: TestUser;

  test('should show landing page', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await expect(page).toHaveURL('/');
    // Landing page should have some branding or call-to-action
    await expect(page.getByText(/purestat|analytics|get started/i).first()).toBeVisible();
  });

  test('should navigate to register page', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await page.getByRole('link', { name: /register|sign up|get started/i }).first().click();

    await page.waitForURL('**/register');
    await expect(page).toHaveURL(/\/register/);
    await expect(page.getByRole('button', { name: /register|sign up/i })).toBeVisible();
  });

  test('should register a new user', async ({ page }) => {
    registeredUser = await registerAndLogin(page);

    await expect(page).toHaveURL(/\/orgs/);
  });

  test('should login with registered user', async ({ page }) => {
    // Register a user first so we have valid credentials
    const user = generateUser();
    await registerAndLogin(page, user);

    // Logout
    await logout(page);

    // Login again
    await login(page, user.email, user.password);

    await expect(page).toHaveURL(/\/orgs/);
  });

  test('should show error for invalid credentials', async ({ page }) => {
    await page.goto('/login');
    await page.waitForLoadState('networkidle');

    await page.getByLabel('Email').fill('nonexistent@purestat.test');
    await page.getByLabel('Password').fill('WrongPassword123!');

    await page.getByRole('button', { name: /log\s*in|sign\s*in/i }).click();

    // Should show an error message and stay on login page
    await expect(
      page.getByText(/invalid|incorrect|wrong|not found|unauthorized/i).first()
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
