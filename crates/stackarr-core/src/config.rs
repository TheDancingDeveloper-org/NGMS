use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    #[serde(default)]
    pub torrent: TorrentConfig,
    #[serde(default)]
    pub usenet: UsenetConfig,
    #[serde(default)]
    pub indexarr: IndexarrConfig,
    #[serde(default)]
    pub naming: NamingConfig,
    #[serde(default)]
    pub streaming: StreamingConfig,
    #[serde(default)]
    pub bootstrap: BootstrapConfig,
    #[serde(default)]
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_instance_name")]
    pub instance_name: String,
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default = "default_definitions_dir")]
    pub definitions_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database mode: "external" (default), "managed", or "embedded".
    #[serde(default = "default_db_mode")]
    pub mode: String,
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Where managed PostgreSQL stores its data. Defaults to `{data_dir}/postgres`.
    #[serde(default)]
    pub data_dir: Option<std::path::PathBuf>,
    /// Port for managed PostgreSQL (default 5433, avoids conflict with system PG).
    #[serde(default = "default_pg_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_auth_method")]
    pub method: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TorrentConfig {
    #[serde(default)]
    pub enabled: bool,
    pub download_dir: Option<PathBuf>,
    pub complete_dir: Option<PathBuf>,
    #[serde(default = "default_listen_port")]
    pub listen_port: u16,
    #[serde(default = "default_true")]
    pub dht_enabled: bool,
    #[serde(default = "default_peer_limit")]
    pub peer_limit: usize,
    #[serde(default)]
    pub upload_limit_bps: u64,
    #[serde(default)]
    pub download_limit_bps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsenetConfig {
    #[serde(default)]
    pub enabled: bool,
    pub incomplete_dir: Option<PathBuf>,
    pub complete_dir: Option<PathBuf>,
    #[serde(default = "default_max_active")]
    pub max_active_downloads: usize,
    /// Begin extracting RAR volumes during download instead of waiting for
    /// the job to complete. Requires `unrar` on PATH.
    #[serde(default = "default_true")]
    pub direct_unpack: bool,
    /// Maximum time in seconds to wait for a single NNTP article response
    /// before treating the connection as stalled and reconnecting.
    /// 0 = no timeout. Default: 30.
    #[serde(default = "default_article_timeout")]
    pub article_timeout_secs: u64,
    /// Maximum number of nested archive layers extracted during post-processing.
    #[serde(default = "default_max_nested_archive_depth")]
    pub max_nested_archive_depth: u8,
    #[serde(default)]
    pub servers: Vec<UsenetServerConfig>,
}

impl Default for UsenetConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            incomplete_dir: None,
            complete_dir: None,
            max_active_downloads: default_max_active(),
            direct_unpack: true,
            article_timeout_secs: default_article_timeout(),
            max_nested_archive_depth: default_max_nested_archive_depth(),
            servers: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsenetServerConfig {
    pub name: String,
    pub host: String,
    #[serde(default = "default_nntp_port")]
    pub port: u16,
    #[serde(default = "default_true")]
    pub ssl: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    #[serde(default = "default_connections")]
    pub connections: u16,
    #[serde(default)]
    pub priority: u8,
    /// Optional SOCKS5 proxy URL: socks5://[username:password@]host:port
    #[serde(default)]
    pub proxy_url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexarrConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_indexarr_url")]
    pub url: String,
    pub api_key: Option<String>,
    #[serde(default = "default_indexarr_mode")]
    pub mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingConfig {
    #[serde(default)]
    pub enabled: bool,
    pub transcode_dir: Option<PathBuf>,
    #[serde(default = "default_ffmpeg_path")]
    pub ffmpeg_path: String,
    #[serde(default = "default_ffprobe_path")]
    pub ffprobe_path: String,
    #[serde(default)]
    pub hwaccel: HwAccelConfig,
    #[serde(default = "default_segment_duration")]
    pub segment_duration_secs: u32,
    #[serde(default = "default_max_sessions")]
    pub max_concurrent_sessions: usize,
    #[serde(default = "default_quality_tiers")]
    pub quality_tiers: Vec<QualityTierConfig>,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            transcode_dir: None,
            ffmpeg_path: default_ffmpeg_path(),
            ffprobe_path: default_ffprobe_path(),
            hwaccel: HwAccelConfig::default(),
            segment_duration_secs: default_segment_duration(),
            max_concurrent_sessions: default_max_sessions(),
            quality_tiers: default_quality_tiers(),
        }
    }
}

