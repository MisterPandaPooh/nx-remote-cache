# Contributing

Thanks for helping. The project is small on purpose; please keep it that way.

## Setup

```bash
cargo build                      # server + bench tool (Rust 1.85+)
cd ui && pnpm install && pnpm dev   # dashboard against http://localhost:3000
```

Run the server locally without any cloud account:

```bash
NX_CACHE_STORE_URL=file:///tmp/nx-cache NX_CACHE_TOKEN=dev cargo run
```

## Checks before a pull request

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd ui && pnpm build
```

CI runs the same, plus a Docker build. The e2e Azure path can be exercised with Azurite
(`deploy/docker-compose.yml`), the comparative benchmark with `bench/run.sh`.

## Guidelines

- Never buffer a whole artifact in memory outside the documented inline threshold.
- Any new environment variable goes in `README.md` (Configuration) and `src/config.rs`.
- Answers to the Nx CLI must stay within the spec: `200/401/403/404/409`, plain-text messages that tell the developer what to fix.
- Keep the Docker image on `scratch`; no runtime dependencies.
