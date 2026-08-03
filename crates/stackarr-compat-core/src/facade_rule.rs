//! The façade rule, expressed as data a test can check.
//!
//! A façade crate — any `stackarr-compat-*` member other than
//! `stackarr-compat-core` — contains DTOs, route wiring and translation only.
//! The full statement of the rule, including the parts a machine cannot judge,
//! is in `docs/COMPAT-FACADE-RULE.md`.
//!
//! What is checked here is the part a machine *can* judge: what a façade is
//! allowed to depend on. A façade that reaches past
//! [`FACADE_WORKSPACE_DEPENDENCIES`] is holding domain knowledge, and a façade
//! that opens a database connection or drives a download engine is holding
//! behavior. Both are the rule being broken in the one place it is visible
//! before the code is written.
//!
//! The lists are deliberately short and deliberately editable. Widening
//! [`FACADE_WORKSPACE_DEPENDENCIES`] is a legitimate move when a shared concern
//! genuinely grows a new home in the core — but it is a one-line diff in this
//! file, which is exactly the point: it surfaces in review instead of arriving
//! buried in a manifest.

use std::fmt;

/// Workspace crates a façade may depend on.
///
/// `stackarr-compat-core` carries the shared arr concerns; `stackarr-core`
/// carries the domain types a DTO translates to and from. A façade that needs
/// anything else needs it through one of these two.
pub const FACADE_WORKSPACE_DEPENDENCIES: &[&str] = &["stackarr-compat-core", "stackarr-core"];

/// Dependencies that place storage or an embedded engine inside a façade.
///
/// These are entry points, not implementation details: a façade holding one of
/// them is talking to a database or an engine directly rather than translating
/// a request for the core to answer.
pub const FACADE_FORBIDDEN_DEPENDENCIES: &[&str] =
    &["librtbit", "nzb-web", "nzbdav-core", "rusqlite", "sqlx"];

/// Why a dependency breaks the façade rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The façade reaches into a workspace crate it is not allowed to know
    /// about, which means it is carrying domain knowledge of its own.
    UnlistedWorkspaceCrate,
    /// The façade talks to storage or an embedded engine directly, which means
    /// it is carrying behavior that belongs in the core.
    StorageOrEngine,
}

impl fmt::Display for Reason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnlistedWorkspaceCrate => formatter
                .write_str("a façade may only depend on stackarr-compat-core and stackarr-core"),
            Self::StorageOrEngine => {
                formatter.write_str("a façade may not reach storage or an embedded engine directly")
            }
        }
    }
}

/// A single breach of the façade rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The façade crate that declares the dependency.
    pub crate_name: String,
    /// The dependency that breaks the rule.
    pub dependency: String,
    /// Why it breaks the rule.
    pub reason: Reason,
}

impl fmt::Display for Violation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} depends on {}: {}",
            self.crate_name, self.dependency, self.reason
        )
    }
}

/// Whether `crate_name` names a compatibility façade.
///
/// `stackarr-compat-core` is not a façade — it is the shared core the façades
/// sit on, and it is the one compat crate that is allowed to hold arr-generic
/// behavior.
#[must_use]
pub fn is_facade_crate(crate_name: &str) -> bool {
    crate_name.starts_with("stackarr-compat-") && crate_name != "stackarr-compat-core"
}

/// Every way `dependencies` breaks the façade rule for `crate_name`.
///
/// Returns an empty vector for any crate that is not a façade, so this can be
/// applied to a whole workspace without filtering first.
#[must_use]
pub fn violations(crate_name: &str, dependencies: &[&str]) -> Vec<Violation> {
    if !is_facade_crate(crate_name) {
        return Vec::new();
    }

    dependencies
        .iter()
        .filter_map(|dependency| {
            let reason = if FACADE_FORBIDDEN_DEPENDENCIES.contains(dependency) {
                Reason::StorageOrEngine
            } else if dependency.starts_with("stackarr-")
                && !FACADE_WORKSPACE_DEPENDENCIES.contains(dependency)
            {
                Reason::UnlistedWorkspaceCrate
            } else {
                return None;
            };

            Some(Violation {
                crate_name: crate_name.to_owned(),
                dependency: (*dependency).to_owned(),
                reason,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facades_are_the_compat_crates_other_than_this_one() {
        assert!(is_facade_crate("stackarr-compat-sonarr-v3"));
        assert!(is_facade_crate("stackarr-compat-prowlarr-v1"));
        assert!(!is_facade_crate("stackarr-compat-core"));
        assert!(!is_facade_crate("stackarr-web"));
    }

    #[test]
    fn dtos_route_wiring_and_translation_are_allowed() {
        let dependencies = ["stackarr-compat-core", "stackarr-core", "axum", "serde"];
        assert_eq!(
            violations("stackarr-compat-sonarr-v3", &dependencies),
            Vec::new()
        );
    }

    #[test]
    fn a_facade_may_not_reach_another_workspace_crate() {
        let violations = violations("stackarr-compat-radarr-v3", &["stackarr-quality"]);
        assert_eq!(
            violations,
            vec![Violation {
                crate_name: "stackarr-compat-radarr-v3".to_owned(),
                dependency: "stackarr-quality".to_owned(),
                reason: Reason::UnlistedWorkspaceCrate,
            }]
        );
    }

    #[test]
    fn a_facade_may_not_reach_storage_or_an_engine() {
        let violations = violations("stackarr-compat-sonarr-v3", &["sqlx", "librtbit"]);
        let reasons: Vec<Reason> = violations.iter().map(|entry| entry.reason).collect();
        assert_eq!(
            reasons,
            vec![Reason::StorageOrEngine, Reason::StorageOrEngine]
        );
    }

    #[test]
    fn the_rule_does_not_apply_to_non_facade_crates() {
        assert_eq!(violations("stackarr-web", &["sqlx"]), Vec::new());
        assert_eq!(violations("stackarr-compat-core", &["sqlx"]), Vec::new());
    }

    #[test]
    fn a_violation_reads_as_a_review_comment() {
        let violation = Violation {
            crate_name: "stackarr-compat-sonarr-v3".to_owned(),
            dependency: "sqlx".to_owned(),
            reason: Reason::StorageOrEngine,
        };
        assert_eq!(
            violation.to_string(),
            "stackarr-compat-sonarr-v3 depends on sqlx: \
             a façade may not reach storage or an embedded engine directly"
        );
    }
}
