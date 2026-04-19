# Embedded Download Engines

StackArr ships two embedded download engines as first-class alternatives to external clients (qBittorrent, SABnzbd, etc.). Both run in-process, require no sidecar containers, and integrate directly with the `DownloadClientManager` for automated grab dispatch.

| Engine | Library | Vendored From | Protocol | Synthetic Client ID |
|--------|---------|---------------|----------|---------------------|
| Torrent | `librtbit` | rustTorrent (`crates/torrent/`) | BitTorrent | `-1` |
| Usenet | `nzb-web` / `nzb-core` | rustnzbd (`crates/usenet/`) | NNTP/Usenet | `-2` |

---

## Torrent Engine (librtbit)

### Initialization

The torrent engine starts when `config.torrent.enabled = true` in the TOML config or when the `torrent_embedded` module flag is set in the database. Initialization occurs in two places:

- **Startup** (`src/main.rs`): Creates a `librtbit::Session` with `SessionOptions`, then wraps it in a `librtbit::Api`.
- **Post-setup** (`AppState::init_torrent_engine`): Called after first-boot wizard or module toggle when the engine was not started at boot.

#### SessionOptions

```rust
SessionOptions {
    disable_dht: !cfg.torrent.dht_enabled,         // DHT peer discovery
    completed_folder: Option<PathBuf>,              // Move completed torrents here
    persistence: Some(SessionPersistenceConfig::Json {
        folder: Some(download_dir.join(".session")) // Resume data across restarts
    }),
    fastresume: true,                               // Skip hash-check on restart
    ..Default::default()
}
```

Key behaviors:
- **DHT**: Enabled by default (`dht_enabled = true`). Provides decentralized peer discovery.
- **Persistence**: JSON-based session state stored in `<download_dir>/.session/`. Enables fast resume without re-hashing pieces.
- **Completed folder**: Optional separate directory for finished torrents. Configured via TOML `torrent.complete_dir` or DB key `torrent_complete_dir`.

#### Directory resolution order

1. DB `app_config` key (e.g. `torrent_download_dir`)
2. TOML config (`torrent.download_dir`)
3. Hardcoded default (`/downloads/torrent`)

### State Management

```
AppState {
    torrent_session: ArcSwapOption<librtbit::Session>,
    torrent_api:     ArcSwapOption<librtbit::Api>,
}
```

Both fields use `ArcSwapOption` so the engine can be initialized after startup (e.g. after first-boot setup completes) without restarting the server. Handlers load the current value with `load_full()` and return `503 Service Unavailable` if `None`.

### API Endpoints

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `GET` | `/api/v1/torrent/status` | `torrent_status` | Session stats: speeds, peers, counters, enabled flag |
| `GET` | `/api/v1/torrent/list` | `torrent_list` | All torrents with optional stats |
| `GET` | `/api/v1/torrent/settings` | `torrent_settings_get` | Current engine settings |
| `PUT` | `/api/v1/torrent/settings` | `torrent_settings_update` | Update settings (persists dirs to DB) |
| `POST` | `/api/v1/torrent/add` | `torrent_add` | Add torrent by URL (magnet or .torrent) |
| `POST` | `/api/v1/torrent/add/upload` | `torrent_add_upload` | Add torrent via multipart file upload |
| `GET` | `/api/v1/torrent/{id}` | `torrent_details` | Single torrent detail |
| `GET` | `/api/v1/torrent/{id}/stats` | `torrent_stats` | Per-torrent statistics |
| `POST` | `/api/v1/torrent/{id}/pause` | `torrent_pause` | Pause a torrent |
| `POST` | `/api/v1/torrent/{id}/resume` | `torrent_resume` | Resume a paused torrent |
| `POST` | `/api/v1/torrent/{id}/delete` | `torrent_delete` | Delete torrent (`?deleteFiles=true` removes data) |

#### Torrent identification

Torrents are identified by ID (integer) or info hash (hex string). The `TorrentIdOrHash::parse()` helper accepts either form in path parameters.

#### Settings

Readable and writable at runtime via the settings endpoints:

