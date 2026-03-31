# Arz — Unified Media Manager

## Context

Replace Sonarr + Radarr + Prowlarr with a single Rust application. The core is a **media manager** — TV series and movie library management with automated search, grab, and organization. Download clients (rustTorrent, rustnzbd) and indexing (Indexarr) are optional embedded modules that can be swapped for external equivalents (qBittorrent, SABnzbd, Prowlarr, etc.).

**Key design principles:**
- Media manager first, download clients second
- Everything optional except the core media library
- First-boot wizard configures enabled modules
- Disabled modules are hidden from UI entirely
- PostgreSQL database (avoids SQLite contention)
- Single binary, single container + optional Indexarr sidecar

---

## 1. Cargo Workspace Structure

```
arz/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── arz-core/                 # Domain models, DB, config, error types
│   ├── arz-media/                # Media library: series, movies, episodes, files
│   ├── arz-parser/               # Release name parser (quality, episode, language)
│   ├── arz-quality/              # Quality profiles, custom formats, decision engine
│   ├── arz-indexer/              # Indexer hub: Newznab/Torznab client, Indexarr integration
│   ├── arz-download/             # Download client abstraction layer
│   ├── arz-import/               # Post-download import: match, rename, move, organize
│   ├── arz-scheduler/            # Background jobs: RSS, search, import scan, housekeeping
│   ├── arz-metadata/             # External metadata: TMDB, TVDB, OMDB clients
│   ├── arz-web/                  # Axum HTTP server, REST API, WebSocket, auth
│   └── arz-notify/               # Notifications: webhook, Telegram, Discord, email, etc.
├── src/
│   └── main.rs                   # Binary: CLI args, startup, signal handling
├── ui/                           # React frontend (Vite + TypeScript)
├── migrations/                   # PostgreSQL migrations (sqlx)
├── docker/
│   ├── Dockerfile
│   └── docker-compose.yml        # arz + optional indexarr sidecar
└── config.example.toml
```

### Crate dependency graph

```
main.rs
  └── arz-web
        ├── arz-media
        │     ├── arz-core
        │     ├── arz-parser
        │     └── arz-metadata
        ├── arz-quality
        │     ├── arz-core
        │     └── arz-parser
        ├── arz-indexer
        │     ├── arz-core
        │     └── arz-parser
        ├── arz-download
        │     └── arz-core
        ├── arz-import
        │     ├── arz-core
        │     ├── arz-media
        │     ├── arz-parser
        │     └── arz-quality
        ├── arz-scheduler
        │     ├── arz-core
        │     ├── arz-media
        │     ├── arz-indexer
        │     ├── arz-download
        │     ├── arz-import
        │     └── arz-quality
        └── arz-notify
              └── arz-core
```

### Feature flags (Cargo features on workspace)

```toml
[features]
default = ["ui"]
ui = []                           # Embed React SPA
torrent-embedded = ["librtbit"]   # Embedded rustTorrent engine
usenet-embedded = ["nzb-core", "nzb-web", "nzb-nntp", "nzb-decode", "nzb-postproc"]
indexarr-sidecar = []             # Indexarr HTTP client integration
```

---

## 2. Domain Model

### 2.1 Media Library (arz-media)

```rust
// --- TV ---
pub struct Series {
    pub id: i64,
    pub title: String,
    pub clean_title: String,       // normalized for matching
    pub sort_title: String,
    pub overview: Option<String>,
    pub status: SeriesStatus,      // Continuing, Ended, Upcoming, Deleted
    pub network: Option<String>,
    pub air_time: Option<NaiveTime>,
    pub first_aired: Option<NaiveDate>,
    pub year: Option<i32>,
    pub runtime: Option<i32>,      // minutes
    pub path: PathBuf,             // library root folder for this series
    pub root_folder_id: i64,
    pub quality_profile_id: i64,
    pub season_folder: bool,
    pub monitored: bool,
    pub use_scene_numbering: bool,
    pub series_type: SeriesType,   // Standard, Daily, Anime
    // External IDs
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    pub tvmaze_id: Option<i64>,
    pub mal_id: Option<i64>,
    pub tags: Vec<i64>,
    pub added_at: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
}

pub struct Episode {
    pub id: i64,
    pub series_id: i64,
    pub episode_file_id: Option<i64>,
    pub season_number: i32,
    pub episode_number: i32,
    pub absolute_number: Option<i32>,
    pub scene_season_number: Option<i32>,
    pub scene_episode_number: Option<i32>,
    pub scene_absolute_number: Option<i32>,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub runtime: Option<i32>,
    pub monitored: bool,
    pub grabbed: bool,              // currently being downloaded
    pub last_search_time: Option<DateTime<Utc>>,
}

// --- Movies ---
pub struct Movie {
    pub id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub year: Option<i32>,
    pub studio: Option<String>,
    pub path: PathBuf,
    pub root_folder_id: i64,
    pub quality_profile_id: i64,
    pub monitored: bool,
    pub minimum_availability: Availability, // Announced, InCinemas, Released
    pub movie_file_id: Option<i64>,
    // External IDs
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    // Dates
    pub in_cinemas: Option<NaiveDate>,
    pub physical_release: Option<NaiveDate>,
    pub digital_release: Option<NaiveDate>,
    pub tags: Vec<i64>,
    pub added_at: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub collection_tmdb_id: Option<i64>,
}

// --- Shared ---
pub struct MediaFile {
    pub id: i64,
    pub media_type: MediaType,     // Series, Movie
    pub relative_path: String,
    pub size: i64,
    pub date_added: DateTime<Utc>,
    pub quality: QualityModel,
    pub languages: Vec<Language>,
    pub scene_name: Option<String>,
    pub release_group: Option<String>,
    pub release_hash: Option<String>,
    pub edition: Option<String>,    // movies: Director's Cut, etc.
    pub media_info: Option<MediaInfo>,
    pub indexer_flags: i32,
}

pub enum MediaType { Series, Movie }
pub enum SeriesStatus { Continuing, Ended, Upcoming, Deleted }
pub enum SeriesType { Standard, Daily, Anime }
pub enum Availability { Announced, InCinemas, Released }
```

