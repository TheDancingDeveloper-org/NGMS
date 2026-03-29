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
- `health`, `system`, `auth`, `admin`, `series`, `movies`, `episodes`, `queue`, `history`, `calendar`, `wanted`
- `quality`, `indexers`, `downloadclients`, `naming`, `medialibraryfolders`, `tags`
- `torrent`, `usenet`, `releases`, `importlists`, `indexarr`, `discover`, `plex`
- `stream`, `remote`, `search`, `blocklist`, `backup`, `logs`
- `images` — TMDB/TVDB image proxy with SHA-256 disk cache, SSRF domain allowlist, and `Cache-Control` headers
- `general` — General configuration endpoints (instance name, auth method, grab strategy) and bootstrap config
- `mediamanagement` — Media management config (recycle bin path/cleanup days) and recycle bin CRUD (list, delete, empty)
- `user` — User profile updates (display name, avatar, password change), device listing/deletion, session listing/revocation
- `progress` — Watch progress tracking: continue-watching feed, per-file/per-series/per-movie progress, upsert/delete
- `requests` — Media request CRUD with approve/decline workflow, pending count, library duplicate detection
- `watchlist` — User watchlist add/remove/list (filtered by media type) and per-item star ratings CRUD
- `notifications` — User notification list (unread filter, pagination), mark read/read-all, unread count, push subscription management
- `activities` — System activity log (list with limit, running count)
- `bootstrap` — Bootstrap admin endpoints: register/recover server name, registration status, check-name availability, check-port reachability

**Middleware** (`middleware.rs`):
- `RequireApiKey` — validates admin API key from header/bearer/query param
- `RequireAuth` — accepts admin API key, session cookie, OR device token (UUID)
- `RequireUser` — resolves a logged-in user from session cookie, device token (UUID), or legacy API key; first-boot bypass when no users exist
- `RequireAdmin` — wraps `RequireUser` and returns 403 if the user's role is not `admin`
- `RateLimit` — IP-based rate limiting (50 req/sec)
- `create_rate_limiter(per_second: u32) -> Arc<KeyedRateLimiter>` — constructs an IP-keyed governor rate limiter
- `client_ip(parts: &Parts) -> IpAddr` — extracts client IP from `X-Forwarded-For`, `X-Real-IP`, or falls back to localhost
- `mask_secret()` / `redact_sensitive_fields()` — sensitive field redaction in logs

**Dependencies**: All other stackarr crates, axum, tower-http, utoipa (OpenAPI)

---

### stackarr-media

**Purpose**: CRUD services for series, movies, and episodes. The primary domain logic layer.

**Key types**:
- `SeriesService { pool }` — `list()`, `get(id)`, `create(input)`, `update(id, input)`, `delete(id)`
- `MovieService { pool }` — same pattern
- `EpisodeService { pool }` — `list_by_series(id)`, `get(id)`, `create(input)`, `set_monitored(id, bool)`, `update_monitored(id, bool)`, `set_season_monitored(series_id, season, bool)`, `set_bulk_monitored(ids, bool)`
- `CalendarService { pool }` — `get_calendar(start, end) -> Vec<CalendarEntry>`. Returns upcoming episodes between two dates joined with series data (title, monitored, has_file).
- `WantedService { pool }` — `missing(page, page_size) -> WantedPage` (monitored episodes/movies without a file, aired in the past) and `cutoff_unmet(page, page_size) -> WantedPage` (items with files below quality profile cutoff). Both return paginated results combining series episodes and movies via UNION queries.
- `MetadataRefreshService { pool }` — `find_stale_series()` / `find_stale_movies()` (items not synced in 12+ hours), `mark_series_synced(id)` / `mark_movie_synced(id)`, `update_series_metadata(id, ...)` / `update_movie_metadata(id, ...)` for TMDB-sourced refreshes.
- `ImportListService { pool }` — CRUD + `sync_list(id)`
- Input types: `CreateSeriesInput`, `UpdateSeriesInput`, `CreateMovieInput`, `CreateEpisodeInput`, etc.
- Result types: `CalendarEntry`, `WantedPage`, `WantedRecord`

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
- `EpisodeInfo` — season, episodes, absolute numbers, air dates. Internal type (not re-exported from `lib.rs`), accessible via `ParsedRelease.episode_info` field.

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
- `DecisionEngine` — evaluates releases against quality profiles using a chain of `DecisionSpecification` implementations. `new()` creates the default spec chain; `decide(context) -> DownloadDecision` returns approved/rejected with reasons.
- `GrabStrategy` enum — `BestQuality` (default: quality first, indexer priority as tiebreaker) or `IndexerPriority` (indexer priority first, then quality)
- `rank_releases(decisions, strategy) -> Vec<DownloadDecision>` — sorts approved decisions by the chosen strategy (quality/CF score/seeders/age/indexer priority)
- `quality_name(num: i32) -> &'static str` — human-readable label for a quality discriminant number (e.g., 11 -> "WEBDL-1080p")
- `parser_quality_to_num(q: Quality) -> i32` — maps parser `Quality` enum to core model discriminant
- `is_quality_allowed(quality_num, profile) -> bool` — checks if a quality is allowed in a profile (handles nested groups)

