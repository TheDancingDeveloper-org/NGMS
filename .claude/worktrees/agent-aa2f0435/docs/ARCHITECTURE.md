# Architecture

## System Overview

StackArr is a monolithic Rust binary that embeds a web server, background scheduler, and optional download engines (torrent + usenet). It connects to PostgreSQL for persistence and serves a React SPA for the UI.

```
                          ┌──────────────┐
                          │  React SPA   │
                          │  (ui/dist)   │
                          └──────┬───────┘
                                 │ /api/v1/*
                          ┌──────▼───────┐
                          │  Axum Router │
                          │  (stackarr-  │
                          │   web)       │
                          └──────┬───────┘
                                 │
              ┌──────────────────┼──────────────────┐
              │                  │                  │
      ┌───────▼──────┐  ┌───────▼──────┐  ┌───────▼──────┐
      │   Services   │  │  Scheduler   │  │  Embedded    │
      │  (media,     │  │  (background │  │  Engines     │
      │   indexer,   │  │   tasks)     │  │  (torrent,   │
      │   quality)   │  │              │  │   usenet)    │
      └───────┬──────┘  └───────┬──────┘  └───────┬──────┘
              │                  │                  │
              └──────────────────┼──────────────────┘
                                 │
                          ┌──────▼───────┐
                          │  PostgreSQL  │
                          │  (sqlx pool) │
                          └──────────────┘
```

## Binary Startup Sequence

`src/main.rs` orchestrates initialization:

1. **CLI parsing** — clap with derive. Config path, bind, port, database-url, log-level, subcommands (`migrate`).
2. **Tracing init** — `tracing_subscriber` with env-filter, configured before anything else.
3. **Config load** — Read TOML file (generate default if missing). CLI/env override values.
4. **Database connect** — `PgPool` with configurable max connections. Run sqlx migrations.
5. **Module check** — Load `enabled_modules` from DB. First boot = no modules enabled yet.
6. **Engine init** — Conditionally start librtbit (torrent) and nzb-web (usenet) engines based on config.
7. **Build AppState** — `Arc<AppState>` containing DB pool, config (ArcSwap), modules, engine handles.
8. **Start scheduler** — Spawns background tasks as tokio tasks within a JoinSet.
9. **Start HTTP server** — Axum router with all routes, CORS, tracing middleware, SPA fallback.
10. **Graceful shutdown** — Ctrl+C handler drops scheduler handle, stopping all background tasks.

## AppState (Shared State)

```rust
pub struct AppState {
    pub db: Database,                                    // PgPool wrapper
    pub config: Arc<ArcSwap<AppConfig>>,                // Hot-reloadable config
    pub modules: EnabledModules,                         // Feature flags
    pub torrent_session: Option<Arc<librtbit::Session>>, // Torrent engine
    pub torrent_api: Option<librtbit::Api>,              // Torrent control API
    pub usenet_queue: Option<Arc<nzb_web::QueueManager>>,// Usenet engine
    pub indexarr_client: Option<Arc<IndexarrClient>>,    // Indexarr sidecar
    pub cardigann_engine: Arc<CardigannEngine>,           // Cardigann indexer defs
    pub indexer_manager: Arc<RwLock<IndexerManager>>,     // All indexer clients
    pub download_manager: Arc<RwLock<DownloadClientManager>>, // All download clients
    pub rate_limiter: Option<Arc<KeyedRateLimiter>>,      // IP-based rate limiting
    pub stream_session_manager: Option<Arc<SessionManager>>, // Video streaming
}
```

Passed to every Axum handler via `State<Arc<AppState>>`.

## Crate Dependency Graph

```
src/main.rs
├── stackarr-core         (config, db, models, errors)
├── stackarr-web          (routes, middleware)
│   ├── stackarr-media    (series/movie CRUD)
│   │   ├── stackarr-parser
│   │   └── stackarr-metadata
│   ├── stackarr-quality  (profiles, scoring)
│   │   └── stackarr-parser
│   ├── stackarr-indexer  (search, RSS)
│   │   ├── stackarr-parser
│   │   └── stackarr-cardigann
│   ├── stackarr-download (client management)
│   ├── stackarr-import   (disk scan, rename)
│   │   ├── stackarr-parser
│   │   ├── stackarr-media
│   │   └── stackarr-quality
│   ├── stackarr-notify   (webhooks, Discord)
│   ├── stackarr-migrate  (Sonarr/Radarr import)
│   │   └── stackarr-parser
│   ├── stackarr-plex     (Plex API, sync)
│   │   └── stackarr-metadata
│   └── stackarr-stream   (video streaming, HLS, ffmpeg)
├── stackarr-scheduler    (background tasks)
│   ├── stackarr-media
│   ├── stackarr-metadata
│   ├── stackarr-indexer
│   ├── stackarr-download
│   ├── stackarr-import
│   ├── stackarr-quality
│   └── stackarr-plex
├── stackarr-bootstrap    (standalone discovery node)
├── stackarr-cardigann-parity (QA tool: Prowlarr parity testing)
├── librtbit              (embedded torrent engine, 12 sub-crates)
└── nzb-web               (embedded usenet engine, 5 sub-crates)
```