### 2.2 Parser (arz-parser)

```rust
pub struct ParsedRelease {
    pub title: String,
    pub release_title: String,      // original full name
    pub year: Option<i32>,
    pub quality: QualityModel,
    pub languages: Vec<Language>,
    pub release_group: Option<String>,
    pub release_hash: Option<String>,
    pub edition: Option<String>,
    // TV-specific (Option — absent for movies)
    pub season_number: Option<i32>,
    pub episode_numbers: Vec<i32>,
    pub absolute_episode_numbers: Vec<i32>,
    pub air_date: Option<NaiveDate>,  // daily shows
    pub is_full_season: bool,
    pub is_multi_season: bool,
    pub is_special: bool,
    // Movie-specific
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
}

pub struct QualityModel {
    pub quality: Quality,
    pub revision: Revision,
    pub source: QualitySource,      // where detected from
}

pub enum Quality {
    Unknown,
    SDTV, DVD, WEBDL480p, WEBRip480p, Bluray480p,
    HDTV720p, WEBDL720p, WEBRip720p, Bluray720p,
    HDTV1080p, WEBDL1080p, WEBRip1080p, Bluray1080p, Remux1080p,
    HDTV2160p, WEBDL2160p, WEBRip2160p, Bluray2160p, Remux2160p,
    Raw,
}

pub struct Revision {
    pub version: i32,   // 1 = original, 2+ = proper/repack
    pub real: i32,      // 0 = normal, 1+ = REAL tag
    pub is_repack: bool,
}
```

### 2.3 Quality Profiles (arz-quality)

```rust
pub struct QualityProfile {
    pub id: i64,
    pub name: String,
    pub cutoff: Quality,            // stop upgrading at this level
    pub items: Vec<QualityProfileItem>,
    pub upgrade_allowed: bool,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
}

pub struct QualityProfileItem {
    pub quality: Option<Quality>,   // None = group header
    pub allowed: bool,
    pub items: Vec<QualityProfileItem>, // nested groups
}

pub struct CustomFormat {
    pub id: i64,
    pub name: String,
    pub specifications: Vec<FormatSpecification>,
}

pub struct FormatSpecification {
    pub field: FormatField,         // ReleaseName, Quality, Language, IndexerFlag, etc.
    pub pattern: String,            // regex or enum match
    pub negate: bool,
    pub required: bool,
}

pub struct CustomFormatScore {
    pub profile_id: i64,
    pub format_id: i64,
    pub score: i32,
}
```

### 2.4 Decision Engine (arz-quality)

```rust
pub struct DownloadDecision {
    pub remote_release: RemoteRelease,
    pub rejections: Vec<Rejection>,
}

pub struct Rejection {
    pub reason: String,
    pub rejection_type: RejectionType, // Permanent, Temporary
}

/// Each spec checks one concern. Run in order, short-circuit on permanent reject.
pub trait DecisionSpecification: Send + Sync {
    fn is_satisfied(&self, decision: &DecisionContext) -> SpecificationResult;
}

// Specifications:
// - QualityAllowedSpec
// - QualityCutoffSpec (already have good enough?)
// - SizeLimitSpec (min/max per quality)
// - AgeLimitSpec (usenet retention)
// - BlocklistSpec
// - AlreadyImportedSpec
// - QueueConflictSpec (already downloading?)
// - CustomFormatScoreSpec
// - ProtocolSpec (usenet vs torrent preference)
// - MinimumSeedersSpec (torrents)
// - RepackSpec (prefer repacks of same release)
// - RawDiskSpec (reject raw disk images)
// - SampleSpec (reject samples)
// - LanguageSpec
```

### 2.5 Indexer Hub (arz-indexer)

```rust
pub struct IndexerConfig {
    pub id: i64,
    pub name: String,
    pub indexer_type: IndexerType,
    pub base_url: String,
    pub api_key: Option<String>,
    pub protocol: DownloadProtocol,  // Usenet, Torrent
    pub categories: Vec<i32>,        // Newznab category IDs
    pub enabled: bool,
    pub priority: i32,
    pub supports_search: bool,
    pub supports_rss: bool,
    pub proxy: Option<ProxyConfig>,
}

pub enum IndexerType {
    Newznab,                        // Standard Newznab (NZB indexers)
    Torznab,                        // Standard Torznab (torrent indexers)
    IndexarrTorznab,                // Indexarr sidecar via Torznab
    IndexarrApi,                    // Indexarr sidecar via REST API
}

pub struct ReleaseInfo {
    pub guid: String,
    pub title: String,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub indexer_id: i64,
    pub protocol: DownloadProtocol,
    pub size: i64,
    pub age: i64,                   // days
    pub publish_date: DateTime<Utc>,
    // Torrent-specific
    pub info_hash: Option<String>,
    pub magnet_url: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    // Usenet-specific
    pub nzb_url: Option<String>,
    // IDs from indexer
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    // Parsed
    pub categories: Vec<i32>,       // Newznab category IDs
    pub indexer_flags: Vec<IndexerFlag>,
}

pub enum DownloadProtocol { Usenet, Torrent }
```

### 2.6 Download Client Abstraction (arz-download)

