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
FROM ghcr.io/linuxserver/baseimage-debian:bookworm

COPY --from=builder /build/target/release/stackarr /usr/local/bin/stackarr
COPY --from=ui-builder /ui/dist /ui

# s6 service definition
COPY docker/root/ /

ENV STACKARR_UI_DIR=/ui
EXPOSE 9111

VOLUME /config
