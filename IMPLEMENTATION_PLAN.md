# StackArr — Implementation Plan

## Current State

The workspace compiles clean (0 errors, 4 minor clippy warnings) with 75 parser tests passing. The binary builds but **cannot actually start** — config loading panics without a file, migrations reference a path that won't resolve, and the DB URL has no CLI/env override.

### Crate Status

| Crate | % Done | State |
|-------|--------|-------|
| stackarr-core | 95% | Real — models, config, DB pool, error types |
| stackarr-parser | 98% | Real — 75 tests, release name parsing complete |
| stackarr-media | 95% | Real — Series/Movie/Episode CRUD with SQL queries |
| stackarr-web | 90% | Real routes, but search/grab/setup/health are no-ops |
| stackarr-download | 85% | Real — qBit, Transmission, SABnzbd, NZBGet clients |
| stackarr-indexer | 85% | Real Newznab/Torznab XML client; Indexarr sidecar stubbed |
| stackarr-metadata | 85% | Real TMDB client, no caching or rate limiting |
| stackarr-notify | 80% | Webhook + Discord real; Telegram/Slack/email missing |
| stackarr-quality | 50% | CRUD works; decision engine specs all return "allow" |
| stackarr-import | 45% | File scanning works; import pipeline is a no-op |
| stackarr-scheduler | 40% | Task framework works; all 3 tasks are no-op stubs |
| main.rs | 95% | CLI + startup, but panics without config file |

### Design Philosophy

**Media manager first, download client second.** Before a user downloads anything, they should be able to:
1. Import their existing Sonarr/Radarr/Prowlarr configuration
2. Scan their existing library and match files to media
3. See their complete library state — what they have, what's missing, what quality

---

## Phase 0 — Make It Boot

**Goal**: `docker compose up` starts StackArr + Postgres, API responds to curl.

**Worktree**: `phase0-bootable`

**Files to change**:
- `src/main.rs` — Add `--database-url` / `STACKARR_DATABASE_URL` env, generate default config if missing, fix migration path
- `crates/stackarr-core/src/config.rs` — Add `AppConfig::default()` and `AppConfig::generate_default(path)`
- `crates/stackarr-core/src/db.rs` — Fix `sqlx::migrate!()` path (use runtime migrations or correct relative path)
- `docker/Dockerfile` — Multi-stage build (Rust builder → runtime with binary)
- `docker/docker-compose.yml` — StackArr + Postgres 17, volumes, health check
- `config.example.toml` — Documented example config

**Acceptance test**:
```bash
docker compose up -d
curl http://localhost:8989/health              # → 200
curl http://localhost:8989/api/v1/system/status # → { "version": "0.1.0", "firstBoot": true }
curl http://localhost:8989/api/v1/series        # → []
```

**Estimated scope**: ~200 lines changed

---

## Phase 1 — First-Boot Wizard + Core CRUD

**Goal**: User can complete first-boot setup, configure root folders, quality profiles, and settings through the API. No UI yet — just a working API contract.

**Worktree**: `phase1-first-boot`

**Depends on**: Phase 0

**Files to change**:
- `crates/stackarr-web/src/routes/system.rs` — Implement `POST /api/v1/setup/init`: validate input, persist enabled_modules to DB, save root folders, generate API key
- `crates/stackarr-core/src/db.rs` — Add `load_enabled_modules()`, `save_enabled_modules()`, `is_first_boot()` queries
- `src/main.rs` — Load EnabledModules from DB on startup (currently hardcoded to all-false default)
- `crates/stackarr-web/src/routes/system.rs` — GET status: load first_boot flag from DB, return real enabled modules
- `crates/stackarr-web/src/routes/quality.rs` — Verify CRUD works end-to-end against real Postgres
- Add API routes for root folders: `GET/POST/DELETE /api/v1/rootfolder`
- Add API routes for tags: `GET/POST/PUT/DELETE /api/v1/tag`
- Add API routes for naming config: `GET/PUT /api/v1/config/naming`

