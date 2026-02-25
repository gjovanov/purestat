/**
 * Purestat Intro Video Recording
 *
 * This Playwright test records a full user journey as a video.
 * It injects an on-screen transcription overlay at each scene,
 * creating a narrated walkthrough suitable for a product intro.
 *
 * Run:
 *   cd e2e && bunx playwright test video/record-intro.spec.ts
 *
 * Output:
 *   test-results/record-intro-*/video.webm
 *   Convert to MP4: ffmpeg -i video.webm -c:v libx264 -crf 20 purestat-intro.mp4
 */
import { test, type Page } from '@playwright/test';
import transcriptions from './transcriptions.json';

// ---------------------------------------------------------------------------
// Overlay helpers
// ---------------------------------------------------------------------------

async function injectOverlay(page: Page) {
  await page.evaluate(() => {
    if (document.getElementById('ps-overlay')) return;

    const overlay = document.createElement('div');
    overlay.id = 'ps-overlay';
    Object.assign(overlay.style, {
      position: 'fixed',
      bottom: '40px',
      left: '50%',
      transform: 'translateX(-50%)',
      zIndex: '99999',
      background: 'rgba(15, 23, 42, 0.88)',
      color: '#E2E8F0',
      padding: '16px 32px',
      borderRadius: '12px',
      fontSize: '22px',
      fontFamily: "'Inter', 'Segoe UI', system-ui, sans-serif",
      fontWeight: '500',
      letterSpacing: '0.01em',
      maxWidth: '720px',
      textAlign: 'center',
      backdropFilter: 'blur(8px)',
      border: '1px solid rgba(99, 102, 241, 0.3)',
      boxShadow: '0 8px 32px rgba(0, 0, 0, 0.4)',
      opacity: '0',
      transition: 'opacity 0.4s ease',
      pointerEvents: 'none',
    });
    document.body.appendChild(overlay);
  });
}

async function showCaption(page: Page, text: string) {
  await page.evaluate((t) => {
    const el = document.getElementById('ps-overlay');
    if (!el) return;
    el.textContent = t;
    el.style.opacity = '1';
  }, text);
}

async function hideCaption(page: Page) {
  await page.evaluate(() => {
    const el = document.getElementById('ps-overlay');
    if (el) el.style.opacity = '0';
  });
}

async function caption(page: Page, scene: number) {
  const t = transcriptions.find((s) => s.scene === scene);
  if (!t) return;
  await showCaption(page, t.text);
  await page.waitForTimeout(t.duration);
  await hideCaption(page);
  await page.waitForTimeout(400); // fade-out gap
}

function delay(page: Page, ms: number) {
  return page.waitForTimeout(ms);
}

async function typeSlowly(page: Page, selector: string, text: string, delayMs = 60) {
  const locator = page.locator(selector).first();
  await locator.click();
  for (const char of text) {
    await locator.pressSequentially(char, { delay: delayMs });
  }
}

// ---------------------------------------------------------------------------
// Video recording test
// ---------------------------------------------------------------------------

