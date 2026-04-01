//! Search execution: load definition → build URL → fetch → parse → return releases.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::categories::CategoryMapper;

/// A release found by a Cardigann indexer search.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardigannRelease {
    pub guid: String,
    pub title: String,
    pub download_url: Option<String>,
    pub info_url: Option<String>,
    pub indexer_id: i64,
    pub indexer_name: String,
    pub size: i64,
    pub age_days: i64,
    pub publish_date: DateTime<Utc>,
    pub info_hash: Option<String>,
    pub magnet_url: Option<String>,
    pub seeders: Option<i32>,
    pub leechers: Option<i32>,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
    pub tmdb_id: Option<i64>,
    pub categories: Vec<i32>,
    pub indexer_flags: Vec<String>,
}
use crate::definition::{CardigannDefinition, FieldBlock, ResponseBlock, SearchPathBlock};
use crate::filters::{self, apply_filter};
use crate::selector::{self, Document};
use crate::template::{QueryContext, TemplateContext};

/// A Cardigann-based indexer instance, ready to search.
#[derive(Debug, Clone)]
pub struct CardigannIndexer {
    pub definition: CardigannDefinition,
    pub config: HashMap<String, String>,
    pub base_url: String,
    pub indexer_id: i64,
    pub enabled: bool,
    http_client: Client,
    category_mapper: CategoryMapper,
}

/// Search query parameters.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub query: String,
    pub categories: Vec<i32>,
    pub search_type: SearchType,
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i64>,
    pub tmdb_id: Option<i64>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub enum SearchType {
    #[default]
    Search,
    TvSearch,
    MovieSearch,
    MusicSearch,
    BookSearch,
}

impl CardigannIndexer {
    /// Create a new indexer from a definition + user config.
    pub fn new(
        definition: CardigannDefinition,
        config: HashMap<String, String>,
        indexer_id: i64,
    ) -> Result<Self> {
        let base_url = config
            .get("baseUrl")
            .or_else(|| config.get("baseurl"))
            .cloned()
            .or_else(|| definition.links.first().cloned())
            .ok_or_else(|| anyhow::anyhow!("no base URL for indexer {}", definition.name))?;

        let category_mapper = CategoryMapper::from_caps(&definition.caps);

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .cookie_store(true)
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()
            .context("failed to build HTTP client")?;

        Ok(Self {
            definition,
            config,
            base_url,
            indexer_id,
            enabled: true,
            http_client: client,
            category_mapper,
        })
    }

