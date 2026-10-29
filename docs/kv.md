# Key-value store

Beside the cache, `/v1/kv` is a small in-memory LRU store for CI glue that has nothing to do
with Nx: a feature flag, the last green commit, a JSON blob shared between jobs. Values keep
the `Content-Type` they were stored with (JSON is validated, anything else is stored verbatim:
strings, numbers, binary), every key can carry a TTL, and the same tokens apply (read-only
tokens can only read).

```bash
curl -X PUT -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  --data '{"canary":true,"ratio":0.25}' "https://nx-cache.example.com/v1/kv/flags?ttl=12h"
curl -H "Authorization: Bearer $TOKEN" https://nx-cache.example.com/v1/kv/flags      # 200, application/json
curl -H "Authorization: Bearer $TOKEN" "https://nx-cache.example.com/v1/kv?prefix=fl" # {"keys":[...]}
curl -X DELETE -H "Authorization: Bearer $TOKEN" https://nx-cache.example.com/v1/kv/flags
```

Three scopes: `?level=global` (default), `?level=pipeline` (one namespace per repository) and
`?level=run` (one per CI run); the identifiers come from the token context the cache already
receives (`;run=…;repo=…`) or from `?pipeline=` / `?run=`. Scopes keep keys apart, they are not
an access boundary: any token can name any scope. `?secret=true` marks a value: only the
read-write token can read it back (the read-only token, the one pull requests hold, gets 403),
and the dashboard masks it until someone holding the read-write token clicks reveal. The
dashboard lists every key of every scope.

Running several replicas? Set `NX_CACHE_KV_BACKEND=redis` and `NX_CACHE_KV_REDIS_URL`: keys live
in your Redis (one round trip per operation, Redis' own expiry), visible to every pod. The
default `memory` backend keeps them in one process.

Budget `NX_CACHE_KV_BYTES` (64 MB, LRU eviction of the memory backend), `NX_CACHE_KV_ENABLED=false` to switch the store off, value cap `NX_CACHE_KV_MAX_VALUE` (1 MB),
keys `[A-Za-z0-9_.:@-]{1,200}`. The store lives in one process: with several replicas, pin
`/v1/kv` to one of them at the ingress, or accept per-pod views. Full API: [`docs/api.md`](api.md).
