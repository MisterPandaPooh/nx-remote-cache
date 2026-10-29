# Benchmark results

Generated 2026-10-29T15:29:50Z on arm64 · Apple M4 Pro · Docker 29.2.0

Every server runs in its own container; `nx-cache-bench` runs in a container on the same Docker network with 32 concurrent connections, 1000 requests per phase (400 from 1 MB up, 4 with 2 connections from 256 MB up), after an unmeasured warm-up of at most 64 uploads.
Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them.

## Artifact size: 4096 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 1250 | 5.1 | 18.79 | 51.71 | 92.46 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2145 | 8.8 | 11.96 | 25.38 | 72.48 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 2912 | 0.0 | 10.23 | 14.84 | 49.34 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 1289 | 5.3 | 20.87 | 48.57 | 84.27 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 158293 | 648.4 | 0.14 | 0.21 | 0.34 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3118 | 0.0 | 11.20 | 14.83 | 24.26 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | PUT | 780 | 3.2 | 36.15 | 65.73 | 90.93 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET hit | 1922 | 7.9 | 16.22 | 20.32 | 38.63 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET miss | 2140 | 0.0 | 14.75 | 16.87 | 23.47 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | PUT | 968 | 4.0 | 30.40 | 57.04 | 92.21 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET hit | 2621 | 10.7 | 12.26 | 19.88 | 34.35 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET miss | 3360 | 0.0 | 9.49 | 11.84 | 19.27 | 0 |
| rerodrigues/cacheiro (Node) on S3 | PUT | 841 | 3.4 | 33.19 | 68.04 | 100.21 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET hit | 1538 | 6.3 | 21.07 | 28.36 | 44.42 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET miss | 2600 | 0.0 | 10.91 | 13.12 | 50.55 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 52222 | 213.9 | 0.43 | 1.10 | 2.14 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 96304 | 394.5 | 0.13 | 0.21 | 0.30 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 102892 | 0.0 | 0.23 | 0.43 | 0.79 | 0 |
| enxtur/nx-caching-server (Go) on local disk | PUT | 52102 | 213.4 | 0.25 | 1.51 | 2.88 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET hit | 75400 | 308.8 | 0.18 | 0.45 | 3.07 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET miss | 139231 | 0.0 | 0.12 | 0.23 | 0.53 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | PUT | 34475 | 141.2 | 0.77 | 1.15 | 1.59 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET hit | 36562 | 149.8 | 0.83 | 1.06 | 1.28 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET miss | 35831 | 0.0 | 0.85 | 1.03 | 1.19 | 0 |
| rerodrigues/cacheiro (Node) on local disk | PUT | 4335 | 17.8 | 6.42 | 8.35 | 34.20 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET hit | 5358 | 21.9 | 5.51 | 8.04 | 9.64 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET miss | 16171 | 0.0 | 1.65 | 3.12 | 5.01 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | PUT | 283 | 1.2 | 110.03 | 169.81 | 176.62 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET hit | 253 | 1.0 | 127.39 | 131.88 | 134.16 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET miss | 430 | 0.0 | 72.69 | 76.20 | 128.07 | 0 |