    /// Perform login if the definition requires it (private trackers).
    async fn perform_login(&self) -> Result<()> {
        let login = match &self.definition.login {
            Some(login) => login,
            None => return Ok(()), // No login required (public indexer)
        };

        let method = login.method.as_deref().unwrap_or("form");

        // Build a template context for login field expansion
        let mut config = self.config.clone();
        let sitelink = if self.base_url.ends_with('/') {
            self.base_url.clone()
        } else {
            format!("{}/", self.base_url)
        };
        config.insert("sitelink".into(), sitelink.clone());

        let login_ctx = TemplateContext {
            config,
            keywords: String::new(),
            categories: Vec::new(),
            query: QueryContext::default(),
            result: HashMap::new(),
            true_val: "true".into(),
            false_val: String::new(),
        };

        match method {
            "form" | "post" => {
                // Build form inputs from definition
                let mut form_data = HashMap::new();
                if let Some(ref inputs) = login.inputs {
                    for (key, val_template) in inputs {
                        let value = crate::template::expand(val_template, &login_ctx)?;
                        form_data.insert(key.clone(), value);
                    }
                }

                let login_path = login.path.as_deref().unwrap_or("login");
                let login_url = format!("{}{}", sitelink, login_path.trim_start_matches('/'));

                tracing::debug!(
                    indexer = %self.definition.name,
                    url = %login_url,
                    "performing Cardigann login"
                );

                let resp = self.http_client
                    .post(&login_url)
                    .form(&form_data)
                    .send()
                    .await
                    .context("login request failed")?;

                let status = resp.status();
                if status.is_client_error() || status.is_server_error() {
                    bail!("login failed with HTTP {}", status.as_u16());
                }

                // Check login error selectors if defined
                if let Some(ref errors) = login.error {
                    let body = resp.text().await.unwrap_or_default();
                    for err_block in errors {
                        if let Some(ref sel) = err_block.selector {
                            if crate::selector::html_has_selector(&body, sel) {
                                let msg = err_block.message
                                    .as_ref()
                                    .and_then(|m| m.text.as_deref())
                                    .map(|t| crate::template::expand(t, &login_ctx).unwrap_or_else(|_| t.to_string()))
                                    .unwrap_or_else(|| format!("login error detected by selector: {sel}"));
                                bail!("{msg}");
                            }
                        }
                    }
                }

                tracing::debug!(indexer = %self.definition.name, "login successful");
                Ok(())
            }
            "cookie" => {
                // Cookie-based login: set cookies from config
                if let Some(ref cookies) = login.cookies {
                    for cookie_str in cookies {
                        let expanded = crate::template::expand(cookie_str, &login_ctx)?;
                        // Parse "name=value" and set on cookie jar via a dummy request
                        let cookie_header = format!("{}; domain={}", expanded,
                            url::Url::parse(&sitelink).ok().and_then(|u| u.host_str().map(String::from)).unwrap_or_default());
                        tracing::debug!(indexer = %self.definition.name, cookie = %cookie_header, "setting login cookie");
                    }
                }
                Ok(())
            }
            other => {
                tracing::warn!(indexer = %self.definition.name, method = other, "unsupported login method");
                Ok(())
            }
        }
    }

    /// Execute a search and return normalized releases.
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<CardigannRelease>> {
        // Perform login before search (for private trackers)
        self.perform_login().await?;

        let mut all_releases = Vec::new();
        let ctx = self.build_context(query)?;

        let paths = self.resolve_search_paths();

        for search_path in &paths {
            match self.execute_search_path(search_path, &ctx).await {
                Ok(releases) => all_releases.extend(releases),
                Err(e) => {
                    tracing::warn!(
                        indexer = %self.definition.name,
                        path = %search_path.path,
                        error = %e,
                        "search path failed"
                    );
                }
            }

            // Respect request delay
            if let Some(delay) = self.definition.request_delay {
                tokio::time::sleep(Duration::from_secs_f64(delay)).await;
            }
        }

        Ok(all_releases)
    }

    /// Build a template context from the search query and indexer config.
    fn build_context(&self, query: &SearchQuery) -> Result<TemplateContext> {
        let mut config = self.config.clone();

        // Ensure sitelink is set
        let sitelink = if self.base_url.ends_with('/') {
            self.base_url.clone()
        } else {
            format!("{}/", self.base_url)
        };
        config.insert("sitelink".into(), sitelink);

        // Apply default settings from the definition
        if let Some(ref settings) = self.definition.settings {
            for field in settings {
                if !config.contains_key(&field.name) {
                    if let Some(ref default) = field.default {
                        let val = match default {
                            serde_yaml::Value::String(s) => s.clone(),
                            serde_yaml::Value::Bool(b) => b.to_string(),
                            serde_yaml::Value::Number(n) => n.to_string(),
                            _ => String::new(),
                        };
                        config.insert(field.name.clone(), val);
                    }
                }
            }
        }

        // Apply keywords filters
        let mut keywords = query.query.clone();
        if let Some(ref kw_filters) = self.definition.search.keywordsfilters {
            for f in kw_filters {
                keywords = apply_filter(&f.name, &keywords, &f.args)?;
            }
        }

        // Map Newznab categories to indexer categories
        let categories: Vec<String> = self
            .category_mapper
            .from_newznab(&query.categories);

        Ok(TemplateContext {
            config,
            keywords,
            categories,
            query: QueryContext {
                imdbid: query.imdb_id.clone().unwrap_or_default(),
                tvdbid: query.tvdb_id.map(|id| id.to_string()).unwrap_or_default(),
                tmdbid: query.tmdb_id.map(|id| id.to_string()).unwrap_or_default(),
                season: query.season.map(|s| s.to_string()).unwrap_or_default(),
                ep: query.episode.map(|e| e.to_string()).unwrap_or_default(),
                ..Default::default()
            },
            result: HashMap::new(),
            true_val: "true".into(),
            false_val: String::new(),
        })
    }

