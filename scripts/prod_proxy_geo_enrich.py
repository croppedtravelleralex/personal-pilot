#!/usr/bin/env python3
from __future__ import annotations

import argparse
import ipaddress
import json
import socket
import sqlite3
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

from prod_proxy_pool_hygiene import (
    MODE_PROD_LIVE,
    load_config_source_map,
    normalize_mode,
    open_db,
    resolve_path,
    selected_labels_from_config,
    to_int,
)


ROOT_DIR = Path(__file__).resolve().parents[1]
GEO_VERIFY_SOURCE = "geoip_host_enrich"
DEFAULT_GEO_PROVIDER_ORDER = ("ip-api", "ipapi", "ipinfo")
UNKNOWN_COUNTRY_VALUES = {"", "unknown", "null", "<null>"}
UNKNOWN_REGION_VALUES = {"", "unknown", "global", "null", "<null>"}
US_WEST_REGION_CODES = {
    "AK",
    "AZ",
    "CA",
    "CO",
    "HI",
    "ID",
    "MT",
    "NM",
    "NV",
    "OR",
    "UT",
    "WA",
    "WY",
}
US_WEST_REGION_NAMES = {
    "alaska",
    "arizona",
    "california",
    "colorado",
    "hawaii",
    "idaho",
    "montana",
    "nevada",
    "new mexico",
    "oregon",
    "utah",
    "washington",
    "wyoming",
}
EU_WEST_COUNTRIES = {
    "AT",
    "BE",
    "CH",
    "CZ",
    "DE",
    "DK",
    "ES",
    "FI",
    "FR",
    "GB",
    "IE",
    "IT",
    "LU",
    "NL",
    "NO",
    "PL",
    "PT",
    "SE",
    "UK",
}
AP_SOUTHEAST_COUNTRIES = {
    "AU",
    "HK",
    "ID",
    "IN",
    "KR",
    "MY",
    "NZ",
    "PH",
    "SG",
    "TH",
    "TW",
    "VN",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Infer proxy geo metadata from the proxy host IP so prod_live can stop scoring "
            "everything as unknown/global before a deeper runtime probe is available."
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
        default="",
        help="Optional proxy source config used to derive the selected source set",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Apply inferred geo updates to the database",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=200,
        help="Maximum proxy rows to enrich in one run",
    )
    parser.add_argument(
        "--only-status",
        action="append",
        default=[],
        help="Restrict to one or more proxy statuses; may be repeated or comma-separated",
    )
    parser.add_argument(
        "--source-label",
        action="append",
        default=[],
        help="Optional source_label filter; may be repeated",
    )
    parser.add_argument(
        "--force-refresh",
        action="store_true",
        help="Refresh geo metadata even when region/country fields are already populated",
    )
    parser.add_argument(
        "--disable-hostname-resolution",
        action="store_true",
        help="Only enrich rows whose host is already a public IP literal",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=int,
        default=8,
        help="Network timeout for DNS and geo lookup requests",
    )
    parser.add_argument(
        "--sleep-ms",
        type=int,
        default=75,
        help="Small delay between outbound geo lookup requests",
    )
    parser.add_argument(
        "--summary-json",
        help="Optional JSON summary output path",
    )
    return parser.parse_args()


def normalize_statuses(raw_values: list[str]) -> list[str]:
    values: list[str] = []
    for raw in raw_values:
        for part in str(raw).split(","):
            status = part.strip().lower()
            if status:
                values.append(status)
    if not values:
        return ["active"]
    deduped: list[str] = []
    seen: set[str] = set()
    for value in values:
        if value in seen:
            continue
        seen.add(value)
        deduped.append(value)
    return deduped


def is_public_ip(ip_text: str) -> bool:
    try:
        ip_obj = ipaddress.ip_address(ip_text)
    except ValueError:
        return False
    return not (
        ip_obj.is_private
        or ip_obj.is_loopback
        or ip_obj.is_link_local
        or ip_obj.is_multicast
        or ip_obj.is_reserved
        or ip_obj.is_unspecified
    )


