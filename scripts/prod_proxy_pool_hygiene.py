#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import sqlite3
import sys
import time
from pathlib import Path


ROOT_DIR = Path(__file__).resolve().parents[1]
MODE_DEMO_PUBLIC = "demo_public"
MODE_PROD_LIVE = "prod_live"
DEFAULT_SOURCE_CONCENTRATION_CAP_PERCENT = 75.0
DEFAULT_TOP1_SOURCE_KEEP_CANDIDATE_CAP = 40
DEFAULT_UNDERREPRESENTED_ACTIVE_THRESHOLD = 5


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Compact proxy candidate pools and quarantine low-quality sources without "
            "rebuilding the runtime binary."
        ),
    )
    parser.add_argument(
        "--db",
        default=(ROOT_DIR / "data" / "persona_pilot.db").as_posix(),
        help="SQLite database path",
    )
    parser.add_argument(
        "--mode",
        default=MODE_PROD_LIVE,
        help="Target runtime mode: demo_public or prod_live",
    )
    parser.add_argument(
        "--config",
        default=os.environ.get("PERSONA_PILOT_PROXY_HARVEST_CONFIG", ""),
        help="Optional proxy source config used to derive the selected source set",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Apply the planned quarantine and compaction actions",
    )
    parser.add_argument(
        "--promotion-threshold",
        type=float,
        default=0.03,
        help="Quarantine when promotion_rate falls below this threshold",
    )
    parser.add_argument(
        "--health-threshold",
        type=float,
        default=45.0,
        help="Quarantine when health_score falls below this threshold",
    )
    parser.add_argument(
        "--quarantine-seconds",
        type=int,
        default=6 * 60 * 60,
        help="Quarantine duration for low-quality sources",
    )
    parser.add_argument(
        "--quarantine-min-decision-count",
        type=int,
        default=20,
        help="Require at least active + rejected decisions before auto-quarantine",
    )
    parser.add_argument(
        "--max-active-for-quarantine",
        type=int,
        default=0,
        help="Only quarantine sources whose active_count is at or below this value",
    )
    parser.add_argument(
        "--keep-candidate-per-source",
        type=int,
        default=120,
        help="Maximum candidate rows to retain per eligible source",
    )
    parser.add_argument(
        "--candidate-min-per-source",
        type=int,
        default=40,
        help="Minimum candidate rows to retain per non-quarantined source",
    )
    parser.add_argument(
        "--candidate-per-active",
        type=int,
        default=20,
        help="Candidate cap grows by this multiplier times active_count",
    )
    parser.add_argument(
        "--source-concentration-cap-percent",
        type=float,
        default=DEFAULT_SOURCE_CONCENTRATION_CAP_PERCENT,
        help="When top1 active share exceeds this cap, tighten candidate retention for the dominant source",
    )
    parser.add_argument(
        "--top1-source-keep-candidate-cap",
        type=int,
        default=DEFAULT_TOP1_SOURCE_KEEP_CANDIDATE_CAP,
        help="Tightened candidate cap applied to the top1 source when concentration is too high",
    )
    parser.add_argument(
        "--underrepresented-active-threshold",
        type=int,
        default=DEFAULT_UNDERREPRESENTED_ACTIVE_THRESHOLD,
        help="Sources below this active inventory are treated as underrepresented during concentration balancing",
    )
    parser.add_argument(
        "--underrepresented-source-keep-candidate-cap",
        type=int,
        default=None,
        help="Optional candidate cap for underrepresented non-top1 sources; defaults to keep-candidate-per-source",
    )
    parser.add_argument(
        "--keep-rejected-per-source",
        type=int,
        default=20,
        help="Maximum rejected rows to retain per eligible source",
    )
    parser.add_argument(
        "--rejected-min-per-source",
        type=int,
        default=10,
        help="Minimum rejected rows to retain per non-quarantined source",
    )
    parser.add_argument(
        "--rejected-per-active",
        type=int,
        default=5,
        help="Rejected cap grows by this multiplier times active_count",
    )
    parser.add_argument(
        "--source-label",
        action="append",
        default=[],
        help="Optional source_label filter; may be passed multiple times",
    )
    parser.add_argument(
        "--summary-json",
        help="Optional JSON summary output path",
    )
    parser.add_argument(
        "--protect-recent-seconds",
        type=int,
        default=600,
        help="Do not delete proxies referenced by recent verify/browser activity within this window",
    )
    return parser.parse_args()


