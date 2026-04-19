# NGMS

**NGMS** is a self-hosted, unified media management server written in Rust. It replaces a full arr-stack (series manager + movie manager + indexer proxy) with a single binary that embeds torrent and usenet download engines, a Cardigann-compatible indexer engine, Plex integration, video streaming, and a React frontend.

---

## Features

- **Series & Movie Management** — Monitor, search, download, and organise your library automatically
- **Embedded Torrent Engine** — Full BitTorrent client (DHT, PEX, local service discovery, UPnP) — no external client required
- **Embedded Usenet Engine** — NNTP download client with direct-unpack, PAR2 repair, parallel connections, and multiple server support
- **Indexer Support** — Newznab, Torznab, and Cardigann (YAML definitions) indexers; Indexarr sidecar integration
- **Quality Profiles** — Flexible quality scoring, custom format tags, upgrade rules
- **Release Parser** — Structured metadata extraction from release names (quality, resolution, codec, audio, languages, episodes)
- **Plex Integration** — Library scanning, watchlist sync, activity monitoring
- **Video Streaming** — Direct play, HLS adaptive streaming, FFmpeg transcoding with hardware acceleration (Intel QSV, VAAPI, NVENC)
- **Notifications** — Discord, Telegram, Slack, email, and webhooks
- **Multi-User Auth** — Session-based login, API keys, device tokens, HTTP Basic Auth, role-based access, invite system
- **REST API** — Full JSON API with Swagger UI at `/swagger-ui/`
- **Remote Access** — Bootstrap relay for secure remote connections without port forwarding

---

## Quick Start

### Docker Compose (recommended)

```yaml
services:
  ngms:
    image: ghcr.io/ausagentsmith-org/ngms:latest
    container_name: ngms
    ports:
      - "9111:9111"
    volumes:
      - /path/to/config:/config
      - /path/to/media:/media
      - /path/to/downloads:/downloads
    environment:
      - PUID=1000
      - PGID=1000
      - TZ=UTC
      - STACKARR_DATABASE_URL=postgresql://ngms:ngms@postgres:5432/ngms
    depends_on:
      postgres:
        condition: service_healthy
    restart: unless-stopped

  postgres:
    image: postgres:17-alpine
    environment:
      POSTGRES_USER: ngms
      POSTGRES_PASSWORD: ngms
      POSTGRES_DB: ngms
    volumes:
      - ngms_pgdata:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ngms"]
      interval: 5s
      timeout: 5s
      retries: 10
    restart: unless-stopped

volumes:
  ngms_pgdata:
```

Then open `http://localhost:9111` to complete first-boot setup.

### Standalone (embedded PostgreSQL)

A standalone image with managed PostgreSQL is available — no external database required:

```bash
docker run -d \
  -p 9111:9111 \
  -v ngms-data:/config \
  ghcr.io/ausagentsmith-org/ngms:standalone
```

---

## Building from Source

### Prerequisites

- Rust 1.88+ (`rustup` recommended)
- Node.js 22+
- PostgreSQL 17
- `pkg-config`, `libssl-dev`, `cmake` (Debian/Ubuntu)

### Build

```bash
# Build the React frontend
cd ui && npm ci && npm run build && cd ..

# Build the Rust binary
cargo build --release

# The binary is at target/release/stackarr
```

### Run

```bash
# Start a dev Postgres instance
docker compose -f docker/docker-compose.dev.yml up -d

# Run with default config (auto-generated on first run)
./target/release/stackarr --config stackarr.toml
```

The UI dev server (with hot reload) proxies to the backend:

```bash
cd ui && npm run dev   # Vite on :3000, proxies API to :8989
```

---

## Configuration

NGMS is configured via a TOML file, environment variables, and CLI flags. A default config is generated on first startup.

```toml
[general]
instance_name = "NGMS"
bind_addr     = "0.0.0.0"
port          = 8989
data_dir      = "/config"
log_level     = "info"

[database]
url             = "postgresql://ngms:ngms@localhost:5432/ngms"
max_connections = 20

[auth]
method = "forms"     # "forms" | "basic" | "none"

[torrent]
enabled      = false
download_dir = "/downloads/torrent"
complete_dir = "/downloads/torrent-complete"
listen_port  = 6881
dht_enabled  = true

[usenet]
enabled          = false
incomplete_dir   = "/downloads/usenet/incomplete"
complete_dir     = "/downloads/usenet/complete"
max_active_downloads = 3
direct_unpack    = true

[[usenet.servers]]
name        = "Primary"
host        = "news.example.com"
port        = 563
ssl         = true
username    = "user"
password    = "pass"
connections = 20

[indexarr]
enabled = false
url     = "http://indexarr:8080"
api_key = ""
```

