# Versioning and releases

One version number, [semantic](https://semver.org): the server (`Cargo.toml`), the dashboard
(`ui/package.json`), the Helm chart (`version` and `appVersion`) and the default image tag in the
chart always say the same `X.Y.Z`. `scripts/release.py check` enforces it in CI and in the release
workflow. Before `1.0`, minor versions may change behaviour; patch versions never do.

## What a release publishes

| Artifact | Where | Tags |
|---|---|---|
| Image | `ghcr.io/misterpandapooh/nx-remote-cache` and `docker.io/misterpandapooh/nx-remote-cache` | `X.Y.Z`, `X.Y`, `X` (from 1.0), `latest`; `linux/amd64` and `linux/arm64`; SBOM and SLSA provenance attached; signed with cosign (keyless) |
| Helm chart | `oci://ghcr.io/misterpandapooh/charts/nx-remote-cache` | `--version X.Y.Z`; signed with cosign |
| Binaries | GitHub Release `vX.Y.Z` | `nx-remote-cache-X.Y.Z-linux-{amd64,arm64}.tar.gz`, `SHA256SUMS`, the chart `.tgz`, release notes from `CHANGELOG.md` |

Pushes to `main` keep publishing `ghcr.io/…:main` and `…:sha-<commit>` from the CI workflow;
`latest` only moves on a release.

Verify a signature (no key to distribute, the identity is the workflow itself):

```bash
cosign verify ghcr.io/misterpandapooh/nx-remote-cache:0.1.0 \
  --certificate-identity-regexp '^https://github.com/MisterPandaPooh/nx-remote-cache/\.github/workflows/release\.yml@refs/tags/v' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

## Cutting a release

Keep `CHANGELOG.md` current as you go, under a `## Unreleased` heading. Then either:

- **Actions → Release → Run workflow**, enter `X.Y.Z`. The workflow bumps every versioned file,
  turns `## Unreleased` into `## X.Y.Z (date)`, commits `release: vX.Y.Z` on `main`, tags it, and
  carries on with the publishing jobs.
- Or locally: `python3 scripts/release.py bump X.Y.Z`, commit, `git tag vX.Y.Z`, push the commit
  and the tag. The tag push runs the same publishing jobs after checking the files match.

A version with a suffix (`1.2.0-rc.1`) is published as a pre-release and does not move `latest`.

## Secrets and settings

| Name | Kind | Scope | Purpose |
|---|---|---|---|
| `DOCKERHUB_USERNAME` | secret | environment `release` | Docker Hub account (`misterpandapooh`). |
| `DOCKERHUB_TOKEN` | secret | environment `release` | A Docker Hub **access token** (Account settings → Personal access tokens) with *Read & Write* on `misterpandapooh/nx-remote-cache`, never the account password. Rotate it from Docker Hub; nothing else uses it. |

Everything else runs on the job's `GITHUB_TOKEN`: ghcr.io images and chart (`packages: write`),
the release (`contents: write`), the version commit (`contents: write` on the prepare job only)
and the OIDC token cosign signs with (`id-token: write`).

## Why the workflow is safe to leave unattended

- It only runs on a `v*` tag push or a manual dispatch, never on pull requests, so secrets are
  never exposed to code from a fork.
- The publishing jobs run in the `release` environment. The two Docker Hub secrets live there,
  not at repository level, so no other workflow can read them. Add required reviewers to that
  environment and every release waits for a human click.
- Every job declares the least permissions it needs; the workflow default is none.
- Every action is pinned to a commit SHA, with the version in a comment; Dependabot proposes
  updates.
- Images and chart are signed keylessly: there is no signing key to leak or rotate, and the
  signature names the exact workflow and tag that produced the artifact.
- Provenance and SBOM are attached to every image, so what shipped can be audited later.
- A tag created by the workflow cannot start another workflow (GitHub blocks `GITHUB_TOKEN`
  recursion), which is why the dispatch path continues in the same run.
- The repository ruleset `release tags` forbids updating or deleting `v*` tags: a published
  version always points at the commit it was built from.
