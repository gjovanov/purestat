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
docker-compose -f docker-compose.full.yml up -d
```

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
| `PURESTAT__AUTH__JWT_SECRET` | (required) | Secret key for JWT signing |
| `PURESTAT__AUTH__JWT_EXPIRY` | `7d` | JWT token expiry duration |
| `PURESTAT__AUTH__HASH_SALT` | (required) | Salt for daily visitor hash rotation |

### OAuth

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__OAUTH__GOOGLE_CLIENT_ID` | (empty) | Google OAuth client ID |
| `PURESTAT__OAUTH__GOOGLE_CLIENT_SECRET` | (empty) | Google OAuth client secret |
| `PURESTAT__OAUTH__GITHUB_CLIENT_ID` | (empty) | GitHub OAuth client ID |
| `PURESTAT__OAUTH__GITHUB_CLIENT_SECRET` | (empty) | GitHub OAuth client secret |
| `PURESTAT__OAUTH__FACEBOOK_CLIENT_ID` | (empty) | Facebook OAuth client ID |
| `PURESTAT__OAUTH__FACEBOOK_CLIENT_SECRET` | (empty) | Facebook OAuth client secret |
| `PURESTAT__OAUTH__LINKEDIN_CLIENT_ID` | (empty) | LinkedIn OAuth client ID |
| `PURESTAT__OAUTH__LINKEDIN_CLIENT_SECRET` | (empty) | LinkedIn OAuth client secret |
| `PURESTAT__OAUTH__MICROSOFT_CLIENT_ID` | (empty) | Microsoft OAuth client ID |
| `PURESTAT__OAUTH__MICROSOFT_CLIENT_SECRET` | (empty) | Microsoft OAuth client secret |
| `PURESTAT__OAUTH__REDIRECT_BASE_URL` | `http://localhost:3000` | Base URL for OAuth callbacks |

### Stripe

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__STRIPE__SECRET_KEY` | (empty) | Stripe secret API key |
| `PURESTAT__STRIPE__WEBHOOK_SECRET` | (empty) | Stripe webhook signing secret |
| `PURESTAT__STRIPE__PRO_PRICE_ID` | (empty) | Stripe Price ID for the Pro plan |
| `PURESTAT__STRIPE__BUSINESS_PRICE_ID` | (empty) | Stripe Price ID for the Business plan |

### Rate Limiting

| Variable | Default | Description |
|----------|---------|-------------|
| `PURESTAT__RATE_LIMIT__AUTH_RPM` | `10` | Auth endpoints: requests per minute per IP |
| `PURESTAT__RATE_LIMIT__API_RPM` | `100` | API endpoints: requests per minute per IP |
| `PURESTAT__RATE_LIMIT__TRACKER_RPM` | `1000` | Tracker endpoint: requests per minute per IP |
| `PURESTAT__RATE_LIMIT__STATS_RPM` | `30` | Stats/export endpoints: requests per minute per IP |

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
