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
    views/           # Page-level view components
    App.vue          # Root component
    main.ts          # Application entry point
```

## Routes

| Path | View Component | Auth | Description |
|------|---------------|------|-------------|
| `/` | Home | No | Landing page |
| `/login` | Login | No | Email/password and OAuth login |
| `/register` | Register | No | New account registration |
| `/invite/:code` | InviteAccept | No | Accept organization invite |
| `/orgs` | OrgList | Yes | List user's organizations |
| `/org/:orgId/sites` | SiteList | Yes | List sites in an organization |
| `/org/:orgId/site/:siteId` | Dashboard | Yes | Analytics dashboard |
| `/org/:orgId/site/:siteId/goals` | Goals | Yes | Goal management and conversions |
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

Key actions: `login()`, `logout()`, `register()`, `fetchMe()`, `toggleDarkMode()`, `setLocale()`.

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

Analytics data for the dashboard.

| State | Description |
|-------|-------------|
| `metrics` | Aggregate metric values (visitors, pageviews, bounce rate, avg duration) |
| `timeseries` | Time-bucketed data for the main chart |
| `sources` | Top referral sources |
| `pages` | Top pages |
| `locations` | Visitor locations (country, region, city) |
| `devices` | Browser, OS, and device type breakdowns |
| `period` | Currently selected date period |
| `filters` | Active filters |

Key actions: `fetchStats()`, `setPeriod()`, `setFilter()`, `clearFilters()`.

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
| `conversions` | Conversion data per goal |

Key actions: `fetchGoals()`, `createGoal()`, `deleteGoal()`, `fetchConversions()`.

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

The main analytics dashboard (`/org/:orgId/site/:siteId`) is composed of the following components:

```
Dashboard
  DatePicker              # Period selection (today, 7d, 30d, custom)
  FilterBar               # Active filter chips with clear button
  RealtimeBadge           # Live visitor count indicator
  MetricCards             # Top-level metrics (visitors, pageviews, bounce rate, duration)
  TimeseriesChart         # Main line/area chart (Chart.js via vue-chartjs)
  TopChart                # Bar chart for top N items
  SourcesTable            # Referral sources breakdown
  PagesTable              # Top pages breakdown
  LocationsMap            # Visitor map by country
  DevicesTable            # Browser, OS, device type tables
  GoalsTable              # Goal conversions summary
```

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
