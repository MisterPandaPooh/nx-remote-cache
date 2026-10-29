# Architecture and development

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

## Spec conformance

[`spec/nx-remote-cache-openapi.json`](../spec/nx-remote-cache-openapi.json) is the OpenAPI document
published by Nx, copied verbatim. [`tests/spec_conformance.rs`](../tests/spec_conformance.rs) walks every
operation, status code and content type in it against the live router, and fails on any
difference in either direction. The matrix is in [`spec/README.md`](../spec/README.md). How the Nx
client, the local cache and the daemon actually behave, read from the Nx sources, is in
[`docs/nx-internals.md`](nx-internals.md).

## Development

```bash
cargo test                                   # unit + HTTP + spec conformance
cd ui && pnpm install && pnpm build && cd ..  # dashboard, embedded by the next cargo build
NX_CACHE_STORE_URL=file:///tmp/nx-cache NX_CACHE_TOKEN=dev cargo run
docker build -t nx-remote-cache:dev .        # multi-stage, ends on scratch
bench/run.sh                                 # comparative benchmark (needs Docker)
```

See [CONTRIBUTING.md](../CONTRIBUTING.md). Released under the [MIT license](../LICENSE).
