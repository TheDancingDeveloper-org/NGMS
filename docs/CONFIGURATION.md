# Configuration

StackArr is configured via a TOML file, environment variables, and CLI flags. CLI/env overrides take precedence over the config file.

## Config File

Default location: `stackarr.toml` in the working directory. Override with `--config PATH` or `STACKARR_CONFIG`.

If the file doesn't exist on startup, a default is generated.

### Full Config Reference

```toml
[general]
instance_name = "StackArr"      # Display name in UI
bind_addr = "0.0.0.0"           # Listen address
port = 8989                     # Listen port
data_dir = "/config"            # Data/config directory
log_level = "info"              # trace, debug, info, warn, error

[database]
url = "postgresql://stackarr:stackarr@localhost:5432/stackarr"
max_connections = 20            # PgPool max connections

[auth]
method = "forms"                # "forms", "basic", "none"
# api_key = "your-api-key"     # Optional API key

[torrent]
enabled = false                 # Enable embedded torrent engine
download_dir = "/downloads/torrent"
complete_dir = "/downloads/torrent-complete"
listen_port = 6881              # DHT/peer listen port
dht_enabled = true              # Enable DHT
peer_limit = 200                # Max peers per torrent
upload_limit_bps = 0            # 0 = unlimited
download_limit_bps = 0          # 0 = unlimited

[usenet]
enabled = false                 # Enable embedded usenet engine
incomplete_dir = "/downloads/usenet/incomplete"
complete_dir = "/downloads/usenet/complete"
max_active_downloads = 3        # Concurrent downloads

[[usenet.servers]]              # NNTP servers (array of tables)
name = "Primary"
host = "news.example.com"
port = 563                      # 563 for SSL, 119 for plain
ssl = true
username = "user"
password = "pass"
connections = 20                # Parallel NNTP connections
priority = 0                    # Lower = higher priority

[indexarr]
enabled = false                 # Enable Indexarr sidecar
url = "http://indexarr:8080"    # Indexarr URL
api_key = ""                    # API key
mode = "peer"                   # "peer" or "full"

[naming.series]
rename = true                   # Enable file renaming on import
standard = "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]"
daily = "{Series Title} - {Air-Date} - {Episode Title} [{Quality Title}]"
anime = "{Series Title} - S{season:00}E{episode:00} - {Absolute Episode} - {Episode Title} [{Quality Title}]"
season_folder = "Season {season:00}"

[naming.movie]
rename = true
standard = "{Movie Title} ({Release Year}) [{Quality Title}]"
folder = "{Movie Title} ({Release Year})"

[streaming]
enabled = false                 # Enable streaming server
transcode_dir = "/config/transcode"  # Temp directory for HLS segments
ffmpeg_path = "ffmpeg"          # Path to ffmpeg binary (auto-detects jellyfin-ffmpeg)
ffprobe_path = "ffprobe"        # Path to ffprobe binary (auto-detects jellyfin-ffmpeg)
segment_duration_secs = 6       # HLS segment duration
max_concurrent_sessions = 3     # Max concurrent transcode sessions

[streaming.hwaccel]
enabled = false                 # Enable hardware acceleration
accel_type = "vaapi"            # "vaapi", "qsv", "nvenc"
# device = "/dev/dri/renderD128" # GPU device path (optional, defaults to /dev/dri/renderD128)

[[streaming.quality_tiers]]     # Adaptive bitrate quality tiers (array of tables)
name = "4K"
max_width = 3840
max_height = 2160
video_bitrate = 40000000        # 40 Mbps
audio_bitrate = 640000          # 640 kbps

[[streaming.quality_tiers]]
name = "1080p"
max_width = 1920
max_height = 1080
video_bitrate = 8000000         # 8 Mbps
audio_bitrate = 192000          # 192 kbps

[[streaming.quality_tiers]]
name = "720p"
max_width = 1280
max_height = 720
video_bitrate = 2500000         # 2.5 Mbps
audio_bitrate = 128000          # 128 kbps

[[streaming.quality_tiers]]
name = "480p"
max_width = 854
max_height = 480
video_bitrate = 1500000         # 1.5 Mbps
audio_bitrate = 96000           # 96 kbps

[bootstrap]
enabled = false                 # Enable remote access via bootstrap
url = ""                        # Bootstrap node URL
token = ""                      # Bootstrap registration token
advertise_port = 9111           # Port to advertise to bootstrap
upnp_enabled = false            # Enable UPnP port forwarding
database_path = "bootstrap.db"  # SQLite database path (bootstrap binary only)
```

## Environment Variables

All env vars are prefixed with `STACKARR_`:

