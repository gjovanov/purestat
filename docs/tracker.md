# JavaScript Tracker

The Purestat tracker is a lightweight JavaScript snippet (< 1KB gzipped) that collects pageview and custom event data from your website without using cookies or any client-side storage.

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

The tracker automatically detects single-page application navigation by monkey-patching `history.pushState` and `history.replaceState`, and by listening for the `popstate` event. Whenever the URL path changes, a new `pageview` event is sent.

No additional configuration is needed for SPAs built with Vue, React, Angular, Svelte, or any other framework that uses the History API.

### Event Payload

Each event sent to the server includes:

```json
{
  "domain": "example.com",
  "url": "https://example.com/pricing",
  "referrer": "https://google.com/search?q=analytics",
  "event_name": "pageview",
  "props": {}
}
```

The server derives the visitor identity from the request IP address and User-Agent header using a daily-rotating hash. No personally identifiable information is stored.

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

**File download:**

```javascript
document.querySelectorAll('a[href$=".pdf"]').forEach(link => {
  link.addEventListener('click', () => {
    purestat('file-download', { props: { file: link.href } })
  })
})
```

**Outbound link click:**

```javascript
document.querySelectorAll('a[href^="http"]').forEach(link => {
  if (link.hostname !== window.location.hostname) {
    link.addEventListener('click', () => {
      purestat('outbound-click', { props: { url: link.href } })
    })
  }
})
```

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

The tracker script is under **1KB gzipped**. It has zero dependencies and is built from TypeScript using Rollup with maximum minification and tree-shaking.

### Privacy

- **No cookies** -- The tracker does not set, read, or rely on any cookies.
- **No localStorage** -- No data is persisted on the client side.
- **No fingerprinting** -- The tracker does not collect screen resolution, installed fonts, canvas data, or any other fingerprinting signals.
- **No cross-site tracking** -- Visitor hashes are scoped to each domain and rotate daily.
- **IP not stored** -- The server uses the IP address only to compute the daily visitor hash and to derive approximate geolocation. The raw IP is never written to the database.

### Do Not Track

The tracker respects the browser's Do Not Track (DNT) setting by default. If `navigator.doNotTrack === "1"`, no events are sent.

### Bot Filtering

The tracker checks the User-Agent string and skips sending events for known bots and crawlers.

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
    reverse_proxy /api/event tracker:3001
}
```

**Nginx:**

```nginx
location /js/purestat.js {
    proxy_pass http://tracker:3001/js/purestat.js;
}

location /api/event {
    proxy_pass http://tracker:3001/api/event;
}
```

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
