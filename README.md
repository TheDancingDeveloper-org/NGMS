# StackArr

StackArr is an experimental, unified Rust service intended to replace Sonarr,
Radarr, and Prowlarr while keeping BitTorrent and Usenet download engines in the
same process.

> [!WARNING]
> StackArr is pre-alpha and is not yet usable as an arr replacement. Its native
> API and UI are under active development, and the Sonarr, Radarr, and Prowlarr
> compatibility APIs described below have not been implemented yet.

The source repository and current GHCR package retain the historical `NGMS`
name. The product and Rust crates use `StackArr`. This deliberate split keeps
existing links and image consumers stable while the compatibility work is in
progress.

## Compatibility promise

StackArr will expose wire-compatible Sonarr v3, Radarr v3, and Prowlarr v1
façades over one media-type-aware core. The native `/api/v1` API remains
additive and unchanged. Compatibility is measured from checked-in OpenAPI
specifications, recorded golden responses, and unmodified third-party clients;
it is not inferred from similarly named internal endpoints.

Current status and client-by-client progress are tracked in
[API compatibility](docs/API-COMPATIBILITY.md). The full execution sequence and
objective gates are in the [Unified Arr plan](docs/UNIFIED-ARR-PLAN.md).

## v1 definition of done

- [ ] Overseerr adds a series and a movie, sees them appear, and tracks availability.
- [ ] Bazarr discovers the library and fetches subtitles.
- [ ] Recyclarr syncs a TRaSH config without error.
- [ ] nzb360 connects, browses, and manages the queue, including SignalR updates.
- [ ] Homepage and Homarr widgets show correct counts.
- [ ] A real Sonarr, Radarr, and Prowlarr installation migrates in one command.
- [ ] A single container operates without an external download client.
- [ ] Resident memory under load remains below 150 MiB.

## Development

The backend is a Rust 2024 workspace; the admin and desktop interfaces are
React 19 applications in `ui/` and `client/`.

```bash
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/TESTING.md](docs/TESTING.md)
before starting a change. New behavior is developed test-first from its
authoritative specification.

## Engines and database

The torrent engine is the crates.io `swarmforge` release family, imported under
historical `librtbit` aliases. The Usenet engine is the crates.io `nzb-*`
family. Published engines must never be copied into this repository.

The target application database is MariaDB 11.4 LTS. SQLite remains a read-only
input format for importing existing arr databases and is used independently by
the discovery bootstrap tool.

## Security and license

Report security issues privately as described in [SECURITY.md](SECURITY.md).
StackArr is licensed under [GPL-3.0-only](LICENSE).