def normalize_mode(raw_mode: str) -> str:
    value = raw_mode.strip().lower().replace("-", "_")
    if value == MODE_PROD_LIVE:
        return MODE_PROD_LIVE
    return MODE_DEMO_PUBLIC


def to_int(value: object, default: int = 0) -> int:
    try:
        return int(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return default


def to_float(value: object, default: float = 0.0) -> float:
    try:
        return float(value)  # type: ignore[arg-type]
    except (TypeError, ValueError):
        return default


def parse_ts(value: object) -> int:
    try:
        if value is None:
            return 0
        text = str(value).strip()
        if not text:
            return 0
        return int(float(text))
    except (TypeError, ValueError):
        return 0


def bool_flag(value: object) -> bool:
    return bool(to_int(value))


def derive_ratio_percent(active_total: int, candidate_total: int, rejected_total: int) -> float:
    denominator = active_total + candidate_total + rejected_total
    if denominator <= 0:
        return 0.0
    return round((active_total / denominator) * 100.0, 4)


def share_percent(numerator: int, denominator: int) -> float:
    if denominator <= 0:
        return 0.0
    return round((numerator / denominator) * 100.0, 4)


def open_db(db_path: Path) -> sqlite3.Connection:
    conn = sqlite3.connect(db_path.as_posix(), timeout=30)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    conn.execute("PRAGMA busy_timeout = 30000")
    return conn


def table_exists(conn: sqlite3.Connection, table_name: str) -> bool:
    row = conn.execute(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ? LIMIT 1",
        (table_name,),
    ).fetchone()
    return row is not None


def column_exists(conn: sqlite3.Connection, table_name: str, column_name: str) -> bool:
    if not table_exists(conn, table_name):
        return False
    rows = conn.execute(f"PRAGMA table_info({table_name})").fetchall()
    return any(str(row["name"]) == column_name for row in rows)


def load_protected_proxy_ids(
    conn: sqlite3.Connection,
    *,
    now_ts: int,
    protect_recent_seconds: int,
) -> set[str]:
    protected: set[str] = set()
    if table_exists(conn, "tasks"):
        proxy_id_expr = (
            "COALESCE(NULLIF(TRIM(proxy_id), ''), NULLIF(TRIM(CAST(json_extract(input_json, '$.proxy_id') AS TEXT)), ''))"
            if column_exists(conn, "tasks", "proxy_id")
            else "NULLIF(TRIM(CAST(json_extract(input_json, '$.proxy_id') AS TEXT)), '')"
        )
        task_rows = conn.execute(
            f"""
            SELECT DISTINCT {proxy_id_expr} AS protected_proxy_id
            FROM tasks
            WHERE {proxy_id_expr} IS NOT NULL
              AND (
                status IN ('queued', 'running')
                OR (
                  kind IN ('verify_proxy', 'open_page', 'get_html', 'get_title', 'get_final_url', 'extract_text')
                  AND CAST(COALESCE(started_at, queued_at, finished_at, created_at, '0') AS INTEGER) >= ? - ?
                )
              )
            """,
            (now_ts, max(protect_recent_seconds, 0)),
        ).fetchall()
        protected.update(
            str(row["protected_proxy_id"]).strip()
            for row in task_rows
            if str(row["protected_proxy_id"] or "").strip()
        )
    if table_exists(conn, "proxy_session_bindings"):
        binding_rows = conn.execute(
            """
            SELECT DISTINCT proxy_id
            FROM proxy_session_bindings
            WHERE proxy_id IS NOT NULL
              AND TRIM(proxy_id) != ''
              AND (expires_at IS NULL OR CAST(expires_at AS INTEGER) > ?)
            """,
            (now_ts,),
        ).fetchall()
        protected.update(
            str(row["proxy_id"]).strip()
            for row in binding_rows
            if str(row["proxy_id"] or "").strip()
        )
    return protected


def resolve_path(raw_path: str) -> Path:
    expanded = Path(os.path.expandvars(os.path.expanduser(raw_path.strip())))
    if expanded.is_absolute():
        return expanded.resolve()
    return (ROOT_DIR / expanded).resolve()


def load_config_source_map(config_path: str) -> dict[str, dict[str, object]]:
    if not config_path.strip():
        return {}
    path = resolve_path(config_path)
    if not path.exists():
        raise SystemExit(f"config not found: {path}")
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, list):
        items = payload
    elif isinstance(payload, dict) and isinstance(payload.get("items"), list):
        items = payload["items"]
    else:
        raise SystemExit("proxy source config must be an array or an object with items[]")
    source_map: dict[str, dict[str, object]] = {}
    for item in items:
        if not isinstance(item, dict):
            continue
        source_label = str(item.get("source_label") or "").strip()
        if not source_label:
            continue
        source_map[source_label] = {
            "source_label": source_label,
            "source_kind": str(item.get("source_kind") or "").strip() or "unknown",
            "source_tier": item.get("source_tier"),
            "for_demo": bool(item.get("for_demo", True)),
            "for_prod": bool(item.get("for_prod", False)),
            "validation_mode": item.get("validation_mode"),
            "expected_geo_quality": item.get("expected_geo_quality"),
            "cost_class": item.get("cost_class"),
            "enabled": bool(item.get("enabled", True)),
            "config_json": item.get("config_json") or {},
            "interval_seconds": max(to_int(item.get("interval_seconds"), 300), 30),
            "base_proxy_score": to_float(item.get("base_proxy_score"), 1.0),
        }
    return source_map


