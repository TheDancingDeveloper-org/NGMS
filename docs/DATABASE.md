# Database

PostgreSQL 17 is required. SQLite is only used for reading *arr migration databases (rusqlite in `stackarr-migrate`).

## Connection

```rust
// stackarr-core/src/db.rs
pub struct Database { pool: PgPool }

impl Database {
    pub async fn connect(config: &DatabaseConfig) -> Result<Self>
    pub fn pool(&self) -> &PgPool
    pub async fn run_migrations(&self) -> Result<()>
    pub async fn is_first_boot(&self) -> Result<bool>
    pub async fn load_enabled_modules(&self) -> Result<EnabledModules>
    pub async fn save_enabled_modules(&self, modules: &EnabledModules) -> Result<()>
}
```

Connection string format: `postgresql://user:pass@host:port/dbname`
Default max connections: 20 (configurable in `[database]` section).

## Schema Overview

14 migration files in `migrations/`:

| Migration | Description |
|-----------|-------------|
| `001_initial.sql` | All core tables, seeded data |
| `002_streaming.sql` | `streaming_sessions` table |
| `003_health_check.sql` | Health check fields on `indexers` and `download_clients` |
| `004_remote_access.sql` | `remote_clients` table |
| `005_quality_profile_media_type.sql` | Add `media_type` column to `quality_profiles` |
| `006_users.sql` | User system: users, sessions, devices, invites, watch progress, requests, watchlist, ratings, notifications, push subscriptions; links `streaming_sessions` to users |
| `007_language_queue.sql` | Add `language` to `quality_profiles`, `original_language` to `movies`, queue media index |
| `008_system_activities.sql` | `system_activities` table for background task tracking |
| `009_plex_verify_tls.sql` | Add `verify_tls` to `plex_servers` |
| `010_media_management.sql` | `recycle_bin` table, seed media management config |
| `011_plex_deep_integration.sql` | Plex events, webhook secrets, unified streaming |
| `012_rss.sql` | RSS feed subscriptions |
| `013_queue_output_path.sql` | Add `output_path` and `stale_count` to `queue` |
| `014_custom_format_fields.sql` | Add `include_custom_format_when_renaming` to `custom_formats`, `min_upgrade_format_score` to `quality_profiles` |

### Table Groups

#### Configuration
| Table | Purpose |
|-------|---------|
| `app_config` | Key-value config store (key TEXT PK, value JSONB) |
| `enabled_modules` | Module on/off flags (module_name TEXT PK, enabled BOOL, config JSONB) |
| `naming_config` | File naming patterns per media type |

#### Media Libraries
| Table | Purpose |
|-------|---------|
| `media_library_folders` | Root directories for TV/Movies (path, media_type, free_space) |
| `tags` | User-defined tags for categorization |

#### Quality
| Table | Purpose |
|-------|---------|
| `quality_profiles` | Named profiles with cutoff, upgrade settings, items (JSONB), media_type, language, min_upgrade_format_score |
| `custom_formats` | Custom format rules (specifications JSONB, include_custom_format_when_renaming) |
| `custom_format_scores` | Profile <> format junction with score |

#### TV Series
| Table | Purpose |
|-------|---------|
| `series` | Series metadata (title, external IDs, path, status, images, genres, tags) |
| `seasons` | Season records with monitored flag |
| `episodes` | Episode details (numbering, air dates, file reference) |
| `episode_files` | Episode <> media_file junction (multi-episode support) |

#### Movies
| Table | Purpose |
|-------|---------|
| `movies` | Movie metadata (title, external IDs, path, availability, dates, original_language) |

#### Shared
| Table | Purpose |
|-------|---------|
| `media_files` | File records (path, size, quality JSONB, languages JSONB, scene info) |
| `alternative_titles` | Alt titles for matching (clean_title, scene_name flag) |

#### Downloads
| Table | Purpose |
|-------|---------|
| `indexers` | Indexer configs (type, URL, API key, categories, protocol) |
| `download_clients` | Download client configs (type, protocol, config JSONB) |
| `queue` | In-progress downloads (status, download_id, error tracking) |
| `history` | Event log (grabbed/imported/failed/renamed/ignored) |
| `blocklist` | Rejected releases |

