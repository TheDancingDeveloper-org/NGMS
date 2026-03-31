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
| stackarr-web | 100% | All routes working (50+ endpoints), search/grab fully wired |
| stackarr-download | 90% | qBit, Transmission, SABnzbd, NZBGet + embedded torrent/usenet clients; DownloadClientManager exists but not in AppState |
| stackarr-indexer | 90% | Newznab/Torznab client working; IndexarrClient substantially complete; IndexerManager implemented but not in AppState |
| stackarr-metadata | 85% | TMDB client, no caching or rate limiting |
| stackarr-migrate | 100% | Sonarr/Radarr/Prowlarr SQLite readers + Postgres writer with ID mapping |
| stackarr-import | 90% | Disk scan + naming tokens + process_completed_download pipeline |
| stackarr-scheduler | 80% | Real metadata refresh task; import scan task; RSS sync is no-op stub; missing_search_task absent |
| stackarr-notify | 80% | Webhook + Discord; Telegram/Slack/email missing |
| stackarr-quality | 100% | CRUD works; 9/9 specs implemented with real logic; 46 tests passing |
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

## ~~Phase 4 — Search + Decision Engine + Grab~~ COMPLETE

- ~~All 9 decision specifications implemented~~ (QualityAllowedSpec, CutoffSpec, MinSizeSpec, MaxSizeSpec, BlocklistSpec, QueueConflictSpec, MinimumSeedersSpec, CustomFormatScoreSpec, AlreadyImportedSpec)
- ~~`rank_releases()` — quality → seeders → age → indexer priority~~
- ~~IndexerManager + DownloadClientManager in AppState, loaded from DB at startup~~
- ~~Search handler: indexer fanout → decision engine → ranking → JSON response~~
- ~~Grab handler: download client dispatch → queue entry → history entry~~
- ~~46 quality tests passing~~

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
- **6.1** Implement rss_sync_task (fetch RSS → parse → match to media → decision engine → grab)
- **6.2** Add missing_search_task (periodic search for missing media)
- **6.3** Command endpoints: RssSync, SeriesSearch, EpisodeSearch, MovieSearch, MissingSearch
- **6.4** Add `commands` table for async command tracking

---

## Phase 7 — Indexarr Sidecar Integration

**Goal**: Optional Indexarr as a torrent indexer source.

**Status**: ~75% COMPLETE — client and infrastructure done, search integration remaining.

**Done**:
- ~~Complete IndexarrClient (Torznab passthrough + REST + health)~~ — torznab_search, rest_search, status, health_check all implemented (crates/stackarr-indexer/src/indexarr.rs)
- ~~GET /api/v1/indexarr/status route~~ — exists and works (crates/stackarr-web/src/routes/indexarr.rs)
- ~~docker-compose optional Indexarr service~~ — exists with `profiles: [indexarr]` (docker/docker-compose.yml)

**What still needs to happen**:
- **7.1** Integrate IndexarrClient into search fanout (depends on Phase 4 search handler)

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
- ~~Wire API stubs to actual librtbit::Session and nzb_web::QueueManager in AppState~~ — engines initialized on boot if enabled in config (src/main.rs:179-293)

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

## Phase 10 — Security Hardening

**Goal**: Secure the stack before any network exposure beyond localhost.

**Status**: NOT STARTED.

**Security audit completed 2026-03-26.** Current state: zero authentication on all 50+ API endpoints, permissive CORS, no rate limiting. The system is fully open to anyone with network access.

### 10a — Authentication + Authorization (CRITICAL)

- [ ] **10a.1** Add auth middleware to all API routes — validate API key from `Authorization` header (key already generated at first boot but never checked)
- [ ] **10a.2** Protect `/api/v1/setup/init` — currently the only endpoint with any access control (checks `enabled_count == 0`)
- [ ] **10a.3** Stop returning sensitive data in GET responses — indexer API keys, Plex auth tokens, and download client credentials are all returned verbatim; mask or redact them
- [ ] **10a.4** Session/token auth for the UI (the API key alone is insufficient for browser-based auth)

### 10b — CORS + CSRF + Headers (CRITICAL)

- [ ] **10b.1** Replace `CorsLayer::permissive()` (`crates/stackarr-web/src/lib.rs:39`) with explicit origin allowlist
- [ ] **10b.2** Add CSRF protection on state-changing endpoints (POST/PUT/DELETE)
- [ ] **10b.3** Add security response headers: `Content-Security-Policy`, `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Strict-Transport-Security`

### 10c — Input Validation + Error Handling (HIGH)

- [ ] **10c.1** Path traversal — canonicalize and boundary-check user-supplied paths in media library folder CRUD (`crates/stackarr-web/src/routes/medialibraryfolders.rs:98` calls `std::fs::metadata(&body.path)` without canonicalization)
- [ ] **10c.2** URL encoding — Newznab query builder (`crates/stackarr-indexer/src/newznab.rs:113-122`) concatenates parameters without encoding; use `serde_urlencoded` or `url::Url`
- [ ] **10c.3** Sanitize error responses — stop returning raw `format!("database error: {e}")` to clients; log full errors server-side, return generic messages to clients (affects indexers.rs, downloadclients.rs, episodes.rs, system.rs, plex.rs, discover.rs)
- [ ] **10c.4** Validate file type/size on `/api/v1/system/migrate` upload endpoint

### 10d — TLS + Transport Security (HIGH)

