# Benchmark results

Generated 2026-10-28T23:42:30Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs in a container on the same Docker network with 32 concurrent connections, 400 requests per phase.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 908 | 3.7 | 21.98 | 94.41 | 124.61 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 1712 | 7.0 | 14.24 | 37.02 | 81.33 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3841 | 0.0 | 9.03 | 13.73 | 23.09 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 887 | 3.6 | 24.98 | 98.08 | 113.81 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 45020 | 184.4 | 0.47 | 0.77 | 1.04 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3619 | 0.0 | 10.07 | 14.99 | 24.17 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 589 | 2.4 | 39.16 | 96.00 | 120.07 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1833 | 7.5 | 16.40 | 22.89 | 38.64 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1761 | 0.0 | 14.70 | 25.20 | 58.89 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 28247 | 115.7 | 0.82 | 1.70 | 3.32 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 53328 | 218.4 | 0.34 | 0.61 | 0.93 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 53420 | 0.0 | 0.45 | 0.72 | 1.09 | 0 |
| nx-caching-server (Go) on local disk | PUT | 27221 | 111.5 | 0.67 | 2.49 | 4.16 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 42178 | 172.8 | 0.32 | 0.81 | 3.25 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 57260 | 0.0 | 0.33 | 0.58 | 0.80 | 0 |

## Artifact size: 262144 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 758 | 198.7 | 28.24 | 104.36 | 134.51 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 1248 | 327.1 | 12.92 | 72.72 | 94.76 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3917 | 0.0 | 8.89 | 15.32 | 24.24 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 698 | 182.9 | 26.60 | 115.86 | 181.04 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 10903 | 2858.2 | 2.07 | 4.86 | 11.53 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3528 | 0.0 | 10.77 | 14.55 | 26.12 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 499 | 130.7 | 48.00 | 122.08 | 174.51 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1397 | 366.2 | 18.19 | 50.15 | 60.98 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 2188 | 0.0 | 14.28 | 16.31 | 26.90 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 6702 | 1756.9 | 4.46 | 6.48 | 10.85 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 10360 | 2715.9 | 2.70 | 4.44 | 6.30 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 52070 | 0.0 | 0.44 | 0.90 | 1.81 | 0 |
| nx-caching-server (Go) on local disk | PUT | 15945 | 4179.8 | 0.86 | 4.70 | 7.30 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 9765 | 2559.9 | 2.83 | 4.50 | 8.81 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 58645 | 0.0 | 0.42 | 0.66 | 0.87 | 0 |

## Artifact size: 4194304 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 336 | 1408.2 | 85.59 | 209.69 | 244.04 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 575 | 2410.7 | 50.91 | 88.37 | 122.57 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3012 | 0.0 | 7.59 | 18.25 | 59.82 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 329 | 1378.7 | 82.99 | 209.67 | 279.12 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 498 | 2087.5 | 55.94 | 98.81 | 137.70 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 4565 | 0.0 | 6.97 | 14.85 | 22.25 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 178 | 746.5 | 163.54 | 235.02 | 401.00 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 471 | 1974.6 | 65.21 | 82.50 | 122.12 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1822 | 0.0 | 17.23 | 19.57 | 29.56 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 1343 | 5632.2 | 22.39 | 32.32 | 46.26 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 877 | 3677.6 | 34.75 | 54.56 | 70.17 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 56657 | 0.0 | 0.39 | 0.61 | 0.80 | 0 |
| nx-caching-server (Go) on local disk | PUT | 3463 | 14524.7 | 6.51 | 16.47 | 25.10 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 1025 | 4297.2 | 29.27 | 43.36 | 82.02 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 47329 | 0.0 | 0.39 | 0.83 | 2.66 | 0 |

