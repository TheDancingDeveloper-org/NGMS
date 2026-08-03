#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 The StackArr Authors

"""Verify every first-party source file carries the GPL-3.0 header.

Run with --fix to insert the header into files that are missing it.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
SPDX = "SPDX-License-Identifier: GPL-3.0-only"
COPYRIGHT = "Copyright (C) 2026 The StackArr Authors"
COPYRIGHT_PATTERN = re.compile(r"^Copyright \(C\) \d{4}(-\d{4})? The StackArr Authors$")

# Comment prefix and suffix per source extension.
SLASH = ("// ", "")
HASH = ("# ", "")
BLOCK = ("/* ", " */")
STYLES = {
    ".rs": SLASH,
    ".ts": SLASH,
    ".tsx": SLASH,
    ".js": SLASH,
    ".jsx": SLASH,
    ".mjs": SLASH,
    ".cjs": SLASH,
    ".css": BLOCK,
    ".py": HASH,
    ".sh": HASH,
}


def tracked_sources() -> list[pathlib.Path]:
    listing = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    paths = (pathlib.Path(name) for name in listing.split("\0") if name)
    return sorted(path for path in paths if path.suffix in STYLES)


def header_lines(path: pathlib.Path) -> list[str]:
    prefix, suffix = STYLES[path.suffix]
    return [f"{prefix}{SPDX}{suffix}", f"{prefix}{COPYRIGHT}{suffix}"]


def has_header(path: pathlib.Path, lines: list[str]) -> bool:
    prefix, suffix = STYLES[path.suffix]
    body = lines[1:] if lines and lines[0].startswith("#!") else lines
    if len(body) < 2:
        return False

    def uncomment(line: str) -> str | None:
        if not line.startswith(prefix) or not line.endswith(suffix):
            return None
        return line[len(prefix) : len(line) - len(suffix)] if suffix else line[len(prefix) :]

    spdx, copyright_line = uncomment(body[0]), uncomment(body[1])
    return spdx == SPDX and copyright_line is not None and bool(COPYRIGHT_PATTERN.match(copyright_line))


def insert_header(path: pathlib.Path, lines: list[str]) -> list[str]:
    shebang = lines[:1] if lines and lines[0].startswith("#!") else []
    rest = lines[len(shebang) :]
    while rest and not rest[0].strip():
        rest.pop(0)
    return shebang + header_lines(path) + [""] + rest


def main() -> int:
    fix = "--fix" in sys.argv[1:]
    sources = tracked_sources()
    missing: list[pathlib.Path] = []

    for path in sources:
        absolute = ROOT / path
        text = absolute.read_text()
        lines = text.split("\n")
        if has_header(path, lines):
            continue
        if not fix:
            missing.append(path)
            continue
        absolute.write_text("\n".join(insert_header(path, lines)))

    if missing:
        print("missing GPL-3.0 header:", file=sys.stderr)
        for path in missing:
            print(f"  {path}", file=sys.stderr)
        print(
            f"{len(missing)} file(s) without a header; "
            "run python3 scripts/check_license_headers.py --fix",
            file=sys.stderr,
        )
        return 1

    print(f"license headers verified: {len(sources)} files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
