#!/usr/bin/env bash
# End-to-end test of kv-set.sh and kv-get.sh against a running server.
# Usage: actions/test.sh <server-url> <read-write-token>
set -euo pipefail

server="${1:?usage: test.sh <server-url> <token>}"
token="${2:?usage: test.sh <server-url> <token>}"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
set_sh="$here/kv-set-values/kv-set.sh"
get_sh="$here/kv-get-values/kv-get.sh"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export GITHUB_OUTPUT="$tmp/output" GITHUB_ENV="$tmp/env"
export GITHUB_RUN_ID=777 GITHUB_REPOSITORY=acme/app GITHUB_REF_NAME=main GITHUB_ACTOR=ci
: > "$GITHUB_OUTPUT"; : > "$GITHUB_ENV"
export INPUT_SERVER="$server" INPUT_TOKEN="$token"

fail() { echo "FAIL: $*" >&2; exit 1; }
expect() { # description, grep args...
  local what="$1"; shift
  grep -q "$@" || fail "$what"
  echo "ok: $what"
}
reset_inputs() { unset INPUT_LEVEL INPUT_SECRET INPUT_TTL INPUT_VALUES INPUT_KEY INPUT_VALUE INPUT_CONTENT_TYPE INPUT_KEYS INPUT_EXPORT_ENV INPUT_REQUIRED; }

# 1. several secret values at run level, with a TTL
reset_inputs
INPUT_LEVEL=run INPUT_SECRET=true INPUT_TTL=1h \
  INPUT_VALUES=$'preview-url=https://pr-1.preview.example.com\npreview-token=s3cr3t-value' \
  bash "$set_sh" > "$tmp/set1.out"
cat "$tmp/set1.out"
expect "set: two values stored" -F "count=2" "$GITHUB_OUTPUT"
expect "set: log line has level, ttl and secret" -F "stored preview-url (level run, ttl 1h, secret)" "$tmp/set1.out"
expect "set: secret value masked" -F "::add-mask::s3cr3t-value" "$tmp/set1.out"

# 2. one JSON value at pipeline level, no ttl, not secret
reset_inputs
INPUT_LEVEL=pipeline INPUT_KEY=build-number INPUT_VALUE=42 INPUT_CONTENT_TYPE=application/json \
  bash "$set_sh" > "$tmp/set2.out"
cat "$tmp/set2.out"
expect "set: plain log line without trailing flags" -Fx "kv-set-values: stored build-number (level pipeline)" "$tmp/set2.out"

# 3. an invalid key is rejected
reset_inputs
if INPUT_LEVEL=run INPUT_KEY='bad key' INPUT_VALUE=x bash "$set_sh" > "$tmp/set3.out" 2>&1; then
  fail "set: invalid key was accepted"
fi
expect "set: invalid key rejected" -F "invalid key" "$tmp/set3.out"

# 4. read back with one missing key and export to env
: > "$GITHUB_OUTPUT"
reset_inputs
INPUT_LEVEL=run INPUT_KEYS=$'preview-url\npreview-token\nno-such-key' INPUT_EXPORT_ENV=true \
  bash "$get_sh" > "$tmp/get1.out"
cat "$tmp/get1.out"
expect "get: found=2" -Fx "found=2" "$GITHUB_OUTPUT"
expect "get: url in values json" -F "https://pr-1.preview.example.com" "$GITHUB_OUTPUT"
expect "get: token in values json" -F "s3cr3t-value" "$GITHUB_OUTPUT"
expect "get: env name without trailing underscore" -F "KV_PREVIEW_URL<<" "$GITHUB_ENV"
expect "get: env token exported" -F "KV_PREVIEW_TOKEN<<" "$GITHUB_ENV"
if grep -q "KV_PREVIEW_URL_" "$GITHUB_ENV"; then fail "get: env name has a trailing underscore"; fi
expect "get: secrets masked in stdout" -F "::add-mask::s3cr3t-value" "$tmp/get1.out"
expect "get: missing key reported" -F "no-such-key not found" "$tmp/get1.out"

# 5. required missing key fails
reset_inputs
if INPUT_LEVEL=run INPUT_KEYS=no-such-key INPUT_REQUIRED=true bash "$get_sh" > "$tmp/get2.out" 2>&1; then
  fail "get: required missing key did not fail"
fi
expect "get: required missing key fails" -F "not found" "$tmp/get2.out"

echo "actions: ok"
