# TODO3 — Media Import + Archive features

Status snapshot from the session that landed the first cut of Feature 1
(import recommendations) and Feature 2 (.torrent/.nzb archival).

Backend: `cargo check --workspace`, `cargo clippy -D warnings` (on touched
crates), and `cargo test -p stackarr-core -p stackarr-download
-p stackarr-import --lib` (137 tests) all green.
Frontend: `npm run build` clean.

---

## Feature 2 — .torrent / .nzb archive (shipped)

- New `[storage.archive]` config section — `crates/stackarr-core/src/config.rs`
  (`StorageConfig`, `ArchiveConfig`, `resolved_*_dir` helpers).
- Default dirs rooted at `{general.data_dir}/archive/{Torrents,Usenet/NZBs,Usenet/NZBs/failed}`.
- NZB archival hook in `EmbeddedUsenetClient::add()`
  (`crates/stackarr-download/src/embedded_usenet.rs`) — saves raw bytes after
  decompress + XML validation, before queue submission. Best-effort; logged
  but never blocks a grab.
- Torrent archival hook in `EmbeddedTorrentClient::add()`
  (`crates/stackarr-download/src/embedded_torrent.rs`) — fetches HTTP(S)
  `.torrent` once, saves it, passes bytes to librtbit via
  `AddTorrent::TorrentFileBytes` to avoid double-download. Magnets skipped
  silently. Post-add, file is renamed to include the info_hash for
  correlation.
- `archive_cleanup` scheduler task in `crates/stackarr-scheduler/src/lib.rs`
  alongside `recycle_bin_cleanup`. Count-based, mtime-sorted. Runs on an
  interval from `ArchiveCleanupConfig`. Emits activity row like other tasks.
- Scheduler wiring in `src/main.rs` — builds `ArchiveCleanupConfig` from
  current `AppConfig`, creates the three dirs at startup.
- Failed NZB move hook in `download_sync_task` — when a queue row transitions
  to `Failed` for a `DownloadProtocol::Usenet` item, calls
  `stackarr_download::embedded_usenet::move_archive_to_failed(nzb_dir,
  failed_dir, download_id)`. Archive paths captured from scheduler struct.
- Read-path + write-path API at `/api/v1/config/storage`
  (`crates/stackarr-web/src/routes/general.rs`). DB keys `archive_*` override
  TOML; PUT returns `restartRequired: true`.
- Settings UI → **Storage / Archive** tab
  (`ui/src/pages/Settings.tsx::StorageTab`). Enable toggle, editable paths
  (with resolved defaults as placeholder), 3 count caps, cleanup interval,
  amber "restart required" banner.

### Decisions locked in
- Global count caps (not per-category/per-indexer).
- Sort key: mtime on disk (simpler; works for failed NZBs that may lack a
  history row).
- Magnet torrents skipped silently — no archive row.
- Failed NZBs stored in a separate `failed/` bucket with its own cap so
  debugging artefacts aren't evicted by normal churn.

### Open follow-ups
- **Hot reload of archive settings.** Currently the `EmbeddedTorrentClient`,
  `EmbeddedUsenetClient`, and the `archive_cleanup` scheduler task all
  capture config snapshots at construction. Changing values via the UI
  persists to DB but takes effect on next app restart. The UI warns the user.
  Proper fix is an `Arc<ArcSwap<Option<PathBuf>>>` inside each client so the
  dirs can be swapped live, plus a config-reload hook that rebuilds the
  scheduler task's interval without restarting the join set.
- **DB-override merge on startup.** `get_storage_config` merges DB over TOML
  on read, but `main.rs` only reads TOML when constructing the scheduler +
  clients. DB-persisted overrides are currently ignored at startup. Needs a
  helper that merges `app_config` rows into `AppConfig` after load, or
  exposes a `merged_archive_config()` getter.
- **Torrent `.torrent` bytes path for archive.** Only HTTP(S) grabs archive.
  Magnets (`magnet:?xt=…`) never persist a file because there's nothing to
  save until librtbit resolves the metadata. If you want magnet-archive
  parity, hook librtbit's metadata-ready event and serialise the info dict
  back to `.torrent` bytes.

