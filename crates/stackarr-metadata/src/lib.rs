use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Client for The Movie Database (TMDB) API.
pub struct TmdbClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
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
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://api.themoviedb.org/3".to_string(),
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            base_url,
        }
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get detailed info for a TV series by TMDB id.
    pub async fn get_series(&self, tmdb_id: i64) -> anyhow::Result<TmdbSeriesDetail> {
        let url = format!(
            "{}/tv/{tmdb_id}?api_key={}&append_to_response=external_ids",
            self.base_url, self.api_key
        );
        tracing::debug!("TMDB get series: {tmdb_id}");
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get detailed info for a movie by TMDB id.
    pub async fn get_movie(&self, tmdb_id: i64) -> anyhow::Result<TmdbMovieDetail> {
        let url = format!(
            "{}/movie/{tmdb_id}?api_key={}",
            self.base_url, self.api_key
        );
        tracing::debug!("TMDB get movie: {tmdb_id}");
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get all movie genres.
    pub async fn get_movie_genres(
        &self,
        language: Option<&str>,
    ) -> anyhow::Result<Vec<TmdbGenre>> {
        let mut url = format!("{}/genre/movie/list?api_key={}", self.base_url, self.api_key);
        if let Some(lang) = language { url.push_str(&format!("&language={lang}")); }
        tracing::debug!("TMDB movie genres");
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        let body: serde_json::Value = resp.json().await?;
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        let body: serde_json::Value = resp.json().await?;
        let genres: Vec<TmdbGenre> = serde_json::from_value(
            body.get("genres").cloned().unwrap_or(serde_json::Value::Array(vec![]))
        )?;
        Ok(genres)
    }

    /// Get available languages from TMDB.
    pub async fn get_languages(&self) -> anyhow::Result<Vec<TmdbLanguage>> {
        let url = format!("{}/configuration/languages?api_key={}", self.base_url, self.api_key);
        tracing::debug!("TMDB languages");
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }

    /// Get keyword details by ID.
    pub async fn get_keyword(&self, keyword_id: i64) -> anyhow::Result<TmdbKeyword> {
        let url = format!("{}/keyword/{keyword_id}?api_key={}", self.base_url, self.api_key);
        tracing::debug!("TMDB keyword: {keyword_id}");
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
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
        let resp = self.client.get(&url).send().await?.error_for_status()?;
        Ok(resp.json().await?)
    }
}
