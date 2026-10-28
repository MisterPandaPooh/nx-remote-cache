# Benchmark results

Generated 2026-10-28T22:49:55Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs on the host with 32 concurrent connections, 400 requests per phase.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on local disk | PUT 0 | req | req/s | p50 | p90 | p99 | 400 |
| nx-remote-cache (Rust) on local disk | GET hit | req | req/s | p50 | p90 | p99 | 400 |
| nx-remote-cache (Rust) on local disk | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | GET miss | req | req/s | p50 | p90 | p99 | 0 |

## Artifact size: 262144 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on local disk | PUT 0 | req | req/s | p50 | p90 | p99 | 400 |
| nx-remote-cache (Rust) on local disk | GET hit | req | req/s | p50 | p90 | p99 | 400 |
| nx-remote-cache (Rust) on local disk | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | GET miss | req | req/s | p50 | p90 | p99 | 0 |

## Artifact size: 4194304 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-remote-cache (Rust) on local disk | PUT 0 | req | req/s | p50 | p90 | p99 | 400 |
| nx-remote-cache (Rust) on local disk | GET hit | req | req/s | p50 | p90 | p99 | 400 |
| nx-remote-cache (Rust) on local disk | GET miss | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | PUT 400 | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | GET hit | req | req/s | p50 | p90 | p99 | 0 |
| nx-caching-server (Go) on local disk | GET miss | req | req/s | p50 | p90 | p99 | 0 |

