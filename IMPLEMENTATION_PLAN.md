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

### What Doesn't Work Yet

- **Can't boot**: No DATABASE_URL env var support, no default config generation, migration path broken
- **Decision engine**: All specifications are stubs — every release gets approved
- **Import pipeline**: Scans files but doesn't parse, match, rename, or move them
- **Scheduler tasks**: RSS sync, import scan, metadata refresh all log "no-op stub"
- **Release grab**: Web handler logs the request but never sends to a download client
- **Setup wizard**: POST /api/v1/setup/init always returns `{ success: true }`
- **No UI**: Raw JSON API only

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

**Goal**: User can complete first-boot setup, add series/movies, configure root folders and quality profiles through the API. No UI yet — just a working API contract.

**Worktree**: `phase1-first-boot`

**Depends on**: Phase 0

**Files to change**:
- `crates/stackarr-web/src/routes/system.rs` — Implement `POST /api/v1/setup/init`: validate input, persist enabled_modules to DB, save root folders, generate API key
- `crates/stackarr-core/src/db.rs` — Add `load_enabled_modules()`, `save_enabled_modules()`, `is_first_boot()` queries
- `src/main.rs` — Load EnabledModules from DB on startup (currently hardcoded to all-false default)
- `crates/stackarr-web/src/routes/system.rs` — GET status: load first_boot flag from DB, return real enabled modules
- `crates/stackarr-web/src/routes/quality.rs` — Verify CRUD works end-to-end against real Postgres (currently untested)
- Add API routes for root folders: `GET/POST/DELETE /api/v1/rootfolder`
- Add API routes for tags: `GET/POST/PUT/DELETE /api/v1/tag`
- Add API routes for download clients: `GET/POST/PUT/DELETE /api/v1/downloadclient` with `POST /test`
- Add API routes for indexers: `GET/POST/PUT/DELETE /api/v1/indexer` with `POST /test`

**Acceptance test**:
```bash
# Complete first-boot
curl -X POST localhost:8989/api/v1/setup/init -d '{
  "modules": { "tv_management": true, "movie_management": true, "torrent_external": true },
  "root_folders": [{ "path": "/media/tv", "media_type": "series" }, { "path": "/media/movies", "media_type": "movie" }]
}'

# Add a quality profile
curl -X POST localhost:8989/api/v1/qualityprofile -d '{ "name": "HD", "cutoff": 13, ... }'

# Add a download client
curl -X POST localhost:8989/api/v1/downloadclient -d '{ "name": "qBit", "client_type": "qbittorrent", ... }'

# Subsequent startup loads modules from DB
docker compose restart stackarr
curl localhost:8989/api/v1/system/status  # → { "firstBoot": false, "modules": { "tv_management": true, ... } }
```

**Estimated scope**: ~500 lines new, ~100 lines changed

---

## Phase 2 — Metadata + Library Management

**Goal**: User can search TMDB, add series/movies to their library, and see episodes populate. Metadata auto-refreshes.

**Worktree**: `phase2-metadata`

**Depends on**: Phase 1

**Files to change**:
- `crates/stackarr-metadata/src/lib.rs` — Add rate limiter (governor crate), add DB cache layer for TMDB responses
- `crates/stackarr-media/src/lib.rs` — Add `add_series_from_tmdb(tmdb_id, quality_profile_id, root_folder_id, monitored)` that fetches metadata + creates all episodes
- `crates/stackarr-media/src/lib.rs` — Add `add_movie_from_tmdb(tmdb_id, ...)` equivalent
- `crates/stackarr-media/src/lib.rs` — Add `refresh_series(id)` / `refresh_movie(id)` for re-syncing metadata
- `crates/stackarr-web/src/routes/series.rs` — Wire lookup endpoint: `GET /api/v1/series/lookup?term=` → search TMDB
- `crates/stackarr-web/src/routes/movies.rs` — Same for movies
- `crates/stackarr-web/src/routes/` — Add calendar endpoint: `GET /api/v1/calendar?start=&end=`
- `crates/stackarr-web/src/routes/` — Add wanted endpoints: `GET /api/v1/wanted/missing`, `GET /api/v1/wanted/cutoff`
- `crates/stackarr-scheduler/src/lib.rs` — Implement `metadata_refresh_task`: find stale series/movies, call refresh

