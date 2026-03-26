use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UsenetConfig {
    #[serde(default)]
    pub enabled: bool,
    pub incomplete_dir: Option<PathBuf>,
    pub complete_dir: Option<PathBuf>,
    #[serde(default = "default_max_active")]
    pub max_active_downloads: usize,
    #[serde(default)]
    pub servers: Vec<UsenetServerConfig>,
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
pub struct NamingConfig {
    #[serde(default)]
    pub series: SeriesNaming,
    #[serde(default)]
    pub movie: MovieNaming,
}

impl Default for NamingConfig {
    fn default() -> Self {
        Self {
            series: SeriesNaming::default(),
            movie: MovieNaming::default(),
        }
    }
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
    pub notifications: bool,
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
    3
}
fn default_nntp_port() -> u16 {
    563
}
fn default_connections() -> u16 {
    8
}
fn default_indexarr_url() -> String {
    "http://indexarr:8080".to_string()
}
fn default_indexarr_mode() -> String {
    "peer".to_string()
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

impl AppConfig {
    pub fn load(path: &std::path::Path) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            crate::Error::Config(format!("failed to read config file {}: {e}", path.display()))
        })?;
        toml::from_str(&content)
            .map_err(|e| crate::Error::Config(format!("failed to parse config: {e}")))
    }
}
