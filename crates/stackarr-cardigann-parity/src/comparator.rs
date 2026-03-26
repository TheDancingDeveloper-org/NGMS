//! Compare Prowlarr search results with StackArr Cardigann engine results.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use stackarr_cardigann::search::{CardigannIndexer, CardigannRelease, SearchQuery, SearchType};
use stackarr_cardigann::CardigannEngine;

use crate::prowlarr_api::{ProwlarrClient, ProwlarrRelease};

/// Result of a parity test for one indexer + one query.
#[derive(Debug, Clone)]
pub struct ParityResult {
    pub indexer_name: String,
    pub indexer_id: String,
    pub query: String,
    pub prowlarr_count: usize,
    pub stackarr_count: usize,
    pub matched_titles: usize,
    pub prowlarr_only: Vec<String>,
    pub stackarr_only: Vec<String>,
    pub size_mismatches: Vec<SizeMismatch>,
    pub parity_pct: f64,
    pub prowlarr_error: Option<String>,
    pub stackarr_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SizeMismatch {
    pub title: String,
    pub prowlarr_size: i64,
    pub stackarr_size: i64,
}

/// Run parity tests across all configured indexers.
pub async fn run_parity_tests(
    prowlarr: &ProwlarrClient,
    engine: &CardigannEngine,
    queries: &[String],
    concurrency: usize,
) -> Result<Vec<ParityResult>> {
    let indexers = prowlarr.list_indexers().await?;
    tracing::info!(count = indexers.len(), "found Prowlarr indexers");

    let mut results = Vec::new();

    // Process indexers with limited concurrency
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));

    for indexer in &indexers {
        if !indexer.enable {
            continue;
        }

        // Find the matching Cardigann definition
        let definition_file = indexer
            .fields
            .iter()
            .find(|f| f.name == "definitionFile")
            .and_then(|f| f.value.as_ref())
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let def = engine.get_definition(definition_file);
        if def.is_none() {
            tracing::debug!(
                name = %indexer.name,
                definition = definition_file,
                "no matching Cardigann definition, skipping"
            );
            continue;
        }

        for query in queries {
            let permit = semaphore.clone().acquire_owned().await?;

            let prowlarr = prowlarr.clone();
            let indexer_id = indexer.id;
            let indexer_name = indexer.name.clone();
            let def_id = definition_file.to_owned();
            let query_str = query.clone();
            let engine_def = def.unwrap().clone();

            // Search Prowlarr
            let prowlarr_result = prowlarr.search_indexer(indexer_id, &query_str).await;

            // Search StackArr Cardigann engine
            let stackarr_result = {
                let config = extract_config_from_prowlarr(&indexer.fields);
                match CardigannIndexer::new(engine_def, config, indexer_id) {
                    Ok(cardigann) => {
                        let sq = SearchQuery {
                            query: query_str.clone(),
                            ..Default::default()
                        };
                        cardigann.search(&sq).await
                    }
                    Err(e) => Err(e),
                }
            };

            let result = compare_results(
                &indexer_name,
                &def_id,
                &query_str,
                prowlarr_result,
                stackarr_result,
            );

            results.push(result);
            drop(permit);
        }
    }

    Ok(results)
}

/// Compare two sets of results.
fn compare_results(
    indexer_name: &str,
    indexer_id: &str,
    query: &str,
    prowlarr: Result<Vec<ProwlarrRelease>>,
    stackarr: Result<Vec<CardigannRelease>>,
) -> ParityResult {
    let (prowlarr_releases, prowlarr_error) = match prowlarr {
        Ok(r) => (r, None),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };

    let (stackarr_releases, stackarr_error) = match stackarr {
        Ok(r) => (r, None),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };

    // Normalize titles for comparison
    let prowlarr_titles: HashMap<String, &ProwlarrRelease> = prowlarr_releases
        .iter()
        .filter_map(|r| r.title.as_ref().map(|t| (normalize_title(t), r)))
        .collect();

    let stackarr_titles: HashMap<String, &CardigannRelease> =
        stackarr_releases
            .iter()
            .map(|r| (normalize_title(&r.title), r))
            .collect();

    // Find matches and mismatches
    let mut matched = 0;
    let mut size_mismatches = Vec::new();
    let mut prowlarr_only = Vec::new();
    let mut stackarr_only = Vec::new();

    for (norm_title, pr) in &prowlarr_titles {
        if let Some(sr) = stackarr_titles.get(norm_title) {
            matched += 1;
            // Check size parity (within 5% tolerance)
            let ps = pr.size.unwrap_or(0);
            let ss = sr.size;
            if ps > 0 && ss > 0 {
                let diff = (ps - ss).unsigned_abs() as f64;
                let max = ps.max(ss) as f64;
                if diff / max > 0.05 {
                    size_mismatches.push(SizeMismatch {
                        title: pr.title.clone().unwrap_or_default(),
                        prowlarr_size: ps,
                        stackarr_size: ss,
                    });
                }
            }
        } else {
            prowlarr_only.push(pr.title.clone().unwrap_or_default());
        }
    }

    for (norm_title, sr) in &stackarr_titles {
        if !prowlarr_titles.contains_key(norm_title) {
            stackarr_only.push(sr.title.clone());
        }
    }

    let total = prowlarr_releases.len().max(stackarr_releases.len());
    let parity_pct = if total == 0 {
        100.0
    } else {
        (matched as f64 / total as f64) * 100.0
    };

    ParityResult {
        indexer_name: indexer_name.to_owned(),
        indexer_id: indexer_id.to_owned(),
        query: query.to_owned(),
        prowlarr_count: prowlarr_releases.len(),
        stackarr_count: stackarr_releases.len(),
        matched_titles: matched,
        prowlarr_only,
        stackarr_only,
        size_mismatches,
        parity_pct,
        prowlarr_error,
        stackarr_error,
    }
}

/// Record raw Prowlarr responses for offline replay.
pub async fn record_responses(
    prowlarr: &ProwlarrClient,
    query: &str,
    output_dir: &Path,
) -> Result<usize> {
    std::fs::create_dir_all(output_dir)?;

    let indexers = prowlarr.list_indexers().await?;
    let mut count = 0;

    for indexer in &indexers {
        if !indexer.enable {
            continue;
        }

        match prowlarr.search_indexer(indexer.id, query).await {
            Ok(results) => {
                let fixture = serde_json::json!({
                    "indexer_id": indexer.id,
                    "indexer_name": indexer.name,
                    "implementation": indexer.implementation,
                    "query": query,
                    "results": results,
                });

                let filename = format!("{}_{}.json", indexer.id, sanitize_filename(&indexer.name));
                let path = output_dir.join(filename);
                std::fs::write(&path, serde_json::to_string_pretty(&fixture)?)?;
                count += 1;
                tracing::info!(name = %indexer.name, results = results.len(), "recorded");
            }
            Err(e) => {
                tracing::warn!(name = %indexer.name, error = %e, "failed to search");
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    Ok(count)
}

/// Normalize a title for fuzzy comparison.
fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .replace(['.', '-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract config map from Prowlarr indexer fields.
fn extract_config_from_prowlarr(
    fields: &[crate::prowlarr_api::ProwlarrField],
) -> HashMap<String, String> {
    let mut config = HashMap::new();
    for field in fields {
        if let Some(ref value) = field.value {
            let val = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => continue,
            };
            config.insert(field.name.clone(), val);
        }
    }
    config
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}
