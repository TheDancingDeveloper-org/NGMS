pub mod prowlarr;
pub mod radarr;
pub mod sonarr;
pub mod writer;

pub use writer::{MigrationData, MigrationReport, MigrationWriter};

use tracing::info;

/// A path prefix mapping: replace `from` with `to` in all imported paths.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PathMapping {
    pub from: String,
    pub to: String,
}

/// Apply path mappings to a string, replacing the first matching `from` prefix with `to`.
fn remap_path(path: &mut String, mappings: &[PathMapping]) {
    for m in mappings {
        if path.starts_with(&m.from) {
            *path = format!("{}{}", m.to, &path[m.from.len()..]);
            return;
        }
    }
}

/// Run a complete migration from *arr SQLite databases to StackArr Postgres.
///
/// Provide `None` for any database you don't want to import.
/// `path_mappings` remaps imported paths (root folders, series/movie directories)
/// from the old *arr container mounts to StackArr's mounts.
/// When `dry_run` is true, all data is read and merged but nothing is written to Postgres.
pub async fn run_migration(
    pool: &sqlx::MySqlPool,
    sonarr_db: Option<&std::path::Path>,
    radarr_db: Option<&std::path::Path>,
    prowlarr_db: Option<&std::path::Path>,
    path_mappings: &[PathMapping],
    dry_run: bool,
) -> anyhow::Result<MigrationReport> {
    // 1. Read from each provided DB (blocking I/O, run on spawn_blocking)
    let sonarr_data = match sonarr_db {
        Some(path) => {
            info!("reading Sonarr database: {}", path.display());
            let p = path.to_path_buf();
            let data = tokio::task::spawn_blocking(move || sonarr::read_sonarr(&p)).await??;
            Some(data)
        }
        None => None,
    };

    let radarr_data = match radarr_db {
        Some(path) => {
            info!("reading Radarr database: {}", path.display());
            let p = path.to_path_buf();
            let data = tokio::task::spawn_blocking(move || radarr::read_radarr(&p)).await??;
            Some(data)
        }
        None => None,
    };

    let prowlarr_data = match prowlarr_db {
        Some(path) => {
            info!("reading Prowlarr database: {}", path.display());
            let p = path.to_path_buf();
            let data = tokio::task::spawn_blocking(move || prowlarr::read_prowlarr(&p)).await??;
            Some(data)
        }
        None => None,
    };

    // 2. Merge into MigrationData (with dedup)
    let (mut data, merge_warnings) = writer::build_migration_data(
        sonarr_data.as_ref(),
        radarr_data.as_ref(),
        prowlarr_data.as_ref(),
    );

    // 3. Apply path mappings
    if !path_mappings.is_empty() {
        let mut remapped = 0usize;
        for f in &mut data.media_library_folders {
            let before = f.path.clone();
            remap_path(&mut f.path, path_mappings);
            if f.path != before {
                info!(from = %before, to = %f.path, "remapped library folder path");
                remapped += 1;
            }
        }
        for s in &mut data.series {
            let before = s.path.clone();
            remap_path(&mut s.path, path_mappings);
            if s.path != before {
                remapped += 1;
            }
        }
        for m in &mut data.movies {
            let before = m.path.clone();
            remap_path(&mut m.path, path_mappings);
            if m.path != before {
                remapped += 1;
            }
        }
        info!(remapped, "applied path mappings");
    }

    let format_scores_count: usize = data
        .quality_profiles
        .iter()
        .map(|p| p.format_scores.len())
        .sum();

    info!(
        "merged migration data: {} series, {} movies, {} episodes, {} media files, {} profiles, {} custom formats, {} format scores, {} indexers, {} download clients",
        data.series.len(),
        data.movies.len(),
        data.episodes.len(),
        data.media_files.len(),
        data.quality_profiles.len(),
        data.custom_formats.len(),
        format_scores_count,
        data.indexers.len(),
        data.download_clients.len(),
    );

    // 4. Dry-run: count everything and return report without writing
    if dry_run {
        info!("dry run mode -- no data will be written to Postgres");
        return Ok(MigrationReport {
            series_imported: data.series.len(),
            movies_imported: data.movies.len(),
            episodes_imported: data.episodes.len(),
            media_files_imported: data.media_files.len(),
            quality_profiles_imported: data.quality_profiles.len(),
            custom_formats_imported: data.custom_formats.len(),
            format_scores_imported: format_scores_count,
            indexers_imported: data.indexers.len(),
            download_clients_imported: data.download_clients.len(),
            history_events_imported: data.history.len(),
            blocklist_entries_imported: data.blocklist.len(),
            warnings: merge_warnings,
            dry_run: true,
        });
    }

    // 5. Write to Postgres
    let writer = MigrationWriter::new(pool.clone());
    let mut report = writer.write_all(data).await?;
    report.warnings.extend(merge_warnings);

    info!("{report}");

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_path_replaces_prefix() {
        let mappings = vec![
            PathMapping {
                from: "/mnt/movies1/".into(),
                to: "/media/Movies1/".into(),
            },
            PathMapping {
                from: "/TV1/".into(),
                to: "/media/TV1/".into(),
            },
        ];

        let mut p = "/mnt/movies1/Some Movie (2020)".to_string();
        remap_path(&mut p, &mappings);
        assert_eq!(p, "/media/Movies1/Some Movie (2020)");

        let mut p = "/TV1/Some Show (2019)".to_string();
        remap_path(&mut p, &mappings);
        assert_eq!(p, "/media/TV1/Some Show (2019)");
    }

    #[test]
    fn remap_path_no_match_unchanged() {
        let mappings = vec![PathMapping {
            from: "/mnt/movies1/".into(),
            to: "/media/Movies1/".into(),
        }];

        let mut p = "/other/path".to_string();
        remap_path(&mut p, &mappings);
        assert_eq!(p, "/other/path");
    }

    #[test]
    fn remap_path_first_match_wins() {
        let mappings = vec![
            PathMapping {
                from: "/mnt/".into(),
                to: "/short/".into(),
            },
            PathMapping {
                from: "/mnt/movies1/".into(),
                to: "/long/".into(),
            },
        ];

        let mut p = "/mnt/movies1/foo".to_string();
        remap_path(&mut p, &mappings);
        assert_eq!(p, "/short/movies1/foo");
    }

    #[test]
    fn remap_path_empty_mappings() {
        let mut p = "/some/path".to_string();
        remap_path(&mut p, &[]);
        assert_eq!(p, "/some/path");
    }
}