**Acceptance test**:
```bash
# Complete first-boot
curl -X POST localhost:8989/api/v1/setup/init -d '{
  "modules": { "tv_management": true, "movie_management": true },
  "root_folders": [{ "path": "/media/tv", "media_type": "series" }, { "path": "/media/movies", "media_type": "movie" }]
}'

# Subsequent startup loads modules from DB
docker compose restart stackarr
curl localhost:8989/api/v1/system/status  # → { "firstBoot": false, "modules": { "tv_management": true, ... } }
```

**Estimated scope**: ~500 lines new, ~100 lines changed

---

## Phase 2 — *arr Migration + Library Import

**Goal**: Users migrating from Sonarr/Radarr/Prowlarr can import their entire existing setup — series, movies, episodes, quality profiles, indexers, download clients, naming config, history, and blocklist. Users without *arr apps can manually add media via TMDB lookup. After import, a disk scan matches existing files to media.

**Worktree**: `phase2-migration`

**Depends on**: Phase 1

This is the biggest phase — it makes StackArr immediately useful for existing *arr users.

### 2a — New crate: `stackarr-migrate`

Add `crates/stackarr-migrate/` to workspace.

**Dependencies**: `rusqlite` (bundled, to read *arr SQLite DBs), `stackarr-core`, `stackarr-parser`, `serde`, `serde_json`, `anyhow`, `tracing`

```
crates/stackarr-migrate/src/
├── lib.rs
├── sonarr.rs        # Read Sonarr SQLite → StackArr models
├── radarr.rs        # Read Radarr SQLite → StackArr models
├── prowlarr.rs      # Read Prowlarr SQLite → StackArr models
└── writer.rs        # Insert migrated data into Postgres
```

### 2b — Sonarr migration (`sonarr.rs`)

**Source**: Sonarr SQLite DB (typically `sonarr.db` or `nzbdrone.db`)

Real Sonarr DB at `/home/sprooty/Working/TestNet/scraper/sonarr/sonarr.db` for testing.

Tables to read:

| Sonarr Table | → StackArr Table | Key Fields |
|---|---|---|
| `Series` (39 cols) | `series` | Title, CleanTitle, SortTitle, TvdbId, ImdbId, TmdbId, TvMazeId, MalIds, Status, Path, QualityProfileId, Monitored, SeasonFolder, UseSceneNumbering, SeriesType, Network, AirTime, FirstAired, Year, Runtime, Overview, Images, Genres, Tags |
| `Episodes` (23 cols) | `episodes` | SeriesId→mapped, SeasonNumber, EpisodeNumber, AbsoluteEpisodeNumber, SceneSeasonNumber, SceneEpisodeNumber, Title, Overview, AirDateUtc, AirDate, Monitored, EpisodeFileId→mapped, Runtime |
| `EpisodeFiles` (14 cols) | `media_files` + `episode_files` join | Quality (JSON), Size, DateAdded, RelativePath, SceneName, ReleaseGroup, MediaInfo, Languages, IndexerFlags |
| `QualityProfiles` (8 cols) | `quality_profiles` | Name, Cutoff, Items (JSON), UpgradeAllowed, FormatItems, MinFormatScore, CutoffFormatScore |
| `CustomFormats` (4 cols) | `custom_formats` | Name, Specifications (JSON) |
| `RootFolders` | `root_folders` | Path |
| `NamingConfig` (11 cols) | `naming_config` | StandardEpisodeFormat, DailyEpisodeFormat, AnimeEpisodeFormat, SeasonFolderFormat, RenameEpisodes, ColonReplacementFormat |
| `Indexers` (11 cols) | `indexers` | Name, Implementation→indexer_type, Settings (JSON→parse base_url/api_key), EnableRss, EnableAutomaticSearch, Priority |
| `DownloadClients` (9 cols) | `download_clients` | Name, Implementation→client_type, Settings (JSON→parse host/port/api_key), Priority, Enable |
| `Tags` | `tags` | Label |
| `History` (9 cols) | `history` | EpisodeId→mapped, SeriesId→mapped, SourceTitle, Date, Quality, EventType (int→enum), DownloadId |
| `Blocklist` (14 cols) | `blocklist` | SeriesId→mapped, SourceTitle, Quality, Date, TorrentInfoHash, Protocol, Indexer |
| `Notifications` (20 cols) | `notification_providers` | Name, Implementation→provider_type, Settings (JSON), OnGrab, OnDownload, OnUpgrade, OnHealthIssue |