#### Integrations
| Table | Purpose |
|-------|---------|
| `notification_providers` | Notification configs (type, events, config JSONB) |
| `import_lists` | External list sources (type, media_type, config JSONB) |
| `discover_sliders` | Homepage content sections |
| `plex_servers` | Plex server connections (with verify_tls toggle) |
| `plex_libraries` | Plex library mappings |
| `watchlist` | Plex watchlist items |

#### Streaming (migration 002)
| Table | Purpose |
|-------|---------|
| `streaming_sessions` | Active/completed streaming sessions (type, status, transcode progress, codecs, user_id) |

#### Remote Access (migration 004)
| Table | Purpose |
|-------|---------|
| `remote_clients` | Bootstrap-paired remote client tokens and metadata |

#### Users & Authentication (migration 006)
| Table | Purpose |
|-------|---------|
| `users` | User accounts (username, password_hash, role, avatar, enabled) |
| `user_sessions` | Web login sessions with expiry and activity tracking |
| `user_devices` | Per-user device registrations (replaces `remote_clients` for user-scoped access) |
| `invites` | Invite codes for user registration (created_by, claimed_by, role, expiry) |

#### User Engagement (migration 006)
| Table | Purpose |
|-------|---------|
| `watch_progress` | Per-user playback position tracking (continue watching) |
| `media_requests` | User-submitted media requests with approval workflow |
| `user_watchlist` | Per-user watchlist (distinct from Plex `watchlist`) |
| `user_ratings` | User ratings (1-10 scale) per media item |
| `user_notifications` | In-app notification inbox per user |
| `push_subscriptions` | Web Push API subscriptions per user |

#### System (migration 008)
| Table | Purpose |
|-------|---------|
| `system_activities` | Background task tracking (disk scans, imports, transcodes) |

#### Media Management (migration 010)
| Table | Purpose |
|-------|---------|
| `recycle_bin` | Files moved to recycle bin pending scheduled cleanup |

### Health Check Fields (migration 003)

Added to `indexers` and `download_clients`:

| Column | Type | Purpose |
|--------|------|---------|
| `last_health_check` | TIMESTAMPTZ | When last health check ran |
| `health_status` | TEXT | Current health status |
| `consecutive_failures` | INTEGER | Failure count for auto-disable |
| `auto_disabled` | BOOLEAN | Whether auto-disabled due to failures |

### Streaming Sessions (migration 002, updated in 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | UUID PK | Session identifier |
| `media_file_id` | BIGINT FK | Media file being streamed |
| `session_type` | TEXT | `'direct'` or `'transcode'` |
| `status` | TEXT | `'active'`, `'paused'`, `'completed'`, `'error'` |
| `started_at` | TIMESTAMPTZ | Session start time |
| `last_activity` | TIMESTAMPTZ | Last activity timestamp |
| `transcode_progress` | REAL | 0.0 to 1.0 |
| `video_codec` | TEXT | Video codec used |
| `audio_codec` | TEXT | Audio codec used |
| `resolution` | TEXT | Output resolution |
| `bitrate` | BIGINT | Output bitrate |
| `client_info` | TEXT | Client identifier |
| `transcode_dir` | TEXT | Transcode output directory |
| `user_id` | BIGINT FK | User who owns the session (added in migration 006) |

### Remote Clients (migration 004)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | SERIAL PK | Auto-increment ID |
| `client_token` | UUID UNIQUE | Authentication token |
| `client_name` | TEXT | Human-readable client name |
| `created_at` | TIMESTAMPTZ | When client was registered |
| `last_seen` | TIMESTAMPTZ | Last API access |
| `revoked` | BOOLEAN | Whether access has been revoked |

### Quality Profile Updates (migrations 005, 007, 014)

Added to `quality_profiles`:

| Column | Type | Purpose |
|--------|------|---------|
| `media_type` | TEXT | Scopes profile to `'series'`, `'movie'`, or NULL for any |
| `language` | INTEGER NOT NULL DEFAULT -1 | Language preference: -1 = any, -2 = original, positive = specific Radarr language ID |
| `min_upgrade_format_score` | INTEGER NOT NULL DEFAULT 1 | Minimum custom format score improvement required for an upgrade to be considered |

### Movie Updates (migration 007)

