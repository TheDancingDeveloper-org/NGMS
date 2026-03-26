# StackArr — Implementation Plan

## Current State (updated 2026-03-26)

**29 crates**, 74K lines of Rust, 686 tests passing. React UI with 15 pages, 6K lines TypeScript. Full Docker build + CI/CD pipeline. Deployed to Node B (192.168.0.30).

**Architecture change**: rustTorrent and rustnzbd engines vendored directly into the workspace (not sidecars). Single binary includes media management + torrent engine + usenet engine. No external download clients required (but still supported via qBit/Transmission/SABnzbd/NZBGet API clients).

### Crate Status

| Crate | % Done | State |
|-------|--------|-------|
| stackarr-core | 100% | Models, config with defaults, DB pool, migrations, enabled modules |
| stackarr-parser | 100% | Release name parser, 75 tests |
| stackarr-media | 100% | Series/Movie/Episode CRUD + calendar + wanted + metadata refresh services |
| stackarr-web | 95% | All routes working (50+ endpoints), search/grab stubs |
| stackarr-download | 90% | qBit, Transmission, SABnzbd, NZBGet + embedded torrent/usenet clients |
| stackarr-indexer | 85% | Newznab/Torznab client working; Indexarr sidecar stubbed |
| stackarr-metadata | 85% | TMDB client, no caching or rate limiting |
| stackarr-migrate | 100% | Sonarr/Radarr/Prowlarr SQLite readers + Postgres writer with ID mapping |
| stackarr-import | 90% | Disk scan + naming tokens + process_completed_download pipeline |
| stackarr-scheduler | 80% | Real metadata refresh task; import scan task; RSS stub |
| stackarr-notify | 80% | Webhook + Discord; Telegram/Slack/email missing |
| stackarr-quality | 50% | CRUD works; decision engine specs still return "allow" |
| **Torrent engine** (12 crates) | 100% | Vendored from rustTorrent — librtbit, DHT, trackers, bencode, etc. |
| **Usenet engine** (5 crates) | 100% | Vendored from rustnzbd — NNTP, yEnc, par2, post-processing |

### React UI Status

| Page | Status |
|------|--------|
| First Boot Wizard | Done |
| Series List + Detail | Done |
| Movie List + Detail | Done |
| Calendar | Done |
| Queue | Done |
| Torrents (full client UI) | Done |
| Usenet (queue + history + NNTP servers) | Done |
| History | Done |
| Wanted/Missing | Done |
| Settings (7 tabs) | Done |
| Migration | Done |

### Infrastructure

| Component | Status |
|-----------|--------|
| Docker multi-stage build (Node + Rust + slim runtime) | Done |
| docker-compose.yml (dev) | Done |
| docker-compose.prod.yml (Node B with media mounts RO) | Done |
| GitHub Actions CI/CD (check → build → smoke test → deploy) | Done |
| Repo: github.com/AusAgentSmith-org/StackArr | Done |
| GHCR: ghcr.io/ausagentsmith-org/stackarr | Done |

---

## ~~Phase 0 — Make It Boot~~ COMPLETE

- ~~`src/main.rs` — DATABASE_URL env, auto-generate default config~~
- ~~`config.rs` — Default impls, generate_default()~~
- ~~`db.rs` — Migration runner, enabled_modules queries~~
- ~~`docker/Dockerfile` — Multi-stage build~~
- ~~`docker/docker-compose.yml` — StackArr + Postgres 17~~
- ~~`config.example.toml`~~

Verified: `docker compose up --build` → API + UI served on port 9111.

---

## ~~Phase 1 — First-Boot Wizard + Core CRUD~~ COMPLETE

- ~~POST /api/v1/setup/init — persist modules, root folders, API key~~
- ~~GET /api/v1/system/status — real first_boot from DB~~
- ~~CRUD routes: root folders, tags, naming config, download clients, indexers~~
- ~~Health check verifies DB connectivity~~

---

## ~~Phase 2 — *arr Migration + Library Import~~ COMPLETE

- ~~stackarr-migrate crate: sonarr.rs, radarr.rs, prowlarr.rs, writer.rs~~
- ~~CLI: `stackarr migrate --sonarr --radarr --prowlarr [--dry-run]`~~
- ~~API: POST /api/v1/system/migrate (multipart upload)~~
- ~~Disk scan: POST /api/v1/command {"name":"DiskScan"}~~
- ~~TMDB lookup: GET /api/v1/series/lookup, /movies/lookup~~

Real *arr backup DBs at TestData/arr-backups/ (535 series, 1212 movies, 6 indexers).

---

## ~~Phase 3 — Library Views + Metadata Refresh~~ COMPLETE

- ~~GET /api/v1/calendar?start=&end=~~
- ~~GET /api/v1/wanted/missing, /wanted/cutoff~~
- ~~GET /api/v1/series/{id}/episodes~~
- ~~PUT /api/v1/episode/{id}, /episode/monitor (bulk)~~
- ~~CalendarService, WantedService, MetadataRefreshService~~
- ~~metadata_refresh_task — real TMDB integration~~
- ~~Commands: RefreshSeries, RefreshMovie, RefreshAll~~

---

## Phase 4 — Search + Decision Engine + Grab

**Goal**: User can search indexers for a release, the decision engine filters/ranks results, and grabbing sends to a download client.

**Status**: NOT STARTED — this is the next critical phase.

