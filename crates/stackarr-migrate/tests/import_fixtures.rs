//! Integration test: import real Sonarr, Radarr, Prowlarr backups.
//!
//! Requires:
//!   - A running Postgres on localhost:5433 (docker compose -f docker/docker-compose.dev.yml up -d)
//!   - Fixture files in test-fixtures/ at the repo root
//!
//! Run with: cargo test -p stackarr-migrate --test import_fixtures -- --ignored --nocapture

use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("test-fixtures")
}

fn has_fixtures() -> bool {
    let dir = fixtures_dir();
    dir.join("sonarr.db").exists()
        && dir.join("radarr.db").exists()
        && dir.join("prowlarr.db").exists()
}

#[tokio::test]
#[ignore = "requires running postgres and test-fixtures"]
async fn test_import_all_fixtures() {
    if !has_fixtures() {
        eprintln!(
            "SKIP: test-fixtures/*.db not found — run from repo root after extracting backups"
        );
        return;
    }

    let db = stackarr_core::test_helpers::TestDb::new().await;
    let dir = fixtures_dir();

    let report = stackarr_migrate::run_migration(
        &db.pool,
        Some(dir.join("sonarr.db").as_path()),
        Some(dir.join("radarr.db").as_path()),
        Some(dir.join("prowlarr.db").as_path()),
        &[],
        false,
    )
    .await
    .expect("migration should succeed");

    eprintln!("{report}");

    assert!(report.series_imported > 0, "should import series");
    assert!(report.movies_imported > 0, "should import movies");
    assert!(report.episodes_imported > 0, "should import episodes");
    assert!(
        report.quality_profiles_imported > 0,
        "should import quality profiles"
    );
    assert!(report.indexers_imported > 0, "should import indexers");

    // Verify data landed in Postgres
    let series_count: (i64,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM series")
        .fetch_one(&db.pool)
        .await
        .expect("count series");
    assert_eq!(series_count.0 as usize, report.series_imported);

    let movie_count: (i64,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM movies")
        .fetch_one(&db.pool)
        .await
        .expect("count movies");
    assert_eq!(movie_count.0 as usize, report.movies_imported);

    let episode_count: (i64,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM episodes")
        .fetch_one(&db.pool)
        .await
        .expect("count episodes");
    assert_eq!(episode_count.0 as usize, report.episodes_imported);

    let tag_count: (i64,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM tags")
        .fetch_one(&db.pool)
        .await
        .expect("count tags");
    assert!(tag_count.0 > 0, "should have tags");

    if !report.warnings.is_empty() {
        eprintln!("Warnings ({}):", report.warnings.len());
        for w in &report.warnings {
            eprintln!("  - {w}");
        }
    }

    db.cleanup().await;
}

#[tokio::test]
#[ignore = "requires running postgres and test-fixtures"]
async fn test_import_sonarr_only() {
    if !has_fixtures() {
        eprintln!("SKIP: test-fixtures/sonarr.db not found");
        return;
    }

    let db = stackarr_core::test_helpers::TestDb::new().await;
    let dir = fixtures_dir();

    let report = stackarr_migrate::run_migration(
        &db.pool,
        Some(dir.join("sonarr.db").as_path()),
        None,
        None,
        &[],
        false,
    )
    .await
    .expect("sonarr-only migration should succeed");

    eprintln!("{report}");
    assert!(report.series_imported > 0);
    assert!(report.episodes_imported > 0);
    assert_eq!(report.movies_imported, 0);

    db.cleanup().await;
}

#[tokio::test]
#[ignore = "requires running postgres and test-fixtures"]
async fn test_import_radarr_only() {
    if !has_fixtures() {
        eprintln!("SKIP: test-fixtures/radarr.db not found");
        return;
    }

    let db = stackarr_core::test_helpers::TestDb::new().await;
    let dir = fixtures_dir();

    let report = stackarr_migrate::run_migration(
        &db.pool,
        None,
        Some(dir.join("radarr.db").as_path()),
        None,
        &[],
        false,
    )
    .await
    .expect("radarr-only migration should succeed");

    eprintln!("{report}");
    assert!(report.movies_imported > 0);
    assert_eq!(report.series_imported, 0);

    db.cleanup().await;
}

#[tokio::test]
#[ignore = "requires running postgres and test-fixtures"]
async fn test_dry_run() {
    if !has_fixtures() {
        eprintln!("SKIP: test-fixtures not found");
        return;
    }

    let db = stackarr_core::test_helpers::TestDb::new().await;
    let dir = fixtures_dir();

    let report = stackarr_migrate::run_migration(
        &db.pool,
        Some(dir.join("sonarr.db").as_path()),
        Some(dir.join("radarr.db").as_path()),
        Some(dir.join("prowlarr.db").as_path()),
        &[],
        true,
    )
    .await
    .expect("dry run should succeed");

    assert!(report.dry_run);
    assert!(report.series_imported > 0, "dry run should count series");

    // Verify nothing was written
    let count: (i64,) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM series")
        .fetch_one(&db.pool)
        .await
        .expect("count");
    assert_eq!(count.0, 0, "dry run should not write data");

    db.cleanup().await;
}
