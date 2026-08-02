#!/usr/bin/env python3
"""Mechanically rename sqlx PostgreSQL driver types to the MariaDB/MySQL driver."""

from __future__ import annotations

import pathlib


ROOT = pathlib.Path(__file__).resolve().parents[1]
REPLACEMENTS = (
    ("sqlx::postgres::PgPoolOptions", "sqlx::mysql::MySqlPoolOptions"),
    ("sqlx::Postgres", "sqlx::MySql"),
    ("sqlx::PgPool", "sqlx::MySqlPool"),
    ("postgres::PgPoolOptions", "mysql::MySqlPoolOptions"),
    ("PgPoolOptions", "MySqlPoolOptions"),
    ("PgPool", "MySqlPool"),
)


def main() -> None:
    changed = 0
    sources = [*(ROOT / "crates").rglob("*.rs"), *(ROOT / "src").rglob("*.rs")]
    for path in sorted(sources):
        text = path.read_text()
        updated = text
        for old, new in REPLACEMENTS:
            updated = updated.replace(old, new)
        if updated != text:
            path.write_text(updated)
            changed += 1
    print(f"updated sqlx backend types in {changed} Rust files")


if __name__ == "__main__":
    main()