**Acceptance test**:
```bash
# Search for a show
curl "localhost:8989/api/v1/series/lookup?term=breaking+bad"
# → [{ "tmdbId": 1396, "title": "Breaking Bad", ... }]

# Add it to library
curl -X POST localhost:8989/api/v1/series -d '{ "tmdbId": 1396, "qualityProfileId": 1, "rootFolderId": 1, "monitored": true }'
# → { "id": 1, "title": "Breaking Bad", "seasons": [...], "episodeCount": 62 }

# Check episodes were created
curl localhost:8989/api/v1/series/1/episodes
# → [{ "seasonNumber": 1, "episodeNumber": 1, "title": "Pilot", ... }, ...]

# Calendar shows upcoming episodes
curl "localhost:8989/api/v1/calendar?start=2026-03-20&end=2026-04-20"
```

**Estimated scope**: ~800 lines new, ~200 lines changed

---

## Phase 3 — Search + Decision Engine + Grab

**Goal**: User can search indexers for a release, the decision engine filters/ranks results, and grabbing sends to a download client.

**Worktree**: `phase3-search-grab`

**Depends on**: Phase 2

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
- `crates/stackarr-quality/src/lib.rs` — Add `rank_releases(decisions: Vec<DownloadDecision>) -> Vec<DownloadDecision>` — sort approved releases by quality rank, custom format score, protocol preference, indexer priority, age/seeders
- `crates/stackarr-web/src/routes/releases.rs` — Implement search handler:
  - Take episodeId or movieId query param
  - Look up media in DB to get external IDs
  - Build TvSearchCriteria / MovieSearchCriteria
  - Call IndexerManager.search()
  - Run decision engine on results
  - Return ranked list with approval/rejection reasons
- `crates/stackarr-web/src/routes/releases.rs` — Implement grab handler:
  - Select download client by protocol
  - Call client.add(GrabRequest)
  - Insert queue record
  - Insert history record (Grabbed)
  - Send notification (on_grab)
- `crates/stackarr-web/src/state.rs` — Add `IndexerManager` and `DownloadClientManager` to AppState
- `src/main.rs` — Initialize IndexerManager and DownloadClientManager from DB config on startup

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

# Check history
curl localhost:8989/api/v1/history
# → [{ "eventType": "grabbed", "sourceTitle": "Breaking.Bad.S01E01...", ... }]
```

**Estimated scope**: ~1000 lines new, ~300 lines changed

---

## Phase 4 — Import Pipeline

**Goal**: When a download completes, StackArr detects it, matches it to the right series/movie, renames/moves the file to the library, and updates the database.

**Worktree**: `phase4-import`

**Depends on**: Phase 3

**Files to change**:
- `crates/stackarr-import/src/lib.rs` — Full rewrite of import pipeline:
  - `process_completed_download(item, queue_item)`:
    1. Scan download output path for media files
    2. Parse each filename with `stackarr-parser::parse_release`
    3. Match to series+episode or movie (using queue_item as primary hint, fallback to title matching)
    4. Run quality check (is this actually an upgrade?)
    5. Build target path using naming config tokens
    6. Move or hardlink file to library
    7. Insert `media_files` row
    8. Update `episode.episode_file_id` or `movie.movie_file_id`
    9. Delete old file if quality upgrade
    10. Return ImportResult with success/failure details
- `crates/stackarr-import/src/naming.rs` — New file: token-based filename builder
  - Parse format strings like `{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]`
  - Replace tokens with actual values
  - Handle special characters, path length limits, colon replacement
- `crates/stackarr-scheduler/src/lib.rs` — Implement `completed_download_task`:
  - Poll all download clients every 60s
  - Match completed items to queue records
  - Call ImportService.process_completed_download()
  - Remove queue record on success
  - Add to blocklist on failure (configurable)
  - Trigger re-search on failure (configurable)
  - Send notifications (on_import / on_failure)
- `crates/stackarr-web/src/routes/` — Add manual import routes:
  - `GET /api/v1/manualimport?folder=` — scan folder, show matches
  - `POST /api/v1/manualimport` — execute import of selected files

**Acceptance test**:
```bash
# Simulate a completed download (put file in download dir)
cp test.mkv /downloads/complete/tv/Breaking.Bad.S01E01.1080p.BluRay.x264-GROUP/

