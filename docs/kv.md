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

Budget `NX_CACHE_KV_BYTES` (64 MB, LRU eviction), value cap `NX_CACHE_KV_MAX_VALUE` (1 MB),
keys `[A-Za-z0-9_.:@-]{1,200}`. The store lives in one process: with several replicas, pin
`/v1/kv` to one of them at the ingress, or accept per-pod views. Full API: [`docs/api.md`](api.md).
