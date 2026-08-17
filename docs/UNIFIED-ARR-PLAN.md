# StackArr product plan and target state

**Status:** Canonical and in execution

**Baseline:** `TheDancingDeveloper-org/NGMS@dd26a0aa`

**Issue inventory:** 54 open issues, reviewed 2026-08-05

**Owner:** TheDancingDeveloper-org

This document is the single authority for product scope, architectural decisions,
phase order, and completion gates. GitHub issues describe bounded implementation
work; they do not override this plan. Detailed documents describe the current
implementation unless they explicitly say “target state.” `PLAN.md`,
`IMPLEMENTATION_PLAN.md`, `TODO3.md`, and the client phase documents are historical
references, not competing roadmaps.

There are no unresolved product or architecture questions in this plan. New
information can change a decision through a pull request that updates this document,
the affected issue, and its tests together.

## 1. Product outcome

StackArr will be one self-hosted Rust service that replaces Sonarr, Radarr, and
Prowlarr over a shared media domain. It keeps the native `/api/v1` API and admin UI,
adds wire-compatible arr façades, embeds BitTorrent and Usenet engines, imports an
existing arr installation, and manages TRaSH/Profilarr profiles as native data.

The repository and image remain `TheDancingDeveloper-org/NGMS` and
`ghcr.io/thedancingdeveloper-org/ngms`. The product and Rust crate family are named
StackArr. The license is GPL-3.0-only.

### v1 completion contract

v1 is complete only when all of the following are demonstrated with released
artifacts and unmodified clients:

- Overseerr connects to logical Sonarr and Radarr façades, adds a series and a
  movie, and observes availability changes.
- Bazarr discovers both libraries and completes a subtitle workflow.
- Recyclarr reads and writes quality definitions, quality profiles, and custom
  formats without contract errors.
- nzb360 connects, searches, mutates the queue, triggers commands, and receives
  SignalR updates.
- Homepage and Homarr show correct health, media, and queue counts.
- One command imports real Sonarr, Radarr, Prowlarr, and SABnzbd configuration and
  data, with a dry-run report and no source mutation.
- A stock Sonarr can use StackArr through its qBittorrent-compatible and
  SABnzbd-compatible download-client endpoints.
- The standard image works with external MariaDB 11.4; the standalone image starts
  StackArr and a private MariaDB 11.4 service from one container and one persistent
  `/config` volume.
- The conformance suite is green for the declared endpoint set, the MariaDB suite is
  green against a live service, and line coverage does not regress below the
  committed per-crate baseline.
- Resident memory remains below 150 MiB during the documented mixed TV/film search,
  grab, and queue workload. MariaDB is measured and reported separately so the
  application budget is not obscured by deployment mode.

## 2. Scope and source hierarchy

### In v1

- unified TV and film library management;
- Cardigann, Newznab, Torznab, and the optional Indexarr adapter;
- embedded `swarmforge` BitTorrent and published `nzb-*` Usenet engines;
- external download-client support and legacy qBittorrent/SABnzbd façades;
- Sonarr v3, Radarr v3, and Prowlarr v1 compatibility;
- arr/SABnzbd migration;
- quality definitions, custom formats, TRaSH subscriptions, three-way merge,
  provenance, and profile-change simulation;
- parser, decision-engine, scene-numbering, and XEM parity; and
- unified indexer planning, resource budgets, and declarative providers.

### Preserved but frozen through P5

`stackarr-stream`, Stremio routes, bootstrap discovery, and the existing client-app
features continue to compile and receive security, data-integrity, compatibility,
and test maintenance. They receive no new product behavior before P5 exits.

### Deferred beyond v1

Discovery, trending, requests, watchlist, ratings, PWA expansion, music, books, and
ownership of a media-streaming server are not v1 roadmap work. Existing code and
native `/api/v1` routes are preserved; the deferral prevents further expansion.
Jellyfin/Plex integrations remain integrations, not capabilities StackArr attempts to
replace.

### Governing sources

Use these in descending order:

1. checked-in, hash-pinned OpenAPI contracts and reviewed golden captures for wire
   behavior;
2. upstream arr behavioral tests for domain behavior, ported as tests without copying
   implementation code;
3. TRaSH Guides data and the ProfSync questionnaire for profile behavior;
4. this plan and the native `/api/v1` contract for StackArr-specific behavior; and
5. a GitHub issue for bounded acceptance criteria.

When sources disagree, the higher source wins and the lower artifact is corrected in
the same change.

