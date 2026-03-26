use chrono::NaiveDate;
use serde::Deserialize;

/// Client for The Movie Database (TMDB) API.
pub struct TmdbClient {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

// ── Result types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct TmdbSearchResults<T> {
    pub page: i64,
    pub total_pages: i64,
    pub total_results: i64,
    pub results: Vec<T>,
}

#[derive(Debug, Clone, Deserialize)]
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
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
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
    pub popularity: f64,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct TmdbGenre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TmdbNetwork {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TmdbExternalIds {
    pub tvdb_id: Option<i64>,
    pub imdb_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TmdbCollection {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TmdbCompany {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TmdbSeason {
    pub id: i64,
    pub season_number: i32,
    pub name: String,
    pub air_date: Option<NaiveDate>,
    #[serde(default)]
    pub episodes: Vec<TmdbEpisode>,
}

#[derive(Debug, Clone, Deserialize)]
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
}
