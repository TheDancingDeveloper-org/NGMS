# StackArr — CLAUDE.md

StackArr is a unified media management server written in Rust that replaces Sonarr + Radarr + Prowlarr with a single binary. It manages TV series and movies with embedded torrent (librtbit) and usenet (nzb) download engines, an integrated indexer sidecar (Indexarr), Plex integration, and a React TypeScript frontend.

## Quick Reference

| Item | Value |
|------|-------|
| Language | Rust 2024 edition, TypeScript (React 19) |
| Build | `cargo build --release` (backend), `npm run build` in `ui/` (frontend) |
| Test | `cargo test --workspace --lib` |
| Dev DB | `docker compose -f docker/docker-compose.dev.yml up -d` (Postgres on :5433) |
| Dev UI | `cd ui && npm run dev` (Vite on :3000, proxies API to :8989) |
| Run | `cargo run -- --config stackarr.toml` |
| Lint | `cargo clippy --workspace`, `cd ui && npm run lint` |
| Port | 8989 (dev), 9111 (prod/Docker) |
| Database | PostgreSQL 17 (required). **Never use SQLite for application data.** |
| Config | TOML file + env vars (`STACKARR_*`) + CLI flags |
| Deploy | GitHub Actions → GHCR → Node B via SSH |

## Repository Layout

```
├── src/main.rs              # Binary entrypoint (CLI, init, server startup)
├── crates/
│   ├── stackarr-core/       # Config, DB, error types, models, migrations
│   ├── stackarr-web/        # Axum routes, middleware, AppState, SPA serving
│   ├── stackarr-media/      # Series/Movie/Episode CRUD services
│   ├── stackarr-parser/     # Release name → structured metadata parser
│   ├── stackarr-quality/    # Quality profiles, format scoring
│   ├── stackarr-indexer/    # Newznab/Torznab/Cardigann/Indexarr search clients
│   ├── stackarr-cardigann/  # Prowlarr-compatible YAML indexer definition engine
│   ├── stackarr-cardigann-parity/ # QA binary: Prowlarr parity testing
│   ├── stackarr-download/   # Download client trait + manager
│   ├── stackarr-import/     # Disk scan, file import, rename engine
│   ├── stackarr-scheduler/  # Background task scheduler
│   ├── stackarr-metadata/   # TMDB API client (cached + rate-limited)
│   ├── stackarr-notify/     # Webhook/Discord/Telegram/Slack/Email notifications
│   ├── stackarr-migrate/    # Sonarr/Radarr/Prowlarr SQLite → Postgres migration
│   ├── stackarr-plex/       # Plex API, scanner, watchlist sync
│   ├── stackarr-stream/     # Video streaming (direct play, HLS, ffmpeg transcode)
│   ├── stackarr-bootstrap/  # Standalone discovery node for remote access
│   ├── torrent/             # Vendored librtbit (12 crates, from rustTorrent)
│   └── usenet/              # Vendored nzb engine (5 crates, from rustnzbd)
├── ui/                      # React 19 + TypeScript + Tailwind v4 + TanStack Query
├── migrations/              # PostgreSQL migrations (sqlx)
├── docker/                  # Dockerfile, compose files, s6 service defs
└── .github/workflows/       # CI/CD pipeline
```

## Architecture at a Glance

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full system design.

```
┌─────────────────────────────────────────────────────────┐
│                    React SPA (ui/)                       │
│  TanStack Query → apiFetch() → /api/v1/*                │
└──────────────────────┬──────────────────────────────────┘
                       │ HTTP
┌──────────────────────▼──────────────────────────────────┐
│                  Axum Router (stackarr-web)              │
│  Routes: series, movies, queue, calendar, torrent, ...  │
│  State: Arc<AppState> (DB pool, config, engines)        │
├─────────────────────────────────────────────────────────┤
│              Service Layer                               │
│  SeriesService · MovieService · SearchService           │
│  ImportService · QualityProfileService · PlexScanner    │
├─────────────────────────────────────────────────────────┤
│  stackarr-core       │  stackarr-scheduler              │
│  Database, Config,   │  RSS sync, import scan,          │
│  Models, Errors      │  metadata refresh, Plex tasks    │
├──────────────────────┼──────────────────────────────────┤
│  Embedded Engines    │  External Integrations            │
│  librtbit (torrent)  │  TMDB, Newznab, Cardigann,       │
│  nzb-web (usenet)    │  Indexarr, Plex, Discord,        │
│  stackarr-stream     │  Webhooks, Telegram, Slack       │
├──────────────────────┴──────────────────────────────────┤
│                PostgreSQL 17 (sqlx)                      │
└─────────────────────────────────────────────────────────┘
```

