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
    /// Priority of the source indexer (lower = higher priority). Set post-search by IndexerManager.
    #[serde(default = "default_indexer_priority")]
    pub indexer_priority: i32,
}

fn default_indexer_priority() -> i32 {
    25
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
            self.base_url,
            urlencoding::encode(action),
            urlencoding::encode(&self.api_key),
        );
        for (k, v) in extra {
            url.push_str(&format!(
                "&{}={}",
                urlencoding::encode(k),
                urlencoding::encode(v),
            ));
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
    let mut enclosure_url: Option<String> = None;
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
                    enclosure_url = None;
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
                if tag == "enclosure" {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        if key == "url" {
                            enclosure_url = Some(String::from_utf8_lossy(&attr.value).to_string());
                        }
                    }
                } else if tag.ends_with("attr") {
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
                // Use unescape() to decode XML entities (&amp; → &) and append
                // because quick-xml may split text around entity references into
                // multiple Text events.
                let raw = e.xml_content().unwrap_or_default().to_string();
                let text = quick_xml::escape::unescape(&raw)
                    .unwrap_or(std::borrow::Cow::Owned(raw.clone()))
                    .to_string();
                match current_tag.as_str() {
                    "title" => title.push_str(&text),
                    "guid" => guid.push_str(&text),
                    "link" => link.push_str(&text),
                    "pubDate" => pub_date_str.push_str(&text),
                    "comments" => info_url.push_str(&text),
                    "size" => size = text.parse().unwrap_or(size),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if name == "item" && in_item {
                    let publish_date = parse_rfc2822_lenient(&pub_date_str);
                    let age_days = (Utc::now() - publish_date).num_days().max(0);

                    // Prefer enclosure URL (always a complete URL) over link
                    // (which may be an info page or split by XML entity parsing).
                    let download = enclosure_url.clone().or_else(|| {
                        if link.is_empty() {
                            None
                        } else {
                            Some(link.clone())
                        }
                    });

                    let nzb_url = if protocol == Protocol::Usenet {
                        download.clone()
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
                        download_url: download,
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
                        indexer_priority: 25,
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
    let trimmed = xml.trim_start();
    if trimmed.starts_with("<!DOCTYPE html")
        || trimmed.starts_with("<html")
        || trimmed.starts_with("<HTML")
    {
        bail!(
            "indexer returned an HTML page instead of XML — check that the URL points to a valid Newznab/Torznab API endpoint"
        );
    }

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
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "available" && val == "yes" {
                                caps.search_available = true;
                            }
                        }
                    }
                    "tv-search" => {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val = String::from_utf8_lossy(&attr.value).to_string();
                            if key == "available" && val == "yes" {
                                caps.tv_search_available = true;
                            }
                        }
                    }
                    "movie-search" => {
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
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
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
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

/// Generate candidate base URLs to probe, in priority order.
/// First the URL as-given, then with common API path suffixes stripped.
pub fn candidate_base_urls(url: &str) -> Vec<String> {
    let base = url.trim_end_matches('/').to_string();
    let mut candidates = vec![base.clone()];
    for suffix in &["/api/v1", "/api/v2", "/api"] {
        if base.to_lowercase().ends_with(suffix) {
            candidates.push(base[..base.len() - suffix.len()].to_string());
            break;
        }
    }
    candidates
}

/// Best-effort RFC 2822 date parsing with fallback to RFC 3339.
fn parse_rfc2822_lenient(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc2822(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)))
        .or_else(|_| s.parse::<DateTime<Utc>>())
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_newznab_xml ───────────────────────────────────────────────

    #[test]
    fn test_parse_newznab_xml_single_item() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:newznab="http://www.newznab.com/DTD/2010/feeds/attributes/">
<channel>
  <item>
    <title>Test.Release.S01E01.720p.HDTV-GROUP</title>
    <guid>abc123</guid>
    <link>http://example.com/download/abc123</link>
    <pubDate>Sat, 01 Jan 2025 12:00:00 +0000</pubDate>
  </item>
</channel>
</rss>"#;
        let results = parse_newznab_xml(xml, 1, "TestIndexer", Protocol::Usenet).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test.Release.S01E01.720p.HDTV-GROUP");
        assert_eq!(results[0].guid, "abc123");
        assert_eq!(results[0].indexer_id, 1);
        assert_eq!(results[0].indexer_name, "TestIndexer");
        assert_eq!(results[0].protocol, Protocol::Usenet);
        assert!(results[0].nzb_url.is_some());
    }

    #[test]
    fn test_parse_newznab_xml_with_newznab_attrs() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:newznab="http://www.newznab.com/DTD/2010/feeds/attributes/">
<channel>
  <item>
    <title>Big.Movie.2024.1080p</title>
    <guid>xyz789</guid>
    <link>http://example.com/nzb/xyz789</link>
    <pubDate>Mon, 15 Jan 2025 08:30:00 +0000</pubDate>
    <newznab:attr name="size" value="5368709120" />
    <newznab:attr name="category" value="2000" />
    <newznab:attr name="category" value="2040" />
    <newznab:attr name="tvdbid" value="12345" />
    <newznab:attr name="imdbid" value="tt9876543" />
    <newznab:attr name="tmdbid" value="67890" />
  </item>