def normalize_country(value: object) -> str | None:
    text = str(value or "").strip().upper()
    if text.lower() in UNKNOWN_COUNTRY_VALUES:
        return None
    return text or None


def normalize_region(value: object) -> str | None:
    text = str(value or "").strip().lower()
    if text in UNKNOWN_REGION_VALUES:
        return None
    return text or None


def slugify(value: str) -> str:
    chars: list[str] = []
    previous_dash = False
    for ch in value.strip().lower():
        if ch.isalnum():
            chars.append(ch)
            previous_dash = False
            continue
        if previous_dash:
            continue
        chars.append("-")
        previous_dash = True
    return "".join(chars).strip("-")


def canonical_region(
    country_code: str | None,
    region_name: str | None,
    region_code: str | None,
) -> str | None:
    country = normalize_country(country_code)
    region_name_norm = str(region_name or "").strip().lower()
    region_code_norm = str(region_code or "").strip().upper()
    if not country:
        return slugify(region_name_norm) or None
    if country in {"GB", "UK"}:
        return "eu-west"
    if country == "US":
        if region_code_norm in US_WEST_REGION_CODES or region_name_norm in US_WEST_REGION_NAMES:
            return "us-west"
        return "us-east"
    if country in EU_WEST_COUNTRIES:
        return "eu-west"
    if country == "JP":
        return "jp"
    if country == "CN":
        return "cn"
    if country in AP_SOUTHEAST_COUNTRIES:
        return "ap-southeast"
    return country.lower()


def append_verify_source(existing: object) -> str:
    current = str(existing or "").strip()
    if not current:
        return GEO_VERIFY_SOURCE
    parts = [part.strip() for part in current.split("+") if part.strip()]
    if GEO_VERIFY_SOURCE not in parts:
        parts.append(GEO_VERIFY_SOURCE)
    return "+".join(parts)


def needs_geo_refresh(row: sqlite3.Row, *, force_refresh: bool) -> bool:
    if force_refresh:
        return True
    country = normalize_country(row["country"])
    region = normalize_region(row["region"])
    exit_country = normalize_country(row["last_exit_country"])
    exit_region = normalize_region(row["last_exit_region"])
    exit_ip = str(row["last_exit_ip"] or "").strip()
    return (
        country is None
        or region is None
        or exit_country is None
        or exit_region is None
        or not is_public_ip(exit_ip)
    )


def resolve_public_ip(
    host: str,
    *,
    resolve_hostnames: bool,
    timeout_seconds: int,
    cache: dict[str, str | None],
) -> tuple[str | None, str | None]:
    if host in cache:
        cached = cache[host]
        if cached:
            return cached, "cache"
        return None, "cache_miss"
    if is_public_ip(host):
        cache[host] = host
        return host, "literal"
    if not resolve_hostnames:
        cache[host] = None
        return None, "hostname_resolution_disabled"
    try:
        previous_timeout = socket.getdefaulttimeout()
        socket.setdefaulttimeout(timeout_seconds)
        try:
            infos = socket.getaddrinfo(host, None, proto=socket.IPPROTO_TCP)
        finally:
            socket.setdefaulttimeout(previous_timeout)
    except OSError:
        cache[host] = None
        return None, "dns_resolution_failed"
    for info in infos:
        ip_text = str(info[4][0]).strip()
        if is_public_ip(ip_text):
            cache[host] = ip_text
            return ip_text, "dns_resolved"
    cache[host] = None
    return None, "non_public_dns_result"


def http_json(url: str, timeout_seconds: int) -> dict[str, object]:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/json",
            "User-Agent": "PersonaPilot/prod-proxy-geo-enrich",
        },
    )
    with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
        return json.loads(response.read().decode("utf-8"))


