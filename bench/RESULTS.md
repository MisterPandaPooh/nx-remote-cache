# Benchmark results

Generated 2026-10-29T02:35:00Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs in a container on the same Docker network with 32 concurrent connections, 1000 requests per phase (400 from 1 MB up), after an unmeasured warm-up of 64 uploads.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 1247 | 5.1 | 21.44 | 51.71 | 88.94 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2347 | 9.6 | 11.39 | 26.36 | 52.30 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3271 | 0.0 | 10.58 | 13.33 | 22.84 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 1263 | 5.2 | 22.02 | 48.17 | 91.97 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 139215 | 570.2 | 0.16 | 0.25 | 0.32 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3141 | 0.0 | 10.70 | 14.10 | 24.52 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 764 | 3.1 | 36.01 | 70.01 | 102.69 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1986 | 8.1 | 15.50 | 20.36 | 32.34 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 2244 | 0.0 | 13.95 | 16.98 | 21.08 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 50984 | 208.8 | 0.49 | 1.15 | 1.94 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 129013 | 528.4 | 0.17 | 0.29 | 0.40 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 93155 | 0.0 | 0.25 | 0.52 | 0.80 | 0 |
| nx-caching-server (Go) on local disk | PUT | 55563 | 227.6 | 0.28 | 1.20 | 3.05 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 93779 | 384.1 | 0.18 | 0.43 | 1.55 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 133899 | 0.0 | 0.12 | 0.27 | 1.41 | 0 |

## Artifact size: 262144 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 1119 | 293.4 | 23.26 | 58.25 | 100.02 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2090 | 548.0 | 11.77 | 29.38 | 65.13 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3218 | 0.0 | 10.46 | 12.95 | 22.11 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 1104 | 289.3 | 23.51 | 60.52 | 92.33 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 52914 | 13871.0 | 0.36 | 0.68 | 2.54 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3097 | 0.0 | 11.03 | 14.19 | 24.39 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 692 | 181.3 | 41.26 | 74.19 | 102.22 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 1741 | 456.4 | 17.19 | 23.36 | 38.43 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 2081 | 0.0 | 13.88 | 15.78 | 59.82 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 22534 | 5907.1 | 1.04 | 2.65 | 4.65 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 43240 | 11335.0 | 0.40 | 0.96 | 4.17 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 92291 | 0.0 | 0.25 | 0.49 | 0.92 | 0 |
| nx-caching-server (Go) on local disk | PUT | 19041 | 4991.5 | 0.87 | 3.70 | 7.10 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 51449 | 13487.1 | 0.38 | 1.02 | 2.02 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 128083 | 0.0 | 0.15 | 0.28 | 0.81 | 0 |

## Artifact size: 4194304 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 409 | 1716.9 | 73.24 | 148.44 | 209.44 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 997 | 4181.9 | 24.06 | 66.42 | 84.04 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 4194 | 0.0 | 8.08 | 12.97 | 21.94 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 430 | 1802.7 | 61.83 | 135.37 | 202.02 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 796 | 3338.5 | 26.70 | 92.70 | 144.81 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 4201 | 0.0 | 8.48 | 13.38 | 20.33 | 0 |
| nx-cache-server (Deno) on S3 | PUT | 165 | 694.1 | 158.90 | 203.49 | 244.90 | 0 |
| nx-cache-server (Deno) on S3 | GET hit | 509 | 2133.1 | 60.26 | 77.59 | 108.77 | 0 |
| nx-cache-server (Deno) on S3 | GET miss | 1799 | 0.0 | 17.40 | 19.22 | 27.72 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 2897 | 12151.3 | 8.84 | 17.02 | 26.45 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 4023 | 16874.9 | 6.78 | 11.30 | 15.62 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 89857 | 0.0 | 0.22 | 0.42 | 0.75 | 0 |
| nx-caching-server (Go) on local disk | PUT | 2278 | 9554.4 | 8.15 | 25.09 | 88.62 | 0 |
| nx-caching-server (Go) on local disk | GET hit | 6551 | 27478.5 | 3.37 | 7.58 | 27.54 | 0 |
| nx-caching-server (Go) on local disk | GET miss | 137868 | 0.0 | 0.14 | 0.28 | 0.40 | 0 |

