#!/usr/bin/env python3
"""Draw images/benchmark.svg from bench/RESULTS.md.

Small panels, one per arena, artifact size and operation. Inside a panel every bar starts at 0
on the same linear scale, so a bar twice as long is twice the throughput; the number after each
bar is the absolute requests per second. This project's memory tier is a bar like any other in
the GET hit panels of the object-storage arena, labelled as hits served from RAM.

Usage: chart.py [output.svg]
"""
import re
import sys
from pathlib import Path

root = Path(__file__).resolve().parent.parent
results = (root / "bench" / "RESULTS.md").read_text()
out = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "images" / "benchmark.svg"

rows = {}  # size -> server -> phase -> req/s
for size, body in zip(*[iter(re.split(r"^## Artifact size: (\d+) bytes$", results, flags=re.M)[1:])] * 2):
    for line in body.splitlines():
        m = re.match(r"\| (.+?) \| (PUT|GET hit|GET miss) \| (\d+) \|", line)
        if m:
            rows.setdefault(int(size), {}).setdefault(m[1], {})[m[2]] = int(m[3])

# One colour per server, the same in every panel.
COLORS = {
    "this project, hits from RAM (memory tier)": "#7dd3fc",
    "this project, hits from S3": "#38bdf8",
    "this project": "#38bdf8",
    "the-inconvenience-store (Go)": "#34d399",
    "IKatsuba (Deno)": "#fbbf24",
    "cacheiro (Node)": "#f472b6",
    "enxtur (Go)": "#34d399",
    "ShadiBlitz (Rust)": "#fb923c",
    "jappyjan (Node)": "#a78bfa",
}
# (key in RESULTS.md, label, phases drawn)
ARENAS = [
    ("Object storage: every server on the same S3 bucket", [
        ("nx-remote-cache (Rust) on S3, memory tier on", "this project, hits from RAM (memory tier)", ["GET hit"]),
        ("nx-remote-cache (Rust) on S3, memory tier off", "this project, hits from S3", ["PUT", "GET hit"]),
        ("the-inconvenience-store/nx-remote-cache (Go) on S3", "the-inconvenience-store (Go)", ["PUT", "GET hit"]),
        ("IKatsuba/nx-cache-server (Deno) on S3", "IKatsuba (Deno)", ["PUT", "GET hit"]),
        ("rerodrigues/cacheiro (Node) on S3", "cacheiro (Node)", ["PUT", "GET hit"]),
    ]),
    ("Local disk: every server on the same RAM-backed tmpfs", [
        ("nx-remote-cache (Rust) on local disk", "this project", ["PUT", "GET hit"]),
        ("enxtur/nx-caching-server (Go) on local disk", "enxtur (Go)", ["PUT", "GET hit"]),
        ("ShadiBlitz/nx-cache-server (Rust) on local disk", "ShadiBlitz (Rust)", ["PUT", "GET hit"]),
        ("rerodrigues/cacheiro (Node) on local disk", "cacheiro (Node)", ["PUT", "GET hit"]),
        ("jappyjan/nx-cache-server (Node) on local disk", "jappyjan (Node)", ["PUT", "GET hit"]),
    ]),
]
SIZES = sorted(rows)
PHASES = ["PUT", "GET hit"]

W, PAD, COL_GAP = 1100, 28, 28
PANEL_W = (W - 2 * PAD - COL_GAP) // 2
LABEL_W, VALUE_W = 236, 96
BAR_MAX = PANEL_W - LABEL_W - VALUE_W - 24
BAR_H, ROW_H = 11, 21
svg = []
y = PAD


def human(n):
    return f"{n // 1024} KB" if n < 1 << 20 else f"{n // (1 << 20)} MB"


def fmt(n):
    return f"{n:,}".replace(",", " ")


def text(x, yy, s, size=12, fill="#94a3b8", anchor="start", weight="normal", mono=False):
    fam = "ui-monospace, SF Mono, Menlo, monospace" if mono else "ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, sans-serif"
    svg.append(f'<text x="{x:.1f}" y="{yy:.1f}" font-size="{size}" fill="{fill}" text-anchor="{anchor}" font-weight="{weight}" font-family="{fam}">{s}</text>')


text(PAD, y + 18, "Throughput in requests per second (req/s), higher is better", 18, "#e2e8f0", weight="600")
text(PAD, y + 40, "Every open-source Nx cache server with a published image, in containers on one Apple M4 Pro, 32 connections, 1 000 requests per phase (400 at 4 MB).", 12)
text(PAD, y + 57, "In each panel every bar starts at 0 on one linear scale: twice the length is twice the throughput. Full tables and method: bench/RESULTS.md", 12)
y += 80

for arena, servers in ARENAS:
    text(PAD, y + 14, arena, 15, "#e2e8f0", weight="600")
    y += 26
    # legend: one swatch per server of this arena
    lx = PAD
    for _, label, _ in servers:
        svg.append(f'<rect x="{lx}" y="{y + 2}" width="12" height="12" rx="3" fill="{COLORS[label]}"/>')
        text(lx + 17, y + 12, label, 11, "#cbd5e1")
        lx += 17 + len(label) * 6.4 + 22
    y += 26
    for size in SIZES:
        data = rows.get(size, {})
        panel_top = y
        panel_h = 0
        for col, phase in enumerate(PHASES):
            x0 = PAD + col * (PANEL_W + COL_GAP)
            entries = [(key, label) for key, label, phases in servers if phase in phases]
            peak = max((data.get(k, {}).get(phase, 0) for k, _ in entries), default=1) or 1
            h = 30 + len(entries) * ROW_H + 10
            panel_h = max(panel_h, h)
            svg.append(f'<rect x="{x0}" y="{panel_top}" width="{PANEL_W}" height="{h}" rx="10" fill="#111a2e" stroke="#1e293b"/>')
            text(x0 + 14, panel_top + 19, f"{phase} · {human(size)} artifacts", 12, "#cbd5e1", weight="600")
            yy = panel_top + 30
            bx = x0 + 14 + LABEL_W
            svg.append(f'<line x1="{bx}" y1="{yy}" x2="{bx}" y2="{yy + len(entries) * ROW_H}" stroke="#334155"/>')
            for key, label in entries:
                ours = label.startswith("this project")
                v = data.get(key, {}).get(phase, 0)
                w = max(1.5, BAR_MAX * v / peak)
                text(bx - 8, yy + ROW_H / 2 + 4, label, 11, "#e2e8f0" if ours else "#94a3b8", anchor="end", weight="600" if ours else "normal")
                svg.append(f'<rect x="{bx}" y="{yy + (ROW_H - BAR_H) / 2:.1f}" width="{w:.1f}" height="{BAR_H}" rx="2" fill="{COLORS[label]}" opacity="{1 if ours else 0.85}"/>')
                text(bx + w + 6, yy + ROW_H / 2 + 4, f"{fmt(v)} req/s", 10.5, "#e2e8f0" if ours else "#94a3b8", mono=True)
                yy += ROW_H
        y = panel_top + panel_h + 12
    y += 18

H = y
doc = f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img" aria-label="nx-remote-cache benchmark">\n<rect width="{W}" height="{H}" rx="14" fill="#0b1120"/>\n' + "\n".join(svg) + "\n</svg>\n"
out.write_text(doc)
print(f"wrote {out} ({W}x{H})")