def lookup_geo_ip_api(ip_text: str, timeout_seconds: int) -> dict[str, object] | None:
    payload = http_json(
        "http://ip-api.com/json/"
        + urllib.parse.quote(ip_text, safe="")
        + "?fields=status,message,countryCode,region,regionName,query",
        timeout_seconds,
    )
    if str(payload.get("status") or "").strip().lower() != "success":
        return None
    return {
        "provider": "ip-api",
        "ip": str(payload.get("query") or ip_text).strip(),
        "country_code": normalize_country(payload.get("countryCode")),
        "region_code": str(payload.get("region") or "").strip() or None,
        "region_name": str(payload.get("regionName") or "").strip() or None,
    }


def lookup_geo_ipapi(ip_text: str, timeout_seconds: int) -> dict[str, object] | None:
    payload = http_json(
        "https://ipapi.co/" + urllib.parse.quote(ip_text, safe="") + "/json/",
        timeout_seconds,
    )
    if str(payload.get("error") or "").strip():
        return None
    country_code = normalize_country(payload.get("country_code"))
    if not country_code:
        return None
    return {
        "provider": "ipapi",
        "ip": str(payload.get("ip") or ip_text).strip(),
        "country_code": country_code,
        "region_code": str(payload.get("region_code") or "").strip() or None,
        "region_name": str(payload.get("region") or "").strip() or None,
    }


def lookup_geo_ipinfo(ip_text: str, timeout_seconds: int) -> dict[str, object] | None:
    payload = http_json(
        "https://ipinfo.io/" + urllib.parse.quote(ip_text, safe="") + "/json",
        timeout_seconds,
    )
    country_code = normalize_country(payload.get("country"))
    if not country_code:
        return None
    return {
        "provider": "ipinfo",
        "ip": str(payload.get("ip") or ip_text).strip(),
        "country_code": country_code,
        "region_code": None,
        "region_name": str(payload.get("region") or "").strip() or None,
    }


def lookup_geo_ip(ip_text: str, *, timeout_seconds: int) -> tuple[dict[str, object] | None, str | None]:
    lookup_errors: list[str] = []
    for provider in DEFAULT_GEO_PROVIDER_ORDER:
        try:
            if provider == "ip-api":
                result = lookup_geo_ip_api(ip_text, timeout_seconds)
            elif provider == "ipapi":
                result = lookup_geo_ipapi(ip_text, timeout_seconds)
            else:
                result = lookup_geo_ipinfo(ip_text, timeout_seconds)
        except (urllib.error.URLError, TimeoutError, json.JSONDecodeError) as exc:
            lookup_errors.append(f"{provider}:{exc}")
            continue
        if result:
            return result, None
        lookup_errors.append(f"{provider}:empty")
    return None, "; ".join(lookup_errors) if lookup_errors else None


def build_distribution_summary(
    conn: sqlite3.Connection,
    *,
    mode: str,
    source_labels: list[str],
) -> dict[str, object]:
    params: list[object] = []
    if source_labels:
        placeholders = ", ".join("?" for _ in source_labels)
        source_filter = f"p.source_label IN ({placeholders})"
        params.extend(source_labels)
    else:
        mode_column = "for_prod" if mode == MODE_PROD_LIVE else "for_demo"
        source_filter = f"COALESCE(s.enabled, 0) = 1 AND COALESCE(s.{mode_column}, 0) = 1"
    status_rows = conn.execute(
        f"""
        SELECT status, COUNT(*) AS total
        FROM proxies AS p
        LEFT JOIN proxy_harvest_sources AS s ON s.source_label = p.source_label
        WHERE {source_filter}
        GROUP BY status
        ORDER BY total DESC, status ASC
        """,
        params,
    ).fetchall()
    active_rows = conn.execute(
        f"""
        SELECT
            region,
            country,
            last_exit_region,
            last_exit_country
        FROM proxies AS p
        LEFT JOIN proxy_harvest_sources AS s ON s.source_label = p.source_label
        WHERE {source_filter}
          AND p.status = 'active'
        """,
        params,
    ).fetchall()

    def count_values(rows: list[sqlite3.Row], key: str, *, region: bool = False) -> dict[str, int]:
        counts: dict[str, int] = {}
        for row in rows:
            raw_value = normalize_region(row[key]) if region else normalize_country(row[key])
            if not raw_value:
                continue
            counts[raw_value] = counts.get(raw_value, 0) + 1
        return dict(sorted(counts.items(), key=lambda item: (-item[1], item[0])))

    return {
        "status_counts": {
            str(row["status"]): to_int(row["total"])
            for row in status_rows
        },
        "active_region_counts": count_values(active_rows, "region", region=True),
        "active_country_counts": count_values(active_rows, "country"),
        "active_exit_region_counts": count_values(active_rows, "last_exit_region", region=True),
        "active_exit_country_counts": count_values(active_rows, "last_exit_country"),
    }


