#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 The StackArr Authors

"""Enforce NGMS's public dependency boundary and SwarmForge release set."""

from __future__ import annotations

import pathlib
import subprocess
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
CRATES_IO_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"

SWARMFORGE = {
    "librtbit": ("swarmforge", "91806d3fedb13ffdfd86e903d62c6e0a46921c1b080ca0a105e25887fcaff7e2"),
    "bencode": ("swarmforge-bencode", "62025700dfaaaaa83979d2660a6fc14370ed273990cce0dbc76b0782a22fdb05"),
    "buffers": ("swarmforge-buffers", "3d580701883234fe6f873c0996e38feb829635ef970bbf341eb44ba148a27ed2"),
    "clone_to_owned": ("swarmforge-clone-to-owned", "2bb3b8f73c82f8996e0ace7e18f6122b679121e6e2c2375b12785a8d4b6ae3de"),
    "librtbit-core": ("swarmforge-core", "f832d9e4a4b8da07731824ce53ccec3b1efa26007d28a9a20bda0c2cd9a190a5"),
    "dht": ("swarmforge-dht", "b906ebbdf427da92ed396e1467e73bd773c95065f62f501b422268d8dee9a957"),
    "librtbit-lsd": ("swarmforge-lsd", "7d97411d5e5bf18353e3ca6fc22f3dc42659b7217bf53aec70f6d6f24cf7978e"),
    "peer_binary_protocol": ("swarmforge-peer-protocol", "f5bb477c07172aeeebe5c363472e210d0766073b40ef39475aa3ac5e486b7c94"),
    "sha1w": ("swarmforge-sha1-wrapper", "433e984d736e80f61a4171482657ec6bd0108b0803a973bd84c712e571e0481b"),
    "tracker_comms": ("swarmforge-tracker-comms", "e2c1a9c97cd297611004f9c56ea00c2587492bfd409aac1e55162abad27da659"),
    "librtbit-upnp": ("swarmforge-upnp", "74d398b4babe325834bd32ea02c3fd97dacda0e3f0b365ac484fe71f62ea3825"),
    "upnp-serve": ("swarmforge-upnp-serve", "2b438743bf8cbc23f6ef9aece56c1c4d1ab25cf943dda734d91f09bd48c2cb61"),
}

PUBLIC_ENGINE_DEPENDENCIES = {
    "nzb-web": "=0.4.21",
    "nzb-core": "=0.2.17",
    "nzb-nntp": "=0.2.23",
    "nzb-decode": "=0.1.3",
    "nzb-postproc": "=0.2.7",
    "nzb-news": "=0.1.13",
    "nzb-dispatch": "=0.2.7",
    "nzbdav-core": "=0.5.7",
    "nzbdav-dav": "=0.5.7",
    "nzbdav-stream": "=0.5.7",
    "nzbdav-rar": "=0.5.7",
    "nzbdav-pipeline": "=0.5.7",
}

FORBIDDEN_VENDORED_ENGINE_DIRECTORIES = {
    "torrent",
    "usenet",
    *(package for package, _checksum in SWARMFORGE.values()),
    *PUBLIC_ENGINE_DEPENDENCIES,
}


def fail(message: str) -> None:
    print(f"dependency boundary violation: {message}", file=sys.stderr)
    raise SystemExit(1)


def dependency_version(value: object) -> str | None:
    if isinstance(value, str):
        return value
    if isinstance(value, dict):
        version = value.get("version")
        return version if isinstance(version, str) else None
    return None


def main() -> None:
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    dependencies = manifest["workspace"]["dependencies"]

    for alias, (package, _checksum) in SWARMFORGE.items():
        value = dependencies.get(alias)
        if not isinstance(value, dict):
            fail(f"{alias} must be a package alias table")
        if value.get("package") != package or value.get("version") != "=0.1.0":
            fail(f"{alias} must alias {package} at exactly =0.1.0")
        forbidden = {"registry", "git", "path"}.intersection(value)
        if forbidden:
            fail(f"{alias} contains forbidden source keys: {sorted(forbidden)}")

    for name, expected_version in PUBLIC_ENGINE_DEPENDENCIES.items():
        value = dependencies.get(name)
        if dependency_version(value) != expected_version:
            fail(f"{name} must be pinned to {expected_version}")
        if isinstance(value, dict) and {"registry", "git", "path"}.intersection(value):
            fail(f"{name} must resolve from crates.io")

    crates_dir = ROOT / "crates"
    vendored = sorted(
        path.name
        for path in crates_dir.iterdir()
        if path.is_dir() and path.name in FORBIDDEN_VENDORED_ENGINE_DIRECTORIES
    )
    if vendored:
        fail(f"published engines are re-vendored under crates/: {vendored}")

    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    locked = {(package["name"], package["version"]): package for package in lock["package"]}
    for _alias, (name, checksum) in SWARMFORGE.items():
        package = locked.get((name, "0.1.0"))
        if package is None:
            fail(f"Cargo.lock does not select {name} 0.1.0")
        if package.get("source") != CRATES_IO_SOURCE:
            fail(f"{name} does not resolve from crates.io")
        if package.get("checksum") != checksum:
            fail(f"{name} checksum differs from the published release record")

    private_markers = ("repo.indexarr.net", "100.92.54.45", "registries.forgejo")
    source_files = [
        ROOT / "Cargo.toml",
        ROOT / "Cargo.lock",
        ROOT / "docker" / "Dockerfile",
        ROOT / "docker" / "Dockerfile.standalone",
    ]
    source_files.extend((ROOT / ".github" / "workflows").glob("*.yml"))
    for path in source_files:
        contents = path.read_text()
        for marker in private_markers:
            if marker in contents:
                fail(f"{path.relative_to(ROOT)} still contains {marker}")

    if (ROOT / ".woodpecker" / "main.yml").exists():
        fail("the retired Forgejo/Woodpecker pipeline is still present")

    tracked = subprocess.check_output(
        ["git", "ls-files", "-z"], cwd=ROOT
    ).split(b"\0")
    forbidden_identity = b"".join((b"sp", b"roo", b"ty"))
    former_org = b"".join((b"Aus", b"Agent", b"Smith"))
    for raw_path in filter(None, tracked):
        relative = pathlib.Path(raw_path.decode())
        lowered_path = raw_path.lower()
        if forbidden_identity in lowered_path:
            fail(f"tracked path contains retired identity: {relative}")
        path = ROOT / relative
        contents = (
            path.readlink().as_posix().encode().lower()
            if path.is_symlink()
            else path.read_bytes().lower()
        )
        if forbidden_identity in contents:
            fail(f"{relative} contains the retired identity")
        if former_org.lower() in contents:
            fail(f"{relative} contains the former GitHub organization")

    if any(path.startswith(b".claude/worktrees/") for path in tracked):
        fail("a generated Claude worktree is tracked")

    print(
        "migration boundary verified: public sources, canonical ownership, "
        "and all 12 SwarmForge packages"
    )


if __name__ == "__main__":
    main()
