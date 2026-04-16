#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
import sys
import time
import urllib.parse
import urllib.request
from pathlib import Path


DEFAULT_DB_PATH = "data/persona_pilot.db"


def now_ts() -> str:
    return str(int(time.time()))


def strip_hash_comment_lines(raw: str) -> str:
    kept: list[str] = []
    for line in raw.splitlines():
        if line.lstrip().startswith("#"):
            continue
        kept.append(line)
    return "\n".join(kept)


def resolve_local_path(
    base_dir: Path,
    raw_path: object,
    *,
    fallback_dir: Path | None = None,
) -> str:
    expanded = Path(os.path.expandvars(os.path.expanduser(str(raw_path).strip())))
    if expanded.is_absolute():
        return expanded.resolve().as_posix()
    config_relative = (base_dir / expanded).resolve()
    fallback_relative = (fallback_dir / expanded).resolve() if fallback_dir is not None else None
    if config_relative.exists():
        return config_relative.as_posix()
    if fallback_relative is not None and fallback_relative.exists():
        return fallback_relative.as_posix()
    if expanded.parts[:1] in [(".",), ("..",)] or len(expanded.parts) == 1:
        return config_relative.as_posix()
    if fallback_relative is not None:
        return fallback_relative.as_posix()
    return config_relative.as_posix()


def resolve_source_config_paths(
    sources: list[dict[str, object]],
    *,
    config_dir: Path,
    cwd_dir: Path,
) -> list[dict[str, object]]:
    resolved: list[dict[str, object]] = []
    for source in sources:
        source_kind = str(source.get("source_kind") or "")
        config_json = dict(source.get("config_json") or {})
        if source_kind in {"text_file", "json_file"} and config_json.get("path"):
            config_json["path"] = resolve_local_path(
                config_dir,
                config_json["path"],
                fallback_dir=cwd_dir,
            )
        resolved.append({**source, "config_json": config_json})
    return resolved


def default_config_path() -> str:
    return resolve_local_path(
        Path.cwd(),
        os.environ.get("PERSONA_PILOT_PROXY_HARVEST_CONFIG", "data/proxy_sources.json"),
    )


def ensure_schema(conn: sqlite3.Connection) -> None:
    conn.execute("PRAGMA foreign_keys = ON")
    columns = {
        row[1]
        for row in conn.execute("PRAGMA table_info(proxies)").fetchall()
    }
    for name, sql in [
        ("source_label", "ALTER TABLE proxies ADD COLUMN source_label TEXT"),
        ("last_seen_at", "ALTER TABLE proxies ADD COLUMN last_seen_at TEXT"),
        ("promoted_at", "ALTER TABLE proxies ADD COLUMN promoted_at TEXT"),
    ]:
        if name not in columns:
            conn.execute(sql)
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS proxy_harvest_sources (
            source_label TEXT PRIMARY KEY,
            source_kind TEXT NOT NULL,
            enabled INTEGER NOT NULL DEFAULT 1,
            config_json TEXT NOT NULL,
            interval_seconds INTEGER NOT NULL DEFAULT 300,
            base_proxy_score REAL NOT NULL DEFAULT 1.0,
            consecutive_failures INTEGER NOT NULL DEFAULT 0,
            backoff_until TEXT,
            last_run_started_at TEXT,
            last_run_finished_at TEXT,
            last_run_status TEXT,
            last_error TEXT,
            health_score REAL NOT NULL DEFAULT 100.0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        """
    )
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS proxy_harvest_runs (
            id TEXT PRIMARY KEY,
            source_label TEXT,
            source_kind TEXT,
            fetched_count INTEGER NOT NULL DEFAULT 0,
            accepted_count INTEGER NOT NULL DEFAULT 0,
            deduped_count INTEGER NOT NULL DEFAULT 0,
            rejected_count INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL,
            summary_json TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT
        )
        """
    )
    conn.execute(
        """
        CREATE INDEX IF NOT EXISTS idx_proxies_endpoint_dedupe
        ON proxies(scheme, host, port, username, provider, region)
        """
    )
    conn.commit()


