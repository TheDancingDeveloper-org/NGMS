# Domain Models

All model structs live in `stackarr-core/src/models.rs` and use `#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]`.

## Enums

### MediaType
```rust
pub enum MediaType { Series, Movie }
```
Stored as TEXT in Postgres. Used throughout to distinguish TV vs movie operations.

### SeriesStatus
```rust
pub enum SeriesStatus { Continuing, Ended, Upcoming, Deleted }
```

### SeriesType
```rust
pub enum SeriesType { Standard, Daily, Anime }
```
Affects episode numbering: Standard uses S##E##, Daily uses air dates, Anime uses absolute numbers.

### Availability (Movies)
```rust
pub enum Availability { Announced, InCinemas, Released }
```
Controls when a movie is considered "available" for download.

### DownloadProtocol
```rust
pub enum DownloadProtocol { Usenet, Torrent }
```

### DownloadStatus
```rust
pub enum DownloadStatus { Queued, Downloading, Paused, PostProcessing, Completed, Failed, Warning }
```

### HistoryEventType
```rust
pub enum HistoryEventType { Grabbed, Imported, DownloadFailed, FileDeleted, FileRenamed, DownloadIgnored }
```

### Quality (24 variants)
```rust
pub enum Quality {
    Unknown = 0,
    SDTV = 1, DVD = 2,
    WEBDL480p = 3, WEBRip480p = 4, Bluray480p = 5,
    HDTV720p = 6, WEBDL720p = 7, WEBRip720p = 8, Bluray720p = 9,
    HDTV1080p = 10, WEBDL1080p = 11, WEBRip1080p = 12, Bluray1080p = 13, Remux1080p = 14,
    HDTV2160p = 15, WEBDL2160p = 16, WEBRip2160p = 17, Bluray2160p = 18, Remux2160p = 19,
    Raw = 20,
}
```

### Language (27 variants)
```rust
pub enum Language {
    Unknown, Multi,
    English, French, Spanish, German, Italian, Portuguese,
    Japanese, Chinese, Korean, Russian, Arabic, Hindi,
    Polish, Dutch, Swedish, Norwegian, Danish, Finnish,
    Turkish, Czech, Hungarian, Romanian, Greek, Hebrew,
    Thai, Vietnamese, Indonesian,
}
```

### DiscoverSliderType (17 variants)
```rust
pub enum DiscoverSliderType {
    Trending, PopularMovies, PopularTv, UpcomingMovies, UpcomingTv,
    RecentlyAdded, MovieGenres, TvGenres,
    TmdbMovieGenre, TmdbTvGenre, TmdbMovieKeyword, TmdbTvKeyword,
    TmdbSearch, TmdbStudio, TmdbNetwork,
    TmdbMovieStreamingServices, TmdbTvStreamingServices,
}
```

---

## Core Media Models

### Series
```rust
pub struct Series {
    pub id: i64,
    pub title: String,
    pub clean_title: String,              // Normalized for matching
    pub sort_title: String,
    pub overview: Option<String>,
    pub status: SeriesStatus,
    pub series_type: SeriesType,
    pub network: Option<String>,
    pub air_time: Option<NaiveTime>,
    pub first_aired: Option<NaiveDate>,
    pub year: Option<i32>,
    pub runtime: Option<i32>,             // Episode runtime in minutes
    pub path: String,                     // Filesystem path
    pub media_library_folder_id: Option<i32>,
    pub quality_profile_id: i32,
    pub season_folder: bool,              // Organize into season folders
    pub monitored: bool,
    pub use_scene_numbering: bool,

    // External IDs
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,          // "tt0903747" format
    pub tmdb_id: Option<i64>,
    pub tvmaze_id: Option<i64>,
    pub mal_id: Option<i64>,

    // Metadata
    pub images: Option<serde_json::Value>,  // [{coverType, url}]
    pub genres: Option<Vec<String>>,
    pub tags: Option<Vec<i32>>,

    // Timestamps
    pub added_at: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub media_added_at: Option<DateTime<Utc>>,

    // Plex
    pub plex_rating_key: Option<String>,
    pub plex_rating_key_4k: Option<String>,
}
```

### Season
```rust
pub struct Season {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub monitored: bool,
}
```

