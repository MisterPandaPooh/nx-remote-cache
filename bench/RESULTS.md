# Benchmark results

Generated 2026-10-28T23:39:36Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs in a container on the same Docker network with 32 concurrent connections, 400 requests per phase.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 876 | 3.6 | 23.54 | 92.01 | 126.83 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 1612 | 6.6 | 14.53 | 54.24 | 74.02 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 4086 | 0.0 | 8.92 | 13.20 | 22.81 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 884 | 3.6 | 22.74 | 93.52 | 124.38 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 44480 | 182.2 | 0.45 | 0.76 | 1.18 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3549 | 0.0 | 10.18 | 15.02 | 27.92 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 545 | 2.2 | 45.05 | 107.40 | 136.74 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1700 | 7.0 | 16.42 | 26.75 | 56.68 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1811 | 0.0 | 15.20 | 21.49 | 53.01 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 27148 | 111.2 | 0.77 | 1.86 | 4.87 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 51580 | 211.3 | 0.39 | 0.68 | 1.82 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 58045 | 0.0 | 0.38 | 0.72 | 1.17 | 0 |
| nx-caching-server (Go) on local disk | PUT | 29302 | 120.0 | 0.69 | 2.21 | 3.48 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 42961 | 176.0 | 0.32 | 0.80 | 2.25 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 57787 | 0.0 | 0.32 | 0.55 | 0.80 | 0 |

## Artifact size: 262144 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 785 | 205.8 | 25.92 | 106.04 | 125.06 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 1206 | 316.1 | 15.17 | 66.86 | 74.55 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 4797 | 0.0 | 7.10 | 11.37 | 18.95 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 768 | 201.4 | 26.43 | 102.97 | 135.54 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 10338 | 2710.0 | 2.41 | 4.94 | 8.48 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 2459 | 0.0 | 10.88 | 25.40 | 60.64 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 544 | 142.5 | 46.58 | 98.25 | 161.44 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1418 | 371.6 | 17.74 | 47.16 | 59.78 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 2158 | 0.0 | 14.59 | 16.93 | 21.31 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 6859 | 1797.9 | 4.26 | 6.46 | 9.78 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 10012 | 2624.5 | 2.63 | 4.75 | 9.84 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 55708 | 0.0 | 0.45 | 0.68 | 0.89 | 0 |
| nx-caching-server (Go) on local disk | PUT | 13512 | 3542.2 | 1.05 | 5.51 | 8.50 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 9511 | 2493.2 | 2.72 | 5.08 | 12.67 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 58571 | 0.0 | 0.43 | 0.63 | 0.83 | 0 |

## Artifact size: 4194304 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 338 | 1417.3 | 83.21 | 178.17 | 242.39 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 538 | 2257.1 | 54.40 | 92.88 | 145.60 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 4424 | 0.0 | 7.85 | 13.45 | 22.21 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 346 | 1449.8 | 87.58 | 175.18 | 238.26 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 555 | 2329.2 | 52.08 | 86.71 | 141.03 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3024 | 0.0 | 7.29 | 16.50 | 59.42 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 179 | 750.5 | 164.83 | 239.81 | 338.68 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 477 | 2000.1 | 64.17 | 83.30 | 120.69 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1870 | 0.0 | 16.73 | 19.33 | 28.68 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 884 | 3707.9 | 33.60 | 52.02 | 71.31 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 872 | 3655.7 | 34.11 | 52.28 | 78.34 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 52251 | 0.0 | 0.43 | 0.69 | 0.91 | 0 |
| nx-caching-server (Go) on local disk | PUT | 2724 | 11426.3 | 6.27 | 24.78 | 42.30 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 1017 | 4266.8 | 28.30 | 43.40 | 87.00 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 44392 | 0.0 | 0.39 | 0.86 | 3.58 | 0 |