# Scheduler picks it up within 60s, or trigger manually:
curl -X POST localhost:8989/api/v1/command -d '{ "name": "ImportScan" }'

# File moved to library
ls /media/tv/Breaking\ Bad/Season\ 01/
# → "Breaking Bad - S01E01 - Pilot [Bluray-1080p].mkv"

# Episode now has a file
curl localhost:8989/api/v1/episode/1
# → { "hasFile": true, "episodeFile": { "quality": { "quality": "Bluray1080p" }, ... } }

# History shows import
curl localhost:8989/api/v1/history
# → [{ "eventType": "imported", ... }]
```

**Estimated scope**: ~1200 lines new, ~200 lines changed

---

## Phase 5 — RSS Automation

**Goal**: StackArr automatically monitors indexer RSS feeds and grabs new releases for monitored media without manual intervention.

**Worktree**: `phase5-rss`

**Depends on**: Phase 4

**Files to change**:
- `crates/stackarr-scheduler/src/lib.rs` — Implement `rss_sync_task`:
  1. For each enabled indexer: fetch RSS feed (Newznab `t=search` with no query, or `t=tvsearch`/`t=movie` by category)
  2. Parse each release with `stackarr-parser::parse_release`
  3. Match releases to monitored series/movies in DB (by title similarity + external IDs)
  4. For matches: run decision engine
  5. For approved: auto-grab (same as manual grab flow)
  6. Track last RSS sync time per indexer to avoid re-processing
- `crates/stackarr-indexer/src/newznab.rs` — Add `rss_feed(categories)` method (fetch recent releases)
- `crates/stackarr-scheduler/src/lib.rs` — Add `missing_search_task`:
  - Periodically search for monitored episodes/movies that have no file
  - Configurable interval (default: disabled / manual trigger)
- `crates/stackarr-web/src/routes/` — Add command endpoints:
  - `POST /api/v1/command` with `{ "name": "RssSync" }` — trigger manual RSS sync
  - `POST /api/v1/command` with `{ "name": "SeriesSearch", "seriesId": 1 }` — search all indexers for a series
  - `POST /api/v1/command` with `{ "name": "EpisodeSearch", "episodeId": 5 }` — search for specific episode
  - `POST /api/v1/command` with `{ "name": "MovieSearch", "movieId": 3 }` — search for a movie
  - `POST /api/v1/command` with `{ "name": "MissingSearch" }` — search all missing media
- Add `commands` table to track async command status (id, name, status, started_at, ended_at, message)

**Acceptance test**:
```bash
# Trigger RSS sync
curl -X POST localhost:8989/api/v1/command -d '{ "name": "RssSync" }'

# New episode airs, RSS picks it up → auto-grabbed → auto-imported
# Within 15 min + download time:
curl localhost:8989/api/v1/history
# → [{ "eventType": "grabbed", "sourceTitle": "New.Show.S01E05..." }, { "eventType": "imported", ... }]

# Search for specific missing episode
curl -X POST localhost:8989/api/v1/command -d '{ "name": "EpisodeSearch", "episodeId": 42 }'
```

**Estimated scope**: ~800 lines new, ~200 lines changed

---

## Phase 6 — Indexarr Sidecar Integration

**Goal**: Indexarr runs as an optional Docker sidecar in peer-only mode. StackArr uses it as a torrent indexer source alongside standard Newznab/Torznab indexers.

**Worktree**: `phase6-indexarr`

**Depends on**: Phase 5

**Files to change**:
- `crates/stackarr-indexer/src/indexarr.rs` — Complete implementation:
  - Torznab passthrough (Indexarr's `/api/torznab` endpoint, same XML format as Newznab)
  - REST API search (`GET /api/v1/search?q=...`) with richer results (trackers, content classification)
  - Health check (`GET /health`)
  - Status fetch (`GET /api/v1/stats` — index size, sync status, peer count)
  - Auto-detect sidecar on startup (try `http://indexarr:8080/health`)
- `crates/stackarr-indexer/src/search.rs` — Integrate IndexarrClient into search fanout alongside NewznabClients
- `crates/stackarr-web/src/routes/` — Add Indexarr status routes:
  - `GET /api/v1/indexarr/status` — sidecar health, index stats, sync status
