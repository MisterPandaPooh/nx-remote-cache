#!/usr/bin/env python3
"""Turn nx-cache-bench output lines into Markdown table rows. Usage: LABEL=... parse.py < bench-output"""
import os
import re
import sys

PATTERN = re.compile(
    r"^(PUT|GET hit|GET miss)\s+(\d+) req\s+(\d+) req/s\s+([\d.]+) MB/s\s+"
    r"p50\s+([\d.]+) ms\s+p90\s+([\d.]+) ms\s+p99\s+([\d.]+) ms\s+failures (\d+)"
)
label = os.environ.get("LABEL", "?")
for line in sys.stdin:
    m = PATTERN.match(line.strip())
    if m:
        phase, _n, rps, mbs, p50, p90, p99, fail = m.groups()
        print(f"| {label} | {phase} | {rps} | {mbs} | {p50} | {p90} | {p99} | {fail} |")
