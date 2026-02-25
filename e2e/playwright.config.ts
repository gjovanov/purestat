import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 1,
  workers: 1,
  reporter: [['html'], ['list']],
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:5173',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    video: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: [
    {
      command: 'cargo run -p purestat-api',
      cwd: '..',
      port: 3000,
      reuseExistingServer: true,
      timeout: 120_000,
    },
    {
      command: 'bun run dev',
      cwd: '../ui',
      port: 5173,
      reuseExistingServer: true,
      timeout: 30_000,
    },
  ],
});