**ID mapping**: Sonarr IDs → StackArr IDs. Maintain a `HashMap<i64, i64>` per entity type during migration. Quality profile IDs especially critical since Series/Movies reference them.

**Settings JSON parsing**: Sonarr stores download client/indexer config as JSON in a `Settings` column. Structure varies by `Implementation` type:
- `Sabnzbd`: `{ "host": "...", "port": 8080, "apiKey": "...", "tvCategory": "..." }`
- `QBittorrent`: `{ "host": "...", "port": 8080, "username": "...", "password": "...", "tvCategory": "..." }`
- `Newznab`: `{ "baseUrl": "...", "apiKey": "...", "categories": [...] }`
- Parse these into StackArr's `config JSONB` column

### 2c — Radarr migration (`radarr.rs`)

**Source**: Radarr SQLite DB (typically `radarr.db`)

Real Radarr DB at `/home/sprooty/Working/profsync/test/radarr-config/radarr.db` for testing.

| Radarr Table | → StackArr Table | Notes |
|---|---|---|
| `Movies` (10 cols) + `MovieMetadata` (30 cols) | `movies` | Must JOIN — Movies has config (Path, QualityProfileId, Monitored, MinimumAvailability), MovieMetadata has metadata (Title, TmdbId, ImdbId, Year, Overview, InCinemas, PhysicalRelease, DigitalRelease, Images, Genres, Studio, CollectionTmdbId) |
| `MovieFiles` (12 cols) | `media_files` | Quality, Size, DateAdded, RelativePath, SceneName, ReleaseGroup, MediaInfo, Edition, Languages |
| `QualityProfiles` (9 cols) | `quality_profiles` | Same as Sonarr + Language field |
| `CustomFormats` | `custom_formats` | Same structure |
| `RootFolders` | `root_folders` | Path (media_type = 'movie') |
| `NamingConfig` (5 cols) | `naming_config` | StandardMovieFormat, MovieFolderFormat, RenameMovies, ColonReplacementFormat |
| `Indexers` | `indexers` | Same structure as Sonarr |
| `DownloadClients` | `download_clients` | Same structure, merge with Sonarr's (deduplicate by name+host) |
| `Tags` | `tags` | Merge with Sonarr's (deduplicate by label) |
| `History` | `history` | MovieId→mapped, same event types |
| `Blocklist` | `blocklist` | MovieId→mapped |
| `Notifications` | `notification_providers` | Merge with Sonarr's (deduplicate) |
| `AlternativeTitles` | `alternative_titles` | CleanTitle, Title |

### 2d — Prowlarr migration (`prowlarr.rs`)

**Source**: Prowlarr SQLite DB (typically `prowlarr.db`)

Real Prowlarr DB at `/var/lib/docker/volumes/indexarr-prowlarr_prowlarr-config/_data/prowlarr.db` for testing.

| Prowlarr Table | → StackArr Table | Notes |
|---|---|---|
| `Indexers` (11 cols) | `indexers` | Prowlarr is the source of truth for indexer configs. Name, Implementation (Newznab/Torznab/etc.), Settings (JSON with baseUrl, apiKey, categories), Enable, Priority |
| `Tags` | `tags` | Merge |

