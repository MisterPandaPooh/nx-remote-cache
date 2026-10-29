<p align="center">
  <img src="images/logo.svg" width="112" alt="nx-remote-cache logo">
</p>

<h1 align="center">nx-remote-cache</h1>

<p align="center">
  <b>A self-hosted Nx remote cache that is small, fast and boring to run.</b><br>
  <i>One 4 MB static binary. Azure Blob, S3, GCS or local disk behind it. A dashboard in front of it.</i><br>
  Implements the official <a href="https://nx.dev/docs/kb/self-hosted-caching">Nx self-hosted cache API</a>, byte for byte, with a conformance test that walks the published OpenAPI spec.
</p>

<table align="center">
  <tr>
    <td align="center" width="25%"><h3>⚡</h3></td>
    <td align="center" width="25%"><h3>☁️</h3></td>
    <td align="center" width="25%"><h3>📊</h3></td>
    <td align="center" width="25%"><h3>🔒</h3></td>
  </tr>
  <tr>
    <td align="center" valign="top"><b>Built for the hot path</b><br><sub>Streaming in both directions, create-only writes, an in-memory tier for hot artifacts, sub-millisecond misses</sub></td>
    <td align="center" valign="top"><b>Any object store</b><br><sub>Azure Blob first (Workload Identity, SAS, key, Azurite), plus S3, GCS, local disk and memory, one URL to switch</sub></td>
    <td align="center" valign="top"><b>Nx Cloud-style dashboard</b><br><sub>Hit rate, throughput, latency percentiles, retention, live events linked to the CI run that produced them</sub></td>
    <td align="center" valign="top"><b>Kubernetes-native</b><br><sub>Scratch image, non-root, health and readiness probes, Prometheus metrics, graceful drain, manifests included</sub></td>
  </tr>
</table>

<p align="center">
  <a href="https://github.com/MisterPandaPooh/nx-remote-cache/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/MisterPandaPooh/nx-remote-cache/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/MisterPandaPooh/nx-remote-cache/pkgs/container/nx-remote-cache"><img alt="Docker image" src="https://img.shields.io/badge/ghcr.io-nx--remote--cache-2496ed?logo=docker&logoColor=white"></a>
  <img alt="Image size" src="https://img.shields.io/badge/image-4%20MB-2ea44f">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.85%2B-f74c00?logo=rust&logoColor=white">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-lightgrey">
</p>

<p align="center">
  <img src="images/dashboard.png" width="920" alt="nx-remote-cache dashboard">
</p>

## Why

Nx can push task outputs to any server that speaks two HTTP calls: `PUT /v1/cache/{hash}` and
`GET /v1/cache/{hash}`. Nx Cloud does that and much more; the community servers do it in Deno or Go
with a single backend. This one is written in Rust around three ideas:

1. **Never hold an artifact in memory.** Uploads stream into multipart writes, downloads stream out
   of the store. A 2 GB `node_modules` snapshot costs the same RAM as a 2 KB lint result.
2. **Reads are the product.** Misses answer in well under a millisecond, hits for small artifacts
   come straight from an LRU in memory, and every answer the Nx client could misread (409, 403, 413)
   is delivered reliably, because the request body is drained before responding.
3. **Nothing to operate.** One binary, one URL for the store, one token. Retention is a lifecycle
   policy on the bucket or a built-in sweeper. The dashboard is embedded in the same binary.

## Quick start

```bash
docker run --rm -p 3000:3000 \
  -e NX_CACHE_STORE_URL=file:///data -e NX_CACHE_TOKEN=dev-token \
  --tmpfs /data:uid=65534,gid=65534 \
  ghcr.io/misterpandapooh/nx-remote-cache:latest
```

Point Nx (≥ 20.8) at it:

```bash
export NX_SELF_HOSTED_REMOTE_CACHE_SERVER=http://localhost:3000
export NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN=dev-token
nx run-many -t build
```

Open <http://localhost:3000> and paste the token: that is the dashboard.

## Deploy on Kubernetes with Azure Blob