## Request Flow

```
HTTP Request
  → Axum Router (path matching)
  → TraceLayer (request logging)
  → CorsLayer (credential support, standard methods)
  → Security Headers (X-Frame-Options, X-Content-Type-Options, X-XSS-Protection,
                      Referrer-Policy, Permissions-Policy)
  → Rate Limiter (50 req/sec per IP, if enabled)
  → Route Handler
    → Auth Extractor:
        RequireApiKey → validates X-Api-Key header, Authorization: Bearer, or ?apikey= query
        RequireAuth   → accepts API key OR remote client token (UUID)
    → Extract: State(state), Path(id), Query(params), Json(body)
    → Service method (e.g., SeriesService::list)
      → sqlx query against PgPool
      → Return Result<T>
    → Match Result:
        Ok(data)  → (StatusCode::OK, Json(data)).into_response()
        Err(e)    → (StatusCode::*, Json({"error": "..."})).into_response()
```

Public routes (`/health`, `/api/v1/system/status`, `/api/v1/setup/init`, `/api/v1/auth/status`, `/api/v1/auth/setup`) bypass authentication.

## Background Task Architecture

The `Scheduler` owns a `tokio::task::JoinSet` and spawns one task per background job. Each task runs an `interval` loop:

| Task | Interval | Module Gate | Description |
|------|----------|-------------|-------------|
| RSS Sync | 15 min | external_indexers / indexarr_sidecar | Poll indexer RSS feeds for new releases |
| Import Scan | 1 min | — (always) | Process completed downloads from queue |
| Metadata Refresh | 12 hrs | tv_management / movie_management | Fetch updated metadata from TMDB |
| Import List Sync | 1 hr | tv_management / movie_management | Sync items from external lists (Trakt, TMDB, etc.) |
| Disk Scan | 12 hrs | tv_management / movie_management | Scan media folders for new/changed files |
| Health Check | 5 min | — (always) | Validate indexer and download client connectivity |
| Plex Recent Scan | 5 min | plex_integration | Detect recently added Plex content |
| Plex Full Scan | 24 hrs | plex_integration | Full Plex library reconciliation |
| Plex Watchlist | 1 hr | plex_integration | Sync Plex watchlist items |
| Plex Token Refresh | 12 hrs | plex_integration | Refresh Plex auth tokens |
| Availability Sync | 24 hrs | plex_integration | Update content availability status |

Tasks are module-aware — only spawned if the corresponding module is enabled. Health checks track consecutive failures and can auto-disable unhealthy indexers/download clients.

## Module System

StackArr features are gated by `EnabledModules`:

```rust
pub struct EnabledModules {
    pub tv_management: bool,
    pub movie_management: bool,
    pub torrent_embedded: bool,
    pub usenet_embedded: bool,
    pub torrent_external: bool,
    pub usenet_external: bool,
    pub indexarr_sidecar: bool,
    pub external_indexers: bool,
    pub plex_integration: bool,
    pub notifications: bool,
    pub streaming: bool,
    pub remote_access: bool,
}
```

- Stored in DB (`enabled_modules` table), set during first-boot setup.
- Controls which scheduler tasks run, which engines initialize, which UI navigation items appear.
- First boot (`POST /api/v1/system/setup`) enables chosen modules and creates initial media library folders.

## Embedded Engines

### Torrent (librtbit)

Vendored from the rustTorrent project. When `config.torrent.enabled = true`:

- A `librtbit::Session` is created with configured listen port, DHT, peer limits, speed limits.
- The session runs torrent protocol in the background (DHT, peer exchange, piece downloading).
- `librtbit::Api` provides control: add torrent, pause, resume, delete, list, status.
- Exposed via `/api/v1/torrent/*` routes.

### Usenet (nzb-web)

Vendored from the rustnzbd project. When `config.usenet.enabled = true`:

- A `nzb_web::QueueManager` is created with configured NNTP servers, download dirs, concurrency.
- Handles NZB file downloading: article fetching, yEnc decoding, PAR2 repair, unrar.
- Exposed via `/api/v1/usenet/*` routes.

## Cardigann Indexer Engine

StackArr includes a Cardigann engine (`stackarr-cardigann`) compatible with Prowlarr's YAML indexer definitions. This allows adding indexers from a catalog of YAML definitions rather than requiring hardcoded client implementations.

