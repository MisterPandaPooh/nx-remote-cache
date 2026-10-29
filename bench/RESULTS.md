# Benchmark results

Generated 2026-10-29T08:34:38Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs in a container on the same Docker network with 32 concurrent connections, 1000 requests per phase (400 from 1 MB up), after an unmeasured warm-up of 64 uploads.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 1219 | 5.0 | 24.86 | 48.83 | 83.46 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2520 | 10.3 | 11.22 | 22.72 | 54.97 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3090 | 0.0 | 9.86 | 16.06 | 26.13 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 1293 | 5.3 | 21.06 | 46.15 | 93.29 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 135822 | 556.3 | 0.15 | 0.26 | 0.53 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3121 | 0.0 | 10.61 | 14.27 | 23.02 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | PUT | 737 | 3.0 | 37.60 | 75.75 | 95.10 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET hit | 1959 | 8.0 | 15.58 | 19.62 | 46.27 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET miss | 1715 | 0.0 | 15.00 | 21.05 | 107.44 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | PUT | 933 | 3.8 | 31.03 | 60.78 | 95.55 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET hit | 2631 | 10.8 | 12.54 | 21.65 | 32.08 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET miss | 3283 | 0.0 | 9.77 | 12.34 | 20.47 | 0 |
| rerodrigues/cacheiro (Node) on S3 | PUT | 748 | 3.1 | 36.48 | 77.29 | 109.43 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET hit | 1074 | 4.4 | 25.24 | 42.97 | 104.65 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET miss | 2254 | 0.0 | 13.37 | 18.28 | 22.10 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 54137 | 221.7 | 0.48 | 0.95 | 1.49 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 133935 | 548.6 | 0.18 | 0.28 | 0.42 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 85695 | 0.0 | 0.28 | 0.51 | 1.03 | 0 |
| enxtur/nx-caching-server (Go) on local disk | PUT | 48817 | 200.0 | 0.31 | 1.81 | 2.98 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET hit | 71204 | 291.6 | 0.19 | 0.65 | 3.12 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET miss | 81822 | 0.0 | 0.22 | 0.44 | 1.41 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | PUT | 40568 | 166.2 | 0.73 | 1.07 | 1.42 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET hit | 34705 | 142.2 | 0.81 | 1.10 | 1.47 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET miss | 38607 | 0.0 | 0.80 | 0.95 | 1.13 | 0 |
| rerodrigues/cacheiro (Node) on local disk | PUT | 4502 | 18.4 | 6.09 | 8.88 | 20.82 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET hit | 4929 | 20.2 | 5.88 | 9.05 | 12.11 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET miss | 13544 | 0.0 | 1.96 | 4.10 | 5.66 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | PUT | 233 | 1.0 | 138.54 | 195.33 | 249.25 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET hit | 236 | 1.0 | 136.38 | 141.43 | 152.43 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET miss | 397 | 0.0 | 78.02 | 84.09 | 136.64 | 0 |