1. Create a storage account and a container (`az storage container create -n nx-cache ...`).
2. Give the pod an identity. With AKS Workload Identity the pod receives `AZURE_CLIENT_ID`,
   `AZURE_TENANT_ID` and `AZURE_FEDERATED_TOKEN_FILE` automatically; grant that identity
   *Storage Blob Data Contributor* on the container. No key leaves Azure. (Managed identity, SAS
   tokens and account keys work too, see [Stores](#stores).)
3. Apply the manifests, after editing the account name and the secret:

```bash
kubectl create namespace nx-remote-cache
kubectl -n nx-remote-cache create secret generic nx-remote-cache \
  --from-literal=NX_CACHE_TOKEN="$(openssl rand -hex 32)" \
  --from-literal=NX_CACHE_READ_TOKEN="$(openssl rand -hex 32)"
kubectl apply -k deploy/k8s
```

`deploy/k8s/` holds a Deployment (2 replicas, probes, non-root, read-only root FS), Service,
PodDisruptionBudget and an Ingress example with the body-size and timeout settings big artifacts
need. Terminate TLS at the ingress: the Nx client only trusts the OS certificate store.

For the dashboard behind your SSO (Keycloak, SAML, Kong) see [`deploy/sso/`](deploy/sso/README.md).
To try the Azure path locally, `docker compose -f deploy/docker-compose.yml up` starts the server
against [Azurite](https://github.com/Azure/Azurite).

## Stores

`NX_CACHE_STORE_URL` picks the backend; credentials come from the provider's usual environment
variables (read by [`object_store`](https://docs.rs/object_store)).

| Store | URL | Credentials |
|---|---|---|
| Azure Blob | `az://<container>/<prefix>` | `AZURE_STORAGE_ACCOUNT_NAME` + one of: Workload Identity (`AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_FEDERATED_TOKEN_FILE`), managed identity (nothing), `AZURE_STORAGE_SAS_TOKEN`, `AZURE_STORAGE_ACCOUNT_KEY`, `AZURE_STORAGE_CLIENT_SECRET`. Azurite: `AZURE_STORAGE_USE_EMULATOR=true` (+ `AZURITE_BLOB_STORAGE_URL`). |
| S3 / MinIO / R2 | `s3://<bucket>/<prefix>` | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`, optional `AWS_ENDPOINT` (+ `AWS_ALLOW_HTTP=true`), IRSA / instance profile |
| Google Cloud Storage | `gs://<bucket>/<prefix>` | `GOOGLE_SERVICE_ACCOUNT` / `GOOGLE_APPLICATION_CREDENTIALS`, or workload identity |
| Local disk | `file:///data/nx-cache` | none; mount a volume, `fsGroup: 65534` |
| Memory | `memory:///` | none; tests and demos |

Artifacts are stored as `<prefix>/<hash>`, immutable. On object stores the uploading CI run is
recorded as metadata (`nx_run`, `nx_repo`, ...), see [CI runs](#link-artifacts-to-ci-runs).

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `NX_CACHE_STORE_URL` | required | store, see above |
| `NX_CACHE_TOKEN` | | read-write token(s), comma-separated |
| `NX_CACHE_READ_TOKEN` | | read-only token(s): `PUT` answers 403 |
| `NX_CACHE_ALLOW_ANONYMOUS` | `false` | run without tokens (private networks only) |
| `PORT`, `NX_CACHE_BIND` | `3000`, `0.0.0.0` | listener |
| `NX_CACHE_MAX_ARTIFACT_BYTES` | `4GB` | larger uploads get 413 |
| `NX_CACHE_INLINE_PUT_THRESHOLD` | `8MB` | up to this size an upload is one atomic create; above, a streamed multipart upload |
| `NX_CACHE_MULTIPART_CHUNK_BYTES` | `8MB` | part size (≥ 5 MB) |
| `NX_CACHE_MULTIPART_CONCURRENCY` | `4` | parts in flight per upload |
| `NX_CACHE_MAX_CONCURRENT_PUTS` | `64` | uploads in flight per pod; bounds memory to about `64 × max(8MB, 8MB × 4)` |
| `NX_CACHE_MEMORY_CACHE_BYTES` | `256MB` | in-memory LRU for hot artifacts (`0` disables) |
| `NX_CACHE_MEMORY_CACHE_MAX_ITEM` | `4MB` | largest artifact kept in memory |
| `NX_CACHE_TTL` | off | retention: delete artifacts older than e.g. `30d` |
| `NX_CACHE_MAX_TOTAL_BYTES` | off | retention: keep the prefix under e.g. `500GB` (oldest first) |
| `NX_CACHE_SWEEP_INTERVAL` | `1h` | retention pass period |
| `NX_CACHE_STORE_TIMEOUT_SECS` / `_CONNECT_TIMEOUT_SECS` / `_MAX_RETRIES` | `300` / `5` / `3` | store client |
| `NX_CACHE_UI_ENABLED` | `true` | serve the dashboard at `/` |
| `NX_CACHE_UI_TOKEN` | | dedicated dashboard token (any cache token also works) |
| `NX_CACHE_UI_TRUSTED_HEADER` | | header set by an SSO proxy that opens the dashboard, e.g. `X-Auth-Request-Email` |
| `NX_CACHE_LOG_FORMAT`, `RUST_LOG` | `json`, `info` | logs |

Sizes accept `B/KB/MB/GB`, durations `s/m/h/d/w`.

## Performance

Measured with the included load generator against every server running in a container on the
same machine, same S3 bucket (S3Mock) or same RAM-backed disk, 32 concurrent connections, 1,000
requests per phase (400 for 4 MB artifacts) after an unmeasured warm-up. Full tables, method and how to rerun:
[`bench/RESULTS.md`](bench/RESULTS.md).

<!-- BENCH:START -->
**4 KB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 1,247 | 2,347 (10 MB/s) | 10.58 / 22.84 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 1,263 | 139,215 (570 MB/s) | 10.70 / 24.52 ms |
| nx-cache-server (Deno) on S3 | 764 | 1,986 (8 MB/s) | 13.95 / 21.08 ms |
| nx-remote-cache (Rust) on local disk | 50,984 | 129,013 (528 MB/s) | 0.25 / 0.80 ms |
| nx-caching-server (Go) on local disk | 55,563 | 93,779 (384 MB/s) | 0.12 / 1.41 ms |

**256 KB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 1,119 | 2,090 (548 MB/s) | 10.46 / 22.11 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 1,104 | 52,914 (13871 MB/s) | 11.03 / 24.39 ms |
| nx-cache-server (Deno) on S3 | 692 | 1,741 (456 MB/s) | 13.88 / 59.82 ms |
| nx-remote-cache (Rust) on local disk | 22,534 | 43,240 (11335 MB/s) | 0.25 / 0.92 ms |
| nx-caching-server (Go) on local disk | 19,041 | 51,449 (13487 MB/s) | 0.15 / 0.81 ms |

**4 MB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 409 | 997 (4182 MB/s) | 8.08 / 21.94 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 430 | 796 (3338 MB/s) | 8.48 / 20.33 ms |
| nx-cache-server (Deno) on S3 | 165 | 509 (2133 MB/s) | 17.40 / 27.72 ms |
| nx-remote-cache (Rust) on local disk | 2,897 | 4,023 (16875 MB/s) | 0.22 / 0.75 ms |
| nx-caching-server (Go) on local disk | 2,278 | 6,551 (27478 MB/s) | 0.14 / 0.40 ms |
<!-- BENCH:END -->

Reading the tables: on object storage, the arena that matters in production, the Rust server
uploads 1.6 to 2.5× faster than the Deno server, answers misses 25 to 50 % sooner, serves
store-path hits 20 to 95 % faster and hot artifacts from the memory tier 25 to 70× faster. On a
RAM-backed local disk against the Go server, uploads are a tie at 4 KB and 20 to 30 % faster at
256 KB and 4 MB; 4 KB hits are 40 % faster. The one place the Go server wins is large hits from
disk (4 MB: 1.6×): it hands the file to the socket with `sendfile(2)`, which hyper cannot do, so
we read it into memory first. The memory tier covers that case for anything hot.

What makes the difference:

- **Streaming, both ways.** The Rust server never buffers a body above the inline threshold;
  Deno's `nx-cache-server` reads uploads fully before sending them to S3.
- **Memory tier.** Hot artifacts up to 4 MB are served from RAM (`x-cache-tier: memory`),
  without a store round trip. Artifacts are immutable, so the copy can never be stale.
- **Misses are cheap.** A miss is one `HEAD`-sized `GET` on the store with no body, answered
  from a pool of keep-alive connections; on local disk it is a single `open`.
- **Nothing between the socket and the store.** hyper + tokio, no runtime, no GC, ~4 MB image.
- **mimalloc in the static build.** The `scratch` image is a musl binary, and musl's allocator
  serialises on a global lock; swapping it for mimalloc roughly doubled the local-disk numbers.

```bash
cargo build --release --bin nx-cache-bench
./target/release/nx-cache-bench --url https://nx-cache.example.com --token $TOKEN \
  --requests 500 --concurrency 32 --size 262144
```

## Retention

Recommended: a lifecycle policy on the bucket, it costs nothing and scales to millions of blobs.
[`deploy/retention/`](deploy/retention/README.md) has ready-made policies for Azure (true LRU with
last-access tracking), S3 and GCS. The built-in sweeper (`NX_CACHE_TTL`, `NX_CACHE_MAX_TOTAL_BYTES`)
adds what providers do not offer: a hard size budget, retention on local disk, TTLs under a day.
Deleting is always safe: a missing artifact is a cache miss and Nx rebuilds the task.

## Security

- **Cache API**: bearer tokens, compared in constant time. One read-write token for trusted
  branches, one read-only token for pull requests, so a fork cannot poison the cache.
- **Dashboard**: the shell is public, its API needs a token (`NX_CACHE_UI_TOKEN` or a cache token)
  entered once in the browser, or trusts an SSO proxy header. Keycloak/OIDC, SAML through
  Keycloak brokering, and Kong Gateway recipes: [`deploy/sso/`](deploy/sso/README.md).
- **Server**: hashes are validated (`[A-Za-z0-9_-]{1,128}`), uploads are size-capped and
  concurrency-capped, credentials never appear in logs or in `/api/v1/stats`.

## Use it from GitHub Actions

[`examples/github-actions/nx-ci.yml`](examples/github-actions/nx-ci.yml) is a complete workflow:
read-only token on pull requests, read-write on `main`, a warm job after each merge, and a job
summary with the hit rate.

The server is also a plain content-addressed blob store, so it works **outside Nx**. The composite
action in [`.github/actions/cache`](.github/actions/cache/action.yml) restores and saves any path by
key, like `actions/cache`, against your own server:

```yaml
- uses: MisterPandaPooh/nx-remote-cache/.github/actions/cache@main
  with:
    server: https://nx-cache.example.com
    token: ${{ secrets.NX_CACHE_TOKEN }}
    key: node-modules-${{ runner.os }}-${{ hashFiles('pnpm-lock.yaml') }}
    path: node_modules
```

### Link artifacts to CI runs

The Nx client sends nothing but the hash and a token. The token is opaque to Nx, so the server
accepts a context after the secret:

```yaml
NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN: "${{ secrets.NX_CACHE_TOKEN }};run=${{ github.run_id }};repo=${{ github.repository }};ref=${{ github.ref_name }};actor=${{ github.actor }}"
```

Every event then carries who asked (`ctx`) and, for hits, who produced the artifact (`origin`,
stored as blob metadata on upload). The dashboard turns that into links to the GitHub Actions runs:
"produced by run 1234 on `main`, reused by run 1290 on `feat/x`".

## Dashboard and observability

- `/` dashboard: hit rate ring, 24 h activity, latency p50/p90/p99 per operation, retention,
  memory tier, live event feed (Server-Sent Events) with CI run links, a connect snippet.
- `/api/v1/stats`, `/api/v1/events`, `/api/v1/events/stream`: the JSON behind it, see
  [`docs/api.md`](docs/api.md).
- `/metrics`: Prometheus (`nx_cache_requests_total{kind}`, bytes, latency histograms, memory
  tier, retention, store size). `/healthz`, `/readyz` (probes the store).
- Logs: one JSON line per cache request with method, path, status and latency.

Stats are per pod and in memory; they reset on restart. Point Prometheus at `/metrics` for history.

## Spec conformance

[`spec/nx-remote-cache-openapi.json`](spec/nx-remote-cache-openapi.json) is the OpenAPI document
published by Nx, copied verbatim. [`tests/spec_conformance.rs`](tests/spec_conformance.rs) walks every
operation, status code and content type in it against the live router, and fails on any
difference in either direction. The matrix is in [`spec/README.md`](spec/README.md). How the Nx
client, the local cache and the daemon actually behave, read from the Nx sources, is in
[`docs/nx-internals.md`](docs/nx-internals.md).

## Architecture

```
Nx CLI ──PUT/GET /v1/cache/{hash}──▶ axum + hyper (HTTP/1.1, h2c)
                                      │  auth (constant-time) · hash check · body counting
                                      │  memory tier (LRU, write-through)
                                      ▼
                              object_store ──▶ Azure Blob | S3 | GCS | disk | memory
                                      │
                 stats (atomics, histograms, 24 h timeline, event ring, SSE) ──▶ dashboard (React, embedded)
                 retention sweeper (TTL, size cap) · Prometheus · health
```

`src/` is about 1 500 lines: `routes.rs` (handlers), `store.rs` (object_store adapter),
`auth.rs`, `memcache.rs`, `stats.rs`, `config.rs`, `ui.rs`. The dashboard lives in `ui/`
(Vite + React + Tailwind, ~90 kB gzipped) and is embedded at build time.

## Development

```bash
cargo test                                   # unit + HTTP + spec conformance
cd ui && pnpm install && pnpm build && cd ..  # dashboard, embedded by the next cargo build
NX_CACHE_STORE_URL=file:///tmp/nx-cache NX_CACHE_TOKEN=dev cargo run
docker build -t nx-remote-cache:dev .        # multi-stage, ends on scratch
bench/run.sh                                 # comparative benchmark (needs Docker)
```

See [CONTRIBUTING.md](CONTRIBUTING.md). Released under the [MIT license](LICENSE).