### Episode
```rust
pub struct Episode {
    pub id: i64,
    pub series_id: i64,
    pub season_number: i32,
    pub episode_number: i32,

    // Alternate numbering
    pub absolute_number: Option<i32>,           // Anime absolute
    pub scene_season_number: Option<i32>,        // Scene numbering override
    pub scene_episode_number: Option<i32>,
    pub scene_absolute_number: Option<i32>,

    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub air_date_utc: Option<DateTime<Utc>>,
    pub runtime: Option<i32>,
    pub monitored: bool,
    pub episode_file_id: Option<i64>,            // FK → media_files
    pub last_search_time: Option<DateTime<Utc>>,
}
```

### Movie
```rust
pub struct Movie {
    pub id: i64,
    pub title: String,
    pub clean_title: String,
    pub sort_title: String,
    pub overview: Option<String>,
    pub year: Option<i32>,
    pub studio: Option<String>,
    pub path: String,
    pub media_library_folder_id: Option<i32>,
    pub quality_profile_id: i32,
    pub monitored: bool,
    pub minimum_availability: Availability,
    pub movie_file_id: Option<i64>,               // FK → media_files

    // External IDs
    pub tmdb_id: Option<i64>,
    pub imdb_id: Option<String>,

    // Release dates
    pub in_cinemas: Option<NaiveDate>,
    pub physical_release: Option<NaiveDate>,
    pub digital_release: Option<NaiveDate>,

    // Metadata
    pub images: Option<serde_json::Value>,
    pub genres: Option<Vec<String>>,
    pub tags: Option<Vec<i32>>,
    pub collection_tmdb_id: Option<i64>,

    // Timestamps
    pub added_at: DateTime<Utc>,
    pub last_info_sync: Option<DateTime<Utc>>,
    pub media_added_at: Option<DateTime<Utc>>,

    // Plex
    pub plex_rating_key: Option<String>,
    pub plex_rating_key_4k: Option<String>,
}
```

---

## File Models

### MediaFile
```rust
pub struct MediaFile {
    pub id: i64,
    pub media_type: MediaType,
    pub relative_path: String,             // Relative to library root
    pub size: i64,                         // Bytes
    pub date_added: DateTime<Utc>,
    pub quality: serde_json::Value,        // QualityModel JSON
    pub languages: serde_json::Value,      // [Language] JSON
    pub scene_name: Option<String>,        // Original release name
    pub release_group: Option<String>,
    pub release_hash: Option<String>,
    pub edition: Option<String>,           // Director's Cut, Extended, etc.
    pub media_info: Option<serde_json::Value>, // Video/audio codec info
    pub indexer_flags: i32,
}
```

### EpisodeFile (Junction)
```rust
// Links episodes to media_files (supports multi-episode files)
pub struct EpisodeFile {
    pub id: i64,
    pub episode_id: i64,
    pub media_file_id: i64,
}
```

---

## Quality Models

### QualityModel (used in JSONB columns)
```rust
pub struct QualityModel {
    pub quality: Quality,
    pub revision: Revision,
}

pub struct Revision {
    pub version: i32,     // 1 = original, 2 = PROPER/REPACK
    pub real: i32,        // 1 if REAL tag present
    pub is_repack: bool,
}
```

### QualityProfile
```rust
pub struct QualityProfile {
    pub id: i32,
    pub name: String,
    pub cutoff: i32,                    // Quality ID at which upgrades stop
    pub upgrade_allowed: bool,
    pub min_format_score: i32,
    pub cutoff_format_score: i32,
    pub items: serde_json::Value,       // Ordered list of allowed qualities
}
```

### CustomFormat
```rust
pub struct CustomFormat {
    pub id: i32,
    pub name: String,
    pub specifications: serde_json::Value,  // Format matching rules
}
```

---

## Download & Queue Models

### QueueItem
```rust
pub struct QueueItem {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub episode_id: Option<i64>,         // NULL for movies
    pub title: String,                   // Release title
    pub quality: serde_json::Value,
    pub languages: Option<serde_json::Value>,
    pub size: Option<i64>,
    pub status: DownloadStatus,
    pub download_id: String,             // ID from download client
    pub download_client_id: Option<i64>,
    pub indexer_id: Option<i64>,
    pub protocol: DownloadProtocol,
    pub error_message: Option<String>,
    pub added_at: DateTime<Utc>,
}
```

