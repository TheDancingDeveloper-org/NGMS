# StackArr — Implementation Plan

## Current State (updated 2026-03-26)

**29 crates**, 74K lines of Rust, 686 tests passing. React UI with 15 pages, 6K lines TypeScript. Full Docker build + CI/CD pipeline. Deployed to Node B (192.168.1.75).

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
| stackarr-metadata | 100% | TMDB client with rate limiting (4 req/s) + LRU cache (2000 entries, TTL), shared in AppState |
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
| Repo: github.com/TheDancingDeveloper-org/NGMS | Done |
| GHCR: ghcr.io/thedancingdeveloper-org/ngms | Done |

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

## ~~Phase 7 — Indexarr Sidecar Integration~~ COMPLETE

**Goal**: Optional Indexarr as a torrent indexer source.

- ~~Complete IndexarrClient (Torznab passthrough + REST + health)~~ — torznab_search, rest_search, status, health_check all implemented (crates/stackarr-indexer/src/indexarr.rs)
- ~~GET /api/v1/indexarr/status route~~ — exists and works (crates/stackarr-web/src/routes/indexarr.rs)
- ~~docker-compose optional Indexarr service~~ — exists with `profiles: [indexarr]` (docker/docker-compose.yml)
- ~~**7.1** IndexarrClient integrated into search fanout~~ — SearchService includes Indexarr via `with_indexarr()`, /search page in sidebar

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
- ~~GHCR: ghcr.io/thedancingdeveloper-org/ngms~~
- ~~docker-compose.prod.yml with all media mounts READ ONLY~~
- ~~Port 9111~~

---

## Phase 10 — Security Hardening

**Goal**: Secure the stack before any network exposure beyond localhost.

**Status**: NOT STARTED.

**Security audit completed 2026-03-26.** Updated 2026-03-28 with auth middleware, secret redaction, Plex TLS config, and upload validation.

### 10a — Authentication + Authorization (CRITICAL) — DONE

- [x] **10a.1** `require_auth_middleware` applied via `from_fn_with_state` to all 35+ protected routes — validates session cookie, device token, API key, or first-boot bypass
- [x] **10a.2** `/api/v1/setup/init` already protected (checks `enabled_count == 0`)
- [x] **10a.3** `redact_sensitive_fields()` applied to indexer, download client, and Plex server GET/POST/PUT responses — masks API keys, auth tokens, and passwords
- [x] **10a.4** Full user-based auth with session cookies, device tokens, and admin API key — `RequireUser` extractor handles all auth methods

### 10b — CORS + CSRF + Headers (CRITICAL)

- [x] **10b.3** Security response headers already present: `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `X-XSS-Protection`, `Referrer-Policy`, `Permissions-Policy`
- [ ] **10b.1** Replace `CorsLayer` mirror_request() with explicit origin allowlist
- [ ] **10b.2** Add CSRF protection on state-changing endpoints (POST/PUT/DELETE)

### 10c — Input Validation + Error Handling (HIGH) — DONE

- [x] **10c.1** Path traversal — already uses `std::fs::canonicalize()` + `is_dir()` in medialibraryfolders.rs
- [x] **10c.2** URL encoding — already uses `urlencoding::encode()` in newznab.rs
- [x] **10c.3** Error responses — already return generic "internal server error" with full details in `tracing::error!`
- [x] **10c.4** File upload validation — 500 MB size limit + SQLite header check on migrate endpoint

### 10d — TLS + Transport Security (HIGH) — DONE

- [x] **10d.1** Plex TLS — `verify_tls` column added to `plex_servers` (migration 009), configurable per-server, `from_server()` uses `server.verify_tls`, new servers default to `true`
- [x] **10d.2** Usenet NNTP — `ssl_verify` already defaults to `true`
- [x] **10d.3** Usenet credentials — `send_command` trace only logs command verb, not arguments

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
- ~~**11.15** TMDB rate limiting + caching~~ — DONE: shared `TmdbClient` in AppState (rate-limited 4 req/s + 2000-entry LRU cache), used by discover routes and scheduler tasks
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
| ~~WI-4~~ | ~~**Auth middleware on all protected routes** + secret redaction in responses~~ | ~~10a~~ | ~~DONE~~ |
| ~~WI-5~~ | ~~**Input validation** (path traversal, URL encoding, error sanitization, upload validation)~~ | ~~10c~~ | ~~DONE~~ |
| ~~WI-6~~ | ~~**TLS verification** (Plex configurable per-server, Usenet default-on, credential redaction)~~ | ~~10d~~ | ~~DONE~~ |
| WI-7 | **Rate limiting** (wire existing `governor` dep to API middleware) | 10e | Small |
| WI-8 | **CORS allowlist + CSRF + Docker non-root** | 10b,f | Small |
| ~~WI-9~~ | ~~**Indexarr search fanout** (already wired into search flow + /search page)~~ | ~~7~~ | ~~DONE~~ |
| WI-10 | **Integration tests** | 11 | Medium |
| ~~WI-11~~ | ~~**TMDB rate limiting + caching** (shared client in AppState)~~ | ~~11~~ | ~~DONE~~ |
| WI-12 | **Import lists, scene mapping, custom formats** | 11 | Large |
| WI-13 | **Additional notifications** (Telegram, Slack, email) | 11 | Small |
| WI-14 | **OpenAPI/Swagger + Prometheus metrics** (dependencies present, need wiring) | 11 | Medium |

### Security Audit Summary (updated 2026-03-28)

| Category | Severity | Status |
|----------|----------|--------|
| API Authentication | ~~CRITICAL~~ | **FIXED** — `require_auth_middleware` on all protected routes (session, device token, API key, first-boot bypass) |
| CORS | CRITICAL | `mirror_request()` allows any origin (TODO: explicit allowlist) |
| CSRF | CRITICAL | No tokens, no origin validation |
| Path Traversal | ~~HIGH~~ | **OK** — canonicalize + is_dir check |
| TLS Verification | ~~HIGH~~ | **FIXED** — Plex configurable per-server (verify_tls column), Usenet defaults to true |
| Sensitive Data in Responses | ~~HIGH~~ | **FIXED** — `redact_sensitive_fields()` applied to indexer, download client, Plex responses |
| Error Leakage | ~~HIGH~~ | **OK** — Generic "internal server error" returned, full details in tracing::error! |
| Rate Limiting | HIGH | `governor` in Cargo.toml, keyed rate limiter created but not applied to API middleware |
| URL Injection | ~~HIGH~~ | **OK** — `urlencoding::encode()` used on all Newznab params |
| Security Headers | ~~MEDIUM~~ | **PARTIAL** — X-Frame-Options, X-Content-Type-Options, XSS-Protection present; CSP + HSTS missing |
| Docker Root | MEDIUM | No USER directive in Dockerfile |
| Credentials Storage | MEDIUM | Usenet creds in plaintext config |
| File Upload | ~~MEDIUM~~ | **FIXED** — 500 MB size limit + SQLite header validation on migrate endpoint |
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

- GitHub: https://github.com/TheDancingDeveloper-org/NGMS
- GHCR: ghcr.io/thedancingdeveloper-org/ngms
- Deploy: Node B (192.168.1.75), port 9111
