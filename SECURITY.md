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

## Container and Kubernetes

The image is `FROM scratch`, runs as uid 65534 with no shell, and the manifests set
`runAsNonRoot`, `readOnlyRootFilesystem`, `allowPrivilegeEscalation: false`, all capabilities
dropped and the `RuntimeDefault` seccomp profile.
