# StackArr repository guidance

This is the canonical guidance for work in this repository. `CLAUDE.md` is a
compatibility pointer to this file.

## Product and source

- Product and crate family: **StackArr**.
- Canonical source, issues, and CI: `TheDancingDeveloper-org/NGMS` on GitHub.
- Repository and GHCR path retain `NGMS` for compatibility until an explicitly
  approved rename migration.
- Target database: MariaDB 11.4 LTS through the `sqlx` MySQL driver.
- License: GPL-3.0-only.

Read `README.md`, `CONTRIBUTING.md`, `docs/UNIFIED-ARR-PLAN.md`, and the issue
being implemented before changing behavior.

`docs/UNIFIED-ARR-PLAN.md` is the only active roadmap and owns future state,
phase order, and architecture decisions. Topic documents describe the current
implementation unless they explicitly identify a target state. `PLAN.md`,
`IMPLEMENTATION_PLAN.md`, `TODO3.md`, and the client phase documents are not
backlogs.

## Workspace

The following list is checked against `Cargo.toml` in CI.

<!-- workspace-members:start -->
- `crates/stackarr-core`
- `crates/stackarr-media`
- `crates/stackarr-parser`
- `crates/stackarr-quality`
- `crates/stackarr-indexer`
- `crates/stackarr-download`
- `crates/stackarr-import`
- `crates/stackarr-scheduler`
- `crates/stackarr-metadata`
- `crates/stackarr-web`
- `crates/stackarr-notify`
- `crates/stackarr-migrate`
- `crates/stackarr-plex`
- `crates/stackarr-cardigann`
- `crates/stackarr-cardigann-parity`
- `crates/stackarr-stream`
- `crates/stackarr-postgres`
<!-- workspace-members:end -->

The final entry is renamed to `stackarr-mariadb` during P1. Update this list in
the same commit as any workspace-member change.

The torrent engine is consumed from crates.io through the `swarmforge` package
family and historical `librtbit` dependency aliases. The Usenet engine is the
published crates.io `nzb-*` family. There is no live vendored engine directory.
Never copy a published engine into `crates/`.

## Scope boundary

Core work covers embedded engines, Cardigann/indexers, migration, the unified
media domain, arr façades, TRaSH/Profilarr profiles, and decision/parser parity.
Until P5 is complete, `stackarr-stream`, Stremio routes, and
`stackarr-bootstrap` are frozen except for maintenance and tests. Discovery,
trending, requests, and watchlist features are deferred.

Compatibility crates contain DTOs, route wiring, and translation only. Put
business rules in the core.

## Required workflow

- Develop new behavior test-first from the governing specification.
- Preserve the native `/api/v1` contract; arr façades are additive.
- Add unit tests for logic and integration tests for I/O boundaries. Add GUI/E2E
  coverage for user-visible flows.
- Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-features --
  -D warnings`, and `cargo test --workspace --all-features` before committing.
- Do not add an undocumented `#[allow]`.
- Do not vendor published crates or add private dependency sources.
- Do not add AI co-author trailers or list AI systems as contributors.
