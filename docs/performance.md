# Performance

Every open-source self-hosted Nx cache server with a published image, each on every store it
supports, measured with the included load generator. Every server runs in a container on the
same machine; the load generator runs in a container on the same Docker network, 32 concurrent
connections, 1,000 requests per phase (400 for 4 MB artifacts; 4 requests on 2 connections for
1 GB artifacts, where the figure that matters is MB/s) after an unmeasured warm-up.
Two arenas: the same S3 bucket (S3Mock, path-style; servers whose SDK insists on
virtual-hosted-style addressing go through a one-hop nginx rewrite) and the same RAM-backed
local disk. Full tables, method and how to rerun: [`bench/RESULTS.md`](../bench/RESULTS.md).

| Server | Language | Stores | Benchmarked on |
|---|---|---|---|
| nx-remote-cache (this project) | Rust | Azure Blob, S3, GCS, disk, memory | S3 (memory tier on and off), disk |
| [the-inconvenience-store/nx-remote-cache](https://github.com/the-inconvenience-store/nx-remote-cache) | Go | S3 | S3 |
| [IKatsuba/nx-cache-server](https://github.com/IKatsuba/nx-cache-server) | Deno | S3 | S3 |
| [rerodrigues/cacheiro](https://github.com/rerodrigues/nx-remote-cache-server) | Node | disk, S3, GCS, Azure | S3, disk |
| [enxtur/nx-caching-server](https://github.com/enxtur/nx-caching-server) | Go | disk | disk |
| [ShadiBlitz/nx-cache-server](https://github.com/ShadiBlitz/nx-cache-server) | Rust | disk, S3, GCS, Azure | disk only: every S3 upload failed inside its AWS SDK (streaming body without a length), through the rewrite and directly |
| [jappyjan/nx-cache-server](https://github.com/jappyjan/nx-cache-server) | Node | disk | disk (image built from its Dockerfile) |
| [slaweekq/nx-cache-s3](https://github.com/devslaweekq/nx-cache-s3-remote) | Node | S3 | not run: amd64-only image, would be emulated on this arm64 machine |
| [nxcite/nx-cache-server](https://github.com/nxcite/nx-cache-server) | Rust | S3 | not run: no published image |

<p align="center"><img src="../images/benchmark.svg" width="920" alt="Benchmark: requests per second per server, per artifact size"></p>

**Reading the chart.** One panel per arena, artifact size and operation; inside a panel every bar starts at 0 on one linear scale, so a bar twice as long is twice the throughput, and the number after each bar is the absolute requests per second. `this project` is nx-remote-cache. In the object-storage arena it has two rows: *hits from S3* is the same path every other server takes; *hits from RAM (memory tier)* is hot artifacts up to 4 MB served from memory (`NX_CACHE_MEMORY_CACHE_BYTES`, 256 MB by default) with no store round trip, which only this project offers. Uploads go through the same path in both cases.

<!-- BENCH:START -->
**4 KB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 1,250 | 2,145 (9 MB/s) | 10.23 / 49.34 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 1,289 | 158,293 (648 MB/s) | 11.20 / 24.26 ms |
| IKatsuba/nx-cache-server (Deno) on S3 | 780 | 1,922 (8 MB/s) | 14.75 / 23.47 ms |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | 968 | 2,621 (11 MB/s) | 9.49 / 19.27 ms |
| rerodrigues/cacheiro (Node) on S3 | 841 | 1,538 (6 MB/s) | 10.91 / 50.55 ms |
| nx-remote-cache (Rust) on local disk | 52,222 | 96,304 (394 MB/s) | 0.23 / 0.79 ms |
| enxtur/nx-caching-server (Go) on local disk | 52,102 | 75,400 (309 MB/s) | 0.12 / 0.53 ms |
| ShadiBlitz/nx-cache-server (Rust) on local disk | 34,475 | 36,562 (150 MB/s) | 0.85 / 1.19 ms |
| rerodrigues/cacheiro (Node) on local disk | 4,335 | 5,358 (22 MB/s) | 1.65 / 5.01 ms |
| jappyjan/nx-cache-server (Node) on local disk | 283 | 253 (1 MB/s) | 72.69 / 128.07 ms |

**256 KB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 1,151 | 2,106 (552 MB/s) | 9.84 / 21.57 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 1,136 | 38,116 (9992 MB/s) | 10.66 / 23.92 ms |
| IKatsuba/nx-cache-server (Deno) on S3 | 657 | 1,634 (428 MB/s) | 15.25 / 66.73 ms |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | 864 | 2,483 (651 MB/s) | 9.98 / 19.29 ms |
| rerodrigues/cacheiro (Node) on S3 | 752 | 1,236 (324 MB/s) | 10.50 / 17.88 ms |
| nx-remote-cache (Rust) on local disk | 26,613 | 38,980 (10218 MB/s) | 0.21 / 0.65 ms |
| enxtur/nx-caching-server (Go) on local disk | 19,204 | 60,737 (15922 MB/s) | 0.13 / 1.60 ms |
| ShadiBlitz/nx-cache-server (Rust) on local disk | 25,311 | 28,026 (7347 MB/s) | 0.70 / 0.97 ms |
| rerodrigues/cacheiro (Node) on local disk | 2,992 | 3,650 (957 MB/s) | 1.45 / 3.11 ms |
| jappyjan/nx-cache-server (Node) on local disk | 129 | 151 (40 MB/s) | 123.70 / 223.20 ms |

**4 MB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 474 | 955 (4004 MB/s) | 7.03 / 29.56 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 454 | 845 (3542 MB/s) | 8.75 / 22.39 ms |
| IKatsuba/nx-cache-server (Deno) on S3 | 197 | 523 (2195 MB/s) | 16.69 / 24.27 ms |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | 399 | 1,085 (4551 MB/s) | 6.84 / 61.53 ms |
| rerodrigues/cacheiro (Node) on S3 | 138 | 239 (1002 MB/s) | 11.40 / 20.06 ms |
| nx-remote-cache (Rust) on local disk | 2,215 | 3,527 (14792 MB/s) | 0.28 / 0.70 ms |
| enxtur/nx-caching-server (Go) on local disk | 2,023 | 9,508 (39880 MB/s) | 0.13 / 0.38 ms |
| ShadiBlitz/nx-cache-server (Rust) on local disk | 1,858 | 3,070 (12875 MB/s) | 0.78 / 1.18 ms |
| rerodrigues/cacheiro (Node) on local disk | 382 | 503 (2110 MB/s) | 2.46 / 5.80 ms |
| jappyjan/nx-cache-server (Node) on local disk | 81 | 113 (475 MB/s) | 144.46 / 253.78 ms |

**1 GB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 0 | 2 (2103 MB/s) | 1.87 / 1.90 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 0 | 3 (3219 MB/s) | 2.15 / 2.18 ms |
| IKatsuba/nx-cache-server (Deno) on S3 | 0 | 4 (4050 MB/s) | 5.00 / 5.02 ms |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | 1 | 3 (3249 MB/s) | 2.82 / 2.96 ms |
| rerodrigues/cacheiro (Node) on S3 | 0 | 0 (0 MB/s) | 7.33 / 9.49 ms |
| nx-remote-cache (Rust) on local disk | 5 | 9 (9869 MB/s) | 0.18 / 0.19 ms |
| enxtur/nx-caching-server (Go) on local disk | 11 | 28 (29827 MB/s) | 0.22 / 0.23 ms |
| ShadiBlitz/nx-cache-server (Rust) on local disk | 7 | 4 (4150 MB/s) | 0.28 / 0.30 ms |
| rerodrigues/cacheiro (Node) on local disk | 0 | 0 (0 MB/s) | 0.53 / 0.85 ms |
| jappyjan/nx-cache-server (Node) on local disk | 2 | 2 (1755 MB/s) | 1.47 / 1.84 ms |
<!-- BENCH:END -->

Reading the tables, honestly:

**Object storage.** Uploads: nx-remote-cache is first at every size (1.2 to 1.3× the Go server
`the-inconvenience-store`, 1.5 to 2.4× Deno, 1.5 to 3.4× cacheiro). Store-path hits: the Go server
is ahead at every size, by 12 to 20 % (4 KB, 256 KB, 4 MB); Deno is 10 to 50 % behind, cacheiro 30
to 80 % behind. Misses: nx-remote-cache and the Go server are within about 10 % of each other
(both around 7 to 10 ms, bounded by S3Mock), Deno and cacheiro 25 to 60 % slower. With the memory
tier, hot hits leave the store out entirely: 20 to 70× the fastest store path at 4 KB and 256 KB.

The 4 MB upload used to be the Go server's clearest win (about 25 %). Until 0.1.0 nx-remote-cache
buffered every body up to the inline threshold, then issued one atomic `PUT`; the Go SDK streams a
known-length body straight to the store. Bodies between 1 MB and 64 MB now go through a presigned
`PUT` as they arrive (`NX_CACHE_STREAM_PUT_THRESHOLD`, see [configuration](configuration.md)),
which closed the gap: the table shows the last pass, and over five passes at 4 MB the two servers
trade places (229 to 474 against 307 to 399 req/s), so read that tier as a tie with about ±30 %
of run-to-run noise on S3Mock. The remaining store-path hit gap is on the way down: nx-remote-cache
has the lower median at 4 KB and 256 KB but a heavier p90 / p99 tail, which is not profiled yet
and is on the list. The memory tier at 4 MB is slower than the plain store path: 400 artifacts of
4 MB cycle through a 256 MB budget, so every read fills and evicts for nothing; the tier should
skip fills when the working set exceeds its budget.

**1 GB artifacts.** Memory is the first thing a 1 GB artifact tests: the Node server `cacheiro`
buffers uploads and failed every one of them on both stores, and the first two attempts at this
tier ran the Docker VM out of memory until the load generator streamed its downloads. This tier is
4 requests on 2 connections, so treat anything within 30 % as a tie. On S3Mock, uploads reach 270
to 390 MB/s here against 560 MB/s for the Go server and 515 MB/s for Deno: both send one
`PutObject`, while 1 GB is above the presigned-`PUT` threshold (64 MB) and goes as 8 MB multipart
parts that S3Mock then copies into the final object on completion, a cost real S3 does not bill
the same way. Downloads: 2.1 to 3.2 GB/s here, 3.2 GB/s for the Go server, 4.1 GB/s for Deno,
all near the bench client's own limit. On local disk `enxtur` wins both ways at this size (11 300
against 5 200 MB/s up, 29 800 against 9 900 MB/s down, `sendfile` again); ShadiBlitz is 1.5×
ahead on uploads and nx-remote-cache 2.4× ahead on downloads.

**Local disk.** nx-remote-cache is first on uploads at every size (level with the Go server
`enxtur` at 4 KB, 1.1 to 1.4× ahead at 256 KB and 4 MB, 1.05 to 1.5× ShadiBlitz, 6 to 12×
cacheiro) and on small hits (1.3× the Go server, 2.6× ShadiBlitz at 4 KB). Larger hits from disk
are the one clear loss: `enxtur` serves 1.6× more 256 KB hits and 2.7× more 4 MB hits per second.
It hands the file to the socket with `sendfile(2)`; hyper cannot, so nx-remote-cache reads the
file into user space in chunks. Larger read chunks for big files would close part of that gap;
the memory tier covers it for anything hot. Its misses also peak higher (a miss is one `stat`
there, an `open` plus the middleware stack here). `jappyjan`'s stdlib server is 10 to 600× slower
depending on size: it is synchronous and single-threaded by design.

What makes nx-remote-cache fast:

- **Streaming, both ways.** Bodies above 1 MB stream to the store as they arrive: one presigned
  `PUT` up to 64 MB, multipart parts above; downloads stream out of the store. Deno's
  `nx-cache-server` and `cacheiro` read uploads fully before sending them to S3.
- **Memory tier.** Hot artifacts up to 4 MB are served from RAM (`x-cache-tier: memory`),
  without a store round trip. Artifacts are immutable, so the copy can never be stale.
- **Misses are cheap.** A miss is one `GET` on the store with no body, answered from a pool of
  keep-alive connections; on local disk it is a single `open`.
- **Nothing between the socket and the store.** hyper + tokio, no runtime, no GC, ~4 MB image.
- **mimalloc in the static build.** The `scratch` image is a musl binary, and musl's allocator
  serialises on a global lock; swapping it for mimalloc roughly doubled the local-disk numbers.

```bash
cargo build --release --bin nx-cache-bench
./target/release/nx-cache-bench --url https://nx-cache.example.com --token $TOKEN \
  --requests 500 --concurrency 32 --size 262144
```