/// A quality tier for adaptive streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityTierConfig {
    pub name: String,
    pub max_width: u32,
    pub max_height: u32,
    /// Video bitrate in bits per second.
    pub video_bitrate: u64,
    /// Audio bitrate in bits per second.
    pub audio_bitrate: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HwAccelConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_hwaccel_type")]
    pub accel_type: String,
    pub device: Option<String>,
}

impl Default for HwAccelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            accel_type: default_hwaccel_type(),
            device: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NamingConfig {
    #[serde(default)]
    pub series: SeriesNaming,
    #[serde(default)]
    pub movie: MovieNaming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeriesNaming {
    #[serde(default = "default_true")]
    pub rename: bool,
    #[serde(default = "default_series_standard_format")]
    pub standard: String,
    #[serde(default = "default_series_daily_format")]
    pub daily: String,
    #[serde(default = "default_series_anime_format")]
    pub anime: String,
    #[serde(default = "default_season_folder_format")]
    pub season_folder: String,
}

impl Default for SeriesNaming {
    fn default() -> Self {
        Self {
            rename: true,
            standard: default_series_standard_format(),
            daily: default_series_daily_format(),
            anime: default_series_anime_format(),
            season_folder: default_season_folder_format(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieNaming {
    #[serde(default = "default_true")]
    pub rename: bool,
    #[serde(default = "default_movie_format")]
    pub standard: String,
    #[serde(default = "default_movie_folder_format")]
    pub folder: String,
}

impl Default for MovieNaming {
    fn default() -> Self {
        Self {
            rename: true,
            standard: default_movie_format(),
            folder: default_movie_folder_format(),
        }
    }
}

/// Top-level storage settings — currently just archival of .torrent/.nzb files
/// so the user has a forensic record of every grab for debugging and manual re-add.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageConfig {
    #[serde(default)]
    pub archive: ArchiveConfig,
}

/// Count-based archive of .torrent and .nzb files.
///
/// All directories default to subdirs of `general.data_dir` when left empty.
/// Retention is count-based: after each cleanup pass, only the most recently
/// modified N files survive in each directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Directory for archived .torrent files. Empty → `{data_dir}/archive/Torrents`.
    #[serde(default)]
    pub torrent_dir: Option<PathBuf>,
    /// Directory for archived .nzb files (successful/in-flight grabs).
    /// Empty → `{data_dir}/archive/Usenet/NZBs`.
    #[serde(default)]
    pub nzb_dir: Option<PathBuf>,
    /// Directory for .nzb files of failed jobs (moved here when a usenet job
    /// reaches a terminal `Failed` state). Empty → `{data_dir}/archive/Usenet/NZBs/failed`.
    #[serde(default)]
    pub nzb_failed_dir: Option<PathBuf>,
    #[serde(default = "default_archive_count")]
    pub max_torrent_files: usize,
    #[serde(default = "default_archive_count")]
    pub max_nzb_files: usize,
    #[serde(default = "default_failed_archive_count")]
    pub max_failed_nzb_files: usize,
    #[serde(default = "default_archive_cleanup_hours")]
    pub cleanup_interval_hours: u64,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            torrent_dir: None,
            nzb_dir: None,
            nzb_failed_dir: None,
            max_torrent_files: default_archive_count(),
            max_nzb_files: default_archive_count(),
            max_failed_nzb_files: default_failed_archive_count(),
            cleanup_interval_hours: default_archive_cleanup_hours(),
        }
    }
}

impl ArchiveConfig {
    /// Resolve `torrent_dir` against `data_dir`, applying the default subpath
    /// when unset.
    pub fn resolved_torrent_dir(&self, data_dir: &Path) -> PathBuf {
        self.torrent_dir
            .clone()
            .unwrap_or_else(|| data_dir.join("archive").join("Torrents"))
    }

    pub fn resolved_nzb_dir(&self, data_dir: &Path) -> PathBuf {
        self.nzb_dir
            .clone()
            .unwrap_or_else(|| data_dir.join("archive").join("Usenet").join("NZBs"))
    }

    pub fn resolved_nzb_failed_dir(&self, data_dir: &Path) -> PathBuf {
        self.nzb_failed_dir.clone().unwrap_or_else(|| {
            data_dir
                .join("archive")
                .join("Usenet")
                .join("NZBs")
                .join("failed")
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BootstrapConfig {
    #[serde(default)]
    pub enabled: bool,
    /// URL of the bootstrap node (e.g., "https://bootstrap.example.com")
    pub url: Option<String>,
    /// Token to authenticate with the bootstrap node
    pub token: Option<String>,
    /// Port to advertise (defaults to general.port)
    pub advertise_port: Option<u16>,
    /// Whether to use UPnP to forward the advertise port
    #[serde(default)]
    pub upnp_enabled: bool,
    /// Port for the HTTPS listener serving wildcard cert from bootstrap.
    /// Default: 9443. Set to 0 to disable direct TLS.
    #[serde(default = "default_tls_port")]
    pub tls_port: u16,
}

fn default_tls_port() -> u16 {
    9443
}
fn default_archive_count() -> usize {
    500
}
fn default_failed_archive_count() -> usize {
    200
}
fn default_archive_cleanup_hours() -> u64 {
    6
}

/// Modules the user has chosen to enable (persisted in DB, set at first-boot).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnabledModules {
    pub tv_management: bool,
    pub movie_management: bool,
    pub torrent_embedded: bool,
    pub usenet_embedded: bool,
    pub torrent_external: bool,
    pub usenet_external: bool,
    pub indexarr_sidecar: bool,
    pub external_indexers: bool,
    pub plex_integration: bool,
    pub notifications: bool,
    pub streaming: bool,
    pub remote_access: bool,
    pub stremio_addon: bool,
    pub dav_streaming: bool,
}

// --- Default value functions ---

fn default_instance_name() -> String {
    "StackArr".to_string()
}
fn default_bind_addr() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    8989
}
fn default_data_dir() -> PathBuf {
    PathBuf::from("/config")
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_max_connections() -> u32 {
    20
}
fn default_db_mode() -> String {
    "external".to_string()
}
fn default_pg_port() -> u16 {
    5433
}
fn default_auth_method() -> String {
    "forms".to_string()
}
fn default_listen_port() -> u16 {
    6881
}
fn default_true() -> bool {
    true
}
fn default_peer_limit() -> usize {
    200
}
fn default_max_active() -> usize {
    1
}
fn default_nntp_port() -> u16 {
    563
}
fn default_connections() -> u16 {
    8
}
fn default_article_timeout() -> u64 {
    30
}
fn default_max_nested_archive_depth() -> u8 {
    5
}
fn default_indexarr_url() -> String {
    "http://indexarr:8080".to_string()
}
fn default_indexarr_mode() -> String {
    "peer".to_string()
}
fn default_ffmpeg_path() -> String {
    // Prefer jellyfin-ffmpeg (installed to /usr/lib/jellyfin-ffmpeg/)
    if std::path::Path::new("/usr/lib/jellyfin-ffmpeg/ffmpeg").exists() {
        "/usr/lib/jellyfin-ffmpeg/ffmpeg".to_string()
    } else {
        "ffmpeg".to_string()
    }
}
fn default_ffprobe_path() -> String {
    if std::path::Path::new("/usr/lib/jellyfin-ffmpeg/ffprobe").exists() {
        "/usr/lib/jellyfin-ffmpeg/ffprobe".to_string()
    } else {
        "ffprobe".to_string()
    }
}
fn default_segment_duration() -> u32 {
    6
}
fn default_max_sessions() -> usize {
    3
}
fn default_hwaccel_type() -> String {
    "vaapi".to_string()
}
fn default_quality_tiers() -> Vec<QualityTierConfig> {
    vec![
        QualityTierConfig {
            name: "4K".into(),
            max_width: 3840,
            max_height: 2160,
            video_bitrate: 40_000_000,
            audio_bitrate: 640_000,
        },
        QualityTierConfig {
            name: "4K Low".into(),
            max_width: 3840,
            max_height: 2160,
            video_bitrate: 20_000_000,
            audio_bitrate: 384_000,
        },
        QualityTierConfig {
            name: "1080p".into(),
            max_width: 1920,
            max_height: 1080,
            video_bitrate: 8_000_000,
            audio_bitrate: 192_000,
        },
        QualityTierConfig {
            name: "1080p Low".into(),
            max_width: 1920,
            max_height: 1080,
            video_bitrate: 4_000_000,
            audio_bitrate: 128_000,
        },
        QualityTierConfig {
            name: "720p".into(),
            max_width: 1280,
            max_height: 720,
            video_bitrate: 2_500_000,
            audio_bitrate: 128_000,
        },
        QualityTierConfig {
            name: "480p".into(),
            max_width: 854,
            max_height: 480,
            video_bitrate: 1_500_000,
            audio_bitrate: 96_000,
        },
    ]
}
fn default_definitions_dir() -> PathBuf {
    // In Docker, definitions are copied to /definitions
    let docker_path = PathBuf::from("/definitions");
    if docker_path.exists() {
        return docker_path;
    }
    // Dev fallback: relative to project root
    PathBuf::from("crates/stackarr-cardigann/definitions")
}
fn default_series_standard_format() -> String {
    "{Series Title} - S{season:00}E{episode:00} - {Episode Title} [{Quality Title}]".to_string()
}
fn default_series_daily_format() -> String {
    "{Series Title} - {Air-Date} - {Episode Title} [{Quality Title}]".to_string()
}
fn default_series_anime_format() -> String {
    "{Series Title} - S{season:00}E{episode:00} - {Absolute Episode} - {Episode Title} [{Quality Title}]".to_string()
}
fn default_season_folder_format() -> String {
    "Season {season:00}".to_string()
}
fn default_movie_format() -> String {
    "{Movie Title} ({Release Year}) [{Quality Title}]".to_string()
}
fn default_movie_folder_format() -> String {
    "{Movie Title} ({Release Year})".to_string()
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            instance_name: default_instance_name(),
            bind_addr: default_bind_addr(),
            port: default_port(),
            data_dir: default_data_dir(),
            log_level: default_log_level(),
            definitions_dir: default_definitions_dir(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            mode: default_db_mode(),
            url: "postgresql://stackarr:stackarr@localhost:5432/stackarr".to_string(),
            max_connections: default_max_connections(),
            data_dir: None,
            port: default_pg_port(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            method: default_auth_method(),
            api_key: None,
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> crate::Result<Self> {
        if !path.exists() {
            return Err(crate::Error::Config(format!(
                "config file not found: {} — use generate_default() to create one",
                path.display()
            )));
        }
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::Error::Config(format!(
                "failed to read config file {}: {e}",
                path.display()
            ))
        })?;
        toml::from_str(&content)
            .map_err(|e| crate::Error::Config(format!("failed to parse config: {e}")))
    }

    /// Validate config values and emit warnings for common misconfigurations.
    /// Returns an error only for values that will definitely break at runtime.
    pub fn validate(&self) -> crate::Result<()> {
        // Database URL is required for external mode
        if self.database.mode == "external" && self.database.url.is_empty() {
            return Err(crate::Error::Config(
                "database.url is required when database.mode = \"external\". \
                 Set it in your config file or pass --database-url."
                    .to_string(),
            ));
        }

        // Port sanity
        if self.general.port == 0 {
            return Err(crate::Error::Config("general.port cannot be 0".to_string()));
        }

        // Auth method must be recognized
        let valid_auth = ["forms", "none", "apikey"];
        if !valid_auth.contains(&self.auth.method.as_str()) {
            return Err(crate::Error::Config(format!(
                "auth.method '{}' is not valid. Must be one of: {}",
                self.auth.method,
                valid_auth.join(", ")
            )));
        }

        // Warn about common misconfigurations (non-fatal)
        if self.streaming.enabled && self.streaming.transcode_dir.is_none() {
            tracing::warn!(
                "streaming is enabled but streaming.transcode_dir is not set — \
                 transcoded segments will use a temp directory"
            );
        }

        if self.torrent.enabled && self.torrent.download_dir.is_none() {
            tracing::warn!(
                "torrent engine is enabled but torrent.download_dir is not set — \
                 downloads will use a temp directory"
            );
        }

        if self.usenet.enabled && self.usenet.incomplete_dir.is_none() {
            tracing::warn!(
                "usenet engine is enabled but usenet.incomplete_dir is not set — \
                 downloads will use a temp directory"
            );
        }

        Ok(())
    }

    pub fn generate_default(path: &Path) -> crate::Result<Self> {
        let config = Self::default();
        let content = toml::to_string_pretty(&config).map_err(|e| {
            crate::Error::Config(format!("failed to serialize default config: {e}"))
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_values() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.general.port, 8989);
        assert_eq!(cfg.general.bind_addr, "0.0.0.0");
        assert_eq!(cfg.general.log_level, "info");
        assert_eq!(cfg.general.instance_name, "StackArr");
        assert_eq!(cfg.general.data_dir, PathBuf::from("/config"));
        assert_eq!(cfg.database.max_connections, 20);
        assert_eq!(cfg.auth.method, "forms");
        assert!(cfg.auth.api_key.is_none());
        assert_eq!(cfg.usenet.max_active_downloads, 1);
        assert!(cfg.usenet.direct_unpack);
        assert_eq!(cfg.usenet.article_timeout_secs, 30);
        assert_eq!(cfg.usenet.max_nested_archive_depth, 5);
    }

    #[test]
    fn test_config_roundtrip_toml() {
        let original = AppConfig::default();
        let toml_str = toml::to_string_pretty(&original).expect("serialize");
        let parsed: AppConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(parsed.general.port, original.general.port);
        assert_eq!(parsed.general.bind_addr, original.general.bind_addr);
        assert_eq!(parsed.database.url, original.database.url);
        assert_eq!(parsed.torrent.listen_port, original.torrent.listen_port);
        assert_eq!(
            parsed.usenet.max_active_downloads,
            original.usenet.max_active_downloads
        );
    }

    #[test]
    fn test_config_load_missing_file() {
        let result = AppConfig::load(Path::new("/nonexistent/path/config.toml"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("config file not found"));
    }

    #[test]
    fn test_config_partial_toml_uses_defaults() {
        let toml_str = r#"
[general]
port = 9090

[database]
url = "postgresql://test:test@localhost:5432/test"

[auth]
method = "none"
"#;
        let cfg: AppConfig = toml::from_str(toml_str).expect("parse partial config");
        assert_eq!(cfg.general.port, 9090);
        // Fields not in the TOML should get defaults
        assert_eq!(cfg.general.bind_addr, "0.0.0.0");
        assert_eq!(cfg.general.instance_name, "StackArr");
        assert!(!cfg.torrent.enabled);
        assert!(!cfg.usenet.enabled);
    }

    #[test]
    fn test_enabled_modules_default_all_false() {
        let modules = EnabledModules::default();
        assert!(!modules.tv_management);
        assert!(!modules.movie_management);
        assert!(!modules.torrent_embedded);
        assert!(!modules.usenet_embedded);
        assert!(!modules.torrent_external);
        assert!(!modules.usenet_external);
        assert!(!modules.indexarr_sidecar);
        assert!(!modules.external_indexers);
        assert!(!modules.plex_integration);
        assert!(!modules.notifications);
        assert!(!modules.streaming);
        assert!(!modules.remote_access);
    }
}