| Setting | Type | Persisted |
|---------|------|-----------|
| `downloadFolder` | `String` | DB `torrent_download_dir` |
| `completedFolder` | `Option<String>` | DB `torrent_complete_dir` |
| `uploadLimitBps` | `u32` | In-memory only (session ratelimits) |
| `downloadLimitBps` | `u32` | In-memory only (session ratelimits) |
| `peerLimit` | `usize` | In-memory only |
| `concurrentInitLimit` | `usize` | In-memory only |
| `dhtEnabled` | `bool` | Read-only (set at init) |

---

## Usenet Engine (nzb-web)

### Initialization

The usenet engine starts when `config.usenet.enabled = true` AND at least one NNTP server is configured (either in TOML or in the `download_clients` DB table). Initialization occurs in:

- **Startup** (`src/main.rs`): Merges TOML and DB server configs, opens the queue SQLite database, creates a `QueueManager`.
- **Post-setup** (`AppState::init_usenet_engine`): Same logic, triggered after first-boot or when a server is added via the API.

#### Server configuration merge

1. TOML `[[usenet.servers]]` entries are converted to `nzb_core::config::ServerConfig` with IDs like `server-0`, `server-1`, etc.
2. DB rows from `download_clients WHERE client_type = 'embedded_usenet'` are deserialized with IDs like `db-server-{id}`. **Rows with `enabled = false` are skipped** — the engine has no concept of a "loaded but paused" server. Once a `ServerConfig` is handed to `nzb-news`, wrapper workers persistently try to hold connections against it, and the supervisor keeps respawning them on failure; letting a disabled row through wastes connection slots and spams auth failures for credentials the operator has explicitly turned off. The same filter applies in the hot-reload path (`refresh_engine_servers` in `routes/usenet.rs`) that runs on any add/update/delete of a download client.
3. Both lists are concatenated (TOML first, then enabled DB rows).

If the combined list is empty, the engine does not start.

#### Server offline recovery (nzb-news supervisor)

As of `nzb-news 0.1.10`, the engine self-heals from full-pool retirement. When every wrapper worker for a server retires (typically after `MAX_CONSECUTIVE_CONNECT_FAILURES = 3` consecutive terminal-class errors — `Auth`, `AuthRequired`, or `ServiceUnavailable`), the wrapper no longer closes the server's queue permanently. Instead it sends an `AllWrappersExited` message; the scheduler's per-server supervisor schedules a respawn after an exponential cooldown (30s → 60s → … → 600s cap, reset after a 5-minute healthy window). Log lines to watch: `supervisor: server offline, scheduling respawn` and `supervisor: respawning wrapper pool`. Prior to 0.1.10 the same path called `queue.close()` and the server stayed latched offline for the process lifetime.

#### QueueManager creation

```rust
QueueManager::new(
    nzb_servers,          // Merged server list
    nzb_db,               // SQLite DB at <incomplete_dir>/usenet_queue.db
    incomplete_dir,       // Working directory for active downloads
    complete_dir,         // Destination for finished downloads
    log_buffer,           // Per-job log capture
    max_active_downloads, // Concurrent download limit
    Vec::new(),           // Categories (empty at init)
    0,                    // Speed limit (0 = unlimited)
    0,                    // History retention (0 = unlimited)
    direct_unpack,        // Extract RARs during download
)
```

After creation:
- `restore_from_db()` resumes any interrupted jobs from the SQLite queue database.
- `spawn_speed_tracker()` starts background speed measurement.

#### Directory resolution order

1. DB `app_config` key (e.g. `usenet_incomplete_dir`, `usenet_complete_dir`)
2. TOML config (`usenet.incomplete_dir`, `usenet.complete_dir`)
3. Hardcoded defaults (`/downloads/usenet/incomplete`, `/downloads/usenet/complete`)

Directories are created automatically if they do not exist.

### State Management

```
AppState {
    usenet_queue: ArcSwapOption<nzb_web::QueueManager>,
}
```

Same `ArcSwapOption` pattern as the torrent engine. The usenet engine can additionally be started dynamically when the first NNTP server is added via the API -- the `usenet_servers_add` handler calls `init_usenet_engine()` if the queue is `None`.

### API Endpoints

