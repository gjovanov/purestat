# API Reference

## Base URL

```
https://purestat.ai/api
```

For self-hosted instances, replace with your configured domain.

## Authentication

Purestat uses **JWT tokens** for authentication. Tokens are issued on login and can be provided in two ways:

1. **httpOnly cookie** (name: `token`) -- Set automatically on login/register. Used by the SPA frontend.
2. **Authorization header** -- `Authorization: Bearer <token>`. Used for programmatic API access.

API keys (for stats endpoints only) use the `X-API-Key` header.

### Obtaining a Token

Tokens are returned as httpOnly cookies on successful calls to `/api/auth/register`, `/api/auth/login`, and the OAuth callback endpoints.

## Error Responses

All errors follow a consistent JSON format:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Email is required"
  }
}
```

Common error codes:

| HTTP Status | Code | Description |
|-------------|------|-------------|
| 400 | `VALIDATION_ERROR` | Invalid or missing request parameters |
| 401 | `UNAUTHORIZED` | Missing or invalid authentication |
| 403 | `FORBIDDEN` | Insufficient permissions |
| 404 | `NOT_FOUND` | Resource not found |
| 409 | `CONFLICT` | Resource already exists |
| 429 | `RATE_LIMITED` | Too many requests |
| 500 | `INTERNAL_ERROR` | Server error |

## Rate Limiting

Rate limits are enforced per IP address via Redis sliding window counters.

| Endpoint Group | Limit |
|----------------|-------|
| Auth endpoints | 10 requests / minute |
| Tracker (event ingest) | 1000 requests / minute |
| API (authenticated) | 100 requests / minute |
| Stats / Export | 30 requests / minute |

When rate limited, the response includes:

```
HTTP/1.1 429 Too Many Requests
Retry-After: 30
```

---

## Public Endpoints

### POST /api/auth/register

Create a new user account.

**Request:**

```json
{
  "email": "user@example.com",
  "name": "Jane Doe",
  "password": "securepassword123"
}
```

**Response (201):**

```json
{
  "user": {
    "id": "507f1f77bcf86cd799439011",
    "email": "user@example.com",
    "name": "Jane Doe",
    "created_at": "2026-02-25T10:00:00Z"
  }
}
```

Sets the `token` httpOnly cookie.

### POST /api/auth/login

Authenticate with email and password.

**Request:**

```json
{
  "email": "user@example.com",
  "password": "securepassword123"
}
```

**Response (200):**

```json
{
  "user": {
    "id": "507f1f77bcf86cd799439011",
    "email": "user@example.com",
    "name": "Jane Doe",
    "created_at": "2026-02-25T10:00:00Z"
  }
}
```

Sets the `token` httpOnly cookie.

### POST /api/auth/logout

Clear the authentication cookie.

**Response (200):**

```json
{
  "message": "Logged out"
}
```

### GET /api/oauth/{provider}

Redirect to OAuth provider login page. Supported providers: `google`, `github`, `facebook`, `linkedin`, `microsoft`.

**Response:** `302 Redirect` to provider authorization URL.

### GET /api/oauth/{provider}/callback

OAuth callback handler. Creates or links user account, then redirects to the SPA.

**Response:** `302 Redirect` to `/` with `token` cookie set.

### GET /api/invite/{code}

Get invite details by code.

**Response (200):**

```json
{
  "invite": {
    "id": "507f1f77bcf86cd799439022",
    "org_name": "Acme Corp",
    "role": "viewer",
    "email": "invitee@example.com",
    "expires_at": "2026-03-04T10:00:00Z"
  }
}
```

### POST /api/invite/{code}/accept

Accept an invitation and join the organization. Requires authentication.

**Response (200):**

```json
{
  "org_id": "507f1f77bcf86cd799439033",
  "role": "viewer"
}
```

---

## Tracker Endpoint

### POST /api/event

Ingest a pageview or custom event from the JS tracker.

**Request:**

```json
{
  "domain": "example.com",
  "url": "https://example.com/pricing",
  "referrer": "https://google.com",
  "event_name": "pageview",
  "props": {}
}
```

**Response (202):**

```json
{
  "ok": true
}
```

The visitor hash is computed server-side from the request IP, User-Agent, domain, and a daily-rotating salt. The raw IP address is never stored.

---

## Authenticated Endpoints

All endpoints below require authentication via JWT cookie or Bearer token.

### User Profile

#### GET /api/me

Get the authenticated user's profile.

**Response (200):**

```json
{
  "user": {
    "id": "507f1f77bcf86cd799439011",
    "email": "user@example.com",
    "name": "Jane Doe",
    "created_at": "2026-02-25T10:00:00Z",
    "updated_at": "2026-02-25T10:00:00Z"
  }
}
```

#### PUT /api/me

Update the authenticated user's profile.

**Request:**

```json
{
  "name": "Jane Smith",
  "email": "jane.smith@example.com"
}
```

**Response (200):** Updated user object.

### Organizations

#### GET /api/org

List organizations the authenticated user belongs to.

**Response (200):**

```json
{
  "orgs": [
    {
      "id": "507f1f77bcf86cd799439033",
      "name": "Acme Corp",
      "slug": "acme-corp",
      "plan": "pro",
      "role": "owner",
      "created_at": "2026-02-25T10:00:00Z"
    }
  ]
}
```

#### POST /api/org

Create a new organization. The authenticated user becomes the owner.

**Request:**

```json
{
  "name": "Acme Corp"
}
```

**Response (201):**

```json
{
  "org": {
    "id": "507f1f77bcf86cd799439033",
    "name": "Acme Corp",
    "slug": "acme-corp",
    "plan": "free",
    "created_at": "2026-02-25T10:00:00Z"
  }
}
```

#### GET /api/org/{org_id}

Get organization details. Requires membership.

#### PUT /api/org/{org_id}

Update organization. Requires `owner` or `admin` role.

#### DELETE /api/org/{org_id}

Delete organization and all associated data. Requires `owner` role.

### Sites

#### GET /api/org/{org_id}/site

List all sites in the organization.

**Response (200):**

```json
{
  "sites": [
    {
      "id": "507f1f77bcf86cd799439044",
      "domain": "example.com",
      "name": "Example Site",
      "timezone": "America/New_York",
      "public": false,
      "created_at": "2026-02-25T10:00:00Z"
    }
  ]
}
```

#### POST /api/org/{org_id}/site

Create a new site. Requires `owner` or `admin` role. Enforces plan limits on site count.

**Request:**

```json
{
  "domain": "example.com",
  "name": "Example Site",
  "timezone": "America/New_York"
}
```

**Response (201):** Site object.

#### GET /api/org/{org_id}/site/{site_id}

Get site details.

#### PUT /api/org/{org_id}/site/{site_id}

Update site settings. Requires `owner` or `admin` role.

#### DELETE /api/org/{org_id}/site/{site_id}

Delete site and all associated analytics data. Requires `owner` or `admin` role.

### Goals

#### GET /api/org/{org_id}/site/{site_id}/goal

List all goals for a site.

#### POST /api/org/{org_id}/site/{site_id}/goal

Create a conversion goal.

**Request (event goal):**

```json
{
  "goal_type": "event",
  "event_name": "signup"
}
```

**Request (page goal):**

```json
{
  "goal_type": "page",
  "page_path": "/thank-you"
}
```

**Response (201):** Goal object.

#### DELETE /api/org/{org_id}/site/{site_id}/goal/{goal_id}

Delete a goal. Requires `owner` or `admin` role.

### Members

#### GET /api/org/{org_id}/member

List organization members with their roles.

#### PUT /api/org/{org_id}/member/{member_id}

Update a member's role. Requires `owner` role.

**Request:**

```json
{
  "role": "admin"
}
```

#### DELETE /api/org/{org_id}/member/{member_id}

Remove a member from the organization. Requires `owner` or `admin` role. Owners cannot be removed.

### Invites

#### GET /api/org/{org_id}/invite

List pending invites for the organization.

#### POST /api/org/{org_id}/invite

Create an invitation. Requires `owner` or `admin` role.

**Request:**

```json
{
  "email": "teammate@example.com",
  "role": "viewer"
}
```

**Response (201):**

```json
{
  "invite": {
    "id": "507f1f77bcf86cd799439055",
    "email": "teammate@example.com",
    "role": "viewer",
    "code": "abc123def456",
    "expires_at": "2026-03-04T10:00:00Z"
  }
}
```

#### DELETE /api/org/{org_id}/invite/{invite_id}

Revoke a pending invite.

### API Keys

#### GET /api/org/{org_id}/site/{site_id}/api-key

List API keys for a site (shows name, prefix, scopes, and last used -- never the full key).

#### POST /api/org/{org_id}/site/{site_id}/api-key

Create a new API key. The full key is returned only once.

**Request:**

```json
{
  "name": "CI Dashboard",
  "scopes": ["stats:read", "realtime:read"]
}
```

**Response (201):**

```json
{
  "api_key": {
    "id": "507f1f77bcf86cd799439066",
    "name": "CI Dashboard",
    "key": "ps_live_a1b2c3d4e5f6g7h8i9j0k1l2m3n4o5p6",
    "key_prefix": "ps_live_",
    "scopes": ["stats:read", "realtime:read"],
    "created_at": "2026-02-25T10:00:00Z"
  }
}
```

The `key` field is only returned at creation time. Store it securely.

#### DELETE /api/org/{org_id}/site/{site_id}/api-key/{key_id}

Revoke an API key.

### Analytics

#### POST /api/org/{org_id}/site/{site_id}/stats

Query aggregated analytics data. This is the primary endpoint for the dashboard.

**Request:**

```json
{
  "period": "30d",
  "date_from": "2026-01-26",
  "date_to": "2026-02-25",
  "metrics": ["visitors", "pageviews", "bounce_rate", "avg_duration"],
  "dimensions": ["date"],
  "filters": {
    "pathname": "/blog/*",
    "country": "US",
    "browser": "Chrome",
    "utm_source": "twitter"
  },
  "order_by": "visitors",
  "order_dir": "desc",
  "limit": 100
}
```

**Available metrics:**

| Metric | Description |
|--------|-------------|
| `visitors` | Unique visitor count (by visitor hash) |
| `pageviews` | Total pageview events |
| `sessions` | Total sessions |
| `bounce_rate` | Percentage of single-pageview sessions |
| `avg_duration` | Average session duration in seconds |
| `events` | Total custom events |
| `conversions` | Goal completion count |
| `conversion_rate` | Goal completion rate as percentage |

**Available dimensions:**

| Dimension | Description |
|-----------|-------------|
| `date` | Date (bucketed by day) |
| `pathname` | Page path |
| `referrer` | Referrer URL |
| `utm_source` | UTM source parameter |
| `utm_medium` | UTM medium parameter |
| `utm_campaign` | UTM campaign parameter |
| `browser` | Browser name |
| `os` | Operating system |
| `device_type` | Device type: `desktop`, `mobile`, `tablet` |
| `country` | Country code (ISO 3166-1 alpha-2) |
| `region` | Region/state |
| `city` | City name |
| `event_name` | Custom event name |

**Available period shortcuts:**

| Period | Description |
|--------|-------------|
| `today` | Current day |
| `7d` | Last 7 days |
| `30d` | Last 30 days |
| `month` | Current calendar month |
| `6mo` | Last 6 months |
| `12mo` | Last 12 months |
| `year` | Current calendar year |
| `custom` | Use `date_from` and `date_to` |

**Response (200):**

```json
{
  "metrics": {
    "visitors": 12543,
    "pageviews": 34210,
    "bounce_rate": 42.5,
    "avg_duration": 185
  },
  "dimensions": [
    {
      "date": "2026-02-25",
      "visitors": 450,
      "pageviews": 1230
    },
    {
      "date": "2026-02-24",
      "visitors": 423,
      "pageviews": 1180
    }
  ]
}
```

#### GET /api/org/{org_id}/site/{site_id}/realtime

Get real-time visitor data (last 5 minutes).

**Response (200):**

```json
{
  "visitors": 23,
  "pageviews": 47,
  "top_pages": [
    { "pathname": "/", "visitors": 8 },
    { "pathname": "/pricing", "visitors": 5 },
    { "pathname": "/docs", "visitors": 4 }
  ]
}
```

#### GET /api/org/{org_id}/site/{site_id}/export

Export analytics data as CSV. Accepts the same query parameters as the stats endpoint (as URL query params).

**Query Parameters:**

- `period` -- Period shortcut
- `date_from` / `date_to` -- Custom date range
- `metrics` -- Comma-separated metrics
- `dimensions` -- Comma-separated dimensions
- `filters[pathname]`, `filters[country]`, etc. -- Filters

**Response:**

```
HTTP/1.1 200 OK
Content-Type: text/csv
Content-Disposition: attachment; filename="purestat-export-2026-02-25.csv"

