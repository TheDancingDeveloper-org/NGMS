use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use leaky_bucket::RateLimiter;
use lru::LruCache;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

// ── Cache types ─────────────────────────────────────────────────────────────

/// TTL categories for cached responses.
#[derive(Clone, Copy)]
enum CacheTtl {
    /// Search and list results: 1 hour.
    Search,
    /// Detail/single-resource fetches: 24 hours.
    Detail,
}

impl CacheTtl {
    fn duration(self) -> Duration {
        match self {
            Self::Search => Duration::from_secs(60 * 60),       // 1 hour
            Self::Detail => Duration::from_secs(60 * 60 * 24),  // 24 hours
        }
    }
}

struct CacheEntry {
    value: serde_json::Value,
    expires_at: Instant,
}

/// Client for The Movie Database (TMDB) API.
pub struct TmdbClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    limiter: Arc<RateLimiter>,
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
}

// ── Result types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbSearchResults<T> {
    pub page: i64,
    pub total_pages: i64,
    pub total_results: i64,
    pub results: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbSeries {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub overview: Option<String>,
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub genre_ids: Vec<i64>,
    #[serde(default)]
    pub vote_average: f64,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub vote_count: i64,
    #[serde(default)]
    pub original_language: Option<String>,
}

/// A multi-result from TMDB trending that can be either a movie or TV show.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbTrendingItem {
    pub id: i64,
    pub media_type: String, // "movie" or "tv"
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub first_air_date: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub genre_ids: Vec<i64>,
    #[serde(default)]
    pub vote_average: f64,
    #[serde(default)]
    pub vote_count: i64,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub original_language: Option<String>,
}

impl TmdbTrendingItem {
    /// Unified display title (movie uses `title`, TV uses `name`).
    pub fn display_title(&self) -> &str {
        self.title
            .as_deref()
            .or(self.name.as_deref())
            .unwrap_or("Unknown")
    }
}

/// Language entry from TMDB /configuration/languages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbLanguage {
    pub iso_639_1: String,
    pub english_name: String,
    #[serde(default)]
    pub name: String,
}

/// Keyword entry from TMDB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbKeyword {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbSeriesDetail {
    pub id: i64,
    pub name: String,
    pub overview: Option<String>,
    pub first_air_date: Option<String>,
    pub status: Option<String>,
    pub number_of_seasons: Option<i32>,
    pub number_of_episodes: Option<i32>,
    #[serde(default)]
    pub episode_run_time: Vec<i32>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub genres: Vec<TmdbGenre>,
    #[serde(default)]
    pub networks: Vec<TmdbNetwork>,
    pub external_ids: Option<TmdbExternalIds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbMovie {
    pub id: i64,
    pub title: String,
    #[serde(default)]
    pub overview: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub poster_path: Option<String>,
    #[serde(default)]
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub genre_ids: Vec<i64>,
    #[serde(default)]
    pub vote_average: f64,
    #[serde(default)]
    pub vote_count: i64,
    #[serde(default)]
    pub popularity: f64,
    #[serde(default)]
    pub original_language: Option<String>,
}

// ── Discovery filter parameters ─────────────────────────────────────────────

/// Filters for the TMDB `/discover/movie` and `/discover/tv` endpoints.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverFilters {
    pub page: Option<i64>,
    pub sort_by: Option<String>,
    pub with_genres: Option<String>,
    pub without_genres: Option<String>,
    pub with_keywords: Option<String>,
    pub without_keywords: Option<String>,
    pub with_companies: Option<String>,
    pub with_networks: Option<String>,
    pub with_watch_providers: Option<String>,
    pub watch_region: Option<String>,
    pub with_original_language: Option<String>,
    pub primary_release_date_gte: Option<String>,
    pub primary_release_date_lte: Option<String>,
    pub first_air_date_gte: Option<String>,
    pub first_air_date_lte: Option<String>,
    pub with_runtime_gte: Option<i32>,
    pub with_runtime_lte: Option<i32>,
    pub vote_average_gte: Option<f64>,
    pub vote_average_lte: Option<f64>,
    pub vote_count_gte: Option<i64>,
    pub vote_count_lte: Option<i64>,
    pub with_status: Option<String>,
    pub certification: Option<String>,
    pub certification_country: Option<String>,
    pub language: Option<String>,
}

