# The façade rule

> A compat crate contains DTOs, route wiring and translation **only**. Any logic
> that appears in a façade belongs in the core.

This is the difference between one product and three. Sonarr v3, Radarr v3 and
Prowlarr v1 compatibility are three views of the same StackArr domain. The
moment a façade decides something for itself, that decision exists in one view
and not the others, and the three views start drifting apart — silently, one
endpoint at a time.

## The crates

| Crate | Role |
| --- | --- |
| `stackarr-compat-core` | Shared arr concerns: authentication, error shapes, `ProviderResource` field reflection, the SignalR hub. |
| `stackarr-compat-sonarr-v3` | Thin façade. Not yet created. |
| `stackarr-compat-radarr-v3` | Thin façade. Not yet created. |
| `stackarr-compat-prowlarr-v1` | Thin façade. Not yet created. |

A *façade* is any `stackarr-compat-*` crate other than `stackarr-compat-core`.
`stackarr-compat-core` is the one compat crate allowed to hold behavior, and
only behavior that is arr-generic rather than domain-specific: how a request is
authenticated, how an error is shaped, how a hub message is framed.

## What belongs in a façade

- Serde types matching the arr wire contract, field for field, including
  casing, nullability and ordering.
- Route registration, path prefixes, per-façade API keys and instance identity.
- Translation between those wire types and StackArr domain types.
- Tests that pin the wire contract: golden-file comparisons and generated
  conformance tests.

## What does not

- Filtering, sorting, ranking, scoring, matching or eligibility decisions.
- Quality, custom-format, profile, naming or release-decision behavior.
- Database access, migrations, transactions or caching policy.
- Talking to an indexer, download client or metadata provider.
- Defaults that a client will read as data. If a value has to be *chosen*
  rather than *converted*, the core chooses it and the façade reports it.

A translation may be tedious — an enum with twenty arms, a shape that needs
flattening — without being logic. The test is whether the same request against a
different façade would need the same decision made again. If it would, it
belongs in the core.

## How it is enforced

Three layers, deliberately overlapping:

1. **Review.** The pull request template carries the rule as a checkbox, and it
   is a legitimate, expected reason to reject a change.
2. **A test.** `stackarr_compat_core::facade_rule` holds the mechanical part of
   the rule as data, and the crate's `facade_rule` integration test applies it to
   every workspace member on every `cargo test` run. A façade may depend on
   `stackarr-compat-core` and `stackarr-core`; anything else in the
   `stackarr-*` family fails the build, as does a direct dependency on storage
   (`sqlx`, `rusqlite`) or an embedded engine (`librtbit`, `nzb-web`,
   `nzbdav-core`).
3. **Conformance.** Façade behavior is pinned to recorded arr responses, so
   logic invented inside a façade shows up as a wire-shape diff.

The test catches the boundary being crossed in a manifest, which is where it is
cheapest to catch. It cannot catch a `match` arm that quietly invents a rule —
that is what layers 1 and 3 are for.

## Changing the allowlist

`FACADE_WORKSPACE_DEPENDENCIES` in
`crates/stackarr-compat-core/src/facade_rule.rs` is short and editable on
purpose. Widening it is sometimes correct: a shared concern may genuinely grow a
new home in the core. It is a one-line diff in a file whose only job is this
rule, so it arrives in review as a visible decision rather than buried in a
manifest.

Before widening it, check the alternative: the thing the façade wants usually
belongs behind `stackarr-core`, or is a shared arr concern that belongs in
`stackarr-compat-core` where all three façades get it at once.

See [UNIFIED-ARR-PLAN.md](UNIFIED-ARR-PLAN.md) §5 for the target architecture
and [API-COMPATIBILITY.md](API-COMPATIBILITY.md) for the pinned wire contracts.
