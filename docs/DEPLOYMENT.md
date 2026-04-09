# Deployment

## Docker

### Multi-Stage Build

The Dockerfile (`docker/Dockerfile`) uses 4 stages:

1. **ui-builder** (node:22-alpine) — `npm ci && npm run build` → `ui/dist/` (admin UI)
2. **client-builder** (node:22-alpine) — `npm ci && npm run build` → `client/dist/` (player app)
3. **builder** (rust:1.88-bookworm) — copies UI dist, `cargo build --release` → binary
4. **runtime** (linuxserver/baseimage-debian:bookworm) — copies binary + both UIs, installs jellyfin-ffmpeg7 + unrar + p7zip-full, runs via s6-overlay

### s6-overlay Service

The runtime uses LinuxServer's base image with s6-overlay for:
- Process supervision (auto-restart on crash)
- PUID/PGID support (run as non-root user `abc`)
- Health check integration

Service files in `docker/root/etc/s6-overlay/s6-rc.d/svc-stackarr/`:
- `type`: `longrun`
- `run`: `s6-setuidgid abc /usr/local/bin/stackarr --config /config/stackarr.toml`
- `data/check`: `curl -sf http://localhost:9111/health`

### Docker Compose Files

| File | Purpose |
|------|---------|
| `docker/docker-compose.yml` | Standard deployment (stackarr + postgres + optional indexarr) |
| `docker/docker-compose.dev.yml` | Dev only — just PostgreSQL on port 5433 |
| `docker/docker-compose.prod.yml` | Production on Node B — read-only media mounts, memory limits |

### Production Compose (Node B)

```yaml
services:
  stackarr:
    image: ghcr.io/ausagentsmith-org/stackarr:latest
    ports: ["9111:9111"]
    environment:
      - PUID=1000
      - PGID=1000
      - STACKARR_DATABASE_URL=postgresql://stackarr:stackarr@postgres:5432/stackarr
      - TZ=Australia/Sydney
    volumes:
      - /mnt/2tnvme/docker/volumes/stackarr_config:/config
      - /mnt/data2/TV1:/media/TV1:ro          # Read-only media
      - /mnt/data1/TV2:/media/TV2:ro
      - /mnt/data3/TV3:/media/TV3:ro
      - /mnt/24T/movies/movies1:/media/Movies1:ro
      - /mnt/data1/movies2:/media/Movies2:ro
      - /mnt/data3/MoviesUHD:/media/MoviesUHD:ro
      - /mnt/4tnvme/docker/volumes/sabnzbd_downloads:/downloads/usenet:ro
      - /mnt/4tnvme/docker/volumes/rtbit_downloads:/downloads/torrent:ro
    deploy:
      resources:
        limits:
          memory: 4096m
    healthcheck:
      test: ["CMD", "curl", "-sf", "http://localhost:9111/health"]
      interval: 30s

  postgres:
    image: postgres:17-alpine
    volumes:
      - /mnt/2tnvme/docker/volumes/stackarr_pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U stackarr"]
```

Key production details:
- All media mounted **read-only** — StackArr reads, download clients handle writes
- Memory limited to 4GB
- Health check on `/health` endpoint
- Deploy path: `/mnt/2tnvme/docker/volumes/stackarr/`

### Indexarr Sidecar

Optional — enable with Docker profile:
```bash
docker compose --profile indexarr up -d
```

Provides integrated indexer functionality alongside StackArr.

---

## CI/CD Pipeline

### GitHub Actions (`.github/workflows/docker-deploy.yml`)

Triggers: push to `main`, pull requests, manual dispatch.

```
┌──────────┐     ┌───────────────────┐     ┌────────────────┐     ┌──────────┐
│  check   │────▶│ build-and-publish │────▶│ container-test │────▶│  deploy  │
│          │     │ (skip on PRs)     │     │                │     │(main only│
└──────────┘     └───────────────────┘     └────────────────┘     └──────────┘
```

#### Job: check
- Runs on: `[self-hosted, node-b]`
- Steps:
  1. Install system deps (build-essential, pkg-config, libssl-dev, cmake, gcc-12)
  2. Install Rust stable (clippy, rustfmt)
  3. Install Node.js 22
  4. `cargo check --workspace`
  5. `cargo test --workspace --lib`
  6. `cd ui && npm ci && npx tsc --noEmit`

#### Job: build-and-publish
- Needs: check
- Skips: pull requests
- Steps:
  1. Docker Buildx setup
  2. GHCR login
  3. Build + push with tags: `branch`, `sha`, `latest` (main only)
  4. GHA cache for Docker layers

#### Job: container-test
- Needs: build-and-publish
- Smoke test: pull image, run `stackarr --help`

#### Job: deploy
- Needs: build-and-publish + container-test
- Only: main branch
- Steps:
  1. GHCR login
  2. SSH setup (deploy key)
  3. Pull latest image
  4. SCP docker-compose.prod.yml to Node B
  5. `docker compose up -d --remove-orphans`
  6. Health check polling (12 attempts, 10s apart)
  7. Prune old images on success
  8. Show logs on failure

### Self-Hosted Runner

CI runs on a self-hosted GitHub Actions runner on **Node B** (192.168.1.75). Tagged as `[self-hosted, node-b]`.

Uses GCC 12 for compilation (CC/CXX env vars set).

### Container Registry

Images published to: `ghcr.io/ausagentsmith-org/stackarr`

Tags:
- `latest` — current main branch
- `main` — branch ref
- `<full-sha>` — exact commit

---

## Manual Deployment

```bash
# Build locally
cargo build --release
cd ui && npm ci && npm run build && cd ..

# Run
./target/release/stackarr --config /path/to/stackarr.toml

# Or with Docker
docker build -f docker/Dockerfile -t stackarr .
docker run -p 9111:9111 \
    -e STACKARR_DATABASE_URL=postgresql://... \
    -v /path/to/config:/config \
    -v /media:/media:ro \
    stackarr
```

## Network

| Service | Port | Purpose |
|---------|------|---------|
| StackArr (dev) | 8989 | API + UI |
| StackArr (prod) | 9111 | API + UI |
| PostgreSQL (dev) | 5433 | Database |
| PostgreSQL (prod) | 5432 (internal) | Database (docker network only) |
| Vite dev server | 3000 | Frontend HMR |
| Indexarr | 8080 | Indexer sidecar |
| Torrent engine | 6881 | DHT/peer connections |
| Bootstrap node | configurable | Remote access discovery |

## Volumes

| Volume | Path | Content |
|--------|------|---------|
| Config | `/config` | stackarr.toml, data files |
| PG Data | postgres volume | PostgreSQL data |
| Media (TV) | `/media/TV*` | TV series files (read-only in prod) |
| Media (Movies) | `/media/Movies*` | Movie files (read-only in prod) |
| Downloads | `/downloads/*` | Download client output (read-only in prod) |
