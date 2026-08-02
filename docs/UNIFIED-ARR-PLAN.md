# NGMS → Unified Arr: Comprehensive Plan

**Status:** In execution
**Date:** 2026-08-02 (re-baselined against `github/main` the same day)
**Owner:** TheDancingDeveloper-org
**Scope:** Turn NGMS into a single Rust service that replaces Sonarr + Radarr + Prowlarr,
speaks their legacy APIs wire-compatibly, treats TRaSH Guides / Profilarr as first-class,
and keeps the embedded torrent + Usenet engines.

Every number was measured on 2026-08-02 against `TheDancingDeveloper-org/NGMS@main` — not
the stale local checkout (see §0.1).
Measurement commands are in [Appendix E](#appendix-e--how-the-numbers-were-measured) so
they can be re-run and challenged.

---

## 0. Execution handoff

**Read this section first. It is written for the agent that will execute the plan.**

### 0.1 Where the code is

```bash
git clone https://github.com/TheDancingDeveloper-org/NGMS.git ngms-unified-arr
cd ngms-unified-arr
git switch -c feat/unified-arr-execution
```

**Do not work in the legacy shared NGMS checkout.** It is a disjoint 43-commit branch with no
common ancestor (`git merge-base main github/main` returns empty) and a HEAD that does not
exist on GitHub. The first draft of this plan was measured against it and was materially
wrong. Everything here has since been re-measured against canonical GitHub `main`. If you find a
discrepancy, trust the repo and correct this file.

Naming differs deliberately: the stable **repository/image identity** is `NGMS`, the
**product and crates** are `StackArr` / `stackarr-*`, and the **torrent engine** ships as
`swarmforge` on crates.io. See resolved decision D7 (§10.4).

### 0.2 What already exists — do not recreate

Created on the repo on 2026-08-02 while drafting this plan:

- **31 labels** — `phase:p0`…`phase:p7` (8), `area:*` (16), `type:*` (5), `risk:high`,
  `blocked`. Stock GitHub and Dependabot labels also present.
- **8 milestones** — `P0`…`P7`, numbers **1–8**, descriptions matching §7.

Completed during execution on 2026-08-02:

- **76 issues** created from the authoritative manifest in
  [Appendix F](#appendix-f--initial-github-issue-backlog).
- **Org Projects v2 board** created with the required custom fields and all issues added.

Already true of the repo, so *not* work items despite appearing in older plans: GitHub
Actions is live, all engine crates come from crates.io, and `crates/usenet/` is gone.

### 0.2b Decision log

Ratified by the owner on 2026-08-02. These are **settled** — do not re-litigate them.

| # | Decision | Ruling |
|---|---|---|
| D2 | Database target | **MariaDB.** Pin **11.4 LTS**. Unblocks all of P1. |
| D3 | Licence | **GPL-3.0.** Unblocks T3 and T27. |
| D4 | Product scope | **Freeze list ratified as written in §4.3.** |

Consequence of D2 that the executing agent must respect: MariaDB 10.5+ *does* support
`RETURNING` on `INSERT`, and T24 may use it to reduce the 78-site rewrite. **New code must
still be written `RETURNING`-free** so a later SQLite or MySQL 8 backend stays cheap. If you
want to overturn that, raise it — do not just start using it.

Resolved during execution on 2026-08-02: **D1** keeps GitHub repository `NGMS` and
the existing GHCR path; **D6** keeps exact engine pins and uses grouped weekly
Dependabot/Renovate pull requests; **D7** uses StackArr as the product/crate name while
leaving the repository and image identifiers stable. D5, D8, and D9 block later phases.

### 0.3 Order of operations

1. Read §1 (decision), §3 (principles), §4 (decisions required) in full.
2. **Resolve the blocking decisions before writing code.** D2 (MariaDB vs MySQL 8) blocks
   all of P1. D4 (scope freeze) is the mitigation for the top risk in the register.
3. Create the issues (Appendix F).
4. P0, then P1. Do not begin P2 until P1's exit criteria are met and green.

### 0.4 Rules of engagement

Not style preferences — each one is why a specific past failure happened.

1. **Test-first, from a spec.** §6.2 is binding. For compat work the spec is machine-
   readable; generate the failing test from `openapi.json` before the handler exists.
2. **Never vendor a published crate.** §2.4 is the cautionary tale. Need a change in
   `nzb-*` or `swarmforge`? Change it upstream and bump the pin.
3. **Façades contain no logic** — DTOs and translation only. Anything else goes in core.
4. **Do not widen scope silently.** §4.3 lists what is frozen. If a task appears to need a
   frozen subsystem, stop and raise it.
5. **Correct this document when reality disagrees.** It is a working artefact, not a
   record. Re-run Appendix E and update the numbers.
6. **Report honestly.** If an exit criterion is unmet, say so. Do not mark it done.

### 0.5 What "done" means

Appendix A is the v1 definition of done, and it is deliberately expressed as *third-party
software working unmodified* rather than internal completeness. If Overseerr, Bazarr,
Recyclarr, nzb360 and Homepage all work against it, the project has succeeded regardless of
what remains unimplemented.

---

## 1. The decision

**Do not start a fourth greenfield attempt. Extend NGMS.**

The case rests on three findings, each of which was surprising enough to change the
recommendation:

1. **It is real, and substantial.** 70,303 lines of own code, **1,060 tests** (15.1 per
   1k LOC) and 5 TODO/`unimplemented!()` markers. The hard parts — Cardigann engine with a
   parity harness, 549 indexer definitions, release parser, custom-format scoring, import,
   scheduler, and working Sonarr/Radarr/Prowlarr DB importers — already exist and are
   tested.

2. **The engine-dependency question is already settled.** All seven Usenet crates and the
   torrent engine come pinned from crates.io. What remains is `crates/torrent/` — 44,197
   lines that are not a workspace member and never compile. Deleting it removes 39% of the
   apparent tree at zero cost.

3. **The gap to the target is additive.** The project already covers ~48% of Sonarr's
   resource topology, 42% of Radarr's and 55% of Prowlarr's, under its own `/api/v1`
   namespace. What is missing is a compatibility façade and a finite list of resources —
   not a rewrite.

The alternative — porting 542k lines of C# — is not the shape of the work. The three
reference apps are forks of one common ancestor (NzbDrone) and are ~70% the same program.
The port is *one* core parameterised by media type, with thin per-app façades.

### What we are explicitly not doing

- Not porting the C#. We reimplement against specs (see §3).
- Not porting the 405 FluentMigrator migrations. One importer reads final-state DBs.
- Not keeping the project's own 18 migrations either. It deploys fresh, so the schema collapses
  to a single baseline and is redesigned once, now, for the unified model (§4.2).
- Not porting 243k lines of arr React. We keep and grow the NGMS UI.
- Not maintaining a Usenet or torrent fork. Both become upstream dependencies.

---

## 2. Evidence base

### 2.1 Reference sources

Located at `Active/RefenceMaterials/reference/External Repos/`. All GPL-3.0.

| Repo | Version | Prod C# LOC | Test LOC | Frontend LOC | DB migrations | OpenAPI |
|---|---|---|---|---|---|---|
| Sonarr | v4.0.13.2931 (2026-03-17) | 224,140 | 73,212 | 87,957 | 223 | v3: 162 paths / 136 schemas; v5 also in-tree |
| Radarr | v6.2.0.10390 (2026-04-19) | 191,008 | 59,418 | 101,159 | 140 | v3: 164 paths / 137 schemas |
| Prowlarr | (2025-10-04) | 127,064 | 19,054 | 54,082 | 42 | v1: 93 paths / 70 schemas |

Sonarr core subsystem sizes, for effort calibration:

| Subsystem | LOC | Notes |
|---|---|---|
| `Parser/` | 4,519 | plus 4,644 LOC of parser tests — the single richest spec asset |
| `DecisionEngine/` | 3,454 | **30 specifications**, order-sensitive |
| `Organizer/` | 2,088 | naming tokens |
| `CustomFormats/` | 906 | the TRaSH hinge |

Provider long tail (per app): ~47 indexers, ~47 notifications, ~21–23 download clients,
~35 import lists. Deduplicated across the three: **~100 unique providers**.

### 2.2 The project as it stands

> **Baseline: `github.com/TheDancingDeveloper-org/NGMS`, branch `main`, 499 commits, last
> pushed 2026-07-31.** Measured 2026-08-02.
>
> ⚠️ **Do not use the legacy shared NGMS checkout as the source of truth.** That local
> directory is a disjoint 43-commit branch — `git merge-base main github/main` is empty and
> its HEAD `7dbec753` does not exist on GitHub. An earlier draft of this plan was measured
> against it and was wrong in several material respects (§2.4). Work from a fresh clone.

Crates are named `stackarr-*`; the torrent engine is published on crates.io as
**`swarmforge`**, aliased to the historical `librtbit` names in `Cargo.toml`.

| Crate | LOC | Tests | | Crate | LOC | Tests |
|---|---|---|---|---|---|---|
| stackarr-web | 24,867 | 62 | | stackarr-indexer | 2,202 | 38 |
| stackarr-core | 4,943 | 44 | | stackarr-media | 2,132 | 22 |
| stackarr-scheduler | 4,475 | 11 | | stackarr-parser | 1,838 | 175 |
| stackarr-import | 4,411 | 137 | | stackarr-plex | 1,798 | 22 |
| stackarr-migrate | 4,409 | 24 | | stackarr-metadata | 1,222 | 21 |
| stackarr-quality | 4,137 | 158 | | stackarr-postgres | 1,216 | 20 |
| stackarr-cardigann | 3,850 | 90 | | stackarr-notify | 1,110 | 31 |
| stackarr-stream | 3,645 | 111 | | stackarr-cardigann-parity | 989 | 0 |
| stackarr-download | 3,059 | 94 | | | | |

**Own code: 70,303 LOC / 1,060 tests** (15.1 tests per 1k LOC).
Plus `crates/torrent/` — 44,197 LOC and 396 tests that **are not a workspace member and do
not compile or run**.

- **API paths:** 246 distinct, 49 resources.
- **Cardigann definitions:** 549 YAML.
- **Database:** PostgreSQL 17 via `sqlx` 0.8.6; **18** migrations; **zero** compile-time
  `query!` macros despite `macros`/`derive` features being enabled. See §4.2.
- **Engines:** all seven `nzb-*` crates pinned from crates.io at current versions; torrent
  via `swarmforge` from crates.io. **No live vendored engine remains.**
- **CI:** GitHub Actions on self-hosted runners (`node-b`) — jobs `rust`, `ui`,
  `container` → GHCR, including a "Verify public dependency boundary" step.
- **Docs:** 26 files in `docs/`.
- **TODO/`unimplemented!()` markers:** 5 across 70k LOC.

### 2.3 API coverage gap

Crude top-level-resource match against the checked-in OpenAPI specs (undercounts —
plural/singular and nested routers are missed). StackArr exposes **49** resources total.

| Target | Resources | Present | Missing |
|---|---|---|---|
| Sonarr v3 | 42 | **20** | autotagging, customfilter, delayprofile, diskspace, episodefile, health, importlistexclusion, indexerflag, language, languageprofile, localization, manualimport, mediacover, metadata, parse, **qualitydefinition**, releaseprofile, remotepathmapping, rename, rootfolder, seasonpass, update |
| Radarr v3 | 43 | **18** | alttitle, collection, credit, exclusions, extrafile, movie, moviefile, + most of the above |
| Prowlarr v1 | 20 | **11** | applications\*, appprofile\*, customfilter, health, indexerproxy, indexerstats, indexerstatus, localization, update |

\* `applications` / `appprofile` are **deleted, not ported** — unification removes the sync
problem they exist to solve.

Note `customformat` **is** already present (migration `014_custom_format_fields.sql`), which
materially de-risks the TRaSH work in P5. `qualitydefinition` is not.

### 2.4 The vendored-engine lesson

**This is now history, not a task — but the lesson is the reason for several P0 guardrails.**

An earlier NGMS line vendored `rustnzbd` in March 2026 and made 5 local commits. Every one
of them was subsequently absorbed upstream (ramp-up delay → `c7d8294`; the crate-dependency
-direction refactor → `crates/nzb-nntp/src/config.rs`; the nzb-web re-exports →
`nzb-web/src/lib.rs:3-5`). The fork delta ended at **zero**, while the vendored copy drifted
~12,400 lines behind four months of upstream releases. It went unnoticed because the
vendored crates declared `version.workspace = true` and so carried **no version identity** —
nothing could have reported the drift.

**The current repo has already fixed this.** All seven Usenet crates are pinned from
crates.io, and the torrent engine comes from `swarmforge`:

| Crate | Pinned | | Crate | Pinned |
|---|---|---|---|---|
| nzb-web | =0.4.21 | | nzb-postproc | =0.2.7 |
| nzb-core | =0.2.17 | | nzb-news | =0.1.13 |
| nzb-nntp | =0.2.23 | | nzb-dispatch | =0.2.7 |
| nzb-decode | =0.1.3 | | swarmforge (torrent) | =0.1.0 |

What survives as work: **`crates/torrent/` is still on disk** — 44,197 LOC and 396 tests
that are not workspace members and never build. Delete it (P1/T15). And add the guardrails
that make silent re-vendoring impossible (P0/T12) — the repo already has a "Verify public
dependency boundary" CI step to build on.

### 2.5 Documentation drift found

`CLAUDE.md` on `main` still documents a layout the `Cargo.toml` contradicts. Correct in
P0/T6, then enforce in CI (P1/T29).

| Claim in `CLAUDE.md` | Reality |
|---|---|
| "`torrent/` — Vendored librtbit (12 crates, from rustTorrent)" | Not a workspace member. Consumed from crates.io as `swarmforge`. The directory is dead. |
| "`usenet/` — Vendored nzb engine (5 crates, from rustnzbd)" | **The directory no longer exists.** All seven crates come from crates.io. |
| "PostgreSQL 17 (required). **Never use SQLite for application data.**" | Superseded — the project is moving to MariaDB (§4.2). This line must change with it. |

---

## 3. Guiding principles

1. **Spec-driven, not source-driven.** Three specs, in priority order:
   - the checked-in `openapi.json` files (the wire contract),
   - the arr NUnit test corpus, ~151k LOC (the behavioural contract — mine it, don't read
     the implementation),
   - the TRaSH Guides JSON repo (the quality contract).
2. **Additive.** `/api/v1` is not touched. Compatibility arrives as new crates.
3. **Compatibility is the moat, not nostalgia.** The measure of success is that Overseerr,
   Bazarr, Recyclarr, nzb360 and Homepage work unmodified.
4. **Depend, don't vendor.** Every shared engine is a published, versioned crate with
   Renovate on it. The Usenet fork is the cautionary tale.
5. **Delete before adding.** P1 removes 44k dead lines before a feature is written.
6. **Tests first, always.** See §6.

---

## 4. Decisions required before P1

These are cheap now and expensive later. Each needs an explicit answer.

### 4.1 Licence — **DECIDED: GPL-3.0** (D3, 2026-08-02)

`Cargo.toml` declares MIT. Sonarr, Radarr and Prowlarr are all GPL-3.0, and we intend to
derive from their OpenAPI specs and mine their test corpus as our behavioural spec. That
makes NGMS a derivative work.

**Ruling: relicense to GPL-3.0.** Accepted with the consequence understood — this forecloses
a closed-source commercial edition. Executed in P1 by T3 (LICENSE file, `Cargo.toml`
`license` field) and T27 (source headers).

Note the vendored Usenet crates are MIT; consuming them from crates.io under GPL-3.0 is
fine (MIT is GPL-compatible).

### 4.2 Database — swap Postgres → MySQL, and drop all migrations

**DECIDED (D2, 2026-08-02): MariaDB, pinned to 11.4 LTS.** The project deploys fresh, so
there is no upgrade path to preserve and all existing migrations are deleted in favour of a
single baseline schema.

This is the right moment for both changes and the worst possible moment to defer them —
every query written from P1 onward locks the dialect in, and the façade work adds hundreds
of queries.

#### The fresh-deploy dividend

Because there is no installed base, the 18 existing migrations collapse to
one `001_baseline.sql`. That is worth more than the tidiness: **it means the schema can be
restructured freely, right now, for the unified media model** described in §5 — the
media-type-generic core, the profile-provenance tables needed by P5, and the explainable-
decision records needed by P6. Doing that schema work later costs a migration chain and a
data backfill. Doing it in P1 costs nothing.

So P1 does not merely translate the schema — it designs the *target* schema once.

#### Swap inventory (measured 2026-08-02)

| Item | Count | Effort |
|---|---|---|
| **Compile-time `sqlx::query!` macros** | **0** | **None — the saving grace.** All queries are runtime `sqlx::query(...)` despite `macros`/`derive` being enabled. No `.sqlx` offline cache to regenerate, no live DB at compile time. |
| Positional placeholders `$1..$n` → `?` | **1,420** | Mostly scriptable — **but see the trap below** |
| `RETURNING` clauses | **78** | **The single largest cost.** MySQL has no `RETURNING`. |
| `ON CONFLICT` | **77** | → `ON DUPLICATE KEY UPDATE` / `INSERT IGNORE` |
| `jsonb` references in Rust | **56** | → `JSON` |
| `PgPool` / `Postgres` type refs | **153** | Mechanical → `MySqlPool`. **Correction (2026-08-02):** the count was 173 before `crates/torrent/` was deleted, and the claim that these are concentrated in `stackarr-postgres` was wrong — that crate contained **zero** `PgPool` and `sqlx::` references. It was an embedded-Postgres *server provisioner*, not a query layer. The refs are spread across ten crates: scheduler 35, import 29, core 27, web 21, media 16, plex 11, quality 5, migrate 4, stream 3, notify 1. See the note under §7 P1. |
| `BIGSERIAL` / `SERIAL` in schema | 20 / 16 | → `BIGINT AUTO_INCREMENT` / `INT AUTO_INCREMENT` |
| `JSONB` in schema | 27 | → `JSON` (MySQL 8 stores JSON binary; no `jsonb` keyword) |
| `gen_random_uuid()` | 2 | → `UUID()` (MySQL 8) or generate app-side (preferred — portable) |
| `ILIKE` | ~2 | → `LIKE` (MySQL collations are case-insensitive by default) |
| `sqlx` features | 1 line | `"postgres"` → `"mysql"` |

**The placeholder trap.** `$1` is a *named* position and may legally repeat or appear out
of order within a query; `?` is positional-by-occurrence. Any query that reuses `$1` twice,
or binds out of order, silently breaks under a naive regex rewrite. The conversion script
must detect non-monotonic or repeated placeholders and fail loudly rather than convert
them. Assume a handful need hand-rewriting.

**`RETURNING` is the real work.** 78 sites, each currently doing insert-and-read-back in
one round trip. Under MySQL each becomes either `INSERT` + `SELECT LAST_INSERT_ID()` inside
a transaction, or an insert followed by a re-select on a natural key. Both are correct;
both are more code and one more round trip.

#### MySQL or MariaDB?

Worth an explicit choice, because it materially changes cost:

- **MariaDB 10.5+ supports `RETURNING` on `INSERT`** (and 10.0+ on `DELETE`). That could
  eliminate most of the 78-site rewrite.
- MySQL 8.0 does not support `RETURNING` at all.
- `sqlx`'s `mysql` driver targets both; MariaDB is wire-compatible.

**Ruling: MariaDB 11.4 LTS.** T24 may use MariaDB's `RETURNING` support opportunistically
to reduce the 78-site rewrite, but **new code must be written `RETURNING`-free** so a later
SQLite or MySQL 8 backend stays cheap. Document the target in `CONFIGURATION.md` and
`docs/DATABASE.md`, and pin the version in the CI service container.

#### What this does not solve

MySQL/MariaDB is still a server process. The NAS/Raspberry Pi adoption concern that
motivated the earlier SQLite suggestion remains open — Sonarr and Radarr ship with SQLite
and need no external database. Two ways to close it, neither in P1:

1. Ship MariaDB inside the container (s6-overlay already in use) so single-box installs are
   still one `docker run`. **Recommended** — cheap, and preserves the "just works" story.
2. Add a SQLite backend later behind the same query layer. Much cheaper *if* the P1 rewrite
   avoids dialect-specific constructs, which is another reason to avoid `RETURNING`.

Track as open question §10.8.

### 4.3 Product scope boundary — **RATIFIED** (D4, 2026-08-02)

NGMS today is Sonarr + Radarr + Prowlarr + Overseerr + a media server + a P2P discovery
layer. Adding TRaSH/Profilarr widens it further. **Scope, not Rust, is the risk.**

| Subsystem | LOC | Recommendation |
|---|---|---|
| Embedded torrent + Usenet engines | (external) | **Core.** The one thing Sonarr structurally cannot do. Single container, no SABnzbd/NZBGet/qBittorrent. |
| `stackarr-cardigann` + 549 defs | 3,797 | **Core.** This is the Prowlarr replacement. |
| `stackarr-migrate` | 3,726 | **Core.** Adoption depends on it. |
| `stackarr-stream` (HLS/transcode) | 3,477 | **Freeze.** Jellyfin's job. Pure maintenance surface. |
| `stremio` routes | — | **Freeze or spin out.** |
| `stackarr-plex` | 1,563 | **Keep, low priority.** Integration, not ownership. |
| `discover` / `trending` / `requests` / `watchlist` | — | **Defer.** Overseerr does this and we will be API-compatible with it anyway. |
| `stackarr-bootstrap` (UPnP, BIP39) | 1,200 | **Freeze.** Interesting, orthogonal, unfinished. |

**This table is ratified and binding.** "Freeze" = keeps compiling and stays tested,
accepts no new features, revisited after P5. A PR that adds functionality to a frozen
subsystem should be rejected on those grounds alone — scope is the top entry in the risk
register, and this list is its only real mitigation.

### 4.4 Versioning and release

Adopt the `rustnzbd` model that is demonstrably working: independently versioned crates,
published, Renovate-managed. Any NGMS crate another project might consume
(`stackarr-cardigann` is the obvious candidate — 549 definitions is a community asset) gets
published.

### 4.5 CI platform — **already GitHub Actions; extend it**

Resolved by reality: the repo is already public on GitHub with GitHub Actions on
self-hosted runners. See §6.3 — the work is **extending** the pipeline (conformance,
coverage ratchet, multi-arch, MariaDB service), not migrating to it.

---

## 5. Target architecture

```
stackarr-core          ── media-type-generic domain, storage, config
  stackarr-domain-tv       series / season / episode adapter
  stackarr-domain-film     movie / collection adapter
  stackarr-domain-*        (future: music, books) — plugin-shaped, not forks

stackarr-decision      ── NEW. Ported decision engine, 30 specs, explainable
stackarr-quality       ── custom formats, quality definitions, TRaSH scoring
stackarr-profiles      ── NEW. TRaSH/Profilarr subscription, 3-way merge, provenance
stackarr-indexer       ── Cardigann + Newznab/Torznab + Indexarr
stackarr-download      ── embedded torrent (librtbit) + Usenet (nzb-web) + external clients
stackarr-import        ── scan, import, rename, organise
stackarr-metadata      ── TMDB/TVDB, scene numbering, XEM
stackarr-notify        ── declarative HTTP providers (see §7.5)
stackarr-scheduler     ── background tasks
stackarr-migrate       ── Sonarr/Radarr/Prowlarr/SABnzbd importers

stackarr-web           ── /api/v1  (native, unchanged)
stackarr-compat-core   ── NEW. Shared arr concerns: ProviderResource field reflection,
                      X-Api-Key + querystring auth, SignalR hub, error shapes
  stackarr-compat-sonarr-v3     ── NEW. thin façade ─┐
  stackarr-compat-radarr-v3     ── NEW. thin façade ─┼─ all over the same core
  stackarr-compat-prowlarr-v1   ── NEW. thin façade ─┘
```

**Façade rule:** a compat crate contains DTOs, route wiring and translation *only*. Any
logic that appears in a façade belongs in the core. This is enforceable in review and is
the difference between one product and three.

**Instance identity.** Overseerr must be able to point "Sonarr" at one endpoint and
"Radarr" at another. Serve each façade on its own port *and* its own path prefix, with a
per-façade API key, so both deployment styles work.

### 5.1 Compatibility details that get missed

- **SignalR.** `/signalr/messages` — negotiate handshake plus the JSON hub protocol over
  WebSocket. nzb360, LunaSea and the arr web UIs depend on it. Not optional; goes in
  `stackarr-compat-core`.
- **`ProviderResource` field reflection.** The UI and Prowlarr build settings forms from
  the `fields[]` array returned by provider endpoints — including `selectOptions`,
  `privacy`, `hidden` and **ordering**. Must match byte-for-byte in shape.
- **Auth.** `X-Api-Key` header *and* `?apikey=` querystring, plus the forms-auth cookie.
- **Version sniffing.** Clients gate features on `/api/v3/system/status`. Pick the reported
  version deliberately and write it down.
- **Download clients over legacy protocols.** Expose the embedded engines via a
  SABnzbd-compatible and qBittorrent-WebUI-compatible API. This gives the façade's
  `downloadclient` resource something real to point at — *and* lets someone's existing
  Sonarr use NGMS as its download client with zero changes. Cheapest possible on-ramp:
  adopt the download half before committing to the arr half.

---

## 6. Engineering practice

### 6.1 Where NGMS already stands

| | StackArr | rdpapp |
|---|---|---|
| Rust LOC (own) | 70,303 | 45,906 |
| Tests | 1,060 | 289 |
| Tests per 1k LOC | **15.1** | 6.3 |

StackArr is already 2.4× denser in tests than rdpapp. What rdpapp has that NGMS lacks is not volume
— it is **discipline and gates**:

- `CLAUDE.md` §4 mandates `cargo build && cargo fmt && cargo clippy -- -D warnings &&
  cargo test` before every commit, and explicitly requires new tests for new logic.
- **Contract/golden files** — `contracts/v1/library-snapshot.json`. A frozen, versioned
  artefact that fails the build when the shape changes.
- **Live integration tests** — `tests/rdp-live/`.
- **A task runner** — `justfile` with `check`, `test-web`, `test-integration`,
  `test-backup-restore`, `test-visual`, `smoke`.
- **Operational drills** — `ci/backup-restore-drill.sh`.

### 6.2 The TDD standard to adopt

Applied strictly from P1 onward. The compat work is uniquely well suited to it because the
specification *already exists in machine-readable form*.

1. **Red first, from the spec.** For every façade endpoint, the test is generated from
   `openapi.json` before the handler exists: request shape, response schema, status codes.
   The endpoint is not "started" until a failing test names it.
2. **Golden files are the contract.** Adopt rdpapp's `contracts/` pattern at
   `contracts/arr-v3/<resource>.json` — captured real responses from live Sonarr/Radarr.
   A diff is a build failure, never a silent drift.
3. **Mine, don't invent, the behavioural tests.** Sonarr's 4,644 LOC of parser tests and
   its 30 DecisionEngine specifications are the spec for `stackarr-parser` and `stackarr-decision`.
   Port the *test cases* first; the implementation follows to make them pass. This is the
   single highest-leverage activity in the whole plan.
4. **Property tests where the input space is hostile.** Release-name parsing and
   custom-format scoring get `proptest`, not just examples.
5. **`mock-nntp-server`** (free with the crates.io move) makes Usenet integration tests
   hermetic. Use it.
6. **Gates are non-negotiable.** `fmt --check`, `clippy -- -D warnings`, `test --workspace`
   block merge. No exceptions, no `#[allow]` without a comment naming the reason.
7. **Coverage as a ratchet, not a target.** Record it per crate; the number may not go
   down. `coverage-watchdog` already exists in this workspace — wire it in.
8. **A `justfile`**, mirroring rdpapp: `check`, `test`, `test-compat`, `test-e2e`,
   `conformance`, `smoke`.

### 6.3 CI — extend the existing pipeline

**Current state.** `.github/workflows/ci.yml` runs on **self-hosted runners**
(`[self-hosted, node-b, linux, x64, rust]`) with jobs `rust`, `ui` and `container` → GHCR,
plus a "Verify public dependency boundary" step and Dependabot. This is far more mature
than the abandoned local branch suggested — the migration to GitHub happened on 2026-07-30
(`dbfacaca feat: migrate NGMS to GitHub and SwarmForge`).

**What is still missing.** Four gaps, in order of weight:

1. **No conformance job.** The single most important gate in this plan (P2) has nowhere to
   run yet.
2. **Self-hosted only.** Every job requires the `node-b` runner. An outside contributor
   cannot get CI on a fork — which defeats the adoption argument for being public at all.
   At minimum the `rust` and `ui` jobs should run on `ubuntu-latest`.
3. **No coverage ratchet.** `coverage-watchdog` exists in the workspace and is unused here.
4. **No multi-arch build.** linux/arm64 and musl static builds matter for the NAS audience.

**Target pipeline** (`.github/workflows/ci.yml`):

| Job | Contents |
|---|---|
| `lint` | `fmt --check`, `clippy --workspace -- -D warnings` — **move to `ubuntu-latest`** |
| `test` | `cargo test --workspace` against a MariaDB service container (pinned to the §4.2 target) — **`ubuntu-latest`** |
| `conformance` | replay recorded arr traffic against the façade; diff golden files |
| `build` | matrix: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-musl` |
| `docker` | multi-arch buildx → GHCR (`ghcr.io/<org>/ngms`) |
| `release` | tag-triggered, binaries + image + changelog |
| `coverage` | ratchet check |

Notes:
- Use `Swatinem/rust-cache` for the hosted jobs; keep sccache on the self-hosted ones.
- Add a `rust-toolchain.toml` so local and CI agree.
- Keep `container` and deploy on self-hosted — those legitimately need the homelab.
- No private registry credentials are needed: `swarmforge` and `nzb-*` are on crates.io.

---

## 7. Phase plan

Phases are sequential; work within a phase is parallelisable. Each has an exit criterion
that is objectively checkable.

### P0 — Repository, guardrails, and work tracking

No code changes. Stand up the place the work will be tracked, and the guardrails that stop
the next eighteen months drifting the way the Usenet fork did. This phase exists because
every problem found in §2.5 — dead vendored code, a four-month-stale fork, three false
claims in `CLAUDE.md`, a CI pipeline with no build step — is a *process* failure, not an
engineering one. Fix the process first.

**Resolved:** source, issues, projects, and CI live in `TheDancingDeveloper-org/NGMS`.

#### P0.1 — Repository

**The repo already exists**: `TheDancingDeveloper-org/NGMS`, public, with GitHub Actions
live. Labels and milestones were pre-created; the 76 issues and roadmap board were added
during execution. P0.1 is *configure and populate*, not create.

| Item | Detail |
|---|---|
| Description + topics | Set to an honest pre-alpha unified-arr description with `sonarr`, `radarr`, `prowlarr`, `bittorrent`, `trash-guides`, and `media-automation` topics. |
| Repo name | D7 resolved: StackArr product/crates; stable `NGMS` repository and GHCR identifiers (§10.4). |
| Source of truth | **GitHub owns code, issues and CI.** Forgejo keeps a mirror and the private deploy path only. Do not run two issue trackers. |
| Stale local checkout | The legacy shared checkout is a disjoint 43-commit branch (§2.2). Workspace policy requires it to remain untouched pending a separate deployment/deprecation decision. Work instead in a clean canonical GitHub checkout. |
| Licence | GPL-3.0-only per D3; executed by T3/T27. |

#### P0.2 — Documentation set

Written before the first issue, so contributors arrive to a repo that explains itself.

| File | Contents |
|---|---|
| `README.md` | What it is, **honest status (pre-alpha, not usable yet)**, the compatibility promise, the Appendix A definition of done as a public checklist |
| `LICENSE` | GPL-3.0 (§4.1) |
| `CONTRIBUTING.md` | The §6.2 TDD standard as binding policy: red-first from spec, golden files, mandatory gates, no `#[allow]` without a named reason |
| `CODE_OF_CONDUCT.md` | Contributor Covenant |
| `SECURITY.md` | Disclosure path — this software holds indexer and download-client credentials |
| `AGENTS.md` / `CLAUDE.md` | Corrected. All three false claims from §2.5 fixed, plus MariaDB. |
| `docs/UNIFIED-ARR-PLAN.md` | This document |
| `docs/API-COMPATIBILITY.md` | New. Target versions, what is and is not implemented, per-client support matrix |
| `docs/TESTING.md` | New. How to run each tier; how to add a conformance golden file |
| `docs/DATABASE.md` | Rewritten for MariaDB + single baseline schema |
| `docs/ARCHITECTURE.md` | Updated for the §5 crate layout |

#### P0.3 — Guardrails

The point of this sub-phase is that none of §2.5 can silently recur.

| Guardrail | Enforces |
|---|---|
| Branch protection on `main` | No direct push; PR + green CI required; linear history |
| Required checks | `lint`, `test`, `conformance`, `build` (§6.3) |
| `CODEOWNERS` | Self for now; makes review routing explicit when contributors arrive |
| `.github/workflows/ci.yml` | The §6.3 pipeline — the first real build the project has had |
| `renovate.json` | Extended to crates.io `nzb-*`. **This is the specific control that would have caught the four-month Usenet drift.** |
| **`no-vendored-crates` CI check** | Fails the build if a `crates/` subdirectory duplicates a published dependency. The direct lesson of §2.4. |
| **Doc-drift check** | CI asserts `CLAUDE.md`'s crate list and workspace-member claims match `Cargo.toml`. The direct lesson of §2.5. Lands in P1 as T29, once T6 has corrected the file. |
| Coverage ratchet | `coverage-watchdog`; the number may not go down |
| Issue templates | `bug`, `feature`, `compat-gap`, `provider`, `decision` |
| PR template | Checklist: tests added, gates pass, docs updated, no new vendored code |

#### P0.4 — Tracking structure

**Milestones** — `P0`…`P7`, **already created** (numbers 1–8).

**Labels — already created (31).** For reference:

```
phase:p0 … phase:p7
area:compat-sonarr  area:compat-radarr  area:compat-prowlarr  area:compat-core
area:db  area:parser  area:decision  area:quality  area:trash  area:indexer
area:download  area:import  area:metadata  area:ci  area:docs  area:ui
type:epic  type:task  type:bug  type:decision  type:spike  type:chore
risk:high  good-first-issue  help-wanted  blocked
```

**Project (Projects v2)** — board with custom fields `Phase`, `Area`, `Size` (XS–XL),
`Risk`, `Spec source` (which of the three §3 specs governs the item).

#### P0.5 — Initial work items

The full enumerated backlog is in [Appendix F](#appendix-f--initial-github-issue-backlog):
**9 decision issues, 8 phase epics, and 59 concrete tasks — 76 items**, with full bodies
and acceptance criteria in the machine-readable manifest `docs/backlog.json`.

Two rules for the backlog:

1. **Decisions are issues.** Every item in §4 and §10 becomes a `type:decision` issue that
   blocks its dependent work. D1/D2/D3/D4/D6/D7 are resolved; later decisions retain
   their explicit phase dependencies.
2. **P3's issues are generated, not written.** The conformance harness (P2) emits one issue
   per unimplemented endpoint from the 419 OpenAPI paths, ranked by recorded real-client
   traffic. Hand-writing them now would be guessing — the whole point of P2 is that
   priority is measured. Appendix B stays a placeholder until then.

**Exit:** repo public with green CI; branch protection active; all 76 items created,
labelled, milestoned and on the board; §10.3 and §10.9 closed.

---

### P1 — Consolidation (the cheapest, highest-ratio work)

No new features. Shrink and correct the foundation.

| Task | Detail | Δ LOC |
|---|---|---|
| Delete `crates/torrent/` | Dead — not a workspace member, never compiles. Torrent comes from crates.io `swarmforge`. Verify no path refs first. | −44,197 |
| ~~Delete `crates/usenet/`~~ | **Already done.** All seven `nzb-*` crates pinned from crates.io (§2.4). No action. | — |
| **Delete all migrations** | 18 files → one `001_baseline.sql`. Fresh deploy, no upgrade path to preserve. | — |
| **Design the target schema** | Not a translation. Bake in the media-type-generic model (§5), P5 profile-provenance tables, P6 decision records — while it is still free. | — |
| **Postgres → MariaDB** | 1,420 placeholders, 78 `RETURNING`, 77 `ON CONFLICT`, 153 `PgPool` refs, 56 `jsonb`. Zero `query!` macros. `stackarr-postgres` becomes `stackarr-mariadb` — but this is a **rewrite, not a rename**: that crate is 1,216 LOC of embedded-Postgres provisioning (download binaries, `initdb`, supervise a child process) with no query code in it, and MariaDB has no drop-in equivalent. Whether StackArr still ships a self-provisioning database is an open product decision. See §4.2. | ~1,800 touched |
| Relicense | MIT → GPL-3.0 across workspace + headers | — |
| Correct `CLAUDE.md` | Three false claims (§2.5) + the database change | — |
| Toolchain | Add `rust-toolchain.toml`; unpin CI from 1.88 | — |
| `justfile` | Mirror rdpapp targets | — |
| GitHub Actions | **Extend** the existing pipeline (§6.3): conformance job, coverage ratchet, MariaDB service, multi-arch build | — |
| Renovate/Dependabot | Confirm the crates.io `nzb-*` and `swarmforge` pins are watched — they are `=`-pinned, so nothing bumps them automatically | — |

**Result: ~70,300 lines, down from 114,500 — a 39% reduction with zero functionality lost.**
The Usenet dividend has already been banked upstream; what remains here is the dead torrent
tree and the database swap.

**Ordering within P1 matters.** Do the deletions first (they shrink the surface the
database swap has to cross), then the schema design, then the dialect swap, then CI last so
it validates the finished state. Specifically: the 44k dead torrent lines are removed *before* anyone
counts queries to convert.

**Exit:**
- green CI on GitHub Actions including a multi-arch build
- `cargo tree` shows no path dependency on a vendored engine
- all 1,060 tests pass against MariaDB
- a fresh `docker run` reaches `/health` from an empty database using only
  `001_baseline.sql`

**Risk:** low-to-medium. The deletions are near-zero risk. The database swap is the real
content of this phase — 1,420 placeholder conversions is where a silent bug hides, which is
why the conversion script must fail loudly on repeated or non-monotonic `$n` (§4.2) and why
the 1,060 existing tests are the gate. The workspace has also never been verified to build in
this session — see §10.1.

### P2 — Conformance harness

The measuring instrument. Nothing after this is guesswork.

- Record real HTTP traffic from the live Sonarr/Radarr/Prowlarr instances (and from
  Overseerr, Bazarr, Recyclarr, nzb360, Homepage hitting them) — a capturing proxy.
- Store as versioned golden files under `contracts/arr-v3/`.
- Build a replay harness: fire recorded requests at NGMS, diff JSON structurally
  (schema + shape, tolerant of ids/timestamps).
- Generate a failing test per `openapi.json` path — 419 across the three specs — all red.
- **Produce the ranked backlog**: intersect "what real clients actually call" with the
  missing-resource matrix (§2.3). This converts "port everything" into a finite ordered
  list, and is the single most valuable output of the phase.
- Fixtures already available: `myotherrepos/StackArr/test-fixtures/sonarr_backup.zip` and
  `radarr_backup.zip`.

**Exit:** `just conformance` runs, reports a coverage percentage, and the ranked backlog
exists as a document.

### P3 — Read-only compatibility

Make the ecosystem *see* NGMS.

- `stackarr-compat-core`: auth (header + querystring), error shapes, `ProviderResource` field
  reflection, SignalR hub.
- `stackarr-compat-sonarr-v3` / `-radarr-v3` / `-prowlarr-v1`: all GET endpoints.
- Priority order comes from P2, but expect: `system/status`, `rootfolder`, `diskspace`,
  `health`, `series`/`movie`, `episode`, `qualityprofile`, `customformat`, `tag`,
  `queue`, `history`, `calendar`, `parse`.
- `customformat` + `qualitydefinition` are first among equals — they are the TRaSH hinge,
  and the engine half already exists in `stackarr-quality/src/custom_formats.rs` (479 LOC) with
  no API surface on it.

**Exit:** Overseerr, Bazarr and Homepage connect and display correct data against NGMS
unmodified. Recyclarr can read.

### P4 — Write path and migration

- POST/PUT/DELETE across the façades.
- `manualimport`, `rename`, `command` (the arr task-queue endpoint), `release` (grab).
- Harden `stackarr-migrate`: real Sonarr/Radarr/Prowlarr DBs in, verified state out. Add the
  SABnzbd importer already sitting in `nzb-core::sabnzbd_import`.
- Expose embedded engines as SABnzbd/qBittorrent-compatible clients (§5.1).

**Exit:** a real Sonarr + Radarr + Prowlarr install migrates in one command and continues
operating. Recyclarr can write. nzb360 can manage.

### P5 — TRaSH + Profilarr native

Where the differentiated value is, and where nothing upstream can follow.

- **Subscribed profiles as first-class objects**: upstream ref + local overrides + **3-way
  merge** on update, with diff preview and changelog. Recyclarr and Configarr clobber; we
  merge. This is the headline.
- **Provenance on every custom format**: "from TRaSH `2026-07-14`, score overridden
  500 → 350."
- **Git-backed profile packs** — Profilarr's real contribution, as a core sync source
  rather than a sidecar service.
- **Compiled scoring**: `RegexSet`/Aho-Corasick over the whole TRaSH set in one pass,
  replacing N formats × M regexes per release per indexer per RSS cycle. This is a genuine
  hot path and it scales with how seriously a user takes TRaSH.
- **Simulation** — the feature nobody else can build: *"show me what would have been
  grabbed differently over the last 90 days if I applied this profile change."* We have
  history and the release cache in one database. Recyclarr cannot do this. Sonarr cannot
  do this.
- Port `myotherrepos/ArrProfileGenerator` (ProfSync) wizard logic — TRaSH profile
  generation already solved in Python, needs translating not designing.

**Exit:** a user can subscribe to a TRaSH profile, take an upstream update without losing
local edits, and preview the historical impact before applying.

### P6 — Decision engine and parser parity

The correctness core. Deliberately after compatibility, because P2's harness is what makes
it verifiable.

- Port Sonarr's **30 DecisionEngine specifications**, order-sensitive.
- Port the 4,644 LOC parser test corpus *first*, then grow `stackarr-parser` (currently 1,155
  LOC vs Sonarr's 4,519) to pass it. Expect this to be the largest single body of work in
  the plan.
- **Explainable decisions**: emit a structured, replayable object with the full per-spec
  score breakdown, and expose it as an API resource. Sonarr's rejection reasons are strings
  in a log; "why didn't it grab this?" is the single most common user complaint in the arr
  community. This is a headline feature disguised as a refactor.
- Metadata: TVDB/TMDB direct, **scene numbering and XEM mapping**. Budget as a project in
  itself — this is where naive rewrites break and never recover.

**Exit:** the ported arr test corpus passes.

### P7 — Unification dividends

Only possible once one core owns everything.

- **Delete Prowlarr's `Applications/` subsystem** — no `AppIndexerMap`, no sync command, no
  sync levels, no drift. An indexer is defined once.
- **Deduplicated indexer traffic** — one query planner, one cache, one rate limiter across
  all media types. Today Sonarr and Radarr independently hammer the same tracker with
  separate limiters. Fewer requests, materially lower ban risk.
- **One download queue, one disk budget, one bandwidth budget** — today the arrs fight each
  other for client slots and free space.
- **Cross-seed internally** — unified file index + embedded torrent engine makes matching
  existing files against a new tracker an internal query, not an external daemon.
- **Shared release cache with failure memory** — "seen this hash, failed 3× on import,
  don't re-grab."
- **Media types as a type system, not forks** — anime becomes first-class rather than a
  hack on the series model; music/books become plugins rather than Lidarr/Readarr forks.
- **Global dry-run** for naming, profile changes, upgrade sweeps.
- **Declarative notification providers** (§7.5 below).

### 7.5 Collapsing the provider long tail

~100 unique providers is the majority of the boring work and it never stops, because
upstream keeps adding. Mitigations, in order of leverage:

| Category | Count | Strategy |
|---|---|---|
| Torrent indexers | ~47×3 | **Already solved** — Cardigann YAML, 549 definitions in tree |
| Usenet indexers | — | Newznab/Torznab generic |
| Notifications | ~47×3 | **Make declarative.** Most are an HTTP POST with a body template. YAML/JSON templates, not 47 Rust structs. |
| Import lists | ~35×3 | Mostly declarative too (Trakt/IMDb/TMDb list fetch + parse) |
| Download clients | ~21×3 | Hand-written, but only ~8 matter (qBit, Deluge, Transmission, rTorrent, SAB, NZBGet, + embedded) |

Making notifications and import lists **data rather than code** collapses roughly 60% of
the tail.

---

## 8. Risk register

| # | Risk | Severity | Mitigation |
|---|---|---|---|
| 1 | **Scope.** NGMS is already 6 products fused; TRaSH/Profilarr widens it | **Highest** | §4.3 freeze list. Enforce at review. Scope is what kills this, not Rust. |
| 2 | **Metadata / scene numbering / XEM.** Where naive rewrites die | High | Treat as its own project in P6. Consider running a Skyhook equivalent. |
| 3 | **Parser parity.** 1,155 LOC vs Sonarr's 4,519 + 12 years of edge cases | High | Port the 4,644 LOC test corpus first. Never write parser code without a failing test from it. |
| 4 | **Upstream drift.** Sonarr v5 (`Api.V5`) already in-tree; 3 moving targets | Medium | Pin v3 as the compatibility target. v5 is a later façade, not a parallel one. |
| 5 | **Bus factor of one.** 43 commits, single branch, one contributor | Medium | Public GitHub (§6.3) is the mitigation. So is test density. |
| 6 | **Database swap introduces silent data bugs.** 1,420 placeholder rewrites, 78 `RETURNING` sites | Medium | Conversion script fails loudly on repeated/non-monotonic `$n`; 1,060 tests are the gate; hand-review every `RETURNING` site. §4.2. |
| 6b | **MariaDB still needs a server process** — the NAS "just works" story | Medium | Ship MariaDB in-container via s6-overlay. Keep new code free of `RETURNING` so a SQLite backend stays cheap later. §10.8. |
| 7 | **Re-vendoring temptation.** The Usenet fork happened once | Medium | Principle §3.4. Renovate. Never `version.workspace = true` on a vendored crate. |
| 8 | **Build unverified.** Modified `Cargo.lock`, private registry deps, no `target/` | Medium | First action of P1. |
| 9 | **Licence contamination** discovered late | Low but severe | Resolve in P1 (§4.1). |
| 10 | **SignalR underestimated** | Low | Scope it explicitly into `stackarr-compat-core`, not "later". |

---

## 9. Effort shape

Not a schedule — a shape. Relative weights.

| Phase | Weight | Character |
|---|---|---|
| P0 Repo + guardrails | ▏ | Days. Governance, documentation, CI, and tracking. |
| P1 Consolidation | ▍ | Deletion is days. The MySQL swap and schema redesign are the real content — call it a couple of weeks, and do not rush the 1,420 placeholder conversions. |
| P2 Conformance harness | ▎ | Tooling. Pays for everything after. |
| P3 Read-only compat | ▍▍ | Broad, shallow, highly parallel. |
| P4 Write path + migration | ▍▍ | Broad, medium depth. |
| P5 TRaSH/Profilarr | ▍▍▍ | The differentiated work. |
| P6 Decision engine + parser | ▍▍▍▍▍ | **The bulk.** Deep, correctness-critical, test-led. |
| P7 Dividends | ▍▍ | Incremental, individually shippable. |

After deduplication, dropping the frontends and skipping migration history, genuine core
parity is roughly 120–180k lines of Rust plus the provider tail — against ~47.5k that
already exists. A credible 80/20 (TV + film + indexers + TRaSH-native, ecosystem
compatible) is a focused multi-month effort. Full parity down to every provider and every
twelve-year-old edge case is a year-scale commitment. **Scope to the 80/20 and let the
conformance harness say when it is reached.**

---

## 10. Open questions

1. ~~**Does the workspace build?**~~ **RESOLVED 2026-08-02:** a clean canonical clone
   completed `cargo build --workspace` on the stock execution host before P1 changes.
2. ~~**Does `librtbit` want to go to crates.io?**~~ **RESOLVED: already published** as
   `swarmforge`, aliased to the `librtbit` names in `Cargo.toml`. Public CI needs no
   private registry credentials.
3. ~~**Public repo — which org?**~~ **RESOLVED 2026-08-02: `TheDancingDeveloper-org`.**
   The repo already exists there, public, with 499 commits and GitHub Actions.
   The former source organization is not the canonical route for this repository.
4. ~~**Name.**~~ **RESOLVED 2026-08-02:** StackArr is the product and crate family;
   `NGMS` and its GHCR path remain stable distribution identifiers. The README and
   repository guidance document the deliberate divergence.
5. **Does Indexarr merge in or stay separate?** `stackarr-indexer/src/indexarr.rs` (152 LOC)
   integrates it today. Overlaps heavily with the Prowlarr replacement story.
6. **Reported version string** for `/api/v3/system/status` — clients gate features on it.
7. **Multi-instance semantics.** Sonarr users often run two instances (e.g. 1080p and 4K).
   Does one NGMS present as two Sonarrs, or does the unified model make that obsolete?
   Affects the façade's instance-identity design in P3.
8. **How is MariaDB delivered to end users?** Bundled in-container via s6-overlay
   (recommended — preserves one-command install), external only, or both? Sonarr and Radarr
   need no external database at all; whatever we choose must not make NGMS harder to try
   than the thing it replaces. See §4.2.
9. ~~**MariaDB or MySQL 8?**~~ **RESOLVED 2026-08-02: MariaDB 11.4 LTS**, with new code
   kept `RETURNING`-free for portability. See §0.2b.

---

## Appendix A — Definition of done for v1

Objectively checkable, no interpretation required:

- [ ] Overseerr adds a series and a movie, sees them appear, tracks availability
- [ ] Bazarr discovers the library and fetches subtitles
- [ ] Recyclarr syncs a TRaSH config without error
- [ ] nzb360 connects, browses, manages the queue (validates SignalR)
- [ ] Homepage/Homarr widgets show correct counts
- [ ] A real Sonarr + Radarr + Prowlarr install migrates in one command
- [ ] Single container, no external download client required
- [ ] Memory under load < 150 MB RSS

## Appendix B — Ranked missing-resource list

Placeholder. **Generated by P2** from the intersection of the §2.3 matrix with recorded
real-client traffic. Not written by hand — the whole point is that priority is measured,
not guessed.

## Appendix C — Assets already in the workspace

| Asset | Location | Use |
|---|---|---|
| Sonarr/Radarr/Prowlarr source + OpenAPI + tests | `Active/RefenceMaterials/reference/External Repos/` | The three specs |
| Arr DB fixtures | `myotherrepos/StackArr/test-fixtures/{sonarr,radarr}_backup.zip` | Migration + conformance |
| TRaSH profile generator (Python) | `myotherrepos/ArrProfileGenerator` (ProfSync) | Port into `stackarr-profiles` (P5) |
| Usenet engine | crates.io `nzb-*` | Dependency, not fork |
| Torrent engine | crates.io `swarmforge` (aliased to `librtbit` names) | Dependency, already |
| Coverage tooling | `Active/apps/coverage-watchdog` | P1 ratchet |
| CI reference pipeline | `Active/rdpapp/.woodpecker/ci.yml` | Job structure worth mirroring |
| Woodpecker failure modes | `Active/rdpapp/ciblock.md` | Why the homelab CI path stays minimal |

## Appendix D — Deletion checklist for P1

```
crates/torrent/                     44,197 LOC  — dead, not a workspace member
crates/usenet/                      16,869 LOC  — replaced by crates.io nzb-web
migrations/001..011_*.sql              647 SQL  — fresh deploy; → one 001_baseline.sql
Cargo.toml: 5 usenet workspace members
Cargo.toml: nzb-* path deps → nzb-web = "0.4.21"
Cargo.toml: sqlx features "postgres" → "mysql"
Cargo.toml: license MIT → GPL-3.0
imports in stackarr-download, stackarr-web  — 17 symbols, all verified present upstream
131 PgPool/Postgres type refs        22 files  — → MySqlPool
1,420 $n placeholders                        — → ?  (script must fail on repeats)
 63 RETURNING clauses                          — → LAST_INSERT_ID() / re-select
 55 ON CONFLICT                                — → ON DUPLICATE KEY UPDATE / INSERT IGNORE
 36 jsonb refs                                 — → JSON
CLAUDE.md: 3 false claims + database change
```

## Appendix E — How the numbers were measured

```bash
REF="Active/RefenceMaterials/reference/External Repos"

# Reference sizes
find $REF/Sonarr/src -name '*.cs' -not -name '*.Test.cs' | xargs cat | wc -l
python3 -c "import json;d=json.load(open('$REF/Sonarr/src/Sonarr.Api.V3/openapi.json'));\
print(len(d['paths']),len(d['components']['schemas']))"

# NGMS
find Active/apps/NGMS -name '*.rs' -not -path '*/target/*' | xargs cat | wc -l
grep -rn '#\[test\]\|#\[tokio::test\]' --include='*.rs' crates src | grep -v '/torrent/\|/usenet/' | wc -l
grep -rho '"/api/v[0-9]*/[a-z0-9/_:{}.-]*"' crates/stackarr-web/src | sort -u | wc -l

# Usenet fork delta
git -C Active/apps/NGMS log --numstat -- crates/usenet
diff -r Active/apps/NGMS/crates/usenet/nzb-core/src Active/apps/rustnzbd/crates/nzb-core/src

# crates.io
curl -s https://crates.io/api/v1/crates/nzb-web | python3 -m json.tool

# Database coupling (§4.2)
ls migrations | wc -l ; cat migrations/*.sql | wc -l
grep -rho '\$[0-9]\+'            --include='*.rs' crates src | wc -l   # 1420
grep -rhoi 'returning'           --include='*.rs' crates src | wc -l   # 78
grep -rhoi 'on conflict'         --include='*.rs' crates src | wc -l   # 77
grep -rho  'PgPool\|sqlx::Postgres\|postgres::' --include='*.rs' crates src | wc -l  # 173
grep -rho  'sqlx::query[_a-z]*!' --include='*.rs' crates src | wc -l   # 0 — no macros
grep -rhoi 'jsonb\|serial\|gen_random' migrations/*.sql | tr 'A-Z' 'a-z' | sort | uniq -c
```

---

## Appendix F — Initial GitHub issue backlog

**76 items: 9 decisions, 8 epics, 59 tasks.** The authoritative, machine-readable manifest
is **`docs/backlog.json`** — title, body, labels and milestone for every item. This appendix
is the human-readable index of it.

Labels (31) and milestones (`P0`–`P7`, numbers 1–8) **already exist on the repo** — see
§0.2. Do not recreate them.

P3–P7 are deliberately coarser than P0–P1: fine-grained compatibility issues are *generated*
by the P2 harness (T34) from measured client traffic, not guessed at now. That is also why
Appendix B is a placeholder.

### Creation script

```bash
R=TheDancingDeveloper-org/NGMS
python3 - <<'EOF' > /tmp/mk-issues.sh
import json
for it in json.load(open('docs/backlog.json')):
    labels = ','.join(it['labels'])
    body = it['body'].replace("'", "'\''")
    title = it['title'].replace("'", "'\''")
    print(f"gh issue create -R $R --title '{title}' --body '{body}' "
          f"--label '{labels}' --milestone '{it['milestone']}'")
EOF
bash /tmp/mk-issues.sh
```

Run it once, from the repo root of a fresh clone. Verify with
`gh issue list -R $R --limit 100 | wc -l` → 76.

### Index

#### P0 — Repository, guardrails and work tracking (20 items)

| ID | Title | Type | Risk |
|---|---|---|---|
| D1 | Confirm repo identity and org | decision |  |
| D2 | MariaDB 11.x or MySQL 8 as the pinned target [RESOLVED: MariaDB 11.4 LTS] | decision | ⚠️ |
| D3 | Relicense MIT to GPL-3.0 [RESOLVED: GPL-3.0] | decision | ⚠️ |
| D4 | Ratify the product scope freeze list [RESOLVED: scope freeze ratified] | decision | ⚠️ |
| D6 | Update policy for `=`-pinned crates.io dependencies | decision |  |
| D7 | Align project naming | decision |  |
| E0 | Repository, guardrails and work tracking | epic |  |
| T1 | Configure repo metadata and retire the stale checkout | task |  |
| T2 | README: honest status, compatibility promise, public DoD | task |  |
| T3 | Add LICENSE (GPL-3.0) | task |  |
| T4 | CONTRIBUTING.md encoding the TDD standard | task |  |
| T5 | CODE_OF_CONDUCT.md and SECURITY.md | task |  |
| T6 | Correct CLAUDE.md / AGENTS.md drift | task |  |
| T7 | docs/API-COMPATIBILITY.md | task |  |
| T8 | docs/TESTING.md | task |  |
| T9 | Branch protection, required checks, CODEOWNERS | task |  |
| T10 | Issue and PR templates | task |  |
| T11 | Ensure dependency automation watches the pinned engine crates | task | ⚠️ |
| T12 | Extend the dependency-boundary CI check to forbid re-vendoring | task | ⚠️ |
| T13 | Create the Projects v2 board | task |  |

#### P1 — Consolidation (18 items)

| ID | Title | Type | Risk |
|---|---|---|---|
| D5 | How is MariaDB delivered to end users | decision |  |
| E1 | Consolidation: delete dead code, MariaDB swap, baseline schema | epic |  |
| T14 | Verify a clean clone builds on a stock runner | task |  |
| T15 | Delete crates/torrent/ | task |  |
| T16 | Audit the `=`-pinned crates.io dependencies | task |  |
| T17 | Add rust-toolchain.toml | task |  |
| T18 | Add a justfile | task |  |
| T19 | Extend ci.yml | task |  |
| T20 | Design the target baseline schema | task | ⚠️ |
| T21 | Collapse 18 migrations to one baseline | task |  |
| T22 | Swap sqlx driver and rename stackarr-postgres | task |  |
| T23 | Convert 1,420 positional placeholders | task | ⚠️ |
| T24 | Rewrite 78 RETURNING sites | task | ⚠️ |
| T25 | Rewrite 77 ON CONFLICT clauses | task |  |
| T26 | Convert JSON and identity column types | task |  |
| T27 | Apply GPL-3.0 headers | task |  |
| T28 | Wire coverage-watchdog as a ratchet | task |  |
| T29 | CI doc-drift check | task |  |

#### P2 — Conformance harness (6 items)

| ID | Title | Type | Risk |
|---|---|---|---|
| E2 | Conformance harness and measured backlog | epic |  |
| T30 | Capturing proxy for live arr traffic | task |  |
| T31 | Golden-file store at contracts/arr-v3/ | task |  |
| T32 | Replay and structural-diff harness | task |  |
| T33 | Generate a failing test per OpenAPI path | task |  |
| T34 | Ranked backlog generator | task |  |

#### P3 — Read-only compatibility (11 items)

| ID | Title | Type | Risk |
|---|---|---|---|
| D8 | Does Indexarr merge in or stay separate | decision |  |
| D9 | Multi-instance semantics and reported version string | decision |  |
| E3 | Read-only arr API compatibility | epic |  |
| T35 | stackarr-compat-core skeleton | task |  |
| T36 | Arr authentication | task |  |
| T37 | ProviderResource field reflection | task | ⚠️ |
| T38 | SignalR hub | task | ⚠️ |
| T39 | Sonarr v3 façade — GET endpoints | task |  |
| T40 | Radarr v3 façade — GET endpoints | task |  |
| T41 | Prowlarr v1 façade — GET endpoints | task |  |
| T42 | qualitydefinition resource | task |  |

#### P4 — Write path and migration (5 items)

| ID | Title | Type | Risk |
|---|---|---|---|
| E4 | Write path, migration and download-client compatibility | epic |  |
| T43 | Write path across all three façades | task |  |
| T44 | manualimport, rename, command, release | task |  |
| T45 | Harden the migration importers | task |  |
| T46 | Expose embedded engines over legacy download-client protocols | task |  |

#### P5 — TRaSH + Profilarr (7 items)

| ID | Title | Type | Risk |
|---|---|---|---|
| E5 | TRaSH + Profilarr native | epic |  |
| T47 | stackarr-profiles crate | task |  |
| T48 | Three-way merge on upstream profile update | task |  |
| T49 | Custom-format provenance | task |  |
| T50 | Compiled custom-format scoring | task |  |
| T51 | Profile-change simulation | task |  |
| T52 | Port ProfSync wizard logic | task |  |

#### P6 — Decision engine and parser (5 items)

| ID | Title | Type | Risk |
|---|---|---|---|
| E6 | Decision engine and parser parity | epic |  |
| T53 | Port the Sonarr parser test corpus first | task | ⚠️ |
| T54 | Port the 30 DecisionEngine specifications | task | ⚠️ |
| T55 | Explainable decisions | task |  |
| T56 | Metadata: TVDB/TMDB direct, scene numbering, XEM | task | ⚠️ |

#### P7 — Unification dividends (4 items)

| ID | Title | Type | Risk |
|---|---|---|---|
| E7 | Unification dividends | epic |  |
| T57 | Unified indexer query planner | task |  |
| T58 | Declarative notification and import-list providers | task |  |
| T59 | Unified download queue and resource budgets | task |  |
### Blocking order

```
RESOLVED 2026-08-02:  D2 = MariaDB 11.4 LTS   D3 = GPL-3.0   D4 = scope freeze ratified
   └─> P1 is unblocked; T3 and T27 are unblocked

RESOLVED 2026-08-02:
D1 = retain TheDancingDeveloper-org/NGMS and the existing GHCR path
D6 = exact pins plus grouped weekly dependency-update pull requests
D7 = StackArr product/crates; stable NGMS repository/image identifiers

T6 (fix CLAUDE.md) ──> T29 (enforce in CI)
T15 (delete torrent) ──> T12 check passes
T20 (schema design) ──> T21..T26 (the swap)
T30..T33 (harness) ──> T34 (ranked backlog) ──> all P3 issue generation
D9 ──> T39, T40, T41
```