test.describe('Purestat Intro Video', () => {
  test.use({
    video: { mode: 'on', size: { width: 1280, height: 720 } },
    viewport: { width: 1280, height: 720 },
    launchOptions: { slowMo: 80 },
  });

  test('record full intro walkthrough', async ({ page }) => {
    test.setTimeout(300_000); // 5 minutes max

    await injectOverlay(page);

    // -----------------------------------------------------------------------
    // Scene 1: Landing page
    // -----------------------------------------------------------------------
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await injectOverlay(page);
    await delay(page, 800);
    await caption(page, 1);

    // Scroll through features smoothly
    await page.evaluate(() => window.scrollTo({ top: 500, behavior: 'smooth' }));
    await delay(page, 1500);
    await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'smooth' }));
    await delay(page, 1000);

    // -----------------------------------------------------------------------
    // Scene 2: Register
    // -----------------------------------------------------------------------
    await page.getByRole('link', { name: 'Get Started Free' }).click();
    await page.waitForURL('**/register');
    await page.waitForLoadState('networkidle');
    await injectOverlay(page);
    await delay(page, 500);
    await caption(page, 2);

    // Fill registration form with visible typing
    await typeSlowly(page, 'input[type="email"]', 'demo@purestat.ai');
    await delay(page, 300);
    await typeSlowly(page, 'input:below(:text("Username"))', 'demo');
    await delay(page, 300);
    await typeSlowly(page, 'input:below(:text("Display name"))', 'Alex Demo');
    await delay(page, 300);
    await typeSlowly(page, 'input[type="password"]', 'SecureP@ss123');
    await delay(page, 500);

    await page.getByRole('button', { name: 'Create Account' }).click();
    await page.waitForURL('**/orgs', { timeout: 15_000 });
    await delay(page, 1000);

    // -----------------------------------------------------------------------
    // Scene 3: Create organization
    // -----------------------------------------------------------------------
    await injectOverlay(page);
    await caption(page, 3);

    await page.getByRole('button', { name: /create/i }).click();
    await delay(page, 500);
    await typeSlowly(page, 'input:below(:text("Organization"))', 'Acme Analytics');
    await delay(page, 500);
    await page.getByRole('button', { name: /create/i }).last().click();
    await delay(page, 1500);

    // Navigate to sites
    const orgLink = page.locator('a:has-text("Acme Analytics")').first();
    await orgLink.click();
    await page.waitForLoadState('networkidle');
    await delay(page, 800);

    // -----------------------------------------------------------------------
    // Scene 4: Add site
    // -----------------------------------------------------------------------
    await injectOverlay(page);
    await caption(page, 4);

    await page.getByRole('button', { name: /add site/i }).click();
    await delay(page, 500);
    await typeSlowly(page, 'input:below(:text("Domain"))', 'acme.com');
    await delay(page, 300);

    // Fill site name if visible
    const siteNameInput = page.locator('input:below(:text("Site name"))').first();
    if (await siteNameInput.isVisible({ timeout: 1000 }).catch(() => false)) {
      await typeSlowly(page, 'input:below(:text("Site name"))', 'Acme Website');
      await delay(page, 300);
    }

    await page.getByRole('button', { name: /create|save/i }).last().click();
    await delay(page, 1500);

    // -----------------------------------------------------------------------
    // Scene 5: View tracking code in site settings
    // -----------------------------------------------------------------------
    // Navigate to site settings to show the tracking snippet
    const siteLink = page.locator('a:has-text("acme.com")').first();
    if (await siteLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await siteLink.click();
      await page.waitForLoadState('networkidle');
    }

    // Try navigating to settings via sidebar or direct URL
    const settingsLink = page.locator('a:has-text("Settings")').first();
    if (await settingsLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await settingsLink.click();
      await page.waitForLoadState('networkidle');
    }

    await injectOverlay(page);
    await delay(page, 500);
    await caption(page, 5);

    // Highlight the tracking code snippet
    const codeBlock = page.locator('code').first();
    if (await codeBlock.isVisible({ timeout: 2000 }).catch(() => false)) {
      await codeBlock.scrollIntoViewIfNeeded();
      await delay(page, 1500);
    }

    // Click copy button if available
    const copyBtn = page.getByRole('button', { name: /copy/i }).first();
    if (await copyBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
      await copyBtn.click();
      await delay(page, 1000);
    }

    // -----------------------------------------------------------------------
    // Scene 6: Fast-forward (navigate to dashboard)
    // -----------------------------------------------------------------------
    // Navigate to dashboard
    const dashboardLink = page.locator('a:has-text("Dashboard")').first();
    if (await dashboardLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await dashboardLink.click();
    } else {
      await page.goBack();
      await page.goBack();
    }
    await page.waitForLoadState('networkidle');
    await injectOverlay(page);
    await delay(page, 500);
    await caption(page, 6);

    // -----------------------------------------------------------------------
    // Scene 7: View dashboard
    // -----------------------------------------------------------------------
    await delay(page, 500);
    await caption(page, 7);

    // Hover over metric cards
    const metricCards = page.locator('.v-card').first();
    if (await metricCards.isVisible({ timeout: 2000 }).catch(() => false)) {
      await metricCards.hover();
      await delay(page, 800);
    }

    // Scroll down to show more dashboard content
    await page.evaluate(() => window.scrollTo({ top: 400, behavior: 'smooth' }));
    await delay(page, 1500);

    // -----------------------------------------------------------------------
    // Scene 8: Filters and breakdown
    // -----------------------------------------------------------------------
    await caption(page, 8);

    // Click through date picker options if visible
    const dateBtn = page.getByRole('button', { name: '30D' })
      .or(page.getByText('30D'))
      .first();
    if (await dateBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
      await dateBtn.click();
      await delay(page, 800);
    }

    // Scroll to show sources/pages/locations tables
    await page.evaluate(() => window.scrollTo({ top: 600, behavior: 'smooth' }));
    await delay(page, 1500);
    await page.evaluate(() => window.scrollTo({ top: 900, behavior: 'smooth' }));
    await delay(page, 1500);

    // -----------------------------------------------------------------------
    // Scene 9: Goals
    // -----------------------------------------------------------------------
    // Navigate to goals
    const goalsLink = page.locator('a:has-text("Goals")').first();
    if (await goalsLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await goalsLink.click();
      await page.waitForLoadState('networkidle');
    }

    await injectOverlay(page);
    await delay(page, 500);
    await caption(page, 9);

    // Create a goal
    const createGoalBtn = page.getByRole('button', { name: /create goal/i }).first();
    if (await createGoalBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
      await createGoalBtn.click();
      await delay(page, 500);

      // Fill goal form
      const goalNameInput = page.getByLabel(/goal name|name/i).first();
      if (await goalNameInput.isVisible({ timeout: 1000 }).catch(() => false)) {
        await typeSlowly(page, 'input:below(:text("Goal name"))', 'Signup');
        await delay(page, 300);
      }

      // Select custom event type (should be default)
      const eventNameInput = page.getByLabel(/event name/i).first();
      if (await eventNameInput.isVisible({ timeout: 1000 }).catch(() => false)) {
        await typeSlowly(page, 'input:below(:text("Event name"))', 'signup');
        await delay(page, 300);
      }

      // Submit
      const createBtn = page.getByRole('button', { name: /^create$/i }).last();
      if (await createBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
        await createBtn.click();
        await delay(page, 1500);
      }
    }

    // -----------------------------------------------------------------------
    // Scene 10: Invite team member
    // -----------------------------------------------------------------------
    const membersLink = page.locator('a:has-text("Members")').first();
    if (await membersLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await membersLink.click();
      await page.waitForLoadState('networkidle');
    }

    await injectOverlay(page);
    await delay(page, 500);
    await caption(page, 10);

    // Create invite
    const inviteBtn = page.getByRole('button', { name: /invite/i }).first();
    if (await inviteBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
      await inviteBtn.click();
      await delay(page, 500);

      const emailInput = page.getByLabel(/email/i).first();
      if (await emailInput.isVisible({ timeout: 1000 }).catch(() => false)) {
        await typeSlowly(page, 'input:below(:text("Email"))', 'teammate@acme.com');
        await delay(page, 500);
      }

      const submitInvite = page.getByRole('button', { name: /create invite/i }).last();
      if (await submitInvite.isVisible({ timeout: 1000 }).catch(() => false)) {
        await submitInvite.click();
        await delay(page, 1500);
      }
    }

    // -----------------------------------------------------------------------
    // Scene 11: Billing
    // -----------------------------------------------------------------------
    const billingLink = page.locator('a:has-text("Billing")').first();
    if (await billingLink.isVisible({ timeout: 2000 }).catch(() => false)) {
      await billingLink.click();
      await page.waitForLoadState('networkidle');
    }

    await injectOverlay(page);
    await delay(page, 500);
    await caption(page, 11);

    // Scroll to show all plan cards
    await page.evaluate(() => window.scrollTo({ top: 300, behavior: 'smooth' }));
    await delay(page, 2000);

    // -----------------------------------------------------------------------
    // Scene 12: Closing — back to landing
    // -----------------------------------------------------------------------
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await injectOverlay(page);
    await delay(page, 1000);

    // Show closing caption with logo visible
    await caption(page, 12);
    await delay(page, 1000);
  });
});