| Variable | Maps To | Example |
|----------|---------|---------|
| `STACKARR_CONFIG` | CLI `--config` | `/config/stackarr.toml` |
| `STACKARR_BIND` | `general.bind_addr` | `0.0.0.0` |
| `STACKARR_PORT` | `general.port` | `8989` |
| `STACKARR_DATABASE_URL` | `database.url` | `postgresql://...` |
| `STACKARR_LOG_LEVEL` | `general.log_level` | `debug` |

Env vars override the TOML file values.

## CLI Arguments

```
USAGE:
    stackarr [OPTIONS] [SUBCOMMAND]

OPTIONS:
    --config <PATH>          Config file path [env: STACKARR_CONFIG]
    --bind <ADDR>            Bind address [env: STACKARR_BIND]
    --port <PORT>            Port number [env: STACKARR_PORT]
    --database-url <URL>     Database URL [env: STACKARR_DATABASE_URL]
    --log-level <LEVEL>      Log level [env: STACKARR_LOG_LEVEL]

SUBCOMMANDS:
    migrate    Import from Sonarr/Radarr/Prowlarr databases
```

### Migrate Subcommand

```
stackarr migrate [OPTIONS]

OPTIONS:
    --sonarr <PATH>      Path to Sonarr SQLite database
    --radarr <PATH>      Path to Radarr SQLite database
    --prowlarr <PATH>    Path to Prowlarr SQLite database
    --dry-run            Show what would be imported without writing
```

## Priority Order

1. CLI arguments (highest)
2. Environment variables
3. TOML config file
4. Default values (lowest)

## Hot Reload

Config is stored as `Arc<ArcSwap<AppConfig>>` in AppState. The config can be swapped atomically without restarting the server. Handlers always read the latest config snapshot.

```rust
// Reading config in a handler:
let config = state.config.load();
let instance_name = &config.general.instance_name;
```

## Feature Flags (Compile-Time)

| Flag | Crate | Effect |
|------|-------|--------|
| `ui` | root | Enable UI serving (default: on) |
| `testing` | stackarr-core | Expose TestDb helper for integration tests |

## Module System (Runtime)

Modules are enabled/disabled at runtime via the `enabled_modules` DB table. Set during first-boot setup (`POST /api/v1/system/setup`).

```rust
pub struct EnabledModules {
    pub tv_management: bool,       // TV series features
    pub movie_management: bool,    // Movie features
    pub torrent_embedded: bool,    // Embedded torrent engine
    pub usenet_embedded: bool,     // Embedded usenet engine
    pub torrent_external: bool,    // External torrent clients
    pub usenet_external: bool,     // External usenet clients
    pub indexarr_sidecar: bool,    // Indexarr integration
    pub external_indexers: bool,   // Newznab/Torznab indexers
    pub plex_integration: bool,    // Plex server integration
    pub notifications: bool,       // Notification providers
    pub streaming: bool,           // Video streaming server
    pub remote_access: bool,       // Bootstrap remote access
}
```

Modules control:
- Which scheduler tasks spawn
- Which engines are initialized
- Which UI navigation items appear
- Which API endpoints return data vs 503

## Media Management Settings (Runtime)

These settings are stored in the `app_config` database table (not the TOML file) and managed via the Media Management API (`/api/v1/mediamanagement`):

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `recycle_bin_path` | string | `""` (empty = disabled) | Directory path for recycled files. When set, replaced files are moved here instead of permanently deleted. |
| `recycle_bin_cleanup_days` | integer | `7` | Days before recycled files are permanently deleted. Set to `0` to keep forever. |

When a file upgrade occurs (e.g., a higher-quality release replaces an existing file), the old file is moved to the recycle bin directory if configured. A scheduler task periodically cleans up expired entries.

## Naming Tokens

### Series
| Token | Example |
|-------|---------|
| `{Series Title}` | `Breaking Bad` |
| `{season:00}` | `01` (zero-padded) |
| `{episode:00}` | `02` (zero-padded) |
| `{Episode Title}` | `Pilot` |
| `{Quality Title}` | `WEBDL-1080p` |
| `{Air-Date}` | `2024-01-15` |
| `{Absolute Episode:000}` | `001` (anime) |
| `{Release Group}` | `GROUP` |

### Movie
| Token | Example |
|-------|---------|
| `{Movie Title}` | `Inception` |
| `{Release Year}` | `2010` |
| `{Quality Title}` | `Bluray-1080p` |
| `{Edition Tags}` | `Director's Cut` |
| `{Release Group}` | `GROUP` |

### Colon Replacement Strategies

When renaming files, colons are handled by the `colon_replacement` setting:

| Strategy | Input | Output |
|----------|-------|--------|
| `smart` | `Title: Subtitle` | `Title - Subtitle` |
| `dash` | `Title: Subtitle` | `Title- Subtitle` |
| `space` | `Title: Subtitle` | `Title  Subtitle` |
| `spacedash` | `Title: Subtitle` | `Title - Subtitle` |
| (other) | `Title: Subtitle` | `Title Subtitle` |