## 3. Settled decisions

| Area | Ruling |
| --- | --- |
| Product/source naming | StackArr is the product and crate family. NGMS remains the repository and GHCR path until an explicit rename migration. |
| License | GPL-3.0-only. Closed-source redistribution is not a product direction. |
| Database | MariaDB 11.4 LTS through `sqlx`'s MySQL driver. SQLite is read-only migration input and independent bootstrap storage, never the primary application database. |
| Database delivery | Publish two images: standard uses an explicit external `mysql://` URL; standalone supervises a private MariaDB 11.4 s6 service. The application never downloads database binaries. |
| Schema lifecycle | Before the first tagged release, edit the single `001_baseline.sql`. After the first tagged release, preserve it and add ordered forward migrations. No pre-MariaDB upgrade path is supported. |
| Native API | `/api/v1` is preserved. Arr façades are additive and thin. |
| Indexarr (#29) | Indexarr stays an independently deployable service. Its client/translation adapter remains inside `stackarr-indexer`; StackArr does not copy Indexarr internals. The Prowlarr façade presents Indexarr results through the same indexer domain as Cardigann/Newznab/Torznab. |
| Logical instances (#30) | One process supports multiple persisted logical façade instances. Each has a stable ID, name, kind, API-key hash, enable flag, listener and path-prefix settings, and root-folder/tag/profile scope. Instances are filtered views over shared media, queue, history, and providers—not duplicated libraries. One default Sonarr, Radarr, and Prowlarr instance is created. |
| Compatibility listeners | Every logical instance has a canonical path prefix on the main listener. A dedicated listener is optional. Both route to the same instance ID and contract tests. Path-prefix URLs are the durable identity; ports are deployment convenience. |
| Reported versions | System-status reports the contract snapshot, not the StackArr package version: Sonarr `4.0.13.2931`, Radarr `6.2.0.10390`, Prowlarr `2.1.4.5212`. StackArr's real version is exposed in the native status API and an additional compatibility response header where clients tolerate it. |
| Prowlarr applications | `application` and `appprofile` remain intentionally unsupported because one core removes cross-app sync. They return the captured arr-compatible unsupported/not-found behavior and are listed as intentional deviations. |
| Dependencies | Engine families stay exact-pinned on crates.io and receive grouped weekly reviewed updates. Published engines are never vendored or sourced privately. |
| CI runners | Organization jobs use explicit self-hosted labels. Untrusted fork code does not execute automatically on privileged self-hosted runners; a maintainer approves a safe run after review. |
| Coverage | Use pinned `cargo-llvm-cov`. Commit per-crate and workspace line-coverage baselines; CI rejects any decrease. `coverage-watchdog` is not the coverage tool. |
| Linux artifacts | Container manifest: `linux/amd64` and `linux/arm64`. musl: a separate `x86_64-unknown-linux-musl` binary build and smoke test, because musl is not a Docker platform. |

## 4. Target architecture

```text
unmodified clients
  ├─ /api/v3/* Sonarr logical instances ─┐
  ├─ /api/v3/* Radarr logical instances ─┼─ thin DTO/route translators
  ├─ /api/v1/* Prowlarr instances ───────┘        │
  └─ /api/v1/* StackArr native API ───────────────┤
                                                   ▼
        media + quality + profiles + decisions + scheduler
                   │              │              │
          indexer planner   download/import   migration
          ├─ Cardigann      ├─ SwarmForge     ├─ Sonarr
          ├─ Newznab        ├─ nzb-*          ├─ Radarr
          ├─ Torznab        └─ external       ├─ Prowlarr
          └─ Indexarr adapter                  └─ SABnzbd
                   └──────────────┬───────────────┘
                                  ▼
                         MariaDB 11.4 LTS
```

### Crate boundaries

- `stackarr-core`: configuration, storage primitives, logical instances, common
  identifiers, errors, and domain events.
- `stackarr-media`: generic media identity with TV and film adapters.
- `stackarr-parser`: pure release parsing; no I/O or database dependency.
- `stackarr-decision` (new in P6): ordered specifications and replayable outcomes.
- `stackarr-quality`: quality definitions, profiles, custom formats, and scoring.
- `stackarr-profiles` (new in P5): sources, subscriptions, snapshots, overrides,
  merge, provenance, and simulation.
- `stackarr-indexer`: query planning and adapters, including the external Indexarr
  client.
- `stackarr-download`, `stackarr-import`, `stackarr-scheduler`,
  `stackarr-metadata`, `stackarr-notify`, and `stackarr-migrate`: business services
  named by their domains.
- `stackarr-web`: native `/api/v1` and UI delivery.
- `stackarr-compat-core` plus Sonarr/Radarr/Prowlarr crates: DTOs, route wiring,
  authentication translation, SignalR, and error translation only.

Compatibility crates may depend on core services. Core crates never depend on a
compatibility crate. A business rule found in a façade is moved into the appropriate
domain crate before merge.

### Target schema contract for T20

The baseline proposed on `feat/mariadb-baseline` is **not accepted unchanged**. Its P5
profile and P6 decision groups are the correct direction, but the following revision is
the approved T20 contract:

1. `media_entities` is the mandatory owner of common identity. It stores media type,
   source provider and source ID, title/sort title, year, monitored state, library
   folder, quality profile, external IDs, and timestamps. The source key is unique by
   `(media_type, source_provider, source_id)`.
2. `series.entity_id` and `movies.entity_id` are `NOT NULL UNIQUE` foreign keys with
   cascade delete. Common fields are removed from adapter tables; adapters retain only
   TV- or film-specific attributes. Writes cannot create an adapter without a generic
   identity.
3. `media_files` has a stable owning `media_entity_id`, a unique normalized path within
   its library folder, and explicit episode/movie association tables. Polymorphic
   `media_type + media_id` references are not used where a foreign key is possible.
4. Add `compat_instances` and normalized scope tables for root folders, tags, and
   profiles. Store only API-key hashes. Enforce unique instance slug and unique enabled
   listener `(bind_address, port)`; validate unique path prefixes in application logic.
5. Queue, history, blocklist, decision records, and import candidates reference
   `media_entities` where the entity is known. Raw external candidates may keep a null
   entity reference plus their immutable input payload.
6. Retain the five P5 tables, with immutable profile snapshots keyed by source,
   upstream key, revision, and content hash. Overrides use JSON Pointer keys and retain
   base/local values for deterministic three-way merge.
7. Retain the two P6 tables, adding decision schema version, parser version, profile
   snapshot/hash, and ordered step records. A stored decision must be replayable without
   reading mutable current profile data.
8. Use InnoDB, `utf8mb4`, UTC `DATETIME(6)`, application-generated UUIDs where opaque
   IDs are required, `JSON` only for genuinely variable payloads, and indexes for every
   foreign key and measured list/filter path.
9. The baseline must install from empty MariaDB 11.4, reject invalid relationships,
   and pass fixture import, concurrency/upsert, JSON path, and round-trip tests on a
   live service before review acceptance.

This ruling resolves the approval question in #58/#102: revise the branch to this
contract, review the resulting DDL and tests, then split the provisional T21–T26 work
into issue-scoped commits. Documentation does not claim that implementation has landed.

## 5. Compatibility contract

The source snapshots are frozen and copied into `contracts/` during P2 with their
license, source revision, and SHA-256. A later upstream release never changes v1
silently.

| Façade | API | Reported version | OpenAPI source |
| --- | --- | --- | --- |
| Sonarr | v3 | `4.0.13.2931` | Sonarr tag `v4.0.13.2931` |
| Radarr | v3 | `6.2.0.10390` | Radarr tag `v6.2.0.10390` |
| Prowlarr | v1 | `2.1.4.5212` | Prowlarr tag `v2.1.4.5212`, source commit `574721bfb5e5c929b1e585bd5d4d144665dd7a05` |

Required shared behavior includes header and query-string API-key auth, browser forms
auth, exact error/status shapes, ordered `ProviderResource.fields[]`, SignalR negotiate
and JSON hub messages, pagination, date/time serialization, and stable logical-instance
identity. A same-named native endpoint is not compatibility evidence.

## 6. Delivery phases and gates

Phases are sequential. Work inside a phase may run in parallel only when its declared
dependencies and tests allow it. An epic closes only when its exit gate is met.

| Phase | Outcome | Exit gate |
| --- | --- | --- |
| P0 | Repository, license, workflow, issue structure, and documentation guardrails | Complete on `main`; the canonical guidance and public project exist. |
| P1 | MariaDB target baseline and dialect swap; reproducible CI/artifacts | Revised T20 schema accepted; issue-scoped T21–T26 changes merged; live MariaDB tests, coverage ratchet, conformance gate wiring, amd64/arm64 containers, musl artifact, standard image, and standalone image all green. |
| P2 | Capturing proxy, redacted golden store, replay/diff harness, OpenAPI test generation, and measured endpoint backlog | Every pinned operation has a test; real-client captures replay; coverage percentage and traffic-ranked issues are generated reproducibly. |
| P3 | Read-only Sonarr/Radarr/Prowlarr façades | Default and additional logical instances work; auth/provider/SignalR contracts pass; client read flows pass; intentional Prowlarr omissions are explicit. |
| P4 | Write façades, commands, importers, and legacy download-client protocols | Overseerr, Recyclarr, and nzb360 write flows pass; real arr/SAB migration passes; stock Sonarr downloads and imports through StackArr. |
| P5 | Native TRaSH/Profilarr subscriptions and safe updates | Subscribe/apply, provenance, three-way merge preview, compiled scoring benchmark, 90-day simulation, and guided setup pass API/UI/E2E tests. Frozen subsystems may then be reconsidered only through a new plan revision. |
| P6 | Parser and ordered decision parity; direct metadata and scene numbering | Ported parser and 30-specification corpora pass; decisions are queryable/replayable; known-hard scene/XEM fixtures pass. |
| P7 | Benefits possible only in a unified service | Duplicate indexer traffic is measurably reduced; ten notification providers are data-only; one cross-media queue enforces disk/bandwidth reservations and global priority. |

### P1 merge order

1. Revise and accept T20 to the schema contract above.
2. Rebase the MariaDB rescue work; split T21–T26 and standalone delivery into
   independently reviewable commits/PRs.
3. Run all ignored database tests against live MariaDB and retain CI evidence.
4. Merge license-header and coverage work only after each matches the decisions in
   this document; replace the `coverage-watchdog` assumption with `cargo-llvm-cov`.
5. Complete T19 with the separate musl job and both container delivery modes.
6. Close duplicate handoff findings after their owning task contains the evidence.

## 7. Open-issue coverage ledger

This ledger covers all 54 issues that were open on 2026-08-05. “Fold into” means the
finding remains visible but its implementation and closure evidence belong to the named
task; it is not a new architecture decision.

### P1 — consolidation and delivery (11)

| Issue | Disposition |
| --- | --- |
| [#32 E1](https://github.com/TheDancingDeveloper-org/NGMS/issues/32) | Phase epic; close only at the P1 gate. |
| [#57 T19](https://github.com/TheDancingDeveloper-org/NGMS/issues/57) | CI owner: self-hosted policy, live MariaDB, conformance, amd64/arm64, separate musl, and safe fork handling. |
| [#58 T20](https://github.com/TheDancingDeveloper-org/NGMS/issues/58) | Revise the proposed baseline to §4; the required design is settled here. |
| [#59 T21](https://github.com/TheDancingDeveloper-org/NGMS/issues/59) | One fresh-deploy baseline; merge after T20. |
| [#60 T22](https://github.com/TheDancingDeveloper-org/NGMS/issues/60) | Diffuse `sqlx` driver swap plus rename; standalone database delivery is part of acceptance, not a stub crate. |
| [#61 T23](https://github.com/TheDancingDeveloper-org/NGMS/issues/61) | Convert placeholders with an audit that rejects repeats/out-of-order binds. |
| [#62 T24](https://github.com/TheDancingDeveloper-org/NGMS/issues/62) | Hand-review insert identity/concurrency; all new code remains `RETURNING`-free. |
| [#63 T25](https://github.com/TheDancingDeveloper-org/NGMS/issues/63) | Port upserts by behavior, with conflict/concurrency tests. |
| [#64 T26](https://github.com/TheDancingDeveloper-org/NGMS/issues/64) | Convert JSON/identity types and prove JSON query behavior on MariaDB. |
| [#65 T27](https://github.com/TheDancingDeveloper-org/NGMS/issues/65) | Apply consistent SPDX headers; tracked by PR #110. |
| [#66 T28](https://github.com/TheDancingDeveloper-org/NGMS/issues/66) | Implement the `cargo-llvm-cov` baseline and ratchet; rework PR #109 if it assumes `coverage-watchdog`. |

### P2 — measured conformance (6)

| Issue | Disposition |
| --- | --- |
| [#33 E2](https://github.com/TheDancingDeveloper-org/NGMS/issues/33) | Phase epic; close only at the P2 gate. |
| [#68 T30](https://github.com/TheDancingDeveloper-org/NGMS/issues/68) | Capture and redact traffic from the five named client families. |
| [#69 T31](https://github.com/TheDancingDeveloper-org/NGMS/issues/69) | Store contracts with provenance, hashes, review rules, and deterministic normalization. |
| [#70 T32](https://github.com/TheDancingDeveloper-org/NGMS/issues/70) | Replay with structural diffs and explicit dynamic-field matchers. |
| [#71 T33](https://github.com/TheDancingDeveloper-org/NGMS/issues/71) | Generate one initially-red case per pinned OpenAPI operation. |
| [#72 T34](https://github.com/TheDancingDeveloper-org/NGMS/issues/72) | Rank missing endpoints by observed client traffic and generate bounded issues. |

### P3 — read compatibility (11)

| Issue | Disposition |
| --- | --- |
| [#34 E3](https://github.com/TheDancingDeveloper-org/NGMS/issues/34) | Phase epic; close only at the P3 gate. |
| [#29 D8](https://github.com/TheDancingDeveloper-org/NGMS/issues/29) | Resolved by §3: Indexarr remains separate; adapter stays in `stackarr-indexer`. |
| [#30 D9](https://github.com/TheDancingDeveloper-org/NGMS/issues/30) | Resolved by §3: persisted logical instances and pinned reported versions. |
| [#73 T35](https://github.com/TheDancingDeveloper-org/NGMS/issues/73) | Shared compatibility crate and enforceable dependency boundary; tracked by PR #108. |
| [#74 T36](https://github.com/TheDancingDeveloper-org/NGMS/issues/74) | Header/query/cookie authentication per logical instance. |
| [#75 T37](https://github.com/TheDancingDeveloper-org/NGMS/issues/75) | Exact provider-field reflection including order and privacy. |
| [#76 T38](https://github.com/TheDancingDeveloper-org/NGMS/issues/76) | SignalR negotiation and live queue events. |
| [#77 T39](https://github.com/TheDancingDeveloper-org/NGMS/issues/77) | Traffic-ranked Sonarr v3 GET surface. |
| [#78 T40](https://github.com/TheDancingDeveloper-org/NGMS/issues/78) | Traffic-ranked Radarr v3 GET surface. |
| [#79 T41](https://github.com/TheDancingDeveloper-org/NGMS/issues/79) | Traffic-ranked Prowlarr v1 GET surface with the two declared omissions. |
| [#80 T42](https://github.com/TheDancingDeveloper-org/NGMS/issues/80) | Sonarr/Radarr quality-definition resource and Recyclarr read flow. |

### P4 — writes, migration, and protocol compatibility (5)

| Issue | Disposition |
| --- | --- |
| [#35 E4](https://github.com/TheDancingDeveloper-org/NGMS/issues/35) | Phase epic; close only at the P4 gate. |
| [#81 T43](https://github.com/TheDancingDeveloper-org/NGMS/issues/81) | POST/PUT/DELETE across all façades and logical-instance scopes. |
| [#82 T44](https://github.com/TheDancingDeveloper-org/NGMS/issues/82) | Manual import, rename, command, and release/search workflows. |
| [#83 T45](https://github.com/TheDancingDeveloper-org/NGMS/issues/83) | Real Sonarr/Radarr/Prowlarr plus SABnzbd importer, dry run, and rollback-safe failure. |
| [#84 T46](https://github.com/TheDancingDeveloper-org/NGMS/issues/84) | qBittorrent WebUI and SABnzbd protocols backed by embedded engines. |

### P5 — profiles (7)

| Issue | Disposition |
| --- | --- |
| [#36 E5](https://github.com/TheDancingDeveloper-org/NGMS/issues/36) | Phase epic; close only at the P5 gate. |
| [#85 T47](https://github.com/TheDancingDeveloper-org/NGMS/issues/85) | `stackarr-profiles` and subscribed profiles. |
| [#86 T48](https://github.com/TheDancingDeveloper-org/NGMS/issues/86) | Deterministic three-way merge with preview/conflict review. |
| [#87 T49](https://github.com/TheDancingDeveloper-org/NGMS/issues/87) | Source/revision/override provenance in API and UI. |
| [#88 T50](https://github.com/TheDancingDeveloper-org/NGMS/issues/88) | Compiled scoring with a checked-in representative benchmark corpus. |
| [#89 T51](https://github.com/TheDancingDeveloper-org/NGMS/issues/89) | Historical 90-day impact report before apply. |
| [#90 T52](https://github.com/TheDancingDeveloper-org/NGMS/issues/90) | Native guided setup equivalent to the pinned ProfSync questionnaire. |

### P6 — parser, decisions, and metadata (5)

| Issue | Disposition |
| --- | --- |
| [#37 E6](https://github.com/TheDancingDeveloper-org/NGMS/issues/37) | Phase epic; close only at the P6 gate. |
| [#91 T53](https://github.com/TheDancingDeveloper-org/NGMS/issues/91) | Port Sonarr parser tests first, then implementation. |
| [#92 T54](https://github.com/TheDancingDeveloper-org/NGMS/issues/92) | Port all 30 ordered decision specifications and tests. |
| [#93 T55](https://github.com/TheDancingDeveloper-org/NGMS/issues/93) | Persist/query/replay the complete decision breakdown. |
| [#94 T56](https://github.com/TheDancingDeveloper-org/NGMS/issues/94) | Direct TVDB/TMDB metadata, scene numbering, and XEM hard fixtures. |

### P7 — unification dividends (4)

| Issue | Disposition |
| --- | --- |
| [#38 E7](https://github.com/TheDancingDeveloper-org/NGMS/issues/38) | Phase epic; v1 closes only at the P7 gate and §1 contract. |
| [#95 T57](https://github.com/TheDancingDeveloper-org/NGMS/issues/95) | Shared query planner/cache/rate limiter with before/after request metrics. |
| [#96 T58](https://github.com/TheDancingDeveloper-org/NGMS/issues/96) | Declarative provider schema; at least ten notification providers use no provider-specific Rust. |
| [#97 T59](https://github.com/TheDancingDeveloper-org/NGMS/issues/97) | Cross-media priority plus enforceable disk and bandwidth reservations. |

### Handoff findings without milestones (5)

| Issue | Owning disposition |
| --- | --- |
| [#102](https://github.com/TheDancingDeveloper-org/NGMS/issues/102) | The approval question is resolved by the T20 revision contract in §4; close after #58 records and implements it. |
| [#103](https://github.com/TheDancingDeveloper-org/NGMS/issues/103) | After T20 revision, split the rescue branch by T21–T26 and delivery concern before review. |
| [#104](https://github.com/TheDancingDeveloper-org/NGMS/issues/104) | Database delivery was already decided under closed D5; fold implementation into #60/#57 and prove the standalone image. It is unrelated to Indexarr decision #29. |
| [#105](https://github.com/TheDancingDeveloper-org/NGMS/issues/105) | Fold coverage into #66 and musl/multi-arch into #57 under §3. |
| [#106](https://github.com/TheDancingDeveloper-org/NGMS/issues/106) | Fold live MariaDB evidence into #57/#58; passing ignored-only local tests is not acceptance. |

Count check: 11 + 6 + 11 + 5 + 7 + 5 + 4 + 5 = **54**.

## 8. Engineering and release policy

- Start new behavior with a failing test derived from its governing source.
- Unit-test logic; integration-test database, filesystem, network, and process
  boundaries; E2E-test user/client flows.
- Golden capture changes require source provenance and explicit review. Never update a
  fixture merely to make CI green.
- Use runtime `sqlx` queries with audited binds. New application SQL is
  `RETURNING`-free even though MariaDB supports insert `RETURNING`.
- Preserve user data and wire contracts. Migrations are forward-only after the first
  release and must have rollback/recovery instructions even when SQL rollback is not
  possible.
- Required Rust gates are `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-features -- -D warnings`, and `cargo test --workspace --all-features`; phase
  gates add MariaDB, conformance, coverage, UI, and artifact tests.
- Do not vendor published crates, add private sources, add undocumented `#[allow]`, or
  add AI contributor/co-author attribution.

## 9. Documentation maintenance

- This file owns future state and phase order.
- `README.md` owns the public summary and v1 promise.
- `AGENTS.md` and `CONTRIBUTING.md` own contributor workflow.
- `API-COMPATIBILITY.md` owns pinned wire targets and measured client status.
- `ARCHITECTURE.md`, `DATABASE.md`, `DEPLOYMENT.md`, and `CONFIGURATION.md` distinguish
  current implementation from approved target state until P1 lands.
- `docs/backlog.json` is the historical 76-item seed manifest, not live issue state.
- GitHub is the live work-state authority. This ledger is refreshed whenever issues are
  added, closed, split, or materially re-scoped.