- `CardigannEngine` loads YAML definitions from a directory, caching parsed definitions.
- `CardigannIndexer` executes searches by building URLs, fetching HTML/JSON, and parsing results via CSS selectors or JSON paths.
- The `/api/v1/indexer/available` endpoint lists all loaded Cardigann definitions.
- A separate QA binary (`stackarr-cardigann-parity`) validates search result parity against Prowlarr.

## Streaming Engine (stackarr-stream)

When `modules.streaming = true`, the streaming subsystem provides video playback:

- **Direct play** — serves media files via HTTP range requests (no transcoding).
- **Transcode** — spawns FFmpeg processes for HLS output when the client can't play the source format.
- **HLS** — generates M3U8 playlists and TS segments on demand.
- **Subtitles** — extracts embedded subtitle tracks as WebVTT.
- **Session management** — `SessionManager` (DashMap-backed) tracks active sessions, cleanup on timeout.
- Sessions are persisted in the `streaming_sessions` DB table.

## Auth & First-Boot Flow

StackArr uses a setup-based first-boot flow instead of auto-generated credentials:

1. **First boot**: `GET /api/v1/auth/status` returns `setupRequired: true`. The UI shows a setup screen.
2. **Admin creation**: `POST /api/v1/auth/setup` creates the first admin account (username, password, displayName). Rejects if any users exist.
3. **Invites**: Admins create invite codes. Users register via invite code + chosen credentials.
4. **Login**: `POST /api/v1/auth/login` accepts optional `deviceName` — if provided, returns a persistent `deviceToken` for mobile/Tauri clients.
5. **Device auth**: Subsequent requests use the device token as a Bearer token, avoiding re-login.

### Unified Invite + Claim Codes

When bootstrap is enabled, invite codes are auto-registered with the bootstrap discovery service:

- A single 8-char code handles both **server discovery** (bootstrap resolves server name/IP) and **account creation** (invite code for registration).
- **New user flow**: Enter 8-char code in client app -> bootstrap resolves server connection -> client redirects to register page with invite pre-filled.
- **Existing user, new device**: Look up server by name via bootstrap (`GET /api/v1/servers/by-name/{name}`), then log in with existing credentials. No admin involvement needed.

### Server Name Recovery

Server names registered with bootstrap are protected by a **BIP39 12-word mnemonic** recovery phrase. After a server rebuild, the admin can recover ownership of their server name via the recovery phrase, re-associating the name with the new server instance.

## Remote Access (stackarr-bootstrap)

The bootstrap system enables remote server discovery, client pairing, and server name resolution:

- A standalone `stackarr-bootstrap` binary acts as a discovery node with **SQLite persistence** (`server_names` + `pending_claims` tables).
- StackArr instances register a human-readable server name with the bootstrap service.
- Clients resolve server names to connection details via `GET /api/v1/servers/by-name/{name}`.
- Invite codes are auto-registered as unified claim codes when bootstrap is enabled.
- Server name ownership is protected by a BIP39 12-word recovery phrase.
- `RequireAuth` middleware accepts admin API key, session cookie, or device token.

## Data Flow: Search → Grab → Import

```
1. User/RSS triggers search
   → SearchService queries indexers (Newznab/Indexarr)
   → Returns Vec<ReleaseInfo>

2. Decision engine evaluates releases
   → Parser extracts quality/language from title
   → QualityProfile determines if release is wanted
   → Best release selected

3. Grab release
   → DownloadClientManager selects client by protocol
   → Adds torrent/NZB to embedded or external client
   → Creates queue record in DB
   → Creates history record (event: Grabbed)

4. Download completes (detected by scheduler poll)
   → ImportService::process_completed_download()
   → Scans output folder for media files
   → Parser extracts metadata from filenames
   → Naming engine builds destination path
   → File moved/renamed to library folder
   → MediaFile record created in DB
   → Episode/Movie linked to file
   → History record (event: Imported)

5. Notifications dispatched
   → NotificationService::notify() to all providers
```

## Concurrency Model

- **Tokio multi-threaded runtime** — CPU-bound work should be minimal; most operations are I/O-bound.
- **PgPool** — Connection pooling handles concurrent DB access. Default 20 connections.
- **ArcSwap** — Config can be atomically swapped without locks. Readers see consistent snapshots.
- **JoinSet** — Scheduler tasks are independent. One failing task doesn't crash others (errors are logged).
- **Arc sharing** — AppState, engine sessions, and clients are `Arc`-wrapped for cheap cloning across tasks.
- **spawn_blocking** — Used for SQLite reads (migration) and filesystem operations that may block.

## SPA Serving

The React UI is built to `ui/dist/` and served by Axum:
- Static files served from the dist directory (or embedded in Docker image at `/ui`).
- SPA fallback: any non-API route returns `index.html` for client-side routing.
- In development: Vite dev server on :3000 proxies `/api` to `:8989`.
