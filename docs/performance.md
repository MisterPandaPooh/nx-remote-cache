# Performance

Every open-source self-hosted Nx cache server with a published image, each on every store it
supports, measured with the included load generator. Every server runs in a container on the
same machine; the load generator runs in a container on the same Docker network, 32 concurrent
connections, 1,000 requests per phase (400 for 4 MB artifacts) after an unmeasured warm-up.
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
| nx-remote-cache (Rust) on S3, memory tier off | 1,219 | 2,520 (10 MB/s) | 9.86 / 26.13 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 1,293 | 135,822 (556 MB/s) | 10.61 / 23.02 ms |
| IKatsuba/nx-cache-server (Deno) on S3 | 737 | 1,959 (8 MB/s) | 15.00 / 107.44 ms |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | 933 | 2,631 (11 MB/s) | 9.77 / 20.47 ms |
| rerodrigues/cacheiro (Node) on S3 | 748 | 1,074 (4 MB/s) | 13.37 / 22.10 ms |
| nx-remote-cache (Rust) on local disk | 54,137 | 133,935 (549 MB/s) | 0.28 / 1.03 ms |
| enxtur/nx-caching-server (Go) on local disk | 48,817 | 71,204 (292 MB/s) | 0.22 / 1.41 ms |
| ShadiBlitz/nx-cache-server (Rust) on local disk | 40,568 | 34,705 (142 MB/s) | 0.80 / 1.13 ms |
| rerodrigues/cacheiro (Node) on local disk | 4,502 | 4,929 (20 MB/s) | 1.96 / 5.66 ms |
| jappyjan/nx-cache-server (Node) on local disk | 233 | 236 (1 MB/s) | 78.02 / 136.64 ms |

**256 KB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 1,109 | 2,016 (528 MB/s) | 9.91 / 19.73 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 1,050 | 45,456 (11916 MB/s) | 11.33 / 24.76 ms |
| IKatsuba/nx-cache-server (Deno) on S3 | 588 | 1,462 (383 MB/s) | 15.70 / 61.37 ms |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | 872 | 2,421 (635 MB/s) | 9.49 / 20.85 ms |
| rerodrigues/cacheiro (Node) on S3 | 628 | 997 (261 MB/s) | 12.29 / 20.02 ms |
| nx-remote-cache (Rust) on local disk | 22,868 | 41,571 (10898 MB/s) | 0.28 / 0.96 ms |
| enxtur/nx-caching-server (Go) on local disk | 19,738 | 34,669 (9088 MB/s) | 0.14 / 1.48 ms |
| ShadiBlitz/nx-cache-server (Rust) on local disk | 20,800 | 23,206 (6083 MB/s) | 0.76 / 1.08 ms |
| rerodrigues/cacheiro (Node) on local disk | 3,225 | 4,012 (1052 MB/s) | 1.28 / 2.69 ms |
| jappyjan/nx-cache-server (Node) on local disk | 119 | 141 (37 MB/s) | 131.10 / 234.89 ms |

**4 MB artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99

| Server | PUT | GET hit | GET miss |
|---|---:|---:|---:|
| nx-remote-cache (Rust) on S3, memory tier off | 311 | 851 (3570 MB/s) | 8.68 / 20.02 ms |
| nx-remote-cache (Rust) on S3, memory tier on | 383 | 618 (2593 MB/s) | 11.44 / 71.09 ms |
| IKatsuba/nx-cache-server (Deno) on S3 | 166 | 463 (1942 MB/s) | 17.64 / 40.64 ms |
| the-inconvenience-store/nx-remote-cache (Go) on S3 | 408 | 834 (3498 MB/s) | 7.60 / 21.22 ms |
| rerodrigues/cacheiro (Node) on S3 | 148 | 224 (940 MB/s) | 12.08 / 17.98 ms |
| nx-remote-cache (Rust) on local disk | 1,439 | 913 (3830 MB/s) | 0.28 / 1.05 ms |
| enxtur/nx-caching-server (Go) on local disk | 1,610 | 3,217 (13493 MB/s) | 0.18 / 0.57 ms |
| ShadiBlitz/nx-cache-server (Rust) on local disk | 942 | 949 (3980 MB/s) | 0.82 / 1.35 ms |
| rerodrigues/cacheiro (Node) on local disk | 262 | 457 (1918 MB/s) | 1.88 / 4.19 ms |
| jappyjan/nx-cache-server (Node) on local disk | 78 | 111 (464 MB/s) | 150.23 / 269.15 ms |
<!-- BENCH:END -->

Reading the tables, honestly:

**Object storage.** Uploads: nx-remote-cache is first at 4 KB and 256 KB (1.3× the Go server,
1.7 to 1.9× Deno and cacheiro); at 4 MB the Go server (`the-inconvenience-store`) uploads about 25 %
faster. Store-path hits: a tie with the Go server at 4 KB and 4 MB, the Go server about 20 %
ahead at 256 KB; Deno is 25 to 45 % behind, cacheiro 2 to 4× behind. Misses: nx-remote-cache and
the Go server are within about 10 % of each other (both around 10 ms, bounded by S3Mock), Deno and
cacheiro 25 to 50 % slower. With the memory tier, hot hits leave the store out entirely: 20 to
50× the fastest store path at 4 KB and 256 KB.

Where the Go server is ahead and why: for bodies up to the inline threshold (8 MB) nx-remote-cache
buffers the whole upload, then issues one atomic `PUT`; the Go SDK streams a known-length body
straight to the store, so a 4 MB upload starts leaving the process while it is still arriving.
The 256 KB hit gap looks like the same shape on the way down (one extra copy per response) and
is within run-to-run variance for the other sizes. Both are fixable in the store adapter and are
on the list. The memory tier at 4 MB is slower than the plain store path: 400 artifacts of 4 MB
cycle through a 256 MB budget, so every read fills and evicts for nothing; the tier should skip
fills when the working set exceeds its budget.

**Local disk.** nx-remote-cache is first on uploads at 4 KB and 256 KB (1.1 to 1.3× the Go and
Rust competitors, 7 to 12× cacheiro) and on small and medium hits (1.2 to 1.9× the Go server,
1.8 to 3.9× ShadiBlitz). Large hits from disk are the one clear loss: at 4 MB `enxtur` serves
3.5× more hits per second. It hands the file to the socket with `sendfile(2)`; hyper cannot, so
nx-remote-cache reads the file into user space in chunks. Larger read chunks for big files would
close part of that gap; the memory tier covers it for anything hot. The Go server's 4 MB uploads
are also about 10 % faster, and its misses peak higher (a miss is one `stat` there, an `open` plus
the middleware stack here). `jappyjan`'s stdlib server is 10 to 500× slower depending on size: it
is synchronous and single-threaded by design.

What makes nx-remote-cache fast:

- **Streaming, both ways.** Bodies above the inline threshold stream into multipart writes;
  downloads stream out of the store. Deno's `nx-cache-server` and `cacheiro` read uploads fully
  before sending them to S3.
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
