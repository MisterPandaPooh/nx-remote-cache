# Security policy

## Reporting a vulnerability

Please do not open a public issue for a security problem. Use GitHub's private reporting on
this repository (**Security → Report a vulnerability**). You will get an acknowledgement within
a few days and a fix or a mitigation before any public disclosure.

## Supported versions

Only the latest release and `main` receive security fixes.

## What the server trusts and what it does not

- **Cache API** (`/v1/cache/*`): bearer tokens compared in constant time; a read-only token
  cannot upload. Tokens are shared secrets, so generate them with at least 32 random bytes
  (`openssl rand -hex 32`) and rotate them by restarting with a new value.
- **Hashes** are validated against `[A-Za-z0-9_-]{1,128}` before touching any store, which rules
  out path traversal on `file://` stores and key injection on object stores.
- **Uploads** are capped in size (`NX_CACHE_MAX_ARTIFACT_BYTES`) and in number in flight
  (`NX_CACHE_MAX_CONCURRENT_PUTS`); rejected uploads are drained up to 64 MiB so the client
  sees the status code, then the connection is closed.
- **Token context** (`secret;run=…;repo=…`) is attacker-controlled by anyone holding a valid
  token. It is limited to 8 pairs of printable ASCII (keys `[a-z0-9_]{1,16}`, values ≤ 96 bytes),
  stored as object metadata, escaped by React in the dashboard, and only ever turned into a
  link under `https://github.com/`.
- **Dashboard API** (`/api/v1/*`) requires `NX_CACHE_UI_TOKEN` or a cache token. With
  `NX_CACHE_UI_TRUSTED_HEADER` set, any request carrying that header is let in: the server must
  then be reachable only through the proxy that sets it, and the proxy must strip the header
  from incoming requests.
- `/metrics`, `/healthz` and `/readyz` are unauthenticated by design and expose counters only
  (never hashes, tokens or store credentials). Restrict them with a network policy if needed.
- **TLS** is terminated by your ingress or gateway; the server speaks plain HTTP (h1 and h2c).
- Store credentials never appear in logs, `/api/v1/stats` or `/metrics`; URLs are redacted.

## Cache poisoning by untrusted builds (CREEP, CVE-2025-36852)

The attack: a build that untrusted people can influence (a pull request from a fork) holds a
token that can write, computes or guesses the hash a trusted branch will produce later, and
stores a malicious artifact under it first. The trusted build then replays it. Write-once
semantics do not help: the attacker writes first and never has to overwrite.

Whether you are exposed depends only on who holds a token that can write:

- **Not exposed** when pull-request builds use `NX_CACHE_READ_TOKEN` (they can only read) and the
  read-write `NX_CACHE_TOKEN` is reachable only from builds of trusted branches. This is the
  layout of [`examples/github-actions/nx-ci.yml`](examples/github-actions/nx-ci.yml); on GitHub,
  fork pull requests get no secrets at all.
- **Exposed** when pull-request or otherwise untrusted builds hold the read-write token, when
  `NX_CACHE_ENABLE_LOGIN=false` or `NX_CACHE_ALLOW_ANONYMOUS=true` runs on a network those builds
  can reach, or when one token is shared by everyone "for simplicity".

What the server gives you to stay on the right side: read-only tokens; a 403 (not a silent
accept) when a read-only token tries to write; the uploading run recorded as object metadata
and shown in the dashboard for every hit, so a poisoned artifact can be traced to the run that
stored it; and with `?token=` never cached, no way to pick up a token from a URL. If untrusted
builds still need a writable cache among themselves, give them a separate deployment or
`store.prefix` with its own token, never the trusted one.

## Container and Kubernetes

The image is `FROM scratch`, runs as uid 65534 with no shell, and the manifests set
`runAsNonRoot`, `readOnlyRootFilesystem`, `allowPrivilegeEscalation: false`, all capabilities
dropped and the `RuntimeDefault` seccomp profile.

## Hardening summary

- **Cache API**: bearer tokens, compared in constant time. One read-write token for trusted
  branches, one read-only token for pull requests, so a fork cannot poison the cache.
- **Dashboard**: the shell is public, its API needs a token (`NX_CACHE_UI_TOKEN` or a cache token)
  entered once in the browser, or trusts an SSO proxy header. Keycloak/OIDC, SAML through
  Keycloak brokering, and Kong Gateway recipes: [`deploy/sso/`](deploy/sso/README.md).
- **Server**: hashes are validated (`[A-Za-z0-9_-]{1,128}`), uploads are size-capped and
  concurrency-capped, credentials never appear in logs or in `/api/v1/stats`. Responses carry
  `nosniff`, the dashboard a strict CSP, `Referrer-Policy: no-referrer` and `X-Frame-Options`.
- **Trusted header**: with `NX_CACHE_UI_TRUSTED_HEADER` set, any request carrying that header is
  let into the dashboard API. Expose the server only through the proxy that sets it, and make
  sure the proxy strips the header from client requests.
- **Unauthenticated by design**: `/metrics`, `/healthz`, `/readyz` (counters only, no hashes).
- **Tokens**: generate them with 32 random bytes (`openssl rand -hex 32`); the `?token=` form
  for the dashboard is read once and removed from the URL.
- **TLS** is terminated by the ingress; the server speaks plain HTTP (h1, h2c).
