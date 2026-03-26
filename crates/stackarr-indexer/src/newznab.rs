use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// A single release returned by a Newznab / Torznab indexer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub guid: String,
    pub title: String,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub indexer_id: i64,
    pub indexer_name: String,
    pub protocol: Protocol,
    pub size: i64,
    pub age_days: i64,
    pub publish_date: DateTime<Utc>,
    // Torrent-specific
    pub info_hash: Option<String>,
    pub magnet_url: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    // Usenet-specific
    pub nzb_url: Option<String>,
    // External IDs
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<i64>,
    // Categories
    pub categories: Vec<i32>,
    pub indexer_flags: Vec<String>,
}

/// Whether this indexer speaks Newznab (usenet) or Torznab (torrent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Protocol {
    Usenet,
    Torrent,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Usenet => write!(f, "usenet"),
            Protocol::Torrent => write!(f, "torrent"),
        }
    }
}

/// Capabilities reported by a Newznab / Torznab indexer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexerCaps {
    pub search_available: bool,
    pub tv_search_available: bool,
    pub movie_search_available: bool,
    pub categories: Vec<IndexerCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerCategory {
    pub id: i32,
    pub name: String,
}

/// Client for a standard Newznab (or Torznab) XML API.
pub struct NewznabClient {
    base_url: String,
    api_key: String,
    indexer_id: i64,
    indexer_name: String,
    protocol: Protocol,
    http: Client,
}

impl NewznabClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        indexer_id: i64,
        indexer_name: impl Into<String>,
        protocol: Protocol,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            indexer_id,
            indexer_name: indexer_name.into(),
            protocol,
            http: Client::new(),
        }
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    pub fn indexer_id(&self) -> i64 {
        self.indexer_id
    }

    pub fn indexer_name(&self) -> &str {
        &self.indexer_name
    }

    // ── URL building ────────────────────────────────────────────────────

    fn api_url(&self, action: &str, extra: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{}/api?t={}&apikey={}&o=xml",
            self.base_url, action, self.api_key
        );
        for (k, v) in extra {
            url.push_str(&format!("&{k}={v}"));
        }
        url
    }

    // ── Public search methods ───────────────────────────────────────────

    /// Free-text search, optionally filtered by category IDs.
    pub async fn search(
        &self,
        query: &str,
        categories: &[i32],
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let cats = categories
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let mut params: Vec<(&str, &str)> = vec![("q", query)];
        if !cats.is_empty() {
            params.push(("cat", &cats));
        }
        let url = self.api_url("search", &params);
        self.fetch_and_parse(&url).await
    }

    /// TV search by TVDB ID + season/episode.
    pub async fn tv_search(
        &self,
        tvdbid: i64,
        season: Option<i32>,
        episode: Option<i32>,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let tvdb_str = tvdbid.to_string();
        let season_str;
        let ep_str;
        let mut params: Vec<(&str, &str)> = vec![("tvdbid", &tvdb_str)];
        if let Some(s) = season {
            season_str = s.to_string();
            params.push(("season", &season_str));
        }
        if let Some(e) = episode {
            ep_str = e.to_string();
            params.push(("ep", &ep_str));
        }
        let url = self.api_url("tvsearch", &params);
        self.fetch_and_parse(&url).await
    }

    /// Movie search by IMDB or TMDB ID.
    pub async fn movie_search(
        &self,
        imdbid: Option<&str>,
        tmdbid: Option<i64>,
    ) -> anyhow::Result<Vec<ReleaseInfo>> {
        let tmdb_str;
        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(id) = imdbid {
            params.push(("imdbid", id));
        }
        if let Some(id) = tmdbid {
            tmdb_str = id.to_string();
            params.push(("tmdbid", &tmdb_str));
        }
        let url = self.api_url("movie", &params);
        self.fetch_and_parse(&url).await
    }

    /// Fetch the indexer's capabilities (caps).
    pub async fn caps(&self) -> anyhow::Result<IndexerCaps> {
        let url = self.api_url("caps", &[]);
        let body = self
            .http
            .get(&url)
            .send()
            .await
            .context("caps request failed")?
            .text()
            .await
            .context("caps response body")?;

        parse_caps(&body)
    }

    // ── Internal helpers ────────────────────────────────────────────────

    async fn fetch_and_parse(&self, url: &str) -> anyhow::Result<Vec<ReleaseInfo>> {
        debug!(indexer = %self.indexer_name, url = %url, "fetching releases");
        let body = self
            .http
            .get(url)
            .send()
            .await
            .context("indexer request failed")?
            .text()
            .await
            .context("indexer response body")?;

        parse_newznab_xml(&body, self.indexer_id, &self.indexer_name, self.protocol)
    }
}

// ── XML parsing ─────────────────────────────────────────────────────────────

/// Parse a Newznab RSS-style XML response into a vec of [`ReleaseInfo`].
///
/// This is the public entry point used by sibling modules (e.g. `indexarr`).
pub fn parse_newznab_xml_public(
    xml: &str,
    indexer_id: i64,
    indexer_name: &str,
    protocol: Protocol,
) -> anyhow::Result<Vec<ReleaseInfo>> {
    parse_newznab_xml(xml, indexer_id, indexer_name, protocol)
}

