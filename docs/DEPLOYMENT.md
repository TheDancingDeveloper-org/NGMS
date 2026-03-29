# Deployment

## Docker

### Multi-Stage Build

The Dockerfile (`docker/Dockerfile`) uses 4 stages:

1. **ui-builder** (node:22-alpine) — `npm ci && npm run build` → `ui/dist/` (admin UI)
2. **client-builder** (node:22-alpine) — `npm ci && npm run build` → `client/dist/` (player app)
3. **builder** (rust:1.88-bookworm) — copies UI dist, `cargo build --release` → binary
4. **runtime** (linuxserver/baseimage-debian:bookworm) — copies binary + both UIs, installs jellyfin-ffmpeg7, runs via s6-overlay

### s6-overlay Service

The runtime uses LinuxServer's base image with s6-overlay for:
- Process supervision (auto-restart on crash)
- PUID/PGID support (run as non-root user `abc`)
- Health check integration

Service files in `docker/root/etc/s6-overlay/s6-rc.d/svc-ngms/`:
- `type`: `longrun`
- `run`: `s6-setuidgid abc /usr/local/bin/ngms --config /config/ngms.toml`
- `data/check`: `curl -sf http://localhost:9111/health`

### Docker Compose Files

| File | Purpose |
|------|---------|
| `docker/docker-compose.yml` | Standard deployment (ngms + postgres + optional indexarr) |
| `docker/docker-compose.dev.yml` | Dev only — just PostgreSQL on port 5433 |
| `docker/docker-compose.prod.yml` | Production on production server — read-only media mounts, memory limits |

### Production Compose (production server)

```yaml
services:
  ngms:
    image: ghcr.io/ausagentsmith/ngms:latest
    ports: ["9111:9111"]
    environment:
      - PUID=1000
      - PGID=1000
      - NGMS_DATABASE_URL=postgresql://ngms:ngms@postgres:5432/ngms
      - TZ=Australia/Sydney
    volumes:
      - ./config:/config
      - /path/to/tv1:/media/TV1:ro          # Read-only media
      - /path/to/tv2:/media/TV2:ro
      - /path/to/tv3:/media/TV3:ro
      - /path/to/movies1:/media/Movies1:ro
      - /path/to/movies2:/media/Movies2:ro
      - /path/to/moviesuhd:/media/MoviesUHD:ro
      - /path/to/usenet/downloads:/downloads/usenet:ro
      - /path/to/torrent/downloads:/downloads/torrent:ro
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
      - pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ngms"]
```

Key production details:
- All media mounted **read-only** — NGMS reads, download clients handle writes
- Memory limited to 4GB
- Health check on `/health` endpoint
- Deploy path: `/opt/ngms/`

### Indexarr Sidecar

Optional — enable with Docker profile:
```bash
docker compose --profile indexarr up -d
```

Provides integrated indexer functionality alongside NGMS.

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
- Runs on: `[self-hosted, linux]`
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
- Smoke test: pull image, run `ngms --help`

#### Job: deploy
- Needs: build-and-publish + container-test
- Only: main branch
- Steps:
  1. GHCR login
  2. SSH setup (deploy key)
  3. Pull latest image
  4. SCP docker-compose.prod.yml to production server
  5. `docker compose up -d --remove-orphans`
  6. Health check polling (12 attempts, 10s apart)
  7. Prune old images on success
  8. Show logs on failure

### Self-Hosted Runner

CI runs on a GitHub Actions runner. Self-hosted runners can be configured with the `[self-hosted]` label.

Uses GCC 12 for compilation (CC/CXX env vars set).

### Container Registry

Images published to: `ghcr.io/ausagentsmith/ngms`

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
./target/release/ngms --config /path/to/ngms.toml

# Or with Docker
docker build -f docker/Dockerfile -t ngms .
docker run -p 9111:9111 \
    -e NGMS_DATABASE_URL=postgresql://... \
    -v /path/to/config:/config \
    -v /media:/media:ro \
    ngms
```

## Network

| Service | Port | Purpose |
|---------|------|---------|
| NGMS (dev) | 8989 | API + UI |
| NGMS (prod) | 9111 | API + UI |
| PostgreSQL (dev) | 5433 | Database |
| PostgreSQL (prod) | 5432 (internal) | Database (docker network only) |
| Vite dev server | 3000 | Frontend HMR |
| Indexarr | 8080 | Indexer sidecar |
| Torrent engine | 6881 | DHT/peer connections |
| Bootstrap node | configurable | Remote access discovery |

## Volumes

| Volume | Path | Content |
|--------|------|---------|
| Config | `/config` | ngms.toml, data files |
| PG Data | postgres volume | PostgreSQL data |
| Media (TV) | `/media/TV*` | TV series files (read-only in prod) |
| Media (Movies) | `/media/Movies*` | Movie files (read-only in prod) |
| Downloads | `/downloads/*` | Download client output (read-only in prod) |
