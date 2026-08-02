## Summary

<!-- What requirement does this change satisfy? -->

Closes #

## Governing specification

<!-- OpenAPI path, arr behavioral test, TRaSH data, or internal contract. -->

## Evidence

- [ ] A failing test was added before the implementation.
- [ ] New or changed logic has unit tests; I/O boundaries have integration tests.
- [ ] GUI behavior has golden-path and key-edge-case E2E coverage where applicable.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --workspace --all-features -- -D warnings` passes.
- [ ] `cargo test --workspace --all-features` passes.
- [ ] Relevant compatibility/conformance fixtures pass.
- [ ] Documentation and checked-in contracts are updated.
- [ ] No published crate was vendored and no private dependency source was added.
- [ ] No frozen or deferred subsystem was widened.

## Compatibility or data impact

<!-- Describe wire-shape, migration, schema, or operational impact; write “none” if absent. -->
