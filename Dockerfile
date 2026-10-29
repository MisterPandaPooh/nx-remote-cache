# syntax=docker/dockerfile:1.7
# Three stages: build the dashboard, build a static Rust binary, ship it on `scratch`.

FROM node:24-alpine AS ui
WORKDIR /ui
RUN corepack enable
COPY ui/package.json ui/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY ui/ ./
RUN pnpm build

FROM rust:1.93-alpine AS build
RUN apk add --no-cache musl-dev build-base ca-certificates
WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs rustfmt.toml ./
COPY src ./src
COPY --from=ui /ui/dist ./ui/dist
# amd64 and arm64 build at the same time: share the registry under a lock, keep one target
# directory per platform so the two builds never unpack or link over each other.
ARG TARGETPLATFORM
RUN --mount=type=cache,id=cargo-registry,sharing=locked,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target-${TARGETPLATFORM},target=/src/target \
    cargo build --release --locked --bin nx-remote-cache --bin nx-cache-bench \
    && cp target/release/nx-remote-cache /nx-remote-cache \
    && cp target/release/nx-cache-bench /nx-cache-bench

# Load generator, same static build, on alpine so the benchmark can `exec` into a long-lived
# container (docker build --target bench -t nx-remote-cache:bench .)
FROM alpine:3.21 AS bench
RUN apk add --no-cache ca-certificates
COPY --from=build /nx-cache-bench /usr/local/bin/nx-cache-bench
USER 65534:65534
ENTRYPOINT ["nx-cache-bench"]

FROM scratch AS server
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=build /nx-remote-cache /nx-remote-cache
USER 65534:65534
ENV PORT=3000 NX_CACHE_BIND=0.0.0.0 NX_CACHE_LOG_FORMAT=json
EXPOSE 3000
ENTRYPOINT ["/nx-remote-cache"]