- `docker/docker-compose.yml` — Add optional Indexarr service (profile-gated):
  ```yaml
  indexarr:
    image: indexarr:latest
    environment:
      INDEXARR_WORKERS: http_server,sync   # peer-only mode
    profiles: [indexarr]
  ```
- `crates/stackarr-web/src/routes/system.rs` — Setup wizard option for Indexarr (auto-detect URL, test connection)

**Acceptance test**:
```bash
# Start with Indexarr sidecar
COMPOSE_PROFILES=indexarr docker compose up -d

# Indexarr status visible
curl localhost:8989/api/v1/indexarr/status
# → { "connected": true, "indexSize": 50000, "syncStatus": "synced", "peerCount": 3 }

# Search includes Indexarr results alongside other indexers
curl "localhost:8989/api/v1/release?movieId=1"
# → results include { "indexerName": "Indexarr", "protocol": "torrent", ... }
```

**Estimated scope**: ~400 lines new, ~100 lines changed

---

## Phase 7 — Embedded Download Clients

**Goal**: Users who don't want external qBittorrent/SABnzbd can enable embedded rustTorrent and/or rustnzbd engines inside StackArr.

**Worktree**: `phase7-embedded-clients`

**Depends on**: Phase 5

**Files to change**:
- `Cargo.toml` — Wire feature flags to actual library dependencies:
  ```toml
  [features]
  torrent-embedded = ["dep:librtbit"]
  usenet-embedded = ["dep:nzb-core", "dep:nzb-web", ...]

  [dependencies]
  librtbit = { path = "../rustTorrent/crates/librtbit", optional = true, features = ["default-tls"] }
  nzb-core = { path = "../rustnzbd/crates/nzb-core", optional = true }
  nzb-web = { path = "../rustnzbd/crates/nzb-web", optional = true }
  ```
- `crates/stackarr-download/src/embedded_torrent.rs` — New file:
  - `EmbeddedTorrentClient` wrapping `librtbit::Session` + `librtbit::Api`
  - Init: `Session::new_with_opts()` with config-driven options
  - `add()`: `session.add_torrent(AddTorrent::from_url(magnet), opts)`
  - `get_items()`: `api.api_torrent_list()` → map to `DownloadItem`
  - Status mapping: TorrentStatsState → DownloadItemStatus
  - Category support via librtbit's CategoryManager
- `crates/stackarr-download/src/embedded_usenet.rs` — New file:
  - `EmbeddedUsenetClient` wrapping `nzb_web::QueueManager`
  - Init: `nzb_web::startup::initialize(StartupConfig)`
  - `add()`: Download NZB → `parse_nzb()` → `queue_manager.add_job()`
  - `get_items()`: `queue_manager.get_jobs()` → map to `DownloadItem`
  - Status mapping: JobStatus → DownloadItemStatus
- `src/main.rs` — Conditionally init embedded clients based on EnabledModules + feature flags
- `crates/stackarr-web/src/routes/` — Add embedded client status routes:
  - `GET /api/v1/torrent/status` — session stats, active torrents
  - `GET /api/v1/usenet/status` — queue stats, server health
  - `PUT /api/v1/usenet/servers` — manage NNTP servers at runtime

**Acceptance test**:
```bash
# Build with embedded torrent
cargo build --features torrent-embedded

# Enable in first-boot
curl -X POST localhost:8989/api/v1/setup/init -d '{ "modules": { "torrent_embedded": true } }'

# Grab a torrent release → uses embedded client
curl -X POST localhost:8989/api/v1/release -d '{ "guid": "...", "indexerId": 1 }'

# Embedded torrent status
curl localhost:8989/api/v1/torrent/status
# → { "activeTorrents": 1, "downloadSpeed": "5.2 MB/s", "uploadSpeed": "1.1 MB/s" }
```

**Estimated scope**: ~600 lines new, ~100 lines changed

---

## Phase 8 — React UI

**Goal**: Web-based frontend matching the API surface — first-boot wizard, library management, search, queue monitoring.

**Worktree**: `phase8-ui`

**Depends on**: Phase 5 (API must be stable)

**Tech**: React 19 + TypeScript + Vite + TanStack Query + Tailwind

