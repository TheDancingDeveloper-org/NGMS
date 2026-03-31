# Stage 1a: Build React UI (admin)
FROM node:22-alpine AS ui-builder
WORKDIR /ui
COPY ui/package*.json ./
ARG NPM_REGISTRY=https://registry.npmjs.org/
RUN --mount=type=cache,target=/root/.npm \
    npm ci --registry=${NPM_REGISTRY}
COPY ui/ .
RUN npm run build

# Stage 1b: Build React Client App (player)
FROM node:22-alpine AS client-builder
WORKDIR /client
COPY client/package*.json ./
ARG NPM_REGISTRY=https://registry.npmjs.org/
RUN --mount=type=cache,target=/root/.npm \
    npm ci --registry=${NPM_REGISTRY}
COPY client/ .
RUN npm run build

# Stage 2a: Generate dependency recipe (cargo-chef)
FROM rust:1.88-bookworm AS chef
RUN cargo install cargo-chef
# Configure git auth for private GitHub deps (nzb-* crates)
ARG GITHUB_TOKEN
RUN git config --global url."https://x-access-token:${GITHUB_TOKEN}@github.com/".insteadOf "https://github.com/"
WORKDIR /build

FROM chef AS planner
COPY . .
RUN sed -i '/^\[patch\./,/^$/d' Cargo.toml
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2b: Cook dependencies (cached layer — only rebuilds when Cargo.toml/Cargo.lock change)
FROM chef AS deps
COPY --from=planner /build/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json

# Stage 2c: Build application source (uses cached dependencies from above)
FROM deps AS builder
COPY . .
COPY --from=ui-builder /ui/dist /build/ui/dist
# Strip local [patch] overrides (they point to dev machine paths)
RUN sed -i '/^\[patch\./,/^$/d' Cargo.toml
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release

# Stage 3: Runtime (LinuxServer base for PUID/PGID support)
# LSIO base runs services as user `abc` via s6-overlay (non-root by default)
FROM ghcr.io/linuxserver/baseimage-debian:bookworm

LABEL org.opencontainers.image.source="https://github.com/AusAgentSmith-org/StackArr"
LABEL org.opencontainers.image.description="StackArr — unified media manager"

# Install jellyfin-ffmpeg7 (FFmpeg 7.1.x with full QSV/VAAPI/oneVPL + bundled Intel drivers)
# xz-utils needed if runtime ffmpeg download fallback is triggered
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt,sharing=locked \
    rm -f /etc/apt/apt.conf.d/docker-clean \
    && apt-get update \
    && apt-get install -y --no-install-recommends curl gnupg xz-utils \
    && curl -fsSL https://repo.jellyfin.org/jellyfin_team.gpg.key \
       | gpg --dearmor -o /usr/share/keyrings/jellyfin.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/jellyfin.gpg] https://repo.jellyfin.org/debian bookworm main" \
       > /etc/apt/sources.list.d/jellyfin.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends jellyfin-ffmpeg7 \
       p7zip-full \
    && apt-get purge -y gnupg \
    && apt-get autoremove -y

COPY --from=builder /build/target/release/stackarr /usr/local/bin/stackarr
COPY --from=ui-builder /ui/dist /ui
COPY --from=client-builder /client/dist /client

# s6 service definition
COPY docker/root/ /

ENV STACKARR_UI_DIR=/ui
ENV STACKARR_CLIENT_DIR=/client
# Jellyfin-ffmpeg installs to /usr/lib/jellyfin-ffmpeg
ENV LIBVA_DRIVER_NAME=iHD
EXPOSE 9111

VOLUME /config