**What needs to happen**:
- Implement decision specifications in stackarr-quality (QualityAllowedSpec, CutoffSpec, SizeSpec, BlocklistSpec, QueueConflictSpec, AlreadyImportedSpec, CustomFormatScoreSpec, MinimumSeedersSpec)
- Add `rank_releases()` — sort by quality, format score, protocol, indexer priority
- Wire up release search handler — look up media IDs → query indexers → run decision engine → return ranked results
- Wire up grab handler — select download client → add to client → insert queue + history records
- Add IndexerManager and DownloadClientManager to AppState
- Initialize managers from DB config on startup

**Estimated scope**: ~1200 lines new, ~300 lines changed

---

## ~~Phase 5 — Download Import Pipeline~~ COMPLETE

- ~~naming.rs — token system ({Series Title}, S{season:00}E{episode:00}, etc.) with 14 tests~~
- ~~process_completed_download() — scan → parse → rename → move → DB update~~
- ~~import_scan_task — polls queue for completed items, runs import, cleans up~~

---

## Phase 6 — RSS Automation

**Goal**: Auto-monitor indexer RSS feeds and grab new releases for monitored media.

**Status**: NOT STARTED. Depends on Phase 4 (needs decision engine).

**What needs to happen**:
- Implement rss_sync_task (fetch RSS → parse → match to media → decision engine → grab)
- Add missing_search_task (periodic search for missing media)
- Command endpoints: RssSync, SeriesSearch, EpisodeSearch, MovieSearch, MissingSearch
- Add `commands` table for async command tracking

---

## Phase 7 — Indexarr Sidecar Integration

**Goal**: Optional Indexarr as a torrent indexer source.

**Status**: NOT STARTED.

**What needs to happen**:
- Complete IndexarrClient (Torznab passthrough + REST + health)
- Integrate into search fanout
- GET /api/v1/indexarr/status route
- docker-compose optional Indexarr service

---

## ~~Phase 8 — Embedded Download Clients~~ SUPERSEDED

~~Original plan: feature-flagged optional embedded clients.~~

**Decision changed**: rustTorrent (12 crates) and rustnzbd (5 crates) vendored directly into the workspace. No feature flags — always built in. Single binary includes everything.

- ~~embedded_torrent.rs wrapping librtbit::Session~~
- ~~embedded_usenet.rs wrapping nzb_web::QueueManager~~
- ~~Feature flags removed~~

**Additional work done beyond original plan**:
- Full Torrents page in React UI (stats, sortable table, add modal, expand details)
- Full Usenet page in React UI (queue + history + NNTP server management)
- API routes: /api/v1/torrent/* and /api/v1/usenet/* (20 endpoints)
- Sidebar nav updated with Torrents + Usenet tabs

**Remaining**: Wire API stubs to actual librtbit::Session and nzb_web::QueueManager in AppState.

---

## ~~Phase 9 — React UI~~ COMPLETE

- ~~15 pages, 6K lines TypeScript~~
- ~~Dark theme, sidebar nav, TanStack Query~~
- ~~First-boot wizard, series/movie CRUD, calendar, queue, history, wanted~~
- ~~Settings (7 tabs), migration upload, torrents, usenet~~
- ~~Axum serves UI dist with SPA fallback~~

---

## ~~CI/CD + Docker~~ COMPLETE

- ~~GitHub Actions: check → build → smoke test → deploy to Node B~~
- ~~GHCR: ghcr.io/ausagentsmith-org/stackarr~~
- ~~docker-compose.prod.yml with all media mounts READ ONLY~~
- ~~Port 9111~~

---

## Phase 10 — Polish + Hardening

**Goal**: Production-ready for daily use.

**Status**: NOT STARTED.

**Work items**:
- Wire embedded torrent/usenet engines to AppState (start session on boot)
- Integration tests (Postgres in Docker, full flow testing)
- API authentication middleware (API key + session auth)
- Real cutoff comparison in wanted/cutoff endpoint (needs quality profile parsing)
- Backup/restore (export/import DB as JSON)
- Health check system (DB, disk space, client connectivity, indexer health)
- Import lists (TMDB popular, Trakt watchlist, IMDB list)
- Disk scan on schedule
- Scene name mapping / XEM integration for anime
- Custom format specification engine (full regex)
- Blocklist management UI
- Log viewer (WebSocket streaming)
- OpenAPI/Swagger docs (utoipa)
- Prometheus metrics endpoint
- Notification providers: Telegram, Slack, email
- TMDB rate limiting / caching

---

## What's Left — Priority Order

| Priority | Work | Phase | Effort |
|----------|------|-------|--------|
| 1 | **Wire embedded engines to AppState** (torrent session + usenet queue start on boot, API stubs → real) | 10 | Medium |
| 2 | **Decision engine** (quality specs, release ranking, cutoff comparison) | 4 | Large |
| 3 | **Search + grab flow** (indexer fanout → decision engine → download client) | 4 | Medium |
| 4 | **RSS automation** (auto-grab from feeds) | 6 | Medium |
| 5 | **API authentication** | 10 | Small |
| 6 | **Indexarr integration** | 7 | Small |
| 7 | **Integration tests** | 10 | Medium |
| 8 | **Import lists, scene mapping, custom formats** | 10 | Large |

---

## Reference: *arr Database Backups

| App | Database Path |
|-----|--------------|
| Sonarr | `TestData/arr-backups/sonarr/sonarr.db` (535 series, 31967 episodes, 9113 files) |
| Radarr | `TestData/arr-backups/radarr/radarr.db` (1212 movies, 1128 files) |
| Prowlarr | `TestData/arr-backups/prowlarr/prowlarr.db` (6 indexers) |

## Repo

- GitHub: https://github.com/AusAgentSmith-org/StackArr
- GHCR: ghcr.io/ausagentsmith-org/stackarr
- Deploy: Node B (192.168.0.30), port 9111