    /// Resolve which search paths to use.
    fn resolve_search_paths(&self) -> Vec<SearchPathBlock> {
        if let Some(ref paths) = self.definition.search.paths {
            paths.clone()
        } else if let Some(ref path) = self.definition.search.path {
            vec![SearchPathBlock {
                path: path.clone(),
                method: None,
                inputs: None,
                queryseparator: None,
                categories: None,
                inheritinputs: Some(true),
                followredirect: None,
                response: None,
            }]
        } else {
            Vec::new()
        }
    }

    /// Execute a single search path and return parsed releases.
    async fn execute_search_path(
        &self,
        search_path: &SearchPathBlock,
        ctx: &TemplateContext,
    ) -> Result<Vec<CardigannRelease>> {
        // Expand template in path
        let expanded_path = crate::template::expand(&search_path.path, ctx)?;

        // Build full URL
        let url = if expanded_path.starts_with("http://") || expanded_path.starts_with("https://") {
            expanded_path.clone()
        } else {
            let base = if self.base_url.ends_with('/') {
                &self.base_url
            } else {
                &format!("{}/", self.base_url)
            };
            format!("{base}{expanded_path}")
        };

        // Build query parameters from inputs
        let mut url = url::Url::parse(&url).context("failed to parse search URL")?;

        // Merge global inputs
        let mut all_inputs: HashMap<String, String> = HashMap::new();
        if search_path.inheritinputs.unwrap_or(true) {
            if let Some(ref inputs) = self.definition.search.inputs {
                for (k, v) in inputs {
                    let expanded = crate::template::expand(v, ctx)?;
                    all_inputs.insert(k.clone(), expanded);
                }
            }
        }
        // Path-specific inputs override
        if let Some(ref inputs) = search_path.inputs {
            for (k, v) in inputs {
                let expanded = crate::template::expand(v, ctx)?;
                all_inputs.insert(k.clone(), expanded);
            }
        }

        // Add inputs to URL query string
        if !all_inputs.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (k, v) in &all_inputs {
                pairs.append_pair(k, v);
            }
        }

        tracing::debug!(
            indexer = %self.definition.name,
            url = %url,
            "executing Cardigann search"
        );

        // Execute HTTP request
        let method = search_path
            .method
            .as_deref()
            .unwrap_or("get");

        let response = match method.to_lowercase().as_str() {
            "post" => {
                self.http_client
                    .post(url.as_str())
                    .form(&all_inputs)
                    .send()
                    .await?
            }
            _ => self.http_client.get(url.as_str()).send().await?,
        };

        let status = response.status();
        if !status.is_success() {
            bail!(
                "search request failed: {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            );
        }

        let body = response.text().await?;

        // Determine response type
        let is_json = search_path
            .response
            .as_ref()
            .and_then(|r| r.response_type.as_deref())
            .map(|t| t == "json")
            .unwrap_or(false);

        if is_json {
            self.parse_json_response(&body, ctx).await
        } else {
            self.parse_html_response(&body, ctx).await
        }
    }

