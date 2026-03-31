//! Auto-provision all public Cardigann indexers in a Prowlarr instance.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::prowlarr_api::{CreateIndexerPayload, ProwlarrClient, ProwlarrField};

/// Metadata from the Prowlarr definition index.
#[derive(Debug, Deserialize)]
struct DefinitionMeta {
    id: String,
    file: String,
    name: String,
    #[serde(rename = "type")]
    privacy: Option<String>,
    implementation: Option<String>,
    links: Option<Vec<String>>,
}

/// Provision all public indexers from Prowlarr's definition server.
pub async fn provision_public_indexers(client: &ProwlarrClient) -> Result<usize> {
    // Fetch the definition index
    let defs = fetch_definition_index().await?;
    tracing::info!(total = defs.len(), "fetched definition index");

    // Filter to public only
    let public: Vec<_> = defs
        .iter()
        .filter(|d| d.privacy.as_deref() == Some("public"))
        .collect();

    tracing::info!(count = public.len(), "public indexers to provision");

    // Get existing indexers to avoid duplicates
    let existing = client.list_indexers().await?;
    let existing_names: Vec<String> = existing.iter().map(|i| i.name.clone()).collect();

    let mut added = 0;

    for def in &public {
        if existing_names.contains(&def.name) {
            tracing::debug!(name = %def.name, "indexer already exists, skipping");
            continue;
        }

        let base_url = def
            .links
            .as_ref()
            .and_then(|l| l.first())
            .cloned()
            .unwrap_or_default();

        if base_url.is_empty() {
            tracing::warn!(name = %def.name, "no base URL, skipping");
            continue;
        }

        let implementation = def
            .implementation
            .as_deref()
            .unwrap_or("cardigann");

        // Determine config contract and fields based on implementation
        let (config_contract, fields) = match implementation {
            "cardigann" => (
                "CardigannSettings",
                vec![
                    ProwlarrField {
                        name: "definitionFile".into(),
                        value: Some(serde_json::Value::String(def.file.clone())),
                    },
                    ProwlarrField {
                        name: "baseUrl".into(),
                        value: Some(serde_json::Value::String(base_url)),
                    },
                ],
            ),
            other => {
                // Native implementations use their own config contract
                let contract = format!("{other}Settings");
                let fields = vec![ProwlarrField {
                    name: "baseUrl".into(),
                    value: Some(serde_json::Value::String(base_url)),
                }];
                (contract.as_str().to_owned().leak() as &str, fields)
            }
        };

        let payload = CreateIndexerPayload {
            name: def.name.clone(),
            implementation: capitalize_first(implementation),
            implementation_name: capitalize_first(implementation),
            config_contract: config_contract.to_owned(),
            fields,
            enable: true,
            app_profile_id: Some(1),
        };

        match client.add_indexer(&payload).await {
            Ok(indexer) => {
                tracing::info!(
                    name = %def.name,
                    id = indexer.id,
                    "provisioned indexer"
                );
                added += 1;
            }
            Err(e) => {
                tracing::warn!(name = %def.name, error = %e, "failed to provision indexer");
            }
        }

        // Small delay to avoid overwhelming Prowlarr
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(added)
}

/// Fetch the definition index from Prowlarr's server.
async fn fetch_definition_index() -> Result<Vec<DefinitionMeta>> {
    let url = "https://indexers.prowlarr.com/master/11";
    let client = reqwest::Client::new();
    let defs: Vec<DefinitionMeta> = client
        .get(url)
        .send()
        .await?
        .json()
        .await
        .context("failed to parse definition index")?;
    Ok(defs)
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
