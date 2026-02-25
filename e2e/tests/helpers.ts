import { type Page, expect } from '@playwright/test';

let counter = 0;

function nanoid(): string {
  counter++;
  return `${Date.now().toString(36)}${counter.toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

export interface TestUser {
  email: string;
  username: string;
  password: string;
  displayName: string;
}

export function generateUser(): TestUser {
  const id = nanoid();
  return {
    email: `test-${id}@purestat.test`,
    username: `user${id}`,
    password: `P@ssw0rd!${id}`,
    displayName: `Test User ${id}`,
  };
}

export async function registerAndLogin(page: Page, user?: TestUser): Promise<TestUser> {
  const u = user ?? generateUser();

  await page.goto('/register');
  await page.waitForLoadState('networkidle');

  await page.getByLabel('Email').fill(u.email);
  await page.getByLabel('Username').fill(u.username);
  await page.getByLabel('Display name').fill(u.displayName);
  await page.getByLabel('Password', { exact: true }).fill(u.password);
  await page.getByLabel('Confirm password').fill(u.password);

  await page.getByRole('button', { name: /register|sign up/i }).click();

  await page.waitForURL('**/orgs', { timeout: 15_000 });
  await expect(page).toHaveURL(/\/orgs/);

  return u;
}

export async function login(page: Page, email: string, password: string): Promise<void> {
  await page.goto('/login');
  await page.waitForLoadState('networkidle');

  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password').fill(password);

  await page.getByRole('button', { name: /log\s*in|sign\s*in/i }).click();

  await page.waitForURL('**/orgs', { timeout: 15_000 });
  await expect(page).toHaveURL(/\/orgs/);
}

export async function createOrg(page: Page, name: string): Promise<string> {
  await page.goto('/orgs');
  await page.waitForLoadState('networkidle');

  await page.getByRole('button', { name: /create|new|add/i }).click();

  await page.getByLabel('Name').fill(name);
  await page.getByRole('button', { name: /create|save|submit/i }).click();

  // Wait for the org to appear in the list
  await expect(page.getByText(name)).toBeVisible({ timeout: 10_000 });

  // Extract orgId from the URL or the list item link
  const orgLink = page.locator(`a:has-text("${name}")`).first();
  const href = await orgLink.getAttribute('href');
  const orgId = href?.match(/\/org\/([^/]+)/)?.[1] ?? '';

  return orgId;
}

export async function createSite(page: Page, orgId: string, domain: string): Promise<string> {
  await page.goto(`/org/${orgId}/sites`);
  await page.waitForLoadState('networkidle');

  await page.getByRole('button', { name: /create|new|add/i }).click();

  await page.getByLabel(/domain|url|name/i).fill(domain);
  await page.getByRole('button', { name: /create|save|submit/i }).click();

  // Wait for the site to appear in the list
  await expect(page.getByText(domain)).toBeVisible({ timeout: 10_000 });

  // Extract siteId from the list item link
  const siteLink = page.locator(`a:has-text("${domain}")`).first();
  const href = await siteLink.getAttribute('href');
  const siteId = href?.match(/\/site\/([^/]+)/)?.[1] ?? '';

  return siteId;
}

export async function logout(page: Page): Promise<void> {
  // Try user menu / avatar button first, then look for logout
  const userMenu = page.locator('[data-testid="user-menu"], button:has(.v-avatar)').first();
  if (await userMenu.isVisible()) {
    await userMenu.click();
  }

  await page.getByRole('menuitem', { name: /log\s*out|sign\s*out/i })
    .or(page.getByText(/log\s*out|sign\s*out/i))
    .first()
    .click();

  await page.waitForURL('**/login', { timeout: 10_000 });
  await expect(page).toHaveURL(/\/login/);
}