def parse_source_config(raw: str) -> list[dict[str, object]]:
    parsed = json.loads(strip_hash_comment_lines(raw))
    if isinstance(parsed, list):
        items = parsed
    elif isinstance(parsed, dict) and isinstance(parsed.get("items"), list):
        items = parsed["items"]
    else:
        raise ValueError("proxy source config must be an array or object with items[]")
    normalized: list[dict[str, object]] = []
    for item in items:
        if not isinstance(item, dict):
            raise ValueError("each proxy source item must be an object")
        source_label = str(item.get("source_label") or "").strip()
        source_kind = str(item.get("source_kind") or "").strip()
        if not source_label or not source_kind:
            raise ValueError("proxy source item requires source_label and source_kind")
        normalized.append(
            {
                "source_label": source_label,
                "source_kind": source_kind,
                "enabled": bool(item.get("enabled", True)),
                "config_json": item.get("config_json") or {},
                "interval_seconds": max(int(item.get("interval_seconds") or 300), 30),
                "base_proxy_score": float(item.get("base_proxy_score") or 1.0),
            }
        )
    return normalized


def load_config_sources(config_path: str) -> list[dict[str, object]]:
    path = Path(resolve_local_path(Path.cwd(), config_path))
    if not path.exists():
        return []
    return resolve_source_config_paths(
        parse_source_config(path.read_text(encoding="utf-8")),
        config_dir=path.parent,
        cwd_dir=Path.cwd(),
    )


def load_legacy_sources(args: argparse.Namespace) -> list[dict[str, object]]:
    if bool(args.file) == bool(args.url):
        return []
    source_kind = args.source_kind
    if not source_kind:
        source_kind = "text_file" if args.file else "text_url"
    config_json = (
        {"path": resolve_local_path(Path.cwd(), args.file)}
        if args.file
        else {"url": args.url}
    )
    return [
        {
            "source_label": args.source_label[0] if args.source_label else "harvest_mvp",
            "source_kind": source_kind,
            "enabled": True,
            "config_json": config_json,
            "interval_seconds": 300,
            "base_proxy_score": 1.0,
        }
    ]


def sync_source_registry(conn: sqlite3.Connection, sources: list[dict[str, object]]) -> None:
    now = now_ts()
    for source in sources:
        conn.execute(
            """
            INSERT INTO proxy_harvest_sources (
                source_label, source_kind, enabled, config_json, interval_seconds,
                base_proxy_score, consecutive_failures, backoff_until, last_run_started_at,
                last_run_finished_at, last_run_status, last_error, health_score,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, 0, NULL, NULL, NULL, NULL, NULL, 100.0, ?, ?)
            ON CONFLICT(source_label) DO UPDATE SET
                source_kind = excluded.source_kind,
                enabled = excluded.enabled,
                config_json = excluded.config_json,
                interval_seconds = excluded.interval_seconds,
                base_proxy_score = excluded.base_proxy_score,
                updated_at = excluded.updated_at
            """,
            (
                source["source_label"],
                source["source_kind"],
                1 if source["enabled"] else 0,
                json.dumps(source["config_json"], ensure_ascii=False),
                int(source["interval_seconds"]),
                float(source["base_proxy_score"]),
                now,
                now,
            ),
        )
    conn.commit()