Environment variables override TOML values:

| Variable | Description |
|---|---|
| `STACKARR_DATABASE_URL` | PostgreSQL connection string |
| `STACKARR_PORT` | Listen port |
| `STACKARR_BIND` | Listen address |
| `STACKARR_CONFIG` | Path to TOML config file |
| `STACKARR_TMDB_API_KEY` | TMDB API key for metadata |

Full configuration reference: [docs/CONFIGURATION.md](docs/CONFIGURATION.md)

---

## Architecture

NGMS is a single Rust binary exposing an Axum HTTP server backed by PostgreSQL.

```
┌─────────────────────────────────────────────────────────┐
│                    React SPA (ui/)                       │
│  TanStack Query → apiFetch() → /api/v1/*                │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP
┌──────────────────────▼──────────────────────────────────┐
│                  Axum Router (stackarr-web)              │
│  Routes: series, movies, queue, calendar, torrent ...   │
│  State: Arc<AppState> (DB pool, config, engines)        │
├─────────────────────────────────────────────────────────┤
│              Service Layer                               │
│  SeriesService · MovieService · SearchService           │
│  ImportService · QualityProfileService · PlexScanner    │
├──────────────────────┬──────────────────────────────────┤
│  Background Scheduler│  Embedded Engines                │
│  RSS sync, import,   │  librtbit (torrent)              │
│  metadata refresh,   │  nzb-web (usenet)                │
│  Plex sync, health   │  stackarr-stream (video)         │
├──────────────────────┴──────────────────────────────────┤
│                PostgreSQL 17 (sqlx)                      │
└─────────────────────────────────────────────────────────┘
```

### Crate Layout

```
crates/
├── stackarr-core/       # Config, DB, error types, models, migrations
├── stackarr-web/        # Axum routes, middleware, AppState
├── stackarr-media/      # Series/Movie/Episode CRUD services
├── stackarr-parser/     # Release name → structured metadata
├── stackarr-quality/    # Quality profiles and format scoring
├── stackarr-indexer/    # Newznab/Torznab/Cardigann/Indexarr clients
├── stackarr-cardigann/  # Cardigann YAML indexer engine + bundled definitions
├── stackarr-download/   # Download client abstraction + manager
├── stackarr-import/     # Disk scan, file import, rename engine
├── stackarr-scheduler/  # Background task scheduler (15 tasks)
├── stackarr-metadata/   # TMDB API client (cached + rate-limited)
├── stackarr-notify/     # Webhook/Discord/Telegram/Slack/Email
├── stackarr-plex/       # Plex API, scanner, watchlist sync
├── stackarr-stream/     # HLS streaming, FFmpeg transcoding
├── stackarr-migrate/    # Import tool for legacy media manager databases
├── torrent/             # Vendored librtbit (BitTorrent engine, 12 crates)
└── usenet/              # Vendored nzb engine (usenet engine, 7 crates)
```

Full architecture reference: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

---

## API

The full REST API is documented interactively at `/swagger-ui/` and as an OpenAPI spec at `/api-docs/openapi.json`.

All endpoints are under `/api/v1/`. Authentication via:
- `X-Api-Key` header
- `Authorization: Bearer <key>`
- `?apikey=<key>` query parameter
- Session cookie (`stackarr_session`)

Full API reference: [docs/API-REFERENCE.md](docs/API-REFERENCE.md)

---

## Authentication

NGMS supports four authentication modes, configured via `auth.method`:

| Mode | Description |
|---|---|
| `forms` | Session-based login (default). Cookie + JSON token. |
| `basic` | HTTP Basic Auth. Browser native dialog. |
| `none` | No authentication (trusted network only). |

Additional features:
- **API keys** — single system-wide key compatible with Sonarr/Radarr clients
- **Device tokens** — long-lived tokens for mobile and desktop (Tauri) clients
- **Role-based access** — `admin` and `user` roles
- **Invite system** — admin-generated invite codes for new user registration
- **First-boot bypass** — all requests are admin until the first user is created