Added to `movies`:

| Column | Type | Purpose |
|--------|------|---------|
| `original_language` | INTEGER | Radarr language ID (1=English, 2=French, 3=Spanish, etc.) for resolving "Original" language profiles |

### Queue Index (migration 007)

Added index `idx_queue_media` on `queue(media_type, media_id)` for media-item-based conflict checking.

### Plex Server Updates (migration 009)

Added to `plex_servers`:

| Column | Type | Purpose |
|--------|------|---------|
| `verify_tls` | BOOLEAN NOT NULL DEFAULT false | Per-server TLS certificate verification toggle (false for backward compat with self-signed certs) |

### Users (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | User identifier |
| `username` | TEXT NOT NULL UNIQUE | Login name |
| `display_name` | TEXT NOT NULL | Display name |
| `password_hash` | TEXT NOT NULL | Argon2 password hash |
| `role` | TEXT NOT NULL DEFAULT 'user' | Role: `'admin'`, `'user'` |
| `avatar_url` | TEXT | Optional avatar URL |
| `enabled` | BOOLEAN NOT NULL DEFAULT true | Whether account is active |
| `created_at` | TIMESTAMPTZ | Account creation time |
| `updated_at` | TIMESTAMPTZ | Last profile update time |

### User Sessions (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | UUID PK | Session identifier (auto-generated) |
| `user_id` | BIGINT FK | Owning user (CASCADE delete) |
| `token_hash` | TEXT NOT NULL UNIQUE | Hashed session token |
| `user_agent` | TEXT | Browser/client user agent |
| `ip_address` | INET | Client IP address |
| `created_at` | TIMESTAMPTZ | Session creation time |
| `expires_at` | TIMESTAMPTZ | Session expiry time |
| `last_active` | TIMESTAMPTZ | Last request time |

Indexes: `idx_user_sessions_user(user_id)`, `idx_user_sessions_expires(expires_at)`.

### User Devices (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | SERIAL PK | Auto-increment ID |
| `user_id` | BIGINT FK | Owning user (CASCADE delete) |
| `device_token` | UUID NOT NULL UNIQUE | Authentication token |
| `device_name` | TEXT | Human-readable device name |
| `device_type` | TEXT | Device type (e.g., "android", "ios") |
| `created_at` | TIMESTAMPTZ | Registration time |
| `last_seen` | TIMESTAMPTZ | Last API access |
| `revoked` | BOOLEAN NOT NULL DEFAULT false | Whether device access is revoked |

Indexes: `idx_user_devices_token(device_token)`, `idx_user_devices_user(user_id)`.

### Invites (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | SERIAL PK | Auto-increment ID |
| `code` | TEXT NOT NULL UNIQUE | Invite code string |
| `created_by` | BIGINT FK | Admin user who created the invite |
| `claimed_by` | BIGINT FK | User who redeemed the invite (NULL if unclaimed) |
| `role` | TEXT NOT NULL DEFAULT 'user' | Role assigned on claim |
| `expires_at` | TIMESTAMPTZ | Optional expiry time |
| `created_at` | TIMESTAMPTZ | Creation time |

Index: `idx_invites_code(code)`.

### Watch Progress (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | Progress record ID |
| `user_id` | BIGINT FK | Owning user (CASCADE delete) |
| `media_file_id` | BIGINT FK | Media file being watched (CASCADE delete) |
| `media_type` | TEXT NOT NULL | `'series'` or `'movie'` |
| `media_id` | BIGINT NOT NULL | ID of series or movie |
| `episode_id` | BIGINT | Episode ID (NULL for movies) |
| `position_secs` | REAL NOT NULL DEFAULT 0 | Current playback position in seconds |
| `duration_secs` | REAL NOT NULL DEFAULT 0 | Total duration in seconds |
| `completed` | BOOLEAN NOT NULL DEFAULT false | Whether playback is complete |
| `updated_at` | TIMESTAMPTZ | Last progress update |

Unique constraint: `(user_id, media_file_id)`.
Indexes: `idx_watch_progress_user(user_id, updated_at DESC)`, `idx_watch_progress_continue(user_id, completed, updated_at DESC)`, `idx_watch_progress_media(media_type, media_id)`.

