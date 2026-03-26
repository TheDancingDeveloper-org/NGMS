pub mod prowlarr;
pub mod radarr;
pub mod sonarr;
pub mod writer;

pub use writer::{MigrationData, MigrationReport, MigrationWriter};

use tracing::info;

/// Run a complete migration from *arr SQLite databases to StackArr Postgres.
///
/// Provide `None` for any database you don't want to import.
/// When `dry_run` is true, all data is read and merged but nothing is written to Postgres.
pub async fn run_migration(
    pool: &sqlx::PgPool,
    sonarr_db: Option<&std::path::Path>,
    radarr_db: Option<&std::path::Path>,
    prowlarr_db: Option<&std::path::Path>,
    dry_run: bool,
) -> anyhow::Result<MigrationReport> {
    // 1. Read from each provided DB (blocking I/O, run on spawn_blocking)
    let sonarr_data = match sonarr_db {
        Some(path) => {
            info!("reading Sonarr database: {}", path.display());
            let p = path.to_path_buf();
            let data =
                tokio::task::spawn_blocking(move || sonarr::read_sonarr(&p)).await??;
            Some(data)
        }
        None => None,
    };

    let radarr_data = match radarr_db {
        Some(path) => {
            info!("reading Radarr database: {}", path.display());
            let p = path.to_path_buf();
            let data =
                tokio::task::spawn_blocking(move || radarr::read_radarr(&p)).await??;
            Some(data)
        }
        None => None,
    };

    let prowlarr_data = match prowlarr_db {
        Some(path) => {
            info!("reading Prowlarr database: {}", path.display());
            let p = path.to_path_buf();
            let data =
                tokio::task::spawn_blocking(move || prowlarr::read_prowlarr(&p)).await??;
            Some(data)
        }
        None => None,
    };

    // 2. Merge into MigrationData (with dedup)
    let (data, merge_warnings) = writer::build_migration_data(
        sonarr_data.as_ref(),
        radarr_data.as_ref(),
        prowlarr_data.as_ref(),
    );

    info!(
        "merged migration data: {} series, {} movies, {} episodes, {} media files, {} profiles, {} indexers, {} download clients",
        data.series.len(),
        data.movies.len(),
        data.episodes.len(),
        data.media_files.len(),
        data.quality_profiles.len(),
        data.indexers.len(),
        data.download_clients.len(),
    );

    // 3. Dry-run: count everything and return report without writing
    if dry_run {
        info!("dry run mode -- no data will be written to Postgres");
        return Ok(MigrationReport {
            series_imported: data.series.len(),
            movies_imported: data.movies.len(),
            episodes_imported: data.episodes.len(),
            media_files_imported: data.media_files.len(),
            quality_profiles_imported: data.quality_profiles.len(),
            indexers_imported: data.indexers.len(),
            download_clients_imported: data.download_clients.len(),
            history_events_imported: data.history.len(),
            blocklist_entries_imported: data.blocklist.len(),
            warnings: merge_warnings,
            dry_run: true,
        });
    }

    // 4. Write to Postgres
    let writer = MigrationWriter::new(pool.clone());
    let mut report = writer.write_all(data).await?;
    report.warnings.extend(merge_warnings);

    info!("{report}");

    Ok(report)
}