Full auth reference: [docs/AUTH.md](docs/AUTH.md)

---

## Database

NGMS requires **PostgreSQL 17**. SQLite is not supported for application data.

Migrations run automatically on startup. The schema covers:
- Series, seasons, episodes with monitoring state
- Movies with monitoring state
- Quality profiles and custom format definitions
- Indexers and download clients
- Download queue and history
- Users, sessions, device tokens, invites
- Scheduler task registry
- Application configuration (key/value store)

Full database reference: [docs/DATABASE.md](docs/DATABASE.md)

---

## Background Scheduler

15 background tasks run on configurable intervals:

| Task | Default interval | Description |
|---|---|---|
| `rss_sync` | 15 min | Search indexers for monitored releases |
| `import_scan` | 1 min | Scan download dirs for completed files |
| `metadata_refresh` | 12 h | Refresh series/movie metadata from TMDB |
| `missing_search` | 24 h | Search for monitored missing episodes/movies |
| `health_check` | 1 h | Check indexer and download client connectivity |
| `plex_scan` | 15 min | Trigger Plex library scan after imports |
| `plex_watchlist_sync` | 6 h | Sync Plex watchlist to monitored library |
| `cleanup` | 1 h | Remove expired sessions and old log entries |
| `recycle_bin_cleanup` | 24 h | Permanently delete old recycle bin items |
| … | | |

All tasks can be triggered manually via `POST /api/v1/system/tasks/{name}/trigger`.

Full scheduler reference: [docs/SCHEDULER.md](docs/SCHEDULER.md)

---

## Video Streaming

When `streaming.enabled = true`, NGMS can stream media directly to browsers and clients:

- **Direct play** — serves files as-is with range request support
- **HLS** — adaptive bitrate streaming with FFmpeg segmenter
- **Transcode** — real-time transcoding with hardware acceleration where available

Supported hardware acceleration: Intel QSV, VAAPI (Linux), NVIDIA NVENC.

FFmpeg/FFprobe binaries are downloaded automatically if not found on `$PATH`.

Full streaming reference: [docs/streaming.md](docs/streaming.md)

---

## Development

```bash
# Start dev database
docker compose -f docker/docker-compose.dev.yml up -d

# Run backend (port 8989)
cargo run -- --config stackarr.toml

# Run frontend dev server (port 3000, proxies to :8989)
cd ui && npm run dev

# Run tests
cargo test --workspace --lib

# Lint
cargo clippy --workspace -- -D warnings
cd ui && npm run lint
```

Full development guide: [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md)

---

## Documentation

| Document | Description |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, startup sequence, AppState, concurrency model |
| [docs/CRATE-GUIDE.md](docs/CRATE-GUIDE.md) | Per-crate purpose, public API, and dependencies |
| [docs/API-REFERENCE.md](docs/API-REFERENCE.md) | REST endpoints, request/response shapes, error codes |
| [docs/DATABASE.md](docs/DATABASE.md) | Schema, models, migration patterns, query conventions |
| [docs/FRONTEND.md](docs/FRONTEND.md) | React architecture, components, state management |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Full TOML reference, env vars, CLI flags |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Dev setup, testing, building, debugging |
| [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) | Docker, production config, s6 services |
| [docs/AUTH.md](docs/AUTH.md) | Auth methods, extractors, RBAC, invites |
| [docs/NOTIFICATIONS.md](docs/NOTIFICATIONS.md) | Events, providers, dispatch |
| [docs/EMBEDDED-ENGINES.md](docs/EMBEDDED-ENGINES.md) | Torrent and usenet engine lifecycle |
| [docs/PARSER.md](docs/PARSER.md) | Release name parsing engine |
| [docs/SCHEDULER.md](docs/SCHEDULER.md) | Background task scheduler |
| [docs/streaming.md](docs/streaming.md) | Video streaming architecture |
| [docs/DOWNLOAD-IMPORT.md](docs/DOWNLOAD-IMPORT.md) | Download client abstraction, import pipeline |
| [docs/DOMAIN-MODELS.md](docs/DOMAIN-MODELS.md) | Core types, enums, relationships |

---

## License

See [LICENSE](LICENSE).