def select_sources(
    conn: sqlite3.Connection,
    source_labels: list[str] | None,
) -> list[dict[str, object]]:
    rows = conn.execute(
        """
        SELECT source_label, source_kind, enabled, config_json, interval_seconds,
               base_proxy_score, consecutive_failures, backoff_until, health_score
        FROM proxy_harvest_sources
        ORDER BY source_label ASC
        """
    ).fetchall()
    selected: list[dict[str, object]] = []
    wanted = set(source_labels or [])
    for row in rows:
        source = {
            "source_label": row[0],
            "source_kind": row[1],
            "enabled": bool(row[2]),
            "config_json": json.loads(row[3] or "{}"),
            "interval_seconds": int(row[4] or 300),
            "base_proxy_score": float(row[5] or 1.0),
            "consecutive_failures": int(row[6] or 0),
            "backoff_until": row[7],
            "health_score": float(row[8] or 100.0),
        }
        if wanted and source["source_label"] not in wanted:
            continue
        if not source["enabled"]:
            continue
        selected.append(source)
    return selected


def is_source_due(source: dict[str, object], now: int | None = None) -> bool:
    now = int(time.time()) if now is None else now
    backoff_until = source.get("backoff_until")
    if backoff_until not in (None, ""):
        try:
            if int(str(backoff_until)) > now:
                return False
        except ValueError:
            return False
    return True


def filter_due_sources(
    conn: sqlite3.Connection,
    selected_sources: list[dict[str, object]],
    run_once: bool,
) -> list[dict[str, object]]:
    if run_once:
        return selected_sources
    now = int(time.time())
    due_sources: list[dict[str, object]] = []
    for source in selected_sources:
        if not is_source_due(source, now):
            continue
        last_finished = conn.execute(
            """
            SELECT last_run_finished_at
            FROM proxy_harvest_sources
            WHERE source_label = ?
            """,
            (source["source_label"],),
        ).fetchone()
        last_finished_at = last_finished[0] if last_finished else None
        if last_finished_at not in (None, ""):
            try:
                if now - int(str(last_finished_at)) < int(source.get("interval_seconds", 300)):
                    continue
            except ValueError:
                pass
        due_sources.append(source)
    return due_sources


def load_text_lines(source: dict[str, object]) -> list[str]:
    config = dict(source["config_json"])
    source_kind = str(source["source_kind"])
    if source_kind == "text_file":
        path = config.get("path")
        if not path:
            raise ValueError(f"{source['source_label']} requires config_json.path")
        return Path(str(path)).read_text(encoding="utf-8").splitlines()
    if source_kind == "text_url":
        url = config.get("url")
        if not url:
            raise ValueError(f"{source['source_label']} requires config_json.url")
        with urllib.request.urlopen(str(url), timeout=15) as response:
            return response.read().decode("utf-8").splitlines()
    raise ValueError(f"unsupported text source kind: {source_kind}")


def load_json_items(source: dict[str, object]) -> list[dict[str, object]]:
    config = dict(source["config_json"])
    source_kind = str(source["source_kind"])
    if source_kind == "json_file":
        path = config.get("path")
        if not path:
            raise ValueError(f"{source['source_label']} requires config_json.path")
        raw = Path(str(path)).read_text(encoding="utf-8")
    elif source_kind == "json_url":
        url = config.get("url")
        if not url:
            raise ValueError(f"{source['source_label']} requires config_json.url")
        with urllib.request.urlopen(str(url), timeout=15) as response:
            raw = response.read().decode("utf-8")
    else:
        raise ValueError(f"unsupported json source kind: {source_kind}")
    parsed = json.loads(raw)
    if isinstance(parsed, list):
        items = parsed
    elif isinstance(parsed, dict) and isinstance(parsed.get("items"), list):
        items = parsed["items"]
    else:
        raise ValueError("json source must be an array or object with items[]")
    normalized: list[dict[str, object]] = []
    for item in items:
        if isinstance(item, dict):
            normalized.append(item)
        else:
            raise ValueError("json source items must be objects")
    return normalized


def candidate_id_for(record: dict[str, object]) -> str:
    seed = "|".join(
        [
            str(record.get("scheme", "http")),
            str(record.get("host", "")),
            str(record.get("port", "")),
            str(record.get("username") or ""),
            str(record.get("provider") or ""),
            str(record.get("region") or ""),
        ]
    )
    digest = hashlib.sha1(seed.encode("utf-8")).hexdigest()[:12]
    return f"proxy-candidate-{digest}"