def selected_labels_from_config(
    source_map: dict[str, dict[str, object]],
    mode: str,
) -> list[str]:
    flag = "for_prod" if mode == MODE_PROD_LIVE else "for_demo"
    return [
        source_label
        for source_label, metadata in source_map.items()
        if bool(metadata.get("enabled")) and bool(metadata.get(flag))
    ]


def load_target_sources(
    conn: sqlite3.Connection,
    mode: str,
    source_labels: list[str],
    source_map: dict[str, dict[str, object]],
) -> list[dict[str, object]]:
    mode_column = "for_prod" if mode == MODE_PROD_LIVE else "for_demo"
    params: list[object] = []
    if source_labels:
        placeholders = ", ".join("?" for _ in source_labels)
        where_clause = f"s.source_label IN ({placeholders})"
        params.extend(source_labels)
    else:
        where_clause = f"s.enabled = 1 AND s.{mode_column} = 1"
    sql = f"""
        WITH source_counts AS (
            SELECT
                source_label,
                SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END) AS active_count,
                SUM(CASE WHEN status = 'candidate' THEN 1 ELSE 0 END) AS candidate_count,
                SUM(CASE WHEN status = 'candidate_rejected' THEN 1 ELSE 0 END) AS rejected_count
            FROM proxies
            WHERE source_label IS NOT NULL
            GROUP BY source_label
        )
        SELECT
            s.source_label,
            s.source_tier,
            s.for_demo,
            s.for_prod,
            s.validation_mode,
            s.expected_geo_quality,
            s.cost_class,
            s.enabled,
            s.quarantine_until,
            s.health_score,
            COALESCE(c.active_count, 0) AS active_count,
            COALESCE(c.candidate_count, 0) AS candidate_count,
            COALESCE(c.rejected_count, 0) AS rejected_count
        FROM proxy_harvest_sources AS s
        LEFT JOIN source_counts AS c ON c.source_label = s.source_label
        WHERE {where_clause}
        ORDER BY s.source_label ASC
    """
    rows = conn.execute(sql, params).fetchall()
    sources: list[dict[str, object]] = []
    seen_labels: set[str] = set()
    for row in rows:
        active_count = to_int(row["active_count"])
        candidate_count = to_int(row["candidate_count"])
        rejected_count = to_int(row["rejected_count"])
        decision_total = active_count + rejected_count
        promotion_rate = (
            round(active_count / decision_total, 6)
            if decision_total > 0
            else 0.0
        )
        sources.append(
            {
                "source_label": str(row["source_label"]),
                "source_tier": row["source_tier"],
                "for_demo": bool_flag(row["for_demo"]),
                "for_prod": bool_flag(row["for_prod"]),
                "validation_mode": row["validation_mode"],
                "expected_geo_quality": row["expected_geo_quality"],
                "cost_class": row["cost_class"],
                "enabled": bool_flag(row["enabled"]),
                "quarantine_until": row["quarantine_until"],
                "health_score": round(to_float(row["health_score"]), 6),
                "active_count": active_count,
                "candidate_count": candidate_count,
                "rejected_count": rejected_count,
                "decision_total": decision_total,
                "promotion_rate": promotion_rate,
            }
        )
        if str(row["source_label"]) in source_map:
            sources[-1].update(source_map[str(row["source_label"])])
        seen_labels.add(str(row["source_label"]))
    missing_labels = [label for label in source_labels if label not in seen_labels]
    for source_label in missing_labels:
        counts = conn.execute(
            """
            SELECT
                SUM(CASE WHEN status = 'active' THEN 1 ELSE 0 END) AS active_count,
                SUM(CASE WHEN status = 'candidate' THEN 1 ELSE 0 END) AS candidate_count,
                SUM(CASE WHEN status = 'candidate_rejected' THEN 1 ELSE 0 END) AS rejected_count
            FROM proxies
            WHERE source_label = ?
            """,
            (source_label,),
        ).fetchone()
        active_count = to_int(counts["active_count"]) if counts else 0
        candidate_count = to_int(counts["candidate_count"]) if counts else 0
        rejected_count = to_int(counts["rejected_count"]) if counts else 0
        decision_total = active_count + rejected_count
        promotion_rate = (
            round(active_count / decision_total, 6)
            if decision_total > 0
            else 0.0
        )
        metadata = source_map.get(source_label, {})
        sources.append(
            {
                "source_label": source_label,
                "source_tier": metadata.get("source_tier"),
                "for_demo": bool(metadata.get("for_demo", False)),
                "for_prod": bool(metadata.get("for_prod", False)),
                "validation_mode": metadata.get("validation_mode"),
                "expected_geo_quality": metadata.get("expected_geo_quality"),
                "cost_class": metadata.get("cost_class"),
                "enabled": bool(metadata.get("enabled", True)),
                "quarantine_until": None,
                "health_score": 100.0,
                "active_count": active_count,
                "candidate_count": candidate_count,
                "rejected_count": rejected_count,
                "decision_total": decision_total,
                "promotion_rate": promotion_rate,
            }
        )
    return sources


