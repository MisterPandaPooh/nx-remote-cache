#!/usr/bin/env python3
"""Draw images/benchmark.svg from bench/RESULTS.md: one panel per arena and artifact size,
horizontal bars of PUT and GET-hit throughput per server (higher is better)."""
import re
from pathlib import Path

root = Path(__file__).resolve().parent.parent
results = (root / "bench" / "RESULTS.md").read_text()
out = root / "images" / "benchmark.svg"

rows = {}  # size -> server -> phase -> req/s
for size, body in zip(*[iter(re.split(r"^## Artifact size: (\d+) bytes$", results, flags=re.M)[1:])] * 2):
    for line in body.splitlines():
        m = re.match(r"\| (.+?) \| (PUT|GET hit|GET miss) \| (\d+) \|", line)
        if m:
            rows.setdefault(int(size), {}).setdefault(m[1], {})[m[2]] = int(m[3])

ARENAS = [
    ("Object storage (S3Mock)", [
        ("nx-remote-cache (Rust) on S3, memory tier on", "nx-remote-cache, memory tier", "#38bdf8"),
        ("nx-remote-cache (Rust) on S3, memory tier off", "nx-remote-cache, store only", "#6366f1"),
        ("nx-cache-server (Deno) on S3", "nx-cache-server (Deno)", "#64748b"),
    ]),
    ("Local disk (RAM-backed)", [
        ("nx-remote-cache (Rust) on local disk", "nx-remote-cache", "#38bdf8"),
        ("nx-caching-server (Go) on local disk", "nx-caching-server (Go)", "#64748b"),
    ]),
]
PHASES = ["PUT", "GET hit"]
SIZES = sorted(rows)

def human(n):
    return f"{n // 1024} KB" if n < 1 << 20 else f"{n // (1 << 20)} MB"

def fmt(n):
    return f"{n:,}".replace(",", " ")

W, PAD, COL_GAP = 1100, 28, 36
col_w = (W - 2 * PAD - COL_GAP) // 2
label_w, value_w, bar_h, row_gap = 215, 72, 14, 6
bar_w = col_w - label_w - value_w
svg = []
y = PAD

def text(x, yy, s, size=12, fill="#94a3b8", anchor="start", weight="normal", mono=False):
    fam = "ui-monospace, SF Mono, Menlo, monospace" if mono else "ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, sans-serif"
    svg.append(f'<text x="{x}" y="{yy}" font-size="{size}" fill="{fill}" text-anchor="{anchor}" font-weight="{weight}" font-family="{fam}">{s}</text>')

text(PAD, y + 18, "Throughput, requests per second, higher is better", 17, "#e2e8f0", weight="600")
text(PAD, y + 38, "32 connections, 1 000 requests per phase (400 at 4 MB), every server in a container on one Apple M4 Pro", 12)
y += 62

for size in SIZES:
    text(PAD, y + 12, f"{human(size)} artifacts", 14, "#e2e8f0", weight="600")
    y += 26
    panel_top = y
    panel_bottom = y
    for ci, (arena, servers) in enumerate(ARENAS):
        x0 = PAD + ci * (col_w + COL_GAP)
        yy = panel_top
        text(x0, yy + 10, arena, 12, "#64748b", weight="600")
        yy += 22
        data = rows.get(size, {})
        peak = max((data.get(s, {}).get(p, 0) for s, _, _ in servers for p in PHASES), default=1) or 1
        for phase in PHASES:
            text(x0, yy + 10, phase, 11, "#94a3b8", weight="600")
            yy += 16
            for key, label, color in servers:
                v = data.get(key, {}).get(phase, 0)
                w = max(2, round(bar_w * v / peak))
                svg.append(f'<rect x="{x0 + label_w}" y="{yy}" width="{w}" height="{bar_h}" rx="3" fill="{color}" opacity="0.9"/>')
                text(x0 + label_w - 8, yy + bar_h - 3, label, 11, "#cbd5e1", anchor="end")
                text(x0 + label_w + w + 6, yy + bar_h - 3, fmt(v), 11, "#e2e8f0", mono=True)
                yy += bar_h + row_gap
            yy += 6
        panel_bottom = max(panel_bottom, yy)
    y = panel_bottom + 10

text(PAD, y + 8, "Memory-tier hits are served from RAM without a store round trip; the Go server streams disk hits with sendfile(2). Full tables: bench/RESULTS.md", 11, "#64748b")
H = y + 26
doc = f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img" aria-label="nx-remote-cache benchmark">\n<rect width="{W}" height="{H}" rx="14" fill="#0b1120"/>\n' + "\n".join(svg) + "\n</svg>\n"
out.write_text(doc)
print(f"wrote {out} ({W}x{H})")