### Media Requests (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | Request ID |
| `user_id` | BIGINT FK | Requesting user |
| `media_type` | TEXT NOT NULL | `'series'` or `'movie'` |
| `tmdb_id` | BIGINT NOT NULL | TMDB ID of requested media |
| `title` | TEXT NOT NULL | Requested media title |
| `year` | INTEGER | Release year |
| `poster_url` | TEXT | Poster image URL |
| `overview` | TEXT | Media description |
| `status` | TEXT NOT NULL DEFAULT 'pending' | `'pending'`, `'approved'`, `'denied'`, `'available'` |
| `admin_note` | TEXT | Admin response note |
| `approved_by` | BIGINT FK | Admin who approved/denied |
| `created_at` | TIMESTAMPTZ | Request creation time |
| `updated_at` | TIMESTAMPTZ | Last status update |

Unique constraint: `(tmdb_id, media_type)`.
Indexes: `idx_media_requests_user(user_id)`, `idx_media_requests_status(status)`.

### User Watchlist (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | Watchlist entry ID |
| `user_id` | BIGINT FK | Owning user (CASCADE delete) |
| `media_type` | TEXT NOT NULL | `'series'` or `'movie'` |
| `media_id` | BIGINT NOT NULL | ID of series or movie |
| `tmdb_id` | BIGINT NOT NULL | TMDB ID |
| `added_at` | TIMESTAMPTZ | When added to watchlist |

Unique constraint: `(user_id, media_type, media_id)`.
Index: `idx_user_watchlist_user(user_id, added_at DESC)`.

### User Ratings (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | Rating ID |
| `user_id` | BIGINT FK | Owning user (CASCADE delete) |
| `media_type` | TEXT NOT NULL | `'series'` or `'movie'` |
| `media_id` | BIGINT NOT NULL | ID of series or movie |
| `rating` | SMALLINT NOT NULL | Rating value (CHECK: 1-10) |
| `created_at` | TIMESTAMPTZ | When rated |
| `updated_at` | TIMESTAMPTZ | Last rating change |

Unique constraint: `(user_id, media_type, media_id)`.
Indexes: `idx_user_ratings_user(user_id)`, `idx_user_ratings_media(media_type, media_id)`.

### User Notifications (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | Notification ID |
| `user_id` | BIGINT FK | Target user (CASCADE delete) |
| `notification_type` | TEXT NOT NULL | Event type (e.g., `'media_available'`, `'request_approved'`) |
| `title` | TEXT NOT NULL | Notification title |
| `body` | TEXT | Notification body text |
| `data` | JSONB | Structured payload (media IDs, links, etc.) |
| `read` | BOOLEAN NOT NULL DEFAULT false | Read/unread state |
| `created_at` | TIMESTAMPTZ | When notification was created |

Indexes: `idx_user_notifications_user(user_id, read, created_at DESC)`, `idx_user_notifications_created(created_at)`.

### Push Subscriptions (migration 006)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | Subscription ID |
| `user_id` | BIGINT FK | Owning user (CASCADE delete) |
| `endpoint` | TEXT NOT NULL UNIQUE | Web Push endpoint URL |
| `p256dh` | TEXT NOT NULL | ECDH public key |
| `auth` | TEXT NOT NULL | Auth secret |
| `user_agent` | TEXT | Client user agent |
| `created_at` | TIMESTAMPTZ | When subscription was registered |

Index: `idx_push_subscriptions_user(user_id)`.

### System Activities (migration 008)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | Activity ID |
| `activity_type` | TEXT NOT NULL | Activity kind (e.g., `'disk_scan'`, `'import'`, `'transcode'`) |
| `status` | TEXT NOT NULL DEFAULT 'running' | `'running'`, `'completed'`, `'failed'` |
| `title` | TEXT NOT NULL | Human-readable activity title |
| `detail` | TEXT | Additional detail text |
| `progress` | JSONB | Structured progress data (percentComplete, currentItem, etc.) |
| `result` | JSONB | Structured result data on completion |
| `error` | TEXT | Error message on failure |
| `started_at` | TIMESTAMPTZ | Activity start time |
| `updated_at` | TIMESTAMPTZ | Last progress update time |
| `completed_at` | TIMESTAMPTZ | Completion time (NULL while running) |

