# nx-remote-cache — design

Self-hosted Nx remote cache server. Implements the official Nx "custom remote cache"
OpenAPI spec (`PUT|GET /v1/cache/{hash}`), runs as a small Docker image on Kubernetes,
stores artifacts in Azure Blob Storage first, with other stores behind the same abstraction.

## Intent

Nx Cloud is not wanted; the existing community servers (nx-cache-server: Deno + S3 only,
~50 stars) are thin. We want the fastest, most reliable implementation we can run ourselves:
tiny image, no artifact buffering, atomic writes, boring operations.

## What the Nx client actually does (reverse-engineered from `packages/nx/src/native/cache/http_remote_cache.rs`)

- reqwest client, system DNS, no timeout, no retry, default redirect policy (follows up to 10,
  drops `Authorization` on cross-host redirects).
- Env: `NX_SELF_HOSTED_REMOTE_CACHE_SERVER` (base URL), `NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN`
  (optional, sent as `Authorization: Bearer`), `NODE_TLS_REJECT_UNAUTHORIZED=0` disables TLS checks.
- Always sends `Content-Type: application/octet-stream`; GET adds `Accept: application/octet-stream`.
- PUT body is a full in-memory tar (`Vec<u8>`), so `Content-Length` is always present.
- GET: 200 → hit, 404 → miss, anything else → hard error shown to the user.
- PUT: 200 → stored, 409 and 403 → silently skipped, anything else (401, 5xx) → hard error.

Consequences: never return 5xx for a transient store error on PUT if we can avoid it; a flaky
server breaks developer builds. GET misses must be a fast 404.

## Goals / non-goals / anti-goals

Goals: spec-exact behaviour; streaming in both directions (no artifact fully in RAM);
atomic create (409 without a race); Azure Blob with Workload Identity, managed identity,
SAS or account key; local filesystem backend for dev and tests; S3 and GCS compiled behind the
same abstraction; health, readiness, Prometheus metrics, JSON logs, graceful SIGTERM;
< 15 MB image, non-root, runs on k8s with plain manifests.

Non-goals (v1): web UI, per-user tokens, OAuth, analytics, garbage collection inside the
server (use the store's lifecycle policy; documented), TLS termination (the ingress does it).

Anti-goals: anything that makes a cache miss slow, or that buffers a 500 MB tar in memory.

## Architecture (my calls)

- **Rust 2021**, `axum` 0.8 on `tokio`/`hyper` (HTTP/1.1 and h2c).
- **Storage: `object_store` 0.14** (Apache Arrow project). One `ObjectStore` trait, backends
  selected by URL: `az://<container>/<prefix>`, `file:///data`, `s3://bucket`, `gs://bucket`.
  Azure credentials from the standard env (`AZURE_STORAGE_ACCOUNT_NAME` + `AZURE_STORAGE_ACCOUNT_KEY`
  | `AZURE_STORAGE_SAS_TOKEN` | `AZURE_CLIENT_ID`+`AZURE_TENANT_ID`+`AZURE_FEDERATED_TOKEN_FILE`
  (AKS Workload Identity) | IMDS managed identity). Azurite supported for local tests.
- **PUT**: validate `hash` (`^[A-Za-z0-9_-]{1,128}$`), require `Content-Length`, reject above
  `CACHE_MAX_ARTIFACT_BYTES` (default 1 GiB) with 413, stream the body into a multipart upload,
  commit with `PutMode::Create` → store rejects existing blob → **409**. Small bodies
  (< 8 MiB) go through a single put.
- **GET**: stream the blob to the client with `Content-Length`; store `NotFound` → **404**.
  Optional later: `302` to a short-lived SAS URL so downloads bypass the pod.
- **Auth (minimal)**: one `NX_CACHE_TOKEN` (read-write) and an optional `NX_CACHE_READ_TOKEN`
  (PUT → 403). Constant-time compare. If no token is configured the server refuses to start
  unless `NX_CACHE_ALLOW_ANONYMOUS=true`.
- **Ops**: `/healthz` (process up), `/readyz` (store probe), `/metrics` (Prometheus:
  requests, bytes, latency, hits/misses), `tracing` JSON logs, request timeouts, graceful
  shutdown on SIGTERM with a drain window.
- **Layout**: `src/{main,config,app,auth,store,routes}.rs`, `tests/` (HTTP integration on the
  filesystem store), `deploy/k8s/` (Deployment, Service, Secret example, kustomization),
  `Dockerfile` (multi-stage, static musl, `scratch`, non-root), `.dockerignore`, `.gitignore`
  (`target/`), `.github/workflows/ci.yml` (fmt, clippy, test, image build).
- **Size controls**: release profile `lto="fat"`, `codegen-units=1`, `strip=true`,
  `panic="abort"`; `target/` never committed or sent to Docker.

## Left to the builder

Exact metric names, log fields, Azurite test wiring, and the k8s resource requests.

## Addendum (2026-10-29, same day): UI, stats API, performance

**Performance is the differentiator.** Targets: cache miss (404) under 2 ms server time on a
local store, GET/PUT bounded by store bandwidth not by the server, memory flat under
concurrent 500 MB uploads, one static binary with no runtime. A `bench` tool in the repo
measures GET hit/miss and PUT throughput against any running instance.

**Stats and events (per pod, in memory, lock-free counters):**
- `GET /api/v1/stats` → counters (hits, misses, puts, conflicts, forbidden, errors, bytes in/out),
  latency histogram buckets per route, uptime, store kind, recent window (last 24h in 5-min buckets).
- `GET /api/v1/events?limit=N` → ring buffer of the last 1000 events
  `{ts, kind: hit|miss|put|conflict|error, hash, bytes, ms}`.
- `GET /api/v1/events/stream` → Server-Sent Events, live feed of the same events.
- Multi-replica note: each pod reports its own numbers; the UI shows the pod it hits. Shared
  persistence is a later step.

**UI (React, side project in `ui/`):** Vite + React + TypeScript + Tailwind. Look inspired by
Nx Cloud: dark theme, one dashboard page with hit-rate ring, throughput sparkline, counters,
live activity table, storage card, and a "connect your workspace" snippet showing the two env
vars. Built to `ui/dist`, embedded in the Rust binary (`rust-embed`) and served at `/`.
No auth on the UI in v1: expose it only through the cluster-internal ingress, or set
`NX_CACHE_UI_TOKEN` to require `?token=` / Bearer on `/api/v1/*` and `/`.
