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
| nx-remote-cache (Rust) on S3, memory tier off | PUT | 229 | 960.8 | 102.16 | 318.05 | 422.95 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET hit | 925 | 3880.6 | 23.37 | 66.48 | 84.42 | 0 |
| nx-remote-cache (Rust) on S3, memory tier off | GET miss | 4314 | 0.0 | 7.60 | 14.79 | 23.02 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | PUT | 393 | 1649.1 | 77.82 | 147.05 | 273.84 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET hit | 764 | 3203.5 | 28.07 | 112.84 | 146.15 | 0 |
| nx-remote-cache (Rust) on S3, memory tier on | GET miss | 4279 | 0.0 | 8.10 | 12.57 | 21.59 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | PUT | 147 | 618.5 | 158.54 | 220.84 | 807.61 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET hit | 549 | 2304.0 | 55.08 | 75.19 | 105.69 | 0 |
| IKatsuba/nx-cache-server (Deno) on S3 | GET miss | 1975 | 0.0 | 16.12 | 19.56 | 25.79 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | PUT | 391 | 1639.8 | 69.27 | 142.29 | 200.51 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET hit | 1059 | 4440.7 | 27.01 | 46.83 | 74.55 | 0 |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | GET miss | 3095 | 0.0 | 7.17 | 15.87 | 57.80 | 0 |
