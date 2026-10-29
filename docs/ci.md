# Using it from CI

## Use it from GitHub Actions

Give pull-request builds the read-only token and keep the read-write token for trusted branches:
that is what protects you from cache poisoning (CREEP, CVE-2025-36852), where an untrusted build
pre-seeds the entry a trusted build will later look up. Details in [`SECURITY.md`](../SECURITY.md).

[`examples/github-actions/nx-ci.yml`](../examples/github-actions/nx-ci.yml) is a complete workflow:
read-only token on pull requests, read-write on `main`, a warm job after each merge, and a job
summary with the hit rate.

The server is also a plain content-addressed blob store, so it works **outside Nx**. The composite
action in [`.github/actions/cache`](../.github/actions/cache/action.yml) restores and saves any path by
key, like `actions/cache`, against your own server:

```yaml
- uses: MisterPandaPooh/nx-remote-cache/.github/actions/cache@main
  with:
    server: https://nx-cache.example.com
    token: ${{ secrets.NX_CACHE_TOKEN }}
    key: node-modules-${{ runner.os }}-${{ hashFiles('pnpm-lock.yaml') }}
    path: node_modules
```

### Link artifacts to CI runs

The Nx client sends nothing but the hash and a token. The token is opaque to Nx, so the server
accepts a context after the secret:

```yaml
NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN: "${{ secrets.NX_CACHE_TOKEN }};run=${{ github.run_id }};repo=${{ github.repository }};ref=${{ github.ref_name }};actor=${{ github.actor }}"
```

Every event then carries who asked (`ctx`) and, for hits, who produced the artifact (`origin`,
stored as blob metadata on upload). The dashboard turns that into links to the GitHub Actions runs:
"produced by run 1234 on `main`, reused by run 1290 on `feat/x`".
