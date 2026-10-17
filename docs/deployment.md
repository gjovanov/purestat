# Deployment

## Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Docker | 24+ | Container runtime |
| Docker Compose | 2.20+ | Multi-container orchestration |
| Rust | 1.85+ | Backend build (development only) |
| Bun | 1.0+ | Frontend and tracker build (development only) |

## Development Setup

### 1. Start infrastructure services

```bash
docker-compose up -d
```

This starts:
- **MongoDB 7** on port `27017`
- **ClickHouse 24** on ports `8123` (HTTP) and `9000` (native)
- **Redis 7** on port `6379`

ClickHouse must be **24.10 or newer**: [funnels](funnels.md#requirements) use a `windowFunnel` mode that older versions refuse. The `clickhouse/clickhouse-server:24` image is 24.10.2.80; a host with an older image cached under that tag needs a fresh pull.

### 2. Run the backend

```bash
cargo run
```

The API server starts on `http://localhost:3000` by default.

### 3. Run the frontend

```bash
cd ui
bun install
bun dev
```

The Vite dev server starts on `http://localhost:5173` with hot module replacement and API proxying to `localhost:3000`.

### 4. Build the tracker (optional)

```bash
cd tracker-js
bun install
bun run build
```

Output: `tracker-js/dist/purestat.js`

## Docker Deployment

### Full stack with Docker Compose

The `docker-compose.full.yml` file runs the complete application stack including the backend, frontend, and all infrastructure services.

```bash
echo "JWT_SECRET=$(openssl rand -hex 32)" >> .env
docker-compose -f docker-compose.full.yml up -d
```

`JWT_SECRET` signs every login token. Compose refuses to start the stack without it, and the API refuses a `change-me` placeholder or anything under 32 bytes (see [Auth](#auth)).

Services started:
- `mongo` -- MongoDB 7
- `clickhouse` -- ClickHouse 24
- `redis` -- Redis 7
- `api` -- Purestat API server
- `tracker` -- Purestat tracker server
- `ui` -- Purestat frontend (served via Caddy)

### Docker Images

**Backend (`Dockerfile`):**

Multi-stage build using `cargo-chef` for optimal layer caching:

1. **Chef** -- Prepares the dependency recipe.
2. **Cook** -- Builds only dependencies (cached across code changes).
3. **Build** -- Compiles the full application.
4. **Runtime** -- Minimal `debian:bookworm-slim` image with the compiled binary.

```bash
docker build -t purestat-api .
```

**Frontend (`Dockerfile.ui`):**

Two-stage build:

1. **Build** -- Bun installs dependencies and builds the Vue SPA.
2. **Runtime** -- Caddy serves the static files with SPA fallback.

```bash
docker build -t purestat-ui -f Dockerfile.ui .
```

## Production Deployment

### Using docker-compose.prod.yml

The production compose file adds Caddy as a reverse proxy with automatic HTTPS via Let's Encrypt.

```bash
echo "JWT_SECRET=$(openssl rand -hex 32)" >> .env
docker-compose -f docker-compose.prod.yml up -d
```

### Caddy Configuration

The `Caddyfile.prod` configures:

- Automatic TLS certificate provisioning via Let's Encrypt.
- Reverse proxy for the API (`/api/*`) to the API server.
- Reverse proxy for the tracker (`/js/*`, `POST /api/event`) to the tracker server.
- Static file serving for the Vue SPA with HTML5 history mode fallback.
- Gzip compression.
- Security headers (HSTS, X-Content-Type-Options, X-Frame-Options).

Update the domain in `Caddyfile.prod` before deploying:

```
purestat.ai {
    # ...
}
```

### SSL/TLS

Caddy handles SSL/TLS automatically:

- Certificates are provisioned from Let's Encrypt on first request.
- Renewal is automatic (30 days before expiry).
- HTTP is automatically redirected to HTTPS.
- TLS 1.2+ with modern cipher suites.

No manual certificate management is needed. Ensure port 80 and 443 are open and the domain's DNS A record points to the server.

## Environment Variables

All configuration is done via environment variables using the `PURESTAT__SECTION__KEY` format (double underscore separators).

### Server

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__SERVER__HOST` | `0.0.0.0` | Bind address |
| `PURESTAT__SERVER__PORT` | `3000` | API server port |
| `PURESTAT__SERVER__TRACKER_PORT` | `3001` | Tracker server port |
| `PURESTAT__SERVER__CORS_ORIGINS` | `http://localhost:5173` | Comma-separated allowed CORS origins |

### Database

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__DATABASE__MONGODB_URI` | `mongodb://localhost:27017` | MongoDB connection string |
| `PURESTAT__DATABASE__MONGODB_NAME` | `purestat` | MongoDB database name |
| `PURESTAT__DATABASE__CLICKHOUSE_URL` | `http://localhost:8123` | ClickHouse HTTP URL |
| `PURESTAT__DATABASE__CLICKHOUSE_DB` | `purestat` | ClickHouse database name |
| `PURESTAT__DATABASE__CLICKHOUSE_USER` | `default` | ClickHouse username |
| `PURESTAT__DATABASE__CLICKHOUSE_PASSWORD` | (empty) | ClickHouse password |
| `PURESTAT__DATABASE__REDIS_URL` | `redis://localhost:6379` | Redis connection URL |

### Auth

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__JWT__SECRET` | (none, required) | Signs every login token. At least 32 bytes; generate it with `openssl rand -hex 32` |
| `PURESTAT__JWT__ACCESS_TOKEN_TTL_SECS` | `86400` | Access token lifetime, in seconds |
| `PURESTAT__JWT__REFRESH_TOKEN_TTL_SECS` | `604800` | Refresh token lifetime, in seconds |
| `PURESTAT__JWT__ISSUER` | `purestat` | The tokens' issuer |

Anyone who knows the JWT secret can sign in as any user, so the API **refuses to start** when it is unset, shorter than 32 bytes, or one of the `change-me…` placeholders this repository has shipped in `.env.example` and the compose files.

The session cookies (`access_token`, `refresh_token`) are `HttpOnly` and `SameSite=Lax`, and `Secure` when `PURESTAT__APP__FRONTEND_URL` is an `https` address (see [OAuth](#oauth)).

The visitor-hash salt needs no setting: the API generates a random one per day and keeps it in Redis (`PURESTAT__PRIVACY__SALT_TTL_HOURS`, default `48`).

### OAuth

Sign-in with Google and GitHub. It is off by default; with it off, `/api/oauth/*` answers `404` and the sign-in pages show no provider buttons.

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__OAUTH__ENABLED` | `false` | The master switch. `true` mounts `/api/oauth/*` |
| `PURESTAT__OAUTH__GOOGLE__CLIENT_ID` | (none) | The Google OAuth client's ID |
| `PURESTAT__OAUTH__GOOGLE__CLIENT_SECRET` | (none) | The Google OAuth client's secret |
| `PURESTAT__OAUTH__GITHUB__CLIENT_ID` | (none) | The GitHub OAuth App's client ID |
| `PURESTAT__OAUTH__GITHUB__CLIENT_SECRET` | (none) | The GitHub OAuth App's client secret |
| `PURESTAT__APP__FRONTEND_URL` | `http://localhost:5173` | The public address of purestat, where the SPA and `/api` are both served (`https://purestat.ai` in production). The callback addresses derive from it, and an `https` address makes every cookie the API sets `Secure`: the session cookies of a password login, a refresh and an OAuth sign-in, and the OAuth flow cookie. Over `http` (local development) none is. So an `https` address must really be served over https, or browsers drop the cookies |

Note the double underscore between the provider and the field (`GOOGLE__CLIENT_ID`). A provider takes part only when the switch is on and it has both a client ID and a secret; `GET /api/auth/providers` lists the ones that do, and the API logs which at startup. Only Google and GitHub exist: Facebook, LinkedIn and Microsoft were listed once but never worked, and their variables are ignored.

The callback addresses, which you register with each provider exactly as written:

| Provider | Callback address |
|----------|------------------|
| Google | `<PURESTAT__APP__FRONTEND_URL>/api/oauth/google/callback`, e.g. `https://purestat.ai/api/oauth/google/callback` |
| GitHub | `<PURESTAT__APP__FRONTEND_URL>/api/oauth/github/callback`, e.g. `https://purestat.ai/api/oauth/github/callback` |

**Google.** In the Google Cloud Console, under *APIs & Services*:

1. *OAuth consent screen*: the app's name and support address. The scopes purestat asks for are `openid`, `email` and `profile`, which need no Google review.
2. *Credentials* → *Create credentials* → *OAuth client ID*, application type *Web application*.
3. Under *Authorized redirect URIs*, add the Google callback address above. No JavaScript origin is needed: the browser never talks to Google's APIs itself.
4. Put the client ID and secret in `PURESTAT__OAUTH__GOOGLE__CLIENT_ID` and `..._CLIENT_SECRET`.

**GitHub.** Under *Settings* → *Developer settings* → *OAuth Apps* → *New OAuth App* (of a user or an organization):

1. *Homepage URL*: the public address, e.g. `https://purestat.ai`.
2. *Authorization callback URL*: the GitHub callback address above. Leave *Enable Device Flow* off.
3. Generate a client secret, and put the client ID and secret in `PURESTAT__OAUTH__GITHUB__CLIENT_ID` and `..._CLIENT_SECRET`.

purestat asks GitHub for the `user:email` scope only, to read which address is the account's primary one and whether it is verified.

Keep both secrets out of the repository and of any image: in Kubernetes they belong in the sealed `purestat-secret`. For local development, a Google client may have `http://localhost:5173/api/oauth/google/callback` as a redirect address (the Vite dev server proxies `/api`); a GitHub OAuth App has one callback address, so use a second app for development.

How the sign-in works and what it refuses is in [the API reference](api.md#oauth-sign-in).

### Stripe

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__STRIPE__SECRET_KEY` | (empty) | Stripe secret API key |
| `PURESTAT__STRIPE__WEBHOOK_SECRET` | (empty) | Stripe webhook signing secret |
| `PURESTAT__STRIPE__PRO_PRICE_ID` | (empty) | Stripe Price ID for the Pro plan |
| `PURESTAT__STRIPE__BUSINESS_PRICE_ID` | (empty) | Stripe Price ID for the Business plan |

### Rate Limiting

Limits are per client IP address, over a 60-second sliding window in Redis. The groups, headers and 429 response are described in [the API reference](api.md#rate-limiting).

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__RATE_LIMIT__ENABLED` | `true` | Set to `false` to turn rate limiting off |
| `PURESTAT__RATE_LIMIT__AUTH_RPM` | `10` | Auth endpoints: requests per minute per IP |
| `PURESTAT__RATE_LIMIT__API_RPM` | `300` | API endpoints: requests per minute per IP. One dashboard load makes about 15 calls, and realtime polls twice a minute. |
| `PURESTAT__RATE_LIMIT__TRACKER_RPM` | `1000` | Tracker endpoint: requests per minute per IP |
| `PURESTAT__RATE_LIMIT__STATS_RPM` | `120` | Stats/export endpoints: requests per minute per IP. One multi-site dashboard load makes 12 stats calls. |

The limits apply per client, so the API must see the client's address:

- **Behind a reverse proxy,** the proxy in front must append the address it saw to `X-Forwarded-For` (nginx: `proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;`), and every proxy between it and the API must pass that header on. Proxies on private addresses are skipped when the API reads the header from the right.
- **Without a proxy,** the API uses the address of the connection.

If every client seems to share one address, all of them share one limit. Check that the header reaches the API.

`.env.example` sets much higher limits for local development and the integration tests, which sign up and log in from one address.

### Logging

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Log level filter (trace, debug, info, warn, error) |

## Monitoring

### Health Check

The API server exposes a health endpoint:

```
GET /api/health
```

Returns `200 OK` with connectivity status for MongoDB, ClickHouse, and Redis. Use this for Docker health checks and load balancer probes.

### Logging

Purestat uses `tracing` with structured JSON logging in production. Configure the log level via `RUST_LOG`:

```bash
# Show all info and above
RUST_LOG=info

# Debug logging for purestat crates only
RUST_LOG=purestat=debug

# Trace-level for specific crate
RUST_LOG=purestat_api=trace
```

### Metrics

For production monitoring, consider:

- **ClickHouse system tables** -- Query `system.query_log` and `system.metrics` for database performance.
- **MongoDB profiler** -- Enable slow query logging for queries over 100ms.
- **Redis INFO** -- Monitor memory usage and connection counts.
- **Container metrics** -- Use Docker stats, cAdvisor, or Prometheus node exporter.

## Backup Strategy

### MongoDB

Use `mongodump` for regular backups:

```bash
# Full backup
mongodump --uri="mongodb://localhost:27017/purestat" --out=/backups/mongo/$(date +%Y%m%d)

# Restore
mongorestore --uri="mongodb://localhost:27017/purestat" /backups/mongo/20260225/
```

Schedule daily backups via cron:

```bash
0 2 * * * mongodump --uri="mongodb://localhost:27017/purestat" --out=/backups/mongo/$(date +\%Y\%m\%d) --gzip
```

### ClickHouse

Use ClickHouse's built-in backup mechanism:

```bash
# Create backup
clickhouse-client --query "BACKUP DATABASE purestat TO Disk('backups', 'purestat-$(date +%Y%m%d)')"

# Restore
clickhouse-client --query "RESTORE DATABASE purestat FROM Disk('backups', 'purestat-20260225')"
```

Alternatively, export partitions as Parquet files for archival:

```bash
clickhouse-client --query "SELECT * FROM purestat.events WHERE date >= '2026-01-01' AND date < '2026-02-01' FORMAT Parquet" > events-2026-01.parquet
```

### Redis

Redis is used as a cache and for rate limiting. It does not need to be backed up -- data is ephemeral and will be rebuilt automatically on restart.

### Backup Retention

| Database | Frequency | Retention |
|----------|-----------|-----------|
| MongoDB | Daily | 30 days |
| ClickHouse | Weekly | 90 days |
| Redis | N/A | N/A |

A backup keeps the rows of a site deleted after it was taken, until the backup itself expires. If your privacy policy promises deletion within a period, keep backups for less than that.

## Deleting a Site

Deleting a site deletes its rows in ClickHouse ([API](api.md#delete-apiorgorg_idsitesite_id)). If the API connects to ClickHouse as a restricted user, that user needs `ALTER DELETE` on the tables, besides `SELECT` and `INSERT`:

```sql
GRANT SELECT, INSERT ON purestat.* TO purestat_app;
GRANT ALTER DELETE ON purestat.events TO purestat_app;
GRANT ALTER DELETE ON purestat.sessions TO purestat_app;
```

Without it, deleting a site answers `503` and keeps the site.