### HistoryEvent
```rust
pub struct HistoryEvent {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub episode_id: Option<i64>,
    pub event_type: HistoryEventType,
    pub quality: serde_json::Value,
    pub languages: Option<serde_json::Value>,
    pub source_title: String,
    pub download_id: Option<String>,
    pub indexer_id: Option<i64>,
    pub download_client: Option<String>,
    pub data: Option<serde_json::Value>,  // Event-specific data
    pub occurred_at: DateTime<Utc>,
}
```

### ReleaseInfo (Search Results)
```rust
pub struct ReleaseInfo {
    pub guid: String,
    pub title: String,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub indexer_id: i64,
    pub indexer_name: String,
    pub protocol: DownloadProtocol,
    pub size: i64,
    pub age_days: i64,
    pub publish_date: DateTime<Utc>,
    pub info_hash: Option<String>,        // Torrent only
    pub magnet_url: Option<String>,       // Torrent only
    pub seeders: Option<i32>,             // Torrent only
    pub leechers: Option<i32>,            // Torrent only
    pub nzb_url: Option<String>,          // Usenet only
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    pub categories: Vec<i32>,
    pub indexer_flags: Vec<String>,
}
```

---

## Configuration Models

### DownloadClientConfig
```rust
pub struct DownloadClientConfig {
    pub id: i64,
    pub name: String,
    pub client_type: String,              // "transmission", "qbittorrent", "sabnzbd", "nzbget", "rtbit", "nzb"
    pub protocol: DownloadProtocol,
    pub config: serde_json::Value,        // Client-specific config
    pub enabled: bool,
    pub priority: i32,

    // Health check fields (migration 003)
    pub last_health_check: Option<DateTime<Utc>>,
    pub health_status: Option<String>,
    pub consecutive_failures: i32,
    pub auto_disabled: bool,
}
```

### IndexerConfig
```rust
pub struct IndexerConfig {
    pub id: i64,
    pub name: String,
    pub indexer_type: String,             // "newznab", "torznab", "cardigann", "indexarr"
    pub base_url: String,
    pub api_key: Option<String>,
    pub protocol: DownloadProtocol,
    pub categories: Option<Vec<i32>>,
    pub enabled: bool,
    pub priority: i32,
    pub supports_search: bool,
    pub supports_rss: bool,
    pub config: Option<serde_json::Value>,
    pub last_rss_sync: Option<DateTime<Utc>>,

    // Health check fields (migration 003)
    pub last_health_check: Option<DateTime<Utc>>,
    pub health_status: Option<String>,
    pub consecutive_failures: i32,
    pub auto_disabled: bool,
}
```

### MediaLibraryFolder
```rust
pub struct MediaLibraryFolder {
    pub id: i64,
    pub path: String,
    pub media_type: MediaType,
    pub free_space: Option<i64>,          // Bytes
    pub last_checked: Option<DateTime<Utc>>,
}
```

---

## Integration Models

### ImportList
```rust
pub struct ImportList {
    pub id: i64,
    pub name: String,
    pub list_type: String,                // "trakt", "tmdb_list", "imdb_list", etc.
    pub media_type: String,               // "series" or "movie"
    pub config: serde_json::Value,
    pub quality_profile_id: Option<i64>,
    pub media_library_folder_id: Option<i64>,
    pub monitored: bool,
    pub enabled: bool,
    pub poll_interval_secs: i32,
}
```

### Plex Models
```rust
pub struct PlexServer {
    pub id: i64,
    pub name: String,
    pub machine_id: String,
    pub ip: String,
    pub port: i32,
    pub use_ssl: bool,
    pub auth_token: String,
    pub web_app_url: Option<String>,
}

pub struct PlexLibrary {
    pub id: i64,
    pub plex_server_id: i64,
    pub section_id: String,
    pub name: String,
    pub enabled: bool,
    pub library_type: String,             // "show", "movie"
    pub last_scan: Option<DateTime<Utc>>,
}

pub struct WatchlistItem {
    pub id: i64,
    pub tmdb_id: i64,
    pub media_type: MediaType,
    pub plex_rating_key: Option<String>,
    pub auto_requested: bool,
}
```

---

## Parser Output Models

