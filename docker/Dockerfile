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

# Stage 3: Runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/stackarr /usr/local/bin/stackarr
COPY --from=builder /build/migrations /migrations
COPY --from=ui-builder /ui/dist /ui
ENV STACKARR_UI_DIR=/ui
EXPOSE 8989
ENTRYPOINT ["stackarr"]
CMD ["--config", "/config/stackarr.toml"]
