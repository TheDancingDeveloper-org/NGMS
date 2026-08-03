#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 The StackArr Authors

"""Keep AGENTS.md's workspace inventory aligned with Cargo.toml."""

from __future__ import annotations

import pathlib
import re
import sys
import tomllib


ROOT = pathlib.Path(__file__).resolve().parents[1]
START = "<!-- workspace-members:start -->"
END = "<!-- workspace-members:end -->"


def fail(message: str) -> None:
    print(f"workspace documentation drift: {message}", file=sys.stderr)
    raise SystemExit(1)


manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
members = manifest["workspace"]["members"]
document = (ROOT / "AGENTS.md").read_text()

try:
    section = document.split(START, 1)[1].split(END, 1)[0]
except IndexError:
    fail("AGENTS.md is missing the workspace member markers")

documented = re.findall(r"^- `([^`]+)`$", section, flags=re.MULTILINE)
if documented != members:
    fail(f"Cargo.toml has {members!r}, AGENTS.md has {documented!r}")

claude = (ROOT / "CLAUDE.md").read_text()
if "[AGENTS.md](AGENTS.md)" not in claude:
    fail("CLAUDE.md must point to canonical AGENTS.md")

print(f"workspace documentation verified: {len(members)} members")
