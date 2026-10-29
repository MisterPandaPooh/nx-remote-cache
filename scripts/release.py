#!/usr/bin/env python3
"""Release helper: one version everywhere (server, dashboard, chart, image tag, changelog).

  release.py current            print the version in Cargo.toml
  release.py bump X.Y.Z         write X.Y.Z into every versioned file; turn "## Unreleased" into a dated section
  release.py check X.Y.Z        fail unless every versioned file already says X.Y.Z
  release.py notes X.Y.Z        print the CHANGELOG section of X.Y.Z (release notes)
"""
import datetime
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SEMVER = re.compile(r"^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$")


def read(p):
    return (ROOT / p).read_text()


def write(p, s):
    (ROOT / p).write_text(s)


def sub1(text, pattern, repl, what):
    new, n = re.subn(pattern, repl, text, count=1, flags=re.M)
    if n != 1:
        sys.exit(f"release.py: could not find {what}")
    return new


def current():
    return re.search(r'^version = "([^"]+)"', read("Cargo.toml"), re.M).group(1)


def versioned_files():
    """(path, regex capturing the version, replacement template) for every file that carries it."""
    return [
        ("Cargo.toml", r'^(version = ")([^"]+)(")', r"\g<1>{v}\g<3>"),
        ("Cargo.lock", r'(name = "nx-remote-cache"\nversion = ")([^"]+)(")', r"\g<1>{v}\g<3>"),
        ("ui/package.json", r'^(  "version": ")([^"]+)(")', r"\g<1>{v}\g<3>"),
        ("charts/nx-remote-cache/Chart.yaml", r'^(version: )(\S+)', r"\g<1>{v}"),
        ("charts/nx-remote-cache/Chart.yaml", r'^(appVersion: ")([^"]+)(")', r"\g<1>{v}\g<3>"),
        ("charts/nx-remote-cache/Chart.yaml", r'(image: ghcr\.io/misterpandapooh/nx-remote-cache:)(\S+)', r"\g<1>{v}"),
        ("charts/nx-remote-cache/values.yaml", r'^(  tag: ")([^"]+)(")', r"\g<1>{v}\g<3>"),
    ]


def bump(v):
    for path, pattern, repl in versioned_files():
        write(path, sub1(read(path), pattern, repl.replace("{v}", v), f"version in {path}"))
    log = read("CHANGELOG.md")
    today = datetime.date.today().isoformat()
    if re.search(r"^## Unreleased\s*$", log, re.M):
        log = sub1(log, r"^## Unreleased\s*$", f"## {v} ({today})", "Unreleased heading")
        write("CHANGELOG.md", log)
    elif not re.search(rf"^## {re.escape(v)}\b", log, re.M):
        sys.exit(f"release.py: CHANGELOG.md has neither '## Unreleased' nor '## {v}'")
    print(f"bumped to {v}")


def check(v):
    bad = []
    for path, pattern, _ in versioned_files():
        m = re.search(pattern, read(path), re.M)
        found = m.group(2) if m else None
        if found != v:
            bad.append(f"{path}: {found}")
    if not re.search(rf"^## {re.escape(v)}\b", read("CHANGELOG.md"), re.M):
        bad.append(f"CHANGELOG.md: no '## {v}' section")
    if bad:
        sys.exit("release.py: not everything is at " + v + ":\n  " + "\n  ".join(bad))
    print(f"all files at {v}")


def notes(v):
    log = read("CHANGELOG.md")
    m = re.search(rf"^## {re.escape(v)}\b[^\n]*\n(.*?)(?=^## |\Z)", log, re.M | re.S)
    if not m:
        sys.exit(f"release.py: no '## {v}' section in CHANGELOG.md")
    body = m.group(1).strip()
    print(body + "\n\n"
          f"Images: `ghcr.io/misterpandapooh/nx-remote-cache:{v}`, `docker.io/misterpandapooh/nx-remote-cache:{v}` "
          f"(linux/amd64, linux/arm64, signed with cosign).\n"
          f"Chart: `helm install nx-remote-cache oci://ghcr.io/misterpandapooh/charts/nx-remote-cache --version {v}`.")


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "help"
    if cmd == "current":
        print(current())
    elif cmd in ("bump", "check", "notes"):
        v = sys.argv[2] if len(sys.argv) > 2 else ""
        if not SEMVER.match(v):
            sys.exit(f"release.py: '{v}' is not a semantic version (x.y.z)")
        {"bump": bump, "check": check, "notes": notes}[cmd](v)
    else:
        sys.exit(__doc__)