    /// Parse a JSON response into releases.
    async fn parse_json_response(
        &self,
        body: &str,
        base_ctx: &TemplateContext,
    ) -> Result<Vec<CardigannRelease>> {
        let json: JsonValue =
            serde_json::from_str(body).context("failed to parse JSON response")?;

        let rows_selector = self
            .definition
            .search
            .rows
            .selector
            .as_deref()
            .unwrap_or("$");

        let sub_attribute = self.definition.search.rows.attribute.as_deref();
        let multiple = self.definition.search.rows.multiple.unwrap_or(false);

        let rows = selector::select_json_rows(&json, rows_selector, sub_attribute, multiple)?;

        // Check count selector for "no results"
        if let Some(ref count_sel) = self.definition.search.rows.count {
            if let Some(ref sel) = count_sel.selector {
                if let Some(count_val) = crate::selector::json_path_pub(&json, sel) {
                    let count = match count_val {
                        JsonValue::Number(n) => n.as_i64().unwrap_or(0),
                        JsonValue::String(s) => s.parse().unwrap_or(0),
                        _ => 0,
                    };
                    if count == 0 {
                        return Ok(Vec::new());
                    }
                }
            }
        }

        let mut releases = Vec::new();

        for row in &rows {
            match self.extract_release_from_json(row, base_ctx) {
                Ok(Some(release)) => releases.push(release),
                Ok(None) => {}
                Err(e) => {
                    tracing::trace!(
                        indexer = %self.definition.name,
                        error = %e,
                        "failed to extract release from JSON row"
                    );
                }
            }
        }

        Ok(releases)
    }

    /// Parse an HTML response into releases.
    async fn parse_html_response(
        &self,
        body: &str,
        base_ctx: &TemplateContext,
    ) -> Result<Vec<CardigannRelease>> {
        let rows_selector = self
            .definition
            .search
            .rows
            .selector
            .as_deref()
            .unwrap_or("tr");

        let expanded_selector = crate::template::expand(rows_selector, base_ctx)?;
        let after = self.definition.search.rows.after;

        let rows = selector::select_html_rows(body, &expanded_selector, after)?;

        let mut releases = Vec::new();

        for row_html in &rows {
            match self.extract_release_from_html(row_html, base_ctx) {
                Ok(Some(release)) => releases.push(release),
                Ok(None) => {}
                Err(e) => {
                    tracing::trace!(
                        indexer = %self.definition.name,
                        error = %e,
                        "failed to extract release from HTML row"
                    );
                }
            }
        }

        Ok(releases)
    }

    /// Extract a release from a JSON row.
    fn extract_release_from_json(
        &self,
        row: &JsonValue,
        base_ctx: &TemplateContext,
    ) -> Result<Option<CardigannRelease>> {
        let mut ctx = base_ctx.clone();
        let fields = &self.definition.search.fields;

        // Extract all fields, building up `ctx.result` for cross-references
        let mut extracted: HashMap<String, String> = HashMap::new();

        for (name, field) in fields {
            let value = selector::extract_json_field(row, field, &ctx)?;
            if let Some(ref v) = value {
                ctx.result.insert(name.clone(), v.clone());
                extracted.insert(name.clone(), v.clone());
            }
        }

        self.build_release(&extracted)
    }

    /// Extract a release from an HTML row fragment.
    fn extract_release_from_html(
        &self,
        row_html: &str,
        base_ctx: &TemplateContext,
    ) -> Result<Option<CardigannRelease>> {
        let mut ctx = base_ctx.clone();
        let fields = &self.definition.search.fields;

        let mut extracted: HashMap<String, String> = HashMap::new();

        for (name, field) in fields {
            let value = selector::extract_html_field(row_html, field, &ctx)?;
            if let Some(ref v) = value {
                ctx.result.insert(name.clone(), v.clone());
                extracted.insert(name.clone(), v.clone());
            }
        }

        self.build_release(&extracted)
    }