def load_source_snapshot(conn: sqlite3.Connection, source_label: str) -> dict[str, int]:
    row = conn.execute(
        """
        SELECT
            COUNT(*) AS total_count,
            COALESCE(SUM(CASE WHEN status = 'candidate' THEN 1 ELSE 0 END), 0) AS candidate_count,
            COALESCE(SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END), 0) AS active_count,
            COALESCE(SUM(CASE WHEN status = 'candidate_rejected' THEN 1 ELSE 0 END), 0) AS candidate_rejected_count,
            COALESCE(SUM(CASE WHEN provider IS NULL OR TRIM(provider) = '' THEN 1 ELSE 0 END), 0) AS null_provider_count,
            COALESCE(SUM(CASE WHEN region IS NULL OR TRIM(region) = '' THEN 1 ELSE 0 END), 0) AS null_region_count,
            COALESCE(SUM(CASE WHEN country IS NULL OR TRIM(country) = '' THEN 1 ELSE 0 END), 0) AS null_country_count,
            COALESCE(SUM(CASE WHEN status = 'candidate_rejected' AND last_probe_error_category = 'connect_failed' THEN 1 ELSE 0 END), 0) AS connect_failed_count,
            COALESCE(SUM(CASE WHEN status = 'candidate_rejected' AND last_probe_error_category = 'upstream_missing' THEN 1 ELSE 0 END), 0) AS upstream_missing_count
        FROM proxies
        WHERE source_label = ?
        """,
        (source_label,),
    ).fetchone()
    return {
        "total_count": int(row[0] or 0),
        "candidate_count": int(row[1] or 0),
        "active_count": int(row[2] or 0),
        "candidate_rejected_count": int(row[3] or 0),
        "null_provider_count": int(row[4] or 0),
        "null_region_count": int(row[5] or 0),
        "null_country_count": int(row[6] or 0),
        "connect_failed_count": int(row[7] or 0),
        "upstream_missing_count": int(row[8] or 0),
    }


def source_promotion_rate(snapshot: dict[str, int]) -> float:
    decision_total = int(snapshot["active_count"]) + int(snapshot["candidate_rejected_count"])
    if decision_total <= 0:
        return 0.0
    return int(snapshot["active_count"]) / decision_total


def source_null_metadata_ratio(snapshot: dict[str, int]) -> float:
    total_count = int(snapshot["total_count"])
    if total_count <= 0:
        return 0.0
    blank_total = max(
        int(snapshot["null_provider_count"]),
        int(snapshot["null_region_count"]),
        int(snapshot["null_country_count"]),
    )
    return blank_total / total_count


def normalize_record(record: dict[str, object], base_score: float) -> dict[str, object]:
    normalized = dict(record)
    normalized.setdefault("scheme", "http")
    normalized.setdefault("status", "candidate")
    normalized.setdefault("score", base_score)
    normalized.setdefault("id", candidate_id_for(normalized))
    if not normalized.get("host") or normalized.get("port") in (None, ""):
        raise ValueError("candidate requires host and port")
    normalized["port"] = int(normalized["port"])
    normalized["score"] = float(normalized.get("score") or base_score)
    return normalized


def parse_text_candidate(line: str, base_score: float) -> dict[str, object] | None:
    raw = line.strip()
    if not raw or raw.startswith("#"):
        return None
    if raw.startswith("{"):
        return normalize_record(json.loads(raw), base_score)
    parts = raw.split()
    endpoint = parts[0]
    candidate_url = endpoint if "://" in endpoint else f"http://{endpoint}"
    parsed = urllib.parse.urlparse(candidate_url)
    if not parsed.scheme or not parsed.hostname or parsed.port is None:
        raise ValueError(f"unsupported proxy line: {raw}")
    record: dict[str, object] = {
        "scheme": parsed.scheme,
        "host": parsed.hostname,
        "port": parsed.port,
        "username": urllib.parse.unquote(parsed.username) if parsed.username else None,
        "password": urllib.parse.unquote(parsed.password) if parsed.password else None,
        "score": base_score,
    }
    for token in parts[1:]:
        if "=" not in token:
            continue
        key, value = token.split("=", 1)
        record[key] = value
    return normalize_record(record, base_score)


