#!/usr/bin/env bash
# Run the comparative benchmark. Needs: docker, the nx-remote-cache:dev image (docker build -t nx-remote-cache:dev ..),
# and the bench tool (cargo build --release --bin nx-cache-bench, from the repo root).
set -euo pipefail
cd "$(dirname "$0")"
# IN_DOCKER=1 runs the load generator inside the compose network (image nx-remote-cache:bench,
# built with `docker build --target bench -t nx-remote-cache:bench ..`); otherwise the host binary.
IN_DOCKER="${IN_DOCKER:-1}"
BENCH_BIN="${BENCH:-../target/release/nx-cache-bench}"
REQUESTS="${REQUESTS:-1000}"
# Artifacts of 1 MB and more use fewer requests so no store spills out of RAM mid-phase.
BIG_REQUESTS="${BIG_REQUESTS:-400}"
# Artifacts of 256 MB and more: a handful of requests, 2 at a time; the local-disk servers run one
# at a time with an empty RAM-backed disk (4 × 1 GB), the others are stopped meanwhile.
HUGE_REQUESTS="${HUGE_REQUESTS:-4}"
HUGE_CONC="${HUGE_CONC:-2}"
FS_TARGETS="rust-fs go-fs shadi-fs cacheiro-fs node-fs"
CONC="${CONC:-32}"
SIZES="${SIZES:-4096 262144 4194304 1073741824}"
OUT="${OUT:-RESULTS.md}"
# ONLY="rust-fs go-fs" restricts the run to these target ids (space-separated).
ONLY="${ONLY:-}"

if [[ "$IN_DOCKER" == "1" ]]; then docker compose --profile tools up -d --wait 2>&1 | tail -3; else docker compose up -d --wait 2>&1 | tail -3; fi

bench() {
  if [[ "$IN_DOCKER" == "1" ]]; then docker compose exec -T bench nx-cache-bench "$@"; else "$BENCH_BIN" "$@"; fi
}

wait_ready() { # url
  for _ in $(seq 1 60); do
    bench --url "$1" --token bench --requests 1 --concurrency 1 --size 16 2>/dev/null | grep -c "failures 0" >/dev/null && return 0
    sleep 1
  done
  echo "server $1 never became ready" >&2; return 1
}
# (a target that never becomes ready is skipped and named in the report)

# shadi-s3 (ShadiBlitz on S3) is defined in docker-compose.yml but not run: every upload fails with
# an aws-sdk "dispatch failure" (streaming body without a length), on S3Mock through the path-style
# rewrite and directly. It is benchmarked on local disk only.
if [[ "$IN_DOCKER" == "1" ]]; then
  targets=(
    "rust-s3|nx-remote-cache (Rust) on S3, memory tier off|http://rust-s3:3000"
    "rust-s3-memtier|nx-remote-cache (Rust) on S3, memory tier on|http://rust-s3-memtier:3000"
    "deno-s3|IKatsuba/nx-cache-server (Deno) on S3|http://deno-s3:3000"
    "tis-s3|the-inconvenience-store/nx-remote-cache (Go) on S3|http://tis-s3:3000"
    "cacheiro-s3|rerodrigues/cacheiro (Node) on S3|http://cacheiro-s3:3000"
    "rust-fs|nx-remote-cache (Rust) on local disk|http://rust-fs:3000"
    "go-fs|enxtur/nx-caching-server (Go) on local disk|http://go-fs:8090"
    "shadi-fs|ShadiBlitz/nx-cache-server (Rust) on local disk|http://shadi-fs:3000"
    "cacheiro-fs|rerodrigues/cacheiro (Node) on local disk|http://cacheiro-fs:3000"
    "node-fs|jappyjan/nx-cache-server (Node) on local disk|http://node-fs:3000"
  )
else
  targets=(
    "rust-s3|nx-remote-cache (Rust) on S3, memory tier off|http://127.0.0.1:3101"
    "rust-s3-memtier|nx-remote-cache (Rust) on S3, memory tier on|http://127.0.0.1:3102"
    "deno-s3|IKatsuba/nx-cache-server (Deno) on S3|http://127.0.0.1:3201"
    "tis-s3|the-inconvenience-store/nx-remote-cache (Go) on S3|http://127.0.0.1:3203"
    "cacheiro-s3|rerodrigues/cacheiro (Node) on S3|http://127.0.0.1:3204"
    "rust-fs|nx-remote-cache (Rust) on local disk|http://127.0.0.1:3103"
    "go-fs|enxtur/nx-caching-server (Go) on local disk|http://127.0.0.1:3301"
    "shadi-fs|ShadiBlitz/nx-cache-server (Rust) on local disk|http://127.0.0.1:3302"
    "cacheiro-fs|rerodrigues/cacheiro (Node) on local disk|http://127.0.0.1:3303"
    "node-fs|jappyjan/nx-cache-server (Node) on local disk|http://127.0.0.1:3304"
  )
fi

{
  echo "# Benchmark results"
  echo
  echo "Generated $(date -u +%Y-%m-%dT%H:%M:%SZ) on $(uname -m) · $(sysctl -n machdep.cpu.brand_string 2>/dev/null || grep -m1 'model name' /proc/cpuinfo | cut -d: -f2) · Docker $(docker version --format '{{.Server.Version}}')"
  echo
  if [[ "$IN_DOCKER" == "1" ]]; then where="in a container on the same Docker network"; else where="on the host"; fi
  echo "Every server runs in its own container; \`nx-cache-bench\` runs $where with $CONC concurrent connections, $REQUESTS requests per phase ($BIG_REQUESTS from 1 MB up, $HUGE_REQUESTS with $HUGE_CONC connections from 256 MB up), after an unmeasured warm-up of at most $((CONC * 2)) uploads."
  echo "Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them."
  echo
} > "$OUT"

for size in $SIZES; do
  conc="$CONC"
  if (( size >= 1 << 28 )); then n="$HUGE_REQUESTS"; conc="$HUGE_CONC"; elif (( size >= 1 << 20 )); then n="$BIG_REQUESTS"; else n="$REQUESTS"; fi
  {
    echo "## Artifact size: $size bytes"
    echo
    echo "| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |"
    echo "|---|---|---:|---:|---:|---:|---:|---:|"
  } >> "$OUT"
  for t in "${targets[@]}"; do
    IFS='|' read -r id label url <<< "$t"
    if [[ -n "$ONLY" && " $ONLY " != *" $id "* ]]; then continue; fi
    if [[ "$id" == *s3* ]]; then
      # Fresh, empty S3Mock for every S3 target: a filling store would penalise whoever runs last.
      docker compose rm -sf s3 >/dev/null 2>&1; docker compose up -d --wait s3 >/dev/null 2>&1
    elif (( size >= 1 << 28 )); then
      # Huge artifacts on RAM-backed disks: only the measured server keeps a disk, the others are
      # stopped so their gigabytes do not sit in the VM's memory meanwhile.
      docker compose rm -sf $FS_TARGETS >/dev/null 2>&1; docker compose up -d --wait "$id" >/dev/null 2>&1
    fi
    if ! wait_ready "$url"; then echo "| $label | (not ready, skipped) | | | | | | |" >> "$OUT"; continue; fi
    echo ">> $label · $size bytes"
    bench --url "$url" --token bench --requests "$n" --concurrency "$conc" --size "$size" | tail -n 3 | tee /dev/stderr | \
      LABEL="$label" python3 ./parse.py >> "$OUT"
  done
  echo >> "$OUT"
done
echo "written $OUT"
