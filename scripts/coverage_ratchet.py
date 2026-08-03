#!/usr/bin/env python3
"""Per-crate line-coverage ratchet.

Reads an LCOV report, attributes every covered line to the workspace member
that owns the file, and compares the result against the checked-in baseline in
`coverage-baseline.json`. Coverage may rise; it may not fall. Regenerate the
baseline with `--update` and review the diff like any other change.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
BASELINE = ROOT / "coverage-baseline.json"
NON_LIBRARY_DIRS = {"tests", "benches", "examples"}


def fail(message: str) -> None:
    sys.stdout.flush()
    print(f"coverage ratchet: {message}", file=sys.stderr)
    raise SystemExit(1)


def package_name(manifest_dir: pathlib.Path) -> str:
    manifest = tomllib.loads((manifest_dir / "Cargo.toml").read_text())
    return manifest["package"]["name"]


def workspace_packages() -> dict[str, pathlib.Path]:
    """Map every workspace package name to its directory, root package included.

    The root package owns `src/` only, so that files it does not compile — the
    `crates/` members above all — are never attributed to it.
    """
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
    packages = {manifest["package"]["name"]: ROOT / "src"}
    for member in manifest["workspace"]["members"]:
        directory = ROOT / member
        packages[package_name(directory)] = directory
    return packages


def owning_package(path: pathlib.Path, packages: dict[str, pathlib.Path]) -> str | None:
    """Return the package owning `path`, or None for files outside library code."""
    owner: str | None = None
    owned: pathlib.Path | None = None
    depth = -1
    for name, directory in packages.items():
        try:
            relative = path.relative_to(directory)
        except ValueError:
            continue
        if len(directory.parts) > depth:
            owner, owned, depth = name, relative, len(directory.parts)
    if owned is None or set(owned.parts[:-1]) & NON_LIBRARY_DIRS:
        return None
    return owner


def parse_lcov(report: pathlib.Path) -> dict[pathlib.Path, dict[int, int]]:
    """Merge an LCOV report into hit counts per source line."""
    hits: dict[pathlib.Path, dict[int, int]] = collections.defaultdict(dict)
    current: dict[int, int] | None = None
    for raw in report.read_text().splitlines():
        line = raw.strip()
        if line.startswith("SF:"):
            source = pathlib.Path(line[3:])
            if not source.is_absolute():
                source = ROOT / source
            current = hits[pathlib.Path(source.as_posix())]
        elif line.startswith("DA:") and current is not None:
            number, _, count = line[3:].partition(",")
            executions = int(count.split(",")[0])
            key = int(number)
            current[key] = max(current.get(key, 0), executions)
        elif line == "end_of_record":
            current = None
    return hits


def measure(report: pathlib.Path, packages: dict[str, pathlib.Path]) -> dict[str, dict]:
    """Aggregate an LCOV report into per-package line coverage."""
    totals = {name: [0, 0] for name in packages}
    for source, lines in parse_lcov(report).items():
        owner = owning_package(source, packages)
        if owner is None:
            continue
        totals[owner][0] += len(lines)
        totals[owner][1] += sum(1 for executions in lines.values() if executions > 0)
    return {
        name: {
            "lines": lines,
            "covered": covered,
            # A crate the report says nothing about is a measurement gap, not a
            # perfect score: record 0 so the ratchet can only be raised by real
            # numbers arriving later.
            "percent": round(100.0 * covered / lines, 2) if lines else 0.0,
        }
        for name, (lines, covered) in sorted(totals.items())
    }


def report_table(measured: dict[str, dict], baseline: dict[str, dict] | None) -> None:
    width = max(len(name) for name in measured)
    for name, entry in measured.items():
        recorded = (baseline or {}).get(name)
        delta = ""
        if recorded is not None:
            delta = f"  ({entry['percent'] - recorded['percent']:+.2f})"
        print(
            f"  {name:<{width}}  {entry['percent']:6.2f}%"
            f"  {entry['covered']}/{entry['lines']} lines{delta}"
        )


def check(measured: dict[str, dict], document: dict) -> list[str]:
    baseline = document["crates"]
    tolerance = document["tolerance_percent"]
    problems = []
    for name in sorted(baseline.keys() - measured.keys()):
        problems.append(f"{name} is recorded in the baseline but absent from the report")
    for name in sorted(measured.keys() - baseline.keys()):
        problems.append(f"{name} has no recorded baseline; rerun with --update")
    for name in sorted(baseline.keys() & measured.keys()):
        recorded = baseline[name]["percent"]
        current = measured[name]["percent"]
        if current < recorded - tolerance:
            problems.append(
                f"{name} dropped from {recorded:.2f}% to {current:.2f}% "
                f"(tolerance {tolerance:.2f} points)"
            )
    return problems


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lcov", type=pathlib.Path, required=True, help="LCOV report to read")
    parser.add_argument(
        "--update",
        action="store_true",
        help="rewrite the baseline from this report instead of checking it",
    )
    arguments = parser.parse_args()

    if not arguments.lcov.is_file():
        fail(f"{arguments.lcov} does not exist; generate it with `just coverage`")

    packages = workspace_packages()
    measured = measure(arguments.lcov, packages)
    document = json.loads(BASELINE.read_text())

    if arguments.update:
        report_table(measured, document["crates"])
        document["crates"] = measured
        BASELINE.write_text(json.dumps(document, indent=2) + "\n")
        print(f"coverage baseline updated: {len(measured)} crates recorded")
        return

    report_table(measured, document["crates"])
    problems = check(measured, document)
    if problems:
        fail("coverage may not go down\n  " + "\n  ".join(problems))
    print(f"coverage ratchet held: {len(measured)} crates at or above baseline")


if __name__ == "__main__":
    main()