def find_existing(conn: sqlite3.Connection, record: dict[str, object]) -> sqlite3.Row | None:
    params = (
        record["scheme"],
        record["host"],
        int(record["port"]),
        record.get("username"),
        record.get("username"),
        record.get("provider"),
        record.get("provider"),
        record.get("region"),
        record.get("region"),
    )
    exact = conn.execute(
        """
        SELECT id, status
        FROM proxies
        WHERE scheme = ?
          AND host = ?
          AND port = ?
          AND ((username IS NULL AND ? IS NULL) OR username = ?)
          AND ((provider IS NULL AND ? IS NULL) OR provider = ?)
          AND ((region IS NULL AND ? IS NULL) OR region = ?)
        ORDER BY
          CASE status
            WHEN 'active' THEN 0
            WHEN 'candidate' THEN 1
            WHEN 'candidate_rejected' THEN 2
            ELSE 3
          END ASC,
          created_at ASC
        LIMIT 1
        """,
        params,
    ).fetchone()
    if exact is not None:
        return exact
    return conn.execute(
        """
        SELECT id, status
        FROM proxies
        WHERE scheme = ?
          AND host = ?
          AND port = ?
          AND ((username IS NULL AND ? IS NULL) OR username = ?)
          AND (
            provider IS NULL OR TRIM(provider) = ''
            OR (? IS NULL)
            OR provider = ?
          )
          AND (
            region IS NULL OR TRIM(region) = ''
            OR (? IS NULL)
            OR region = ?
          )
        ORDER BY
          CASE
            WHEN (provider IS NULL OR TRIM(provider) = '' OR region IS NULL OR TRIM(region) = '') THEN 0
            ELSE 1
          END ASC,
          CASE status
            WHEN 'active' THEN 0
            WHEN 'candidate' THEN 1
            WHEN 'candidate_rejected' THEN 2
            ELSE 3
          END ASC,
          created_at ASC
        LIMIT 1
        """,
        params,
    ).fetchone()


def upsert_candidate(
    conn: sqlite3.Connection,
    record: dict[str, object],
    source_label: str,
    now: str,
) -> str:
    existing = find_existing(conn, record)
    if existing:
        proxy_id, _status = existing
        conn.execute(
            """
            UPDATE proxies
            SET last_seen_at = ?,
                source_label = CASE
                    WHEN source_label IS NULL OR TRIM(source_label) = '' THEN ?
                    ELSE source_label
                END,
                score = MAX(score, ?),
                provider = CASE
                    WHEN provider IS NULL OR TRIM(provider) = '' THEN ?
                    ELSE provider
                END,
                region = CASE
                    WHEN region IS NULL OR TRIM(region) = '' THEN ?
                    ELSE region
                END,
                country = CASE
                    WHEN country IS NULL OR TRIM(country) = '' THEN ?
                    ELSE country
                END,
                password = COALESCE(password, ?),
                updated_at = ?
            WHERE id = ?
            """,
            (
                now,
                source_label,
                float(record.get("score", 1.0)),
                record.get("provider"),
                record.get("region"),
                record.get("country"),
                record.get("password"),
                now,
                proxy_id,
            ),
        )
        return "deduped"

    conn.execute(
        """
        INSERT INTO proxies (
            id, scheme, host, port, username, password, region, country, provider, status,
            score, success_count, failure_count, source_label, last_seen_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'candidate', ?, 0, 0, ?, ?, ?, ?)
        """,
        (
            record["id"],
            record["scheme"],
            record["host"],
            int(record["port"]),
            record.get("username"),
            record.get("password"),
            record.get("region"),
            record.get("country"),
            record.get("provider"),
            float(record.get("score", 1.0)),
            source_label,
            now,
            now,
            now,
        ),
    )
    return "accepted"