def load_target_proxies(
    conn: sqlite3.Connection,
    *,
    mode: str,
    source_labels: list[str],
    statuses: list[str],
    limit: int,
    force_refresh: bool,
) -> list[sqlite3.Row]:
    params: list[object] = []
    status_placeholders = ", ".join("?" for _ in statuses)
    params.extend(statuses)
    if source_labels:
        source_placeholders = ", ".join("?" for _ in source_labels)
        source_filter = f"p.source_label IN ({source_placeholders})"
        params.extend(source_labels)
    else:
        mode_column = "for_prod" if mode == MODE_PROD_LIVE else "for_demo"
        source_filter = f"COALESCE(s.enabled, 0) = 1 AND COALESCE(s.{mode_column}, 0) = 1"
    rows = conn.execute(
        f"""
        SELECT
            p.id,
            p.host,
            p.port,
            p.status,
            p.score,
            p.source_label,
            p.region,
            p.country,
            p.last_exit_ip,
            p.last_exit_country,
            p.last_exit_region,
            p.last_verify_source,
            p.updated_at,
            COALESCE(s.expected_geo_quality, '') AS expected_geo_quality
        FROM proxies AS p
        LEFT JOIN proxy_harvest_sources AS s ON s.source_label = p.source_label
        WHERE p.status IN ({status_placeholders})
          AND {source_filter}
        ORDER BY
            CASE p.status
                WHEN 'active' THEN 0
                WHEN 'candidate' THEN 1
                ELSE 2
            END ASC,
            COALESCE(p.cached_trust_score, -999999) DESC,
            p.score DESC,
            CAST(COALESCE(p.promoted_at, p.last_seen_at, p.updated_at, p.created_at, '0') AS INTEGER) DESC,
            p.id ASC
        """,
        params,
    ).fetchall()
    selected: list[sqlite3.Row] = []
    for row in rows:
        if not needs_geo_refresh(row, force_refresh=force_refresh):
            continue
        selected.append(row)
        if len(selected) >= max(limit, 0):
            break
    return selected


def update_proxy_geo(
    conn: sqlite3.Connection,
    *,
    proxy_id: str,
    country_code: str,
    region_value: str,
    exit_ip: str,
    now_ts: int,
    last_verify_source: object,
) -> None:
    conn.execute(
        """
        UPDATE proxies
        SET country = ?,
            region = ?,
            last_exit_ip = ?,
            last_exit_country = ?,
            last_exit_region = ?,
            last_verify_source = ?,
            updated_at = ?
        WHERE id = ?
        """,
        (
            country_code,
            region_value,
            exit_ip,
            country_code,
            region_value,
            append_verify_source(last_verify_source),
            str(now_ts),
            proxy_id,
        ),
    )


def maybe_update_source_geo_quality(
    conn: sqlite3.Connection,
    *,
    source_label: str,
    now_ts: int,
) -> bool:
    row = conn.execute(
        """
        SELECT expected_geo_quality
        FROM proxy_harvest_sources
        WHERE source_label = ?
        """,
        (source_label,),
    ).fetchone()
    if not row:
        return False
    current = str(row["expected_geo_quality"] or "").strip().lower()
    if current not in {"", "unknown"}:
        return False
    conn.execute(
        """
        UPDATE proxy_harvest_sources
        SET expected_geo_quality = 'host_geo_inferred',
            updated_at = ?
        WHERE source_label = ?
        """,
        (str(now_ts), source_label),
    )
    return True


