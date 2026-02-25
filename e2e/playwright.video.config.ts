import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './video',
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [['list']],
  use: {
    baseURL: process.env.BASE_URL || 'http://localhost:5173',
    video: { mode: 'on', size: { width: 1280, height: 720 } },
    viewport: { width: 1280, height: 720 },
    launchOptions: { slowMo: 80 },
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