```rust
/// Unified interface — works for embedded AND external clients
#[async_trait]
pub trait DownloadClient: Send + Sync {
    fn protocol(&self) -> DownloadProtocol;
    fn name(&self) -> &str;

    async fn add(&self, release: &GrabRequest) -> Result<String>;  // returns download_id
    async fn get_items(&self) -> Result<Vec<DownloadItem>>;
    async fn remove(&self, download_id: &str, delete_files: bool) -> Result<()>;
    async fn pause(&self, download_id: &str) -> Result<()>;
    async fn resume(&self, download_id: &str) -> Result<()>;
    async fn test(&self) -> Result<()>;
    async fn status(&self) -> Result<ClientStatus>;
}

pub struct GrabRequest {
    pub release: ReleaseInfo,
    pub category: String,
    pub download_url: String,       // NZB URL or magnet/torrent URL
}

pub struct DownloadItem {
    pub download_id: String,
    pub title: String,
    pub status: DownloadStatus,
    pub total_size: i64,
    pub remaining_size: i64,
    pub output_path: Option<PathBuf>,
    pub category: Option<String>,
    pub can_move_files: bool,
    pub can_be_removed: bool,
    pub protocol: DownloadProtocol,
}

pub enum DownloadStatus {
    Queued, Downloading, Paused,
    PostProcessing,                 // usenet: par2/extract
    Completed, Failed, Warning,
}

// --- Implementations ---

// External clients (HTTP API):
pub struct QBittorrentClient { /* qBit WebUI API */ }
pub struct TransmissionClient { /* Transmission RPC */ }
pub struct SabnzbdClient { /* SABnzbd API */ }
pub struct NzbgetClient { /* NZBGet API */ }

// Embedded clients (library calls):
#[cfg(feature = "torrent-embedded")]
pub struct EmbeddedTorrentClient {
    session: Arc<librtbit::Session>,
}

#[cfg(feature = "usenet-embedded")]
pub struct EmbeddedUsenetClient {
    queue_manager: Arc<nzb_web::QueueManager>,
}
```

### 2.7 History & Queue (arz-core)

```rust
pub struct HistoryEvent {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,              // series_id or movie_id
    pub episode_id: Option<i64>,    // TV only
    pub event_type: HistoryEventType,
    pub quality: QualityModel,
    pub languages: Vec<Language>,
    pub source_title: String,       // release name
    pub download_id: Option<String>,
    pub indexer_id: Option<i64>,
    pub download_client: Option<String>,
    pub data: serde_json::Value,    // event-specific metadata
    pub occurred_at: DateTime<Utc>,
}

pub enum HistoryEventType {
    Grabbed, Imported, DownloadFailed,
    FileDeleted, FileRenamed, DownloadIgnored,
}

pub struct QueueItem {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub episode_id: Option<i64>,
    pub quality: QualityModel,
    pub languages: Vec<Language>,
    pub size: i64,
    pub title: String,
    pub status: DownloadStatus,
    pub time_left: Option<Duration>,
    pub download_id: String,
    pub download_client: String,
    pub protocol: DownloadProtocol,
    pub indexer: String,
    pub error_message: Option<String>,
    pub added_at: DateTime<Utc>,
}

pub struct Blocklist {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub source_title: String,
    pub quality: QualityModel,
    pub languages: Vec<Language>,
    pub indexer_id: Option<i64>,
    pub info_hash: Option<String>,   // torrent matching
    pub message: Option<String>,
    pub added_at: DateTime<Utc>,
}
```

### 2.8 Modules/Features Configuration (arz-core)

```rust
/// Persisted in DB — set during first-boot, changeable in settings
pub struct EnabledModules {
    pub tv_management: bool,        // Series/Episodes
    pub movie_management: bool,     // Movies
    pub torrent_embedded: bool,     // Embedded rustTorrent
    pub usenet_embedded: bool,      // Embedded rustnzbd
    pub torrent_external: bool,     // External torrent clients
    pub usenet_external: bool,      // External usenet clients
    pub indexarr_sidecar: bool,     // Indexarr integration
    pub external_indexers: bool,    // Newznab/Torznab indexers
    pub notifications: bool,
}
```

---

## 3. Database Schema (PostgreSQL)

