# Download Clients & Import Pipeline

## Download Client Abstraction

### Trait

```rust
#[async_trait]
pub trait DownloadClient: Send + Sync {
    fn name(&self) -> &str;
    fn protocol(&self) -> DownloadProtocol;
    async fn add(&self, request: &GrabRequest) -> anyhow::Result<String>;     // Returns download ID
    async fn get_items(&self) -> anyhow::Result<Vec<DownloadItem>>;
    async fn remove(&self, id: &str, delete_data: bool) -> anyhow::Result<()>;
    async fn pause(&self, id: &str) -> anyhow::Result<()>;
    async fn resume(&self, id: &str) -> anyhow::Result<()>;
    async fn test(&self) -> anyhow::Result<()>;
    async fn status(&self) -> anyhow::Result<ClientStatus>;
}
```

### Manager

```rust
pub struct DownloadClientManager {
    clients: Vec<(i64, Box<dyn DownloadClient>)>,  // (client_id, client)
}

impl DownloadClientManager {
    pub fn add_client(&mut self, id: i64, client: Box<dyn DownloadClient>);
    pub async fn grab(&self, request: &GrabRequest) -> Result<(i64, String)>;
    pub async fn get_items_all(&self) -> Vec<(i64, Vec<DownloadItem>)>;
}
```

`grab()` selects the first enabled client matching the protocol (usenet or torrent), ordered by priority.

### Supported Clients

| Client | Protocol | Type |
|--------|----------|------|
| librtbit | Torrent | Embedded |
| nzb-web | Usenet | Embedded |
| Transmission | Torrent | External (HTTP API) |
| qBittorrent | Torrent | External (HTTP API) |
| SABnzbd | Usenet | External (HTTP API) |
| NZBGet | Usenet | External (HTTP API) |

### GrabRequest
```rust
pub struct GrabRequest {
    pub title: String,
    pub download_url: String,
    pub protocol: DownloadProtocol,
    pub category: Option<String>,
}
```

### DownloadItem
```rust
pub struct DownloadItem {
    pub id: String,
    pub title: String,
    pub status: DownloadStatus,
    pub progress: f64,          // 0.0 - 1.0
    pub size: i64,
    pub remaining: i64,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
}
```

---

## Import Pipeline

### Overview

```
Download completes
  → Scheduler detects completion (1 min poll)
  → process_completed_download(ctx)
    → Scan output folder for media files
    → Filter out samples (< 50MB with "sample" in name)
    → For each file:
        → parse_release(filename) → quality, languages, group
        → Load naming config from DB
        → Build destination path using naming tokens
        → Move/rename file to library folder
        → Create media_file record
        → Link to episode/movie
        → Create history record (Imported)
  → Return ImportResult
```

### Key Functions

#### process_completed_download
```rust
pub async fn process_completed_download(ctx: ImportContext) -> Result<ImportResult>
```

**ImportContext**:
```rust
pub struct ImportContext {
    pub pool: PgPool,
    pub download_id: String,
    pub output_path: PathBuf,        // Where the download client put the files
    pub media_type: String,          // "series" or "movie"
    pub media_id: i64,               // Series or movie ID
    pub episode_id: Option<i64>,     // For episode-specific grabs
}
```

**ImportResult**:
```rust
pub struct ImportResult {
    pub imported_files: Vec<ImportedFile>,
    pub skipped_files: Vec<String>,
    pub errors: Vec<String>,
}

pub struct ImportedFile {
    pub source_path: String,
    pub dest_path: String,
    pub media_file_id: i64,
    pub quality: String,
    pub size: i64,
}
```

#### disk_scan
```rust
pub async fn disk_scan(pool: &PgPool, root_path: &Path, media_type: &str) -> Result<DiskScanResult>
```

Scans an entire media library folder:
1. Walks the directory tree
2. Matches folders to series/movies by `clean_title`
3. Creates `media_file` records for untracked files
4. Links files to episodes via `episode_files` junction

Expected directory structure:
```
/media/TV1/
  Breaking Bad/
    Season 01/
      Breaking.Bad.S01E01.720p.BluRay.x264-GROUP.mkv
      Breaking.Bad.S01E02.720p.BluRay.x264-GROUP.mkv
    Season 02/
      ...
```

**DiskScanResult**:
```rust
pub struct DiskScanResult {
    pub files_found: usize,
    pub files_matched: usize,
    pub files_unmatched: usize,
    pub files_already_tracked: usize,
    pub unmatched_files: Vec<String>,
}
```

### File Operations

**move_file(src, dest)**: Attempts `rename()` first. If cross-device (EXDEV error), falls back to `copy() + remove()`.

**is_media_extension(ext)**: Accepted extensions: `mkv`, `mp4`, `avi`, `wmv`, `ts`, `m4v`, `flv`, `mov`, `webm`.

**is_sample(path, size)**: Files < 50 MB with "sample" in the filename are filtered out.

### File Naming Engine