date,visitors,pageviews,bounce_rate,avg_duration
2026-02-25,450,1230,42.5,185
2026-02-24,423,1180,44.1,172
```

### Stripe Billing

#### GET /api/stripe/plans

List available billing plans.

**Response (200):**

```json
{
  "plans": [
    {
      "id": "free",
      "name": "Free",
      "price": 0,
      "pageviews_limit": 10000,
      "sites_limit": 1,
      "members_limit": 1
    },
    {
      "id": "pro",
      "name": "Pro",
      "price": 900,
      "pageviews_limit": 100000,
      "sites_limit": 5,
      "members_limit": 5
    },
    {
      "id": "business",
      "name": "Business",
      "price": 2900,
      "pageviews_limit": 1000000,
      "sites_limit": -1,
      "members_limit": -1
    }
  ]
}
```

Prices are in cents. `-1` indicates unlimited.

#### POST /api/stripe/checkout

Create a Stripe Checkout session for plan upgrade.

**Request:**

```json
{
  "org_id": "507f1f77bcf86cd799439033",
  "plan": "pro"
}
```

**Response (200):**

```json
{
  "checkout_url": "https://checkout.stripe.com/c/pay/cs_live_..."
}
```

#### POST /api/stripe/portal

Create a Stripe Customer Portal session for managing subscriptions.

**Request:**

```json
{
  "org_id": "507f1f77bcf86cd799439033"
}
```

**Response (200):**

```json
{
  "portal_url": "https://billing.stripe.com/p/session/..."
}
```

#### POST /api/stripe/webhook

Stripe webhook handler. Processes events such as `checkout.session.completed`, `customer.subscription.updated`, and `customer.subscription.deleted` to keep the org plan in sync.

**Request:** Raw Stripe webhook payload with `Stripe-Signature` header.

**Response (200):**

```json
{
  "received": true
}
```
