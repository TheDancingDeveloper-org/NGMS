FROM rust:1.87-bookworm AS builder
WORKDIR /build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/stackarr /usr/local/bin/stackarr
COPY --from=builder /build/migrations /migrations
EXPOSE 8989
ENTRYPOINT ["stackarr"]
CMD ["--config", "/config/stackarr.toml"]