---

## Feature 1 — Media import with recommendations (MVP shipped)

### Database
- `migrations/018_import_candidates.sql` — new `import_candidates` table with
  parsed_* + suggested_* fields, status machine (`pending`/`accepted`/
  `rejected`/`ignored`/`failed`), FK to `media_library_folders`,
  `target_series_id`/`target_movie_id`, JSONB `data` column. Partial unique
  index `(discovered_path) WHERE status = 'pending'` so re-runs of the
  scheduler dedupe.

### Core
- `crates/stackarr-core/src/models/import_candidate.rs` — `ImportCandidate`
  struct (FromRow, Serialize camelCase), `NewImportCandidate` input, async
  CRUD: `insert_pending`, `list_pending`, `get`, `update_suggestion`,
  `mark_accepted`, `mark_rejected`, `mark_failed`. Registered in
  `crates/stackarr-core/src/models.rs`.

### Disk scan
- `crates/stackarr-import/src/lib.rs::disk_scan` — kept as back-compat
  wrapper around new `disk_scan_in_folder(pool, media_library_folder_id,
  path, media_type)`.
- `scan_series` — now aggregates unmatched files into
  `UnmatchedSeriesGroup` keyed by folder-name-lowercase. After the walk,
  emits one `import_candidates` row per group with `match_kind` =
  `season` (if one unique season parsed) or `series`. JSONB `data` contains
  per-episode breakdown.
- `scan_movies` — emits one `import_candidates` row per unmatched file
  (`match_kind = "movie"`), with parsed year.

### TMDB match
- `crates/stackarr-import/src/tmdb_match.rs` — `suggest_series` /
  `suggest_movie` (takes parsed title + year, returns
  `Option<TmdbSuggestion>`), `refresh_pending_suggestions` batch helper that
  scans pending rows with `confidence = 0`, calls TMDB, updates rows.
  Scoring = normalised Levenshtein similarity (85%) + year bonus (15%).
  `MIN_CONFIDENCE = 0.45` filters noise. Exported from `stackarr-import`
  lib.rs. 5 unit tests.
- Depends on `stackarr-metadata` — added to `stackarr-import/Cargo.toml`.

### Web routes
- `crates/stackarr-web/src/routes/import_candidates.rs` — new file:
  - `GET /api/v1/import-candidates?mediaType=...&limit=...` — list pending
    ordered by confidence desc, discovered_at desc
  - `POST /api/v1/import-candidates/{id}/accept` — body optionally overrides
    `tmdbId`, `mediaLibraryFolderId`, `qualityProfileId`, `monitored`
  - `POST /api/v1/import-candidates/{id}/reject`
- `accept_series` / `accept_movie` helpers — insert minimal row, inline TMDB
  enrichment (overview, poster/fanart, genres, year, runtime, external ids,
  episodes for all seasons), trigger immediate `disk_scan_in_folder` to link
  discovered files, mark candidate accepted. Failure paths call
  `mark_failed` with the error string.
- Module registered in `routes/mod.rs` and merged in `lib.rs`.

