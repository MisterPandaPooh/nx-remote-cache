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
- Benchmark against every open-source Nx cache server with a published image (seven servers, two arenas); honest notes on where the Go servers are ahead.
- Key-value store `redis` backend (`NX_CACHE_KV_BACKEND=redis`, `NX_CACHE_KV_REDIS_URL`): keys shared by
  every replica with Redis' own expiry; `/readyz` pings it.
- Key-value store: isolation levels (`global`, `pipeline`, `run`, identifiers from the token context),
  `?secret=true` values masked in the dashboard with a reveal button, a dashboard view of every key,
  `NX_CACHE_KV_ENABLED`.
- GitHub integration switches: `NX_CACHE_GITHUB_ENABLED`, `NX_CACHE_GITHUB_URL` (GitHub is the only
  CI provider so far).
- `NX_CACHE_ENABLE_LOGIN=false`: works out of the box, no token for the cache API and no login screen.
- Helm chart on the Bitnami `common` library, published as OCI on ghcr.io; images on ghcr.io and Docker Hub,
  signed with cosign, with SBOM and provenance; release workflow (`docs/release.md`).
- Branding: new logo, name and taglines say "free, self-hosted remote cache for Nx" with a trademark note, so the project is not mistaken for Nx or Nx Cloud.
- `/v1/kv`: in-memory LRU key-value store with content types, JSON validation and per-key TTL.
- Logging: `NX_CACHE_LOG_LEVEL`, `NX_CACHE_LOG_FORMAT` (`json`, `pretty`, `compact`), `NX_CACHE_ACCESS_LOG`.
- Security headers (CSP, `nosniff`, `Referrer-Policy`, `X-Frame-Options`), SHA-pinned workflow
  actions, job-scoped permissions, Dependabot, `SECURITY.md`, `docs/configuration.md`.
- Local-disk uploads batch body chunks into 1 MiB writes without copying them; mimalloc replaces musl's
  allocator in the static build (it was the bottleneck of both the server and the load generator).
