# GitHub Actions for nx-remote-cache

Three composite actions, usable from this repository today and published to the Marketplace as
`nx-remote-cache-*` (one repository per action, see [publishing](#publishing)).

| Action | What it does |
|---|---|
| [`remote-cache`](remote-cache/action.yml) | Restore and save any path by key against your server, like `actions/cache`. |
| [`kv-set-values`](kv-set-values/action.yml) | Store `key=value` pairs in the key-value store, scoped globally, per repository or per run, with a TTL; secrets masked. |
| [`kv-get-values`](kv-get-values/action.yml) | Read keys back as a JSON output and, on request, as environment variables; secrets masked. |

All three take `server` and `token`. The run id, repository, ref and actor travel in the token
context, so the server scopes `pipeline` and `run` keys without any extra input and the dashboard
links every artifact to its run.

## remote-cache

```yaml
- uses: MisterPandaPooh/nx-remote-cache/actions/remote-cache@main
  with:
    server: https://nx-cache.example.com
    token: ${{ secrets.NX_CACHE_TOKEN }}
    key: node-modules-${{ runner.os }}-${{ hashFiles('pnpm-lock.yaml') }}
    path: node_modules
```

Outputs `cache-hit`. Keys are immutable: put a content hash in them.

## kv-set-values

```yaml
- uses: MisterPandaPooh/nx-remote-cache/actions/kv-set-values@main
  with:
    server: https://nx-cache.example.com
    token: ${{ secrets.NX_CACHE_TOKEN }}
    level: run            # global | pipeline | run
    ttl: 2h
    secret: "true"        # masked here and in the dashboard
    values: |
      preview-url=https://pr-${{ github.event.number }}.preview.example.com
      preview-token=${{ steps.deploy.outputs.token }}
```

`values` is newline-separated `key=value`; or give one `key` with `value` or `value-file`.
`content-type: application/json` asks the server to validate JSON. Needs a read-write token.

## kv-get-values

```yaml
- id: kv
  uses: MisterPandaPooh/nx-remote-cache/actions/kv-get-values@main
  with:
    server: https://nx-cache.example.com
    token: ${{ secrets.NX_CACHE_READ_TOKEN }}
    level: run
    keys: |
      preview-url
      preview-token
    required: "true"
    export-env: "true"    # KV_PREVIEW_URL, KV_PREVIEW_TOKEN for the next steps
- run: echo "testing ${{ fromJSON(steps.kv.outputs.values)['preview-url'] }}"
```

Outputs `values` (JSON object of the keys found) and `found`. A value the server marks as a
secret is masked before it is written anywhere. Values are text; binary values are not
supported by this action.

## Publishing

The Marketplace lists one action per repository, with `action.yml` at the root. The sources live
here; [`scripts/publish-actions.sh`](../scripts/publish-actions.sh) pushes each folder as a git
subtree to its own repository (`nx-remote-cache-action`, `nx-remote-cache-kv-set-values`,
`nx-remote-cache-kv-get-values`) and tags it. Ticking "Publish this Action to the GitHub
Marketplace" on that repository's release page is the one manual step.