## Artifact size: 262144 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 1109 | 290.7 | 23.64 | 60.21 | 105.07 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2016 | 528.4 | 10.81 | 33.02 | 63.12 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3368 | 0.0 | 9.91 | 11.76 | 19.73 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 1050 | 275.2 | 23.30 | 62.30 | 107.63 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 45456 | 11916.1 | 0.38 | 0.72 | 4.24 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3070 | 0.0 | 11.33 | 14.80 | 24.76 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | PUT | 588 | 154.2 | 49.56 | 75.04 | 116.02 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET hit | 1462 | 383.3 | 20.58 | 28.75 | 58.03 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET miss | 1859 | 0.0 | 15.70 | 18.40 | 61.37 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | PUT | 872 | 228.6 | 34.91 | 65.01 | 100.24 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET hit | 2421 | 634.6 | 13.88 | 21.09 | 35.33 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET miss | 3382 | 0.0 | 9.49 | 12.06 | 20.85 | 0 |
| rerodrigues/cacheiro (Node) on S3 | PUT | 628 | 164.6 | 46.18 | 84.44 | 117.04 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET hit | 997 | 261.2 | 29.83 | 42.59 | 74.64 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET miss | 2508 | 0.0 | 12.29 | 14.90 | 20.02 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 22868 | 5994.8 | 1.01 | 2.06 | 4.44 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 41571 | 10897.6 | 0.37 | 0.98 | 3.34 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 85274 | 0.0 | 0.28 | 0.54 | 0.96 | 0 |
| enxtur/nx-caching-server (Go) on local disk | PUT | 19738 | 5174.3 | 1.03 | 3.68 | 5.22 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET hit | 34669 | 9088.2 | 0.37 | 1.89 | 5.94 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET miss | 123973 | 0.0 | 0.14 | 0.25 | 1.48 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | PUT | 20800 | 5452.7 | 1.33 | 2.21 | 3.24 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET hit | 23206 | 6083.3 | 1.10 | 1.85 | 4.34 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET miss | 40674 | 0.0 | 0.76 | 0.90 | 1.08 | 0 |
| rerodrigues/cacheiro (Node) on local disk | PUT | 3225 | 845.5 | 9.42 | 12.59 | 15.81 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET hit | 4012 | 1051.6 | 7.62 | 10.40 | 11.79 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET miss | 22805 | 0.0 | 1.28 | 1.59 | 2.69 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | PUT | 119 | 31.3 | 255.74 | 322.93 | 340.71 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET hit | 141 | 37.1 | 226.15 | 235.51 | 247.94 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET miss | 238 | 0.0 | 131.10 | 134.16 | 234.89 | 0 |

## Artifact size: 4194304 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 311 | 1303.5 | 92.80 | 191.58 | 281.78 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 851 | 3569.6 | 27.33 | 83.51 | 103.63 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 4025 | 0.0 | 8.68 | 14.50 | 20.02 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 383 | 1605.2 | 82.84 | 158.60 | 227.43 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 618 | 2592.8 | 38.00 | 112.89 | 179.13 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 2238 | 0.0 | 11.44 | 16.02 | 71.09 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | PUT | 166 | 694.9 | 164.32 | 244.37 | 502.63 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET hit | 463 | 1942.1 | 65.60 | 89.94 | 121.90 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET miss | 1712 | 0.0 | 17.64 | 22.21 | 40.64 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | PUT | 408 | 1712.5 | 68.55 | 135.94 | 204.54 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET hit | 834 | 3498.0 | 34.71 | 55.74 | 91.76 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET miss | 4374 | 0.0 | 7.60 | 11.82 | 21.22 | 0 |
| rerodrigues/cacheiro (Node) on S3 | PUT | 148 | 621.0 | 194.81 | 266.66 | 454.15 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET hit | 224 | 940.0 | 132.15 | 183.28 | 270.00 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET miss | 2472 | 0.0 | 12.08 | 15.92 | 17.98 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 1439 | 6035.3 | 13.81 | 49.71 | 98.24 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 913 | 3829.5 | 12.52 | 108.41 | 218.68 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 79504 | 0.0 | 0.28 | 0.56 | 1.05 | 0 |
| enxtur/nx-caching-server (Go) on local disk | PUT | 1610 | 6751.8 | 12.02 | 50.51 | 84.92 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET hit | 3217 | 13493.3 | 6.17 | 23.13 | 39.97 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET miss | 109827 | 0.0 | 0.18 | 0.34 | 0.57 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | PUT | 942 | 3950.6 | 29.29 | 53.00 | 79.83 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET hit | 949 | 3980.0 | 15.27 | 42.65 | 166.56 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET miss | 35722 | 0.0 | 0.82 | 0.98 | 1.35 | 0 |
| rerodrigues/cacheiro (Node) on local disk | PUT | 262 | 1098.7 | 99.91 | 206.64 | 270.66 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET hit | 457 | 1917.9 | 62.38 | 93.82 | 114.98 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET miss | 14843 | 0.0 | 1.88 | 2.68 | 4.19 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | PUT | 78 | 325.9 | 410.43 | 430.98 | 461.21 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET hit | 111 | 464.5 | 289.71 | 308.37 | 319.84 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET miss | 203 | 0.0 | 150.23 | 157.25 | 269.15 | 0 |

