# Database

MariaDB 11.4 LTS is required, reached through the `sqlx` MySQL driver. SQLite is
used only for *reading* arr migration databases (`rusqlite` in
`stackarr-migrate`).

StackArr deploys fresh. There is no upgrade path from the pre-MariaDB schema and
none is planned, which is what makes the single baseline below possible.

## Connection

```rust
// stackarr-core/src/db.rs
pub struct Database { pool: MySqlPool }

impl Database {
    pub async fn connect(config: &DatabaseConfig) -> Result<Self>
    pub fn pool(&self) -> &MySqlPool
    pub async fn run_migrations(&self) -> Result<()>
    pub async fn is_first_boot(&self) -> Result<bool>
    pub async fn load_enabled_modules(&self) -> Result<EnabledModules>
    pub async fn save_enabled_modules(&self, modules: &EnabledModules) -> Result<()>
}
```

Connection string format: `mysql://user:pass@host:port/dbname`
Default max connections: 20 (configurable in the `[database]` section).

Bring a server up locally with:

```bash
docker compose -f docker/docker-compose.dev.yml up -d
```

That publishes MariaDB on `127.0.0.1:3306` with user/password `stackarr`, which
is what `TEST_DATABASE_URL` defaults to.

## Schema

One file: `migrations/001_baseline.sql` — 60 tables, 907 lines. The 18
incremental migrations that preceded it were collapsed into it and deleted.

Do not add a `002_`. Until the first tagged release, schema changes are edits to
the baseline; after it, the migration chain restarts from the shipped schema.

### Table groups

| Group | Tables |
|---|---|
| Configuration | `app_config`, `enabled_modules`, `naming_config`, `media_library_folders`, `tags` |
| Generic media core | `media_entities`, `media_files` |
| TV adapter | `series`, `seasons`, `episodes`, `episode_files` |
| Film adapter | `movies`, `alternative_titles` |
| Quality and formats | `quality_profiles`, `custom_formats`, `custom_format_scores` |
| Indexers and downloads | `indexers`, `download_clients`, `queue`, `history`, `blocklist` |
| Import | `import_candidates`, `recycle_bin` |
| Integrations | `notification_providers`, `import_lists`, `plex_servers`, `plex_libraries`, `plex_events`, `discover_sliders` |
| Streaming | `streaming_sessions`, `watchlist`, `remote_clients` |
| Users | `users`, `user_sessions`, `user_devices`, `invites`, `watch_progress`, `media_requests`, `user_watchlist`, `user_ratings`, `user_notifications`, `push_subscriptions` |
| RSS | `rss_feeds`, `rss_items`, `rss_rules` |
| WebDAV | `dav_items`, `dav_blobs`, `dav_nzb_blobs`, `dav_queue_items`, `dav_history_items`, `dav_health_checks`, `dav_config` |
| System | `system_activities` |
| **P5 — profiles** | `profile_sources`, `profile_subscriptions`, `profile_snapshots`, `profile_overrides`, `custom_format_provenance` |
| **P6 — decisions** | `decision_records`, `decision_steps` |

### Why the last two groups exist now

The baseline was designed, not translated. Three structures were brought
forward because adding them later costs a migration chain and a backfill, and
adding them now costs nothing:

- **`media_entities` / `media_files`** — the media-type-generic core from §5 of
  `UNIFIED-ARR-PLAN.md`. `series` and `movies` are adapters keyed off a shared
  identity (`UNIQUE (media_type, source_key)`), rather than two parallel
  hierarchies. Music and books become rows with a new `media_type`, not forks.
- **P5 profile tables** — `profile_subscriptions` keeps `base_document` beside
  the live profile so an upstream TRaSH update can be **three-way merged**
  against local edits instead of clobbering them. `profile_snapshots` are
  immutable; `profile_overrides` and `custom_format_provenance` record what was
  changed locally and where it came from.
