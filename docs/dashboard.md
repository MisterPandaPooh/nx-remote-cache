# Dashboard and observability

- `/` dashboard: hit rate ring, 24 h activity, latency p50/p90/p99 per operation, retention,
  memory tier, live event feed (Server-Sent Events) with CI run links, a connect snippet.
- `/api/v1/stats`, `/api/v1/events`, `/api/v1/events/stream`: the JSON behind it, see
  [`docs/api.md`](api.md).
- `/metrics`: Prometheus (`nx_cache_requests_total{kind}`, bytes, latency histograms, memory
  tier, retention, store size). `/healthz`, `/readyz` (probes the store).
- Logs: one JSON line per cache request with method, path, status and latency.

Stats are per pod and in memory; they reset on restart. Point Prometheus at `/metrics` for history.

<p align="center"><img src="../images/connect.png" width="560" alt="Dashboard token prompt"></p>
