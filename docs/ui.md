# Frontend / UI

The Purestat frontend is a Vue 3 single-page application that provides the dashboard, site management, and billing interfaces.

## Tech Stack

| Library | Version | Purpose |
|---------|---------|---------|
| Vue | 3.5 | Reactive UI framework |
| Vuetify | 3 | Material Design component library |
| Pinia | 3 | State management |
| Vue Router | 4 | Client-side routing |
| vue-i18n | Latest | Internationalization |
| Chart.js | Latest | Chart rendering |
| vue-chartjs | Latest | Vue bindings for Chart.js |

## Project Structure

```
ui/
  src/
    assets/          # Static assets (logo, images)
    components/      # Reusable Vue components
      common/        # Shared UI components
      dashboard/     # Dashboard-specific components
      layout/        # App shell and navigation
    composables/     # Vue composables (shared logic)
    locales/         # i18n translation files
    plugins/         # Vuetify, i18n, router setup
    router/          # Route definitions
    stores/          # Pinia stores
    utils/           # Pure helpers: periods and their comparison, number formats, dimension labels
    views/           # Page-level view components
    App.vue          # Root component
    main.ts          # Application entry point
  tests/
    unit/            # bun tests of src/utils (bun run test:unit)
    e2e/             # Playwright, against a real account
```

`bun run build` type-checks (`vue-tsc --noEmit`) and builds; `bun run test:unit` runs the unit tests.

## Routes

| Path | View Component | Auth | Description |
|------|---------------|------|-------------|
| `/` | Home | No | Landing page |
| `/login` | Login | No | Email/password and OAuth login; shows why an OAuth sign-in was refused (`?oauth_error=`) |
| `/register` | Register | No | New account registration |
| `/auth/oauth` | OAuthComplete | No | Where a Google or GitHub sign-in lands: loads the session from its cookie |
| `/invite/:code` | InviteAccept | No | Accept organization invite |
| `/orgs` | OrgList | Yes | List user's organizations |
| `/org/:orgId/sites` | SiteList | Yes | List sites in an organization |
| `/org/:orgId/site/:siteId` | Dashboard | Yes | Analytics dashboard |
| `/org/:orgId/site/:siteId/goals` | Goals | Yes | Goal management and conversions |
| `/org/:orgId/site/:siteId/funnels` | Funnels | Yes | Funnel definitions and their report ([Funnels](funnels.md)) |
| `/org/:orgId/site/:siteId/settings` | SiteSettings | Yes | Site configuration |
| `/org/:orgId/site/:siteId/api-keys` | ApiKeys | Yes | API key management |
| `/org/:orgId/members` | Members | Yes | Organization member management |
| `/org/:orgId/billing` | Billing | Yes | Plan selection and Stripe portal |
| `/org/:orgId/settings` | OrgSettings | Yes | Organization settings |
| `/settings` | UserSettings | Yes | User profile settings |

Routes marked as "Auth: Yes" are protected by a navigation guard that redirects unauthenticated users to `/login`.

## Pinia Stores

### app

Global application state.

| State | Description |
|-------|-------------|
| `user` | Authenticated user object (null if not logged in) |
| `darkMode` | Current theme mode (persisted to localStorage) |
| `locale` | Current language locale |
| `loading` | Global loading indicator flag |

Key actions: `login()`, `logout()`, `register()`, `fetchMe()`, `resumeCookieSession()`, `toggleDarkMode()`, `setLocale()`.

#### Signing in with Google or GitHub

The login and sign-up pages ask `GET /api/auth/providers` which providers work on this server and show a "Continue with …" button for each (`components/auth/OAuthButtons.vue`); a server without OAuth shows none. A button is a full-page link to `/api/oauth/{provider}`; the page to return to is kept in `sessionStorage` for the round trip.

The server finishes the sign-in and sends the browser to `/auth/oauth` with the session in HttpOnly cookies. That page calls `resumeCookieSession()`, which loads the user from `GET /api/me` with the cookie. The SPA never reads a credential from an address and never holds this session's token: `localStorage.purestat_session` only remembers that a cookie session may exist, so a reload tries `GET /api/me`. A refused sign-in lands on `/login?oauth_error=<code>`, and the login page shows what the code means. The helpers with no Vue in them are in `utils/oauth.ts`, tested with `bun run test:unit`.

### org

Organization management.

| State | Description |
|-------|-------------|
| `orgs` | List of user's organizations |
| `currentOrg` | Currently selected organization |
| `members` | Members of the current organization |
| `invites` | Pending invites for the current organization |