#### Queue management

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `GET` | `/api/v1/usenet/status` | `usenet_status` | Speed, queue size, active downloads, pause state |
| `GET` | `/api/v1/usenet/queue` | `usenet_queue` | All queued jobs with progress, speed, ETA |
| `GET` | `/api/v1/usenet/queue/{id}` | `usenet_queue_detail` | Single job with file list and logs |
| `POST` | `/api/v1/usenet/add` | `usenet_add` | Add NZB by URL (auto-decompresses gzip) |
| `POST` | `/api/v1/usenet/add/upload` | `usenet_add_upload` | Add NZB via multipart file upload |
| `POST` | `/api/v1/usenet/queue/{id}/pause` | `usenet_queue_pause` | Pause a single job |
| `POST` | `/api/v1/usenet/queue/{id}/resume` | `usenet_queue_resume` | Resume a single job |
| `POST` | `/api/v1/usenet/queue/{id}/delete` | `usenet_queue_delete` | Remove a job from the queue |

#### Queue-wide controls

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `POST` | `/api/v1/usenet/pause-all` | `usenet_pause_all` | Pause entire queue (optional `durationSecs`) |
| `POST` | `/api/v1/usenet/resume-all` | `usenet_resume_all` | Resume entire queue |
| `POST` | `/api/v1/usenet/speed-limit` | `usenet_speed_limit` | Set global speed limit (0 = unlimited) |

#### History

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `GET` | `/api/v1/usenet/history` | `usenet_history` | Completed/failed jobs with post-processing stage results |
| `GET` | `/api/v1/usenet/history/{id}` | `usenet_history_detail` | Single history entry with logs |
| `POST` | `/api/v1/usenet/history/{id}/retry` | `usenet_history_retry` | Re-download from stored NZB data |

#### NNTP server management

Servers are persisted in the `download_clients` table (`client_type = 'embedded_usenet'`) and hot-reloaded into the running engine after every mutation via `refresh_engine_servers()`.

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `GET` | `/api/v1/usenet/servers` | `usenet_servers_list` | List all configured NNTP servers |
| `POST` | `/api/v1/usenet/servers` | `usenet_servers_add` | Add a new server (starts engine if not running) |
| `PUT` | `/api/v1/usenet/servers/{id}` | `usenet_servers_update` | Update server config (partial merge) |
| `DELETE` | `/api/v1/usenet/servers/{id}` | `usenet_servers_delete` | Remove a server |
| `POST` | `/api/v1/usenet/servers/test` | `usenet_servers_test_body` | Test connection from request body (pre-save) |
| `POST` | `/api/v1/usenet/servers/{id}/test` | `usenet_servers_test` | Test a saved server (accepts optional overrides) |

Server test endpoints create a temporary `NntpConnection`, attempt connect + authenticate within a 15-second timeout, then disconnect. Passwords are masked (`********`) in all list/detail responses.

#### Settings

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `GET` | `/api/v1/usenet/settings` | `usenet_settings_get` | Current engine settings |
| `PUT` | `/api/v1/usenet/settings` | `usenet_settings_update` | Update settings (persists to DB) |

Writable settings: `maxActiveDownloads`, `speedLimit`, `historyRetention`, `incompleteDir`, `completeDir`.

#### SABnzbd import

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| `POST` | `/api/v1/usenet/import-sabnzbd` | `import_sabnzbd_ini` | Upload `sabnzbd.ini`, returns preview |
| `POST` | `/api/v1/usenet/import-sabnzbd-api` | `import_sabnzbd_api` | Fetch config from running SABnzbd via API |
| `POST` | `/api/v1/usenet/import-sabnzbd/apply` | `import_sabnzbd_apply` | Apply previewed import (servers + categories) |

The import flow is two-step: parse/preview first, then apply. Masked passwords are rejected at apply time -- the user must enter them manually.

---

## Engine Lifecycle

### Startup flow

1. Load `EnabledModules` from DB (includes `torrent_embedded`, `usenet_embedded` flags).
2. If the module is enabled in DB but not in TOML, override TOML config to `enabled = true`.
3. Initialize each engine if enabled and properly configured.
4. Store engine handles in `ArcSwapOption` fields on `AppState`.
5. Register embedded clients in `DownloadClientManager` with synthetic IDs.

### ArcSwapOption pattern