</channel>
</rss>"#;
        let results = parse_newznab_xml(xml, 2, "NZBGeek", Protocol::Usenet).unwrap();
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert_eq!(r.size, 5_368_709_120);
        assert_eq!(r.categories, vec![2000, 2040]);
        assert_eq!(r.tvdb_id, Some(12345));
        assert_eq!(r.imdb_id.as_deref(), Some("tt9876543"));
        assert_eq!(r.tmdb_id, Some(67890));
    }

    #[test]
    fn test_parse_newznab_xml_torrent_attrs() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:torznab="http://torznab.com/schemas/2015/feed">
<channel>
  <item>
    <title>Linux.Distro.2024.x64</title>
    <guid>tor123</guid>
    <link>http://example.com/torrent/tor123</link>
    <pubDate>Tue, 20 Feb 2025 10:00:00 +0000</pubDate>
    <torznab:attr name="seeders" value="42" />
    <torznab:attr name="leechers" value="10" />
    <torznab:attr name="infohash" value="aabbccdd00112233445566778899aabbccddeeff" />
    <torznab:attr name="magneturl" value="magnet:?xt=urn:btih:aabb" />
    <torznab:attr name="size" value="1073741824" />
  </item>
</channel>
</rss>"#;
        let results = parse_newznab_xml(xml, 3, "Torznab", Protocol::Torrent).unwrap();
        assert_eq!(results.len(), 1);
        let r = &results[0];
        assert_eq!(r.seeders, Some(42));
        assert_eq!(r.leechers, Some(10));
        assert_eq!(
            r.info_hash.as_deref(),
            Some("aabbccdd00112233445566778899aabbccddeeff")
        );
        assert_eq!(r.magnet_url.as_deref(), Some("magnet:?xt=urn:btih:aabb"));
        assert_eq!(r.size, 1_073_741_824);
        assert!(r.nzb_url.is_none()); // torrent protocol, no NZB URL
    }

    #[test]
    fn test_parse_newznab_xml_empty_channel() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel></channel></rss>"#;
        let results = parse_newznab_xml(xml, 1, "Empty", Protocol::Usenet).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_newznab_xml_malformed() {
        let result = parse_newznab_xml("not xml at all <><><<<", 1, "Bad", Protocol::Usenet);
        // Should still succeed with 0 items or error - malformed XML may partially parse
        // The important thing is it doesn't panic
        match result {
            Ok(items) => assert!(items.is_empty()),
            Err(_) => {} // error is also acceptable
        }
    }

    #[test]
    fn test_parse_newznab_xml_multiple_items() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:newznab="http://www.newznab.com/DTD/2010/feeds/attributes/">
<channel>
  <item><title>Release A</title><guid>a</guid><link>http://a</link><pubDate>Sat, 01 Jan 2025 00:00:00 +0000</pubDate></item>
  <item><title>Release B</title><guid>b</guid><link>http://b</link><pubDate>Sat, 01 Jan 2025 00:00:00 +0000</pubDate></item>
  <item><title>Release C</title><guid>c</guid><link>http://c</link><pubDate>Sat, 01 Jan 2025 00:00:00 +0000</pubDate></item>
</channel>
</rss>"#;
        let results = parse_newznab_xml(xml, 1, "Multi", Protocol::Usenet).unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].title, "Release A");
        assert_eq!(results[1].title, "Release B");
        assert_eq!(results[2].title, "Release C");
    }

    // ── parse_caps ──────────────────────────────────────────────────────

    #[test]
    fn test_parse_caps_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<caps>
  <searching>
    <search available="yes" />
    <tv-search available="yes" />
    <movie-search available="no" />
  </searching>
  <categories>
    <category id="2000" name="Movies" />
    <category id="5000" name="TV" />
  </categories>
</caps>"#;
        let caps = parse_caps(xml).unwrap();
        assert!(caps.search_available);
        assert!(caps.tv_search_available);
        assert!(!caps.movie_search_available);
        assert_eq!(caps.categories.len(), 2);
        assert_eq!(caps.categories[0].id, 2000);
        assert_eq!(caps.categories[0].name, "Movies");
        assert_eq!(caps.categories[1].id, 5000);
        assert_eq!(caps.categories[1].name, "TV");
    }

    // ── parse_rfc2822_lenient ───────────────────────────────────────────

    #[test]
    fn test_parse_rfc2822_valid() {
        // Wed, 15 Jul 2026 is the correct day-of-week
        let dt = parse_rfc2822_lenient("Wed, 15 Jul 2026 12:00:00 +0000");
        assert_eq!(dt.year(), 2026);
        assert_eq!(dt.month(), 7);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_parse_rfc2822_rfc3339_fallback() {
        let dt = parse_rfc2822_lenient("2026-06-15T10:30:00+00:00");
        assert_eq!(dt.year(), 2026);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_parse_rfc2822_garbage_fallback() {
        let before = Utc::now();
        let dt = parse_rfc2822_lenient("not a date at all");
        let after = Utc::now();
        // Should fall back to approximately now
        assert!(dt >= before && dt <= after);
    }
}

// Needed for the year()/month()/day() calls in tests
#[cfg(test)]
use chrono::Datelike;
