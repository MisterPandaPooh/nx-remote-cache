# Configuration reference

Everything is read from the environment once, at startup. A bad value stops the process with a
message naming the variable. Sizes accept `123`, `10KB`, `512MB`, `2GB`, `1GiB` (binary units,
case-insensitive); durations accept `45s`, `90m`, `12h`, `30d`, `2w` or a bare number of seconds;
booleans accept `1/0`, `true/false`, `yes/no`, `on/off`.

## Server

| Variable | Default | Description |
|---|---|---|
| `PORT` | `3000` | TCP port to listen on (the Docker image sets it to 3000). |
| `NX_CACHE_BIND` | `0.0.0.0` | IP address to bind. |
| `NX_CACHE_SHUTDOWN_GRACE_SECS` | `20` | On SIGTERM, how long in-flight requests may finish before the process exits. |
| `HOSTNAME` | `local` | Reported as the pod name in `/api/v1/stats`; Kubernetes sets it. |

The server speaks plain HTTP/1.1 and HTTP/2 (h2c). Terminate TLS at your ingress.

## Store

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_STORE_URL` | required | Where artifacts live: `az://<container>/<prefix>`, `s3://<bucket>/<prefix>`, `gs://<bucket>/<prefix>`, `file:///<dir>`, `memory:///`. |
| `NX_CACHE_STORE_TIMEOUT_SECS` | `300` | Timeout of one store request (a multipart part, a download). |
| `NX_CACHE_STORE_CONNECT_TIMEOUT_SECS` | `5` | TCP/TLS connect timeout to the store. |
| `NX_CACHE_STORE_MAX_RETRIES` | `3` | Retries on transient store errors (exponential backoff). |

Credentials come from the provider's usual variables; the store URL never carries them.

**Azure Blob** (`az://`): `AZURE_STORAGE_ACCOUNT_NAME`, then one of Workload Identity
(`AZURE_FEDERATED_TOKEN_FILE`, set by the AKS webhook), managed identity (nothing to set),
`AZURE_STORAGE_ACCOUNT_KEY`, `AZURE_STORAGE_SAS_KEY`, or a service principal
(`AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_CLIENT_SECRET`). Azurite: `AZURE_STORAGE_USE_EMULATOR=true`
plus `AZURE_STORAGE_ENDPOINT` (or `AZURITE_BLOB_STORAGE_URL`).

**S3** (`s3://`): `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`, `AWS_REGION`,
or IRSA / instance profile; `AWS_ENDPOINT` and `AWS_ALLOW_HTTP=true` for MinIO, S3Mock, R2.

**GCS** (`gs://`): `GOOGLE_SERVICE_ACCOUNT` (path or JSON), `GOOGLE_APPLICATION_CREDENTIALS`, or
workload identity on GKE.

**Local disk** (`file://`): the directory must exist and be writable by uid 65534 in the image.
Each artifact is one file named by its hash; uploads go to a dot-prefixed temp file and are renamed
into place, so a reader never sees a partial artifact.

## Authentication

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_TOKEN` | | Read-write bearer token(s), comma-separated. Use it on trusted branches. |
| `NX_CACHE_READ_TOKEN` | | Read-only token(s), comma-separated: `GET` works, `PUT` answers 403. Use it on pull requests. |
| `NX_CACHE_ENABLE_LOGIN` | `true` | `false` opens everything: the cache API accepts requests without a token and the dashboard shows no login screen, even if `NX_CACHE_UI_TOKEN` is set. Works out of the box; for private networks and trying it out. |
| `NX_CACHE_ALLOW_ANONYMOUS` | `false` | Older knob: accept cache requests without a token while the dashboard keeps asking for `NX_CACHE_UI_TOKEN` if one is set. The server refuses to start with no token unless one of the two is set. |

Tokens are compared in constant time. Generate them with `openssl rand -hex 32`. A client may
append context after the token (`secret;run=123;repo=org/app;ref=main;actor=alice`, up to 8
pairs); it is stored as object metadata and shown in the dashboard, never used for auth.

## Uploads and limits

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_MAX_ARTIFACT_BYTES` | `4GB` | Largest accepted upload; above it the server answers 413 (and reads up to 64 MiB of the body so the client sees the status). |
| `NX_CACHE_INLINE_PUT_THRESHOLD` | `8MB` | Uploads up to this size are buffered and written with one create-only call. |
| `NX_CACHE_MULTIPART_CHUNK_BYTES` | `8MB` | Part size for larger uploads (minimum `5MB`, a cloud-store rule). |
| `NX_CACHE_MULTIPART_CONCURRENCY` | `4` | Parts of one upload sent in parallel. |
| `NX_CACHE_MAX_CONCURRENT_PUTS` | `64` | Uploads in flight across the process; further ones wait. |

Memory for uploads is bounded by
`MAX_CONCURRENT_PUTS × max(INLINE_PUT_THRESHOLD, MULTIPART_CHUNK_BYTES × MULTIPART_CONCURRENCY)`:
with the defaults, 64 × 32 MB = 2 GB worst case. Lower `NX_CACHE_MAX_CONCURRENT_PUTS` (16 in the
Kubernetes example) or the chunk settings to fit your pod's memory limit.

