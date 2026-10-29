#!/usr/bin/env bash
# Store key=value pairs in an nx-remote-cache key-value store. Inputs arrive as INPUT_* variables
# (see action.yml); the GitHub run and repository travel in the token context so `pipeline` and
# `run` levels resolve on the server.
set -euo pipefail

: "${INPUT_SERVER:?server is required}" "${INPUT_TOKEN:?token is required}"
server="${INPUT_SERVER%/}"
level="${INPUT_LEVEL:-global}"
case "$level" in global|pipeline|run) ;; *) echo "::error::kv-set-values: level must be global, pipeline or run (got '$level')"; exit 1;; esac
secret="${INPUT_SECRET:-false}"
ctx="$INPUT_TOKEN;run=${GITHUB_RUN_ID:-};repo=${GITHUB_REPOSITORY:-};ref=${GITHUB_REF_NAME:-};actor=${GITHUB_ACTOR:-}"

query="level=$level"
[[ -n "${INPUT_TTL:-}" ]] && query+="&ttl=$INPUT_TTL"
[[ "$secret" == "true" ]] && query+="&secret=true"

put() { # key, value-source (--data-binary argument)
  local key="$1" data="$2" code
  [[ "$key" =~ ^[A-Za-z0-9_.:@-]{1,200}$ ]] || { echo "::error::kv-set-values: invalid key '$key' ([A-Za-z0-9_.:@-]{1,200})"; exit 1; }
  code=$(curl -sS -o /tmp/kv-set.out -w '%{http_code}' -X PUT \
    -H "Authorization: Bearer $ctx" -H "Content-Type: ${INPUT_CONTENT_TYPE:-text/plain}" \
    --data-binary "$data" "$server/v1/kv/$key?$query")
  if [[ "$code" != "204" ]]; then
    echo "::error::kv-set-values: $key -> HTTP $code: $(head -c 300 /tmp/kv-set.out)"; exit 1
  fi
  local info="level $level"
  [[ -n "${INPUT_TTL:-}" ]] && info+=", ttl $INPUT_TTL"
  [[ "$secret" == "true" ]] && info+=", secret"
  echo "kv-set-values: stored $key ($info)"
}

count=0
if [[ -n "${INPUT_KEY:-}" ]]; then
  if [[ -n "${INPUT_VALUE_FILE:-}" ]]; then
    [[ -f "$INPUT_VALUE_FILE" ]] || { echo "::error::kv-set-values: value-file '$INPUT_VALUE_FILE' not found"; exit 1; }
    [[ "$secret" == "true" ]] && echo "::add-mask::$(cat "$INPUT_VALUE_FILE")"
    put "$INPUT_KEY" "@$INPUT_VALUE_FILE"
  else
    [[ "$secret" == "true" && -n "${INPUT_VALUE:-}" ]] && echo "::add-mask::$INPUT_VALUE"
    put "$INPUT_KEY" "${INPUT_VALUE:-}"
  fi
  count=$((count + 1))
fi
while IFS= read -r line; do
  [[ -z "$line" || "$line" == \#* ]] && continue
  [[ "$line" == *=* ]] || { echo "::error::kv-set-values: expected key=value, got '$line'"; exit 1; }
  key="${line%%=*}"; value="${line#*=}"
  [[ "$secret" == "true" && -n "$value" ]] && echo "::add-mask::$value"
  put "$key" "$value"
  count=$((count + 1))
done <<< "${INPUT_VALUES:-}"

[[ "$count" -gt 0 ]] || { echo "::error::kv-set-values: nothing to store (set key/value or values)"; exit 1; }
echo "count=$count" >> "${GITHUB_OUTPUT:-/dev/null}"
