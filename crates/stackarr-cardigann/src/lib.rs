//! Cardigann YAML definition engine for StackArr.
//!
//! Interprets Prowlarr-compatible Cardigann YAML indexer definitions,
//! executing searches by building URLs, fetching HTML/JSON responses,
//! and parsing results via CSS selectors / JSON paths.

pub mod categories;
pub mod definition;
pub mod filters;
pub mod search;
pub mod selector;
pub mod template;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::definition::{CardigannDefinition, CardigannMetaDefinition};
use crate::search::CardigannIndexer;

/// Manages loading and caching of Cardigann YAML definitions.
#[derive(Debug, Clone)]
pub struct CardigannEngine {
    /// Directory containing YAML definition files.
    definitions_dir: PathBuf,
    /// Cached parsed definitions, keyed by definition ID.
    definitions: HashMap<String, CardigannDefinition>,
}

impl CardigannEngine {
    /// Create a new engine loading definitions from a directory.
    pub fn new(definitions_dir: impl Into<PathBuf>) -> Self {
        Self {
            definitions_dir: definitions_dir.into(),
            definitions: HashMap::new(),
        }
    }

    /// Load all YAML definitions from the definitions directory.
    pub fn load_definitions(&mut self) -> Result<usize> {
        self.definitions.clear();

        let dir = &self.definitions_dir;
        if !dir.exists() {
            tracing::warn!(path = %dir.display(), "definitions directory does not exist");
            return Ok(0);
        }

        let mut count = 0;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "yml" || e == "yaml") {
                match self.load_definition(&path) {
                    Ok(def) => {
                        self.definitions.insert(def.id.clone(), def);
                        count += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            file = %path.display(),
                            error = %e,
                            "failed to parse Cardigann definition"
                        );
                    }
                }
            }
        }

        tracing::info!(count, "loaded Cardigann definitions");
        Ok(count)
    }

    /// Load a single definition from a YAML file.
    pub fn load_definition(&self, path: &Path) -> Result<CardigannDefinition> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let def: CardigannDefinition = serde_yaml::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(def)
    }

    /// Get a definition by ID.
    pub fn get_definition(&self, id: &str) -> Option<&CardigannDefinition> {
        self.definitions.get(id)
    }

    /// List all loaded definitions.
    pub fn definitions(&self) -> &HashMap<String, CardigannDefinition> {
        &self.definitions
    }

    /// List only public definitions (type = "public").
    pub fn public_definitions(&self) -> Vec<&CardigannDefinition> {
        self.definitions
            .values()
            .filter(|d| d.privacy.as_deref() == Some("public"))
            .collect()
    }

    /// Create a `CardigannIndexer` from a loaded definition.
    pub fn create_indexer(
        &self,
        definition_id: &str,
        config: HashMap<String, String>,
        indexer_id: i64,
    ) -> Result<CardigannIndexer> {
        let def = self
            .definitions
            .get(definition_id)
            .ok_or_else(|| anyhow::anyhow!("unknown definition: {definition_id}"))?;

        CardigannIndexer::new(def.clone(), config, indexer_id)
    }

    /// Fetch the latest definitions from Prowlarr's definition server.
    pub async fn fetch_definitions(definitions_dir: &Path, version: u32) -> Result<usize> {
        let url = format!("https://indexers.prowlarr.com/master/{version}");
        tracing::info!(%url, "fetching Cardigann definition index");

        let client = reqwest::Client::new();
        let index: Vec<CardigannMetaDefinition> = client
            .get(&url)
            .send()
            .await?
            .json()
            .await
            .context("failed to parse definition index")?;

        std::fs::create_dir_all(definitions_dir)?;
        let mut count = 0;

        for meta in &index {
            let def_url = format!("{url}/{}", meta.file);
            match client.get(&def_url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let body = resp.text().await?;
                    let file_path = definitions_dir.join(format!("{}.yml", meta.file));
                    std::fs::write(&file_path, &body)?;
                    count += 1;
                }
                Ok(resp) => {
                    tracing::warn!(
                        file = %meta.file,
                        status = %resp.status(),
                        "failed to fetch definition"
                    );
                }
                Err(e) => {
                    tracing::warn!(file = %meta.file, error = %e, "failed to fetch definition");
                }
            }

            // Small delay to avoid hammering the server
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        tracing::info!(count, total = index.len(), "fetched Cardigann definitions");
        Ok(count)
    }
}