Both engines use `ArcSwapOption<T>` rather than `Option<T>` so they can be initialized (or replaced) after the server is already running:

```rust
// Check if engine is available
let api = state.torrent_api.load_full();
match &api {
    Some(api) => { /* use it */ },
    None => { /* return 503 */ },
}

// Initialize later (e.g. after first-boot setup)
state.init_torrent_engine().await;
```

The `init_*` methods are idempotent -- they check `load().is_some()` and return early if the engine is already running.

### Graceful shutdown

The HTTP server uses `axum::serve` with `with_graceful_shutdown(shutdown_signal())`, which listens for `CTRL+C`. When the signal fires, the Axum server stops accepting new connections and drains in-flight requests. The `Arc<AppState>` (and thus both engine handles) is dropped when all references go out of scope.

The torrent engine's `Session` uses a `CancellationToken` internally -- the `EmbeddedTorrentClient::test()` method checks `session.cancellation_token().is_cancelled()` to verify liveness.

---

## Integration with DownloadClientManager

Both embedded engines implement the `DownloadClient` trait and are registered in the shared `DownloadClientManager` alongside any external clients (qBittorrent, SABnzbd, NZBGet, etc.).

### Registration

```rust
// Torrent: synthetic ID -1
let client = EmbeddedTorrentClient::new(api);
mgr.add_client(-1, Box::new(client), priority);

// Usenet: synthetic ID -2
let client = EmbeddedUsenetClient::new(Arc::clone(&queue));
mgr.add_client(-2, Box::new(client), priority);
```

Negative IDs distinguish embedded clients from DB-stored external clients (which have positive auto-increment IDs). Priority is read from DB `app_config` keys `embedded_torrent_priority` and `embedded_usenet_priority` (default `0`).

### Grab dispatch

When the scheduler or a manual search triggers a grab, `DownloadClientManager::grab()` selects the highest-priority client matching the required protocol. If the embedded torrent client has priority `0` and an external qBittorrent client has priority `1`, the embedded engine is preferred.

### EmbeddedTorrentClient

- `add()`: Calls `api_add_torrent()` with the download URL.
- `get_items()`: Maps librtbit torrent states (`Live`, `Paused`, `Error`, `Initializing`) to `DownloadItemStatus`.
- `remove()`: `api_torrent_action_delete` (with data) or `api_torrent_action_forget` (metadata only).
- `test()`: Verifies the session is not cancelled, has a listen address, and reports non-zero uptime.

### EmbeddedUsenetClient

- `add()`: Fetches NZB from URL, auto-decompresses gzip, parses XML, adds job to the queue. If `GrabRequest.password` is set (from the indexer API), it overrides the NZB metadata password on the job.
- `get_items()`: Returns active queue jobs plus recent history entries (last 10 minutes) so the import scheduler can see completed items before they age out of the in-memory queue.
- `remove()`, `pause()`, `resume()`: Delegate directly to `QueueManager`.
- `test()`: Verifies at least one server is configured.

### Post-Processing Pipeline (nzb-postproc)

After download completion, the usenet engine runs a post-processing pipeline via `nzb_postproc::run_pipeline()`:

1. **Verify** — Native PAR2 verification using `rust-par2`. Skipped when `articles_failed == 0` (files are CRC-verified during yEnc decode). Includes PAR2-guided deobfuscation: renames obfuscated files to match PAR2 expected names via MD5-16k hash matching.
2. **Repair** — Native PAR2 repair if verification found damaged/missing files. Uses pre-computed verify result to avoid redundant passes.
3. **Extract** — Unpack RAR, 7z, and ZIP archives. Skipped if direct unpack already handled extraction during download.
4. **Cleanup** — Remove par2 files, RAR volumes, and split 7z volumes after successful extraction.

#### Archive Extraction

Extraction shells out to external binaries (installed in the Docker image):

| Format | Primary Tool | Fallback | Docker Package |
|--------|-------------|----------|----------------|
| RAR (4 & 5) | `unrar` | `7z` | `unrar` (non-free) |
| 7z | `7z` / `7zz` / `7za` | — | `p7zip-full` |
| ZIP | Rust `zip` crate | — | _(none — pure Rust)_ |

