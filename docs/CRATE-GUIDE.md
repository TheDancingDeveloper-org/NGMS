# Crate Guide

Every crate in the workspace, what it does, and how to use it.

## StackArr Application Crates

### stackarr-core

**Purpose**: Foundation crate. Config loading, database connection, error types, and all domain model structs.

**Key exports**:
- `AppConfig` — TOML config struct with `load()` and `generate_default()`
- `Database` — PgPool wrapper with `connect()`, `run_migrations()`, `is_first_boot()`, `load_enabled_modules()`
- `Error` / `Result<T>` — Central error type (thiserror)
- All model structs: `Series`, `Episode`, `Movie`, `MediaFile`, `QueueItem`, `HistoryEvent`, etc.
- All enums: `MediaType`, `SeriesStatus`, `SeriesType`, `Quality`, `Language`, `DownloadProtocol`, etc.

**Features**: `testing` — exposes `test_helpers::TestDb` for integration tests.

**Dependencies**: sqlx, serde, toml, thiserror, anyhow, arc-swap, chrono, uuid

---

### stackarr-web

**Purpose**: HTTP API layer. Defines all Axum routes, builds the router, serves the SPA.

**Key exports**:
- `build_router(state: Arc<AppState>) -> Router`
- `run(addr, state)` — starts the HTTP server
- `AppState` struct

**Route modules** (in `src/routes/`):
- `health`, `system`, `series`, `movies`, `episodes`, `queue`, `history`, `calendar`, `wanted`
- `quality`, `indexers`, `downloadclients`, `naming`, `medialibraryfolders`, `tags`
- `torrent`, `usenet`, `releases`, `importlists`, `indexarr`, `discover`, `plex`

**Dependencies**: All other stackarr crates, axum, tower-http, utoipa (Swagger)

---

### stackarr-media

**Purpose**: CRUD services for series, movies, and episodes. The primary domain logic layer.

**Key types**:
- `SeriesService { pool }` — `list()`, `get(id)`, `create(input)`, `update(id, input)`, `delete(id)`
- `MovieService { pool }` — same pattern
- `ImportListService { pool }` — CRUD + `sync_list(id)`
- Input types: `CreateSeriesInput`, `UpdateSeriesInput`, `CreateMovieInput`, etc.

**Behavior**:
- Auto-computes `clean_title` via `stackarr_parser::clean_title()` on create/update.
- Preserves all external IDs (TVDB, TMDB, IMDB, TVMaze, MAL).
- Cascading deletes for seasons, episodes, files when series/movie is deleted.

**Dependencies**: stackarr-parser, stackarr-metadata, sqlx

---

### stackarr-parser

**Purpose**: Parses release names (e.g., `"Show.Name.S01E02.720p.HDTV.x264-GROUP"`) into structured metadata.

**Key exports**:
- `parse_release(name: &str) -> ParsedRelease` — main entry point
- `clean_title(s: &str) -> String` — normalize title for comparison
- `Quality` enum (24 variants), `QualityModel`, `Revision`
- `Language` enum (27 variants)
- `EpisodeInfo` — season, episodes, absolute numbers, air dates

**ParsedRelease fields**:
- `title`, `quality`, `episode_info`, `languages`, `release_group`, `release_hash`, `year`, `edition`, `imdb_id`

**Supports**: Standard (S01E02), multi-episode (S01E01-E05), daily (2024.01.15), anime (absolute), full season, multi-season, scene group detection, repack/proper/real revisions.

**Dependencies**: regex, serde, chrono (no database, no async — pure parsing)

---

### stackarr-quality

**Purpose**: Quality profile management. Determines whether a release meets quality requirements and if it's an upgrade.

**Key types**:
- `QualityProfileService { pool }` — CRUD for quality profiles
- `QualityProfile` — name, cutoff quality, upgrade_allowed, min_format_score, items (JSONB)
- `CustomFormat` — reusable format specifications for scoring

**Logic**: Evaluates a release's quality against a profile's allowed qualities and cutoff. Format scoring adds/subtracts points based on custom format rules.

**Dependencies**: stackarr-parser, sqlx, regex

---

### stackarr-indexer

**Purpose**: Search indexers (Newznab/Torznab/Indexarr) for releases.

**Key exports**:
- `IndexerManager` — manages multiple indexer clients
- `SearchService` — unified search across all indexers
- `NewznabClient` — standard Newznab API client
- `IndexarrClient` — StackArr's sidecar indexer client
- `ReleaseInfo` — search result with download URL, size, quality, age, seeders, etc.
- `TvSearchCriteria`, `MovieSearchCriteria` — typed search parameters

**Behavior**: Parallel search across all enabled indexers. Parses XML responses (Newznab) or JSON (Indexarr). Supports RSS feed polling for new releases.

**Dependencies**: stackarr-parser, sqlx, reqwest, quick-xml, futures

---

### stackarr-download

**Purpose**: Abstraction layer for download clients (embedded and external).

**Key exports**:
- `DownloadClient` trait — `add()`, `get_items()`, `remove()`, `pause()`, `resume()`, `test()`, `status()`
- `DownloadClientManager` — manages multiple clients, selects by protocol/priority
- `GrabRequest` — what to download (URL, title, category)
- `DownloadItem` — current download status (progress, speed, ETA)

