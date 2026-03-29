# Stage 1a: Build React UI (admin)
FROM node:22-alpine AS ui-builder
WORKDIR /ui
COPY ui/package*.json ./
RUN npm ci
COPY ui/ .
RUN npm run build

# Stage 1b: Build React Client App (player)
FROM node:22-alpine AS client-builder
WORKDIR /client
COPY client/package*.json ./
RUN npm ci
COPY client/ .
RUN npm run build

# Stage 2a: Generate dependency recipe (cargo-chef)
FROM rust:1.88-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2b: Cook dependencies (cached layer — only rebuilds when Cargo.toml/Cargo.lock change)
FROM chef AS deps
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Stage 2c: Build application source (uses cached dependencies from above)
FROM deps AS builder
COPY . .
COPY --from=ui-builder /ui/dist /build/ui/dist
RUN cargo build --release

# Stage 3: Runtime (LinuxServer base for PUID/PGID support)
# LSIO base runs services as user `abc` via s6-overlay (non-root by default)
FROM ghcr.io/linuxserver/baseimage-debian:bookworm

LABEL org.opencontainers.image.source="https://github.com/AusAgentSmith-org/StackArr"
LABEL org.opencontainers.image.description="StackArr — unified media manager"

# Install jellyfin-ffmpeg7 (FFmpeg 7.1.x with full QSV/VAAPI/oneVPL + bundled Intel drivers)
# xz-utils needed if runtime ffmpeg download fallback is triggered
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl gnupg xz-utils \
    && curl -fsSL https://repo.jellyfin.org/jellyfin_team.gpg.key \
       | gpg --dearmor -o /usr/share/keyrings/jellyfin.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/jellyfin.gpg] https://repo.jellyfin.org/debian bookworm main" \
       > /etc/apt/sources.list.d/jellyfin.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends jellyfin-ffmpeg7 \
    && apt-get purge -y gnupg \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

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