## Memory tier

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_MEMORY_CACHE_BYTES` | `256MB` | LRU budget for hot artifacts served from RAM (`x-cache-tier: memory`). `0` disables it. |
| `NX_CACHE_MEMORY_CACHE_MAX_ITEM` | `4MB` | Artifacts above this size are never kept in memory. |

Uploads up to 1 MiB are copied into the tier on the way through; larger ones enter it on their
first read. Artifacts are immutable, so a cached copy is never stale.

## Key-value store (`/v1/kv`)

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_KV_ENABLED` | `true` | `false` switches the key-value store off: its routes answer 404 and the dashboard says so. |
| `NX_CACHE_KV_BYTES` | `64MB` | LRU budget of the in-memory key-value store. `0` also disables it. |
| `NX_CACHE_KV_MAX_VALUE` | `1MB` | Largest value accepted (413 above). |

Values keep the `Content-Type` they were stored with; JSON is validated. Per-key TTL with
`?ttl=30s`, isolation with `?level=global|pipeline|run` (identifiers from the token context),
`?secret=true` to mask a value in the dashboard. The store is per process; see
[`api.md`](api.md#key-value-store).

## CI integration

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_GITHUB_ENABLED` | `true` | Turn the run and repository context (`;run=…;repo=…` after the token) into GitHub links in the dashboard. `false` keeps the context but shows no links. |
| `NX_CACHE_GITHUB_URL` | `https://github.com` | Base URL for those links: set it for GitHub Enterprise Server. |

GitHub is the only CI provider integrated so far; others will come with their own variables.

## Retention

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_TTL` | unset | Delete artifacts whose store modification time is older than this (`30d`). Unset keeps them. |
| `NX_CACHE_MAX_TOTAL_BYTES` | unset | When the prefix exceeds this size, delete the oldest artifacts down to 90 % of it. |
| `NX_CACHE_SWEEP_INTERVAL` | `1h` | How often the sweeper runs (first pass 30 s after boot). Only runs when one of the two above is set. |

Prefer the store's own lifecycle policy for TTLs of a day or more
([`deploy/retention/`](../deploy/retention/README.md)); the sweeper adds what providers do not
offer (a hard size budget, retention on local disk, sub-day TTLs). It is safe with several replicas.

## Dashboard

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_UI_ENABLED` | `true` | Serve the dashboard on `/`. The shell is public; its API (`/api/v1/*`) is guarded. |
| `NX_CACHE_UI_TOKEN` | | Dedicated dashboard token. Without it, any cache token opens the dashboard. |
| `NX_CACHE_UI_TRUSTED_HEADER` | | Header set by an SSO proxy (`X-Auth-Request-Email`): a request carrying it passes the guard. Only expose the server through that proxy, and make it strip the header from clients. |

The browser keeps the token in `localStorage`; `?token=…` in the URL is read once and removed.

## Logging

| Variable | Default | Description |
|---|---|---|
| `NX_CACHE_LOG_LEVEL` | `info` | Base level: `error`, `warn`, `info`, `debug`, `trace`, `off`. |
| `RUST_LOG` | | Full `tracing` directives, overriding the level: `info,access=debug,object_store=trace`. |
| `NX_CACHE_LOG_FORMAT` | `json` | `json` (one object per line, fields flattened), `pretty` (human, colours), `compact`. |
| `NX_CACHE_ACCESS_LOG` | `false` | Log every request at `info` (`method`, `path`, `status`, `length`, `ms`). Off, requests are `debug`; failures (5xx) are always `warn`. |

Targets worth filtering: `access` (requests), `nx_remote_cache` (server), `object_store` and
`reqwest` (store traffic).

## Endpoints at a glance

| Path | Auth | Purpose |
|---|---|---|
| `PUT/GET /v1/cache/{hash}` | cache token | Nx remote cache API ([`api.md`](api.md)). |
| `PUT/GET/DELETE /v1/kv/{key}`, `GET /v1/kv` | cache token | In-memory key-value store. |
| `GET /api/v1/stats`, `/api/v1/events`, `/api/v1/events/stream` | dashboard guard | Dashboard data and SSE feed. |
| `GET /metrics` | none | Prometheus text format. |
| `GET /healthz`, `/readyz` | none | Liveness; readiness (store reachable). |
| `GET /` | none | Dashboard shell and assets. |

## Client side (Nx)

| Variable | Description |
|---|---|
| `NX_SELF_HOSTED_REMOTE_CACHE_SERVER` | Base URL of this server. |
| `NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN` | The token, optionally followed by `;run=…;repo=…` context. |
| `NODE_TLS_REJECT_UNAUTHORIZED=0` | Only for self-signed certificates in development. |

Nx uploads an artifact once (409 on a repeat is silently accepted), retries a failed upload six
times, and treats any `GET` status other than 200 and 404 as an error for that task. Details in
[`nx-internals.md`](nx-internals.md).