def sync_source_metadata(
    conn: sqlite3.Connection,
    source_map: dict[str, dict[str, object]],
    source_labels: list[str],
    *,
    now_ts: int,
) -> int:
    if not source_map or not source_labels:
        return 0
    synced = 0
    for source_label in source_labels:
        metadata = source_map.get(source_label)
        if not metadata:
            continue
        current_row = conn.execute(
            """
            SELECT expected_geo_quality
            FROM proxy_harvest_sources
            WHERE source_label = ?
            """,
            (source_label,),
        ).fetchone()
        current_geo_quality = str(current_row["expected_geo_quality"] or "").strip() if current_row else ""
        incoming_geo_quality = str(metadata.get("expected_geo_quality") or "").strip()
        if incoming_geo_quality.lower() in {"", "unknown"} and current_geo_quality.lower() not in {"", "unknown"}:
            expected_geo_quality = current_geo_quality
        else:
            expected_geo_quality = metadata.get("expected_geo_quality")
        conn.execute(
            """
            UPDATE proxy_harvest_sources
            SET source_tier = ?,
                for_demo = ?,
                for_prod = ?,
                validation_mode = ?,
                expected_geo_quality = ?,
                cost_class = ?,
                enabled = ?,
                updated_at = ?
            WHERE source_label = ?
            """,
            (
                metadata.get("source_tier"),
                1 if bool(metadata.get("for_demo")) else 0,
                1 if bool(metadata.get("for_prod")) else 0,
                metadata.get("validation_mode"),
                expected_geo_quality,
                metadata.get("cost_class"),
                1 if bool(metadata.get("enabled", True)) else 0,
                str(now_ts),
                source_label,
            ),
        )
        synced += 1
    return synced


def source_is_currently_quarantined(source: dict[str, object], now_ts: int) -> bool:
    return parse_ts(source.get("quarantine_until")) > now_ts


def should_quarantine_source(
    source: dict[str, object],
    args: argparse.Namespace,
) -> bool:
    if to_int(source.get("decision_total")) < args.quarantine_min_decision_count:
        return False
    if to_int(source.get("active_count")) > args.max_active_for_quarantine:
        return False
    if to_int(source.get("candidate_count")) + to_int(source.get("rejected_count")) <= 0:
        return False
    promotion_rate = to_float(source.get("promotion_rate"))
    health_score = to_float(source.get("health_score"), 100.0)
    return (
        promotion_rate < args.promotion_threshold
        or health_score < args.health_threshold
    )