- [ ] **10d.1** Plex API (`crates/stackarr-plex/src/api.rs:20`) — `danger_accept_invalid_certs(true)` is hardcoded; make configurable, default to verifying certs, allow user opt-out for self-signed setups
- [ ] **10d.2** Usenet NNTP (`crates/usenet/nzb-nntp/src/connection.rs`) — `NoVerifier` accepts any cert, `ssl_verify` defaults to `false`; flip default to `true`
- [ ] **10d.3** Usenet credentials sent via `AUTHINFO PASS` should never appear in logs

### 10e — Rate Limiting (HIGH)

- [ ] **10e.1** Add rate limiting middleware — `governor` is already in Cargo.toml but unused on the main API
- [ ] **10e.2** Priority: auth endpoints first, then general API

### 10f — Docker + Deployment (MEDIUM)

- [ ] **10f.1** Add `USER` directive to Dockerfile — currently runs as root
- [ ] **10f.2** Audit docker-compose credential handling — database URL with `stackarr:stackarr` in plain text

---

## Phase 11 — Polish + Features

**Goal**: Production-ready for daily use.

**Status**: NOT STARTED.

**Work items**:
- ~~**11.1** Wire embedded torrent/usenet engines to AppState (start session on boot)~~ — DONE (src/main.rs:179-293)
- **11.2** Integration tests (Postgres in Docker, full flow testing)
- **11.3** Real cutoff comparison in wanted/cutoff endpoint (needs quality profile parsing)
- **11.4** Backup/restore (export/import DB as JSON)
- **11.5** Health check system (DB, disk space, client connectivity, indexer health)
- **11.6** Import lists (TMDB popular, Trakt watchlist, IMDB list)
- **11.7** Disk scan on schedule
- **11.8** Scene name mapping / XEM integration for anime
- **11.9** Custom format specification engine (full regex)
- **11.10** Blocklist management UI
- **11.11** Log viewer (WebSocket streaming)
- **11.12** OpenAPI/Swagger docs (utoipa)
- **11.13** Prometheus metrics endpoint
- **11.14** Notification providers: Telegram, Slack, email
- **11.15** TMDB rate limiting / caching

---

## What's Left — Priority Order

| Priority | Work | Phase | Effort |
|----------|------|-------|--------|
| ~~WI-1~~ | ~~**Finish decision engine** (AlreadyImportedSpec, CustomFormatScoreSpec, managers in AppState)~~ | ~~4~~ | ~~DONE~~ |
| ~~WI-2~~ | ~~**Wire search + grab handlers** (indexer fanout → decision engine → download client → queue/history)~~ | ~~4~~ | ~~DONE~~ |
| WI-3 | **RSS automation** (rss_sync_task, missing_search_task, command endpoints, commands table) | 6 | Medium |
| WI-4 | **Auth + CORS + CSRF** (middleware on all routes, replace permissive CORS, add CSRF tokens) | 10a-b | Medium |
| WI-5 | **Input validation + error sanitization** (path traversal, URL encoding, generic error responses) | 10c | Small |
| WI-6 | **TLS verification defaults** (Plex configurable, Usenet default-on, credential redaction in logs) | 10d | Small |
| WI-7 | **Rate limiting** (wire existing `governor` dep to API middleware) | 10e | Small |
| WI-8 | **Security headers + Docker non-root** | 10b,f | Small |
| WI-9 | **Indexarr search fanout** (wire IndexarrClient into search flow) | 7 | Small |
| WI-10 | **Integration tests** | 11 | Medium |
| WI-11 | **TMDB rate limiting + caching** | 11 | Small |
| WI-12 | **Import lists, scene mapping, custom formats** | 11 | Large |
| WI-13 | **Additional notifications** (Telegram, Slack, email) | 11 | Small |
| WI-14 | **OpenAPI/Swagger + Prometheus metrics** (dependencies present, need wiring) | 11 | Medium |

### Security Audit Summary (2026-03-26)

| Category | Severity | Status |
|----------|----------|--------|
| API Authentication | CRITICAL | None — API key generated but never validated |
| CORS | CRITICAL | `CorsLayer::permissive()` allows any origin |
| CSRF | CRITICAL | No tokens, no origin validation |
| Path Traversal | HIGH | User paths not canonicalized |
| TLS Verification | HIGH | Disabled for Plex (hardcoded) and Usenet (default off) |
| Sensitive Data in Responses | HIGH | API keys, Plex tokens, credentials returned in GET |
| Error Leakage | HIGH | Raw DB errors returned to clients |
| Rate Limiting | HIGH | `governor` in Cargo.toml but unused |
| URL Injection | HIGH | Newznab query params not URL-encoded |
| Security Headers | MEDIUM | No CSP, X-Frame-Options, HSTS |
| Docker Root | MEDIUM | No USER directive in Dockerfile |
| Credentials Storage | MEDIUM | Usenet creds in plaintext config |
| File Upload | MEDIUM | No type validation on migrate endpoint |
| **Frontend XSS** | **OK** | No dangerouslySetInnerHTML, eval, innerHTML |
| **SQL Injection** | **OK** | Parameterized sqlx throughout |
| **Dependencies** | **OK** | All current, no known CVEs |
| **Cryptography** | **OK** | Proper libraries (rustls, rand 0.9, aws-lc-rs) |
| **.gitignore** | **OK** | .env, config.toml, secrets excluded |
| **Committed Secrets** | **OK** | None found |

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
