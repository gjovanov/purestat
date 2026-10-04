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

Deployment configuration lives in the private sibling repo `gjovanov/purestat-deploy`: Kustomize manifests under `k8s/base/` + `k8s/overlays/prod/`. One pod on `k8s-worker-2`, NodePort 30050 (HTTP → Caddy → api:3000 + tracker:3001 + the UI's static files).

Two custom images:
- `purestat-backend` runs the api, the tracker and the geoip cronjob.
- `purestat-ui` is Caddy serving the Vue SPA and `/js/purestat.js`.

**GitOps.** ArgoCD reconciles the `purestat` Application from `purestat-deploy` at **`main`**, path `k8s/overlays/prod`, with **automated sync (prune + self-heal)**.
- A merge that changes a `newTag` on `main` *is* the production roll, within seconds.
- Self-heal reverts a manual `kubectl` edit, so change git instead.

**Image registry.** `registry.roomler.ai` is self-hosted with basic auth, and the build host is logged in. The pull secret is `regcred`. The weekly retention keeps 2 tags per repo, so push exactly one new tag per release; the previous one stays available for rollback.

**Secrets.** `purestat-secret`, `clickhouse-secret`, `mongodb-secret` and `clickhouse-app-user` are Bitnami SealedSecrets under `k8s/base/sealed/`. Seal with `kubeseal --format yaml < secret.yaml` on a host with cluster access, and never commit the plain Secret.

**ClickHouse users.** The api and tracker connect as `purestat_app`, which holds SELECT and INSERT on `purestat.*` only. Its users.d file and password come from the `clickhouse-app-user` SealedSecret. The user in `clickhouse-secret` is for administration only.

**Caddyfile.** It lives in `k8s/base/caddy/Caddyfile` and is generated into a ConfigMap whose name carries a hash of its content, so an edit rolls the pod.

### Deployment workflow

Build images only from a tag, never from a working tree. Production once ran uncommitted code for five months, and the roll to committed code silently dropped features (#5).

```bash
# 1. Release: merge to master, then tag it.
git tag -a vX.Y.Z -m "..." && git push origin vX.Y.Z

# 2. Build from the tag's committed bytes.
git fetch --tags
rm -rf /tmp/purestat-vX.Y.Z && mkdir /tmp/purestat-vX.Y.Z
git -c core.autocrlf=false archive vX.Y.Z | tar -x -C /tmp/purestat-vX.Y.Z
cd /tmp/purestat-vX.Y.Z
docker build -f Dockerfile        -t registry.roomler.ai/purestat-backend:build-vX.Y.Z .
docker build -f Dockerfile.ui.k8s -t registry.roomler.ai/purestat-ui:build-vX.Y.Z .

# 3. Test the IMAGE, not just the source:
#    docker compose up -d mongo clickhouse redis, run the image on that network,
#    then run the integration suite against it (API_URL=...).

# 4. Push one tag per changed image.
TAG=v$(date -u +%Y%m%d)-$(docker images -q registry.roomler.ai/purestat-backend:build-vX.Y.Z | head -c 12)
docker tag registry.roomler.ai/purestat-backend:build-vX.Y.Z registry.roomler.ai/purestat-backend:$TAG
docker push registry.roomler.ai/purestat-backend:$TAG

# 5. Roll: open a PR on purestat-deploy that changes newTag in
#    k8s/overlays/prod/kustomization.yaml. Merging it is the deploy.
#    Rollback: revert that PR.

# 6. Verify: pods Ready on the new tag, https://purestat.ai/api/health,
#    and the UI after a reload.
```

- `git -c core.autocrlf=false archive` matters on Windows. A checkout with `autocrlf=true` gives the image CRLF shell scripts, and the container then fails with `exec … no such file or directory`.
- The UI is served with `Cache-Control: no-cache` on `index.html` and `immutable` on the hashed `/assets/*`, so a deploy reaches browsers on their next page load.

## K8s deployment placement

Cluster has three zones via `topology.kubernetes.io/zone`: `mars`,
`zeus`, `jupiter` (one master + one worker VM per bare-metal host).
Apps are split by tier (added 2026-05-01 after a mars-host overload
incident):

  - `tier=high-performance` (zeus + jupiter workers): this app, plus
    roomler / roomler-ai / oxmux / lgr / purestat / tickytack / clawui
    (when migrated to K8s).
  - `tier=utility` (mars worker): bauleiter, regal, monitoring stack,
    docker registry, image builds.

Enforced via a Kustomize patch in `purestat-deploy/k8s/overlays/prod/
kustomization.yaml` that puts a required `nodeAffinity` on every
Deployment + StatefulSet. Hostname pins in `base/` are retained where
the StatefulSet PVC uses node-local storage; the tier requirement is
an *additional* constraint — both must match.
