#!/usr/bin/env bash
# Run the comparative benchmark. Needs: docker, the nx-remote-cache:dev image (docker build -t nx-remote-cache:dev ..),
# and the bench tool (cargo build --release --bin nx-cache-bench, from the repo root).
set -euo pipefail
cd "$(dirname "$0")"
BENCH="${BENCH:-../target/release/nx-cache-bench}"
REQUESTS="${REQUESTS:-400}"
CONC="${CONC:-32}"
SIZES="${SIZES:-4096 262144 4194304}"
OUT="${OUT:-RESULTS.md}"

docker compose up -d --wait 2>&1 | tail -3

wait_ready() { # url
  for _ in $(seq 1 60); do
    "$BENCH" --url "$1" --token bench --requests 1 --concurrency 1 --size 16 >/dev/null 2>&1 && return 0
    sleep 1
  done
  echo "server $1 never became ready" >&2; return 1
}

targets=(
  "rust-s3|nx-remote-cache (Rust) on S3, memory tier off|http://127.0.0.1:3101"
  "rust-s3-memtier|nx-remote-cache (Rust) on S3, memory tier on|http://127.0.0.1:3102"
  "deno-s3|nx-cache-server (Deno) on S3|http://127.0.0.1:3201"
  "rust-fs|nx-remote-cache (Rust) on local disk|http://127.0.0.1:3103"
  "go-fs|nx-caching-server (Go) on local disk|http://127.0.0.1:3301"
)

{
  echo "# Benchmark results"
  echo
  echo "Generated $(date -u +%Y-%m-%dT%H:%M:%SZ) on $(uname -m) · $(sysctl -n machdep.cpu.brand_string 2>/dev/null || grep -m1 'model name' /proc/cpuinfo | cut -d: -f2) · Docker $(docker version --format '{{.Server.Version}}')"
  echo
  echo "Every server runs in its own container; \`nx-cache-bench\` runs on the host with $CONC concurrent connections, $REQUESTS requests per phase."
  echo "Phases: PUT (unique hashes), GET hit (same hashes), GET miss (unknown hashes). Latencies are end-to-end as the Nx CLI sees them."
  echo
} > "$OUT"

for size in $SIZES; do
  {
    echo "## Artifact size: $size bytes"
    echo
    echo "| Server | Phase | req/s | MB/s | p50 ms | p90 ms | p99 ms | failures |"
    echo "|---|---|---:|---:|---:|---:|---:|---:|"
  } >> "$OUT"
  for t in "${targets[@]}"; do
    IFS='|' read -r id label url <<< "$t"
    wait_ready "$url" || continue
    echo ">> $label · $size bytes"
    "$BENCH" --url "$url" --token bench --requests "$REQUESTS" --concurrency "$CONC" --size "$size" | tail -n 3 | tee /dev/stderr | \
      LABEL="$label" python3 -c '
import os, re, sys
for line in sys.stdin:
    m = re.match(r"^(PUT|GET hit|GET miss)\s+(\d+) req\s+(\d+) req/s\s+([\d.]+) MB/s\s+p50\s+([\d.]+) ms\s+p90\s+([\d.]+) ms\s+p99\s+([\d.]+) ms\s+failures (\d+)", line)
    if m:
        phase, n, rps, mbs, p50, p90, p99, fail = m.groups()
        print(f"| {os.environ[\"LABEL\"]} | {phase} | {rps} | {mbs} | {p50} | {p90} | {p99} | {fail} |")
' >> "$OUT"
  done
  echo >> "$OUT"
done
echo "written $OUT"