Indexes: `idx_system_activities_status(status, started_at DESC)`, `idx_system_activities_recent(started_at DESC)`.

### Recycle Bin (migration 010)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | BIGSERIAL PK | Entry ID |
| `original_path` | TEXT NOT NULL | Original file path before recycling |
| `recycle_path` | TEXT NOT NULL | Path in recycle bin directory |
| `media_file_id` | BIGINT | Former media_files ID (NULL if record deleted) |
| `media_type` | TEXT NOT NULL | `'series'` or `'movie'` |
| `media_id` | BIGINT NOT NULL | ID of associated series or movie |
| `size` | BIGINT NOT NULL DEFAULT 0 | File size in bytes |
| `recycled_at` | TIMESTAMPTZ | When file was moved to recycle bin |

Index: `idx_recycle_bin_recycled_at(recycled_at)`.

### App Config Seeded Data (migration 010)

Media management defaults added to `app_config`:

| Key | Default Value | Purpose |
|-----|---------------|---------|
| `recycle_bin_path` | `""` (empty string) | Directory path for recycled files (empty = disabled) |
| `recycle_bin_cleanup_days` | `7` | Days before recycled files are permanently deleted |

### Custom Format Fields (migration 014)

Added to `custom_formats`:

| Column | Type | Purpose |
|--------|------|---------|
| `include_custom_format_when_renaming` | BOOLEAN NOT NULL DEFAULT false | Whether to include this custom format's name in the renamed file path |

Added to `quality_profiles`:

| Column | Type | Purpose |
|--------|------|---------|
| `min_upgrade_format_score` | INTEGER NOT NULL DEFAULT 1 | Minimum custom format score improvement required for an upgrade to be considered |

## Bootstrap SQLite Database

The standalone `stackarr-bootstrap` binary uses a separate SQLite database (not PostgreSQL) for its own persistence. This is the only component that writes to SQLite.

### server_names

| Column | Type | Purpose |
|--------|------|---------|
| `name` | TEXT PK | Human-readable server name |
| `server_id` | TEXT | UUID of the registered StackArr server |
| `recovery_hash` | TEXT | Hash of the BIP39 12-word recovery phrase |
| `local_ip` | TEXT | Server's local/LAN IP |
| `public_ip` | TEXT | Server's public IP |
| `port` | INTEGER | Server's advertised port |
| `registered_at` | TEXT | ISO 8601 timestamp |

### pending_claims

| Column | Type | Purpose |
|--------|------|---------|
| `code` | TEXT PK | 8-character claim code |
| `server_id` | TEXT | UUID of the server that created the claim |
| `claim_type` | TEXT | Type of claim (e.g., `"invite"`) |
| `invite_code` | TEXT | The invite code for account registration (matches `code` for unified claims) |
| `local_ip` | TEXT | Server's local IP |
| `public_ip` | TEXT | Server's public IP |
| `port` | INTEGER | Server's port |
| `created_at` | TEXT | ISO 8601 timestamp |

---

## Model Structs

All models are defined in `crates/stackarr-core/src/models/user.rs` and derive `FromRow`, `Serialize`, `Deserialize`.

### User

```rust
pub struct User {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub role: String,
    pub avatar_url: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### UserSession

```rust
pub struct UserSession {
    pub id: Uuid,
    pub user_id: i64,
    pub token_hash: String,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}