## Artifact size: 262144 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 1151 | 301.6 | 22.60 | 58.95 | 100.95 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2106 | 552.0 | 10.90 | 33.56 | 64.34 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 3364 | 0.0 | 9.84 | 13.11 | 21.57 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 1136 | 297.8 | 24.27 | 59.11 | 94.21 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 38116 | 9992.0 | 0.47 | 1.47 | 2.85 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 3135 | 0.0 | 10.66 | 13.63 | 23.92 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | PUT | 657 | 172.1 | 43.96 | 72.47 | 98.39 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET hit | 1634 | 428.4 | 18.42 | 25.38 | 44.16 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET miss | 1909 | 0.0 | 15.25 | 19.16 | 66.73 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | PUT | 864 | 226.6 | 34.32 | 69.05 | 97.36 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET hit | 2483 | 650.9 | 13.21 | 21.52 | 34.00 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET miss | 3362 | 0.0 | 9.98 | 11.59 | 19.29 | 0 |
| rerodrigues/cacheiro (Node) on S3 | PUT | 752 | 197.0 | 38.85 | 69.53 | 104.05 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET hit | 1236 | 324.0 | 24.06 | 33.39 | 73.21 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET miss | 2974 | 0.0 | 10.50 | 12.37 | 17.88 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 26613 | 6976.3 | 0.85 | 2.12 | 4.24 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 38980 | 10218.4 | 0.39 | 1.34 | 3.85 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 100789 | 0.0 | 0.21 | 0.43 | 0.65 | 0 |
| enxtur/nx-caching-server (Go) on local disk | PUT | 19204 | 5034.2 | 0.86 | 4.08 | 7.05 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET hit | 60737 | 15921.9 | 0.28 | 0.79 | 1.93 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET miss | 122053 | 0.0 | 0.13 | 0.26 | 1.60 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | PUT | 25311 | 6635.0 | 1.04 | 1.79 | 2.66 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET hit | 28026 | 7346.7 | 1.00 | 1.49 | 2.07 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET miss | 43772 | 0.0 | 0.70 | 0.84 | 0.97 | 0 |
| rerodrigues/cacheiro (Node) on local disk | PUT | 2992 | 784.2 | 10.41 | 12.90 | 16.19 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET hit | 3650 | 956.8 | 8.49 | 12.25 | 14.06 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET miss | 20715 | 0.0 | 1.45 | 1.65 | 3.11 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | PUT | 129 | 33.8 | 237.02 | 299.99 | 317.61 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET hit | 151 | 39.6 | 212.68 | 220.45 | 225.24 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET miss | 251 | 0.0 | 123.70 | 128.45 | 223.20 | 0 |

## Artifact size: 4194304 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 474 | 1986.1 | 63.74 | 126.38 | 195.94 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 955 | 4003.5 | 21.81 | 77.01 | 106.80 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 4181 | 0.0 | 7.03 | 16.32 | 29.56 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 454 | 1904.8 | 64.57 | 136.91 | 213.00 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 845 | 3542.2 | 28.76 | 85.44 | 115.00 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 4200 | 0.0 | 8.75 | 13.72 | 22.39 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | PUT | 197 | 826.1 | 155.09 | 204.21 | 240.03 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET hit | 523 | 2195.4 | 58.00 | 81.63 | 122.49 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET miss | 1927 | 0.0 | 16.69 | 19.51 | 24.27 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | PUT | 399 | 1672.5 | 72.37 | 144.29 | 221.87 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET hit | 1085 | 4550.7 | 25.98 | 41.44 | 76.67 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET miss | 3121 | 0.0 | 6.84 | 14.45 | 61.53 | 0 |
| rerodrigues/cacheiro (Node) on S3 | PUT | 138 | 579.3 | 189.76 | 259.46 | 732.05 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET hit | 239 | 1002.5 | 126.43 | 164.03 | 219.73 | 0 |
| rerodrigues/cacheiro (Node) on S3 | GET miss | 2532 | 0.0 | 11.40 | 17.52 | 20.06 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 2215 | 9291.5 | 10.27 | 28.44 | 50.66 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 3527 | 14792.0 | 8.48 | 12.01 | 14.81 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 86409 | 0.0 | 0.28 | 0.52 | 0.70 | 0 |
| enxtur/nx-caching-server (Go) on local disk | PUT | 2023 | 8484.8 | 10.07 | 36.12 | 76.71 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET hit | 9508 | 39879.6 | 2.30 | 4.74 | 11.23 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET miss | 143612 | 0.0 | 0.13 | 0.21 | 0.38 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | PUT | 1858 | 7792.8 | 16.14 | 22.21 | 28.06 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET hit | 3070 | 12874.9 | 9.66 | 12.48 | 22.62 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET miss | 37943 | 0.0 | 0.78 | 0.99 | 1.18 | 0 |
| rerodrigues/cacheiro (Node) on local disk | PUT | 382 | 1600.6 | 80.44 | 97.29 | 125.45 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET hit | 503 | 2110.3 | 61.88 | 69.70 | 77.96 | 0 |
| rerodrigues/cacheiro (Node) on local disk | GET miss | 12370 | 0.0 | 2.46 | 3.55 | 5.80 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | PUT | 81 | 341.1 | 385.11 | 419.37 | 451.08 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET hit | 113 | 474.8 | 281.72 | 297.62 | 317.16 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET miss | 210 | 0.0 | 144.46 | 157.80 | 253.78 | 0 |

