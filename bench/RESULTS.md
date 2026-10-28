# Benchmark results

Generated 2026-10-28T23:32:31Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs in a container on the same Docker network with 32 concurrent connections, 400 requests per phase.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 1282 | 5.3 | 22.96 | 38.58 | 49.50 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 3036 | 12.4 | 11.12 | 13.45 | 15.37 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3106 | 0.0 | 10.04 | 11.50 | 12.14 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 999 | 4.1 | 28.94 | 42.81 | 46.33 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 50228 | 205.7 | 0.41 | 0.69 | 1.03 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 2420 | 0.0 | 12.87 | 14.14 | 14.48 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 506 | 2.1 | 60.12 | 73.42 | 106.57 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1822 | 7.5 | 16.52 | 21.01 | 32.96 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1793 | 0.0 | 17.11 | 21.23 | 25.54 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 30120 | 123.4 | 0.78 | 1.52 | 3.50 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 58151 | 238.2 | 0.33 | 0.60 | 1.58 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 50450 | 0.0 | 0.50 | 0.80 | 1.18 | 0 |
| nx-caching-server (Go) on local disk | PUT | 27010 | 110.6 | 0.69 | 2.45 | 6.19 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 54313 | 222.5 | 0.40 | 0.68 | 1.04 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 57547 | 0.0 | 0.37 | 0.82 | 1.83 | 0 |

## Artifact size: 262144 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 676 | 177.1 | 51.55 | 63.01 | 74.97 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 1499 | 392.9 | 16.17 | 52.03 | 61.06 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 1791 | 0.0 | 17.17 | 18.99 | 37.61 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 404 | 105.8 | 54.99 | 114.95 | 331.84 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 9761 | 2558.8 | 2.70 | 5.44 | 8.60 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 1566 | 0.0 | 19.96 | 21.47 | 23.91 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 348 | 91.3 | 96.60 | 107.79 | 136.67 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1156 | 303.0 | 25.91 | 35.33 | 51.59 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1226 | 0.0 | 26.07 | 29.80 | 40.92 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 6652 | 1743.8 | 4.52 | 6.42 | 10.12 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 9349 | 2450.8 | 2.83 | 5.66 | 9.56 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 48132 | 0.0 | 0.53 | 0.89 | 1.06 | 0 |
| nx-caching-server (Go) on local disk | PUT | 17068 | 4474.3 | 1.12 | 3.40 | 6.61 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 9792 | 2567.0 | 2.33 | 5.15 | 19.52 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 54032 | 0.0 | 0.41 | 0.61 | 0.89 | 0 |

## Artifact size: 4194304 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 323 | 1354.7 | 105.50 | 146.90 | 173.98 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 597 | 2504.0 | 49.70 | 76.41 | 116.44 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 1192 | 0.0 | 26.09 | 28.69 | 30.21 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 234 | 980.6 | 133.62 | 196.76 | 226.66 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 631 | 2646.3 | 47.67 | 70.77 | 101.20 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 1148 | 0.0 | 27.24 | 28.95 | 29.80 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 105 | 441.3 | 168.64 | 486.92 | 1477.09 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 529 | 2218.1 | 59.55 | 70.67 | 103.08 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 963 | 0.0 | 31.94 | 33.95 | 41.02 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 860 | 3608.6 | 33.33 | 55.55 | 77.54 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 869 | 3645.0 | 33.07 | 52.07 | 76.61 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 52245 | 0.0 | 0.46 | 0.73 | 1.19 | 0 |
| nx-caching-server (Go) on local disk | PUT | 2652 | 11122.2 | 7.67 | 23.43 | 45.23 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 990 | 4153.8 | 29.96 | 45.25 | 68.66 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 39500 | 0.0 | 0.45 | 0.79 | 3.44 | 0 |

