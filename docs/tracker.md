# JavaScript Tracker

The Purestat tracker is a lightweight JavaScript snippet (under 2 KB gzipped) that collects pageview, engagement and custom event data from your website, and on request outbound link clicks and file downloads, without cookies, and without storing anything in the browser.

## Installation

Add the following script tag to your website's `<head>` or before the closing `</body>` tag:

```html
<script defer data-domain="example.com" src="https://purestat.ai/js/purestat.js"></script>
```

Replace `example.com` with your actual domain as configured in Purestat.

That is it. The tracker will automatically record pageviews on every page load and SPA navigation.

## Attributes

| Attribute | Required | Description |
|-----------|----------|-------------|
| `data-domain` | Yes | The domain of your site as registered in Purestat. Must match exactly (e.g. `example.com`, not `www.example.com` unless that is what you registered). |
| `data-api` | No | Custom API endpoint URL. Defaults to the origin of the script (`https://purestat.ai/api/event`). Use this when self-hosting or proxying the tracker. |
| `data-outbound-links` | No | Report clicks on links to other hosts as `outbound-click` events ([Outbound links and file downloads](#outbound-links-and-file-downloads)). Off unless present. |
| `data-file-downloads` | No | Report clicks on links to files as `file-download` events. Off unless present. Empty, it counts the default file types; with a value, the listed ones (`data-file-downloads="pdf,zip,epub"`). |

An opt-in attribute counts when it is present (`data-outbound-links` or `data-outbound-links=""`), and not when it is absent or `"false"`.

### Example with outbound links and downloads

```html
<script
  defer
  data-domain="example.com"
  data-outbound-links
  data-file-downloads="pdf,zip"
  src="https://purestat.ai/js/purestat.js">
</script>
```

### Example with custom API endpoint

```html
<script
  defer
  data-domain="example.com"
  data-api="https://analytics.example.com/api/event"
  src="https://purestat.ai/js/purestat.js">
</script>
```

## How It Works

### Automatic Pageview Tracking

On initial page load, the tracker sends a `pageview` event with the current URL, referrer, and domain.

### SPA Navigation

The tracker automatically detects single-page application navigation by wrapping `history.pushState` and by listening for the `popstate` event. Whenever the URL path changes, a new `pageview` event is sent. (`history.replaceState` is not tracked: it replaces the current page rather than opening a new one.)

No additional configuration is needed for SPAs built with Vue, React, Angular, Svelte, or any other framework that uses the History API.

### Event Payload

Each event sent to the server includes:

```json
{
  "domain": "example.com",
  "name": "pageview",
  "url": "https://example.com/pricing",
  "referrer": "https://google.com/search?q=analytics",
  "screen_width": 1440,
  "props": {}
}
```

The server derives the visitor identity from the request IP address and User-Agent header using a daily-rotating hash. No personally identifiable information is stored.

### Engagement

The tracker measures how long each page is **read**: the clock runs while the tab is visible **and** the window has the focus, and stops while the tab is hidden or another window has the focus. A tab left open in the background, or behind another application, does not count as reading. Focus inside a frame of the page (an embedded video, say) still counts. It also measures how far down the page the visitor got: the bottom of the screen at its deepest, as a share of the page's height.

It reports both as an `engagement` event when the page is hidden, when the visitor navigates to another page of a single-page app, and when the page is closed. Each report carries:

| Property | Description |
|----------|-------------|
| `duration` | Seconds the page was visible and focused so far |
| `scroll` | The deepest point of the page on screen so far, in percent of its height (25 when the screen shows the top quarter). Absent for a page that does not scroll: such a page has no depth |
| `pv` | A random id for this pageview, held in memory only and never stored in the browser |

A report is sent only when the read time or the depth has grown, and it carries the address of the page it measures. A pageview's engaged time is its longest report, and its depth its deepest. Reports sent before `pv` existed each stand for their own pageview; reports from before the focus and the depth were measured count visible time only and carry no depth.

A pageview is a **read** when it was read for 30 seconds or more, or scrolled to 75 % of the page or further. The reports turn these into the `reads`, `read_rate`, `median_read_s` and `scroll_depth` metrics ([API](api.md#post-apiv1query)) and the Pages report of the dashboard.

### Outbound links and file downloads

With `data-outbound-links` or `data-file-downloads` on the script tag, a click on a link (a left click, Enter on a focused link, or a middle click that opens a tab) is reported as an event of its own:

| Event | When | `url` property |
|-------|------|----------------|
| `file-download` | The link's path ends in a counted file type, on any host (`data-file-downloads`) | The link's address |
| `outbound-click` | The link goes to another host than the page's (`data-outbound-links`) | The link's address |

A link to a file on another host is a `file-download` when downloads are counted. The address is the link's origin and path only: its query, its fragment and any credentials are never sent, and the server keeps a link event's `url` the same way whatever sent it. Only `http:` and `https:` links count, so `mailto:` and `javascript:` links are never reported.

The default file types are `pdf`, `xlsx`, `docx`, `txt`, `rtf`, `csv`, `exe`, `key`, `pps`, `ppt`, `pptx`, `7z`, `pkg`, `rar`, `gz`, `zip`, `avi`, `mov`, `mp4`, `mpeg`, `wmv`, `midi`, `mp3`, `wav`, `wma`, `dmg`, `iso`, `msi`, `deb`, `rpm`, `apk`, `tar`, `bz2` and `xz`.

They are custom events like any other: create a custom-event goal for `outbound-click` or `file-download` to see them as conversions, and break them down with the `event_name` dimension.

## Custom Events

Track user interactions and conversions by calling the global `purestat()` function.

### Basic Custom Event

```javascript
purestat('signup')
```

### Custom Event with Properties

```javascript
purestat('signup', {
  props: {
    plan: 'pro',
    source: 'landing-page'
  }
})
```

### Examples

**Button click tracking:**

```html
<button onclick="purestat('cta-click', { props: { location: 'hero' } })">
  Get Started
</button>
```

**Form submission:**

```javascript
document.querySelector('#signup-form').addEventListener('submit', () => {
  purestat('form-submit', { props: { form: 'signup' } })
})
```

**File downloads and outbound links** need no code: add `data-file-downloads` and `data-outbound-links` to the script tag ([Outbound links and file downloads](#outbound-links-and-file-downloads)).

## Custom Properties

Custom properties are key-value pairs attached to events. They allow you to segment and filter analytics data.

- Property names and values must be strings.
- Maximum 10 properties per event.
- Property names are limited to 64 characters.
- Property values are limited to 256 characters.

```javascript
purestat('purchase', {
  props: {
    product: 'T-Shirt',
    color: 'blue',
    size: 'L'
  }
})
```

Properties are available as filters in the dashboard and can be used in the stats API.

## Revenue Tracking

Track revenue by including `revenue` in the event options:

```javascript
purestat('purchase', {
  revenue: {
    amount: 29.99,
    currency: 'USD'
  },
  props: {
    plan: 'pro'
  }
})
```

- `amount` -- Numeric value (supports decimals up to 4 places).
- `currency` -- ISO 4217 currency code (e.g. `USD`, `EUR`, `GBP`).

Revenue data is displayed in the goals section of the dashboard and can be queried via the stats API.

## Technical Details

### Transport

The tracker uses `navigator.sendBeacon()` as the primary transport method. This ensures events are delivered reliably even when the user navigates away from the page or closes the tab. If `sendBeacon` is not available, it falls back to a synchronous `XMLHttpRequest`.

### Size

The tracker script is about **3.4 KB minified, 1.8 KB gzipped**, the click events included. It has zero dependencies and is built from TypeScript using Rollup with maximum minification and tree-shaking.

### Privacy

- **No cookies** -- The tracker does not set, read, or rely on any cookies.
- **Nothing stored in the browser** -- The tracker writes nothing to `localStorage` or anywhere else. It reads one key, `purestat_ignore`, which only you set (see below).
- **No fingerprinting** -- The tracker does not collect screen resolution, installed fonts, canvas data, or any other fingerprinting signals.
- **No cross-site tracking** -- Visitor hashes are scoped to each domain and rotate daily.
- **Addresses without their query** -- The page address, the referrer and a clicked link's address are kept as far as their path; a query string or fragment can carry anything, so none is stored.
- **IP not stored** -- The server uses the IP address only to compute the daily visitor hash and to derive approximate geolocation. The raw IP is never written to the database.

### What is never counted

The tracker sends nothing, and `window.purestat()` does nothing, when:

- the page is on `localhost`, a `*.localhost` or `*.local` name, or a loopback, private (RFC 1918), link-local, carrier-grade NAT (`100.64.0.0/10`) or unique-local IPv6 address;
- the page was opened from a file (`file:`);
- the browser is driven by automation (`navigator.webdriver` is true), as in Playwright, Puppeteer and Selenium runs;
- the browser carries the ignore flag. To keep your own visits out of your stats, run this once in the browser's console on your site:

  ```javascript
  localStorage.setItem('purestat_ignore', 'true')
  ```

  `localStorage.removeItem('purestat_ignore')` undoes it.

The server applies its own rules to every event, whatever sent it, and answers `202` either way:

- **Hostnames.** An event counts for a site only when its page's host is the site's domain, `www.` + the domain, or one of the site's *allowed hostnames* (site settings). Events from any other host, and from the local and private hosts above, are dropped. The same rule applies to every report, including data recorded before it existed.
- **Bots.** An event whose User-Agent contains `bot`, `crawler`, `spider`, `headless`, `HeadlessChrome`, `Lighthouse`, `PhantomJS`, `curl/`, `wget/`, `python-requests`, `Go-http-client`, `node-fetch` or `axios/` (case-insensitive) is dropped.

The browser's Do Not Track setting is not read.

### Error Handling

The tracker silently catches and ignores all errors. It will never throw exceptions, display console errors, or affect your site's functionality in any way.

## Self-Hosting the Tracker

When self-hosting Purestat, the tracker script is served by your own infrastructure. You can also proxy the tracker through your own domain to avoid ad-blocker interference.

### Serve from your domain

Configure your web server to proxy the tracker script:

**Caddy:**

```
analytics.example.com {
    reverse_proxy /js/* tracker:3001
    reverse_proxy /api/event api:3000
}
```

**Nginx:**

```nginx
location /js/purestat.js {
    proxy_pass http://tracker:3001/js/purestat.js;
}

location /api/event {
    proxy_pass http://api:3000/api/event;
}
```

Events go to the API (`purestat-api`): it resolves the site, applies the hostname and bot rules above, and keeps the visit. The standalone `purestat-tracker` binary does none of that, and its events belong to no site.

Then reference the proxied script in your HTML:

```html
<script
  defer
  data-domain="example.com"
  data-api="https://analytics.example.com/api/event"
  src="https://analytics.example.com/js/purestat.js">
</script>
```

### First-party subdomain

Using a subdomain on your own domain (e.g. `analytics.example.com`) helps bypass ad blockers that block third-party analytics scripts. The `data-api` attribute ensures events are sent to your own domain as well.

### Building from source

The tracker is located in the `tracker-js/` directory and can be rebuilt:

```bash
cd tracker-js
bun install
bun run build
```

The output file is `dist/purestat.js`.