def main() -> int:
    args = parse_args()
    mode = normalize_mode(args.mode)
    db_path = Path(args.db).expanduser().resolve()
    if not db_path.exists():
        raise SystemExit(f"database not found: {db_path}")

    statuses = normalize_statuses(args.only_status)
    source_map = load_config_source_map(args.config) if args.config.strip() else {}
    selected_labels = list(dict.fromkeys(item for item in args.source_label if item.strip()))
    if not selected_labels and source_map:
        selected_labels = selected_labels_from_config(source_map, mode)

    conn = open_db(db_path)
    now_ts = int(time.time())
    dns_cache: dict[str, str | None] = {}
    lookup_cache: dict[str, tuple[dict[str, object] | None, str | None]] = {}

    try:
        before_summary = build_distribution_summary(conn, mode=mode, source_labels=selected_labels)
        target_rows = load_target_proxies(
            conn,
            mode=mode,
            source_labels=selected_labels,
            statuses=statuses,
            limit=args.limit,
            force_refresh=args.force_refresh,
        )

        source_summaries: dict[str, dict[str, object]] = {}
        lookup_succeeded = 0
        lookup_failed = 0
        updated_proxy_rows = 0
        updated_source_rows = 0
        skipped_private_host = 0
        skipped_unresolvable_host = 0
        dns_resolved_hosts = 0
        sample_updates: list[dict[str, object]] = []

        if args.apply:
            conn.execute("BEGIN")

        for row in target_rows:
            source_label = str(row["source_label"] or "<none>")
            summary = source_summaries.setdefault(
                source_label,
                {
                    "source_label": source_label,
                    "proxy_count": 0,
                    "updated_count": 0,
                    "lookup_failed_count": 0,
                    "skipped_private_host_count": 0,
                    "skipped_unresolvable_host_count": 0,
                    "dns_resolved_count": 0,
                    "region_counts": {},
                    "country_counts": {},
                    "expected_geo_quality_before": str(row["expected_geo_quality"] or "").strip() or None,
                    "expected_geo_quality_after": str(row["expected_geo_quality"] or "").strip() or None,
                },
            )
            summary["proxy_count"] = to_int(summary["proxy_count"]) + 1

            host = str(row["host"] or "").strip()
            resolved_ip, resolution_status = resolve_public_ip(
                host,
                resolve_hostnames=not args.disable_hostname_resolution,
                timeout_seconds=max(args.timeout_seconds, 1),
                cache=dns_cache,
            )
            if resolution_status == "dns_resolved":
                dns_resolved_hosts += 1
                summary["dns_resolved_count"] = to_int(summary["dns_resolved_count"]) + 1
            if not resolved_ip:
                if resolution_status in {"literal", "cache"}:
                    skipped_private_host += 1
                    summary["skipped_private_host_count"] = to_int(summary["skipped_private_host_count"]) + 1
                elif resolution_status == "non_public_dns_result":
                    skipped_private_host += 1
                    summary["skipped_private_host_count"] = to_int(summary["skipped_private_host_count"]) + 1
                else:
                    skipped_unresolvable_host += 1
                    summary["skipped_unresolvable_host_count"] = (
                        to_int(summary["skipped_unresolvable_host_count"]) + 1
                    )
                continue

            if resolved_ip not in lookup_cache:
                lookup_cache[resolved_ip] = lookup_geo_ip(
                    resolved_ip,
                    timeout_seconds=max(args.timeout_seconds, 1),
                )
                if args.sleep_ms > 0:
                    time.sleep(max(args.sleep_ms, 0) / 1000.0)
            geo_payload, geo_error = lookup_cache[resolved_ip]
            if not geo_payload:
                lookup_failed += 1
                summary["lookup_failed_count"] = to_int(summary["lookup_failed_count"]) + 1
                continue

            country_code = normalize_country(geo_payload.get("country_code"))
            region_value = canonical_region(
                country_code,
                str(geo_payload.get("region_name") or "").strip() or None,
                str(geo_payload.get("region_code") or "").strip() or None,
            )
            if not country_code or not region_value:
                lookup_failed += 1
                summary["lookup_failed_count"] = to_int(summary["lookup_failed_count"]) + 1
                continue

            lookup_succeeded += 1
            if args.apply:
                update_proxy_geo(
                    conn,
                    proxy_id=str(row["id"]),
                    country_code=country_code,
                    region_value=region_value,
                    exit_ip=str(geo_payload.get("ip") or resolved_ip),
                    now_ts=now_ts,
                    last_verify_source=row["last_verify_source"],
                )
            updated_proxy_rows += 1
            summary["updated_count"] = to_int(summary["updated_count"]) + 1
            region_counts = dict(summary["region_counts"])
            country_counts = dict(summary["country_counts"])
            region_counts[region_value] = to_int(region_counts.get(region_value)) + 1
            country_counts[country_code] = to_int(country_counts.get(country_code)) + 1
            summary["region_counts"] = dict(
                sorted(region_counts.items(), key=lambda item: (-item[1], item[0]))
            )
            summary["country_counts"] = dict(
                sorted(country_counts.items(), key=lambda item: (-item[1], item[0]))
            )
            if summary["updated_count"] == 1 and source_label != "<none>":
                changed = False
                if args.apply:
                    changed = maybe_update_source_geo_quality(
                        conn,
                        source_label=source_label,
                        now_ts=now_ts,
                    )
                if changed:
                    updated_source_rows += 1
                    summary["expected_geo_quality_after"] = "host_geo_inferred"
                elif str(summary.get("expected_geo_quality_before") or "").strip().lower() in {"", "unknown"}:
                    summary["expected_geo_quality_after"] = "host_geo_inferred"

            if len(sample_updates) < 20:
                sample_updates.append(
                    {
                        "proxy_id": str(row["id"]),
                        "host": host,
                        "resolved_ip": resolved_ip,
                        "source_label": None if source_label == "<none>" else source_label,
                        "country": country_code,
                        "region": region_value,
                        "provider": geo_payload.get("provider"),
                        "lookup_error": geo_error,
                    }
                )

        if args.apply:
            conn.commit()
        after_summary = build_distribution_summary(conn, mode=mode, source_labels=selected_labels)
        payload = {
            "generated_at": now_ts,
            "db_path": db_path.as_posix(),
            "mode": mode,
            "dry_run": not args.apply,
            "config_path": resolve_path(args.config).as_posix() if args.config.strip() else None,
            "selected_source_labels": selected_labels,
            "only_statuses": statuses,
            "limit": max(args.limit, 0),
            "before": before_summary,
            "after": after_summary,
            "apply_result": {
                "updated_proxy_rows": updated_proxy_rows,
                "updated_source_rows": updated_source_rows,
                "lookup_succeeded": lookup_succeeded,
                "lookup_failed": lookup_failed,
                "skipped_private_host": skipped_private_host,
                "skipped_unresolvable_host": skipped_unresolvable_host,
                "dns_resolved_hosts": dns_resolved_hosts,
            },
            "source_summaries": [
                {
                    **summary,
                    "region_counts": dict(
                        sorted(
                            dict(summary.get("region_counts") or {}).items(),
                            key=lambda item: (-to_int(item[1]), item[0]),
                        )
                    ),
                    "country_counts": dict(
                        sorted(
                            dict(summary.get("country_counts") or {}).items(),
                            key=lambda item: (-to_int(item[1]), item[0]),
                        )
                    ),
                }
                for _, summary in sorted(
                    source_summaries.items(),
                    key=lambda item: (-to_int(item[1].get("updated_count")), item[0]),
                )
            ],
            "sample_updates": sample_updates,
        }
        text = json.dumps(payload, ensure_ascii=False, indent=2)
        print(text)
        if args.summary_json:
            output_path = Path(args.summary_json).expanduser().resolve()
            output_path.parent.mkdir(parents=True, exist_ok=True)
            output_path.write_text(text + "\n", encoding="utf-8")
    except Exception:
        if args.apply:
            conn.rollback()
        raise
    finally:
        conn.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