```sql
-- Core
CREATE TABLE app_config (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL
);

CREATE TABLE enabled_modules (
    id SERIAL PRIMARY KEY,
    module TEXT UNIQUE NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT false,
    config JSONB
);

CREATE TABLE root_folders (
    id SERIAL PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    media_type TEXT NOT NULL,        -- 'series' | 'movie'
    free_space BIGINT,
    last_checked TIMESTAMPTZ
);

CREATE TABLE tags (
    id SERIAL PRIMARY KEY,
    label TEXT NOT NULL UNIQUE
);

-- Quality system
CREATE TABLE quality_profiles (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    cutoff INTEGER NOT NULL,
    upgrade_allowed BOOLEAN NOT NULL DEFAULT true,
    min_format_score INTEGER NOT NULL DEFAULT 0,
    cutoff_format_score INTEGER NOT NULL DEFAULT 0,
    items JSONB NOT NULL             -- ordered quality items tree
);

CREATE TABLE custom_formats (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    specifications JSONB NOT NULL
);

CREATE TABLE custom_format_scores (
    profile_id INTEGER REFERENCES quality_profiles(id) ON DELETE CASCADE,
    format_id INTEGER REFERENCES custom_formats(id) ON DELETE CASCADE,
    score INTEGER NOT NULL,
    PRIMARY KEY (profile_id, format_id)
);

-- TV
CREATE TABLE series (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    overview TEXT,
    status TEXT NOT NULL DEFAULT 'continuing',
    series_type TEXT NOT NULL DEFAULT 'standard',
    network TEXT,
    air_time TIME,
    first_aired DATE,
    year INTEGER,
    runtime INTEGER,
    path TEXT NOT NULL,
    root_folder_id INTEGER REFERENCES root_folders(id),
    quality_profile_id INTEGER REFERENCES quality_profiles(id),
    season_folder BOOLEAN NOT NULL DEFAULT true,
    monitored BOOLEAN NOT NULL DEFAULT true,
    use_scene_numbering BOOLEAN NOT NULL DEFAULT false,
    tvdb_id INTEGER,
    imdb_id TEXT,
    tmdb_id INTEGER,
    tvmaze_id INTEGER,
    mal_id INTEGER,
    images JSONB,
    genres TEXT[],
    tags INTEGER[],
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_info_sync TIMESTAMPTZ
);
CREATE INDEX idx_series_tvdb ON series(tvdb_id);
CREATE INDEX idx_series_tmdb ON series(tmdb_id);
CREATE INDEX idx_series_imdb ON series(imdb_id);
CREATE INDEX idx_series_clean_title ON series(clean_title);

CREATE TABLE seasons (
    id SERIAL PRIMARY KEY,
    series_id INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    season_number INTEGER NOT NULL,
    monitored BOOLEAN NOT NULL DEFAULT true,
    UNIQUE(series_id, season_number)
);

CREATE TABLE episodes (
    id SERIAL PRIMARY KEY,
    series_id INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    season_number INTEGER NOT NULL,
    episode_number INTEGER NOT NULL,
    absolute_number INTEGER,
    scene_season_number INTEGER,
    scene_episode_number INTEGER,
    scene_absolute_number INTEGER,
    title TEXT,
    overview TEXT,
    air_date DATE,
    air_date_utc TIMESTAMPTZ,
    runtime INTEGER,
    monitored BOOLEAN NOT NULL DEFAULT true,
    episode_file_id INTEGER REFERENCES media_files(id) ON DELETE SET NULL,
    last_search_time TIMESTAMPTZ,
    UNIQUE(series_id, season_number, episode_number)
);
CREATE INDEX idx_episodes_air_date ON episodes(air_date_utc);

-- Movies
CREATE TABLE movies (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    sort_title TEXT NOT NULL,
    overview TEXT,
    year INTEGER,
    studio TEXT,
    path TEXT NOT NULL,
    root_folder_id INTEGER REFERENCES root_folders(id),
    quality_profile_id INTEGER REFERENCES quality_profiles(id),
    monitored BOOLEAN NOT NULL DEFAULT true,
    minimum_availability TEXT NOT NULL DEFAULT 'released',
    movie_file_id INTEGER REFERENCES media_files(id) ON DELETE SET NULL,
    tmdb_id INTEGER,
    imdb_id TEXT,
    in_cinemas DATE,
    physical_release DATE,
    digital_release DATE,
    images JSONB,
    genres TEXT[],
    tags INTEGER[],
    collection_tmdb_id INTEGER,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_info_sync TIMESTAMPTZ
);
CREATE INDEX idx_movies_tmdb ON movies(tmdb_id);
CREATE INDEX idx_movies_imdb ON movies(imdb_id);
CREATE INDEX idx_movies_clean_title ON movies(clean_title);

CREATE TABLE alternative_titles (
    id SERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    media_id INTEGER NOT NULL,
    title TEXT NOT NULL,
    clean_title TEXT NOT NULL,
    scene_name BOOLEAN NOT NULL DEFAULT false
);
CREATE INDEX idx_alt_titles_clean ON alternative_titles(clean_title);

-- Media files (shared between TV and movies)
CREATE TABLE media_files (
    id SERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    size BIGINT NOT NULL,
    date_added TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    quality JSONB NOT NULL,
    languages JSONB NOT NULL,
    scene_name TEXT,
    release_group TEXT,
    release_hash TEXT,
    edition TEXT,
    media_info JSONB,
    indexer_flags INTEGER NOT NULL DEFAULT 0
);

-- Episode-to-file join (multi-episode files)
CREATE TABLE episode_files (
    episode_id INTEGER NOT NULL REFERENCES episodes(id) ON DELETE CASCADE,
    media_file_id INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    PRIMARY KEY (episode_id, media_file_id)
);

-- Indexers
CREATE TABLE indexers (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    indexer_type TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_key TEXT,
    protocol TEXT NOT NULL,
    categories INTEGER[],
    enabled BOOLEAN NOT NULL DEFAULT true,
    priority INTEGER NOT NULL DEFAULT 25,
    supports_search BOOLEAN NOT NULL DEFAULT true,
    supports_rss BOOLEAN NOT NULL DEFAULT true,
    config JSONB,                    -- proxy, extra settings
    last_rss_sync TIMESTAMPTZ
);

-- Download clients
CREATE TABLE download_clients (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    client_type TEXT NOT NULL,       -- 'embedded_torrent', 'embedded_usenet', 'qbittorrent', 'sabnzbd', etc.
    protocol TEXT NOT NULL,
    config JSONB NOT NULL,           -- host, port, api_key, category mappings
    enabled BOOLEAN NOT NULL DEFAULT true,
    priority INTEGER NOT NULL DEFAULT 1
);

-- Queue (tracked downloads in progress)
CREATE TABLE queue (
    id SERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    media_id INTEGER NOT NULL,
    episode_id INTEGER,
    title TEXT NOT NULL,
    quality JSONB NOT NULL,
    languages JSONB,
    size BIGINT,
    status TEXT NOT NULL,
    download_id TEXT NOT NULL,
    download_client_id INTEGER REFERENCES download_clients(id),
    indexer_id INTEGER REFERENCES indexers(id),
    protocol TEXT NOT NULL,
    error_message TEXT,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_queue_download_id ON queue(download_id);

-- History
CREATE TABLE history (
    id BIGSERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    media_id INTEGER NOT NULL,
    episode_id INTEGER,
    event_type TEXT NOT NULL,
    quality JSONB NOT NULL,
    languages JSONB,
    source_title TEXT NOT NULL,
    download_id TEXT,
    indexer_id INTEGER,
    download_client TEXT,
    data JSONB,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_history_media ON history(media_type, media_id);
CREATE INDEX idx_history_occurred ON history(occurred_at DESC);
CREATE INDEX idx_history_download_id ON history(download_id);

-- Blocklist
CREATE TABLE blocklist (
    id SERIAL PRIMARY KEY,
    media_type TEXT NOT NULL,
    media_id INTEGER NOT NULL,
    source_title TEXT NOT NULL,
    quality JSONB NOT NULL,
    languages JSONB,
    indexer_id INTEGER,
    info_hash TEXT,
    message TEXT,
    added_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_blocklist_media ON blocklist(media_type, media_id);
CREATE INDEX idx_blocklist_hash ON blocklist(info_hash);

-- Naming conventions
CREATE TABLE naming_config (
    id SERIAL PRIMARY KEY,
    media_type TEXT NOT NULL UNIQUE,
    rename_files BOOLEAN NOT NULL DEFAULT true,
    standard_format TEXT,            -- token pattern: {Series Title} - S{season:00}E{episode:00}
    daily_format TEXT,
    anime_format TEXT,
    season_folder_format TEXT,
    movie_format TEXT,
    movie_folder_format TEXT,
    colon_replacement TEXT NOT NULL DEFAULT 'smart'
);

-- Notifications
CREATE TABLE notification_providers (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,     -- 'webhook', 'telegram', 'discord', etc.
    config JSONB NOT NULL,
    on_grab BOOLEAN NOT NULL DEFAULT false,
    on_import BOOLEAN NOT NULL DEFAULT false,
    on_upgrade BOOLEAN NOT NULL DEFAULT false,
    on_health_issue BOOLEAN NOT NULL DEFAULT false,
    on_failure BOOLEAN NOT NULL DEFAULT false,
    enabled BOOLEAN NOT NULL DEFAULT true
);

-- Import lists (external sources of media to add)
CREATE TABLE import_lists (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    list_type TEXT NOT NULL,         -- 'tmdb_popular', 'trakt_watchlist', 'imdb_list', etc.
    media_type TEXT NOT NULL,
    config JSONB NOT NULL,
    quality_profile_id INTEGER REFERENCES quality_profiles(id),
    root_folder_id INTEGER REFERENCES root_folders(id),
    monitored BOOLEAN NOT NULL DEFAULT true,
    enabled BOOLEAN NOT NULL DEFAULT true,
    poll_interval_secs INTEGER NOT NULL DEFAULT 3600
);
```

