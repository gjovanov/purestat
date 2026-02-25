# Use Cases

Step-by-step walkthroughs of common user flows in Purestat.

## 1. New User Signup to First Dashboard View

A new user registers, creates an organization, adds their website, installs the tracker, and views their first analytics data.

### Steps

1. **Register an account**

   Navigate to `/register` and create an account with email and password, or use an OAuth provider (Google, GitHub, Facebook, LinkedIn, Microsoft).

   ```
   POST /api/auth/register
   { "email": "user@example.com", "name": "Jane Doe", "password": "..." }
   ```

2. **Create an organization**

   After login, navigate to `/orgs` and click "Create Organization." Every user needs at least one organization to manage sites.

   ```
   POST /api/org
   { "name": "Acme Corp" }
   ```

   The user is automatically assigned the `owner` role. The organization starts on the **Free** plan (10,000 pageviews/month, 1 site, 1 member).

3. **Add a site**

   Navigate to `/org/:orgId/sites` and click "Add Site." Enter the website domain and optional display name.

   ```
   POST /api/org/:orgId/site
   { "domain": "example.com", "name": "Example Site", "timezone": "America/New_York" }
   ```

4. **Install the tracker**

   The site settings page shows the tracker installation snippet. Copy it and add it to your website's HTML:

   ```html
   <script defer data-domain="example.com" src="https://purestat.ai/js/purestat.js"></script>
   ```

   The tracker starts sending pageview events immediately once visitors arrive.

5. **View the dashboard**

   Navigate to `/org/:orgId/site/:siteId` to see the analytics dashboard. The dashboard shows:
   - Metric cards (visitors, pageviews, bounce rate, average duration)
   - Real-time visitor badge
   - Timeseries chart
   - Top sources, pages, locations, and devices

   Data begins appearing within seconds of the tracker sending its first event.

---

## 2. Invite a Team Member

An organization owner or admin invites a colleague to view the analytics dashboard.

### Steps

1. **Create an invite**

   Navigate to `/org/:orgId/members` and click "Invite Member." Enter the colleague's email and select a role (`admin` or `viewer`).

   ```
   POST /api/org/:orgId/invite
   { "email": "colleague@example.com", "role": "viewer" }
   ```

   The system generates a unique invite code and provides a shareable link: `https://purestat.ai/invite/:code`.

2. **Share the invite link**

   Copy the invite link and send it to the colleague via email, Slack, or any other channel. The invite expires after 7 days.

3. **Colleague accepts the invite**

   The colleague opens the link, which navigates to `/invite/:code`. If they do not have a Purestat account, they are prompted to register first. Once authenticated, they accept the invite:

   ```
   POST /api/invite/:code/accept
   ```

4. **Colleague views the dashboard**

   The colleague is added to the organization with the `viewer` role. They can now navigate to `/org/:orgId/sites`, select a site, and view its analytics dashboard.

   Viewers can see all dashboard data but cannot modify site settings, create goals, or manage members.

---

## 3. Set Up Goals and Track Conversions

A user configures conversion goals to track specific user actions on their website.

### Steps

1. **Navigate to goals**

   Go to `/org/:orgId/site/:siteId/goals` and click "Add Goal."

2. **Create a page goal**

   Track visits to a specific page (e.g., a thank-you page after form submission):

   ```
   POST /api/org/:orgId/site/:siteId/goal
   { "goal_type": "page", "page_path": "/thank-you" }
   ```

3. **Create an event goal**

   Track a custom event (e.g., newsletter signups):

   ```
   POST /api/org/:orgId/site/:siteId/goal
   { "goal_type": "event", "event_name": "signup" }
   ```

4. **Add custom event tracking to the website**

   Add JavaScript calls to trigger the custom event when users perform the action:

   ```javascript
   document.querySelector('#signup-form').addEventListener('submit', () => {
     purestat('signup', { props: { source: 'homepage' } })
   })
   ```

5. **Track revenue (optional)**

   For purchase events, include revenue data:

   ```javascript
   purestat('purchase', {
     revenue: { amount: 29.99, currency: 'USD' },
     props: { plan: 'pro' }
   })
   ```

6. **View conversion data**

   The goals page shows each goal with its conversion count and conversion rate. The dashboard's GoalsTable component also displays a summary. Use the stats API to query conversion data programmatically:

   ```
   POST /api/org/:orgId/site/:siteId/stats
   { "period": "30d", "metrics": ["conversions", "conversion_rate"], "dimensions": ["event_name"] }
   ```

---

## 4. Create an API Key and Query Stats Programmatically

A developer creates an API key to fetch analytics data from their CI pipeline or custom integration.

