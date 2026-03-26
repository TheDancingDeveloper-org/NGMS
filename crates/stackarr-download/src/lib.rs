pub mod client;
pub mod manager;
pub mod nzbget;
pub mod qbittorrent;
pub mod sabnzbd;
pub mod transmission;

#[cfg(feature = "torrent-embedded")]
pub mod embedded_torrent;
#[cfg(feature = "usenet-embedded")]
pub mod embedded_usenet;

pub use client::*;
pub use manager::DownloadClientManager;