---

## 4. REST API Design

```
# First-boot & system
GET    /api/v1/system/status         # Version, uptime, enabled modules
GET    /api/v1/system/health         # Health checks
POST   /api/v1/setup/init            # First-boot: set modules + basic config
PUT    /api/v1/setup/modules         # Enable/disable modules (restart required for embedded clients)

# Series (hidden if tv_management disabled)
GET    /api/v1/series                # List all
POST   /api/v1/series                # Add series (lookup + add)
GET    /api/v1/series/{id}
PUT    /api/v1/series/{id}
DELETE /api/v1/series/{id}
GET    /api/v1/series/lookup?term=   # Search TVDB/TMDB
GET    /api/v1/series/{id}/episodes
GET    /api/v1/episode/{id}
PUT    /api/v1/episode/{id}          # Toggle monitored, etc.

# Movies (hidden if movie_management disabled)
GET    /api/v1/movie                 # List all
POST   /api/v1/movie                 # Add movie
GET    /api/v1/movie/{id}
PUT    /api/v1/movie/{id}
DELETE /api/v1/movie/{id}
GET    /api/v1/movie/lookup?term=    # Search TMDB

# Calendar
GET    /api/v1/calendar?start=&end=  # Upcoming episodes + movie releases

# Wanted
GET    /api/v1/wanted/missing        # Monitored, no file
GET    /api/v1/wanted/cutoff         # Have file, below cutoff quality

# Media files
GET    /api/v1/mediafile             # All files
GET    /api/v1/mediafile/{id}
DELETE /api/v1/mediafile/{id}
PUT    /api/v1/rename                # Preview rename
POST   /api/v1/rename                # Execute rename

# Manual import
GET    /api/v1/manualimport?folder=  # Scan folder, return matches
POST   /api/v1/manualimport          # Execute import of selected files

# Quality profiles
GET    /api/v1/qualityprofile
POST   /api/v1/qualityprofile
PUT    /api/v1/qualityprofile/{id}
DELETE /api/v1/qualityprofile/{id}

# Custom formats
GET    /api/v1/customformat
POST   /api/v1/customformat
PUT    /api/v1/customformat/{id}
DELETE /api/v1/customformat/{id}

# Indexers
GET    /api/v1/indexer
POST   /api/v1/indexer
PUT    /api/v1/indexer/{id}
DELETE /api/v1/indexer/{id}
POST   /api/v1/indexer/{id}/test

# Download clients
GET    /api/v1/downloadclient
POST   /api/v1/downloadclient
PUT    /api/v1/downloadclient/{id}
DELETE /api/v1/downloadclient/{id}
POST   /api/v1/downloadclient/{id}/test

# Releases (search results)
GET    /api/v1/release?episodeId=    # Search indexers for episode
GET    /api/v1/release?movieId=      # Search indexers for movie
POST   /api/v1/release               # Grab a release (send to download client)
POST   /api/v1/release/push          # External push (webhook from indexer)

# Queue
GET    /api/v1/queue                 # Current downloads
DELETE /api/v1/queue/{id}            # Remove + optional blocklist
POST   /api/v1/queue/grab/{id}      # Force grab pending item

# History
GET    /api/v1/history               # Paginated event log
POST   /api/v1/history/failed/{id}   # Mark as failed → blocklist + re-search

# Blocklist
GET    /api/v1/blocklist
DELETE /api/v1/blocklist/{id}
DELETE /api/v1/blocklist/bulk

# Commands (async job triggers)
POST   /api/v1/command               # { name: "SeriesSearch", seriesId: 1 }
GET    /api/v1/command/{id}          # Check command status

# Supported commands:
#   SeriesSearch, SeasonSearch, EpisodeSearch
#   MovieSearch
#   RssSync
#   RefreshSeries, RefreshMovie
#   DiskScan (rescan library)
#   MissingSearch, CutoffSearch
#   Housekeeping

# Config
GET    /api/v1/config/naming
PUT    /api/v1/config/naming
GET    /api/v1/config/general
PUT    /api/v1/config/general

# Embedded torrent client status (hidden if not enabled)
GET    /api/v1/torrent/status        # rustTorrent session stats
GET    /api/v1/torrent/list          # Active torrents

# Embedded usenet client status (hidden if not enabled)
GET    /api/v1/usenet/status         # rustnzbd queue stats
GET    /api/v1/usenet/servers        # NNTP server config

# Notifications
GET    /api/v1/notification
POST   /api/v1/notification
PUT    /api/v1/notification/{id}
DELETE /api/v1/notification/{id}
POST   /api/v1/notification/{id}/test

# Tags
GET    /api/v1/tag
POST   /api/v1/tag
PUT    /api/v1/tag/{id}
DELETE /api/v1/tag/{id}

# Root folders
GET    /api/v1/rootfolder
POST   /api/v1/rootfolder
DELETE /api/v1/rootfolder/{id}

# Import lists
GET    /api/v1/importlist
POST   /api/v1/importlist
PUT    /api/v1/importlist/{id}
DELETE /api/v1/importlist/{id}

# Log
GET    /api/v1/log                   # Paginated log entries
WS     /api/v1/log/stream            # Real-time log WebSocket

# Indexarr sidecar (hidden if not enabled)
GET    /api/v1/indexarr/status       # Sidecar health + stats
GET    /api/v1/indexarr/search       # Proxy search through sidecar
```