**Supported clients**:
- Embedded: librtbit (torrent), nzb-web (usenet)
- External: Transmission, qBittorrent, SABnzbd, NZBGet

**Dependencies**: librtbit, nzb-core, nzb-web, reqwest, async-trait

---

### stackarr-import

**Purpose**: Disk scanning, file import, and filename renaming.

**Key exports**:
- `process_completed_download(ctx: ImportContext) -> Result<ImportResult>` — import a completed download
- `disk_scan(pool, root_path, media_type) -> Result<DiskScanResult>` — scan library folder
- `ImportService` — legacy service wrapper
- `LocalFile` — discovered file (path, size, extension)
- `ImportResult` — imported files, skipped, errors
- `DiskScanResult` — found, matched, unmatched, already tracked

**Naming engine** (in `naming.rs`):
- `build_episode_filename(format, ...)` — tokens: `{Series Title}`, `{season:00}`, `{episode:00}`, `{Episode Title}`, `{Quality Title}`, `{Release Group}`
- `build_movie_filename(format, ...)` — tokens: `{Movie Title}`, `{Release Year}`, `{Quality Title}`, `{Edition Tags}`
- `sanitize_filename(name, colon_replacement)` — removes illegal chars, handles colons (smart/dash/space)

**Media extensions**: mkv, mp4, avi, wmv, ts, m4v, flv, mov, webm
**Sample detection**: files < 50MB with "sample" in name

**Dependencies**: stackarr-parser, stackarr-media, stackarr-quality, sqlx, walkdir, tokio

---

### stackarr-scheduler

**Purpose**: Background task orchestrator. Spawns periodic tasks in a JoinSet.

**Key type**: `Scheduler` — configured with intervals, calls `start()` to spawn all tasks.

**Tasks**: RSS sync (15m), import scan (1m), metadata refresh (12h), import list sync (1h), Plex tasks (various).

**Pattern**: Each task runs `interval(duration)` loop, logs errors but continues running. Module-aware — only spawns relevant tasks.

**Dependencies**: All service crates (media, metadata, indexer, download, import, quality, plex), sqlx, tokio

---

### stackarr-metadata

**Purpose**: TMDB (The Movie Database) API client.

**Key type**: `TmdbClient { api_key }` with methods:
- `search_series(query, year)`, `search_movie(query, year)`
- `get_series(tmdb_id)`, `get_movie(tmdb_id)`, `get_season(series_id, season_num)`
- `get_trending(media_type, time_window, page, language)`
- `discover_movies(filters)`, `discover_tv(filters)`
- `get_movie_recommendations(id)`, `get_tv_recommendations(id)`
- `get_movie_genres()`, `get_tv_genres()`, `get_languages()`

**Types**: `TmdbSearchResults<T>`, `TmdbSeriesDetail`, `TmdbMovieDetail`, `TmdbSeason`, `TmdbEpisode`, `TmdbTrendingItem`, `DiscoverFilters`

**Dependencies**: reqwest, serde, chrono

---

### stackarr-notify

**Purpose**: Event-driven notification dispatch.

**Key exports**:
- `NotificationEvent` enum — `Grab`, `Import`, `Upgrade`, `HealthIssue`, `DownloadFailure`
- `NotificationProvider` trait — `name()`, `send(event)`, `test()`
- `NotificationService` — fan-out to all providers (errors don't stop dispatch)
- Providers: `WebhookProvider` (JSON POST), `DiscordProvider` (webhook)

**Dependencies**: reqwest, async-trait, serde

---

### stackarr-migrate

**Purpose**: Import data from Sonarr/Radarr/Prowlarr SQLite databases into StackArr's PostgreSQL.

**Usage**: `stackarr migrate --sonarr path.db --radarr path.db --prowlarr path.db [--dry-run]`

**Flow**: Read SQLite (rusqlite, spawn_blocking) → merge/deduplicate → write to Postgres (or report counts in dry-run).

**Output**: `MigrationReport` — counts of imported items + warnings.

**Dependencies**: stackarr-parser, sqlx, rusqlite, serde, chrono, anyhow

---

### stackarr-plex

**Purpose**: Plex Media Server integration.

**Key exports**:
- `PlexApi` — HTTP client for Plex XML API
- `PlexTvApi` — plex.tv cloud API
- `PlexScanner` — scans and syncs Plex library to StackArr
- `WatchlistSync` — syncs Plex user watchlist to discover
- `TokenRefresh` — maintains auth tokens
- `AvailabilitySync` — updates movie availability status

**Dependencies**: stackarr-metadata, sqlx, reqwest, regex

---

## Vendored Engine Crates

### Torrent Engine (12 crates in `crates/torrent/`)

Vendored from the rustTorrent project. Key crate: **librtbit** — full BitTorrent client library.

Sub-crates: bencode, buffers, clone_to_owned, dht, librtbit-core, librtbit-lsd, peer_binary_protocol, sha1w, tracker_comms, upnp, upnp-serve.

### Usenet Engine (5 crates in `crates/usenet/`)

Vendored from the rustnzbd project. Key crate: **nzb-web** — NZB download queue manager.

Sub-crates: nzb-core (domain types), nzb-decode (yEnc), nzb-nntp (NNTP protocol), nzb-postproc (PAR2/unrar).