```

### UserDevice

```rust
pub struct UserDevice {
    pub id: i32,
    pub user_id: i64,
    pub device_token: Uuid,
    pub device_name: Option<String>,
    pub device_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
    pub revoked: bool,
}
```

### Invite

```rust
pub struct Invite {
    pub id: i32,
    pub code: String,
    pub created_by: i64,
    pub claimed_by: Option<i64>,
    pub role: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
```

### WatchProgress

```rust
pub struct WatchProgress {
    pub id: i64,
    pub user_id: i64,
    pub media_file_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub episode_id: Option<i64>,
    pub position_secs: f32,
    pub duration_secs: f32,
    pub completed: bool,
    pub updated_at: DateTime<Utc>,
}
```

### MediaRequest

```rust
pub struct MediaRequest {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub tmdb_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub poster_url: Option<String>,
    pub overview: Option<String>,
    pub status: String,
    pub admin_note: Option<String>,
    pub approved_by: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### UserWatchlistItem

```rust
pub struct UserWatchlistItem {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub tmdb_id: i64,
    pub added_at: DateTime<Utc>,
}
```

### UserRating

```rust
pub struct UserRating {
    pub id: i64,
    pub user_id: i64,
    pub media_type: String,
    pub media_id: i64,
    pub rating: i16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### UserNotification

```rust
pub struct UserNotification {
    pub id: i64,
    pub user_id: i64,
    pub notification_type: String,
    pub title: String,
    pub body: Option<String>,
    pub data: Option<serde_json::Value>,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}
```

### PushSubscription

```rust
pub struct PushSubscription {
    pub id: i64,
    pub user_id: i64,
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}
```

### SystemActivity

```rust
pub struct SystemActivity {
    pub id: i64,
    pub activity_type: String,
    pub status: String,
    pub title: String,
    pub detail: Option<String>,
    pub progress: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

### RecycleBinEntry

Defined in `crates/stackarr-import/src/recycle_bin.rs`:

```rust
pub struct RecycleBinEntry {
    pub id: i64,
    pub original_path: String,
    pub recycle_path: String,
    pub media_file_id: Option<i64>,
    pub media_type: String,
    pub media_id: i64,
    pub size: i64,
    pub recycled_at: DateTime<Utc>,
}
```

---

## Key Column Patterns

### JSONB Columns

Used for flexible/nested data that doesn't need individual column indexing:

- `quality` -- `{"quality": {"id": 11, "name": "WEBDL-1080p"}, "revision": {"version": 1, "real": 0, "isRepack": false}}`
- `languages` -- `[{"id": 1, "name": "English"}]`
- `images` -- `[{"coverType": "poster", "url": "https://..."}]`
- `config` -- provider/client-specific configuration
- `items` -- quality profile items (ordered list of allowed qualities)
- `custom_data` -- discover slider configuration
- `data` -- user notification structured payload
- `progress` -- system activity progress data
- `result` -- system activity result data

### Array Columns

- `genres TEXT[]` -- genre strings
- `tags INT[]` -- tag IDs
- `categories INT[]` -- indexer category IDs

### External IDs

Series and movies store multiple external identifiers:

| Column | Source |
|--------|--------|
| `tvdb_id` | TheTVDB |
| `tmdb_id` | TMDB |
| `imdb_id` | IMDb (string, e.g., "tt0903747") |
| `tvmaze_id` | TVMaze |
| `mal_id` | MyAnimeList |
| `plex_rating_key` | Plex |
| `plex_rating_key_4k` | Plex (4K library) |

### Timestamps

All timestamps are `TIMESTAMPTZ` (stored as UTC):
- `added_at` -- when the record was created (DEFAULT NOW())
- `created_at` -- when the record was created (DEFAULT NOW())
- `updated_at` -- when the record was last modified
- `last_info_sync` -- when metadata was last refreshed
- `last_search_time` -- when a search was last performed
- `occurred_at` -- when a history event happened
- `last_rss_sync` -- when an indexer RSS feed was last polled
- `expires_at` -- session/invite expiry time
- `last_active` -- last session activity
- `last_seen` -- last device API access
- `recycled_at` -- when a file was moved to recycle bin
- `started_at` / `completed_at` -- activity lifecycle timestamps

## Seeded Data

The initial migration seeds:

**3 Quality Profiles:**
1. "Any" -- all 19 quality levels enabled
2. "HD-1080p" -- HDTV/WEBDL/WEBRip/Bluray 1080p
3. "Ultra-HD" -- 2160p variants + Remux

**Naming Configs:**
- Series: standard, daily, anime formats + season folder
- Movie: standard format + folder format

**8 Discover Sliders:**
- Trending, Popular Movies, Popular TV, Upcoming Movies, Upcoming TV, Recently Added, Movie Genres, TV Genres

**Media Management Config (migration 010):**
- `recycle_bin_path` = `""` (disabled by default)
- `recycle_bin_cleanup_days` = `7`

## Query Patterns

### Direct SQL with sqlx

```rust
// Typed query with FromRow
let series = sqlx::query_as::<_, Series>("SELECT * FROM series WHERE id = $1")
    .bind(id)
    .fetch_one(pool)
    .await?;

// Insert returning ID
let row = sqlx::query_scalar::<_, i64>(
    "INSERT INTO series (title, clean_title, path, ...) VALUES ($1, $2, $3, ...) RETURNING id"
)
    .bind(&input.title)
    .bind(&clean)
    .bind(&input.path)
    .fetch_one(pool)
    .await?;

// Update
sqlx::query("UPDATE series SET title = $1, monitored = $2 WHERE id = $3")
    .bind(&input.title)
    .bind(input.monitored)
    .bind(id)
    .execute(pool)
    .await?;

// Delete
sqlx::query("DELETE FROM series WHERE id = $1")
    .bind(id)
    .execute(pool)
    .await?;
```

### Pagination Pattern

```rust
let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM history")
    .fetch_one(pool).await?;

let records = sqlx::query_as::<_, HistoryEvent>(
    "SELECT * FROM history ORDER BY occurred_at DESC LIMIT $1 OFFSET $2"
)
    .bind(page_size)
    .bind((page - 1) * page_size)
    .fetch_all(pool).await?;
```

### JSONB Queries

```rust
// Insert JSONB
sqlx::query("INSERT INTO media_files (quality, languages, ...) VALUES ($1::jsonb, $2::jsonb, ...)")
    .bind(serde_json::to_value(&quality)?)
    .bind(serde_json::to_value(&languages)?)
    .execute(pool).await?;
```

### User Authentication

```rust
// Look up user by username for login
let user = sqlx::query_as::<_, User>(
    "SELECT * FROM users WHERE username = $1 AND enabled = true"
)
    .bind(&username)
    .fetch_optional(pool)
    .await?;

// Create session after password verification
let session_id = sqlx::query_scalar::<_, Uuid>(
    "INSERT INTO user_sessions (user_id, token_hash, user_agent, ip_address, expires_at)
     VALUES ($1, $2, $3, $4::inet, $5) RETURNING id"
)
    .bind(user.id)
    .bind(&token_hash)
    .bind(&user_agent)
    .bind(&ip_address)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;

// Validate session token (middleware)
let session = sqlx::query_as::<_, UserSession>(
    "SELECT * FROM user_sessions WHERE token_hash = $1 AND expires_at > NOW()"
)
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;
```

### Watch Progress

```rust
// Upsert playback position (ON CONFLICT on unique(user_id, media_file_id))
sqlx::query(
    "INSERT INTO watch_progress (user_id, media_file_id, media_type, media_id, episode_id, position_secs, duration_secs, completed)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
     ON CONFLICT (user_id, media_file_id) DO UPDATE SET
         position_secs = EXCLUDED.position_secs,
         duration_secs = EXCLUDED.duration_secs,
         completed = EXCLUDED.completed,
         updated_at = NOW()"
)
    .bind(user_id)
    .bind(media_file_id)
    .bind(&media_type)
    .bind(media_id)
    .bind(episode_id)
    .bind(position_secs)
    .bind(duration_secs)
    .bind(completed)
    .execute(pool)
    .await?;

// Continue watching: incomplete items ordered by recent activity
let items = sqlx::query_as::<_, WatchProgress>(
    "SELECT * FROM watch_progress
     WHERE user_id = $1 AND completed = false
     ORDER BY updated_at DESC LIMIT $2"
)
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
```

## Adding a New Migration

1. Create `migrations/NNN_description.sql` (e.g., `011_add_feature.sql`)
2. Write SQL -- sqlx runs migrations in filename order
3. Add corresponding model structs to `stackarr-core/src/models.rs`
4. Derive `FromRow`, `Serialize`, `Deserialize` on new structs
5. Migrations run automatically on startup via `Database::run_migrations()`

## Testing

Integration tests use `stackarr_core::test_helpers::TestDb` (behind `testing` feature flag):
- Creates a temporary database
- Runs all migrations
- Provides a `PgPool` for tests
- Drops the database on `Drop`

```rust
#[tokio::test]
#[ignore] // Requires running Postgres
async fn test_something() {
    let db = TestDb::new("postgresql://stackarr:stackarr@localhost:5433/stackarr").await;
    let pool = db.pool();
    // ... test with pool
}
```
