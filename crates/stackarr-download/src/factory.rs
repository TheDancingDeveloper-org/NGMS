// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 The StackArr Authors

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_qbittorrent() {
        let config = json!({
            "host": "http://localhost:8080",
            "username": "admin",
            "password": "adminadmin"
        });
        let client = build_from_config("qbittorrent", &config).unwrap();
        assert_eq!(client.name(), "qBittorrent");
        assert_eq!(client.protocol(), crate::client::DownloadProtocol::Torrent);
    }

    #[test]
    fn test_build_qbittorrent_case_insensitive() {
        let config = json!({});
        let client = build_from_config("QBittorrent", &config).unwrap();
        assert_eq!(client.name(), "qBittorrent");
    }

    #[test]
    fn test_build_transmission() {
        let config = json!({
            "host": "http://localhost:9091",
            "username": "admin",
            "password": "pass"
        });
        let client = build_from_config("transmission", &config).unwrap();
        assert_eq!(client.name(), "Transmission");
        assert_eq!(client.protocol(), crate::client::DownloadProtocol::Torrent);
    }

    #[test]
    fn test_build_sabnzbd() {
        let config = json!({
            "host": "http://localhost:8080",
            "apiKey": "test-key"
        });
        let client = build_from_config("sabnzbd", &config).unwrap();
        assert_eq!(client.name(), "SABnzbd");
        assert_eq!(client.protocol(), crate::client::DownloadProtocol::Usenet);
    }

    #[test]
    fn test_build_nzbget() {
        let config = json!({
            "host": "http://localhost:6789",
            "username": "nzbget",
            "password": "tegbzn"
        });
        let client = build_from_config("nzbget", &config).unwrap();
        assert_eq!(client.name(), "NZBGet");
        assert_eq!(client.protocol(), crate::client::DownloadProtocol::Usenet);
    }

    #[test]
    fn test_build_unknown_type() {
        let config = json!({});
        let result = build_from_config("deluge", &config);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("unknown download client type"));
    }

    #[test]
    fn test_build_embedded_usenet_not_allowed() {
        let config = json!({});
        let result = build_from_config("embedded_usenet", &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_with_defaults() {
        // Empty config should still work - uses defaults
        let config = json!({});
        let client = build_from_config("qbittorrent", &config).unwrap();
        assert_eq!(client.name(), "qBittorrent");
    }

    #[test]
    fn test_build_transmission_optional_auth() {
        // Transmission supports no auth
        let config = json!({"host": "http://localhost:9091"});
        let client = build_from_config("transmission", &config).unwrap();
        assert_eq!(client.name(), "Transmission");
    }
}