def candidate_keep_limit(source: dict[str, object], args: argparse.Namespace) -> int:
    if bool(source.get("post_quarantined")):
        return 0
    dynamic_limit = max(
        args.candidate_min_per_source,
        to_int(source.get("active_count")) * args.candidate_per_active,
    )
    return max(0, min(args.keep_candidate_per_source, dynamic_limit))


def rejected_keep_limit(source: dict[str, object], args: argparse.Namespace) -> int:
    if bool(source.get("post_quarantined")):
        return 0
    dynamic_limit = max(
        args.rejected_min_per_source,
        to_int(source.get("active_count")) * args.rejected_per_active,
    )
    return max(0, min(args.keep_rejected_per_source, dynamic_limit))


def summarize_source_concentration(
    rows: list[dict[str, object]],
    *,
    eligible_predicate,
) -> dict[str, object]:
    eligible_rows = [row for row in rows if eligible_predicate(row)]
    active_total = sum(to_int(row.get("active_count")) for row in eligible_rows)
    if not eligible_rows or active_total <= 0:
        return {
            "active_total": active_total,
            "top1_source_label": None,
            "top1_source_active_count": 0,
            "source_concentration_top1_percent": 0.0,
        }
    ranked = sorted(
        eligible_rows,
        key=lambda row: (
            -to_int(row.get("active_count")),
            str(row.get("source_label") or ""),
        ),
    )
    top1 = ranked[0]
    top1_active = to_int(top1.get("active_count"))
    return {
        "active_total": active_total,
        "top1_source_label": str(top1.get("source_label") or "").strip() or None,
        "top1_source_active_count": top1_active,
        "source_concentration_top1_percent": share_percent(top1_active, active_total),
    }


def adjusted_candidate_keep_limit(
    source: dict[str, object],
    args: argparse.Namespace,
    concentration_summary: dict[str, object],
) -> tuple[int, int, str | None]:
    base_limit = candidate_keep_limit(source, args)
    adjusted_limit = base_limit
    adjustment_reason: str | None = None
    if bool(source.get("post_quarantined")):
        return adjusted_limit, base_limit, adjustment_reason

    top1_source_label = str(concentration_summary.get("top1_source_label") or "").strip()
    top1_share_percent = to_float(
        concentration_summary.get("source_concentration_top1_percent")
    )
    concentration_cap = max(to_float(args.source_concentration_cap_percent), 0.0)
    if not top1_source_label or top1_share_percent <= concentration_cap:
        return adjusted_limit, base_limit, adjustment_reason

    source_label = str(source.get("source_label") or "").strip()
    if source_label == top1_source_label:
        tightened_cap = min(
            max(args.keep_candidate_per_source, 0),
            max(to_int(args.top1_source_keep_candidate_cap), 0),
        )
        adjusted_limit = min(adjusted_limit, tightened_cap)
        if adjusted_limit < base_limit:
            adjustment_reason = "top1_source_cap"
        return adjusted_limit, base_limit, adjustment_reason

    active_count = to_int(source.get("active_count"))
    underrepresented_threshold = max(to_int(args.underrepresented_active_threshold), 0)
    if active_count >= underrepresented_threshold:
        return adjusted_limit, base_limit, adjustment_reason

    bonus_cap_raw = getattr(args, "underrepresented_source_keep_candidate_cap", None)
    if bonus_cap_raw is None:
        bonus_cap = max(args.keep_candidate_per_source, 0)
    else:
        bonus_cap = max(to_int(bonus_cap_raw), 0)
    boosted_limit = min(
        max(args.keep_candidate_per_source, 0),
        max(bonus_cap, max(args.candidate_min_per_source, 0)),
    )
    adjusted_limit = max(adjusted_limit, boosted_limit)
    if adjusted_limit > base_limit:
        adjustment_reason = "underrepresented_source_bonus"
    return adjusted_limit, base_limit, adjustment_reason


def freshness_score(row: sqlite3.Row) -> int:
    return max(
        parse_ts(row["last_verify_at"]),
        parse_ts(row["last_seen_at"]),
        parse_ts(row["promoted_at"]),
        parse_ts(row["created_at"]),
    )


def metadata_score(row: sqlite3.Row) -> int:
    return sum(
        1
        for key in (
            "provider",
            "region",
            "country",
            "last_exit_country",
            "last_exit_region",
        )
        if str(row[key] or "").strip()
    )