**Decision specifications** (implement `DecisionSpecification` trait):
- `QualityAllowedSpec` — rejects releases not allowed in the profile
- `QualityCutoffSpec` — rejects when cutoff is met or release is not an upgrade
- `MinimumSizeSpec` / `MaximumSizeSpec` — enforces per-tier size limits
- `BlocklistSpec` — rejects blocklisted releases
- `QueueConflictSpec` — rejects duplicates or when queue has equal/higher quality
- `LanguageSpec` — rejects releases not matching profile language (supports Any/-1, Original/-2, specific Radarr language IDs)
- `MinimumSeedersSpec` — rejects torrents with zero seeders
- `CustomFormatScoreSpec` — rejects releases below min_format_score
- `CustomFormatCutoffSpec` — rejects when existing file's CF score meets the cutoff
- `AlreadyImportedSpec` — rejects previously grabbed/imported releases

**Logic**: The `DecisionEngine` runs all specs in order, collecting rejections. A release is approved only if no spec rejects it. `rank_releases()` then sorts approved decisions by the configured `GrabStrategy`.

**Dependencies**: stackarr-parser, stackarr-core, sqlx, serde, governor

---

### stackarr-indexer

**Purpose**: Search indexers (Newznab/Torznab/Indexarr) for releases.

**Key exports**:
- `IndexerManager` — manages multiple indexer clients (Newznab, Indexarr, Cardigann)
- `SearchService` — unified search across all indexers
- `NewznabClient` — standard Newznab/Torznab XML API client
- `IndexarrClient` — StackArr's sidecar indexer client
- `ReleaseInfo` — search result with download URL, size, quality, age, seeders, etc.
- `TvSearchCriteria`, `MovieSearchCriteria`, `TextSearchCriteria` — typed search parameters
- `RestSearchFilters` — query-param-based search filters

**Behavior**: Parallel search across all enabled indexers. Parses XML responses (Newznab) or JSON (Indexarr). Cardigann indexers are built dynamically from YAML definitions via `stackarr-cardigann`. Supports RSS feed polling for new releases.

**Dependencies**: stackarr-parser, stackarr-cardigann, sqlx, reqwest, quick-xml, futures

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

**Tasks and intervals**:
- RSS sync: 15m
- Import scan: 1m
- Metadata refresh: 12h
- Import list sync: 1h
- Scheduled disk scan: 12h
- Plex recent scan: 5m (Plex module only)
- Plex full library scan: 24h (Plex module only)
- Plex watchlist sync: 1h (Plex module only)
- Plex token refresh: 12h (Plex module only)
- Availability sync: 24h (Plex module only)
- Health check: 5m (requires download + indexer managers)
- Recycle bin cleanup: 6h
- Activity cleanup: daily (prunes activities older than 7 days)
- Notification cleanup: daily (prunes notifications older than 30 days)

**Pattern**: Each task runs `interval(duration)` loop, logs errors but continues running. Module-aware — only spawns Plex tasks when `plex_integration` is enabled; skips all core tasks on first boot (no enabled modules).

**Dependencies**: All service crates (media, metadata, indexer, download, import, quality, plex), sqlx, tokio

---

### stackarr-metadata

**Purpose**: TMDB (The Movie Database) API client.

**Key type**: `TmdbClient` with methods:
- `search_series(query, year)`, `search_movie(query, year)`
- `get_series(tmdb_id)`, `get_movie(tmdb_id)`, `get_season(series_id, season_num)`
- `get_trending(media_type, time_window, page, language)`
- `discover_movies(filters)`, `discover_tv(filters)`
- `get_movie_recommendations(id)`, `get_tv_recommendations(id)`
- `get_movie_similar(id)`, `get_tv_similar(id)`
- `get_movie_genres()`, `get_tv_genres()`, `get_languages()`
- `get_keyword(id)`, `get_keyword_movies(id)`

**Caching & rate limiting**:
- Leaky-bucket rate limiter (4 requests/second to TMDB)
- LRU cache (2000 entries) with TTL: 1 hour for searches, 24 hours for detail lookups
- Cache is in-memory (parking_lot mutex)

**Types**: `TmdbSearchResults<T>`, `TmdbSeriesDetail`, `TmdbMovieDetail`, `TmdbSeason`, `TmdbEpisode`, `TmdbTrendingItem`, `DiscoverFilters`