impl DiscoverFilters {
    /// Build query string parameters for TMDB discover endpoint.
    fn to_query_pairs(&self) -> Vec<(&str, String)> {
        let mut pairs = Vec::new();
        if let Some(ref v) = self.page { pairs.push(("page", v.to_string())); }
        if let Some(ref v) = self.sort_by { pairs.push(("sort_by", v.clone())); }
        if let Some(ref v) = self.with_genres { pairs.push(("with_genres", v.clone())); }
        if let Some(ref v) = self.without_genres { pairs.push(("without_genres", v.clone())); }
        if let Some(ref v) = self.with_keywords { pairs.push(("with_keywords", v.clone())); }
        if let Some(ref v) = self.without_keywords { pairs.push(("without_keywords", v.clone())); }
        if let Some(ref v) = self.with_companies { pairs.push(("with_companies", v.clone())); }
        if let Some(ref v) = self.with_networks { pairs.push(("with_networks", v.clone())); }
        if let Some(ref v) = self.with_watch_providers { pairs.push(("with_watch_providers", v.clone())); }
        if let Some(ref v) = self.watch_region { pairs.push(("watch_region", v.clone())); }
        if let Some(ref v) = self.with_original_language { pairs.push(("with_original_language", v.clone())); }
        if let Some(ref v) = self.primary_release_date_gte { pairs.push(("primary_release_date.gte", v.clone())); }
        if let Some(ref v) = self.primary_release_date_lte { pairs.push(("primary_release_date.lte", v.clone())); }
        if let Some(ref v) = self.first_air_date_gte { pairs.push(("first_air_date.gte", v.clone())); }
        if let Some(ref v) = self.first_air_date_lte { pairs.push(("first_air_date.lte", v.clone())); }
        if let Some(v) = self.with_runtime_gte { pairs.push(("with_runtime.gte", v.to_string())); }
        if let Some(v) = self.with_runtime_lte { pairs.push(("with_runtime.lte", v.to_string())); }
        if let Some(v) = self.vote_average_gte { pairs.push(("vote_average.gte", v.to_string())); }
        if let Some(v) = self.vote_average_lte { pairs.push(("vote_average.lte", v.to_string())); }
        if let Some(v) = self.vote_count_gte { pairs.push(("vote_count.gte", v.to_string())); }
        if let Some(v) = self.vote_count_lte { pairs.push(("vote_count.lte", v.to_string())); }
        if let Some(ref v) = self.with_status { pairs.push(("with_status", v.clone())); }
        if let Some(ref v) = self.certification { pairs.push(("certification", v.clone())); }
        if let Some(ref v) = self.certification_country { pairs.push(("certification_country", v.clone())); }
        if let Some(ref v) = self.language { pairs.push(("language", v.clone())); }
        pairs
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbMovieDetail {
    pub id: i64,
    pub title: String,
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub status: Option<String>,
    pub runtime: Option<i32>,
    pub budget: Option<i64>,
    pub revenue: Option<i64>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    #[serde(default)]
    pub genres: Vec<TmdbGenre>,
    pub belongs_to_collection: Option<TmdbCollection>,
    pub imdb_id: Option<String>,
    #[serde(default)]
    pub production_companies: Vec<TmdbCompany>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbGenre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbNetwork {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbExternalIds {
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbCollection {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbCompany {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbSeason {
    pub id: i64,
    pub season_number: i32,
    pub name: String,
    pub air_date: Option<NaiveDate>,
    #[serde(default)]
    pub episodes: Vec<TmdbEpisode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmdbEpisode {
    pub id: i64,
    pub episode_number: i32,
    pub season_number: i32,
    pub name: String,
    pub overview: Option<String>,
    pub air_date: Option<NaiveDate>,
    pub runtime: Option<i32>,
}

// ── Client implementation ───────────────────────────────────────────────────

impl TmdbClient {
    fn build_limiter() -> Arc<RateLimiter> {
        Arc::new(
            RateLimiter::builder()
                .initial(4)
                .max(4)
                .interval(Duration::from_millis(250)) // 4 tokens/sec
                .build(),
        )
    }

    fn build_cache() -> Arc<Mutex<LruCache<String, CacheEntry>>> {
        Arc::new(Mutex::new(LruCache::new(
            NonZeroUsize::new(2000).expect("non-zero"),
        )))
    }

    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://api.themoviedb.org/3".to_string(),
            limiter: Self::build_limiter(),
            cache: Self::build_cache(),
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url,
            limiter: Self::build_limiter(),
            cache: Self::build_cache(),
        }
    }

    // ── Internal helpers ────────────────────────────────────────────────────

    /// Check the cache for a valid (non-expired) entry.
    fn cache_get(&self, key: &str) -> Option<serde_json::Value> {
        let mut cache = self.cache.lock();
        if let Some(entry) = cache.get(key) {
            if Instant::now() < entry.expires_at {
                return Some(entry.value.clone());
            }
            // Expired -- remove it.
            cache.pop(key);
        }
        None
    }

    /// Store a value in the cache with the given TTL.
    fn cache_put(&self, key: String, value: serde_json::Value, ttl: CacheTtl) {
        let entry = CacheEntry {
            value,
            expires_at: Instant::now() + ttl.duration(),
        };
        self.cache.lock().put(key, entry);
    }

    /// Rate-limited GET that honours the cache.
    async fn cached_get(
        &self,
        url: &str,
        ttl: CacheTtl,
    ) -> anyhow::Result<serde_json::Value> {
        // 1. Check cache
        if let Some(hit) = self.cache_get(url) {
            tracing::trace!("TMDB cache hit: {url}");
            return Ok(hit);
        }

        // 2. Rate-limit
        self.limiter.acquire_one().await;

        // 3. Fetch
        let resp = self.client.get(url).send().await?.error_for_status()?;
        let value: serde_json::Value = resp.json().await?;

        // 4. Store
        self.cache_put(url.to_string(), value.clone(), ttl);

        Ok(value)
    }

    /// Search for TV series by name.
    pub async fn search_series(
        &self,
        query: &str,
        year: Option<i32>,
    ) -> anyhow::Result<TmdbSearchResults<TmdbSeries>> {
        let mut url = format!("{}/search/tv?api_key={}&query={}", self.base_url, self.api_key, query);
        if let Some(y) = year {
            url.push_str(&format!("&first_air_date_year={y}"));
        }
        tracing::debug!("TMDB search series: {query}");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Search for movies by name.
    pub async fn search_movie(
        &self,
        query: &str,
        year: Option<i32>,
    ) -> anyhow::Result<TmdbSearchResults<TmdbMovie>> {
        let mut url = format!("{}/search/movie?api_key={}&query={}", self.base_url, self.api_key, query);
        if let Some(y) = year {
            url.push_str(&format!("&year={y}"));
        }
        tracing::debug!("TMDB search movie: {query}");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get detailed info for a TV series by TMDB id.
    pub async fn get_series(&self, tmdb_id: i64) -> anyhow::Result<TmdbSeriesDetail> {
        let url = format!(
            "{}/tv/{tmdb_id}?api_key={}&append_to_response=external_ids",
            self.base_url, self.api_key
        );
        tracing::debug!("TMDB get series: {tmdb_id}");
        let value = self.cached_get(&url, CacheTtl::Detail).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get detailed info for a movie by TMDB id.
    pub async fn get_movie(&self, tmdb_id: i64) -> anyhow::Result<TmdbMovieDetail> {
        let url = format!(
            "{}/movie/{tmdb_id}?api_key={}",
            self.base_url, self.api_key
        );
        tracing::debug!("TMDB get movie: {tmdb_id}");
        let value = self.cached_get(&url, CacheTtl::Detail).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get season details including episodes.
    pub async fn get_season(
        &self,
        series_tmdb_id: i64,
        season_number: i32,
    ) -> anyhow::Result<TmdbSeason> {
        let url = format!(
            "{}/tv/{series_tmdb_id}/season/{season_number}?api_key={}",
            self.base_url, self.api_key
        );
        tracing::debug!("TMDB get season: series={series_tmdb_id} season={season_number}");
        let value = self.cached_get(&url, CacheTtl::Detail).await?;
        Ok(serde_json::from_value(value)?)
    }

    // ── Discovery endpoints ─────────────────────────────────────────────────

    /// Get trending media (all, movie, or tv) for a given time window.
    pub async fn get_trending(
        &self,
        media_type: &str, // "all", "movie", or "tv"
        time_window: &str, // "day" or "week"
        page: Option<i64>,
        language: Option<&str>,
    ) -> anyhow::Result<TmdbSearchResults<TmdbTrendingItem>> {
        let mut url = format!(
            "{}/trending/{media_type}/{time_window}?api_key={}",
            self.base_url, self.api_key
        );
        if let Some(p) = page { url.push_str(&format!("&page={p}")); }
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB trending: {media_type}/{time_window}");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Discover movies with advanced filters.
    pub async fn discover_movies(
        &self,
        filters: &DiscoverFilters,
    ) -> anyhow::Result<TmdbSearchResults<TmdbMovie>> {
        let mut url = format!("{}/discover/movie?api_key={}", self.base_url, self.api_key);
        for (key, val) in filters.to_query_pairs() {
            url.push_str(&format!("&{key}={val}"));
        }
        tracing::debug!("TMDB discover movies");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Discover TV shows with advanced filters.
    pub async fn discover_tv(
        &self,
        filters: &DiscoverFilters,
    ) -> anyhow::Result<TmdbSearchResults<TmdbSeries>> {
        let mut url = format!("{}/discover/tv?api_key={}", self.base_url, self.api_key);
        for (key, val) in filters.to_query_pairs() {
            url.push_str(&format!("&{key}={val}"));
        }
        tracing::debug!("TMDB discover tv");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get movie recommendations for a given movie.
    pub async fn get_movie_recommendations(
        &self,
        movie_id: i64,
        page: Option<i64>,
        language: Option<&str>,
    ) -> anyhow::Result<TmdbSearchResults<TmdbMovie>> {
        let mut url = format!(
            "{}/movie/{movie_id}/recommendations?api_key={}",
            self.base_url, self.api_key
        );
        if let Some(p) = page { url.push_str(&format!("&page={p}")); }
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB movie recommendations: {movie_id}");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get similar movies for a given movie.
    pub async fn get_movie_similar(
        &self,
        movie_id: i64,
        page: Option<i64>,
        language: Option<&str>,
    ) -> anyhow::Result<TmdbSearchResults<TmdbMovie>> {
        let mut url = format!(
            "{}/movie/{movie_id}/similar?api_key={}",
            self.base_url, self.api_key
        );
        if let Some(p) = page { url.push_str(&format!("&page={p}")); }
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB movie similar: {movie_id}");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get TV show recommendations for a given series.
    pub async fn get_tv_recommendations(
        &self,
        tv_id: i64,
        page: Option<i64>,
        language: Option<&str>,
    ) -> anyhow::Result<TmdbSearchResults<TmdbSeries>> {
        let mut url = format!(
            "{}/tv/{tv_id}/recommendations?api_key={}",
            self.base_url, self.api_key
        );
        if let Some(p) = page { url.push_str(&format!("&page={p}")); }
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB tv recommendations: {tv_id}");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get similar TV shows for a given series.
    pub async fn get_tv_similar(
        &self,
        tv_id: i64,
        page: Option<i64>,
        language: Option<&str>,
    ) -> anyhow::Result<TmdbSearchResults<TmdbSeries>> {
        let mut url = format!(
            "{}/tv/{tv_id}/similar?api_key={}",
            self.base_url, self.api_key
        );
        if let Some(p) = page { url.push_str(&format!("&page={p}")); }
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB tv similar: {tv_id}");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get all movie genres.
    pub async fn get_movie_genres(
        &self,
        language: Option<&str>,
    ) -> anyhow::Result<Vec<TmdbGenre>> {
        let mut url = format!("{}/genre/movie/list?api_key={}", self.base_url, self.api_key);
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB movie genres");
        let body = self.cached_get(&url, CacheTtl::Detail).await?;
        let genres: Vec<TmdbGenre> = serde_json::from_value(
            body.get("genres").cloned().unwrap_or(serde_json::Value::Array(vec![]))
        )?;
        Ok(genres)
    }

    /// Get all TV genres.
    pub async fn get_tv_genres(
        &self,
        language: Option<&str>,
    ) -> anyhow::Result<Vec<TmdbGenre>> {
        let mut url = format!("{}/genre/tv/list?api_key={}", self.base_url, self.api_key);
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB tv genres");
        let body = self.cached_get(&url, CacheTtl::Detail).await?;
        let genres: Vec<TmdbGenre> = serde_json::from_value(
            body.get("genres").cloned().unwrap_or(serde_json::Value::Array(vec![]))
        )?;
        Ok(genres)
    }

    /// Get available languages from TMDB.
    pub async fn get_languages(&self) -> anyhow::Result<Vec<TmdbLanguage>> {
        let url = format!("{}/configuration/languages?api_key={}", self.base_url, self.api_key);
        tracing::debug!("TMDB languages");
        let value = self.cached_get(&url, CacheTtl::Detail).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get keyword details by ID.
    pub async fn get_keyword(&self, keyword_id: i64) -> anyhow::Result<TmdbKeyword> {
        let url = format!("{}/keyword/{keyword_id}?api_key={}", self.base_url, self.api_key);
        tracing::debug!("TMDB keyword: {keyword_id}");
        let value = self.cached_get(&url, CacheTtl::Detail).await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Get movies by keyword.
    pub async fn get_movies_by_keyword(
        &self,
        keyword_id: i64,
        page: Option<i64>,
        language: Option<&str>,
    ) -> anyhow::Result<TmdbSearchResults<TmdbMovie>> {
        let mut url = format!(
            "{}/keyword/{keyword_id}/movies?api_key={}",
            self.base_url, self.api_key
        );
        if let Some(p) = page { url.push_str(&format!("&page={p}")); }
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB movies by keyword: {keyword_id}");
        let value = self.cached_get(&url, CacheTtl::Search).await?;
        Ok(serde_json::from_value(value)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path_regex};

    #[test]
    fn test_trending_item_display_title_movie() {
        let item = TmdbTrendingItem {
            id: 1,
            media_type: "movie".into(),
            title: Some("Inception".into()),
            name: None,
            overview: None,
            release_date: None,
            first_air_date: None,
            poster_path: None,
            backdrop_path: None,
            genre_ids: vec![],
            vote_average: 0.0,
            vote_count: 0,
            popularity: 0.0,
            original_language: None,
        };
        assert_eq!(item.display_title(), "Inception");
    }

    #[test]
    fn test_trending_item_display_title_tv() {
        let item = TmdbTrendingItem {
            id: 2,
            media_type: "tv".into(),
            title: None,
            name: Some("Breaking Bad".into()),
            overview: None,
            release_date: None,
            first_air_date: None,
            poster_path: None,
            backdrop_path: None,
            genre_ids: vec![],
            vote_average: 0.0,
            vote_count: 0,
            popularity: 0.0,
            original_language: None,
        };
        assert_eq!(item.display_title(), "Breaking Bad");
    }

    #[test]
    fn test_trending_item_display_title_unknown() {
        let item = TmdbTrendingItem {
            id: 3,
            media_type: "tv".into(),
            title: None,
            name: None,
            overview: None,
            release_date: None,
            first_air_date: None,
            poster_path: None,
            backdrop_path: None,
            genre_ids: vec![],
            vote_average: 0.0,
            vote_count: 0,
            popularity: 0.0,
            original_language: None,
        };
        assert_eq!(item.display_title(), "Unknown");
    }

    #[test]
    fn test_discover_filters_to_query_pairs() {
        let filters = DiscoverFilters {
            page: Some(2),
            sort_by: Some("popularity.desc".into()),
            with_genres: Some("28,12".into()),
            vote_average_gte: Some(7.0),
            ..Default::default()
        };
        let pairs = filters.to_query_pairs();
        assert!(pairs.contains(&("page", "2".into())));
        assert!(pairs.contains(&("sort_by", "popularity.desc".into())));
        assert!(pairs.contains(&("with_genres", "28,12".into())));
        assert!(pairs.contains(&("vote_average.gte", "7".into())));
    }

    #[test]
    fn test_discover_filters_empty() {
        let filters = DiscoverFilters::default();
        assert!(filters.to_query_pairs().is_empty());
    }

    #[tokio::test]
    async fn test_search_series_wiremock() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "page": 1,
            "total_pages": 1,
            "total_results": 1,
            "results": [{
                "id": 1396,
                "name": "Breaking Bad",
                "overview": "A chemistry teacher diagnosed with cancer.",
                "first_air_date": "2008-01-20",
                "genre_ids": [18, 80],
                "vote_average": 8.9,
                "popularity": 100.0,
                "vote_count": 5000
            }]
        });

        Mock::given(method("GET"))
            .and(path_regex(r"/search/tv.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let client = TmdbClient::with_base_url("test-key".into(), mock_server.uri());
        let results = client.search_series("Breaking Bad", None).await.unwrap();
        assert_eq!(results.total_results, 1);
        assert_eq!(results.results[0].name, "Breaking Bad");
        assert_eq!(results.results[0].id, 1396);
    }

    #[tokio::test]
    async fn test_get_movie_detail_wiremock() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "id": 550,
            "title": "Fight Club",
            "overview": "An insomniac office worker...",
            "release_date": "1999-10-15",
            "status": "Released",
            "runtime": 139,
            "budget": 63000000,
            "revenue": 101209702,
            "genres": [{"id": 18, "name": "Drama"}],
            "imdb_id": "tt0137523",
            "production_companies": [{"id": 508, "name": "Regency Enterprises"}]
        });

        Mock::given(method("GET"))
            .and(path_regex(r"/movie/550.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let client = TmdbClient::with_base_url("test-key".into(), mock_server.uri());
        let movie = client.get_movie(550).await.unwrap();
        assert_eq!(movie.id, 550);
        assert_eq!(movie.title, "Fight Club");
        assert_eq!(movie.runtime, Some(139));
        assert_eq!(movie.imdb_id.as_deref(), Some("tt0137523"));
        assert_eq!(movie.genres.len(), 1);
        assert_eq!(movie.genres[0].name, "Drama");
    }

    #[tokio::test]
    async fn test_get_trending_wiremock() {
        let mock_server = MockServer::start().await;

        let body = serde_json::json!({
            "page": 1,
            "total_pages": 5,
            "total_results": 100,
            "results": [
                {"id": 1, "media_type": "movie", "title": "Trending Movie", "vote_average": 7.5, "popularity": 50.0, "vote_count": 100, "genre_ids": []},
                {"id": 2, "media_type": "tv", "name": "Trending Show", "vote_average": 8.0, "popularity": 60.0, "vote_count": 200, "genre_ids": []}
            ]
        });

        Mock::given(method("GET"))
            .and(path_regex(r"/trending/.*"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&body))
            .mount(&mock_server)
            .await;

        let client = TmdbClient::with_base_url("test-key".into(), mock_server.uri());
        let results = client.get_trending("all", "week", None, None).await.unwrap();
        assert_eq!(results.results.len(), 2);
        assert_eq!(results.results[0].display_title(), "Trending Movie");
        assert_eq!(results.results[1].display_title(), "Trending Show");
    }
}
