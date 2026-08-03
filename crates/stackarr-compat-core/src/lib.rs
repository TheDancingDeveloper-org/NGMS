//! Shared foundation for the arr compatibility façades.
//!
//! The Sonarr v3, Radarr v3 and Prowlarr v1 façades are three views of one
//! product. Everything they have in common — authentication, error shapes,
//! `ProviderResource` field reflection and the SignalR hub — belongs here, and
//! everything that decides anything belongs in the core crates behind them.
//!
//! # The façade rule
//!
//! **A compat crate contains DTOs, route wiring and translation only. Any logic
//! that appears in a façade belongs in the core.**
//!
//! That rule is what keeps StackArr one product instead of three. It is
//! documented in full in `docs/COMPAT-FACADE-RULE.md` and expressed as
//! checkable data in [`facade_rule`], which the `facade_rule` integration test
//! applies to every workspace member.
//!
//! # Scope of this crate
//!
//! This crate is a skeleton. It currently carries the façade rule itself; the
//! shared arr concerns land on top of it:
//!
//! - authentication — `X-Api-Key` header, `?apikey=` querystring, forms-auth
//!   cookie;
//! - arr error response shapes and status codes;
//! - `ProviderResource` field reflection, preserving option shape, privacy,
//!   visibility and ordering; and
//! - the SignalR negotiate handshake and JSON hub protocol.
//!
//! Each arrives with the golden files and generated conformance tests that
//! govern it, never ahead of them.

#![warn(missing_docs)]

pub mod facade_rule;
