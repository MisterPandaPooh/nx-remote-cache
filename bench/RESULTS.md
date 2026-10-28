# Benchmark results

Generated 2026-10-28T23:00:10Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs in a container on the same Docker network with 32 concurrent connections, 400 requests per phase.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 899 | 3.7 | 23.32 | 96.90 | 113.84 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2038 | 8.3 | 12.91 | 34.20 | 54.44 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 2733 | 0.0 | 8.08 | 15.40 | 73.39 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 1648 | 6.8 | 17.04 | 31.84 | 44.49 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 43099 | 176.5 | 0.49 | 0.73 | 1.02 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 2778 | 0.0 | 8.05 | 15.85 | 50.00 | 0 |