### Steps

1. **Navigate to API keys**

   Go to `/org/:orgId/site/:siteId/api-keys` and click "Create API Key."

2. **Create a key with specific scopes**

   ```
   POST /api/org/:orgId/site/:siteId/api-key
   { "name": "CI Dashboard", "scopes": ["stats:read", "realtime:read"] }
   ```

   The response includes the full API key. **This is the only time the full key is shown.** Copy it and store it securely.

   ```json
   {
     "api_key": {
       "key": "ps_live_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6",
       "scopes": ["stats:read", "realtime:read"]
     }
   }
   ```

3. **Use the key to query stats**

   Include the key in the `X-API-Key` header:

   ```bash
   curl -X POST https://purestat.ai/api/org/:orgId/site/:siteId/stats \
     -H "X-API-Key: ps_live_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6" \
     -H "Content-Type: application/json" \
     -d '{
       "period": "7d",
       "metrics": ["visitors", "pageviews"],
       "dimensions": ["date"]
     }'
   ```

4. **Query real-time data**

   ```bash
   curl https://purestat.ai/api/org/:orgId/site/:siteId/realtime \
     -H "X-API-Key: ps_live_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6"
   ```

5. **Manage keys**

   View all keys (showing name, prefix, scopes, and last-used timestamp) on the API keys page. Revoke keys that are no longer needed:

   ```
   DELETE /api/org/:orgId/site/:siteId/api-key/:keyId
   ```

---

## 5. Upgrade Plan and Manage Billing

An organization owner upgrades from the Free plan to Pro to unlock more pageviews, sites, and team members.

### Steps

1. **View available plans**

   Navigate to `/org/:orgId/billing` to see the plan comparison:

   | Plan | Price | Pageviews/mo | Sites | Members |
   |------|-------|-------------|-------|---------|
   | Free | $0 | 10,000 | 1 | 1 |
   | Pro | $9/mo | 100,000 | 5 | 5 |
   | Business | $29/mo | 1,000,000 | Unlimited | Unlimited |

2. **Start checkout**

   Click "Upgrade" on the desired plan. This creates a Stripe Checkout session:

   ```
   POST /api/stripe/checkout
   { "org_id": "...", "plan": "pro" }
   ```

   The user is redirected to Stripe's hosted checkout page to enter payment details.

3. **Complete payment**

   After successful payment, Stripe redirects back to Purestat. The Stripe webhook (`POST /api/stripe/webhook`) processes the `checkout.session.completed` event and updates the organization's plan.

4. **Manage subscription**

   To update payment methods, view invoices, or cancel the subscription, click "Manage Billing" which opens the Stripe Customer Portal:

   ```
   POST /api/stripe/portal
   { "org_id": "..." }
   ```

5. **Plan enforcement**

   When the organization exceeds its plan limits (too many sites, too many members, or pageview quota reached), the system displays a warning and blocks the limited action until the plan is upgraded.

   Downgrading a plan does not delete existing sites or members but prevents creating new ones until the counts are within the new plan's limits.

---

## 6. Export Data as CSV

A user exports their analytics data for external analysis or record-keeping.

### Steps

1. **Navigate to the dashboard**

   Go to `/org/:orgId/site/:siteId` and configure the desired date range and filters using the DatePicker and FilterBar.

2. **Click Export**

   Click the "Export CSV" button in the dashboard toolbar. The frontend calls the export endpoint with the current query parameters:

   ```
   GET /api/org/:orgId/site/:siteId/export?period=30d&metrics=visitors,pageviews,bounce_rate&dimensions=date
   ```

3. **Download the file**

   The browser downloads a CSV file named `purestat-export-YYYY-MM-DD.csv`:

   ```csv
   date,visitors,pageviews,bounce_rate,avg_duration
   2026-02-25,450,1230,42.5,185
   2026-02-24,423,1180,44.1,172
   2026-02-23,398,1050,39.8,191
   ```

4. **Export with filters**

   Filters applied on the dashboard are included in the export. For example, exporting only traffic from the United States:

   ```
   GET /api/org/:orgId/site/:siteId/export?period=30d&metrics=visitors,pageviews&dimensions=pathname&filters[country]=US
   ```

5. **Programmatic export via API key**

   Developers can automate exports using an API key with the `export:read` scope:

   ```bash
   curl "https://purestat.ai/api/org/:orgId/site/:siteId/export?period=7d&metrics=visitors,pageviews&dimensions=date" \
     -H "X-API-Key: ps_live_..." \
     -o analytics-export.csv
   ```

   This is useful for scheduled reports, data pipelines, or importing into spreadsheets and BI tools.
