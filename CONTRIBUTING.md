# Contributing to StackArr

StackArr is pre-alpha. Compatibility and data-integrity changes are accepted
only with executable evidence against the governing specification.

## Before opening a change

1. Find or open the GitHub issue that defines the requirement and acceptance
   criteria.
2. Identify the governing source: arr OpenAPI, arr behavioral tests, TRaSH
   Guides data, or an internal StackArr contract.
3. Keep frozen subsystems frozen: `stackarr-stream`, Stremio routes, and
   `stackarr-bootstrap` accept maintenance fixes and tests but no new features
   before P5 is complete.
4. Do not vendor a published crate. Changes to `nzb-*` or `swarmforge` belong
   upstream and arrive through a reviewed dependency update.

## Test-driven development is required

- Start red. Add the smallest failing test from the specification before the
  implementation.
- For compatibility endpoints, generate request, response, and status-code
  assertions from the checked-in OpenAPI specification before adding a handler.
- Treat `contracts/arr-v3/` golden files as versioned wire contracts. A shape
  difference must be reviewed; never update a golden file merely to make CI
  green.
- Port behavioral test cases from the arr test corpus. Do not translate or copy
  implementation code.
- Use property tests for hostile input spaces such as release parsing and
  custom-format scoring.
- Façade crates contain DTOs, route wiring, and translation only. Domain logic
  belongs in the core.
- A new `#[allow(...)]` must have an adjacent comment that names the concrete
  reason it is necessary.

## Required gates

The repository toolchain is pinned in `rust-toolchain.toml`. Before every
commit, run:

```bash
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
```

When a change affects the UI, compatibility contracts, or container runtime,
also run the applicable tier documented in [docs/TESTING.md](docs/TESTING.md).
The `justfile` provides the canonical task names as each phase lands.

## Pull requests

Use a focused branch and the pull request template. Link the issue, name the
specification, include the red-to-green test evidence, and call out database or
wire-format changes. Pull requests require the protected-branch checks and a
review. Do not add AI co-author trailers or list an AI system as a contributor.

By participating, you agree to follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