Binary search order for RAR: `unrar` → `unrar-free` → `rar` → fallback to `7z`.
Binary search order for 7z: `7z` → `7zz` → `7za`.

#### Archive Passwords

Passwords flow through the system from two sources:

1. **Indexer API**: Newznab `<newznab:attr name="password" value="..."/>` — extracted during search and passed via `ReleaseInfo.password` → `GrabRequest.password` → `NzbJob.password`.
2. **NZB metadata**: `<meta type="password">value</meta>` in the NZB XML — extracted by the NZB parser into `NzbJob.password`.

API-provided passwords override NZB metadata passwords when both are present.

The password is passed to extractors as `-p<password>` (unrar/7z). When no password is set, `-p-` is used to suppress interactive prompts and fail immediately on encrypted archives. All subprocess calls use `stdin(Stdio::null())` to prevent hanging.

#### Direct Unpack

When `direct_unpack = true` (default), RAR volumes are extracted during download as each volume completes assembly. This overlaps extraction with download, reducing total processing time. The direct unpacker spawns `unrar x -vp` (pause between volumes) and feeds volumes as they become ready.

If direct unpack fails (e.g. article failures corrupt a volume), the pipeline falls back to normal PAR2 repair + extract.

---

## Configuration

### TOML `[torrent]` section

```toml
[torrent]
enabled = true
download_dir = "/downloads/torrent"
complete_dir = "/downloads/torrent/complete"
listen_port = 6881
dht_enabled = true
peer_limit = 200
upload_limit_bps = 0
download_limit_bps = 0
```

### TOML `[usenet]` section

```toml
[usenet]
enabled = true
incomplete_dir = "/downloads/usenet/incomplete"
complete_dir = "/downloads/usenet/complete"
max_active_downloads = 3
direct_unpack = true

[[usenet.servers]]
name = "Primary"
host = "news.example.com"
port = 563
ssl = true
username = "user"
password = "pass"
connections = 20
priority = 0
```

### DB `app_config` keys

| Key | Type | Engine | Description |
|-----|------|--------|-------------|
| `torrent_download_dir` | `string` | Torrent | Active download directory |
| `torrent_complete_dir` | `string` | Torrent | Completed torrent directory |
| `embedded_torrent_priority` | `integer` | Torrent | Priority in DownloadClientManager |
| `usenet_incomplete_dir` | `string` | Usenet | Working directory for active downloads |
| `usenet_complete_dir` | `string` | Usenet | Destination for finished downloads |
| `usenet_max_active_downloads` | `integer` | Usenet | Concurrent download limit |
| `embedded_usenet_priority` | `integer` | Usenet | Priority in DownloadClientManager |
| `usenet_categories` | `json array` | Usenet | Categories (imported from SABnzbd) |

### DB `download_clients` table

Usenet NNTP servers are stored as rows with `client_type = 'embedded_usenet'`. Each row's `config` column holds a JSON-serialized `ServerConfig`. The `enabled` and `priority` columns are authoritative (overriding any values inside the JSON blob). Server mutations via the API trigger `refresh_engine_servers()` which calls `QueueManager::update_servers()` to hot-reload the server list without restarting the engine.

---

## Source Files

| File | Purpose |
|------|---------|
| `crates/stackarr-web/src/state.rs` | `AppState` struct, `init_torrent_engine`, `init_usenet_engine` |
| `crates/stackarr-web/src/routes/torrent.rs` | Torrent API route handlers and router |
| `crates/stackarr-web/src/routes/usenet.rs` | Usenet API route handlers, server CRUD, SABnzbd import |
| `crates/stackarr-download/src/embedded_torrent.rs` | `EmbeddedTorrentClient` (DownloadClient impl) |
| `crates/stackarr-download/src/embedded_usenet.rs` | `EmbeddedUsenetClient` (DownloadClient impl) |
| `crates/stackarr-download/src/manager.rs` | `DownloadClientManager` (priority-based dispatch) |
| `crates/stackarr-core/src/config.rs` | `TorrentConfig`, `UsenetConfig`, `EnabledModules` |
| `src/main.rs` | Startup initialization and engine registration |
| `crates/torrent/` | Vendored librtbit (12 crates) |
| `crates/usenet/` | Vendored nzb engine (5 crates) |