## Artifact size: 1073741824 bytes

| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |
|---|---|---:|---:|---:|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 0 | 267.8 | 8698.15 | 8789.17 | 8789.17 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 2 | 2103.1 | 1354.74 | 1398.53 | 1398.53 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 1330 | 0.0 | 1.87 | 1.90 | 1.90 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 0 | 385.2 | 5489.32 | 5659.35 | 5659.35 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 3 | 3219.0 | 714.75 | 761.43 | 761.43 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 1205 | 0.0 | 2.15 | 2.18 | 2.18 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | PUT | 0 | 515.1 | 4825.49 | 4832.75 | 4832.75 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET hit | 4 | 4050.1 | 579.97 | 584.88 | 584.88 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET miss | 436 | 0.0 | 5.00 | 5.02 | 5.02 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | PUT | 1 | 559.9 | 5300.28 | 5327.29 | 5327.29 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET hit | 3 | 3248.8 | 733.74 | 759.62 | 759.62 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET miss | 828 | 0.0 | 2.82 | 2.96 | 2.96 | 0 |
| rerodrigues/cacheiro (Node) on S3 | PUT | 0 | 0.0 | 0.00 | 0.00 | 0.00 | 4 |
| rerodrigues/cacheiro (Node) on S3 | GET hit | 0 | 0.0 | 0.00 | 0.00 | 0.00 | 4 |
| rerodrigues/cacheiro (Node) on S3 | GET miss | 302 | 0.0 | 7.33 | 9.49 | 9.49 | 0 |
| nx-remote-cache (Rust) on local disk | PUT | 5 | 5217.7 | 416.81 | 418.52 | 418.52 | 0 |
| nx-remote-cache (Rust) on local disk | GET hit | 9 | 9869.1 | 218.09 | 220.80 | 220.80 | 0 |
| nx-remote-cache (Rust) on local disk | GET miss | 9771 | 0.0 | 0.18 | 0.19 | 0.19 | 0 |
| enxtur/nx-caching-server (Go) on local disk | PUT | 11 | 11319.4 | 201.60 | 207.78 | 207.78 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET hit | 28 | 29827.1 | 70.33 | 78.43 | 78.43 | 0 |
| enxtur/nx-caching-server (Go) on local disk | GET miss | 10180 | 0.0 | 0.22 | 0.23 | 0.23 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | PUT | 7 | 7723.5 | 279.98 | 281.96 | 281.96 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET hit | 4 | 4149.9 | 513.23 | 521.63 | 521.63 | 0 |
| ShadiBlitz/nx-cache-server (Rust) on local disk | GET miss | 7665 | 0.0 | 0.28 | 0.30 | 0.30 | 0 |
| rerodrigues/cacheiro (Node) on local disk | PUT | 0 | 0.0 | 0.00 | 0.00 | 0.00 | 4 |
| rerodrigues/cacheiro (Node) on local disk | GET hit | 0 | 0.0 | 0.00 | 0.00 | 0.00 | 4 |
| rerodrigues/cacheiro (Node) on local disk | GET miss | 2943 | 0.0 | 0.53 | 0.85 | 0.85 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | PUT | 2 | 2510.4 | 865.08 | 865.49 | 865.49 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET hit | 2 | 1754.8 | 1232.99 | 1233.31 | 1233.31 | 0 |
| jappyjan/nx-cache-server (Node) on local disk | GET miss | 1413 | 0.0 | 1.47 | 1.84 | 1.84 | 0 |

