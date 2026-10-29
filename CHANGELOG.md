# Changelog

## Unreleased

Security and correctness fixes from a full review (`docs/superpowers/specs` has the design; the
review itself was not committed).

- **Key-value store**: a value stored with `?secret=true` is now served only to the read-write
  token; the read-only token and the dashboard's reveal without it get `403`. Docs say that
  scopes are namespaces, not an access boundary.
- **Composite actions**: every line of a multi-line secret is masked (`::add-mask::` is one line
  per command); inline values go through stdin so a value starting with `@` is stored verbatim
  instead of making curl read a runner file; random `GITHUB_ENV` delimiter; `;` in a branch name
  cannot inject a context pair; `tar --no-same-owner`, curl timeouts, temp files from `mktemp`.
- **`file://` store**: a failed or truncated upload is never renamed into place, and its cleanup
  no longer deletes a valid artifact a concurrent upload stored under the same hash.
- **Server**: `NX_CACHE_SHUTDOWN_GRACE_SECS` is applied (it was read but unused) and `/readyz`
  answers `503` while draining; an unreachable Redis is reported by `/readyz` without taking the
  pod out of the Service; store errors are logged, not echoed by `/readyz`; `?token=` is accepted
  on the SSE stream only; the memory key-value backend's LRU queue is compacted (it grew without
  bound under heavy reads).
- **Dashboard**: production builds never show generated data (mock mode is dev-only); the
  connect screen asks for the read-only token.
- **Chart**: refuses `memory` with autoscaling, `filesystem` with several replicas on a
  `ReadWriteOnce` volume, and `server.dashboard.token` when the token secret is not rendered;
  tokens set in values replace the stored ones on upgrade; `resourcesPreset` defaults to `medium`;
  NOTES warns about the memory key-value backend with several replicas; SSO examples bypass auth
  for all of `/v1/`.
- **Release workflow**: the chart job used a value that does not exist, so nothing after it ran;
  publishing now needs the fmt/clippy/test gate, the environment approval comes before the
  release commit and tag, and existing release assets are never overwritten.

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
- Chart: `redis.enabled=true` bundles a Redis (Bitnami subchart) for the key-value store; `NX_CACHE_KV_REDIS_PASSWORD`.
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