def health_score_for_run(
    source: dict[str, object],
    status: str,
    fetched_count: int,
    accepted_count: int,
    deduped_count: int,
    rejected_count: int,
    snapshot: dict[str, int],
) -> float:
    failures = int(source.get("consecutive_failures", 0))
    harvest_total = accepted_count + deduped_count + rejected_count
    accepted_ratio = (accepted_count / harvest_total) if harvest_total > 0 else 0.0
    harvest_rejected_ratio = (rejected_count / fetched_count) if fetched_count > 0 else 0.0
    promotion_rate = source_promotion_rate(snapshot)
    null_ratio = source_null_metadata_ratio(snapshot)
    active_count = int(snapshot["active_count"])
    rejected_total = int(snapshot["candidate_rejected_count"])
    verify_decision_total = active_count + rejected_total
    verify_rejected_ratio = (rejected_total / verify_decision_total) if verify_decision_total > 0 else 0.0
    connect_failed_ratio = (
        int(snapshot["connect_failed_count"]) / rejected_total
        if rejected_total > 0
        else 0.0
    )
    upstream_missing_ratio = (
        int(snapshot["upstream_missing_count"]) / rejected_total
        if rejected_total > 0
        else 0.0
    )
    prod_quality_bonus = 0.0
    if (
        bool(source.get("for_prod"))
        and active_count > 0
        and promotion_rate >= 0.1
        and int(snapshot["null_provider_count"]) == 0
        and int(snapshot["null_region_count"]) == 0
        and failures == 0
    ):
        promotion_bonus = min(max(promotion_rate - 0.1, 0.0) * 20.0, 4.0)
        prod_quality_bonus = 8.0 + promotion_bonus
    score = (
        63.0
        + accepted_ratio * 10.0
        + promotion_rate * 26.0
        + min(active_count, 12) * 0.9
        + prod_quality_bonus
        - harvest_rejected_ratio * 8.0
        - verify_rejected_ratio * 14.0
        - null_ratio * 24.0
        - connect_failed_ratio * 10.0
        - upstream_missing_ratio * 6.0
        - failures * 8.0
    )
    if status == "failed":
        score -= 10.0
    elif status == "partial":
        score -= 4.0
    return max(0.0, min(100.0, round(score, 2)))


def backoff_until(source: dict[str, object], failures: int) -> str:
    interval = max(int(source.get("interval_seconds", 300)), 30)
    delay = min(interval * (2 ** max(failures, 1)), 3600)
    return str(int(time.time()) + delay)