### Scheduler integration
- `scheduled_disk_scan` and the two call sites in
  `crates/stackarr-web/src/routes/system.rs` (initial-setup scan + manual
  scan command) + `medialibraryfolders.rs` (on-add scan) now query
  `(id, path, media_type)` and pass `Some(folder_id)` through
  `disk_scan_in_folder`. The old `disk_scan()` wrapper is still used by the
  "rescan a specific series" path (where the folder id isn't meaningful).

### Frontend
- `ui/src/pages/Import.tsx` — new page. Media-type filter chips
  (All / Series / Movies), refresh button, "Scan library now" button that
  fires `system/command` with `RescanMediaLibrary`. Grid of
  `CandidateCard`s with poster, title, confidence %, file count, total
  size, overview, accept/reject buttons, busy spinners, inline toast.
  Accept is disabled when no TMDB suggestion is set (tells user why).
- Route added to `ui/src/App.tsx` (`/import`) as lazy import.
- Nav link added to `ui/src/components/Sidebar.tsx` under Downloads, using
  the `FolderInput` lucide icon.

### Decisions locked in
- Prefer series-level / season-level recommendations; fall back to
  episode-level only when no series grouping is confident.
- "Add as new media folder vs move into existing" wizard is deferred — user
  said all data lives in media folders, which are already periodically
  scanned.
- Confidence never auto-accepts. User reviews and clicks.
- Accept creates the Series/Movie entity pointing at the discovered_path
  in-place — no file moves. The series path IS the on-disk folder.

### Open follow-ups
- **TMDB refresh isn't scheduled.** `refresh_pending_suggestions` exists and
  is tested but nothing calls it on an interval. Candidates get suggestions
  only if something calls the helper directly. Add a scheduler task
  `import_candidates_tmdb_refresh` (hourly) alongside `importer` that calls
  it, and optionally a manual `POST
  /api/v1/import-candidates/refresh-suggestions` endpoint. ~15 LOC each.
- **Add-folder wizard UI** (tab 2 of the original plan). Would let user
  pick a folder via FileBrowser, preview detected candidates, and choose
  "create new media_library_folder" vs "move files into existing folder".
  Deferred because periodic scan of existing media folders covers the
  common case.
- **Bulk accept** — endpoint `POST /api/v1/import-candidates/bulk-accept`
  with filter params (e.g. `minConfidence`, `mediaType`) for "accept all
  ≥90%". Useful once you have trust in the confidence score.
- **"Register in place" vs "move into canonical layout"** distinction on
  accept. Current implementation only does in-place. For users who want
  Sonarr-style layout, we'd need to wire `stackarr-import::naming` into the
  accept flow.
- **Episode-level fallback emission.** Currently if a series folder parses
  as one unified group, we emit one candidate. If the parser can't infer
  the show title, we still emit a single `series`-kind candidate with
  whatever we have. We never emit per-episode rows today. If users hit
  cases where a single folder mixes multiple shows, we'd want to split.
- **Accept flow assumes `media_library_folder_id` is set on the candidate.**
  Safe because the only scenarios that write candidates now are via
  `disk_scan_in_folder` with a known folder id. The legacy `disk_scan()`
  wrapper (used by the per-series rescan command) doesn't emit candidates
  at all, so there's no null-id path to worry about.

---

## Verification that ran green this session

```bash
cargo check --workspace
cargo clippy -p stackarr-core -p stackarr-download -p stackarr-import \
             -p stackarr-scheduler -p stackarr-web --lib -- -D warnings
cargo test  -p stackarr-core -p stackarr-download -p stackarr-import --lib
# 137 passed; 0 failed

cd ui && npm install && npm run build
# built Import-*.js (8.47 kB / 2.81 kB gz) and updated Settings-*.js
```

Pre-existing clippy warnings in `crates/stackarr-postgres/src/lifecycle.rs`
(`unused_mut`, `unused_variables` on Windows-only code paths around lines
397 and 434) were left alone — they're not from this work and fixing them
is outside scope. CI that runs `clippy -D warnings` on the whole workspace
will surface them.

---

## Suggested next-session shopping list

1. Wire `refresh_pending_suggestions` into the scheduler (hourly task) +
   add a manual-trigger endpoint. Without this, the accept flow shows
   "No TMDB suggestion" on most candidates.
2. Verify the whole flow end-to-end on a real library. Plausible bugs:
   media_files insert may fail when `media_library_folders.path` uses
   Windows backslashes vs the walker's forward slashes (check
   `stackarr-import::scan_series` around `components[0]` path matching).
3. Persistence of storage settings — either build the ArcSwap live-reload
   path, or at minimum have `main.rs` merge `app_config` `archive_*` rows
   over the TOML values before constructing clients + scheduler.
4. Consider whether the `import_candidates` cleanup should prune old
   `accepted`/`rejected` rows (no cleanup today — they'll grow forever).
   Candidate for `recycle_bin_cleanup`-style retention.
