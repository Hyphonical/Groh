# syntax=docker/dockerfile:1

# ── Frontend ────────────────────────────────────
FROM oven/bun:1-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package.json ./
COPY frontend/vite.config.js ./vite.config.js
COPY frontend/index.html ./index.html
COPY frontend/src ./src
COPY frontend/public ./public
RUN bun install && bun run build

# ── Builder ─────────────────────────────────────
FROM rust:alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY --from=frontend /app/frontend/build ./frontend/build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release && cp target/release/groh /groh

# ── Runtime ─────────────────────────────────────
FROM alpine:latest AS runtime
COPY --from=builder /groh /usr/local/bin/groh
USER 65534:65534
EXPOSE 3000
ENTRYPOINT ["groh"]
