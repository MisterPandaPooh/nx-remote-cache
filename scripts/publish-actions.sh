#!/usr/bin/env bash
# Push each action folder to its own repository (the Marketplace wants action.yml at the root)
# and move its major tag. Usage: scripts/publish-actions.sh v1.0.0 [owner]
# Needs: gh (logged in), push rights on the target repositories; creates them when missing.
set -euo pipefail
version="${1:?version, e.g. v1.0.0}"
owner="${2:-MisterPandaPooh}"
major="${version%%.*}"
[[ "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must look like v1.2.3" >&2; exit 1; }
cd "$(dirname "$0")/.."
[[ -z "$(git status --porcelain)" ]] || { echo "commit or stash first: the subtree split needs a clean tree" >&2; exit 1; }

publish() { # folder, repository name, description
  local folder="$1" repo="$owner/$2" desc="$3" split
  gh repo view "$repo" >/dev/null 2>&1 || gh repo create "$repo" --public --description "$desc"
  split=$(git subtree split --prefix "actions/$folder" HEAD)
  git push --force "https://github.com/$repo.git" "$split:refs/heads/main"
  git push --force "https://github.com/$repo.git" "$split:refs/tags/$version" "$split:refs/tags/$major"
  gh release view "$version" --repo "$repo" >/dev/null 2>&1 \
    || gh release create "$version" --repo "$repo" --title "$version" --notes "nx-remote-cache $folder action $version. Sources: https://github.com/$owner/nx-remote-cache/tree/main/actions/$folder"
  echo "published $repo @ $version ($major moved)"
}

publish remote-cache  nx-remote-cache-action        "Cache any path by key on your nx-remote-cache server, like actions/cache"
publish kv-set-values nx-remote-cache-kv-set-values "Store values in the nx-remote-cache key-value store (per run, per repository or global; secrets masked)"
publish kv-get-values nx-remote-cache-kv-get-values "Read values from the nx-remote-cache key-value store into outputs and env vars"
echo "Now tick 'Publish this Action to the GitHub Marketplace' on each release page."
