# Using it from CI

## Use it from GitHub Actions

Give pull-request builds the read-only token and keep the read-write token for trusted branches:
that is what protects you from cache poisoning (CREEP, CVE-2025-36852), where an untrusted build
pre-seeds the entry a trusted build will later look up. Details in [`SECURITY.md`](../SECURITY.md).

[`examples/github-actions/nx-ci.yml`](../examples/github-actions/nx-ci.yml) is a complete workflow:
read-only token on pull requests, read-write on `main`, a warm job after each merge, and a job
summary with the hit rate.

The server is also a plain content-addressed blob store, so it works **outside Nx**. The composite
action in [`actions/remote-cache`](../actions/remote-cache/action.yml) restores and saves any path by
key, like `actions/cache`, against your own server:

```yaml
- uses: MisterPandaPooh/nx-remote-cache/actions/remote-cache@main
  with:
    server: https://nx-cache.example.com
    token: ${{ secrets.NX_CACHE_TOKEN }}
    key: node-modules-${{ runner.os }}-${{ hashFiles('pnpm-lock.yaml') }}
    path: node_modules
```

Two more composite actions wrap the key-value store (`/v1/kv`) for passing values between jobs
and workflows; see [`actions/README.md`](../actions/README.md) for every input. `kv-set-values`
stores `key=value` pairs, scoped globally, per repository (`pipeline`) or per run, with a TTL:

```yaml
- uses: MisterPandaPooh/nx-remote-cache/actions/kv-set-values@main
  with:
    server: https://nx-cache.example.com
    token: ${{ secrets.NX_CACHE_TOKEN }}
    level: run
    ttl: 2h
    secret: "true"
    values: |
      preview-url=https://pr-${{ github.event.number }}.preview.example.com
```

`kv-get-values` reads keys back as a JSON output and, with `export-env`, as environment variables:

```yaml
- id: kv
  uses: MisterPandaPooh/nx-remote-cache/actions/kv-get-values@main
  with:
    server: https://nx-cache.example.com
    token: ${{ secrets.NX_CACHE_READ_TOKEN }}
    level: run
    keys: preview-url
    export-env: "true"    # KV_PREVIEW_URL for the next steps
```

Secrets are masked: `kv-set-values` with `secret: "true"` masks the values in the job log and
the server hides them in the dashboard, and `kv-get-values` masks any value the server marks as
a secret before it prints or exports anything.

### Link artifacts to CI runs

The Nx client sends nothing but the hash and a token. The token is opaque to Nx, so the server
accepts a context after the secret:

```yaml
NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN: "${{ secrets.NX_CACHE_TOKEN }};run=${{ github.run_id }};repo=${{ github.repository }};ref=${{ github.ref_name }};actor=${{ github.actor }}"
```

Every event then carries who asked (`ctx`) and, for hits, who produced the artifact (`origin`,
stored as blob metadata on upload). The dashboard turns that into links to the GitHub Actions runs:
"produced by run 1234 on `main`, reused by run 1290 on `feat/x`".
