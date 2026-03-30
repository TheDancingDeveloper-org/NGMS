//! NNTP server and article configuration types.

use serde::{Deserialize, Serialize};

/// NNTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Unique server identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Server hostname
    pub host: String,
    /// Server port
    pub port: u16,
    /// Use SSL/TLS
    pub ssl: bool,
    /// Verify SSL certificates
    pub ssl_verify: bool,
    /// Username for authentication
    pub username: Option<String>,
    /// Password for authentication
    pub password: Option<String>,
    /// Max simultaneous connections
    pub connections: u16,
    /// Server priority (0 = highest)
    pub priority: u8,
    /// Enable this server
    pub enabled: bool,
    /// Article retention in days (0 = unlimited)
    pub retention: u32,
    /// Number of pipelined requests per connection
    pub pipelining: u8,
    /// Server is optional (failure is non-fatal)
    pub optional: bool,
    /// Enable XFEATURE COMPRESS GZIP negotiation
    #[serde(default)]
    pub compress: bool,
    /// Delay in milliseconds between opening new connections (0 = no delay).
    /// Prevents connection bursts that trigger server-side rate limiting.
    #[serde(default)]
    pub ramp_up_delay_ms: u32,
    /// Optional SOCKS5 proxy URL: `socks5://[username:password@]host:port`
    #[serde(default)]
    pub proxy_url: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: String::new(),
            host: String::new(),
            port: 563,
            ssl: true,
            ssl_verify: true,
            username: None,
            password: None,
            connections: 8,
            priority: 0,
            enabled: true,
            retention: 0,
            pipelining: 1,
            optional: false,
            compress: false,
            ramp_up_delay_ms: 250,
            proxy_url: None,
        }
    }
}

/// A single NNTP article (segment of a file).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    /// Message-ID (e.g., "abc123@example.com")
    pub message_id: String,
    /// Segment number (1-based part number)
    pub segment_number: u32,
    /// Encoded size in bytes
    pub bytes: u64,
    /// Has this article been downloaded?
    pub downloaded: bool,
    /// Byte offset in the final file (set after yEnc decode)
    pub data_begin: Option<u64>,
    /// Size of decoded data for this segment
    pub data_size: Option<u64>,
    /// CRC32 of decoded data
    pub crc32: Option<u32>,
    /// Servers that have been tried for this article
    pub tried_servers: Vec<String>,
    /// Number of fetch attempts
    pub tries: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_config_defaults() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.port, 563);
        assert!(cfg.ssl);
        assert!(cfg.ssl_verify);
        assert!(cfg.username.is_none());
        assert!(cfg.password.is_none());
        assert_eq!(cfg.connections, 8);
        assert_eq!(cfg.priority, 0);
        assert!(cfg.enabled);
        assert_eq!(cfg.retention, 0);
        assert_eq!(cfg.pipelining, 1);
        assert!(!cfg.optional);
        // ID should be a valid UUID
        assert!(uuid::Uuid::parse_str(&cfg.id).is_ok());
    }

    #[test]
    fn test_server_config_toml_roundtrip() {
        let original = ServerConfig {
            id: "srv-1".into(),
            name: "Usenet Provider".into(),
            host: "news.example.com".into(),
            port: 563,
            ssl: true,
            ssl_verify: true,
            username: Some("user".into()),
            password: Some("pass".into()),
            connections: 20,
            priority: 0,
            enabled: true,
            retention: 3000,
            pipelining: 5,
            optional: false,
            compress: false,
            ramp_up_delay_ms: 0,
            proxy_url: None,
        };

        let toml_str = toml::to_string_pretty(&original).unwrap();
        let restored: ServerConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(restored.id, original.id);
        assert_eq!(restored.name, original.name);
        assert_eq!(restored.host, original.host);
        assert_eq!(restored.port, original.port);
        assert_eq!(restored.ssl, original.ssl);
        assert_eq!(restored.username, original.username);
        assert_eq!(restored.password, original.password);
        assert_eq!(restored.connections, original.connections);
        assert_eq!(restored.priority, original.priority);
        assert_eq!(restored.retention, original.retention);
        assert_eq!(restored.pipelining, original.pipelining);
        assert_eq!(restored.optional, original.optional);
    }

    #[test]
    fn test_article_serde_roundtrip() {
        let article = Article {
            message_id: "abc123@example.com".to_string(),
            segment_number: 1,
            bytes: 500_000,
            downloaded: false,
            data_begin: Some(0),
            data_size: Some(499_000),
            crc32: Some(0xDEADBEEF),
            tried_servers: vec!["server1".to_string()],
            tries: 2,
        };

        let json = serde_json::to_string(&article).unwrap();
        let deserialized: Article = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.message_id, "abc123@example.com");
        assert_eq!(deserialized.segment_number, 1);
        assert_eq!(deserialized.bytes, 500_000);
        assert!(!deserialized.downloaded);
        assert_eq!(deserialized.data_begin, Some(0));
        assert_eq!(deserialized.data_size, Some(499_000));
        assert_eq!(deserialized.crc32, Some(0xDEADBEEF));
        assert_eq!(deserialized.tried_servers, vec!["server1"]);
        assert_eq!(deserialized.tries, 2);
    }
}
