use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Type of content a discover slider shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum DiscoverSliderType {
    Trending,
    PopularMovies,
    PopularTv,
    UpcomingMovies,
    UpcomingTv,
    RecentlyAdded,
    MovieGenres,
    TvGenres,
    // Custom slider types (configured with custom_data)
    TmdbMovieGenre,
    TmdbTvGenre,
    TmdbMovieKeyword,
    TmdbTvKeyword,
    TmdbSearch,
    TmdbStudio,
    TmdbNetwork,
    TmdbMovieStreamingServices,
    TmdbTvStreamingServices,
}

/// A configurable discover slider for the homepage.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverSlider {
    pub id: i32,
    pub slider_type: DiscoverSliderType,
    pub display_order: i32,
    pub is_built_in: bool,
    pub enabled: bool,
    pub title: Option<String>,
    pub custom_data: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating a custom discover slider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDiscoverSliderInput {
    pub slider_type: DiscoverSliderType,
    pub title: Option<String>,
    pub custom_data: Option<serde_json::Value>,
    pub enabled: Option<bool>,
}

/// Input for updating a discover slider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDiscoverSliderInput {
    pub title: Option<String>,
    pub enabled: Option<bool>,
    pub custom_data: Option<serde_json::Value>,
}

/// Input for reordering all sliders.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReorderSlidersInput {
    /// Ordered list of slider IDs in the desired display order.
    pub slider_ids: Vec<i32>,
}
