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

An **API key** authenticates [`POST /api/v1/query`](#post-apiv1query) with the `X-API-Key` header. A key belongs to one site and reads only that site; it needs the `stats:read` scope. The dashboard endpoints take a JWT, not a key.

### Obtaining a Token

Tokens are returned as httpOnly cookies on successful calls to `/api/auth/register`, `/api/auth/login`, and the OAuth callback endpoints.

## Error Responses

All errors share one JSON shape: a short machine-readable code and a message for people.

```json
{
  "error": "not_found",
  "message": "Site not found"
}
```

| HTTP Status | `error` | Description |
|-------------|---------|-------------|
| 400 | `bad_request` | Invalid or missing request parameters |
| 401 | `unauthorized` | Missing or invalid authentication |
| 403 | `forbidden` | Insufficient permissions |
| 404 | `not_found` | Resource not found, or not in the organization named in the path |
| 409 | `conflict` | Resource already exists |
| 422 | `validation` | The request is well-formed but a value is not acceptable |
| 429 | `rate_limited` | Too many requests (see below) |
| 500 | `internal` | Server error |

## Rate Limiting

Requests are limited per client IP address and endpoint group, over a 60-second sliding window kept in Redis, so every API replica shares the same counts.

| Endpoint group | Paths | Requests per minute |
|----------------|-------|---------------------|
| Auth | `/api/auth/*`, `/api/oauth/*` | 10 |
| Tracker (event ingest) | `/api/event` | 1000 |
| Stats / Export | every path ending in `/stats`, `/query` or `/export`, and `/api/v1/*` | 120 |
| API | every other `/api/*` path | 300 |

`/api/health` and `/api/stripe/webhook` are not limited. The limits are configurable: see [Deployment](deployment.md#rate-limiting).

Every limited response says what the limit is and how much of it is left:

```
X-RateLimit-Limit: 300
X-RateLimit-Remaining: 287
```

Past the limit, the request is refused before it reaches the endpoint:

```
HTTP/1.1 429 Too Many Requests
Retry-After: 12
X-RateLimit-Limit: 10
X-RateLimit-Remaining: 0

{"error": "rate_limited", "message": "Too many requests. Retry in 12 s."}
```

`Retry-After` is the number of seconds until the oldest counted request leaves the window. A refused request is not counted, so a client that waits that long is let through again.

**The client address.** A proxy in front of the API appends the address it saw to `X-Forwarded-For`, so everything to the left of that entry was written by the client and is not trusted. The client is the first address from the right that is not a proxy hop: private, loopback, link-local or carrier-grade NAT (`100.64.0.0/10`). Without one, a public `X-Real-IP` is used, then the address of the TCP connection.

**What is counted.** An IPv4 client is counted by its address, and an IPv6 client by its `/64`, because one IPv6 host usually holds a whole `/64` and could otherwise step around a limit by changing addresses. The counters are stored in Redis under a keyed hash (HMAC-SHA256) of that, never under the address itself: like the rest of purestat, the limiter does not store IP addresses.

**If Redis is unavailable,** or does not answer within 500 ms, the request is let through and the API logs a warning. An outage of the limiter does not take logins and event ingest down with it.

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

> The OAuth sign-in is unfinished, so its two routes are not mounted (they answer `404`) unless the deployment sets `PURESTAT__OAUTH__ENABLED=true`.

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

Ingest a pageview, engagement or custom event from the JS tracker.

**Request:**

```json
{
  "domain": "example.com",
  "name": "pageview",
  "url": "https://example.com/pricing",
  "referrer": "https://google.com",
  "screen_width": 1440,
  "props": {}
}
```

**Response:** `202 Accepted`, with no body. An unknown `domain` is `400`.

An event is dropped, and still answered `202`, when:

- its User-Agent is a bot's (see [the tracker's list](tracker.md#what-is-never-counted));
- its page (`url`) is on `localhost`, a `.local` name or a loopback, private, link-local or carrier-grade NAT address, or has no host;
- its page's host is not one the site counts: the site's `domain`, `www.` + the domain, or one of its `allowed_hostnames`.

The visitor hash is computed server-side from the client IP, User-Agent, domain, and a daily-rotating salt. The client IP is read exactly as the rate limiter reads it (the right-most `X-Forwarded-For` entry that is not a proxy hop), so what a client writes into the header does not change who it is. The raw IP address is never stored.

The page address and the referrer are stored without their query string, fragment and credentials, and so is the `url` property of a link event (`outbound-click`, `file-download`; see [the tracker](tracker.md#outbound-links-and-file-downloads)), whatever sent it.

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

List API keys for a site (shows name, prefix, scopes and revocation -- never the full key).

#### POST /api/org/{org_id}/site/{site_id}/api-key

Create a new API key for the site (admin or owner). The full key is returned only once; purestat keeps only its SHA-256.

**Request:**

```json
{
  "name": "Growth job",
  "scopes": ["stats:read"]
}
```

`scopes` defaults to `["stats:read"]`, the scope `POST /api/v1/query` requires.

**Response (201):**

```json
{
  "id": "507f1f77bcf86cd799439066",
  "name": "Growth job",
  "key": "ps_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "key_prefix": "ps_XXXXXXXX",
  "scopes": ["stats:read"]
}
```

The `key` field is only returned at creation time. Store it securely.

#### DELETE /api/org/{org_id}/site/{site_id}/api-key/{key_id}

Revoke an API key. A revoked key is refused at once.

### Analytics

Every analytics endpoint (query, stats, realtime, export and [`/api/v1/query`](#post-apiv1query)) reads a site through the same rule ingest applies: only events whose page host is the site's domain, `www.` + the domain, or one of its `allowed_hostnames` count, including events recorded before that rule existed. Visitor and pageview counts are over `pageview` events; engagement and custom events are not pageviews. The `entry_page` and `exit_page` breakdowns read visits.

#### POST /api/org/{org_id}/site/{site_id}/query

The dashboard's reports for a signed-in member of the org: the [v1 query](#post-apiv1query) on this site, with the same body, the same validation, the same rows and the same `meta`. What the dashboard shows is what an API key reads. Goals are the site's.

```json
{
  "date_from": "2026-10-01",
  "date_to": "2026-10-14",
  "dimensions": ["channel"],
  "metrics": ["visitors", "visits"],
  "filters": [{ "dimension": "utm_source", "operator": "is", "value": "linkedin" }],
  "limit": 10
}
```

Errors use the shape of the other dashboard endpoints (`{"error": "bad_request", "message": "<what is wrong>"}`): 400 for a body the v1 query refuses, 403 for a caller who is not a member of the org, 404 for a site that is not the org's.

#### POST /api/org/{org_id}/analytics/query?site_ids={id},{id}

The same report over several sites of the org at once, each read through its own hostnames: what the org dashboard shows for the selected sites. `site_ids` is a comma-separated list; a site of another org is a 400. Goals belong to a site, so a request that names goals is a 400. The answer's `meta` carries `sites` (how many were read), `date_from`, `date_to`, `limit`, `offset` and `has_more`.

#### POST /api/org/{org_id}/site/{site_id}/stats

The dashboard's original endpoint, kept for existing clients. The dashboard now reads [`/query`](#post-apiorgorg_idsitesite_idquery), which also reports goals, channels and campaigns.

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

Export the site's events as CSV, oldest first: every event (pageviews, engagement reports and custom events) on the site's hostnames.

**Query Parameters:**

- `date_from` / `date_to` -- UTC days, `YYYY-MM-DD`, inclusive (required)

**Response:**

```
HTTP/1.1 200 OK
Content-Type: text/csv
Content-Disposition: attachment; filename="purestat-export.csv"

time,event_name,path,referrer,referrer_source,country,browser,os,device_type,utm_source,utm_medium,utm_campaign
2026-02-25 10:00:00,pageview,/pricing,https://www.google.com/search,Google,DE,Firefox,Linux,desktop,,,
```

A field that holds a comma, a quote or a line break is quoted (RFC 4180).

### Stats API (API key)

#### POST /api/v1/query

The stats of the site an API key belongs to: up to four dimensions, with visitor, visit, pageview and reading metrics and goal completions per row. Authenticate with the key in `X-API-Key`; the key's site is the site queried, so the body names none.

**Request:**

```
POST /api/v1/query
X-API-Key: ps_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
Content-Type: application/json
```

```json
{
  "date_from": "2026-10-01",
  "date_to": "2026-10-13",
  "dimensions": ["utm_source"],
  "metrics": ["visitors", "pageviews"],
  "goals": ["signup"],
  "filters": [{ "dimension": "path", "operator": "starts_with", "value": "/blog/" }],
  "limit": 100,
  "offset": 0
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `date_from`, `date_to` | Yes | UTC days, `YYYY-MM-DD`, inclusive. `date_to` is not before `date_from`, and at most 366 days after it |
| `dimensions` | No | 0 to 4 of the dimensions below. Without any, the answer is one row of totals |
| `metrics` | Yes | 1 or more of the metrics below |
| `goals` | No | 0 to 10 names of the site's goals |
| `filters` | No | Conditions on any of the dimensions below. Operators: `is`, `is_not`, `contains`, `starts_with`; the value is compared literally and case-sensitively |
| `limit` | No | 1 to 1000, default 100 |
| `offset` | No | 0 or more, default 0 |

A field the table does not list, an unknown name, or a name given twice is a `400`.

**Dimensions:**

| Dimension | Value |
|-----------|-------|
| `date` | The UTC day of the event; for `visits`, `bounce_rate` and `visit_duration`, the day the visit started |
| `hostname` | The page's host |
| `path` | The page's path |
| `entry_page`, `exit_page` | The visit's first and last page |
| `referrer_source` | Where the visit came from: `Direct`, a known source such as `Google`, or the referrer's host |
| `referrer_host` | The host of the visit's referrer |
| `channel` | The visit's channel, from its referrer's host: `search`, `ai_assistant`, `social`, `community`, `code`, `self`, `direct` or `other` (see [Channels and campaigns](#channels-and-campaigns)) |
| `utm_source`, `utm_medium` | The visit's campaign source and medium, from its landing page, normalised (see below) |
| `utm_campaign`, `utm_content`, `utm_term` | The rest of the visit's campaign, as its landing page carried it |
| `utm_source_group`, `utm_medium_group` | The source or medium if it is in the campaign table, else `other` |
| `country`, `device_type`, `browser`, `os` | The visitor's |
| `event_name` | The name of the event a metric counts: `pageview` for `visitors`, `pageviews` and `reads`, a goal's event for that goal |

A visit's referrer, channel, campaign and entry and exit page are the visit's for every event in it, so a goal completed three pages after the landing counts for the landing's campaign. A visit still in progress (one with an event within the session timeout, 30 minutes by default) counts its events under their own referrer and campaign until it ends, so a past day's numbers are final once its last visits have ended.

##### Channels and campaigns

The campaign key `(utm_source, utm_medium, utm_campaign, utm_content)` is the one roomler-ai keys its sign-ups by (FR-102 §3b), and both ends apply the same table, so a key means the same thing in both places. The values are read through the table when a report runs, so data recorded before the table existed is read the same way.

- **`utm_source`** is trimmed, lowercased, then its aliases are applied: `twitter` → `x`, `fb` → `facebook`, `yt` and `youtub` → `youtube`, `li` → `linkedin`, `ig` → `instagram`. **`utm_medium`** is trimmed and lowercased. A value the table does not know is **kept as itself** (lowercased): `Mastodon` reports as `mastodon`.
- **`utm_source_group`** is the source if it is one of `youtube` `tiktok` `x` `linkedin` `facebook` `reddit` `hn` `devto` `hashnode` `medium` `github` `newsletter` `blog` `instagram`, and `other` if not. **`utm_medium_group`** is the medium if it is one of `video` `post` `carousel` `comment` `community` `bio` `article` `email`, and `other` if not. An unknown value is reported under `other`, never dropped. A visit without a campaign has an empty source and medium, which group under `other` too: filter `utm_source` `is_not` `""` to leave those visits out.
- **`channel`** is read from the host of the visit's referrer. Each row of the table below matches its hosts and their subdomains, and the first row that matches wins:

| Channel | Referrer hosts |
|---------|----------------|
| `ai_assistant` | `chatgpt.com`, `chat.openai.com`, `perplexity.ai`, `gemini.google.com`, `copilot.microsoft.com`, `claude.ai` |
| `search` | `bing.com`, `duckduckgo.com`, `ecosia.org` |
| `social` | `t.co`, `x.com`, `twitter.com`, `linkedin.com`, `lnkd.in`, `facebook.com`, `youtube.com`, `youtu.be`, `tiktok.com`, `reddit.com`, `instagram.com` |
| `community` | `news.ycombinator.com`, `lobste.rs`, `dev.to`, `hashnode.com`, `medium.com` |
| `code` | `github.com` |
| `self` | The site's own hostnames (its domain, `www.` + the domain and its allowed hostnames) and their subdomains: navigation within the site, never a source |
| `search` | Any other host with a `google` or `yandex` label before its last one: `google.com`, `www.google.co.uk`, `yandex.ru` (not `googleusercontent.com`) |
| `direct` | No referrer |
| `other` | Any other referrer |

The host is compared in lowercase, without a port or trailing dots, so `l.facebook.com` is `social`, `www.google.co.uk` is `search`, and `roomler.ai.evil.example` is `other`. A campaign link's visit has a channel like any other visit: its referrer's.

**Metrics:**

| Metric | Value |
|--------|-------|
| `visitors` | Distinct visitors with a pageview |
| `pageviews` | Pageview events |
| `visits` | Visits |
| `bounce_rate` | The share of visits with one pageview, from 0 to 1 |
| `visit_duration` | The mean length of a visit, in seconds |
| `reads` | Pageviews read: read (visible and focused) for 30 seconds or more, or scrolled to 75 % of the page or further |
| `read_rate` | `reads` / `pageviews`, from 0 to 1 (capped at 1: a report sent just after midnight UTC lands on the day after its pageview) |
| `median_read_s` | The median read seconds of the pageviews that reported any. A median, never a mean of raw durations |
| `scroll_depth` | The median depth, in percent of the page's height, of the pageviews that reported one (pages that scroll) |

`visits`, `bounce_rate` and `visit_duration` are `null` unless every dimension is one a visit has (`date`, `entry_page`, `exit_page`, `referrer_source`, `referrer_host`, `channel`, the `utm_*` dimensions, `country`, `device_type`, `browser`, `os`); `bounce_rate` and `visit_duration` are also `null` in a row without visits. `read_rate` is `null` in a row without pageviews, `median_read_s` in a row without read time, `scroll_depth` in a row without a depth. A pageview's read time is its longest engagement report and its depth its deepest ([tracker](tracker.md#engagement)); reports from before the tracker measured the focus and the depth count visible time only and carry no depth.

**Goals:** per row, in `goals`, the distinct visitors who completed the goal: an event of the goal's name for a custom-event goal, a pageview of the goal's path for a pageview goal. In `conversion_rates`, those visitors as a share of the row's `visitors`, from 0 to 1 (`null` in a row without visitors, and never above 1: a visit that crosses midnight UTC can complete a goal on a day without a pageview of its visitor's). `conversion_rates` is present when the request names goals.

Rows are sorted by the first metric, descending, then by the dimensions, ascending.

**Response (200):**

```json
{
  "rows": [
    {
      "dimensions": { "utm_source": "linkedin" },
      "metrics": { "visitors": 52, "pageviews": 265 },
      "goals": { "signup": 1 },
      "conversion_rates": { "signup": 0.019230769230769232 }
    }
  ],
  "meta": {
    "site_domain": "roomler.ai",
    "hostnames": ["roomler.ai", "www.roomler.ai"],
    "date_from": "2026-10-01",
    "date_to": "2026-10-13",
    "limit": 100,
    "offset": 0,
    "has_more": false
  }
}
```

`meta.hostnames` lists the hostnames counted for the site; `has_more` says whether another page of rows exists.

**Errors:**

| Status | Body | When |
|--------|------|------|
| 400 | `{"error": "<what is wrong>"}` | The body breaks a rule above |
| 401 | `{"error": "unauthorized"}` | No key, an unknown or revoked key, or a key without `stats:read` |
| 429 | `{"error": "rate_limited", ...}` | Past the Stats group's limit (see Rate Limiting) |

Every value in the request reaches the database as a bound parameter, never as SQL.

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
