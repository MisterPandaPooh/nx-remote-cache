# Performance

Measured with the included load generator against every server running in a container on the
same machine, same S3 bucket (S3Mock) or same RAM-backed disk, 32 concurrent connections, 1,000
requests per phase (400 for 4 MB artifacts) after an unmeasured warm-up. Full tables, method and how to rerun:
[`bench/RESULTS.md`](../bench/RESULTS.md).

<p align="center"><img src="../images/benchmark.svg" width="920" alt="Benchmark: requests per second per server, per artifact size"></p>

<!-- BENCH:START -->
**4 KB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 1,247 | 2,347 (10 MB/s) | 10.58 / 22.84 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 1,263 | 139,215 (570 MB/s) | 10.70 / 24.52 ms |
| nx-cache-server (Deno) on S3 | 764 | 1,986 (8 MB/s) | 13.95 / 21.08 ms |
| nx-remote-cache (Rust) on local disk | 50,984 | 129,013 (528 MB/s) | 0.25 / 0.80 ms |
| nx-caching-server (Go) on local disk | 55,563 | 93,779 (384 MB/s) | 0.12 / 1.41 ms |

**256 KB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 1,119 | 2,090 (548 MB/s) | 10.46 / 22.11 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 1,104 | 52,914 (13871 MB/s) | 11.03 / 24.39 ms |
| nx-cache-server (Deno) on S3 | 692 | 1,741 (456 MB/s) | 13.88 / 59.82 ms |
| nx-remote-cache (Rust) on local disk | 22,534 | 43,240 (11335 MB/s) | 0.25 / 0.92 ms |
| nx-caching-server (Go) on local disk | 19,041 | 51,449 (13487 MB/s) | 0.15 / 0.81 ms |

**4 MB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 409 | 997 (4182 MB/s) | 8.08 / 21.94 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 430 | 796 (3338 MB/s) | 8.48 / 20.33 ms |
| nx-cache-server (Deno) on S3 | 165 | 509 (2133 MB/s) | 17.40 / 27.72 ms |
| nx-remote-cache (Rust) on local disk | 2,897 | 4,023 (16875 MB/s) | 0.22 / 0.75 ms |
| nx-caching-server (Go) on local disk | 2,278 | 6,551 (27478 MB/s) | 0.14 / 0.40 ms |
<!-- BENCH:END -->

Reading the tables: on object storage, the arena that matters in production, the Rust server
uploads 1.6 to 2.5× faster than the Deno server, answers misses 25 to 50 % sooner, serves
store-path hits 20 to 95 % faster and hot artifacts from the memory tier 25 to 70× faster. On a
RAM-backed local disk against the Go server, uploads are a tie at 4 KB and 20 to 30 % faster at
256 KB and 4 MB; 4 KB hits are 40 % faster. The one place the Go server wins is large hits from
disk (4 MB: 1.6×): it hands the file to the socket with `sendfile(2)`, which hyper cannot do, so
we read it into memory first. The memory tier covers that case for anything hot.

What makes the difference:

- **Streaming, both ways.** The Rust server never buffers a body above the inline threshold;
  Deno's `nx-cache-server` reads uploads fully before sending them to S3.
- **Memory tier.** Hot artifacts up to 4 MB are served from RAM (`x-cache-tier: memory`),
  without a store round trip. Artifacts are immutable, so the copy can never be stale.
- **Misses are cheap.** A miss is one `HEAD`-sized `GET` on the store with no body, answered
  from a pool of keep-alive connections; on local disk it is a single `open`.
- **Nothing between the socket and the store.** hyper + tokio, no runtime, no GC, ~4 MB image.
- **mimalloc in the static build.** The `scratch` image is a musl binary, and musl's allocator
  serialises on a global lock; swapping it for mimalloc roughly doubled the local-disk numbers.

```bash
cargo build --release --bin nx-cache-bench
./target/release/nx-cache-bench --url https://nx-cache.example.com --token $TOKEN \
  --requests 500 --concurrency 32 --size 262144
```
