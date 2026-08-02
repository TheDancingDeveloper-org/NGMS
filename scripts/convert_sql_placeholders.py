#!/usr/bin/env python3
"""Audit and optionally convert PostgreSQL `$n` placeholders to MariaDB `?`.

The converter changes only Rust string literals that contain SQL-looking text.
It refuses a file when any literal repeats a placeholder or uses placeholders
out of monotonically increasing order; those queries need a hand rewrite so
bind-by-occurrence cannot silently change their semantics.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys


ROOT = pathlib.Path(__file__).resolve().parents[1]
PLACEHOLDER = re.compile(r"\$(\d+)")
STRING = re.compile(r'(?s)(?:r(?P<hashes>#+)?".*?"(?P=hashes)|"(?:\\.|[^"\\])*")')
SQL_WORD = re.compile(r"\b(?:SELECT|INSERT|UPDATE|DELETE|WITH)\b", re.IGNORECASE)


def rust_sources() -> list[pathlib.Path]:
    return sorted([*(ROOT / "crates").rglob("*.rs"), *(ROOT / "src").rglob("*.rs")])


def issues(path: pathlib.Path, text: str) -> list[str]:
    found: list[str] = []
    for literal in STRING.finditer(text):
        value = literal.group(0)
        if not SQL_WORD.search(value):
            continue
        numbers = [int(number) for number in PLACEHOLDER.findall(value)]
        if not numbers:
            continue
        expected = list(range(1, len(numbers) + 1))
        if numbers != expected:
            line = text.count("\n", 0, literal.start()) + 1
            found.append(f"{path.relative_to(ROOT)}:{line}: {numbers} (expected {expected})")
    return found


def convert(text: str) -> str:
    def replace_literal(match: re.Match[str]) -> str:
        value = match.group(0)
        if SQL_WORD.search(value):
            return PLACEHOLDER.sub("?", value)
        return value

    return STRING.sub(replace_literal, text)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--write", action="store_true", help="rewrite safe SQL literals")
    parser.add_argument(
        "--report",
        type=pathlib.Path,
        help="write the hand-review list as JSON; audit still exits non-zero",
    )
    args = parser.parse_args()

    sources = rust_sources()
    flagged: list[str] = []
    placeholder_count = 0
    for path in sources:
        text = path.read_text()
        placeholder_count += len(PLACEHOLDER.findall(text))
        flagged.extend(issues(path, text))

    if flagged:
        if args.report:
            args.report.write_text(json.dumps({"unsafe_queries": flagged}, indent=2) + "\n")
        print("unsafe PostgreSQL placeholders require hand review:", file=sys.stderr)
        print("\n".join(flagged), file=sys.stderr)
        raise SystemExit(1)

    if args.write:
        for path in sources:
            text = path.read_text()
            updated = convert(text)
            if updated != text:
                path.write_text(updated)

    action = "converted" if args.write else "audited"
    print(f"{action} {placeholder_count} PostgreSQL placeholders across {len(sources)} Rust files")


if __name__ == "__main__":
    main()