def candidate_rank(row: sqlite3.Row) -> tuple[int, int, float, int, int, str]:
    verify_status = str(row["last_verify_status"] or "").strip().lower()
    verify_rank = 3 if verify_status == "ok" else 1 if not verify_status else 0
    return (
        verify_rank,
        metadata_score(row),
        round(to_float(row["score"]), 6),
        freshness_score(row),
        parse_ts(row["created_at"]),
        str(row["id"]),
    )


def rejected_rank(row: sqlite3.Row) -> tuple[int, int, float, int, int, str]:
    error_rank = 1 if str(row["last_probe_error_category"] or "").strip() else 0
    return (
        freshness_score(row),
        error_rank,
        round(to_float(row["score"]), 6),
        metadata_score(row),
        parse_ts(row["created_at"]),
        str(row["id"]),
    )


def load_source_rows(conn: sqlite3.Connection, source_label: str) -> list[sqlite3.Row]:
    return conn.execute(
        """
        SELECT
            id,
            status,
            score,
            provider,
            region,
            country,
            last_verify_status,
            last_verify_at,
            last_seen_at,
            created_at,
            promoted_at,
            last_probe_error_category,
            last_exit_country,
            last_exit_region
        FROM proxies
        WHERE source_label = ?
          AND status IN ('candidate', 'candidate_rejected')
        """,
        (source_label,),
    ).fetchall()


def chunked(items: list[str], size: int) -> list[list[str]]:
    return [items[index:index + size] for index in range(0, len(items), size)]


def summarize_sources(sources: list[dict[str, object]], *, now_ts: int) -> dict[str, object]:
    eligible_sources = [
        source for source in sources if not source_is_currently_quarantined(source, now_ts)
    ]
    active_total = sum(to_int(source.get("active_count")) for source in eligible_sources)
    candidate_total = sum(to_int(source.get("candidate_count")) for source in eligible_sources)
    rejected_total = sum(to_int(source.get("rejected_count")) for source in eligible_sources)
    concentration = summarize_source_concentration(
        sources,
        eligible_predicate=lambda source: not source_is_currently_quarantined(source, now_ts),
    )
    return {
        "source_count": len(sources),
        "eligible_source_count": len(eligible_sources),
        "active_total": active_total,
        "candidate_total": candidate_total,
        "rejected_total": rejected_total,
        "estimated_effective_active_ratio_percent": derive_ratio_percent(
            active_total,
            candidate_total,
            rejected_total,
        ),
        "top1_source_label": concentration["top1_source_label"],
        "top1_source_active_count": concentration["top1_source_active_count"],
        "source_concentration_top1_percent": concentration[
            "source_concentration_top1_percent"
        ],
    }


def summarize_projected(actions: list[dict[str, object]]) -> dict[str, object]:
    active_total = 0
    candidate_total = 0
    rejected_total = 0
    eligible_source_count = 0
    for action in actions:
        if bool(action.get("post_quarantined")):
            continue
        eligible_source_count += 1
        active_total += to_int(action.get("active_count"))
        candidate_total += to_int(action.get("candidate_keep_count"))
        rejected_total += to_int(action.get("rejected_keep_count"))
    concentration = summarize_source_concentration(
        actions,
        eligible_predicate=lambda action: not bool(action.get("post_quarantined")),
    )
    return {
        "source_count": len(actions),
        "eligible_source_count": eligible_source_count,
        "active_total": active_total,
        "candidate_total": candidate_total,
        "rejected_total": rejected_total,
        "estimated_effective_active_ratio_percent": derive_ratio_percent(
            active_total,
            candidate_total,
            rejected_total,
        ),
        "top1_source_label": concentration["top1_source_label"],
        "top1_source_active_count": concentration["top1_source_active_count"],
        "source_concentration_top1_percent": concentration[
            "source_concentration_top1_percent"
        ],
    }