**Dependencies**: reqwest, serde, chrono, leaky-bucket, lru, parking_lot

---

### stackarr-notify

**Purpose**: Event-driven notification dispatch.

**Key exports**:
- `NotificationEvent` enum — `Grab`, `Import`, `Upgrade`, `HealthIssue`, `DownloadFailure`
- `NotificationProvider` trait — `name()`, `send(event)`, `test()`
- `NotificationService` — fan-out to all providers (errors don't stop dispatch)
- Providers: `WebhookProvider` (JSON POST), `DiscordProvider` (webhook), `TelegramProvider` (bot API), `SlackProvider` (webhook), `EmailProvider` (SMTP)

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

### stackarr-stream

**Purpose**: Video streaming server with direct play, HLS transcoding, and subtitle extraction.

**Key exports**:
- `SessionManager` — manages active streaming sessions (DashMap-backed)
- `StreamError` / `StreamResult` — error handling
- Modules: `direct` (HTTP range serving), `ffmpeg` (transcode process), `ffprobe` (media analysis), `hls` (M3U8/TS generation), `subtitle` (WebVTT extraction), `session`, `types`

**Behavior**:
- Direct play serves files with HTTP range requests for compatible codecs.
- Transcoding spawns FFmpeg processes that output HLS segments.
- ffprobe analyzes media files to determine codec/format capabilities.
- Sessions are tracked in-memory (DashMap) and persisted to `streaming_sessions` table.

**Dependencies**: stackarr-core, tokio, tokio-util, sqlx, uuid, dashmap, bytes, mime_guess

---

### stackarr-cardigann

**Purpose**: Prowlarr-compatible YAML indexer definition engine. Interprets YAML definition files to execute searches against indexer sites.

**Key exports**:
- `CardigannEngine` — loads and caches YAML definitions from a directory
- `CardigannDefinition` — parsed indexer definition model
- `CardigannIndexer` — executes searches (builds URLs, fetches HTML/JSON, parses via CSS selectors or JSON paths)
- Modules: `categories`, `definition`, `filters`, `search`, `selector`, `template`

**Behavior**: Reads YAML definitions that describe how to search an indexer (URL patterns, login flows, result selectors). Supports both HTML scraping (CSS selectors) and JSON API parsing.

**Dependencies**: serde_yaml, scraper, indexmap, regex, reqwest, chrono, url (no stackarr deps)

---

### stackarr-cardigann-parity

**Purpose**: QA/testing binary for validating Cardigann engine compatibility with Prowlarr.

**Key functions**:
- Fetch YAML definitions from a running Prowlarr instance
- Validate definition parsing against the Cardigann engine
- Provision test indexers and compare search results for parity

**Dependencies**: stackarr-cardigann, stackarr-indexer, reqwest, tokio

---

### stackarr-bootstrap

**Purpose**: Standalone discovery node for remote server-client pairing, server name resolution, and unified invite/claim code management. Runs as a separate binary.

**Key features**:
- Independent Axum HTTP server with its own state (`BootstrapState`)
- **Server name registration and resolution** — human-readable names mapped to connection details (`GET /api/v1/servers/by-name/{name}`)
- **BIP39 recovery phrases** — 12-word mnemonic protects server name ownership; used for recovery after rebuild
- **Unified claim codes** — accepts server-provided 8-char codes with `claimType` and `inviteCode` metadata, unifying server discovery and account creation into a single code
- **SQLite persistence** — `db.rs` module with rusqlite; `server_names` and `pending_claims` tables replace the previous in-memory DashMap storage
- Used by StackArr instances to enable remote access via the bootstrap protocol

**Key modules**:
- `db.rs` — SQLite persistence layer (rusqlite): `init_db()`, `register_server_name()`, `resolve_name()`, `store_claim()`, `redeem_claim()`

**Dependencies**: axum, tokio, clap, serde_json, rusqlite, uuid, toml, bip39 (no stackarr deps)

---

## Vendored Engine Crates

### Torrent Engine (12 crates in `crates/torrent/`)

Vendored from the rustTorrent project. Key crate: **librtbit** — full BitTorrent client library.

Sub-crates: bencode, buffers, clone_to_owned, dht, librtbit-core, librtbit-lsd, peer_binary_protocol, sha1w, tracker_comms, upnp, upnp-serve.

### Usenet Engine (5 crates in `crates/usenet/`)

Vendored from the rustnzbd project. Key crate: **nzb-web** — NZB download queue manager.

Sub-crates: nzb-core (domain types), nzb-decode (yEnc), nzb-nntp (NNTP protocol), nzb-postproc (PAR2/unrar).
