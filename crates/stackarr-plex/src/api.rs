use anyhow::{Context, Result};
use reqwest::header::{HeaderMap, HeaderValue};

use crate::types::*;

// ── Plex API (direct server) ───────────────────────────────────────────────

/// Client for communicating directly with a Plex Media Server.
pub struct PlexApi {
    client: reqwest::Client,
    base_url: String,
    token: String,
}

impl PlexApi {
    pub fn new(ip: &str, port: i32, use_ssl: bool, token: &str) -> Self {
        Self::with_tls_verify(ip, port, use_ssl, token, true)
    }

    /// Create a Plex API client with configurable TLS certificate verification.
    /// When `verify_tls` is false, self-signed certificates are accepted.
    pub fn with_tls_verify(
        ip: &str,
        port: i32,
        use_ssl: bool,
        token: &str,
        verify_tls: bool,
    ) -> Self {
        let scheme = if use_ssl { "https" } else { "http" };
        Self {
            client: reqwest::Client::builder()
                .danger_accept_invalid_certs(!verify_tls)
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url: format!("{scheme}://{ip}:{port}"),
            token: token.to_string(),
        }
    }

    pub fn from_server(server: &PlexServer) -> Option<Self> {
        let token = server.auth_token.as_ref()?;
        Some(Self::with_tls_verify(
            &server.ip,
            server.port,
            server.use_ssl,
            token,
            server.verify_tls,
        ))
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("X-Plex-Token", HeaderValue::from_str(&self.token).unwrap());
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert("X-Plex-Product", HeaderValue::from_static("StackArr"));
        headers.insert("X-Plex-Device-Name", HeaderValue::from_static("StackArr"));
        headers
    }

    /// Get server info (machine ID, friendly name, version).
    pub async fn get_status(&self) -> Result<PlexServerInfo> {
        let resp: PlexMediaContainer<PlexServerInfo> = self
            .client
            .get(&self.base_url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.media_container)
    }

    /// List all library sections on the server.
    pub async fn get_libraries(&self) -> Result<Vec<PlexLibrarySection>> {
        let url = format!("{}/library/sections", self.base_url);
        let resp: PlexMediaContainer<PlexLibrariesContainer> = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.media_container.directory)
    }

    /// Get all items in a library section (paginated).
    pub async fn get_library_contents(
        &self,
        section_id: &str,
        start: i64,
        size: i64,
    ) -> Result<PlexItemsContainer> {
        let url = format!(
            "{}/library/sections/{section_id}/all?X-Plex-Container-Start={start}&X-Plex-Container-Size={size}",
            self.base_url
        );
        let resp: PlexMediaContainer<PlexItemsContainer> = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.media_container)
    }

    /// Get metadata for a single item by rating key.
    pub async fn get_metadata(&self, rating_key: &str) -> Result<PlexMetadataItem> {
        let url = format!("{}/library/metadata/{rating_key}", self.base_url);
        let resp: PlexMediaContainer<PlexItemsContainer> = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        resp.media_container
            .metadata
            .into_iter()
            .next()
            .context("no metadata returned for rating key")
    }

    /// Get children (seasons for a show, episodes for a season).
    pub async fn get_children(&self, rating_key: &str) -> Result<Vec<PlexMetadataItem>> {
        let url = format!("{}/library/metadata/{rating_key}/children", self.base_url);
        let resp: PlexMediaContainer<PlexItemsContainer> = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.media_container.metadata)
    }

    /// Get recently added items in a library section, sorted by addedAt descending.
    pub async fn get_recently_added(
        &self,
        section_id: &str,
        start: i64,
        size: i64,
    ) -> Result<PlexItemsContainer> {
        let url = format!(
            "{}/library/sections/{section_id}/all?sort=addedAt:desc&X-Plex-Container-Start={start}&X-Plex-Container-Size={size}",
            self.base_url
        );
        let resp: PlexMediaContainer<PlexItemsContainer> = self
            .client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.media_container)
    }
}

// ── PlexTV API (plex.tv) ───────────────────────────────────────────────────

/// Client for the plex.tv cloud API (authentication, watchlist, device discovery).
pub struct PlexTvApi {
    client: reqwest::Client,
    token: String,
}

impl PlexTvApi {
    pub fn new(token: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_default(),
            token: token.to_string(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("X-Plex-Token", HeaderValue::from_str(&self.token).unwrap());
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert("X-Plex-Product", HeaderValue::from_static("StackArr"));
        headers.insert(
            "X-Plex-Client-Identifier",
            HeaderValue::from_static("stackarr-server"),
        );
        headers
    }

    /// Validate the token is still active (ping).
    pub async fn ping_token(&self) -> Result<bool> {
        let resp = self
            .client
            .get("https://plex.tv/api/v2/ping")
            .headers(self.headers())
            .send()
            .await?;
        Ok(resp.status().is_success())
    }

    /// Get the authenticated user's account info.
    pub async fn get_user(&self) -> Result<PlexTvUser> {
        let resp: PlexTvUserContainer = self
            .client
            .get("https://plex.tv/users/account.json")
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(resp.user)
    }

    /// Discover Plex servers the user owns/has access to.
    pub async fn get_servers(&self) -> Result<Vec<PlexResource>> {
        let resp = self
            .client
            .get("https://plex.tv/api/v2/resources?includeHttps=1")
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?;

        let resources: Vec<PlexResource> = resp.json().await?;
        // Filter to only "server" providers
        Ok(resources
            .into_iter()
            .filter(|r| r.provides.contains("server"))
            .collect())
    }

    /// Get the user's Plex watchlist.
    pub async fn get_watchlist(&self) -> Result<Vec<PlexWatchlistItem>> {
        let url = "https://discover.provider.plex.tv/library/sections/watchlist/all";
        let resp = self
            .client
            .get(url)
            .headers(self.headers())
            .send()
            .await?;

        if !resp.status().is_success() {
            // 404 or empty watchlist
            return Ok(Vec::new());
        }

        let container: PlexMediaContainer<PlexWatchlistContainer> = resp.json().await?;
        Ok(container.media_container.metadata)
    }

    /// Create a PIN for the OAuth flow.
    pub async fn create_pin(&self, client_id: &str) -> Result<PlexPinResponse> {
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert("X-Plex-Product", HeaderValue::from_static("StackArr"));
        headers.insert(
            "X-Plex-Client-Identifier",
            HeaderValue::from_str(client_id).unwrap(),
        );

        let pin: PlexPinResponse = self
            .client
            .post("https://plex.tv/api/v2/pins?strong=true")
            .headers(headers)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pin)
    }

    /// Check if a PIN has been authorized (poll during OAuth flow).
    pub async fn check_pin(&self, pin_id: i64, client_id: &str) -> Result<PlexPinResponse> {
        let mut headers = HeaderMap::new();
        headers.insert("Accept", HeaderValue::from_static("application/json"));
        headers.insert(
            "X-Plex-Client-Identifier",
            HeaderValue::from_str(client_id).unwrap(),
        );

        let pin: PlexPinResponse = self
            .client
            .get(format!("https://plex.tv/api/v2/pins/{pin_id}"))
            .headers(headers)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(pin)
    }
}
