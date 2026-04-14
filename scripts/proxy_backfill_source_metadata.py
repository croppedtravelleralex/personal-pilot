#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sqlite3
from pathlib import Path


DEFAULT_DB_PATH = "data/auto_open_browser.db"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Backfill blank proxy provider/region/country from proxy_harvest_sources default_fields.",
    )
    parser.add_argument("--db", default=DEFAULT_DB_PATH, help="SQLite database path")
    parser.add_argument(
        "--source-label",
        action="append",
        default=[],
        help="Only process the selected source_label values",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Report candidate updates without writing them",
    )
    return parser.parse_args()


def load_sources(conn: sqlite3.Connection, labels: list[str]) -> list[sqlite3.Row]:
    conn.row_factory = sqlite3.Row
    if labels:
        placeholders = ",".join("?" for _ in labels)
        query = f"""
            SELECT source_label, config_json
            FROM proxy_harvest_sources
            WHERE source_label IN ({placeholders})
            ORDER BY source_label ASC
        """
        return conn.execute(query, labels).fetchall()
    return conn.execute(
        """
        SELECT source_label, config_json
        FROM proxy_harvest_sources
        ORDER BY source_label ASC
        """
    ).fetchall()


def default_fields(config_json: str) -> dict[str, str]:
    try:
        parsed = json.loads(config_json or "{}")
    except json.JSONDecodeError:
        return {}
    fields = parsed.get("default_fields") if isinstance(parsed, dict) else None
    if not isinstance(fields, dict):
        return {}
    result: dict[str, str] = {}
    for key in ("provider", "region", "country"):
        value = fields.get(key)
        if isinstance(value, str) and value.strip():
            result[key] = value.strip()
    return result


def main() -> int:
    args = parse_args()
    db_path = Path(args.db)
    conn = sqlite3.connect(db_path.as_posix())
    try:
        sources = load_sources(conn, args.source_label)
        total_rows_updated = 0
        total_field_updates = {"provider": 0, "region": 0, "country": 0}
        summaries: list[dict[str, object]] = []

        for row in sources:
            source_label = str(row["source_label"])
            defaults = default_fields(str(row["config_json"] or ""))
            if not defaults:
                summaries.append(
                    {
                        "source_label": source_label,
                        "matched_rows": 0,
                        "updated_rows": 0,
                        "field_updates": {},
                        "skipped": "no default_fields",
                    }
                )
                continue

            matched_rows = conn.execute(
                """
                SELECT COUNT(*)
                FROM proxies
                WHERE source_label = ?
                  AND (
                    (provider IS NULL OR TRIM(provider) = '')
                    OR (region IS NULL OR TRIM(region) = '')
                    OR (country IS NULL OR TRIM(country) = '')
                  )
                """,
                (source_label,),
            ).fetchone()[0]
            field_updates = {}
            for key, value in defaults.items():
                field_updates[key] = conn.execute(
                    f"""
                    SELECT COUNT(*)
                    FROM proxies
                    WHERE source_label = ?
                      AND ({key} IS NULL OR TRIM({key}) = '')
                      AND ? IS NOT NULL
                      AND TRIM(?) != ''
                    """,
                    (source_label, value, value),
                ).fetchone()[0]

            update_sql = """
                UPDATE proxies
                SET provider = CASE
                        WHEN (provider IS NULL OR TRIM(provider) = '') AND ? IS NOT NULL AND TRIM(?) != '' THEN ?
                        ELSE provider
                    END,
                    region = CASE
                        WHEN (region IS NULL OR TRIM(region) = '') AND ? IS NOT NULL AND TRIM(?) != '' THEN ?
                        ELSE region
                    END,
                    country = CASE
                        WHEN (country IS NULL OR TRIM(country) = '') AND ? IS NOT NULL AND TRIM(?) != '' THEN ?
                        ELSE country
                    END
                WHERE source_label = ?
                  AND (
                    ((provider IS NULL OR TRIM(provider) = '') AND ? IS NOT NULL AND TRIM(?) != '')
                    OR ((region IS NULL OR TRIM(region) = '') AND ? IS NOT NULL AND TRIM(?) != '')
                    OR ((country IS NULL OR TRIM(country) = '') AND ? IS NOT NULL AND TRIM(?) != '')
                  )
            """
            params = (
                defaults.get("provider"),
                defaults.get("provider"),
                defaults.get("provider"),
                defaults.get("region"),
                defaults.get("region"),
                defaults.get("region"),
                defaults.get("country"),
                defaults.get("country"),
                defaults.get("country"),
                source_label,
                defaults.get("provider"),
                defaults.get("provider"),
                defaults.get("region"),
                defaults.get("region"),
                defaults.get("country"),
                defaults.get("country"),
            )

            if args.dry_run:
                updated_rows = int(matched_rows)
            else:
                cursor = conn.execute(update_sql, params)
                updated_rows = cursor.rowcount if cursor.rowcount is not None else 0

            total_rows_updated += max(updated_rows, 0)
            for key in total_field_updates:
                total_field_updates[key] += int(field_updates.get(key, 0))
            summaries.append(
                {
                    "source_label": source_label,
                    "matched_rows": int(matched_rows),
                    "updated_rows": max(updated_rows, 0),
                    "field_updates": field_updates,
                }
            )

        if not args.dry_run:
            conn.commit()

        print(
            json.dumps(
                {
                    "db": db_path.as_posix(),
                    "dry_run": args.dry_run,
                    "source_count": len(summaries),
                    "updated_rows": total_rows_updated,
                    "field_updates": total_field_updates,
                    "sources": summaries,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0
    finally:
        conn.close()


if __name__ == "__main__":
    raise SystemExit(main())
