# Stage 1: Build React UI
FROM node:22-alpine AS ui-builder
WORKDIR /ui
COPY ui/package*.json ./
RUN npm ci
COPY ui/ .
RUN npm run build

# Stage 2: Build Rust binary
FROM rust:1.88-bookworm AS builder
WORKDIR /build
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

# s6 service definition
COPY docker/root/ /

ENV STACKARR_UI_DIR=/ui
# Jellyfin-ffmpeg installs to /usr/lib/jellyfin-ffmpeg
ENV LIBVA_DRIVER_NAME=iHD
EXPOSE 9111

VOLUME /config
