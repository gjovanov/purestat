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

## Deployment

Deployment configuration lives in sibling repo `../purestat-deploy/` (GitHub: `gjovanov/purestat-deploy`). Kustomize manifests under `k8s/base/` + `k8s/overlays/prod/`. Pod runs on `k8s-worker-2`, NodePort 30050 (HTTP→Caddy→api:3000 + tracker:3001 + UI static).

Purestat has **two custom images**: `purestat-backend` (shared between api, tracker, geoip-cronjob containers) and `purestat-ui` (Caddy serving the Vue SPA).

**GitOps**: ArgoCD at [argocd.roomler.ai](https://argocd.roomler.ai) reconciles the `purestat` Application from `github.com/gjovanov/purestat-deploy @ gitops-pilot` path `k8s/overlays/prod`. Sync policy is **Manual**.

**Image registry**: `registry.roomler.ai` (self-hosted Docker Registry v2 on mars, basic auth). Pull secret `regcred` in the `purestat` namespace.

Secrets (`purestat-secret`, `clickhouse-secret`, `mongodb-secret`) are sealed via Bitnami SealedSecrets and committed to git under `k8s/base/sealed/`.

### Deployment Workflow

```bash
ssh mars
cd /home/gjovanov/purestat && git pull

# Build both images
docker build -f Dockerfile      -t registry.roomler.ai/purestat-backend:build-$$ .
docker build -f Dockerfile.ui.k8s -t registry.roomler.ai/purestat-ui:build-$$ .

TAG_B="v$(date +%Y%m%d)-$(docker images -q registry.roomler.ai/purestat-backend:build-$$ | head -c 12)"
TAG_U="v$(date +%Y%m%d)-$(docker images -q registry.roomler.ai/purestat-ui:build-$$ | head -c 12)"

docker tag registry.roomler.ai/purestat-backend:build-$$ registry.roomler.ai/purestat-backend:$TAG_B
docker tag registry.roomler.ai/purestat-backend:build-$$ registry.roomler.ai/purestat-backend:latest
docker tag registry.roomler.ai/purestat-ui:build-$$ registry.roomler.ai/purestat-ui:$TAG_U
docker tag registry.roomler.ai/purestat-ui:build-$$ registry.roomler.ai/purestat-ui:latest
docker push registry.roomler.ai/purestat-backend:$TAG_B && docker push registry.roomler.ai/purestat-backend:latest
docker push registry.roomler.ai/purestat-ui:$TAG_U      && docker push registry.roomler.ai/purestat-ui:latest

cd /home/gjovanov/purestat-deploy && git checkout gitops-pilot
# Edit k8s/overlays/prod/kustomization.yaml — update both newTag values
git commit -am "chore(k8s): bump purestat to $TAG_B / $TAG_U" && git push

argocd app sync purestat --grpc-web
curl -sI https://purestat.ai/                # HTTP 200
```

Registry retention: weekly cron keeps at most 2 tags per repo.
