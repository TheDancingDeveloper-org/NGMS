//! TMDB fuzzy-match helper for the import-candidates pipeline.
//!
//! Given a parsed title (and optional year) that the disk scanner could not
//! match against an existing DB row, pick the single best TMDB result and
//! return it along with a confidence score in `[0.0, 1.0]`.
//!
//! Confidence is a blend of:
//!   * Normalized edit distance between parsed and TMDB titles (0.0–0.85).
//!   * +0.15 when the year exactly matches.
//!
//! Results with a confidence below [`MIN_CONFIDENCE`] are treated as "no
//! useful suggestion" and return `None`.

use sqlx::MySqlPool;
use stackarr_core::models::ImportCandidate;
use stackarr_metadata::TmdbClient;

/// Minimum confidence we'll record on a candidate. Anything below this is
/// useless noise and the UI would only frustrate the user.
pub const MIN_CONFIDENCE: f32 = 0.45;

/// A single TMDB suggestion for an [`ImportCandidate`](stackarr_core::models::ImportCandidate).
#[derive(Debug, Clone)]
pub struct TmdbSuggestion {
    pub tmdb_id: i32,
    pub title: String,
    pub year: Option<i32>,
    pub poster_path: Option<String>,
    pub overview: Option<String>,
    pub confidence: f32,
}

/// Suggest a TMDB series for a parsed title+year.
pub async fn suggest_series(
    tmdb: &TmdbClient,
    parsed_title: &str,
    parsed_year: Option<i32>,
) -> Option<TmdbSuggestion> {
    let results = match tmdb.search_series(parsed_title, parsed_year).await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %e, "TMDB series search failed");
            return None;
        }
    };
    let best = results
        .results
        .into_iter()
        .map(|s| {
            let year = s
                .first_air_date
                .as_deref()
                .and_then(|d| d.get(..4))
                .and_then(|y| y.parse::<i32>().ok());
            let conf = score(parsed_title, &s.name, parsed_year, year);
            (conf, s, year)
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))?;

    if best.0 < MIN_CONFIDENCE {
        return None;
    }
    Some(TmdbSuggestion {
        tmdb_id: best.1.id as i32,
        title: best.1.name,
        year: best.2,
        poster_path: best.1.poster_path,
        overview: best.1.overview,
        confidence: best.0,
    })
}

/// Suggest a TMDB movie for a parsed title+year.
pub async fn suggest_movie(
    tmdb: &TmdbClient,
    parsed_title: &str,
    parsed_year: Option<i32>,
) -> Option<TmdbSuggestion> {
    let results = match tmdb.search_movie(parsed_title, parsed_year).await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!(error = %e, "TMDB movie search failed");
            return None;
        }
    };
    let best = results
        .results
        .into_iter()
        .map(|m| {
            let year = m
                .release_date
                .as_deref()
                .and_then(|d| d.get(..4))
                .and_then(|y| y.parse::<i32>().ok());
            let conf = score(parsed_title, &m.title, parsed_year, year);
            (conf, m, year)
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))?;

    if best.0 < MIN_CONFIDENCE {
        return None;
    }
    Some(TmdbSuggestion {
        tmdb_id: best.1.id as i32,
        title: best.1.title,
        year: best.2,
        poster_path: best.1.poster_path,
        overview: best.1.overview,
        confidence: best.0,
    })
}

/// Populate TMDB suggestions on every pending candidate whose confidence is
/// still zero (i.e. the row was written by the disk scanner but the match
/// pass hasn't run yet). This is what the scheduler's match task calls.
///
/// Skips candidates that don't have a parsed_title (there's nothing to
/// search with). Honours [`MIN_CONFIDENCE`] — low-confidence results are
/// simply left untouched and will retry on the next pass.
pub async fn refresh_pending_suggestions(
    pool: &MySqlPool,
    tmdb: &TmdbClient,
) -> anyhow::Result<u32> {
    let rows: Vec<(i64, String, Option<String>, Option<i32>)> = sqlx::query_as(
        "SELECT id, media_type, parsed_title, parsed_year
         FROM import_candidates
         WHERE status = 'pending' AND confidence = 0.0 AND parsed_title IS NOT NULL
         LIMIT 200",
    )
    .fetch_all(pool)
    .await?;

    let mut updated = 0u32;
    for (id, media_type, parsed_title, parsed_year) in rows {
        let Some(title) = parsed_title else { continue };
        if title.trim().is_empty() {
            continue;
        }
        let suggestion = match media_type.as_str() {
            "series" => suggest_series(tmdb, &title, parsed_year).await,
            "movie" => suggest_movie(tmdb, &title, parsed_year).await,
            _ => None,
        };
        if let Some(s) = suggestion {
            if let Err(e) = ImportCandidate::update_suggestion(
                pool,
                id,
                Some(s.tmdb_id),
                Some(&s.title),
                s.year,
                s.poster_path.as_deref(),
                s.overview.as_deref(),
                s.confidence,
            )
            .await
            {
                tracing::warn!(id, error = %e, "failed to update candidate suggestion");
            } else {
                updated += 1;
            }
        }
    }
    Ok(updated)
}

fn score(parsed: &str, tmdb: &str, parsed_year: Option<i32>, tmdb_year: Option<i32>) -> f32 {
    let title_score = title_similarity(parsed, tmdb);
    let year_bonus = match (parsed_year, tmdb_year) {
        (Some(a), Some(b)) if a == b => 0.15,
        (Some(a), Some(b)) if (a - b).abs() == 1 => 0.05,
        _ => 0.0,
    };
    (title_score * 0.85 + year_bonus).min(1.0)
}

/// Normalized title similarity in `[0.0, 1.0]`. Uses a cheap
/// lowercase + alphanumeric normalisation, then Levenshtein distance
/// scaled by the longer string's length.
fn title_similarity(a: &str, b: &str) -> f32 {
    let na = normalize(a);
    let nb = normalize(b);
    if na.is_empty() || nb.is_empty() {
        return 0.0;
    }
    if na == nb {
        return 1.0;
    }
    let dist = levenshtein(&na, &nb) as f32;
    let max_len = na.len().max(nb.len()) as f32;
    (1.0 - dist / max_len).max(0.0)
}

fn normalize(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() {
                Some(' ')
            } else {
                None
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Textbook Levenshtein. O(m*n) time, O(n) space.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ac) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, bc) in b.iter().enumerate() {
            let cost = if ac == bc { 0 } else { 1 };
            curr[j + 1] = (curr[j] + 1).min(prev[j + 1] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_titles_score_1() {
        assert!((title_similarity("Breaking Bad", "Breaking Bad") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalized_ignores_punctuation() {
        assert!((title_similarity("The Office (US)", "the office us") - 1.0).abs() < 1e-6);
    }

    #[test]
    fn year_bonus_applied() {
        let s = score("Foo", "Foo", Some(2020), Some(2020));
        assert!(s >= 0.95);
    }

    #[test]
    fn year_mismatch_still_high_for_title_match() {
        let s = score("Foo", "Foo", Some(2020), Some(2005));
        assert!(s >= 0.80);
    }

    #[test]
    fn nothing_in_common_scores_low() {
        assert!(title_similarity("abc", "xyzqwerty") < 0.2);
    }
}