## Critical Conventions

### Rust

- **Edition 2024**, resolver 3. All crates inherit workspace version/edition/lints.
- **Error handling**: `stackarr_core::Error` (thiserror) with variants: `NotFound`, `AlreadyExists`, `Validation`, `Config`, `Database`, `Io`, `Serialization`, `Http`, `DownloadClient`, `Indexer`, `Parse`, `Other(anyhow)`. Propagate with `?`. Use `anyhow::Context` for rich messages.
- **Async everywhere**: Tokio runtime, `async fn`, no blocking I/O on the main runtime. Use `spawn_blocking` for SQLite reads in migration.
- **Database**: sqlx with `query_as::<_, Model>()` and `FromRow` derives. Direct SQL, no ORM. Connection pool via `PgPool` shared in `AppState`.
- **Config hot-reload**: `Arc<ArcSwap<AppConfig>>` — config can be swapped atomically without restart.
- **API patterns**: Axum extractors (`State`, `Path`, `Query`, `Json`). Return `impl IntoResponse`. Match on `Result` variants for status codes.
- **Logging**: `tracing` crate. Use `tracing::info!`, `tracing::error!` etc. Structured fields preferred.
- **No `unwrap()` in production code.** Use `?` or explicit error handling.

### TypeScript / React

- **React 19** with strict TypeScript. TanStack React Query v5 for server state.
- **No Redux / Zustand** — server is source of truth, TanStack Query handles caching (30s staleTime).
- **Styling**: Tailwind CSS v4 utility classes. Dark theme with slate palette. No CSS modules.
- **API client**: `apiFetch<T>(path, options)` in `api/client.ts`. All types in `api/types.ts`.
- **Hooks**: All data fetching in `hooks/useApi.ts` using `useQuery` / `useMutation`.
- **Module gating**: Navigation items conditionally rendered based on `EnabledModules` from system status.

### Database

- **PostgreSQL only.** SQLite is used read-only for migration imports from *arr databases, and by the standalone `stackarr-bootstrap` binary for its own persistence (`server_names`, `pending_claims` tables).
- 4 migration files: `001_initial.sql`, `002_streaming.sql`, `003_health_check.sql`, `004_remote_access.sql`. Add new as `005_*.sql`, etc.
- JSONB columns for flexible data: `quality`, `languages`, `images`, `config`, `items`, `custom_data`.
- Array columns: `genres TEXT[]`, `tags INT[]`, `categories INT[]`.
- All timestamps are `TIMESTAMPTZ` (UTC).

## Sub-Documentation

| Document | Covers |
|----------|--------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, data flow, module relationships, concurrency model |
| [docs/CRATE-GUIDE.md](docs/CRATE-GUIDE.md) | Every crate's purpose, public API, dependencies, and usage |
| [docs/API-REFERENCE.md](docs/API-REFERENCE.md) | All REST endpoints, request/response shapes, error handling |
| [docs/DATABASE.md](docs/DATABASE.md) | Schema, models, migration patterns, query conventions |
| [docs/FRONTEND.md](docs/FRONTEND.md) | React architecture, components, state management, styling |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | TOML config, env vars, CLI args, feature flags, hot reload |
| [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) | Dev setup, testing, building, debugging, code style |
| [docs/DEPLOYMENT.md](docs/DEPLOYMENT.md) | Docker, CI/CD pipeline, production config, s6 services |
| [docs/DOMAIN-MODELS.md](docs/DOMAIN-MODELS.md) | Core types, enums, relationships, serialization |
| [docs/PARSER.md](docs/PARSER.md) | Release name parsing engine — quality, episodes, languages |
| [docs/DOWNLOAD-IMPORT.md](docs/DOWNLOAD-IMPORT.md) | Download client abstraction, import pipeline, file renaming |
| [docs/streaming.md](docs/streaming.md) | Video streaming architecture — HLS, ffmpeg, direct play |
