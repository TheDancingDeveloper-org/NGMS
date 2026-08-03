//! Applies the façade rule to the workspace as it actually stands.
//!
//! The rule is enforceable at review because review has something to point at:
//! this test fails on the pull request that puts domain knowledge or storage
//! access inside a compatibility façade.

use std::path::{Path, PathBuf};

use stackarr_compat_core::facade_rule::{is_facade_crate, violations};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate lives two directories below the workspace root")
        .to_path_buf()
}

fn read_manifest(path: &Path) -> toml::Value {
    let contents = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    toml::from_str(&contents)
        .unwrap_or_else(|error| panic!("cannot parse {}: {error}", path.display()))
}

fn workspace_members(root: &Path) -> Vec<String> {
    read_manifest(&root.join("Cargo.toml"))["workspace"]["members"]
        .as_array()
        .expect("workspace.members is an array")
        .iter()
        .map(|member| {
            member
                .as_str()
                .expect("a workspace member is a path string")
                .to_owned()
        })
        .collect()
}

/// The dependency names a manifest declares, resolving `package = "..."`
/// renames back to the crate actually being pulled in.
fn declared_dependencies(manifest: &toml::Value) -> Vec<String> {
    let mut names = Vec::new();
    for table in ["dependencies", "build-dependencies"] {
        let Some(entries) = manifest.get(table).and_then(toml::Value::as_table) else {
            continue;
        };
        for (name, specification) in entries {
            let renamed = specification.get("package").and_then(toml::Value::as_str);
            names.push(renamed.unwrap_or(name).to_owned());
        }
    }
    names
}

#[test]
fn every_facade_crate_obeys_the_facade_rule() {
    let root = workspace_root();
    let mut breaches = Vec::new();

    for member in workspace_members(&root) {
        let manifest = read_manifest(&root.join(&member).join("Cargo.toml"));
        let name = manifest["package"]["name"]
            .as_str()
            .expect("a member declares package.name")
            .to_owned();
        if !is_facade_crate(&name) {
            continue;
        }

        let dependencies = declared_dependencies(&manifest);
        let borrowed: Vec<&str> = dependencies.iter().map(String::as_str).collect();
        breaches.extend(violations(&name, &borrowed));
    }

    assert!(
        breaches.is_empty(),
        "the façade rule is broken; move the logic into the core:\n{}",
        breaches
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn this_crate_is_a_workspace_member() {
    assert!(
        workspace_members(&workspace_root())
            .iter()
            .any(|member| member == "crates/stackarr-compat-core"),
        "the façades have no shared core to sit on"
    );
}