**Deduplication**: If user imports both Sonarr+Prowlarr, Prowlarr indexers take priority (they're the canonical source). Match by `baseUrl` to deduplicate.

### 2e — Writer (`writer.rs`)

```rust
pub struct MigrationWriter { pool: PgPool }

impl MigrationWriter {
    /// Write all migrated data in a single transaction
    pub async fn write_all(&self, data: MigrationData) -> Result<MigrationReport>
}

pub struct MigrationData {
    pub quality_profiles: Vec<QualityProfile>,
    pub custom_formats: Vec<CustomFormat>,
    pub root_folders: Vec<RootFolder>,
    pub tags: Vec<Tag>,
    pub naming_config: Vec<NamingConfig>,
    pub indexers: Vec<IndexerConfig>,
    pub download_clients: Vec<DownloadClientConfig>,
    pub notification_providers: Vec<NotificationProvider>,
    pub series: Vec<SeriesWithEpisodes>,
    pub movies: Vec<MovieWithFile>,
    pub history: Vec<HistoryEvent>,
    pub blocklist: Vec<Blocklist>,
}

pub struct MigrationReport {
    pub series_imported: usize,
    pub movies_imported: usize,
    pub episodes_imported: usize,
    pub files_imported: usize,
    pub quality_profiles_imported: usize,
    pub indexers_imported: usize,
    pub download_clients_imported: usize,
    pub history_events_imported: usize,
    pub warnings: Vec<String>,  // e.g., "Skipped duplicate indexer: NZBGeek"
}
```

### 2f — CLI + API integration

**CLI command**:
```bash
stackarr migrate \
  --sonarr /path/to/sonarr.db \
  --radarr /path/to/radarr.db \
  --prowlarr /path/to/prowlarr.db \
  --dry-run  # optional: show what would be imported without writing
```

**API endpoint** (for UI-driven migration):
```
POST /api/v1/system/migrate
Content-Type: multipart/form-data
  sonarr_db: <file>
  radarr_db: <file>
  prowlarr_db: <file>
```

Returns `MigrationReport`.

### 2g — Disk scan + file matching

After migration, series/movies have `Path` set but `episode_file_id` / `movie_file_id` may not match if the import didn't bring file records or paths changed. Disk scan resolves this.

**Files to change**:
- `crates/stackarr-import/src/lib.rs` — Add `disk_scan(root_folder: &RootFolder) -> Result<ScanResult>`:
  1. Walk all subdirectories under root folder
  2. For each media file found, parse filename with `stackarr-parser::parse_release`
  3. Match to series/movie by directory name (clean_title match against DB)
  4. Match to specific episode by parsed season/episode numbers
  5. Create `media_files` record if not already tracked
  6. Link to episode/movie
  7. Report: files matched, files unmatched, new files found

- `crates/stackarr-import/src/naming.rs` — Token-based filename builder (needed for rename preview)

- `crates/stackarr-web/src/routes/` — Add endpoints:
  - `POST /api/v1/command { "name": "DiskScan" }` — scan all root folders
  - `POST /api/v1/command { "name": "DiskScan", "seriesId": 1 }` — scan one series path
  - `GET /api/v1/manualimport?folder=` — scan arbitrary folder, show matches

### 2h — TMDB lookup for non-migrators

Users not coming from *arr apps need to add media manually:

- `crates/stackarr-metadata/src/lib.rs` — Add rate limiter (governor crate)
- `crates/stackarr-media/src/lib.rs` — Add `add_series_from_tmdb()` and `add_movie_from_tmdb()` that fetch metadata + create all episodes
- `crates/stackarr-web/src/routes/series.rs` — Wire `GET /api/v1/series/lookup?term=` → search TMDB
- `crates/stackarr-web/src/routes/movies.rs` — Same for movies

**Acceptance test**:
```bash
# === Migration path ===

# Dry run
stackarr migrate --sonarr /backup/sonarr.db --radarr /backup/radarr.db --prowlarr /backup/prowlarr.db --dry-run
# → "Would import: 45 series, 892 episodes, 312 movies, 3 quality profiles, 8 indexers, 2 download clients, 4521 history events"

# Real migration
stackarr migrate --sonarr /backup/sonarr.db --radarr /backup/radarr.db --prowlarr /backup/prowlarr.db
# → MigrationReport

# Verify
curl localhost:8989/api/v1/series | jq length    # → 45
curl localhost:8989/api/v1/movie | jq length      # → 312
curl localhost:8989/api/v1/qualityprofile          # → profiles from Sonarr/Radarr
curl localhost:8989/api/v1/indexer                  # → indexers from Prowlarr

# Disk scan matches existing files
curl -X POST localhost:8989/api/v1/command -d '{ "name": "DiskScan" }'
# → scans /media/tv and /media/movies, matches files to imported media

curl localhost:8989/api/v1/series/1
# → { "episodeFileCount": 48, "sizeOnDisk": 125000000000 }

# === Manual path (no *arr migration) ===

# Search TMDB
curl "localhost:8989/api/v1/series/lookup?term=breaking+bad"
# → [{ "tmdbId": 1396, "title": "Breaking Bad", ... }]

# Add to library
curl -X POST localhost:8989/api/v1/series -d '{ "tmdbId": 1396, "qualityProfileId": 1, "rootFolderId": 1, "monitored": true }'

# Disk scan picks up existing files
curl -X POST localhost:8989/api/v1/command -d '{ "name": "DiskScan", "seriesId": 1 }'
```

**Estimated scope**: ~2500 lines new (stackarr-migrate crate + disk scan + TMDB lookup)

---

## Phase 3 — Library Views + Metadata Refresh

**Goal**: Calendar, wanted (missing/cutoff), and metadata auto-refresh. The library is now fully browsable and the user can see exactly what they have, what's missing, and what's upcoming.

**Worktree**: `phase3-library`

**Depends on**: Phase 2

**Files to change**:
- `crates/stackarr-web/src/routes/` — Add endpoints:
  - `GET /api/v1/calendar?start=&end=` — upcoming episodes + movie releases
  - `GET /api/v1/wanted/missing` — monitored media with no file
  - `GET /api/v1/wanted/cutoff` — media with file below quality profile cutoff
  - `GET /api/v1/series/{id}/episodes` — episode list for a series
  - `PUT /api/v1/episode/{id}` — toggle monitored
  - `PUT /api/v1/episode/monitor` — bulk monitor/unmonitor
- `crates/stackarr-media/src/lib.rs` — Add `refresh_series(id)` / `refresh_movie(id)` for re-syncing metadata from TMDB
- `crates/stackarr-scheduler/src/lib.rs` — Implement `metadata_refresh_task`: find stale series/movies (last_info_sync > 12h), call refresh
- `crates/stackarr-media/src/lib.rs` — Add queries for wanted/missing/cutoff/calendar

**Acceptance test**:
```bash
# Calendar
curl "localhost:8989/api/v1/calendar?start=2026-03-20&end=2026-04-20"
# → [{ "seriesTitle": "...", "episodeTitle": "...", "airDateUtc": "...", "hasFile": false }, ...]

# Missing
curl localhost:8989/api/v1/wanted/missing
# → paginated list of monitored episodes/movies without files

# Cutoff unmet
curl localhost:8989/api/v1/wanted/cutoff
# → episodes/movies where current file is below quality profile cutoff

# Metadata refresh happens automatically every 12h, or manually:
curl -X POST localhost:8989/api/v1/command -d '{ "name": "RefreshSeries", "seriesId": 1 }'
```

**Estimated scope**: ~600 lines new, ~200 lines changed

---

## Phase 4 — Search + Decision Engine + Grab

**Goal**: User can search indexers for a release, the decision engine filters/ranks results, and grabbing sends to a download client.

**Worktree**: `phase4-search-grab`

**Depends on**: Phase 3 (needs library views to know what's missing/cutoff)

**Files to change**:
- `crates/stackarr-quality/src/lib.rs` — Implement decision specifications:
  - `QualityAllowedSpec`: Parse release quality, check against profile items
  - `QualityCutoffSpec`: Reject if already have file at or above cutoff
  - `MinimumSizeSpec` / `MaximumSizeSpec`: Per-quality size thresholds
  - `BlocklistSpec`: Check release title/hash against blocklist table
  - `QueueConflictSpec`: Check if same media is already in queue
  - `AlreadyImportedSpec`: Check if same quality already on disk
  - `CustomFormatScoreSpec`: Score release against custom format rules
  - `MinimumSeedersSpec`: Torrent-specific seeder threshold
- `crates/stackarr-quality/src/lib.rs` — Add `rank_releases()` — sort approved releases by quality rank, custom format score, protocol preference, indexer priority, age/seeders
- `crates/stackarr-web/src/routes/releases.rs` — Implement search handler:
  - Look up media in DB to get external IDs
  - Build TvSearchCriteria / MovieSearchCriteria
  - Fan out to all enabled indexers
  - Run decision engine on results
  - Return ranked list with approval/rejection reasons
- `crates/stackarr-web/src/routes/releases.rs` — Implement grab handler:
  - Select download client by protocol + priority
  - Call client.add(GrabRequest)
  - Insert queue record + history record (Grabbed)
  - Send notification (on_grab)
- Add API routes for download clients: `GET/POST/PUT/DELETE /api/v1/downloadclient` with `POST /test`
- Add API routes for indexers: `GET/POST/PUT/DELETE /api/v1/indexer` with `POST /test`
- `crates/stackarr-web/src/state.rs` — Add `IndexerManager` and `DownloadClientManager` to AppState
- `src/main.rs` — Initialize managers from DB config on startup

**Acceptance test**:
```bash
# Search for an episode
curl "localhost:8989/api/v1/release?episodeId=1"
# → [{ "title": "Breaking.Bad.S01E01.1080p.BluRay...", "approved": true, "quality": {...}, "rejections": [] }, ...]

# Grab a release
curl -X POST localhost:8989/api/v1/release -d '{ "guid": "abc123", "indexerId": 1 }'
# → { "downloadId": "abc123...", "approved": true }

# Check queue
curl localhost:8989/api/v1/queue
# → [{ "title": "Breaking Bad S01E01", "status": "downloading", ... }]
```

**Estimated scope**: ~1200 lines new, ~300 lines changed

---

## Phase 5 — Download Import Pipeline

**Goal**: When a download completes, StackArr detects it, matches it to the right series/movie, renames/moves the file to the library, and updates the database.

**Worktree**: `phase5-download-import`

**Depends on**: Phase 4

**Files to change**:
- `crates/stackarr-import/src/lib.rs` — Implement `process_completed_download()`:
  1. Scan download output path for media files
  2. Parse each filename with `stackarr-parser::parse_release`
  3. Match to series+episode or movie (using queue_item as hint, fallback to title matching)
  4. Run quality check (is this an upgrade?)
  5. Build target path using naming config tokens
  6. Move or hardlink file to library
  7. Insert `media_files` row, update `episode.episode_file_id` / `movie.movie_file_id`
  8. Delete old file if quality upgrade
- `crates/stackarr-scheduler/src/lib.rs` — Implement `completed_download_task`:
  - Poll all download clients every 60s
  - Match completed items to queue records
  - Call ImportService
  - Handle failures (blocklist + re-search)
  - Send notifications

**Acceptance test**:
```bash
# Download completes → file auto-imported to library
ls /media/tv/Breaking\ Bad/Season\ 01/
# → "Breaking Bad - S01E01 - Pilot [Bluray-1080p].mkv"

curl localhost:8989/api/v1/history
# → [{ "eventType": "imported", ... }]
```

**Estimated scope**: ~1000 lines new, ~200 lines changed

---

## Phase 6 — RSS Automation

**Goal**: StackArr automatically monitors indexer RSS feeds and grabs new releases for monitored media without manual intervention.

**Worktree**: `phase6-rss`

**Depends on**: Phase 5

**Files to change**:
- `crates/stackarr-scheduler/src/lib.rs` — Implement `rss_sync_task`:
  1. Fetch RSS from all enabled indexers
  2. Parse releases, match to monitored media
  3. Run decision engine, auto-grab approved
  4. Track last RSS sync time per indexer
- `crates/stackarr-scheduler/src/lib.rs` — Add `missing_search_task` (search for missing media on configurable schedule)
- `crates/stackarr-web/src/routes/` — Add command endpoints:
  - `POST /api/v1/command` — RssSync, SeriesSearch, EpisodeSearch, MovieSearch, MissingSearch
- Add `commands` table for async command tracking

**Acceptance test**:
```bash
curl -X POST localhost:8989/api/v1/command -d '{ "name": "RssSync" }'
# New episode grabbed automatically within 15 min
```

**Estimated scope**: ~800 lines new, ~200 lines changed

---

## Phase 7 — Indexarr Sidecar Integration

**Goal**: Indexarr runs as an optional Docker sidecar in peer-only mode.

**Worktree**: `phase7-indexarr`

**Depends on**: Phase 6. **Parallel with**: Phase 8

**Files to change**:
- `crates/stackarr-indexer/src/indexarr.rs` — Complete Torznab passthrough + REST search + health + status
- `crates/stackarr-indexer/src/search.rs` — Integrate IndexarrClient into search fanout
- `crates/stackarr-web/src/routes/` — Add `GET /api/v1/indexarr/status`
- `docker/docker-compose.yml` — Optional Indexarr service (profile-gated, peer-only mode)

**Estimated scope**: ~400 lines new, ~100 lines changed

---

## Phase 8 — Embedded Download Clients

**Goal**: Optional embedded rustTorrent and/or rustnzbd engines inside StackArr.

**Worktree**: `phase8-embedded-clients`

**Depends on**: Phase 6. **Parallel with**: Phase 7

**Files to change**:
- `Cargo.toml` — Wire feature flags to `librtbit` and `nzb-*` optional dependencies
- `crates/stackarr-download/src/embedded_torrent.rs` — `EmbeddedTorrentClient` wrapping `librtbit::Session`
- `crates/stackarr-download/src/embedded_usenet.rs` — `EmbeddedUsenetClient` wrapping `nzb_web::QueueManager`
- `src/main.rs` — Conditionally init embedded clients
- `crates/stackarr-web/src/routes/` — Embedded client status routes

**Estimated scope**: ~600 lines new, ~100 lines changed

---

## Phase 9 — React UI

**Goal**: Web-based frontend — first-boot wizard, library, migration, search, queue, calendar.

**Worktree**: `phase9-ui`

**Depends on**: Phase 4 (API must be stable). **Parallel with**: Phases 5-8

**Tech**: React 19 + TypeScript + Vite + TanStack Query + Tailwind

**Key pages**:
- First Boot Wizard (module selection → migration upload → root folders → quality profiles → auth)
- Migration page (upload *arr DBs, see report)
- Series/Movie list + detail views
- Interactive search + grab
- Queue + history
- Calendar + wanted views
- Settings (profiles, clients, indexers, naming, notifications)

**Estimated scope**: ~5000-8000 lines

---

## Phase 10 — Polish + Hardening

**Goal**: Production-ready for daily use.

**Worktree**: `phase10-polish`

**Depends on**: All previous phases

**Work items**:
- Integration tests (Postgres in Docker, full flow testing)
- API authentication middleware (API key + session auth)
- Backup/restore (export/import DB as JSON)
- Health check system (DB, disk space, client connectivity, indexer health)
- Import lists (TMDB popular, Trakt watchlist, IMDB list → auto-add media)
- Disk scan on schedule (detect files added outside StackArr)
- Scene name mapping / XEM integration for anime
- Custom format specification engine (full regex)
- Blocklist management
- Log viewer (WebSocket streaming)
- OpenAPI/Swagger docs (utoipa)
- Prometheus metrics endpoint
- Notification providers: Telegram, Slack, email

---

## Worktree Parallelism Guide

```
Phase 0 ──→ Phase 1 ──→ Phase 2 ──→ Phase 3 ──→ Phase 4 ──→ Phase 5 ──→ Phase 6
                         (migration)  (library)   (search)    (import)    (RSS)
                                                      │                     │
                                                      │       Phase 7 ◄─────┤ (Indexarr)
                                                      │       Phase 8 ◄─────┘ (Embedded clients)
                                                      │
                                          Phase 9 ◄───┘ (UI — parallel with 5-8)

Phase 10 (after all)
```

**Safe parallel pairs** (touch different files):
- Phase 7 + Phase 8 (Indexarr sidecar + embedded clients — different crates)
- Phase 9 + Phases 5-8 (UI is entirely in `ui/`, backend phases touch `crates/`)

**Cannot run in parallel** (same files):
- Phase 4 + Phase 5 (both modify stackarr-web routes and scheduler)
- Phase 5 + Phase 6 (both modify scheduler tasks)

---

## Reference: *arr Database Locations for Testing

| App | Database Path |
|-----|--------------|
| Sonarr | `/home/sprooty/Working/TestNet/scraper/sonarr/sonarr.db` |
| Radarr | `/home/sprooty/Working/profsync/test/radarr-config/radarr.db` |
| Prowlarr | `/var/lib/docker/volumes/indexarr-prowlarr_prowlarr-config/_data/prowlarr.db` |