---

## 5. Integration Architecture

### 5.1 Embedded rustTorrent

```rust
#[cfg(feature = "torrent-embedded")]
pub struct EmbeddedTorrentClient {
    session: Arc<librtbit::Session>,
    api: librtbit::Api,
}

impl EmbeddedTorrentClient {
    pub async fn new(config: &TorrentConfig) -> Result<Self> {
        let opts = SessionOptions {
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(config.data_dir.join("torrent-state")),
            }),
            listen: Some(ListenerOptions { /* ... */ }),
            ..Default::default()
        };
        let session = Session::new_with_opts(config.download_dir.clone(), opts).await?;
        let api = Api::new(Arc::clone(&session), None);
        Ok(Self { session, api })
    }
}

#[async_trait]
impl DownloadClient for EmbeddedTorrentClient {
    async fn add(&self, req: &GrabRequest) -> Result<String> {
        let opts = AddTorrentOptions {
            category: Some(req.category.clone()),
            ..Default::default()
        };
        let resp = self.session.add_torrent(
            AddTorrent::from_url(&req.download_url),
            Some(opts),
        ).await?;
        Ok(resp.into_handle().unwrap().info_hash().to_string())
    }

    async fn get_items(&self) -> Result<Vec<DownloadItem>> {
        let list = self.api.api_torrent_list();
        // Map TorrentListResponse → Vec<DownloadItem>
    }
    // ...
}
```

### 5.2 Embedded rustnzbd

```rust
#[cfg(feature = "usenet-embedded")]
pub struct EmbeddedUsenetClient {
    queue_manager: Arc<nzb_web::QueueManager>,
}

impl EmbeddedUsenetClient {
    pub async fn new(config: &UsenetConfig) -> Result<Self> {
        let startup = nzb_web::startup::initialize(
            StartupConfig {
                config_path: config.config_path.clone(),
                data_dir: Some(config.data_dir.clone()),
                ..Default::default()
            },
            None,
        ).await?;
        Ok(Self { queue_manager: startup.queue_manager })
    }
}

#[async_trait]
impl DownloadClient for EmbeddedUsenetClient {
    async fn add(&self, req: &GrabRequest) -> Result<String> {
        let nzb_data = reqwest::get(&req.download_url).await?.bytes().await?;
        let mut job = nzb_core::nzb_parser::parse_nzb(&req.release.title, &nzb_data)?;
        job.category = req.category.clone();
        self.queue_manager.add_job(job.clone(), Some(nzb_data.to_vec())).await?;
        Ok(job.id)
    }

    async fn get_items(&self) -> Result<Vec<DownloadItem>> {
        let jobs = self.queue_manager.get_jobs();
        // Map NzbJob → DownloadItem
    }
    // ...
}
```

### 5.3 External clients

```rust
// qBittorrent WebUI API
pub struct QBittorrentClient { base_url: String, session: reqwest::Client }
// SABnzbd API
pub struct SabnzbdClient { base_url: String, api_key: String, session: reqwest::Client }
// NZBGet JSON-RPC
pub struct NzbgetClient { base_url: String, session: reqwest::Client }
```

### 5.4 Indexarr Sidecar

```rust
pub struct IndexarrClient {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl IndexarrClient {
    /// Use Torznab API (Sonarr/Radarr compatible)
    pub async fn torznab_search(&self, params: &TorznabQuery) -> Result<Vec<ReleaseInfo>> {
        // GET {base_url}/api/torznab?t=search&q=...&apikey=...
    }

    /// Use native REST API for richer results
    pub async fn search(&self, query: &str, filters: &SearchFilters) -> Result<Vec<ReleaseInfo>> {
        // GET {base_url}/api/v1/search?q=...
    }

    pub async fn status(&self) -> Result<IndexarrStatus> {
        // GET {base_url}/api/v1/stats
    }
}
```

---

## 6. Search Flow

```
User triggers search (manual or scheduled)
    │
    ▼
SearchService
    ├── Build search criteria from media (Series+Episode or Movie)
    │   - Extract IDs: tvdb_id, imdb_id, tmdb_id
    │   - Build query terms: "Show Name S01E05" / "Movie Name 2024"
    │   - Determine categories: TV HD (5040) / Movies HD (2040) etc.
    │
    ├── Fan out to all enabled indexers (parallel)
    │   ├── Newznab indexers → GET /api?t=tvsearch&tvdbid=&season=&ep=
    │   ├── Torznab indexers → GET /api?t=tvsearch&tvdbid=&season=&ep=
    │   ├── Indexarr (Torznab) → GET /api/torznab?t=tvsearch&...
    │   └── Indexarr (REST) → GET /api/v1/search?q=...
    │
    ├── Aggregate all ReleaseInfo results
    │
    ├── Parse each release name → ParsedRelease
    │   - Extract quality, language, release group
    │   - Match to correct media (by title similarity + IDs)
    │
    ├── Run Decision Engine on each release
    │   - Check against quality profile
    │   - Filter by size limits
    │   - Check blocklist
    │   - Check if already in queue
    │   - Check if already imported at same/better quality
    │   - Score custom formats
    │   - Reject or approve
    │
    ├── Sort approved releases by preference
    │   - Quality rank (from profile)
    │   - Custom format score
    │   - Protocol preference (usenet vs torrent)
    │   - Indexer priority
    │   - Age (prefer newer for usenet)
    │   - Seeders (prefer more for torrent)
    │
    └── Grab best release (or return ranked list for interactive search)
        ├── Select download client (by protocol + priority)
        ├── client.add(GrabRequest) → download_id
        ├── Insert queue record
        ├── Insert history record (Grabbed)
        └── Send notification (on_grab)
```

---