def build_actions(
    conn: sqlite3.Connection,
    sources: list[dict[str, object]],
    args: argparse.Namespace,
    *,
    now_ts: int,
) -> list[dict[str, object]]:
    protected_proxy_ids = load_protected_proxy_ids(
        conn,
        now_ts=now_ts,
        protect_recent_seconds=args.protect_recent_seconds,
    )
    concentration_summary = summarize_source_concentration(
        sources,
        eligible_predicate=lambda source: not source_is_currently_quarantined(source, now_ts),
    )
    concentration_top1_source_label = str(
        concentration_summary.get("top1_source_label") or ""
    ).strip()
    concentration_top1_share_percent = to_float(
        concentration_summary.get("source_concentration_top1_percent")
    )
    concentration_active_total = to_int(concentration_summary.get("active_total"))
    actions: list[dict[str, object]] = []
    for source in sources:
        existing_quarantined = source_is_currently_quarantined(source, now_ts)
        low_quality = should_quarantine_source(source, args)
        source["post_quarantined"] = existing_quarantined or low_quality
        (
            candidate_limit,
            candidate_limit_base,
            candidate_keep_adjustment,
        ) = adjusted_candidate_keep_limit(source, args, concentration_summary)
        rejected_limit = rejected_keep_limit(source, args)
        rows = load_source_rows(conn, str(source["source_label"]))
        candidate_rows = [row for row in rows if str(row["status"]) == "candidate"]
        rejected_rows = [row for row in rows if str(row["status"]) == "candidate_rejected"]
        candidate_rows.sort(key=candidate_rank, reverse=True)
        rejected_rows.sort(key=rejected_rank, reverse=True)
        kept_candidate_ids = [str(row["id"]) for row in candidate_rows[:candidate_limit]]
        kept_rejected_ids = [str(row["id"]) for row in rejected_rows[:rejected_limit]]
        protected_candidate_ids = [
            str(row["id"])
            for row in candidate_rows[candidate_limit:]
            if str(row["id"]) in protected_proxy_ids
        ]
        protected_rejected_ids = [
            str(row["id"])
            for row in rejected_rows[rejected_limit:]
            if str(row["id"]) in protected_proxy_ids
        ]
        candidate_delete_ids = [
            str(row["id"])
            for row in candidate_rows[candidate_limit:]
            if str(row["id"]) not in protected_proxy_ids
        ]
        rejected_delete_ids = [
            str(row["id"])
            for row in rejected_rows[rejected_limit:]
            if str(row["id"]) not in protected_proxy_ids
        ]
        delete_ids = candidate_delete_ids + rejected_delete_ids
        quarantine_until = None
        quarantine_action = "keep"
        if low_quality:
            quarantine_until = str(now_ts + max(args.quarantine_seconds, 0))
            if parse_ts(source.get("quarantine_until")) < parse_ts(quarantine_until):
                quarantine_action = "set"
        elif existing_quarantined:
            quarantine_action = "keep_existing"
        source_active_count = to_int(source.get("active_count"))
        actions.append(
            {
                **source,
                "existing_quarantined": existing_quarantined,
                "low_quality": low_quality,
                "post_quarantined": bool(source["post_quarantined"]),
                "source_active_share_percent": share_percent(
                    source_active_count,
                    concentration_active_total,
                ),
                "top1_source_label": concentration_top1_source_label or None,
                "top1_source_share_percent": concentration_top1_share_percent,
                "quarantine_action": quarantine_action,
                "new_quarantine_until": quarantine_until,
                "candidate_keep_limit_base": candidate_limit_base,
                "candidate_keep_limit": candidate_limit,
                "candidate_keep_adjustment": candidate_keep_adjustment,
                "candidate_keep_count": len(kept_candidate_ids) + len(protected_candidate_ids),
                "candidate_delete_count": len(candidate_delete_ids),
                "rejected_keep_limit": rejected_limit,
                "rejected_keep_count": len(kept_rejected_ids) + len(protected_rejected_ids),
                "rejected_delete_count": len(rejected_delete_ids),
                "protected_keep_count": len(protected_candidate_ids) + len(protected_rejected_ids),
                "delete_count": len(delete_ids),
                "delete_ids": delete_ids,
            }
        )
    return actions


def apply_actions(
    conn: sqlite3.Connection,
    actions: list[dict[str, object]],
    *,
    now_ts: int,
) -> dict[str, object]:
    conn.execute("BEGIN")
    try:
        for action in actions:
            if action["quarantine_action"] == "set":
                conn.execute(
                    """
                    UPDATE proxy_harvest_sources
                    SET quarantine_until = ?, updated_at = ?
                    WHERE source_label = ?
                    """,
                    (
                        action["new_quarantine_until"],
                        str(now_ts),
                        action["source_label"],
                    ),
                )
            for chunk in chunked(action["delete_ids"], 500):
                if not chunk:
                    continue
                placeholders = ", ".join("?" for _ in chunk)
                conn.execute(
                    f"DELETE FROM proxies WHERE id IN ({placeholders})",
                    chunk,
                )
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    return {
        "deleted_proxy_rows": sum(to_int(action.get("delete_count")) for action in actions),
        "quarantined_sources": sum(1 for action in actions if action["quarantine_action"] == "set"),
        "deleted_proxy_rows_by_source": {
            str(action.get("source_label")): to_int(action.get("delete_count"))
            for action in actions
            if to_int(action.get("delete_count")) > 0
        },
    }


