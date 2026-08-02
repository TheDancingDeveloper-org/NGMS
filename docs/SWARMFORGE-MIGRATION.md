# SwarmForge dependency migration

NGMS uses the coordinated SwarmForge 0.1.0 package family published by the
canonical rustTorrent repository. All twelve packages resolve from crates.io;
a clean build does not require Forgejo credentials, private Git URLs, or local
path overrides.

## Compatibility aliases

Cargo package aliases keep NGMS's existing Rust imports stable while changing
the package identities atomically:

| NGMS dependency key | crates.io package | Version |
| --- | --- | --- |
| `librtbit` | `swarmforge` | `0.1.0` |
| `bencode` | `swarmforge-bencode` | `0.1.0` |
| `buffers` | `swarmforge-buffers` | `0.1.0` |
| `clone_to_owned` | `swarmforge-clone-to-owned` | `0.1.0` |
| `librtbit-core` | `swarmforge-core` | `0.1.0` |
| `dht` | `swarmforge-dht` | `0.1.0` |
| `librtbit-lsd` | `swarmforge-lsd` | `0.1.0` |
| `peer_binary_protocol` | `swarmforge-peer-protocol` | `0.1.0` |
| `sha1w` | `swarmforge-sha1-wrapper` | `0.1.0` |
| `tracker_comms` | `swarmforge-tracker-comms` | `0.1.0` |
| `librtbit-upnp` | `swarmforge-upnp` | `0.1.0` |
| `upnp-serve` | `swarmforge-upnp-serve` | `0.1.0` |

The versions are exact pins. Grouped weekly Dependabot and Renovate pull
requests surface new releases for review; the pins move only after the complete
engine graph passes the repository gates. `scripts/check_dependency_sources.py` validates
the aliases, crates.io sources, and release checksums in `Cargo.lock`. The root
test target selects `upnp-serve`, which is otherwise dormant, so CI verifies
the complete release family rather than only the eleven runtime-selected
packages.

## Other engine dependencies

The Usenet and NZBDav package families are exact-pinned and also resolve from crates.io. This is
necessary for the Docker build and GitHub Actions jobs to be genuinely
credential-free. The published Usenet API replaces backup-server probing with
a bounded `max_nested_archive_depth` post-processing option; NGMS defaults it
to five.

## Source and rollback policy

The active torrent source of truth is the published SwarmForge family from
`TheDancingDeveloper-org/rustTorrent`. The historical `crates/torrent/` snapshot
was removed during P1/T15. Repository history remains the recovery mechanism;
published engine changes must be made upstream rather than re-vendored here.

Rollback is a reviewed, atomic change of all twelve aliases and the lockfile.
Never mix `librtbit-*` and `swarmforge-*` identities in one graph because Cargo
would treat shared types and traits as belonging to different packages.

NGMS source, CI, and container publication are canonical on GitHub. Main-branch
builds publish `ghcr.io/thedancingdeveloper-org/ngms:latest` and an immutable
`sha-<commit>` tag. The retired Woodpecker pipeline and Forgejo Cargo
credentials are not part of the GitHub build path.