**Structure**:
```
ui/
├── src/
│   ├── api/            # Generated or hand-written API client
│   ├── components/     # Shared UI components
│   ├── pages/
│   │   ├── FirstBoot/  # Setup wizard (5 steps)
│   │   ├── Series/     # List, detail, add, search
│   │   ├── Movies/     # List, detail, add, search
│   │   ├── Calendar/   # Upcoming episodes + movie releases
│   │   ├── Queue/      # Active downloads
│   │   ├── History/    # Event log
│   │   ├── Wanted/     # Missing + cutoff unmet
│   │   ├── Settings/   # Profiles, clients, indexers, naming, notifications
│   │   └── System/     # Status, health, logs
│   ├── hooks/          # useQuery hooks per resource
│   └── App.tsx         # Router + layout
├── package.json
├── vite.config.ts
└── tsconfig.json
```

**Key pages**:
- **First Boot Wizard**: Module selection → download clients → indexers → root folders → quality profile → auth
- **Series List**: Grid/table of series with poster, status, episode count, quality profile
- **Series Detail**: Seasons accordion, episode list with file status, manual search button
- **Movie List**: Grid with poster, year, status, file quality
- **Interactive Search**: Table of results with quality, size, seeders, approval status, grab button
- **Queue**: Real-time download progress, pause/resume/remove buttons
- **Calendar**: Month/week view of upcoming episodes and movie releases
- **Settings**: Tabbed interface for all configuration

**Estimated scope**: ~5000-8000 lines (full SPA)

---

## Phase 9 — Polish + Hardening

**Goal**: Production-ready for daily use.

**Worktree**: `phase9-polish`

**Depends on**: All previous phases

**Work items**:
- Comprehensive integration tests (spin up Postgres in Docker, test full flows)
- API authentication middleware (API key + session-based auth)
- Backup/restore (export/import DB as JSON)
- Health check system (check DB, disk space, download client connectivity, indexer health)
- Import lists (TMDB popular, Trakt watchlist, IMDB list → auto-add media)
- Manual import UI (scan folder, preview matches, execute)
- Rename preview (show before/after for library reorganization)
- Disk scan (detect files added outside StackArr)
- Scene name mapping / XEM integration for anime
- Custom format specification engine (regex matching on release names)
- Blocklist management UI
- Log viewer (WebSocket streaming)
- OpenAPI/Swagger docs (utoipa)
- Prometheus metrics endpoint

---

## Phase 10 — Migration from *arr Apps

**Goal**: Users can migrate their existing Sonarr/Radarr/Prowlarr setup to StackArr without re-adding everything manually.

**Worktree**: `phase10-migration`

**Depends on**: Phase 9

**Work items**:
- Import from Sonarr SQLite: series, episodes, episode_files, quality profiles, history, blocklist
- Import from Radarr SQLite: movies, movie_files, quality profiles, history, blocklist
- Import from Prowlarr SQLite: indexer definitions
- CLI command: `stackarr migrate --sonarr /path/to/sonarr.db --radarr /path/to/radarr.db`
- Map quality profile IDs between systems
- Preserve history and blocklist entries
- Validate file paths exist on disk

---

## Worktree Parallelism Guide

Phases that can run **concurrently** (no dependency conflicts):

```
Phase 0 ──→ Phase 1 ──→ Phase 2 ──→ Phase 3 ──→ Phase 4 ──→ Phase 5
                                          │                       │
                                          │         Phase 6 ◄─────┤
                                          │         Phase 7 ◄─────┘
                                          │
                              Phase 8 ◄───┘  (can start once API is stable)

Phase 9 ──→ Phase 10  (sequential, after everything else)
```

**Safe parallel pairs** (touch different files):
- Phase 6 + Phase 7 (Indexarr sidecar + embedded clients — different crates)
- Phase 8 + Phase 4/5/6/7 (UI is entirely in `ui/`, backend phases touch `crates/`)
- Phase 2 + early Phase 3 work (metadata is in stackarr-metadata, search is in stackarr-indexer/quality)

**Cannot run in parallel** (same files):
- Phase 3 + Phase 4 (both modify stackarr-web routes and scheduler)
- Phase 4 + Phase 5 (both modify scheduler tasks)