    /// Build a `ReleaseInfo` from extracted field values.
    fn build_release(&self, fields: &HashMap<String, String>) -> Result<Option<CardigannRelease>> {
        let title = match fields.get("title") {
            Some(t) if !t.is_empty() => t.clone(),
            _ => return Ok(None),
        };

        let download_url = fields.get("download").or(fields.get("magnet")).cloned();
        let details_url = fields.get("details").cloned();

        // Resolve relative URLs
        let download_url = download_url.map(|u| self.resolve_url(&u));
        let details_url = details_url.map(|u| self.resolve_url(&u));

        let size = fields
            .get("size")
            .and_then(|s| parse_size(s))
            .unwrap_or(0);

        let seeders = fields
            .get("seeders")
            .and_then(|s| s.replace(',', "").parse().ok());

        let leechers = fields
            .get("leechers")
            .and_then(|s| s.replace(',', "").parse().ok());

        let publish_date = fields
            .get("date")
            .and_then(|s| parse_date(s))
            .unwrap_or_else(Utc::now);

        let age_days = (Utc::now() - publish_date).num_days();

        let categories: Vec<i32> = fields
            .get("category")
            .map(|c| self.category_mapper.to_newznab(c))
            .unwrap_or_default();

        let infohash = fields.get("infohash").cloned();
        let magnet = fields.get("magneturl").or(fields.get("magnet")).cloned();
        let imdb_id = fields.get("imdbid").cloned().filter(|s| !s.is_empty());
        let tvdb_id = fields
            .get("tvdbid")
            .and_then(|s| s.parse().ok());
        let tmdb_id = fields
            .get("tmdbid")
            .and_then(|s| s.parse().ok());

        // Build indexer flags from volume factors
        let mut flags = Vec::new();
        if fields.get("downloadvolumefactor").map(String::as_str) == Some("0") {
            flags.push("freeleech".to_owned());
        }

        let guid = infohash
            .clone()
            .or_else(|| download_url.clone())
            .or_else(|| details_url.clone())
            .unwrap_or_else(|| format!("{}-{}", self.definition.id, title));

        Ok(Some(CardigannRelease {
            guid,
            title,
            download_url,
            info_url: details_url,
            indexer_id: self.indexer_id,
            indexer_name: self.definition.name.clone(),
            size,
            age_days,
            publish_date,
            info_hash: infohash,
            magnet_url: magnet,
            seeders,
            leechers,
            tvdb_id,
            imdb_id,
            tmdb_id,
            categories,
            indexer_flags: flags,
        }))
    }

    /// Resolve a potentially relative URL against the base URL.
    fn resolve_url(&self, url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("magnet:") {
            return url.to_owned();
        }
        let base = if self.base_url.ends_with('/') {
            &self.base_url
        } else {
            &self.base_url
        };
        if url.starts_with('/') {
            // Absolute path
            if let Ok(parsed) = url::Url::parse(base) {
                return format!("{}://{}{}", parsed.scheme(), parsed.host_str().unwrap_or(""), url);
            }
        }
        format!("{}/{}", base.trim_end_matches('/'), url.trim_start_matches('/'))
    }
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Parse a human-readable size string like "1.5 GB" or raw bytes.
fn parse_size(s: &str) -> Option<i64> {
    let s = s.trim().replace(',', "");

    // Already numeric (bytes)
    if let Ok(n) = s.parse::<i64>() {
        return Some(n);
    }
    if let Ok(n) = s.parse::<f64>() {
        return Some(n as i64);
    }

    // Human-readable
    let re = regex::Regex::new(r"(?i)([\d.]+)\s*(bytes?|kb?|mb?|gb?|tb?|kib|mib|gib|tib)")
        .ok()?;
    let caps = re.captures(&s)?;
    let num: f64 = caps[1].parse().ok()?;
    let unit = caps[2].to_lowercase();

    let multiplier: f64 = match unit.as_str() {
        "b" | "byte" | "bytes" => 1.0,
        "k" | "kb" => 1_000.0,
        "kib" => 1_024.0,
        "m" | "mb" => 1_000_000.0,
        "mib" => 1_048_576.0,
        "g" | "gb" => 1_000_000_000.0,
        "gib" => 1_073_741_824.0,
        "t" | "tb" => 1_000_000_000_000.0,
        "tib" => 1_099_511_627_776.0,
        _ => 1.0,
    };

    Some((num * multiplier) as i64)
}

/// Parse a date string — tries RFC3339, unix timestamp, and common formats.
fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();

    // RFC 3339
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Unix timestamp
    if let Ok(ts) = s.parse::<i64>() {
        return chrono::DateTime::from_timestamp(ts, 0);
    }

    // Already parsed by a filter (RFC3339 output from timeago/dateparse)
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    None
}