def sanitize_actions(actions: list[dict[str, object]]) -> list[dict[str, object]]:
    sanitized: list[dict[str, object]] = []
    for action in actions:
        cleaned = dict(action)
        cleaned.pop("delete_ids", None)
        sanitized.append(cleaned)
    return sanitized


def main() -> int:
    args = parse_args()
    mode = normalize_mode(args.mode)
    db_path = Path(args.db).expanduser().resolve()
    if not db_path.exists():
        raise SystemExit(f"database not found: {db_path}")
    source_map = load_config_source_map(args.config)
    selected_labels = list(dict.fromkeys(args.source_label))
    if not selected_labels and source_map:
        selected_labels = selected_labels_from_config(source_map, mode)
    if mode != MODE_PROD_LIVE and not args.source_label:
        print(
            "warning: this tool is optimized for prod_live; pass --source-label for narrower use outside prod_live",
            file=sys.stderr,
        )

    now_ts = int(time.time())
    conn = open_db(db_path)
    try:
        sources = load_target_sources(conn, mode, selected_labels, source_map)
        if not sources:
            raise SystemExit(f"no enabled {mode} sources matched the current filter")
        before_summary = summarize_sources(sources, now_ts=now_ts)
        actions = build_actions(conn, sources, args, now_ts=now_ts)
        projected_summary = summarize_projected(actions)
        apply_result = {
            "deleted_proxy_rows": 0,
            "quarantined_sources": 0,
            "synced_source_metadata_rows": 0,
        }
        after_summary = projected_summary
        if args.apply:
            synced_metadata_rows = sync_source_metadata(
                conn,
                source_map,
                selected_labels,
                now_ts=now_ts,
            )
            if synced_metadata_rows > 0:
                conn.commit()
            apply_result = apply_actions(conn, actions, now_ts=now_ts)
            apply_result["synced_source_metadata_rows"] = synced_metadata_rows
            reloaded_sources = load_target_sources(conn, mode, selected_labels, source_map)
            after_summary = summarize_sources(reloaded_sources, now_ts=now_ts)
        payload = {
            "generated_at": now_ts,
            "db_path": db_path.as_posix(),
            "mode": mode,
            "dry_run": not args.apply,
            "config_path": resolve_path(args.config).as_posix() if args.config.strip() else None,
            "selected_source_labels": selected_labels,
            "thresholds": {
                "promotion_threshold": args.promotion_threshold,
                "health_threshold": args.health_threshold,
                "quarantine_seconds": args.quarantine_seconds,
                "quarantine_min_decision_count": args.quarantine_min_decision_count,
                "max_active_for_quarantine": args.max_active_for_quarantine,
                "keep_candidate_per_source": args.keep_candidate_per_source,
                "candidate_min_per_source": args.candidate_min_per_source,
                "candidate_per_active": args.candidate_per_active,
                "source_concentration_cap_percent": args.source_concentration_cap_percent,
                "top1_source_keep_candidate_cap": args.top1_source_keep_candidate_cap,
                "underrepresented_active_threshold": args.underrepresented_active_threshold,
                "underrepresented_source_keep_candidate_cap": args.underrepresented_source_keep_candidate_cap,
                "keep_rejected_per_source": args.keep_rejected_per_source,
                "rejected_min_per_source": args.rejected_min_per_source,
                "rejected_per_active": args.rejected_per_active,
                "protect_recent_seconds": args.protect_recent_seconds,
            },
            "before": before_summary,
            "projected_after": projected_summary,
            "after": after_summary,
            "apply_result": apply_result,
            "source_actions": sanitize_actions(actions),
        }
        text = json.dumps(payload, ensure_ascii=False, indent=2)
        print(text)
        if args.summary_json:
            output_path = Path(args.summary_json).expanduser().resolve()
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(text + "\n", encoding="utf-8")
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