Key actions: `fetchOrgs()`, `createOrg()`, `updateOrg()`, `deleteOrg()`, `fetchMembers()`, `updateMemberRole()`, `removeMember()`, `createInvite()`, `deleteInvite()`.

### site

Site management within an organization.

| State | Description |
|-------|-------------|
| `sites` | List of sites in the current organization |
| `currentSite` | Currently selected site |

Key actions: `fetchSites()`, `createSite()`, `updateSite()`, `deleteSite()`.

### stats

Analytics data for the dashboard. Every number comes from the report routes (`POST /api/org/:orgId/site/:siteId/query`, or `/api/org/:orgId/analytics/query?site_ids=…` for several sites), which run the v1 query of [the API](api.md#post-apiv1query): the dashboard shows what an API key reads.

| State | Description |
|-------|-------------|
| `orgId`, `siteId`, `siteIds` | What the dashboard reads: one site, or several sites of the org |
| `dateRange`, `dateFrom`, `dateTo` | The selected preset (`1d`, `7d`, `30d`, `6mo`, `12mo`) or custom range |
| `period`, `previous` | The period on screen (UTC days, today included) and the one before it, of the same length |
| `filters` | Active filters, applied to every report |
| `overview`, `previousOverview` | Visitors, visits, pageviews, bounce rate and visit duration of both periods |
| `timeseries`, `previousTimeseries` | Visitors and pageviews per day, with 0 for a day without any |
| `goalResults`, `previousGoalResults` | Per goal name, its converting visitors and conversion rate |
| `version` | Bumped on every reload, so each report card refetches |

Key actions: `fetchDashboard()`, `fetchMultiSiteDashboard()`, `runReport()`, `setDateRange()`, `setFilter()` (replaces a filter on the same dimension), `addFilter()`, `removeFilter()`, `clearFilters()`.

### realtime

Live visitor data, polled at regular intervals.

| State | Description |
|-------|-------------|
| `visitors` | Current number of active visitors (last 5 min) |
| `pageviews` | Pageviews in the last 5 minutes |
| `topPages` | Most-viewed pages right now |
| `polling` | Whether polling is active |

Key actions: `startPolling()`, `stopPolling()`, `fetchRealtime()`.

### goals

Goal and conversion tracking.

| State | Description |
|-------|-------------|
| `goals` | List of configured goals for the current site |

Key actions: `fetchGoals()`, `createGoal()`, `deleteGoal()`. The conversions themselves are part of the dashboard's report (`stats.goalResults`).

### funnels

A site's funnels and their reports.

| State | Description |
|-------|-------------|
| `funnels` | The funnels of the current site |

Key actions: `fetchFunnels()`, `createFunnel()`, `updateFunnel()`, `deleteFunnel()`, `fetchReport()`. The Funnels view keeps the selected funnel's report, breakdown and filters itself. Its components are in `components/funnels/`: `FunnelEditor` (the create and edit dialog), `FunnelSteps` (the steps as bars, with conversion and drop-off) and `FunnelBreakdown` (the breakdown table).

### billing

Stripe billing and plan management.

| State | Description |
|-------|-------------|
| `plans` | Available billing plans |
| `currentPlan` | Organization's current plan |

Key actions: `fetchPlans()`, `createCheckout()`, `openPortal()`.

## Component Tree

### Layout Components

```
AppLayout
  AppBar
    LogoLink
    OrgSwitcher
    ThemeToggle (dark/light)
    LocaleSwitcher
    UserMenu
  NavigationDrawer
    SiteNav (dashboard, goals, settings, API keys)
    OrgNav (sites, members, billing, settings)
  AppFooter
```

### Dashboard Components

The analytics dashboard, for one site (`/org/:orgId/site/:siteId`) or for the sites selected in the app bar (`/org/:orgId/dashboard`), is composed of the following components:

```
Dashboard
  DatePicker              # Period selection (today, 7d, 30d, 6m, 12m)
  RealtimeBadge           # Live visitor count indicator
  FilterBar               # "Filter" button and the active filter chips
    FilterDialog          # Dimension, operator and value (with the period's values offered)
  MetricCards             # Visitors, visits, pageviews, bounce rate, visit duration, each with its change
  TopChart                # Visitors and pageviews per day; the previous period's visitors dashed
  BreakdownCard           # One card per report, with tabs, "Show more" and drill-down:
                          #   Sources:   Channels / Sources / Referrers
                          #   Campaigns: Source / Medium / Campaign / Content / Term / Source group / Medium group
                          #   Pages:     Pages (with read rate, read time, scroll) / Entry pages / Exit pages
                          #   Locations: Countries
                          #   Devices:   Devices / Browsers / Operating systems
  GoalsTable              # Per goal: conversions, conversion rate and their change (one site only)
```

#### What the dashboard does

- **Period comparison.** Every metric card, and every goal, shows its change against the previous period of the same length: the day before for Today, the 7 days before for 7D, and so on. Counts and durations change in percent, rates (bounce rate, conversion rate) in percentage points. The chart draws the previous period's visitors as a dashed line, day against day. Days are UTC days, as the API counts them, and the current period includes today so far.
- **Filters.** The Filter button adds a condition on any dimension (page, entry and exit page, hostname, channel, source, referrer, the five UTM parameters, country, device, browser, OS) with `is`, `is not`, `contains` or `starts with`. Every card, chart and number on the page is filtered. A filter on a visit's attribute (its channel, referrer or campaign) selects whole visits, so the pages of a LinkedIn visit count for LinkedIn even after its landing page.
- **Drill-down.** A click on a row filters the dashboard by that value and opens the next tab: a channel opens its sources, a source its referrers, a UTM source its mediums, then campaigns, content and terms; a device type opens its browsers, then operating systems. Each new click on a dimension replaces that dimension's filter.
- **More than 10 rows.** A card shows its top 10 rows; "Show more" adds 25 at a time while there are more.
- **Channels** group visits by their referrer, by the table in [the API reference](api.md#channels-and-campaigns): search, AI assistants, social, community, code, own site, direct and other. The grouping is done by the server, so the dashboard and the API agree.
- **Campaigns** list the visits that carry a campaign. Sources and mediums are normalised (`YouTube`, `youtub` and `yt` are `youtube`; `twitter` is `x`); a source the table does not know is listed as itself, and under `other` in the Source group tab. With a goal selected in the card, each row also shows the goal's conversions and conversion rate.
- **Goals** report the unique visitors who completed each goal (`Conversions`), their share of all visitors (`CR`), and both changes.
- **Engagement per page.** The Pages tab shows, per page, its visitors and pageviews, its **read rate** (the share of its pageviews read: 30 s or more of visible, focused time, or 75 % of the page scrolled), its **read time** (the median, never a mean) and its **scroll** depth (the median of how far down its pageviews got; a dash for a page that does not scroll). Each column header explains itself on hover. Filtered by a channel or a campaign, the same columns show how that traffic reads.

### Common Components

Reusable components shared across views:

- **DataTable** -- Sortable, paginated table with loading skeleton.
- **MetricCard** -- Single metric display with trend indicator.
- **ConfirmDialog** -- Confirmation dialog for destructive actions.
- **EmptyState** -- Illustrated empty state with call-to-action.
- **CopyButton** -- Copy-to-clipboard button (for invite links, API keys, tracker snippet).
- **CodeBlock** -- Syntax-highlighted code display.

## Theming

Purestat supports light and dark modes using Vuetify's theme system.

### Theme Toggle

Users can toggle between light and dark mode using the theme button in the app bar. The preference is persisted to `localStorage` and applied on page load.

### Color Scheme

The Purestat color scheme is defined in the Vuetify plugin configuration:

| Token | Light | Dark | Usage |
|-------|-------|------|-------|
| primary | `#6366F1` (indigo) | `#818CF8` | Buttons, links, active states |
| secondary | `#EC4899` (pink) | `#F472B6` | Accents, highlights |
| background | `#FFFFFF` | `#0F172A` | Page background |
| surface | `#F8FAFC` | `#1E293B` | Cards, dialogs |
| error | `#EF4444` | `#F87171` | Error states |
| success | `#10B981` | `#34D399` | Success states, positive trends |
| warning | `#F59E0B` | `#FBBF24` | Warnings |

### Chart Colors

Chart.js charts use a consistent color palette that adapts to the current theme. The palette is defined in a shared composable (`useChartColors`) and includes distinct colors for up to 10 data series.

## Internationalization (i18n)

The frontend uses `vue-i18n` for internationalization.

### Setup

Translation files are stored in `src/locales/` as JSON files:

```
src/locales/
  en.json    # English (default)
```

### Usage in Components

```vue
<template>
  <h1>{{ t('dashboard.title') }}</h1>
  <p>{{ t('dashboard.visitors', { count: 1234 }) }}</p>
</template>

<script setup>
import { useI18n } from 'vue-i18n'
const { t } = useI18n()
</script>
```

### Adding a Language

1. Create a new JSON file in `src/locales/` (e.g. `de.json`).
2. Copy the structure from `en.json` and translate all values.
3. Register the locale in the i18n plugin configuration.
4. The locale switcher in the app bar will automatically show the new language.

### Number and Date Formatting

`vue-i18n` handles locale-aware number formatting (thousands separators, decimal marks) and date formatting throughout the dashboard.
