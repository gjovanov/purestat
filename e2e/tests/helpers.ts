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

  // Vuetify text fields use the label as accessible name
  await page.getByLabel('Email').fill(u.email);
  await page.getByLabel('Username').fill(u.username);
  await page.getByLabel('Display name').fill(u.displayName);
  await page.getByLabel('Password', { exact: true }).fill(u.password);

  await page.getByRole('button', { name: /create account|register|sign up/i }).click();

  await page.waitForURL('**/orgs', { timeout: 15_000 });
  await expect(page).toHaveURL(/\/orgs/);

  return u;
}

export async function login(page: Page, email: string, password: string): Promise<void> {
  await page.goto('/login');
  await page.waitForLoadState('networkidle');

  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password', { exact: true }).fill(password);

  await page.getByRole('button', { name: /sign in|log\s*in/i }).click();

  await page.waitForURL('**/orgs', { timeout: 15_000 });
  await expect(page).toHaveURL(/\/orgs/);
}

export async function createOrg(page: Page, name: string): Promise<string> {
  await page.goto('/orgs');
  await page.waitForLoadState('networkidle');

  // Two "Create Organization" buttons may exist (header + empty state)
  await page.getByRole('button', { name: /create organization/i }).first().click();

  // Wait for dialog to appear, then fill the org name
  const nameInput = page.getByLabel('Organization name');
  await nameInput.waitFor({ state: 'visible', timeout: 5_000 });
  await nameInput.fill(name);

  // Click the dialog's "Create" submit button (not "Create Organization")
  await page.getByRole('button', { name: /^create$/i }).click();

  // After creation, the app navigates to /org/:orgId/sites
  await page.waitForURL(/\/org\/[^/]+\/sites/, { timeout: 15_000 });

  // Extract orgId from the URL
  const url = page.url();
  const orgId = url.match(/\/org\/([^/]+)/)?.[1] ?? '';

  return orgId;
}

export async function createSite(page: Page, orgId: string, domain: string): Promise<string> {
  await page.goto(`/org/${orgId}/sites`);
  await page.waitForLoadState('networkidle');

  // Two "Add Site" buttons may exist (header + empty state)
  await page.getByRole('button', { name: /add site/i }).first().click();

  // Wait for dialog, fill domain
  const domainInput = page.getByLabel('Domain');
  await domainInput.waitFor({ state: 'visible', timeout: 5_000 });
  await domainInput.fill(domain);

  // Fill site name
  const siteNameInput = page.getByLabel('Site name');
  if (await siteNameInput.isVisible({ timeout: 1000 }).catch(() => false)) {
    await siteNameInput.fill(`${domain} site`);
  }

  // Click the dialog's "Create" submit button
  await page.getByRole('button', { name: /^create$/i }).click();

  // After creation, the app navigates to /org/:orgId/site/:siteId (dashboard)
  await page.waitForURL(/\/org\/[^/]+\/site\/[^/]+/, { timeout: 15_000 });

  // Extract siteId from the URL
  const url = page.url();
  const siteId = url.match(/\/site\/([^/]+)/)?.[1] ?? '';

  return siteId;
}

export async function logout(page: Page): Promise<void> {
  // Click user avatar/menu button
  const userMenu = page.locator('button:has(.v-avatar)').first();
  if (await userMenu.isVisible({ timeout: 3000 }).catch(() => false)) {
    await userMenu.click();
  }

  await page.getByText(/log\s*out|sign\s*out/i).first().click();

  await page.waitForURL('**/login', { timeout: 10_000 });
  await expect(page).toHaveURL(/\/login/);
}