## 7. Download → Import Flow

```
Background: CompletedDownloadService (polls every 60s)
    │
    ├── For each enabled download client:
    │   └── client.get_items() → Vec<DownloadItem>
    │
    ├── Match DownloadItem.download_id to queue records
    │
    ├── For completed items:
    │   │
    │   ▼
    │   ImportService
    │   ├── Scan output_path for media files
    │   ├── Parse each filename → ParsedRelease
    │   ├── Match to Series+Episode or Movie (using queue record as hint)
    │   ├── Run import decision engine (quality upgrade check)
    │   │
    │   ├── For each approved file:
    │   │   ├── Build target path using naming config tokens
    │   │   │   TV:    {root}/{Series}/{Season 01}/{Series - S01E05 - Title [Quality]}
    │   │   │   Movie: {root}/{Movie (Year)}/{Movie (Year) - [Quality]}
    │   │   ├── Move/hardlink/copy file to library
    │   │   ├── Create/update MediaFile record
    │   │   ├── Link to Episode or Movie
    │   │   ├── Delete old file if upgrade
    │   │   ├── Insert history record (Imported)
    │   │   └── Send notification (on_import)
    │   │
    │   └── Remove queue record
    │
    └── For failed items:
        ├── Insert history record (DownloadFailed)
        ├── Add to blocklist (optional, based on settings)
        ├── Remove from download client
        ├── Remove queue record
        ├── Trigger re-search if configured
        └── Send notification (on_failure)
```

---

## 8. Configuration

```toml
# config.toml

[general]
instance_name = "Arz"
bind_addr = "0.0.0.0"
port = 8989
data_dir = "/config"
log_level = "info"

[database]
url = "postgresql://arz:password@localhost:5432/arz"
max_connections = 20

[auth]
method = "forms"                    # none, forms, basic, external
api_key = "auto-generated"

# --- Optional embedded modules ---

[torrent]
enabled = false                     # set true via first-boot
download_dir = "/downloads/torrents/incomplete"
complete_dir = "/downloads/torrents/complete"
listen_port = 6881
dht_enabled = true
peer_limit = 200
upload_limit_bps = 0
download_limit_bps = 0

[usenet]
enabled = false                     # set true via first-boot
incomplete_dir = "/downloads/usenet/incomplete"
complete_dir = "/downloads/usenet/complete"
max_active_downloads = 3

[[usenet.servers]]
name = "Primary"
host = "news.example.com"
port = 563
ssl = true
username = ""
password = ""
connections = 8
priority = 0

[indexarr]
enabled = false                     # set true via first-boot
url = "http://indexarr:8080"
api_key = ""
mode = "peer"                       # peer (sync only), full (DHT + sync)

[naming.series]
rename = true
standard = "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]"
daily = "{Series Title} - {Air-Date} - {Episode Title} [{Quality Title}]"
anime = "{Series Title} - S{season:00}E{episode:00} - {Absolute Episode} - {Episode Title} [{Quality Title}]"
season_folder = "Season {season:00}"

[naming.movie]
rename = true
standard = "{Movie Title} ({Release Year}) [{Quality Title}]{[Edition Tags]}"
folder = "{Movie Title} ({Release Year})"
```

---

## 9. Background Services (arz-scheduler)

All run as tokio tasks inside the single process:

| Service | Interval | Purpose |
|---------|----------|---------|
| **RssSyncService** | 15 min | Poll all indexers' RSS feeds, auto-grab matching releases |
| **CompletedDownloadService** | 60s | Poll download clients, trigger import for completed items |
| **RefreshSeriesService** | 12 hr | Refresh metadata from TVDB/TMDB for all series |
| **RefreshMovieService** | 12 hr | Refresh metadata from TMDB for all movies |
| **DiskScanService** | on-demand | Scan library folders, detect new/removed files |
| **HousekeepingService** | 24 hr | Clean old history, expired blocklist entries, orphaned records |
| **ImportListSyncService** | 1 hr | Poll import lists, auto-add new media |
| **HealthCheckService** | 5 min | Check download clients, indexers, disk space |
| **QueueCleanupService** | 15 min | Detect stale queue items (download client no longer has them) |
| **SearchScheduler** | configurable | Scheduled searches for missing/cutoff media |
| **EmbeddedTorrentMonitor** | 30s | (if enabled) Track rustTorrent session health |
| **EmbeddedUsenetMonitor** | 30s | (if enabled) Track rustnzbd queue health |
| **IndexarrHealthCheck** | 5 min | (if enabled) Check sidecar connectivity |

---

## 10. First-Boot Flow

```
1. User opens http://localhost:8989 for first time
2. App detects no config in DB → serves first-boot wizard

Wizard steps:
  ┌─────────────────────────────────────────────┐
  │ Step 1: Welcome                             │
  │   "What do you want to manage?"             │
  │   [x] TV Series                             │
  │   [x] Movies                                │
  │   [ ] (future: Music, Books)                │
  ├─────────────────────────────────────────────┤
  │ Step 2: Download Clients                    │
  │   Torrent:                                  │
  │   ( ) None                                  │
  │   ( ) Built-in (rustTorrent)                │
  │   ( ) External (qBittorrent/Transmission)   │
  │                                             │
  │   Usenet:                                   │
  │   ( ) None                                  │
  │   ( ) Built-in (rustnzbd)                   │
  │   ( ) External (SABnzbd/NZBGet)             │
  │                                             │
  │   [If external selected → config fields]    │
  ├─────────────────────────────────────────────┤
  │ Step 3: Indexers                            │
  │   ( ) None (add later)                      │
  │   ( ) Built-in Indexarr sidecar             │
  │      └── [auto-detect http://indexarr:8080] │
  │   ( ) External indexers (add later)         │
  ├─────────────────────────────────────────────┤
  │ Step 4: Library Folders                     │
  │   TV root:    [/media/tv    ] [Browse]      │
  │   Movie root: [/media/movies] [Browse]      │
  ├─────────────────────────────────────────────┤
  │ Step 5: Quality Profile                     │
  │   [Create default profiles]                 │
  │   Or: [Import from existing Sonarr/Radarr]  │
  ├─────────────────────────────────────────────┤
  │ Step 6: Authentication                      │
  │   Username: [________]                      │
  │   Password: [________]                      │
  └─────────────────────────────────────────────┘

3. POST /api/v1/setup/init with all selections
4. App initializes enabled modules, starts background services
5. Redirect to dashboard
```

