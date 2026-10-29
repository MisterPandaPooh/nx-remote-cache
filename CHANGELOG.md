# Changelog

## 0.1.0 (2026-10-29)

First release.

- Nx self-hosted cache API (`PUT|GET /v1/cache/{hash}`) with spec-exact status codes.
- Stores: Azure Blob (Workload Identity, managed identity, SAS, account key, Azurite), S3, GCS, local disk, memory.
- Streaming uploads and downloads, create-only writes (409), bounded upload concurrency, in-memory hot tier.
- Retention: TTL and total-size sweeper, plus lifecycle policy recipes.
- Dashboard (React) with live events, token login, retention and latency views.
- Health, readiness, Prometheus metrics, JSON logs, graceful shutdown.
- Docker image on `scratch` (~4 MB), Kubernetes manifests, GitHub Actions CI, comparative benchmark.
- `/v1/kv`: in-memory LRU key-value store with content types, JSON validation and per-key TTL.
- Logging: `NX_CACHE_LOG_LEVEL`, `NX_CACHE_LOG_FORMAT` (`json`, `pretty`, `compact`), `NX_CACHE_ACCESS_LOG`.
- Security headers (CSP, `nosniff`, `Referrer-Policy`, `X-Frame-Options`), SHA-pinned workflow
  actions, job-scoped permissions, Dependabot, `SECURITY.md`, `docs/configuration.md`.
- Local-disk uploads batch body chunks into 1 MiB writes without copying them; mimalloc replaces musl's
  allocator in the static build (it was the bottleneck of both the server and the load generator).
