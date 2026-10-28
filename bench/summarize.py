#!/usr/bin/env python3
"""Condense bench/RESULTS.md into the README's Performance block (between BENCH:START/END)."""
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
results = (root / "bench" / "RESULTS.md").read_text()
readme_path = root / "README.md"

sections = re.split(r"^## Artifact size: (\d+) bytes$", results, flags=re.M)
rows_by_size = {}
for i in range(1, len(sections), 2):
    size = int(sections[i])
    rows = {}
    for line in sections[i + 1].splitlines():
        m = re.match(r"\| (.+?) \| (PUT|GET hit|GET miss) \| (\d+) \| ([\d.]+) \| ([\d.]+) \| ([\d.]+) \| ([\d.]+) \| (\d+) \|", line)
        if m:
            server, phase, rps, mbs, p50, p90, p99, fail = m.groups()
            rows.setdefault(server, {})[phase] = (int(rps), float(mbs), float(p50), float(p99))
    rows_by_size[size] = rows

def human(n):
    return f"{n / 1024:.0f} KB" if n < 1 << 20 else f"{n / (1 << 20):.0f} MB"

out = []
for size, rows in sorted(rows_by_size.items()):
    out.append(f"**{human(size)} artifacts** · PUT req/s · GET hit req/s (MB/s) · GET miss p50 / p99\n")
    out.append("| Server | PUT | GET hit | GET miss |")
    out.append("|---|---:|---:|---:|")
    for server, phases in rows.items():
        put = phases.get("PUT", (0, 0, 0, 0)); hit = phases.get("GET hit", (0, 0, 0, 0)); miss = phases.get("GET miss", (0, 0, 0, 0))
        out.append(f"| {server} | {put[0]:,} | {hit[0]:,} ({hit[1]:.0f} MB/s) | {miss[2]:.2f} / {miss[3]:.2f} ms |")
    out.append("")
block = "\n".join(out).strip() + "\n"

readme = readme_path.read_text()
new = re.sub(r"<!-- BENCH:START -->.*?<!-- BENCH:END -->", "<!-- BENCH:START -->\n" + block + "<!-- BENCH:END -->", readme, flags=re.S)
if new == readme:
    sys.exit("README has no BENCH block")
readme_path.write_text(new)
print(block)
