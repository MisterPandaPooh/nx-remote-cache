# HTTP API

## Nx cache endpoints (spec: https://nx.dev/docs/kb/self-hosted-caching)

| Method | Path | Auth | Responses |
|---|---|---|---|
| `PUT` | `/v1/cache/{hash}` | Bearer (rw) | 200 stored · 401 bad token · 403 read-only token · 409 exists · 413 too large · 411 no Content-Length |
| `GET` | `/v1/cache/{hash}` | Bearer (rw or ro) | 200 `application/octet-stream` · 401 · 404 miss |

`hash` must match `^[A-Za-z0-9_-]{1,128}$`, else 400.

## Ops

| Path | Purpose |
|---|---|
| `GET /healthz` | process alive → `200 ok` |
| `GET /readyz` | store reachable (HEAD on a probe key) → `200 ok` or `503` |
| `GET /metrics` | Prometheus text format |

## Dashboard API (consumed by `ui/`)

All JSON. No auth unless `NX_CACHE_UI_TOKEN` is set (then `Authorization: Bearer` or `?token=`).

### `GET /api/v1/stats`

```json
{
  "version": "0.1.0",
  "uptime_seconds": 12345,
  "started_at": "2026-10-29T10:00:00Z",
  "store": { "kind": "azure", "location": "az://nx-cache/" },
  "pod": "nx-remote-cache-7d9f-abcde",
  "counters": {
    "hits": 1200, "misses": 300, "puts": 290, "conflicts": 4,
    "unauthorized": 0, "forbidden": 1, "errors": 0,
    "bytes_served": 123456789, "bytes_stored": 98765432
  },
  "latency_ms": {
    "get_hit":  { "p50": 3.1, "p90": 12.0, "p99": 40.0, "count": 1200 },
    "get_miss": { "p50": 0.8, "p90": 1.5, "p99": 3.0, "count": 300 },
    "put":      { "p50": 25.0, "p90": 120.0, "p99": 400.0, "count": 290 }
  },
  "timeline": [
    { "ts": "2026-10-29T10:00:00Z", "hits": 10, "misses": 2, "puts": 2, "bytes_served": 1000, "bytes_stored": 500 }
  ]
}
```

`timeline` = last 24 h in 5-minute buckets (288 entries, oldest first, zero-filled).

### `GET /api/v1/events?limit=100`

```json
{ "events": [ { "ts": "2026-10-29T10:00:00.123Z", "kind": "hit", "hash": "abc…", "bytes": 1234, "ms": 3.2, "status": 200 } ] }
```

`kind` ∈ `hit | miss | put | conflict | forbidden | unauthorized | error`. Newest first.

### `GET /api/v1/events/stream`

Server-Sent Events; each `data:` line is one event object as above.

### `retention` block in `GET /api/v1/stats` (added)

```json
"retention": {
  "ttl_seconds": 2592000, "max_total_bytes": 536870912000, "max_artifact_bytes": 4294967296,
  "sweep_interval_seconds": 3600, "deleted_total": 120, "bytes_freed_total": 9876543210,
  "last_sweep": { "at": "2026-10-29T10:00:00Z", "scanned": 51234, "deleted": 12, "bytes_freed": 123456, "total_bytes": 4000000000, "duration_ms": 850 }
}
```
`ttl_seconds`, `max_total_bytes` and `last_sweep` are `null` when retention is off.

## Key-value store

Not part of the Nx API. An in-memory LRU store for small values that CI jobs share, with the same
bearer tokens as the cache (read-only tokens may only `GET`). Disabled with `NX_CACHE_KV_BYTES=0`
(every route then answers 404).

| Method and path | Request | Responses |
|---|---|---|
| `PUT /v1/kv/{key}[?ttl=30s]` | body = the value, `Content-Type` kept as is (default `application/octet-stream`); `application/json` bodies are validated | `204` stored · `400` bad key, bad TTL or invalid JSON · `401` · `403` read-only token · `413` above `NX_CACHE_KV_MAX_VALUE` |
| `GET /v1/kv/{key}` | | `200` with the stored `Content-Type`, `Cache-Control: no-store`, `x-kv-expires-in: <seconds>` when a TTL is set · `404` unknown or expired |
| `DELETE /v1/kv/{key}` | | `204` · `404` · `403` read-only token |
| `GET /v1/kv[?prefix=p&limit=100]` | | `200 {"keys":[{"key","size","content_type","expires_in_secs"?}]}`, sorted, at most 1000 |

Keys match `[A-Za-z0-9_.:@-]{1,200}`. TTLs accept `45s`, `10m`, `12h`, `7d` or seconds. When the
budget is exceeded the least recently read or written key is evicted first; expired keys go on
access or eviction. Metrics: `nx_cache_kv_items`, `nx_cache_kv_bytes`.

```bash
curl -X PUT -H "Authorization: Bearer $TOKEN" -H "Content-Type: text/plain" --data 'a1b2c3d' \
  "$SERVER/v1/kv/last-green:main"
curl -X PUT -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" --data '42' \
  "$SERVER/v1/kv/build-number?ttl=7d"
```
