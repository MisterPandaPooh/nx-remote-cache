# How Nx talks to a remote cache (what this server is built against)

Read from the Nx sources (`packages/nx/src/tasks-runner/cache.ts`,
`packages/nx/src/native/cache/{cache.rs,http_remote_cache.rs}`) and the Nx docs, October 2026.

## Where the cache sits

- **The daemon is not involved.** The Nx daemon watches files and keeps the project graph warm.
  Task output caching happens in the process that runs `nx run-many` / `nx affected`.
- **Local cache first.** `DbCache` (TypeScript) wraps the native `NxCache` (Rust + SQLite at
  `.nx/cache`). Every task hash is looked up locally; only misses go remote.
- **Remote cache selection**, in this order: Nx Cloud, the `@nx/{s3,shared-fs,gcs,azure}-cache`
  plugins (with Powerpack fallbacks), then **`NX_SELF_HOSTED_REMOTE_CACHE_SERVER`** → the native
  `HttpRemoteCache`. That last one is what this project implements.

## The HTTP client (`http_remote_cache.rs`)

- reqwest, system DNS, **no timeout, no retry, default redirects** (up to 10; `Authorization` is
  dropped on cross-host redirects).
- Env: `NX_SELF_HOSTED_REMOTE_CACHE_SERVER` (base URL),
  `NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN` (optional, `Authorization: Bearer`),
  `NODE_TLS_REJECT_UNAUTHORIZED=0` (skip certificate checks).
- Headers: always `Content-Type: application/octet-stream`; GET adds `Accept: application/octet-stream`.
- **GET** `/v1/cache/{hash}`: `200` → hit (body extracted), `404` → miss, **anything else → error
  shown to the developer**.
- **PUT** `/v1/cache/{hash}`: body is the whole archive in memory (so `Content-Length` is always
  set). `200` → stored, `409` and `403` → silently skipped, **anything else → error**. `put` runs
  inside a retry loop (6 attempts, ~20 s) and then **throws**, failing the task.

So a cache server must never answer a transient `5xx` lightly, must answer `404` fast, and must
make sure early answers (`409`, `403`, `413`) actually reach the client: this server drains the
request body before responding, because closing the socket with unread bytes makes the client see
a connection reset instead of the status.

## The artifact format

A gzip-compressed tar (`flate2` + `tar`), entries at the archive root:

| Entry | Content |
|---|---|
| output files | the task outputs, workspace-relative, symlinks not followed |
| `terminalOutput` | UTF-8 terminal log |
| `code` | exit code, 4 bytes big-endian |

On a hit the client unpacks into `.nx/cache/<hash>/`, records the entry in SQLite and restores
outputs into the workspace. The server treats the archive as opaque bytes: it never has to open it,
so a format change in Nx does not affect it.

## Local cache housekeeping (for context)

- `NX_MAX_CACHE_SIZE` / `nx.json#maxCacheSize` (default 10 % of the disk, fallback 100 GB),
  LRU by `accessed_at`, evicted down to 90 %.
- Entries not accessed for 7 days are removed. Remote retention is a separate concern:
  see `deploy/retention/README.md`.

## Load profile to expect

- `getBatch` fetches all local misses of a batch from the remote **in parallel, unbounded**.
  Expect bursts of hundreds of concurrent GETs after a cold checkout.
- Artifacts range from a few KB (lint) to hundreds of MB or GBs (built apps, `node_modules`
  snapshots). Uploads of large artifacts happen while other tasks run, so PUT and GET interleave.
