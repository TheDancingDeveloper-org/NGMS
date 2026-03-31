use crate::client::DownloadClient;

/// Build a download client instance from a `client_type` string and JSON config.
///
/// This is used both at startup (loading from DB) and by the health checker
/// when re-enabling auto-disabled clients.
pub fn build_from_config(
    client_type: &str,
    config: &serde_json::Value,
) -> anyhow::Result<Box<dyn DownloadClient>> {
    match client_type.to_ascii_lowercase().as_str() {
        "qbittorrent" => {
            let host = config["host"].as_str().unwrap_or("http://localhost:8080");
            let username = config["username"].as_str().unwrap_or("");
            let password = config["password"].as_str().unwrap_or("");
            Ok(Box::new(crate::qbittorrent::QBittorrentClient::new(
                host, username, password,
            )))
        }
        "transmission" => {
            let host = config["host"].as_str().unwrap_or("http://localhost:9091");
            let username = config["username"].as_str().map(String::from);
            let password = config["password"].as_str().map(String::from);
            Ok(Box::new(crate::transmission::TransmissionClient::new(
                host, username, password,
            )))
        }
        "sabnzbd" => {
            let host = config["host"].as_str().unwrap_or("http://localhost:8080");
            let api_key = config["apiKey"].as_str().unwrap_or("");
            Ok(Box::new(crate::sabnzbd::SabnzbdClient::new(host, api_key)))
        }
        "nzbget" => {
            let host = config["host"].as_str().unwrap_or("http://localhost:6789");
            let username = config["username"].as_str().unwrap_or("");
            let password = config["password"].as_str().unwrap_or("");
            Ok(Box::new(crate::nzbget::NzbgetClient::new(
                host, username, password,
            )))
        }
        "embedded_usenet" => {
            // Embedded usenet servers are managed by the nzb engine, not the download client manager
            anyhow::bail!("embedded_usenet is managed by the usenet engine, not download clients")
        }
        other => anyhow::bail!("unknown download client type: {other}"),
    }
}