Located in `stackarr-import/src/naming.rs`.

#### Episode Naming
```rust
pub fn build_episode_filename(
    format: &str,
    series_title: &str,
    season: i32,
    episode: i32,
    episode_title: Option<&str>,
    quality: &str,
    release_group: Option<&str>,
    absolute: Option<i32>,
) -> String
```

**Tokens**:
| Token | Replacement |
|-------|-------------|
| `{Series Title}` | Series title |
| `{season:00}` | Season number, zero-padded |
| `{episode:00}` | Episode number, zero-padded |
| `{Episode Title}` | Episode title (empty string if None) |
| `{Quality Title}` | Quality name |
| `{Release Group}` | Group name (empty string if None) |
| `{Absolute Episode:000}` | Absolute number, 3-digit padded |

#### Movie Naming
```rust
pub fn build_movie_filename(
    format: &str,
    title: &str,
    year: Option<i32>,
    quality: &str,
    edition: Option<&str>,
    release_group: Option<&str>,
) -> String
```

**Tokens**: `{Movie Title}`, `{Release Year}`, `{Quality Title}`, `{Edition Tags}`, `{Release Group}`

#### Season Folder
```rust
pub fn build_season_folder(format: &str, season: i32) -> String
```

**Token**: `{season:00}` with zero-padding.

#### Filename Sanitization
```rust
pub fn sanitize_filename(name: &str, colon_replacement: &str) -> String
```

Removes: `/ \ * ? " < > |`
Handles colons via strategy: `smart`, `dash`, `space`, `spacedash`, or remove.
Collapses multiple spaces, trims.

### Database Updates on Import

When a file is imported:
1. `INSERT INTO media_files` — quality, languages, scene_name, release_group, size
2. `INSERT INTO episode_files` — links episode to media_file
3. `UPDATE episodes SET episode_file_id = ...` — marks episode as having a file
4. `INSERT INTO history` — event_type = 'imported', source_title, quality, download_id

### Naming Config (from DB)

```sql
SELECT rename_files, standard_format, daily_format, anime_format,
       season_folder_format, colon_replacement
FROM naming_config
WHERE media_type = $1
```

Defaults if missing:
- `rename_files`: true
- `standard_format`: `{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]`
- `season_folder_format`: `Season {season:00}`
- `movie_format`: `{Movie Title} ({Release Year}) [{Quality Title}]`
- `colon_replacement`: `smart`

---

## Embedded Engine Integration

### Torrent (librtbit)

Initialized in `src/main.rs` when `config.torrent.enabled = true`:

```rust
let session = librtbit::Session::new_with_opts(
    download_dir,
    librtbit::SessionOptions {
        listen_port_range: Some(port..port+1),
        dht: if dht_enabled { librtbit::DhtConfig::On { port } } else { librtbit::DhtConfig::Off },
        peer_opts: librtbit::PeerOpts { max_peers: peer_limit },
        ..Default::default()
    }
).await?;

let api = session.api();
```

The `api` handle is stored in AppState and used by:
- `/api/v1/torrent/*` routes for user-facing torrent management
- `DownloadClient` trait impl for the grab pipeline

### Usenet (nzb-web)

Initialized in `src/main.rs` when `config.usenet.enabled = true`:

```rust
let queue_manager = nzb_web::QueueManager::new(
    nzb_core::NzbConfig {
        incomplete_dir: config.usenet.incomplete_dir.clone(),
        complete_dir: config.usenet.complete_dir.clone(),
        servers: config.usenet.servers.iter().map(|s| nzb_core::ServerConfig { ... }).collect(),
        max_active: config.usenet.max_active_downloads,
    },
    db_path,
).await?;
```

The `queue_manager` is stored in AppState and used by:
- `/api/v1/usenet/*` routes for user-facing NZB management
- `DownloadClient` trait impl for the grab pipeline

---

## Complete Grab Flow

```
1. User clicks "Grab" or RSS auto-selects a release
   ↓
2. ReleaseInfo with download_url and protocol
   ↓
3. DownloadClientManager::grab(GrabRequest { url, protocol, ... })
   → Selects client by protocol + priority
   → Calls client.add(request)
   → Returns (client_id, download_id)
   ↓
4. INSERT INTO queue (media_type, media_id, episode_id, title, quality,
                      status='queued', download_id, download_client_id,
                      indexer_id, protocol)
   ↓
5. INSERT INTO history (event_type='grabbed', source_title, download_id, ...)
   ↓
6. NotificationService::notify(NotificationEvent::Grab { ... })
   ↓
7. Scheduler polls download clients every 1 min
   → client.get_items() → check for status=Completed
   ↓
8. process_completed_download(ImportContext { ... })
   → Scan, parse, rename, move, DB update
   ↓
9. UPDATE queue SET status='completed' (or DELETE)
   INSERT INTO history (event_type='imported', ...)
   NotificationService::notify(NotificationEvent::Import { ... })
```
