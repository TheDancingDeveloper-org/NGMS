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

4 migration files in `migrations/`:

| Migration | Description |
|-----------|-------------|
| `001_initial.sql` | All core tables, seeded data |
| `002_streaming.sql` | `streaming_sessions` table |
| `003_health_check.sql` | Health check fields on `indexers` and `download_clients` |
| `004_remote_access.sql` | `remote_clients` table |

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
| `quality_profiles` | Named profiles with cutoff, upgrade settings, items (JSONB) |
| `custom_formats` | Custom format rules (specifications JSONB) |
| `custom_format_scores` | Profile ↔ format junction with score |

#### TV Series
| Table | Purpose |
|-------|---------|
| `series` | Series metadata (title, external IDs, path, status, images, genres, tags) |
| `seasons` | Season records with monitored flag |
| `episodes` | Episode details (numbering, air dates, file reference) |
| `episode_files` | Episode ↔ media_file junction (multi-episode support) |

#### Movies
| Table | Purpose |
|-------|---------|
| `movies` | Movie metadata (title, external IDs, path, availability, dates) |

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
| `plex_servers` | Plex server connections |
| `plex_libraries` | Plex library mappings |
| `watchlist` | Plex watchlist items |

#### Streaming (migration 002)
| Table | Purpose |
|-------|---------|
| `streaming_sessions` | Active/completed streaming sessions (type, status, transcode progress, codecs) |

#### Remote Access (migration 004)
| Table | Purpose |
|-------|---------|
| `remote_clients` | Bootstrap-paired remote client tokens and metadata |

### Health Check Fields (migration 003)

Added to `indexers` and `download_clients`:

| Column | Type | Purpose |
|--------|------|---------|
| `last_health_check` | TIMESTAMPTZ | When last health check ran |
| `health_status` | TEXT | Current health status |
| `consecutive_failures` | INTEGER | Failure count for auto-disable |
| `auto_disabled` | BOOLEAN | Whether auto-disabled due to failures |

### Streaming Sessions (migration 002)

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
| `bitrate` | INTEGER | Output bitrate |
| `client_info` | TEXT | Client identifier |
| `transcode_dir` | TEXT | Transcode output directory |

### Remote Clients (migration 004)

| Column | Type | Purpose |
|--------|------|---------|
| `id` | SERIAL PK | Auto-increment ID |
| `client_token` | UUID UNIQUE | Authentication token |
| `client_name` | TEXT | Human-readable client name |
| `created_at` | TIMESTAMPTZ | When client was registered |
| `last_seen` | TIMESTAMPTZ | Last API access |
| `revoked` | BOOLEAN | Whether access has been revoked |

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

## Key Column Patterns

### JSONB Columns

Used for flexible/nested data that doesn't need individual column indexing:

- `quality` — `{"quality": {"id": 11, "name": "WEBDL-1080p"}, "revision": {"version": 1, "real": 0, "isRepack": false}}`
- `languages` — `[{"id": 1, "name": "English"}]`
- `images` — `[{"coverType": "poster", "url": "https://..."}]`
- `config` — provider/client-specific configuration
- `items` — quality profile items (ordered list of allowed qualities)
- `custom_data` — discover slider configuration

### Array Columns

- `genres TEXT[]` — genre strings
- `tags INT[]` — tag IDs
- `categories INT[]` — indexer category IDs

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
- `added_at` — when the record was created (DEFAULT NOW())
- `last_info_sync` — when metadata was last refreshed
- `last_search_time` — when a search was last performed
- `occurred_at` — when a history event happened
- `last_rss_sync` — when an indexer RSS feed was last polled

## Seeded Data

The initial migration seeds:

**3 Quality Profiles:**
1. "Any" — all 19 quality levels enabled
2. "HD-1080p" — HDTV/WEBDL/WEBRip/Bluray 1080p
3. "Ultra-HD" — 2160p variants + Remux

**Naming Configs:**
- Series: standard, daily, anime formats + season folder
- Movie: standard format + folder format

**8 Discover Sliders:**
- Trending, Popular Movies, Popular TV, Upcoming Movies, Upcoming TV, Recently Added, Movie Genres, TV Genres

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

## Adding a New Migration

1. Create `migrations/NNN_description.sql` (e.g., `002_add_notifications.sql`)
2. Write SQL — sqlx runs migrations in filename order
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