def run_source(conn: sqlite3.Connection, source: dict[str, object]) -> dict[str, object]:
    source_label = str(source["source_label"])
    source_kind = str(source["source_kind"])
    base_score = float(source.get("base_proxy_score", 1.0))
    run_id = f"proxy-harvest-{int(time.time() * 1000)}-{source_label}"
    started_at = now_ts()
    conn.execute(
        """
        UPDATE proxy_harvest_sources
        SET last_run_started_at = ?, updated_at = ?
        WHERE source_label = ?
        """,
        (started_at, started_at, source_label),
    )

    fetched_count = 0
    accepted_count = 0
    deduped_count = 0
    rejected_count = 0
    errors: list[str] = []
    status = "completed"

    try:
        if source_kind in {"text_file", "text_url"}:
            candidates = []
            for raw_line in load_text_lines(source):
                parsed = parse_text_candidate(raw_line, base_score)
                if parsed is not None:
                    candidates.append(parsed)
        elif source_kind in {"json_file", "json_url"}:
            candidates = [
                normalize_record(item, base_score)
                for item in load_json_items(source)
            ]
        else:
            raise ValueError(f"unsupported source kind: {source_kind}")

        for candidate in candidates:
            fetched_count += 1
            try:
                outcome = upsert_candidate(conn, candidate, source_label, now_ts())
                if outcome == "accepted":
                    accepted_count += 1
                else:
                    deduped_count += 1
            except Exception as exc:  # noqa: BLE001
                rejected_count += 1
                errors.append(str(exc))
        if errors:
            status = "partial"
    except Exception as exc:  # noqa: BLE001
        status = "failed"
        errors.append(str(exc))

    failure_count = int(source.get("consecutive_failures", 0))
    if status == "failed":
        failure_count += 1
        next_backoff = backoff_until(source, failure_count)
        last_error = errors[-1] if errors else "unknown harvest failure"
    else:
        failure_count = 0
        next_backoff = None
        last_error = None

    snapshot = load_source_snapshot(conn, source_label)
    promotion_rate = source_promotion_rate(snapshot)
    null_metadata_count = max(
        int(snapshot["null_provider_count"]),
        int(snapshot["null_region_count"]),
        int(snapshot["null_country_count"]),
    )
    summary = {
        "source_label": source_label,
        "source_kind": source_kind,
        "fetched_count": fetched_count,
        "accepted_count": accepted_count,
        "deduped_count": deduped_count,
        "rejected_count": rejected_count,
        "null_metadata_count": null_metadata_count,
        "active_count_snapshot": int(snapshot["active_count"]),
        "candidate_count_snapshot": int(snapshot["candidate_count"]),
        "candidate_rejected_count_snapshot": int(snapshot["candidate_rejected_count"]),
        "source_promotion_rate_snapshot": promotion_rate,
        "error_summary": errors,
        "promoted_later_count": None,
    }
    finished_at = now_ts()
    health_score = health_score_for_run(
        {**source, "consecutive_failures": failure_count},
        status,
        fetched_count,
        accepted_count,
        deduped_count,
        rejected_count,
        snapshot,
    )

    conn.execute(
        """
        INSERT INTO proxy_harvest_runs (
            id, source_label, source_kind, fetched_count, accepted_count, deduped_count,
            rejected_count, status, summary_json, started_at, finished_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            run_id,
            source_label,
            source_kind,
            fetched_count,
            accepted_count,
            deduped_count,
            rejected_count,
            status,
            json.dumps(summary, ensure_ascii=False),
            started_at,
            finished_at,
        ),
    )
    conn.execute(
        """
        UPDATE proxy_harvest_sources
        SET consecutive_failures = ?,
            backoff_until = ?,
            last_run_finished_at = ?,
            last_run_status = ?,
            last_error = ?,
            health_score = ?,
            updated_at = ?
        WHERE source_label = ?
        """,
        (
            failure_count,
            next_backoff,
            finished_at,
            status,
            last_error,
            health_score,
            finished_at,
            source_label,
        ),
    )
    conn.commit()
    return {
        "run_id": run_id,
        "source_label": source_label,
        "source_kind": source_kind,
        "status": status,
        "fetched_count": fetched_count,
        "accepted_count": accepted_count,
        "deduped_count": deduped_count,
        "rejected_count": rejected_count,
        "null_metadata_count": null_metadata_count,
        "active_count_snapshot": int(snapshot["active_count"]),
        "candidate_count_snapshot": int(snapshot["candidate_count"]),
        "candidate_rejected_count_snapshot": int(snapshot["candidate_rejected_count"]),
        "source_promotion_rate_snapshot": promotion_rate,
        "health_score": health_score,
        "backoff_until": next_backoff,
        "errors": errors,
    }


def show_runs(conn: sqlite3.Connection, limit: int) -> dict[str, object]:
    sources = [
        {
            "source_label": row[0],
            "source_kind": row[1],
            "enabled": bool(row[2]),
            "interval_seconds": row[3],
            "base_proxy_score": row[4],
            "consecutive_failures": row[5],
            "backoff_until": row[6],
            "last_run_started_at": row[7],
            "last_run_finished_at": row[8],
            "last_run_status": row[9],
            "last_error": row[10],
            "health_score": row[11],
        }
        for row in conn.execute(
            """
            SELECT source_label, source_kind, enabled, interval_seconds, base_proxy_score,
                   consecutive_failures, backoff_until, last_run_started_at,
                   last_run_finished_at, last_run_status, last_error, health_score
            FROM proxy_harvest_sources
            ORDER BY source_label ASC
            """
        ).fetchall()
    ]
    runs = [
        {
            "id": row[0],
            "source_label": row[1],
            "source_kind": row[2],
            "fetched_count": row[3],
            "accepted_count": row[4],
            "deduped_count": row[5],
            "rejected_count": row[6],
            "status": row[7],
            "summary_json": json.loads(row[8] or "{}"),
            "started_at": row[9],
            "finished_at": row[10],
        }
        for row in conn.execute(
            """
            SELECT id, source_label, source_kind, fetched_count, accepted_count, deduped_count,
                   rejected_count, status, summary_json, started_at, finished_at
            FROM proxy_harvest_runs
            ORDER BY started_at DESC, id DESC
            LIMIT ?
            """,
            (limit,),
        ).fetchall()
    ]
    return {"sources": sources, "runs": runs}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Harvest candidate proxies into the long-running proxy source registry.",
    )
    parser.add_argument("--db", default=DEFAULT_DB_PATH, help="SQLite database path")
    parser.add_argument(
        "--config",
        default=default_config_path(),
        help="Proxy source config file path",
    )
    parser.add_argument(
        "--source-label",
        action="append",
        default=[],
        help="Run or inspect only the named source label; may be repeated",
    )
    parser.add_argument(
        "--once",
        action="store_true",
        help="Run a one-shot harvest for the selected sources",
    )
    parser.add_argument(
        "--show-runs",
        nargs="?",
        const=10,
        type=int,
        help="Print recent harvest sources and runs as JSON",
    )
    parser.add_argument("--file", help="Legacy single-source local text file input")
    parser.add_argument("--url", help="Legacy single-source text URL input")
    parser.add_argument(
        "--source-kind",
        help="Legacy single-source kind override: text_file/text_url/json_file/json_url",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    conn = sqlite3.connect(args.db)
    try:
        ensure_schema(conn)
        sources = load_legacy_sources(args)
        if not sources:
            sources = load_config_sources(args.config)
        sync_source_registry(conn, sources)

        if args.show_runs is not None:
            print(json.dumps(show_runs(conn, max(args.show_runs, 1)), ensure_ascii=False, indent=2))
            return 0

        selected_sources = select_sources(conn, args.source_label or None)
        if not selected_sources:
            print(
                json.dumps(
                    {
                        "status": "skipped",
                        "reason": "no enabled sources matched",
                        "source_labels": args.source_label,
                    },
                    ensure_ascii=False,
                )
            )
            return 0

        due_sources = filter_due_sources(conn, selected_sources, args.once)
        if not due_sources:
            print(
                json.dumps(
                    {
                        "status": "skipped",
                        "reason": "no due sources matched",
                        "source_labels": args.source_label,
                    },
                    ensure_ascii=False,
                )
            )
            return 0

        results = [run_source(conn, source) for source in due_sources]
        final_status = "completed"
        if any(item["status"] == "failed" for item in results):
            final_status = "failed"
        elif any(item["status"] == "partial" for item in results):
            final_status = "partial"
        print(
            json.dumps(
                {
                    "status": final_status,
                    "source_count": len(results),
                    "selected_source_count": len(selected_sources),
                    "results": results,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
        return 0 if final_status != "failed" else 1
    finally:
        conn.close()


if __name__ == "__main__":
    sys.exit(main())
