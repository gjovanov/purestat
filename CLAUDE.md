# Purestat

Privacy-first, cookie-free web analytics SaaS platform.

## Architecture

Rust workspace with 6 crates: `config` → `db` → `services` → `api` / `tracker` / `tests`

- **config**: Settings loaded from `PURESTAT__*` env vars
- **db**: MongoDB models + ClickHouse schemas + index management
- **services**: DAOs (BaseDao pattern), auth (JWT + argon2), analytics (privacy hashing, ingest, query), stripe
- **api**: Axum REST API with auth extractors, 14 route modules
- **tracker**: Standalone lightweight event ingest server
- **tests**: Integration tests against real MongoDB + ClickHouse

## Key Patterns

- Follow roomler2 patterns exactly (AppState, BaseDao, ApiError, AuthUser extractor)
- MongoDB: `id: Option<ObjectId>` with `#[serde(rename = "_id")]`, `COLLECTION` const, soft deletes
- ClickHouse: `clickhouse` crate with `Row` derive, `time::OffsetDateTime` for timestamps
- Auth: `FromRequestParts` extractor (Bearer header → cookie fallback)
- Privacy: `SHA-256(daily_salt + domain + ip + user_agent)`, daily salt rotation via Redis

## Commands

```bash
# Dev infrastructure
docker-compose up -d

# Run API
cargo run -p purestat-api

# Run tracker
cargo run -p purestat-tracker

# Integration tests
cargo test -p purestat-tests

# Frontend
cd ui && bun install && bun run dev
```

## Environment

All config via `PURESTAT__SECTION__KEY` env vars. See `.env.example`.
