pub mod client;
pub mod embedded_torrent;
pub mod embedded_usenet;
pub mod factory;
pub mod manager;
pub mod nzbget;
pub mod qbittorrent;
pub mod sabnzbd;
pub mod transmission;

pub use client::*;
pub use factory::build_from_config;
pub use manager::DownloadClientManager;