---

## 11. Implementation Phases

### Phase 1: Foundation (MVP — media library + manual search)
- [ ] Cargo workspace scaffold with all crates (empty)
- [ ] PostgreSQL schema + sqlx migrations
- [ ] `arz-core`: config loading, DB pool, error types, enabled modules
- [ ] `arz-metadata`: TMDB client (movies + TV)
- [ ] `arz-media`: Series/Episode/Movie CRUD, root folders
- [ ] `arz-parser`: Release name parser (quality, episodes, title extraction)
- [ ] `arz-web`: Axum server, auth, REST API for media CRUD
- [ ] First-boot API endpoint
- [ ] React UI: first-boot wizard, series/movie list, add/search media
- [ ] Docker: single container with Postgres

### Phase 2: Search & Grab
- [ ] `arz-quality`: Quality profiles, custom formats, decision engine
- [ ] `arz-indexer`: Newznab/Torznab client, Indexarr client
- [ ] `arz-download`: Download client trait + external client implementations (qBit, SABnzbd)
- [ ] Search flow: manual search → decision engine → grab
- [ ] Queue tracking (poll download clients)
- [ ] History + blocklist
- [ ] UI: interactive search, queue view, history

### Phase 3: Automated Workflows
- [ ] `arz-scheduler`: RSS sync, completed download polling
- [ ] `arz-import`: Completed download → scan → match → rename → move
- [ ] Naming config with token system
- [ ] Auto-search for missing/cutoff
- [ ] Calendar view
- [ ] Wanted views (missing + cutoff unmet)

### Phase 4: Embedded Download Clients
- [ ] `arz-download`: EmbeddedTorrentClient wrapping `librtbit`
- [ ] `arz-download`: EmbeddedUsenetClient wrapping `nzb-*` crates
- [ ] First-boot options for embedded vs external
- [ ] UI: embedded client status panels
- [ ] Usenet server configuration in UI

### Phase 5: Indexarr Integration
- [ ] Indexarr sidecar docker-compose
- [ ] `arz-indexer`: IndexarrClient (Torznab + REST)
- [ ] Auto-detect sidecar on startup
- [ ] Indexarr defaults to peer-only mode (sync, no crawling)
- [ ] UI: Indexarr status panel

### Phase 6: Polish & Parity
- [ ] `arz-notify`: Notification providers (webhook, Discord, Telegram)
- [ ] Import lists (TMDB popular, Trakt watchlist, IMDB)
- [ ] Manual import (scan folder, match, import)
- [ ] Disk scan (detect files added outside the app)
- [ ] Custom format specifications (full regex engine)
- [ ] Scene name mapping / XEM integration
- [ ] Backup/restore
- [ ] API key auth for external tool integration
- [ ] Health check system

### Phase 7: Migration Tools
- [ ] Import from Sonarr (SQLite → PostgreSQL migration)
- [ ] Import from Radarr (SQLite → PostgreSQL migration)
- [ ] Import from Prowlarr (indexer definitions)

---

## 12. Key Dependencies

```toml
[workspace.dependencies]
# Async
tokio = { version = "1", features = ["full"] }
# Web
axum = "0.8"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "fs"] }
# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "chrono", "json", "uuid"] }
# HTTP client
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
quick-xml = "0.37"                   # Newznab/Torznab XML parsing
# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
# Error handling
thiserror = "2"
anyhow = "1"
# Util
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
regex = "1"
async-trait = "0.1"
# Embedded clients (optional)
librtbit = { path = "../rustTorrent/crates/librtbit", optional = true }
nzb-core = { path = "../rustnzbd/crates/nzb-core", optional = true }
nzb-web = { path = "../rustnzbd/crates/nzb-web", optional = true }
nzb-nntp = { path = "../rustnzbd/crates/nzb-nntp", optional = true }
nzb-decode = { path = "../rustnzbd/crates/nzb-decode", optional = true }
nzb-postproc = { path = "../rustnzbd/crates/nzb-postproc", optional = true }
```

---

## 13. Docker Deployment

```yaml
# docker-compose.yml
services:
  arz:
    build: .
    ports:
      - "8989:8989"
    volumes:
      - ./config:/config
      - /media/tv:/media/tv
      - /media/movies:/media/movies
      - /downloads:/downloads
    environment:
      - ARZ_DATABASE_URL=postgresql://arz:password@postgres:5432/arz
      - ARZ_TORRENT_ENABLED=true
      - ARZ_USENET_ENABLED=true
      - ARZ_INDEXARR_ENABLED=true
      - ARZ_INDEXARR_URL=http://indexarr:8080
    depends_on:
      - postgres

  postgres:
    image: postgres:17-alpine
    volumes:
      - pgdata:/var/lib/postgresql/data
    environment:
      - POSTGRES_USER=arz
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=arz

  # Optional sidecar
  indexarr:
    image: indexarr:latest
    environment:
      - INDEXARR_WORKERS=http_server,sync    # peer-only mode
      - INDEXARR_DB_BACKEND=sqlite
    ports:
      - "8080:8080"
    profiles:
      - indexarr

volumes:
  pgdata:
```

---

## Verification Plan

### Phase 1 verification:
```bash
# Run migrations
sqlx database create && sqlx migrate run

# Start app
cargo run -- --config config.toml

# Test first-boot
curl http://localhost:8989/api/v1/system/status
# → should return { "firstBoot": true }

# Complete setup
curl -X POST http://localhost:8989/api/v1/setup/init -d '...'

# Add a series
curl -X POST http://localhost:8989/api/v1/series -d '{"tvdbId": 121361}'

# Add a movie
curl -X POST http://localhost:8989/api/v1/movie -d '{"tmdbId": 550}'
```

### Integration test:
```bash
cargo test --workspace
# + docker compose test environment with Postgres
```