### ParsedRelease
```rust
pub struct ParsedRelease {
    pub title: String,                    // Cleaned series/movie title
    pub quality: QualityModel,
    pub episode_info: EpisodeInfo,
    pub languages: Vec<Language>,
    pub release_group: Option<String>,    // e.g., "GROUP"
    pub release_hash: Option<String>,     // 8-char hex in brackets
    pub year: Option<i32>,
    pub edition: Option<String>,          // Director's Cut, etc.
    pub imdb_id: Option<String>,          // tt####### pattern
}
```

### EpisodeInfo
```rust
pub struct EpisodeInfo {
    pub season_number: Option<i32>,
    pub episode_numbers: Vec<i32>,
    pub absolute_episode_numbers: Vec<i32>,
    pub air_date: Option<NaiveDate>,       // Daily shows
    pub is_full_season: bool,
    pub is_multi_season: bool,
    pub is_special: bool,                  // Season 0
}
```

---

## Streaming Models

### StreamSession
```rust
pub struct StreamSession {
    pub id: Uuid,
    pub media_file_id: i64,
    pub session_type: String,              // "direct" or "transcode"
    pub status: String,                    // "active", "paused", "completed", "error"
    pub started_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub transcode_progress: Option<f32>,   // 0.0 to 1.0
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub resolution: Option<String>,
    pub bitrate: Option<i32>,
    pub client_info: Option<String>,
    pub transcode_dir: Option<String>,
}
```

### MediaStreamInfo (ffprobe output)
```rust
pub struct MediaStreamInfo {
    pub video_streams: Vec<VideoStream>,
    pub audio_streams: Vec<AudioStream>,
    pub subtitle_streams: Vec<SubtitleStream>,
    pub duration_secs: f64,
    pub format: String,
    pub bitrate: Option<i64>,
}
```

---

## Auth Models

### AuthStatus
```rust
pub struct AuthStatus {
    pub setup_required: bool,       // true if no users exist (first boot)
    pub registration_enabled: bool, // true if invite-based registration is available
}
```

### SetupRequest
```rust
pub struct SetupRequest {
    pub username: String,
    pub password: String,
    pub display_name: String,
}
```

### LoginRequest
```rust
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub device_name: Option<String>,  // if provided, returns a persistent deviceToken
}
```

### LoginResponse
```rust
pub struct LoginResponse {
    pub user: UserInfo,
    pub device_token: Option<Uuid>,   // present only if device_name was sent
}
```

### BootstrapNameStatus
```rust
pub struct BootstrapNameStatus {
    pub enabled: bool,
    pub name_registered: bool,
    pub server_name: Option<String>,
}
```

### BootstrapRegisterNameResponse
```rust
pub struct BootstrapRegisterNameResponse {
    pub server_name: String,
    pub recovery_phrase: String,       // BIP39 12-word mnemonic
}
```

---

## Remote Access Models

### RemoteClient
```rust
pub struct RemoteClient {
    pub id: i32,
    pub client_token: Uuid,
    pub client_name: String,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
    pub revoked: bool,
}
```

---

## Blocklist Model

### Blocklist
```rust
pub struct Blocklist {
    pub id: i64,
    pub media_type: MediaType,
    pub media_id: i64,
    pub source_title: String,
    pub quality: serde_json::Value,
    pub languages: Option<serde_json::Value>,
    pub indexer_id: Option<i64>,
    pub info_hash: Option<String>,
    pub message: Option<String>,
    pub added_at: DateTime<Utc>,
}
```

---

## Notification Models

### NotificationEvent
```rust
pub enum NotificationEvent {
    Grab { title: String, quality: String, indexer: String },
    Import { title: String, quality: String },
    Upgrade { title: String, old_quality: String, new_quality: String },
    HealthIssue { source: String, message: String },
    DownloadFailure { title: String, message: String },
}
```

---

## Relationships

```
Series 1──* Season
Series 1──* Episode
Episode *──* MediaFile (via episode_files junction)
Movie   1──? MediaFile (movie_file_id)
Series/Movie ──* AlternativeTitle
Series/Movie ──* QualityProfile (quality_profile_id)
Series/Movie ──* MediaLibraryFolder (media_library_folder_id)
QueueItem ──? DownloadClientConfig
QueueItem ──? IndexerConfig
HistoryEvent ──? IndexerConfig
Blocklist ──? IndexerConfig
PlexServer 1──* PlexLibrary
MediaFile 1──* StreamSession (media_file_id)
```
