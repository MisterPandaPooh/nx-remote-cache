# Benchmark results

Generated 2026-10-28T23:36:11Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs in a container on the same Docker network with 32 concurrent connections, 400 requests per phase.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 939 | 3.8 | 24.78 | 88.72 | 107.26 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2022 | 8.3 | 13.67 | 38.02 | 54.57 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 2519 | 0.0 | 8.33 | 15.56 | 84.86 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 895 | 3.7 | 23.34 | 83.45 | 120.03 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 49469 | 202.6 | 0.41 | 0.75 | 1.23 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3508 | 0.0 | 9.97 | 15.85 | 24.96 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 572 | 2.3 | 43.65 | 102.46 | 133.32 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1828 | 7.5 | 16.25 | 22.01 | 42.63 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1789 | 0.0 | 14.61 | 25.75 | 57.60 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 15935 | 65.3 | 1.66 | 3.32 | 4.92 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 60782 | 249.0 | 0.37 | 0.62 | 1.01 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 53712 | 0.0 | 0.47 | 0.77 | 1.07 | 0 |
| nx-caching-server (Go) on local disk | PUT | 32244 | 132.1 | 0.56 | 1.88 | 3.30 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 49811 | 204.0 | 0.28 | 0.71 | 2.38 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 54451 | 0.0 | 0.36 | 0.61 | 0.91 | 0 |

## Artifact size: 262144 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 751 | 197.0 | 26.85 | 119.47 | 130.27 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 1255 | 329.0 | 14.63 | 58.02 | 98.48 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 4414 | 0.0 | 8.15 | 11.48 | 19.39 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 804 | 210.7 | 27.53 | 111.55 | 122.19 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 10487 | 2749.0 | 2.35 | 5.00 | 7.45 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3906 | 0.0 | 9.98 | 13.62 | 24.45 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 551 | 144.5 | 44.90 | 91.41 | 131.12 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1570 | 411.6 | 18.93 | 26.52 | 51.09 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1842 | 0.0 | 14.28 | 23.72 | 59.71 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 5371 | 1407.9 | 5.36 | 9.01 | 13.42 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 9556 | 2505.0 | 2.79 | 5.42 | 8.62 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 53962 | 0.0 | 0.45 | 0.77 | 1.08 | 0 |
| nx-caching-server (Go) on local disk | PUT | 16864 | 4420.8 | 0.89 | 3.84 | 7.18 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 9901 | 2595.5 | 2.56 | 4.83 | 18.38 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 43074 | 0.0 | 0.44 | 0.84 | 2.96 | 0 |

## Artifact size: 4194304 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 340 | 1427.0 | 82.20 | 202.56 | 252.91 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 537 | 2251.3 | 56.09 | 90.61 | 133.56 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3136 | 0.0 | 7.61 | 12.89 | 50.61 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 332 | 1393.3 | 89.55 | 189.94 | 254.98 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 553 | 2317.9 | 54.23 | 85.02 | 118.20 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 4299 | 0.0 | 8.19 | 15.24 | 22.78 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 185 | 777.0 | 158.29 | 225.46 | 378.70 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 465 | 1948.6 | 63.92 | 90.85 | 128.36 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1880 | 0.0 | 16.74 | 18.99 | 26.46 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 1408 | 5904.7 | 20.09 | 33.59 | 65.03 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 811 | 3401.4 | 38.53 | 54.26 | 69.03 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 54420 | 0.0 | 0.42 | 0.65 | 1.13 | 0 |
| nx-caching-server (Go) on local disk | PUT | 3227 | 13536.3 | 6.75 | 15.80 | 34.38 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 1042 | 4370.9 | 26.95 | 44.84 | 80.00 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 44251 | 0.0 | 0.45 | 0.89 | 2.21 | 0 |