- **P6 decision tables** — `decision_records` stores the full input and outcome
  of a grab decision, `decision_steps` the ordered per-specification breakdown.
  This is what makes "why didn't it grab this?" answerable and replayable.

None of these have code behind them yet. They are schema-only until P5 and P6.

## MariaDB conventions

These are the rules the baseline follows. Match them in any new table.

| Concern | Rule |
|---|---|
| Engine | `ENGINE=InnoDB` on every table |
| Charset | `utf8mb4` / `utf8mb4_unicode_ci` |
| Timestamps | `DATETIME(6)`, UTC, `DEFAULT CURRENT_TIMESTAMP(6)` |
| Identity | `AUTO_INCREMENT`, `INT` for config-scale tables and `BIGINT` for row-growth tables |
| Structured columns | `JSON` — replaces both PostgreSQL `jsonb` and PostgreSQL arrays |
| Booleans | `BOOLEAN` (MariaDB stores as `TINYINT(1)`) |
| Placeholders | `?` — MySQL protocol has no `$n` |
| Indexes | declared inline as `KEY` / `UNIQUE KEY`, not as trailing `CREATE INDEX` |
| Long strings in keys | prefix length, e.g. `path(768)`, since InnoDB caps index entries at 3072 bytes |

### Dialect differences that changed query code

| PostgreSQL | MariaDB |
|---|---|
| `$1, $2, …` | `?, ?, …` |
| `INSERT … RETURNING id` | `INSERT …` then `LAST_INSERT_ID()` |
| `INSERT … ON CONFLICT (k) DO UPDATE` | `INSERT … ON DUPLICATE KEY UPDATE` |
| `INSERT … ON CONFLICT DO NOTHING` | `INSERT IGNORE` |
| `jsonb` | `JSON` |
| `TEXT[]` | `JSON` array |
| `PgPool` | `MySqlPool` |

The workspace uses **no** `sqlx::query!` compile-time macros, so none of this is
checked against a live server at build time. The database-backed tests are the
only thing that verifies it.

## Testing

`stackarr-core`'s `TestDb` harness creates a randomly named database per test
(`stackarr_test_<uuid>`), runs the baseline into it, and drops it afterwards.
The account in `TEST_DATABASE_URL` therefore needs server-wide `CREATE`/`DROP`,
not just rights on one schema — `docker/mariadb-init/01-test-grants.sql` grants
that for the dev stack, and CI uses the service container's root account.

Database-backed tests are marked `#[ignore]` so the suite still runs on a
machine with no server:

```bash
cargo test --workspace --all-features              # 1,010 tests, no server needed
cargo test --workspace --all-features -- --ignored # 31 tests, needs MariaDB
```

CI runs both; the second is the only gate that proves the swap against a real
server.

## Index parity with the pre-MariaDB schema

The 18 old migrations declared 60 named indexes; the baseline declares 55 by
name. The eight names that disappeared are all still covered, so the difference
is naming and idiom rather than lost coverage:

| Old index | Covered in the baseline by |
|---|---|
| `idx_invites_code` | `code … UNIQUE` |
| `idx_remote_clients_token` | `client_token … UNIQUE` |
| `idx_user_devices_token` | `device_token … UNIQUE` |
| `idx_user_devices_user` | FK `fk_user_devices_user` (InnoDB indexes every FK) |
| `idx_push_subscriptions_user` | FK `fk_push_subscriptions_user` |
| `idx_user_ratings_user` | leftmost prefix of `uq_user_rating (user_id, …)` |
| `idx_episode_files_episode` | leftmost prefix of `PRIMARY KEY (episode_id, media_file_id)` |
| `idx_import_candidates_pending_path` | renamed to `uq_import_candidates_pending_path` |

## Seeded data

The baseline seeds the rows the application assumes exist on first boot: default
quality profiles and definitions, `naming_config` rows for TV and film, the
built-in `discover_sliders`, recycle-bin config keys, and the three root
`dav_items`. First boot is detected by `is_first_boot()`, not by a seed marker.
