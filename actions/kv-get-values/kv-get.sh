#!/usr/bin/env bash
# Read keys from an nx-remote-cache key-value store into a JSON output and, optionally, env vars.
# Values the server marks as secrets (x-kv-secret: true) are masked before anything is printed.
set -euo pipefail

: "${INPUT_SERVER:?server is required}" "${INPUT_TOKEN:?token is required}" "${INPUT_KEYS:?keys are required}"
command -v jq >/dev/null || { echo "::error::kv-get-values: jq is required on the runner"; exit 1; }
server="${INPUT_SERVER%/}"
level="${INPUT_LEVEL:-global}"
case "$level" in global|pipeline|run) ;; *) echo "::error::kv-get-values: level must be global, pipeline or run (got '$level')"; exit 1;; esac
ctx="$INPUT_TOKEN;run=${GITHUB_RUN_ID:-};repo=${GITHUB_REPOSITORY:-};ref=${GITHUB_REF_NAME:-};actor=${GITHUB_ACTOR:-}"

json="{}"
found=0
while IFS= read -r key; do
  key="${key//[[:space:]]/}"
  [[ -z "$key" || "$key" == \#* ]] && continue
  [[ "$key" =~ ^[A-Za-z0-9_.:@-]{1,200}$ ]] || { echo "::error::kv-get-values: invalid key '$key'"; exit 1; }
  code=$(curl -sS -o /tmp/kv-get.out -D /tmp/kv-get.hdr -w '%{http_code}' \
    -H "Authorization: Bearer $ctx" "$server/v1/kv/$key?level=$level")
  case "$code" in
    200)
      value=$(cat /tmp/kv-get.out)
      if grep -qi '^x-kv-secret: *true' /tmp/kv-get.hdr; then
        [[ -n "$value" ]] && echo "::add-mask::$value"
        echo "kv-get-values: $key (secret, masked)"
      else
        echo "kv-get-values: $key ($(wc -c < /tmp/kv-get.out | tr -d ' ') bytes)"
      fi
      json=$(jq -c --arg k "$key" --arg v "$value" '. + {($k): $v}' <<< "$json")
      found=$((found + 1))
      if [[ "${INPUT_EXPORT_ENV:-false}" == "true" ]]; then
        name="${INPUT_ENV_PREFIX:-KV_}$(printf '%s' "$key" | tr -c 'A-Za-z0-9_' '_' | tr 'a-z' 'A-Z')"
        delim="__KV_$(date +%s%N)__"
        printf '%s<<%s\n%s\n%s\n' "$name" "$delim" "$value" "$delim" >> "${GITHUB_ENV:-/dev/null}"
      fi
      ;;
    404)
      if [[ "${INPUT_REQUIRED:-false}" == "true" ]]; then
        echo "::error::kv-get-values: $key not found (level $level)"; exit 1
      fi
      echo "kv-get-values: $key not found (level $level)"
      ;;
    *)
      echo "::error::kv-get-values: $key -> HTTP $code: $(head -c 300 /tmp/kv-get.out)"; exit 1
      ;;
  esac
done <<< "$INPUT_KEYS"

{
  echo "values=$json"
  echo "found=$found"
} >> "${GITHUB_OUTPUT:-/dev/null}"