fn parse_newznab_xml(
    xml: &str,
    indexer_id: i64,
    indexer_name: &str,
    protocol: Protocol,
) -> anyhow::Result<Vec<ReleaseInfo>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut releases = Vec::new();
    let mut in_item = false;
    let mut current_tag = String::new();

    // Per-item fields
    let mut title = String::new();
    let mut guid = String::new();
    let mut link = String::new();
    let mut size: i64 = 0;
    let mut pub_date_str = String::new();
    let mut info_url = String::new();
    let mut info_hash: Option<String> = None;
    let mut magnet_url: Option<String> = None;
    let mut seeders: Option<i32> = None;
    let mut leechers: Option<i32> = None;
    let mut tvdb_id: Option<i64> = None;
    let mut imdb_id: Option<String> = None;
    let mut tmdb_id: Option<i64> = None;
    let mut categories: Vec<i32> = Vec::new();
    let mut indexer_flags: Vec<String> = Vec::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "item" {
                    in_item = true;
                    title.clear();
                    guid.clear();
                    link.clear();
                    size = 0;
                    pub_date_str.clear();
                    info_url.clear();
                    info_hash = None;
                    magnet_url = None;
                    seeders = None;
                    leechers = None;
                    tvdb_id = None;
                    imdb_id = None;
                    tmdb_id = None;
                    categories.clear();
                    indexer_flags.clear();
                }
                if in_item {
                    current_tag = name;
                }
            }
            Ok(Event::Empty(ref e)) if in_item => {
                // Newznab extended attributes: <newznab:attr name="..." value="..." />
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag.ends_with("attr") {
                    let mut attr_name = String::new();
                    let mut attr_value = String::new();
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        match key.as_str() {
                            "name" => attr_name = val,
                            "value" => attr_value = val,
                            _ => {}
                        }
                    }
                    match attr_name.as_str() {
                        "size" => size = attr_value.parse().unwrap_or(size),
                        "tvdbid" => tvdb_id = attr_value.parse().ok(),
                        "imdb" | "imdbid" => imdb_id = Some(attr_value),
                        "tmdbid" => tmdb_id = attr_value.parse().ok(),
                        "seeders" => {
                            seeders = attr_value.parse().ok();
                        }
                        "leechers" | "peers" => {
                            leechers = attr_value.parse().ok();
                        }
                        "infohash" => info_hash = Some(attr_value),
                        "magneturl" => magnet_url = Some(attr_value),
                        "category" => {
                            if let Ok(cat) = attr_value.parse::<i32>() {
                                categories.push(cat);
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(ref e)) if in_item => {
                let text = e.unescape().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "title" => title = text,
                    "guid" => guid = text,
                    "link" => link = text,
                    "pubDate" => pub_date_str = text,
                    "comments" => info_url = text,
                    "size" => size = text.parse().unwrap_or(size),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "item" && in_item {
                    let publish_date = parse_rfc2822_lenient(&pub_date_str);
                    let age_days = (Utc::now() - publish_date).num_days().max(0);

                    let nzb_url = if protocol == Protocol::Usenet {
                        Some(link.clone())
                    } else {
                        None
                    };

                    releases.push(ReleaseInfo {
                        guid: if guid.is_empty() {
                            link.clone()
                        } else {
                            guid.clone()
                        },
                        title: title.clone(),
                        download_url: if link.is_empty() {
                            None
                        } else {
                            Some(link.clone())
                        },
                        info_url: if info_url.is_empty() {
                            None
                        } else {
                            Some(info_url.clone())
                        },
                        indexer_id,
                        indexer_name: indexer_name.to_string(),
                        protocol,
                        size,
                        age_days,
                        publish_date,
                        info_hash: info_hash.clone(),
                        magnet_url: magnet_url.clone(),
                        seeders,
                        leechers,
                        nzb_url,
                        tvdb_id,
                        imdb_id: imdb_id.clone(),
                        tmdb_id,
                        categories: categories.clone(),
                        indexer_flags: indexer_flags.clone(),
                    });

                    in_item = false;
                }
                if in_item {
                    current_tag.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(releases)
}

/// Parse capabilities XML.
fn parse_caps(xml: &str) -> anyhow::Result<IndexerCaps> {
    let mut caps = IndexerCaps::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "searching" | "search" => {
                        for attr in e.attributes().flatten() {
                            let key =
                                String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "available" && val == "yes" {
                                caps.search_available = true;
                            }
                        }
                    }
                    "tv-search" => {
                        for attr in e.attributes().flatten() {
                            let key =
                                String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "available" && val == "yes" {
                                caps.tv_search_available = true;
                            }
                        }
                    }
                    "movie-search" => {
                        for attr in e.attributes().flatten() {
                            let key =
                                String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "available" && val == "yes" {
                                caps.movie_search_available = true;
                            }
                        }
                    }
                    "category" => {
                        let mut cat_id = 0i32;
                        let mut cat_name = String::new();
                        for attr in e.attributes().flatten() {
                            let key =
                                String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "id" => cat_id = val.parse().unwrap_or(0),
                                "name" => cat_name = val,
                                _ => {}
                            }
                        }
                        if cat_id > 0 {
                            caps.categories.push(IndexerCategory {
                                id: cat_id,
                                name: cat_name,
                            });
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("caps XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(caps)
}

/// Best-effort RFC 2822 date parsing with fallback to RFC 3339.
fn parse_rfc2822_lenient(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc2822(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
        .or_else(|_| s.parse::<DateTime<Utc>>())
        .unwrap_or_else(|_| Utc::now())
}
