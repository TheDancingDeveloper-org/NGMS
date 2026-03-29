//! Cardigann parity test harness.
//!
//! Spins up a dedicated Prowlarr instance, provisions all public indexers,
//! sends identical queries to both Prowlarr and NGMS's Cardigann engine,
//! and compares results for parity.

mod comparator;
mod prowlarr_api;
mod provisioner;
mod report;

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ngms_cardigann::CardigannEngine;

#[derive(Parser)]
#[command(name = "cardigann-parity", about = "Cardigann engine parity test harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch all Cardigann YAML definitions from Prowlarr's server
    FetchDefinitions {
        /// Directory to store YAML files
        #[arg(short, long, default_value = "definitions")]
        output: PathBuf,
        /// Definition version
        #[arg(short, long, default_value = "11")]
        version: u32,
    },

    /// Validate that all definitions can be parsed
    ValidateDefinitions {
        /// Directory containing YAML files
        #[arg(short, long, default_value = "definitions")]
        dir: PathBuf,
    },

    /// Provision all public indexers in the Prowlarr parity instance
    Provision {
        /// Prowlarr API URL
        #[arg(long, default_value = "http://localhost:9797")]
        prowlarr_url: String,
        /// Prowlarr API key (auto-detected from Docker if not specified)
        #[arg(long)]
        api_key: Option<String>,
        /// Only provision public indexers
        #[arg(long, default_value = "true")]
        public_only: bool,
    },

    /// Run parity tests between Prowlarr and NGMS's Cardigann engine
    Test {
        /// Prowlarr API URL
        #[arg(long, default_value = "http://localhost:9797")]
        prowlarr_url: String,
        /// Prowlarr API key
        #[arg(long)]
        api_key: Option<String>,
        /// Definitions directory
        #[arg(short, long, default_value = "definitions")]
        definitions_dir: PathBuf,
        /// Search queries to test
        #[arg(short, long, default_values_t = vec!["linux".to_string(), "ubuntu".to_string()])]
        queries: Vec<String>,
        /// Max concurrent indexer tests
        #[arg(long, default_value = "3")]
        concurrency: usize,
        /// Output report path
        #[arg(short, long, default_value = "parity-report.md")]
        output: PathBuf,
    },

    /// Record Prowlarr responses for offline replay testing
    Record {
        /// Prowlarr API URL
        #[arg(long, default_value = "http://localhost:9797")]
        prowlarr_url: String,
        /// Prowlarr API key
        #[arg(long)]
        api_key: Option<String>,
        /// Directory to store recorded fixtures
        #[arg(short, long, default_value = "fixtures/recorded")]
        output: PathBuf,
        /// Search query
        #[arg(short, long, default_value = "linux")]
        query: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::FetchDefinitions { output, version } => {
            let count =
                CardigannEngine::fetch_definitions(&output, version).await?;
            println!("Fetched {count} definitions to {}", output.display());
        }

        Command::ValidateDefinitions { dir } => {
            let mut engine = CardigannEngine::new(&dir);
            let count = engine.load_definitions()?;
            println!("Successfully parsed {count} definitions from {}", dir.display());

            // Report any definitions that failed
            let total_files = std::fs::read_dir(&dir)?
                .filter(|e| {
                    e.as_ref()
                        .ok()
                        .is_some_and(|e| {
                            e.path().extension().is_some_and(|ext| ext == "yml" || ext == "yaml")
                        })
                })
                .count();

            let failed = total_files - count;
            if failed > 0 {
                println!("WARNING: {failed} definitions failed to parse");
            } else {
                println!("All definitions parsed successfully!");
            }
        }

        Command::Provision {
            prowlarr_url,
            api_key,
            public_only,
        } => {
            let api_key = match api_key {
                Some(k) => k,
                None => prowlarr_api::detect_api_key("prowlarr-parity").await?,
            };
            let client = prowlarr_api::ProwlarrClient::new(&prowlarr_url, &api_key);

            // Wait for Prowlarr to be ready
            client.wait_ready(Duration::from_secs(60)).await?;

            let count = provisioner::provision_public_indexers(&client).await?;
            println!("Provisioned {count} indexers in Prowlarr");
        }

        Command::Test {
            prowlarr_url,
            api_key,
            definitions_dir,
            queries,
            concurrency,
            output,
        } => {
            let api_key = match api_key {
                Some(k) => k,
                None => prowlarr_api::detect_api_key("prowlarr-parity").await?,
            };
            let client = prowlarr_api::ProwlarrClient::new(&prowlarr_url, &api_key);
            client.wait_ready(Duration::from_secs(30)).await?;

            let mut engine = CardigannEngine::new(&definitions_dir);
            engine.load_definitions()?;

            let results =
                comparator::run_parity_tests(&client, &engine, &queries, concurrency).await?;

            report::write_report(&results, &output)?;
            println!("Parity report written to {}", output.display());

            // Print summary
            report::print_summary(&results);
        }

        Command::Record {
            prowlarr_url,
            api_key,
            output,
            query,
        } => {
            let api_key = match api_key {
                Some(k) => k,
                None => prowlarr_api::detect_api_key("prowlarr-parity").await?,
            };
            let client = prowlarr_api::ProwlarrClient::new(&prowlarr_url, &api_key);
            client.wait_ready(Duration::from_secs(30)).await?;

            let count = comparator::record_responses(&client, &query, &output).await?;
            println!("Recorded {count} indexer responses to {}", output.display());
        }
    }

    Ok(())
}
