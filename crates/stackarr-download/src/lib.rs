pub mod client;
pub mod embedded_torrent;
pub mod embedded_usenet;
pub mod manager;
pub mod nzbget;
pub mod qbittorrent;
pub mod rtbit;
pub mod sabnzbd;
pub mod transmission;

pub use client::*;
pub use manager::DownloadClientManager;
