#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shlex
import sqlite3
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from prod_proxy_pool_hygiene import (
    load_config_source_map,
    selected_labels_from_config,
)
from prod_live_presets import DEFAULT_PRESET_NAME, resolve_preset


ROOT_DIR = SCRIPT_DIR.parent
DEFAULT_BASE_URL = "http://127.0.0.1:3000"
TERMINAL_TASK_STATUSES = {"succeeded", "failed", "timed_out", "cancelled"}
MODE_DEMO_PUBLIC = "demo_public"
MODE_PROD_LIVE = "prod_live"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Drive real-source harvest + browser warm traffic + status sampling for long-run proxy validation.",
    )
    parser.add_argument(
        "--base-url",
        default=os.environ.get("PERSONA_PILOT_BASE_URL", DEFAULT_BASE_URL),
        help="Control-plane base URL",
    )
    parser.add_argument(
        "--api-key",
        default=os.environ.get("PERSONA_PILOT_API_KEY", ""),
        help="Optional API key for protected control-plane endpoints",
    )
    parser.add_argument(
        "--db",
        default=(ROOT_DIR / "data" / "persona_pilot.db").as_posix(),
        help="SQLite database path used by proxy_harvest_mvp.py",
    )
    parser.add_argument(
        "--config",
        default=os.environ.get("PERSONA_PILOT_PROXY_HARVEST_CONFIG", ""),
        help="Real proxy source config path; defaults to PERSONA_PILOT_PROXY_HARVEST_CONFIG",
    )
    parser.add_argument(
        "--mode",
        default=os.environ.get("PERSONA_PILOT_PROXY_MODE", MODE_DEMO_PUBLIC),
        help="Expected proxy runtime mode: demo_public or prod_live",
    )
    parser.add_argument(
        "--duration-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_DURATION_SECONDS", "1800")),
        help="Total driver runtime window in seconds",
    )
    parser.add_argument(
        "--harvest-interval-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_HARVEST_INTERVAL_SECONDS", "120")),
        help="Delay between forced harvest runs",
    )
    parser.add_argument(
        "--status-interval-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_STATUS_INTERVAL_SECONDS", "60")),
        help="Delay between /status snapshots",
    )
    parser.add_argument(
        "--browser-interval-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_BROWSER_INTERVAL_SECONDS", "45")),
        help="Delay between browser warm tasks",
    )
    parser.add_argument(
        "--pool-hygiene-interval-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_HYGIENE_INTERVAL_SECONDS", "90")),
        help="Delay between prod_live proxy pool hygiene runs",
    )
    parser.add_argument(
        "--pool-hygiene-script",
        default=os.environ.get(
            "PROXY_REAL_LONGRUN_HYGIENE_SCRIPT",
            (ROOT_DIR / "scripts" / "prod_proxy_pool_hygiene.py").as_posix(),
        ),
        help="Path to the prod proxy pool hygiene script",
    )
    pool_hygiene_group = parser.add_mutually_exclusive_group()
    pool_hygiene_group.add_argument(
        "--enable-pool-hygiene",
        dest="pool_hygiene_enabled",
        action="store_true",
        default=None,
        help="Force-enable periodic prod_live proxy pool hygiene during the long-run driver",
    )
    pool_hygiene_group.add_argument(
        "--disable-pool-hygiene",
        dest="pool_hygiene_enabled",
        action="store_false",
        help="Disable periodic prod_live proxy pool hygiene during the long-run driver",
    )
    parser.add_argument(
        "--pool-hygiene-extra-args",
        default=None,
        help="Explicit pool hygiene args string; overrides env/preset defaults",
    )
    parser.add_argument(
        "--geo-enrich-interval-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_GEO_ENRICH_INTERVAL_SECONDS", "120")),
        help="Delay between prod_live geo enrichment runs",
    )
    parser.add_argument(
        "--geo-enrich-script",
        default=os.environ.get(
            "PROXY_REAL_LONGRUN_GEO_ENRICH_SCRIPT",
            (ROOT_DIR / "scripts" / "prod_proxy_geo_enrich.py").as_posix(),
        ),
        help="Path to the prod proxy geo enrichment script",
    )
    parser.add_argument(
        "--geo-enrich-limit",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_GEO_ENRICH_LIMIT", "200")),
        help="Maximum proxy rows to enrich per geo enrichment window",
    )
    geo_enrich_group = parser.add_mutually_exclusive_group()
    geo_enrich_group.add_argument(
        "--enable-geo-enrich",
        dest="geo_enrich_enabled",
        action="store_true",
        default=None,
        help="Force-enable periodic prod_live geo enrichment during the long-run driver",
    )
    geo_enrich_group.add_argument(
        "--disable-geo-enrich",
        dest="geo_enrich_enabled",
        action="store_false",
        help="Disable periodic prod_live geo enrichment during the long-run driver",
    )
    parser.add_argument(
        "--browser-timeout-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_BROWSER_TIMEOUT_SECONDS", "15")),
        help="Browser task timeout_seconds value",
    )
    parser.add_argument(
        "--task-poll-interval-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_TASK_POLL_INTERVAL_SECONDS", "1")),
        help="Polling interval for terminal browser task status",
    )
    parser.add_argument(
        "--task-poll-max-seconds",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_TASK_POLL_MAX_SECONDS", "45")),
        help="Max seconds to wait for terminal browser task status",
    )
    parser.add_argument(
        "--browser-endpoint",
        default=os.environ.get("PROXY_REAL_LONGRUN_BROWSER_ENDPOINT", "/browser/title"),
        help="Browser endpoint used to generate warm traffic",
    )
    parser.add_argument(
        "--stateful-url",
        default=os.environ.get("PROXY_REAL_LONGRUN_STATEFUL_URL", ""),
        help="Optional stateful continuity URL used to verify cookie/localStorage/sessionStorage reuse",
    )
    parser.add_argument(
        "--stateful-endpoint",
        default=os.environ.get("PROXY_REAL_LONGRUN_STATEFUL_ENDPOINT", "/browser/text"),
        help="Browser endpoint used for the stateful continuity workload",
    )
    parser.add_argument(
        "--stateful-followup-count",
        type=int,
        default=None,
        help="Immediate same-host follow-up requests after the primary stateful probe",
    )
    parser.add_argument(
        "--preset",
        default=os.environ.get(
            "PROXY_VERIFY_REAL_PRESET",
            os.environ.get("PROXY_REAL_LONGRUN_PRESET", DEFAULT_PRESET_NAME),
        ),
        help="prod_live preset name: legacy or stable_v1",
    )
    parser.add_argument(
        "--browser-region",
        action="append",
        default=[],
        help="Optional region hint injected into network_policy_json; may be repeated",
    )
    auto_browser_region_group = parser.add_mutually_exclusive_group()
    auto_browser_region_group.add_argument(
        "--auto-browser-regions-from-db",
        dest="auto_browser_regions_from_db",
        action="store_true",
        default=None,
        help="Load active proxy regions from SQLite and round-robin them into browser demand",
    )
    auto_browser_region_group.add_argument(
        "--disable-auto-browser-regions-from-db",
        dest="auto_browser_regions_from_db",
        action="store_false",
        help="Disable preset/default active-region round-robin from SQLite",
    )
    parser.add_argument(
        "--max-browser-regions",
        type=int,
        default=int(os.environ.get("PROXY_REAL_LONGRUN_MAX_BROWSER_REGIONS", "3")),
        help="Maximum active regions to round-robin when auto-browser-regions-from-db is enabled",
    )
    parser.add_argument(
        "--fingerprint-profile-id",
        default=os.environ.get("PROXY_REAL_LONGRUN_FINGERPRINT_PROFILE_ID", ""),
        help="Optional fingerprint profile used for warm browser tasks",
    )
    parser.add_argument(
        "--warm-url",
        action="append",
        default=[],
        help="Warm URL used to drive browser tasks; may be repeated",
    )
    parser.add_argument(
        "--allow-demo-config",
        action="store_true",
        help="Allow repo-owned demo/seed config paths instead of requiring a real external config",
    )
    parser.add_argument(
        "--raw-output",
        default="reports/proxy_real_longrun_driver_latest.json",
        help="Raw driver payload output path",
    )
    parser.add_argument(
        "--txt-output",
        default="reports/proxy_real_longrun_latest.txt",
        help="Text report output path",
    )
    parser.add_argument(
        "--json-output",
        default="reports/proxy_real_longrun_latest.json",
        help="JSON report output path",
    )
    return parser.parse_args()


def normalize_mode(raw_mode: str) -> str:
    value = raw_mode.strip().lower().replace("-", "_")
    if value == MODE_PROD_LIVE:
        return MODE_PROD_LIVE
    return MODE_DEMO_PUBLIC


def resolve_path(raw_path: str) -> Path:
    expanded = Path(os.path.expandvars(os.path.expanduser(raw_path.strip())))
    if expanded.is_absolute():
        return expanded.resolve()
    return (ROOT_DIR / expanded).resolve()


def normalize_warm_urls(raw_urls: list[str]) -> list[str]:
    if raw_urls:
        return [item.strip() for item in raw_urls if item.strip()]
    env_value = os.environ.get("PROXY_REAL_LONGRUN_WARM_URLS", "").strip()
    if env_value:
        return [item.strip() for item in env_value.split(",") if item.strip()]
    return ["https://example.com"]


def normalize_browser_regions(raw_regions: list[str]) -> list[str]:
    values: list[str] = []
    for raw in raw_regions:
        for part in str(raw).split(","):
            region = part.strip()
            if region:
                values.append(region)
    if not values:
        env_value = os.environ.get("PROXY_REAL_LONGRUN_BROWSER_REGION", "").strip()
        if env_value:
            values.extend(part.strip() for part in env_value.split(",") if part.strip())
    deduped: list[str] = []
    seen: set[str] = set()
    for value in values:
        lowered = value.lower()
        if lowered in seen:
            continue
        seen.add(lowered)
        deduped.append(value)
    return deduped


def first_non_empty_env(*names: str) -> str | None:
    for name in names:
        value = os.environ.get(name)
        if value is not None and str(value).strip():
            return str(value).strip()
    return None


def parse_optional_bool(raw_value: str | None) -> bool | None:
    if raw_value is None:
        return None
    value = str(raw_value).strip().lower()
    if not value:
        return None
    if value in {"1", "true", "yes", "on", "enabled"}:
        return True
    if value in {"0", "false", "no", "off", "disabled"}:
        return False
    raise SystemExit(f"invalid boolean override: {raw_value!r}")


def env_bool(*names: str) -> bool | None:
    for name in names:
        value = parse_optional_bool(os.environ.get(name))
        if value is not None:
            return value
    return None


def env_int(*names: str) -> int | None:
    for name in names:
        raw_value = os.environ.get(name)
        if raw_value is None or not str(raw_value).strip():
            continue
        try:
            return int(str(raw_value).strip())
        except ValueError as exc:
            raise SystemExit(f"invalid integer override for {name}: {raw_value!r}") from exc
    return None


def validate_config_path(config_path: str, allow_demo: bool, mode: str) -> Path:
    if not config_path.strip():
        raise SystemExit(
            "missing real config: set --config or PERSONA_PILOT_PROXY_HARVEST_CONFIG",
        )
    resolved = resolve_path(config_path)
    if not resolved.exists():
        raise SystemExit(f"real config path does not exist: {resolved}")
    repo_seed_paths = {
        (ROOT_DIR / "data" / "proxy_sources.json").resolve(),
        (ROOT_DIR / "data" / "proxy_sources.demo.json").resolve(),
    }
    if mode == MODE_PROD_LIVE and resolved in repo_seed_paths:
        raise SystemExit(
            f"prod_live mode forbids repo seed/demo config: {resolved}",
        )
    if not allow_demo and resolved in repo_seed_paths:
        raise SystemExit(
            "refusing repo seed/demo config for real-longrun driver; pass --allow-demo-config to override",
        )
    return resolved


def build_headers(api_key: str, *, json_body: bool = False) -> dict[str, str]:
    headers = {"Accept": "application/json"}
    if json_body:
        headers["Content-Type"] = "application/json"
    if api_key:
        headers["x-api-key"] = api_key
    return headers


def http_json(method: str, url: str, api_key: str, body: dict[str, object] | None = None) -> dict[str, object]:
    data = None
    if body is not None:
        data = json.dumps(body, ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers=build_headers(api_key, json_body=body is not None),
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:  # noqa: PERF203
        payload = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"http {exc.code} for {url}: {payload}") from exc


def fetch_status(base_url: str, api_key: str) -> dict[str, object]:
    return http_json("GET", f"{base_url.rstrip('/')}/status", api_key)


def poll_task_terminal(
    base_url: str,
    api_key: str,
    task_id: str,
    *,
    poll_interval_seconds: int,
    poll_max_seconds: int,
) -> dict[str, object]:
    attempts = max(poll_max_seconds, 1) // max(poll_interval_seconds, 1)
    attempts = max(attempts, 1)
    for _ in range(attempts):
        detail = http_json("GET", f"{base_url.rstrip('/')}/tasks/{task_id}", api_key)
        status = str(detail.get("status") or "")
        if status in TERMINAL_TASK_STATUSES:
            return detail
        time.sleep(max(poll_interval_seconds, 1))
    raise TimeoutError(f"task {task_id} did not reach terminal status in time")


def run_harvest_once(config_path: Path, db_path: str) -> dict[str, object]:
    started_at = int(time.time())
    cmd = [
        sys.executable,
        str(ROOT_DIR / "scripts" / "proxy_harvest_mvp.py"),
        "--db",
        db_path,
        "--config",
        config_path.as_posix(),
        "--once",
    ]
    completed = subprocess.run(
        cmd,
        cwd=ROOT_DIR,
        check=False,
        capture_output=True,
        text=True,
    )
    payload: dict[str, object]
    try:
        payload = json.loads(completed.stdout) if completed.stdout.strip() else {}
    except json.JSONDecodeError:
        payload = {
            "status": "failed",
            "results": [],
            "decode_error": completed.stdout.strip(),
        }
    results = list(payload.get("results") or [])
    return {
        "type": "harvest",
        "timestamp": started_at,
        "status": str(payload.get("status") or ("completed" if completed.returncode == 0 else "failed")),
        "source_count": int(payload.get("source_count") or len(results)),
        "accepted_count": sum(int(item.get("accepted_count") or 0) for item in results if isinstance(item, dict)),
        "deduped_count": sum(int(item.get("deduped_count") or 0) for item in results if isinstance(item, dict)),
        "rejected_count": sum(int(item.get("rejected_count") or 0) for item in results if isinstance(item, dict)),
        "returncode": completed.returncode,
        "stderr": completed.stderr.strip(),
    }


def run_pool_hygiene_once(
    *,
    mode: str,
    config_path: Path,
    db_path: str,
    hygiene_script: Path,
    hygiene_extra_args: list[str],
) -> dict[str, object]:
    started_at = int(time.time())
    cmd = [
        sys.executable,
        hygiene_script.as_posix(),
        "--db",
        db_path,
        "--mode",
        mode,
        "--config",
        config_path.as_posix(),
        "--apply",
    ]
    if hygiene_extra_args:
        cmd.extend(hygiene_extra_args)
    completed = subprocess.run(
        cmd,
        cwd=ROOT_DIR,
        check=False,
        capture_output=True,
        text=True,
    )
    stdout = completed.stdout.strip()
    stderr = completed.stderr.strip()
    payload: dict[str, object] = {}
    if stdout:
        try:
            payload = json.loads(stdout)
        except json.JSONDecodeError:
            payload = {}
    before = dict(payload.get("before") or {})
    after = dict(payload.get("after") or {})
    apply_result = dict(payload.get("apply_result") or {})
    if completed.returncode != 0:
        return {
            "type": "hygiene",
            "timestamp": started_at,
            "status": "failed",
            "deleted_proxy_rows": 0,
            "quarantined_sources": 0,
            "synced_source_metadata_rows": 0,
            "effective_active_ratio_percent_before": 0.0,
            "effective_active_ratio_percent_after": 0.0,
            "error": stderr or stdout or f"pool hygiene exited with code {completed.returncode}",
        }
    return {
        "type": "hygiene",
        "timestamp": started_at,
        "status": "completed",
        "deleted_proxy_rows": int(apply_result.get("deleted_proxy_rows") or 0),
        "quarantined_sources": int(apply_result.get("quarantined_sources") or 0),
        "synced_source_metadata_rows": int(apply_result.get("synced_source_metadata_rows") or 0),
        "effective_active_ratio_percent_before": float(
            before.get("estimated_effective_active_ratio_percent") or 0.0
        ),
        "effective_active_ratio_percent_after": float(
            after.get("estimated_effective_active_ratio_percent") or 0.0
        ),
    }


def run_geo_enrich_once(
    *,
    mode: str,
    config_path: Path,
    db_path: str,
    geo_enrich_script: Path,
    geo_enrich_limit: int,
) -> dict[str, object]:
    started_at = int(time.time())
    cmd = [
        sys.executable,
        geo_enrich_script.as_posix(),
        "--db",
        db_path,
        "--mode",
        mode,
        "--config",
        config_path.as_posix(),
        "--only-status",
        "active",
        "--limit",
        str(max(geo_enrich_limit, 0)),
        "--apply",
    ]
    completed = subprocess.run(
        cmd,
        cwd=ROOT_DIR,
        check=False,
        capture_output=True,
        text=True,
    )
    stdout = completed.stdout.strip()
    stderr = completed.stderr.strip()
    payload: dict[str, object] = {}
    if stdout:
        try:
            payload = json.loads(stdout)
        except json.JSONDecodeError:
            payload = {}
    before = dict(payload.get("before") or {})
    after = dict(payload.get("after") or {})
    apply_result = dict(payload.get("apply_result") or {})
    after_active_regions = dict(after.get("active_region_counts") or {})
    if completed.returncode != 0:
        return {
            "type": "geo_enrich",
            "timestamp": started_at,
            "status": "failed",
            "updated_proxy_rows": 0,
            "updated_source_rows": 0,
            "lookup_succeeded": 0,
            "lookup_failed": 0,
            "active_region_count_before": len(dict(before.get("active_region_counts") or {})),
            "active_region_count_after": len(after_active_regions),
            "active_regions_after": [],
            "error": stderr or stdout or f"geo enrich exited with code {completed.returncode}",
        }
    return {
        "type": "geo_enrich",
        "timestamp": started_at,
        "status": "completed",
        "updated_proxy_rows": int(apply_result.get("updated_proxy_rows") or 0),
        "updated_source_rows": int(apply_result.get("updated_source_rows") or 0),
        "lookup_succeeded": int(apply_result.get("lookup_succeeded") or 0),
        "lookup_failed": int(apply_result.get("lookup_failed") or 0),
        "active_region_count_before": len(dict(before.get("active_region_counts") or {})),
        "active_region_count_after": len(after_active_regions),
        "active_regions_after": list(after_active_regions.keys())[:5],
    }


def load_browser_regions_from_db(
    *,
    db_path: str,
    mode: str,
    selected_source_labels: list[str],
    max_regions: int,
) -> list[str]:
    if max_regions <= 0:
        return []
    conn = sqlite3.connect(Path(db_path).expanduser().resolve().as_posix(), timeout=10)
    conn.row_factory = sqlite3.Row
    try:
        params: list[object] = []
        if selected_source_labels:
            placeholders = ", ".join("?" for _ in selected_source_labels)
            source_filter = f"p.source_label IN ({placeholders})"
            params.extend(selected_source_labels)
        else:
            mode_column = "for_prod" if mode == MODE_PROD_LIVE else "for_demo"
            source_filter = f"COALESCE(s.enabled, 0) = 1 AND COALESCE(s.{mode_column}, 0) = 1"
        rows = conn.execute(
            f"""
            SELECT p.region, COUNT(*) AS total
            FROM proxies AS p
            LEFT JOIN proxy_harvest_sources AS s ON s.source_label = p.source_label
            WHERE p.status = 'active'
              AND {source_filter}
              AND p.region IS NOT NULL
              AND TRIM(p.region) != ''
              AND LOWER(TRIM(p.region)) NOT IN ('global', 'unknown')
            GROUP BY p.region
            ORDER BY total DESC, p.region ASC
            LIMIT ?
            """,
            [*params, max_regions],
        ).fetchall()
        return [str(row["region"]).strip() for row in rows if str(row["region"]).strip()]
    finally:
        conn.close()


def run_browser_probe(
    *,
    workload: str,
    mode: str,
    base_url: str,
    api_key: str,
    endpoint: str,
    url: str,
    timeout_seconds: int,
    browser_region: str,
    fingerprint_profile_id: str,
    poll_interval_seconds: int,
    poll_max_seconds: int,
) -> dict[str, object]:
    timestamp = int(time.time())
    payload: dict[str, object] = {
        "url": url,
        "timeout_seconds": timeout_seconds,
        "network_policy_json": {"mode": "required_proxy", "proxy_mode": mode},
    }
    if browser_region:
        payload["network_policy_json"]["region"] = browser_region
    if fingerprint_profile_id:
        payload["fingerprint_profile_id"] = fingerprint_profile_id

    try:
        created = http_json("POST", f"{base_url.rstrip('/')}{endpoint}", api_key, payload)
        task_id = str(created.get("id") or "")
        if not task_id:
            raise RuntimeError("browser endpoint did not return task id")
        hot_regions_during_request: list[str] = []
        recent_hot_regions_during_request: list[str] = []
        region_shortages_during_request: list[str] = []
        try:
            status_during_request = fetch_status(base_url, api_key)
            pool_during_request = dict(status_during_request.get("proxy_pool_status") or {})
            hot_regions_during_request = list(pool_during_request.get("hot_regions") or [])
            recent_hot_regions_during_request = list(pool_during_request.get("recent_hot_regions") or [])
            region_shortages_during_request = list(pool_during_request.get("region_shortages") or [])
            if not recent_hot_regions_during_request:
                if browser_region:
                    recent_hot_regions_during_request = [browser_region]
                elif hot_regions_during_request:
                    recent_hot_regions_during_request = list(hot_regions_during_request)
        except Exception:
            hot_regions_during_request = []
            recent_hot_regions_during_request = [browser_region] if browser_region else []
            region_shortages_during_request = []
        detail = poll_task_terminal(
            base_url,
            api_key,
            task_id,
            poll_interval_seconds=poll_interval_seconds,
            poll_max_seconds=poll_max_seconds,
        )
        execution_identity = dict(detail.get("execution_identity") or {})
        return {
            "type": "browser",
            "workload": workload,
            "mode": mode,
            "timestamp": timestamp,
            "endpoint": endpoint,
            "url": url,
            "requested_region": browser_region or None,
            "task_id": task_id,
            "status": str(detail.get("status") or "unknown"),
            "failure_scope": detail.get("failure_scope"),
            "proxy_id": detail.get("proxy_id"),
            "proxy_region": detail.get("proxy_region"),
            "proxy_resolution_status": detail.get("proxy_resolution_status"),
            "hot_regions_during_request": hot_regions_during_request,
            "recent_hot_regions_during_request": recent_hot_regions_during_request,
            "region_shortages_during_request": region_shortages_during_request,
            "identity_session_status": execution_identity.get("identity_session_status"),
            "cookie_restore_count": execution_identity.get("cookie_restore_count"),
            "cookie_persist_count": execution_identity.get("cookie_persist_count"),
            "local_storage_restore_count": execution_identity.get("local_storage_restore_count"),
            "local_storage_persist_count": execution_identity.get("local_storage_persist_count"),
            "session_storage_restore_count": execution_identity.get("session_storage_restore_count"),
            "session_storage_persist_count": execution_identity.get("session_storage_persist_count"),
            "browser_failure_signal": detail.get("browser_failure_signal"),
            "selection_reason_summary": detail.get("selection_reason_summary"),
            "error_message": detail.get("error_message"),
            "title": detail.get("title"),
            "final_url": detail.get("final_url"),
            "content_preview": detail.get("content_preview"),
        }
    except Exception as exc:  # noqa: BLE001
        return {
            "type": "browser",
            "workload": workload,
            "mode": mode,
            "timestamp": timestamp,
            "endpoint": endpoint,
            "url": url,
            "requested_region": browser_region or None,
            "task_id": None,
            "status": "request_failed",
            "failure_scope": "driver_request",
            "proxy_id": None,
            "proxy_region": None,
            "proxy_resolution_status": None,
            "hot_regions_during_request": [],
            "recent_hot_regions_during_request": [],
            "region_shortages_during_request": [],
            "identity_session_status": None,
            "cookie_restore_count": 0,
            "cookie_persist_count": 0,
            "local_storage_restore_count": 0,
            "local_storage_persist_count": 0,
            "session_storage_restore_count": 0,
            "session_storage_persist_count": 0,
            "browser_failure_signal": None,
            "selection_reason_summary": None,
            "error_message": str(exc),
            "error": str(exc),
        }


def write_json(path: Path, payload: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    args = parse_args()
    mode = normalize_mode(args.mode)
    preset = resolve_preset(args.preset)
    preset_name = str(preset.get("preset") or DEFAULT_PRESET_NAME)
    config_path = validate_config_path(args.config, args.allow_demo_config, mode)
    warm_urls = normalize_warm_urls(args.warm_url)
    browser_regions = normalize_browser_regions(args.browser_region)
    stateful_url = args.stateful_url.strip()
    fingerprint_profile_id = args.fingerprint_profile_id.strip()
    env_stateful_followup_count = env_int(
        "PROXY_REAL_LONGRUN_STATEFUL_FOLLOWUP_COUNT",
        "PROXY_VERIFY_REAL_STATEFUL_FOLLOWUP_COUNT",
    )
    stateful_followup_count = max(
        args.stateful_followup_count
        if args.stateful_followup_count is not None
        else (
            env_stateful_followup_count
            if env_stateful_followup_count is not None
            else int(preset.get("stateful_followup_count") or 0)
        ),
        0,
    )
    env_auto_browser_regions = env_bool(
        "PROXY_REAL_LONGRUN_AUTO_BROWSER_REGIONS_FROM_DB",
        "PROXY_VERIFY_REAL_AUTO_BROWSER_REGIONS_FROM_DB",
    )
    auto_browser_regions_from_db = (
        args.auto_browser_regions_from_db
        if args.auto_browser_regions_from_db is not None
        else (
            env_auto_browser_regions
            if env_auto_browser_regions is not None
            else bool(preset.get("auto_browser_regions_from_db"))
        )
    )
    selected_source_labels: list[str] = []
    try:
        source_map = load_config_source_map(config_path.as_posix())
        selected_source_labels = selected_labels_from_config(source_map, mode)
    except Exception:
        selected_source_labels = []

    if mode == MODE_PROD_LIVE and not fingerprint_profile_id:
        raise SystemExit(
            "prod_live mode requires --fingerprint-profile-id (or PROXY_REAL_LONGRUN_FINGERPRINT_PROFILE_ID)",
        )
    env_pool_hygiene_enabled = env_bool(
        "PROXY_REAL_LONGRUN_POOL_HYGIENE",
        "PROXY_VERIFY_REAL_POOL_HYGIENE",
    )
    pool_hygiene_enabled = mode == MODE_PROD_LIVE and (
        args.pool_hygiene_enabled
        if args.pool_hygiene_enabled is not None
        else (
            env_pool_hygiene_enabled
            if env_pool_hygiene_enabled is not None
            else bool(preset.get("pool_hygiene"))
        )
    )
    pool_hygiene_script = resolve_path(args.pool_hygiene_script)
    if pool_hygiene_enabled and not pool_hygiene_script.exists():
        raise SystemExit(f"pool hygiene script does not exist: {pool_hygiene_script}")
    hygiene_extra_args_raw = (
        args.pool_hygiene_extra_args
        if args.pool_hygiene_extra_args is not None
        else first_non_empty_env(
            "PROXY_REAL_LONGRUN_HYGIENE_EXTRA_ARGS",
            "PROXY_VERIFY_REAL_HYGIENE_EXTRA_ARGS",
        )
    )
    if hygiene_extra_args_raw is None:
        hygiene_extra_args = [str(item) for item in (preset.get("pool_hygiene_extra_args") or [])]
    else:
        hygiene_extra_args = shlex.split(hygiene_extra_args_raw)
    env_geo_enrich_enabled = env_bool(
        "PROXY_REAL_LONGRUN_GEO_ENRICH",
        "PROXY_VERIFY_REAL_GEO_ENRICH",
    )
    geo_enrich_enabled = mode == MODE_PROD_LIVE and (
        args.geo_enrich_enabled
        if args.geo_enrich_enabled is not None
        else (
            env_geo_enrich_enabled
            if env_geo_enrich_enabled is not None
            else bool(preset.get("geo_enrich"))
        )
    )
    geo_enrich_script = resolve_path(args.geo_enrich_script)
    if geo_enrich_enabled and not geo_enrich_script.exists():
        raise SystemExit(f"geo enrich script does not exist: {geo_enrich_script}")

    deadline = time.time() + max(args.duration_seconds, 1)
    next_harvest_at = 0.0
    next_status_at = 0.0
    next_browser_at = 0.0
    next_hygiene_at = 0.0 if pool_hygiene_enabled else float("inf")
    next_geo_enrich_at = 0.0 if geo_enrich_enabled else float("inf")
    url_index = 0
    sticky_stateful_region = ""

    events: list[dict[str, object]] = []
    errors: list[str] = []
    status_snapshots: list[dict[str, object]] = []

    print(
        f"[proxy-real-longrun] mode={mode} preset={preset_name} base_url={args.base_url} config={config_path} "
        f"duration_seconds={max(args.duration_seconds, 1)} warm_urls={len(warm_urls)} "
        f"endpoint={args.browser_endpoint} "
        f"stateful_url={stateful_url or 'disabled'} "
        f"stateful_followups={stateful_followup_count} "
        f"fingerprint_profile={fingerprint_profile_id or 'none'} "
        f"browser_regions={','.join(browser_regions) or ('auto-db' if auto_browser_regions_from_db else 'none')} "
        f"pool_hygiene={'enabled' if pool_hygiene_enabled else 'disabled'} "
        f"hygiene_extra_args={' '.join(hygiene_extra_args) if hygiene_extra_args else 'none'} "
        f"geo_enrich={'enabled' if geo_enrich_enabled else 'disabled'}",
    )
    if not fingerprint_profile_id:
        print(
            "[proxy-real-longrun] warning: fingerprint_profile_id is empty; "
            "browser tasks will not exercise auto identity-session continuity",
            file=sys.stderr,
        )
    if not stateful_url:
        print(
            "[proxy-real-longrun] warning: stateful_url is empty; "
            "real-live run will not prove cookie/localStorage/sessionStorage continuity",
            file=sys.stderr,
        )

    while time.time() < deadline:
        now = time.time()
        did_work = False

        if now >= next_harvest_at:
            event = run_harvest_once(config_path, args.db)
            events.append(event)
            print(
                f"[proxy-real-longrun] harvest status={event['status']} "
                f"accepted={event['accepted_count']} deduped={event['deduped_count']} rejected={event['rejected_count']}",
            )
            next_harvest_at = now + max(args.harvest_interval_seconds, 1)
            did_work = True

        if geo_enrich_enabled and now >= next_geo_enrich_at:
            event = run_geo_enrich_once(
                mode=mode,
                config_path=config_path,
                db_path=args.db,
                geo_enrich_script=geo_enrich_script,
                geo_enrich_limit=max(args.geo_enrich_limit, 0),
            )
            events.append(event)
            print(
                f"[proxy-real-longrun] geo_enrich status={event['status']} "
                f"updated_proxies={event.get('updated_proxy_rows') or 0} "
                f"updated_sources={event.get('updated_source_rows') or 0} "
                f"lookup={event.get('lookup_succeeded') or 0}/{event.get('lookup_failed') or 0} "
                f"regions={event.get('active_region_count_before') or 0}"
                f"->{event.get('active_region_count_after') or 0}",
            )
            next_geo_enrich_at = now + max(args.geo_enrich_interval_seconds, 1)
            did_work = True

        if pool_hygiene_enabled and now >= next_hygiene_at:
            event = run_pool_hygiene_once(
                mode=mode,
                config_path=config_path,
                db_path=args.db,
                hygiene_script=pool_hygiene_script,
                hygiene_extra_args=hygiene_extra_args,
            )
            events.append(event)
            print(
                f"[proxy-real-longrun] hygiene status={event['status']} "
                f"deleted={event.get('deleted_proxy_rows') or 0} "
                f"quarantined={event.get('quarantined_sources') or 0} "
                f"ratio={event.get('effective_active_ratio_percent_before') or 0.0}"
                f"->{event.get('effective_active_ratio_percent_after') or 0.0}",
            )
            next_hygiene_at = now + max(args.pool_hygiene_interval_seconds, 1)
            did_work = True

        if now >= next_browser_at:
            target_url = warm_urls[url_index % len(warm_urls)]
            runtime_browser_regions = list(browser_regions)
            if auto_browser_regions_from_db:
                db_regions = load_browser_regions_from_db(
                    db_path=args.db,
                    mode=mode,
                    selected_source_labels=selected_source_labels,
                    max_regions=max(args.max_browser_regions, 1),
                )
                if db_regions:
                    runtime_browser_regions = db_regions
            browser_region = (
                runtime_browser_regions[url_index % len(runtime_browser_regions)]
                if runtime_browser_regions
                else ""
            )
            url_index += 1
            event = run_browser_probe(
                workload="external",
                mode=mode,
                base_url=args.base_url,
                api_key=args.api_key,
                endpoint=args.browser_endpoint,
                url=target_url,
                timeout_seconds=max(args.browser_timeout_seconds, 1),
                browser_region=browser_region,
                fingerprint_profile_id=fingerprint_profile_id,
                poll_interval_seconds=max(args.task_poll_interval_seconds, 1),
                poll_max_seconds=max(args.task_poll_max_seconds, 1),
            )
            events.append(event)
            print(
                f"[proxy-real-longrun] external browser status={event['status']} "
                f"url={target_url} proxy={event.get('proxy_id') or 'none'} "
                f"region={event.get('requested_region') or 'none'} "
                f"hot_regions={','.join(event.get('hot_regions_during_request') or []) or 'none'} "
                f"failure_scope={event.get('failure_scope') or 'none'}",
            )
            if stateful_url:
                stateful_region = sticky_stateful_region or browser_region
                if not sticky_stateful_region and stateful_region:
                    sticky_stateful_region = stateful_region
                stateful_event = run_browser_probe(
                    workload="stateful",
                    mode=mode,
                    base_url=args.base_url,
                    api_key=args.api_key,
                    endpoint=args.stateful_endpoint,
                    url=stateful_url,
                    timeout_seconds=max(args.browser_timeout_seconds, 1),
                    browser_region=stateful_region,
                    fingerprint_profile_id=fingerprint_profile_id,
                    poll_interval_seconds=max(args.task_poll_interval_seconds, 1),
                    poll_max_seconds=max(args.task_poll_max_seconds, 1),
                )
                events.append(stateful_event)
                print(
                    f"[proxy-real-longrun] stateful browser status={stateful_event['status']} "
                    f"proxy={stateful_event.get('proxy_id') or 'none'} "
                    f"region={stateful_event.get('requested_region') or 'none'} "
                    f"hot_regions={','.join(stateful_event.get('hot_regions_during_request') or []) or 'none'} "
                    f"identity={stateful_event.get('identity_session_status') or 'none'} "
                    f"cookies={stateful_event.get('cookie_restore_count') or 0}/{stateful_event.get('cookie_persist_count') or 0}",
                )
                if str(stateful_event.get("status") or "") == "succeeded":
                    for followup_index in range(stateful_followup_count):
                        followup_event = run_browser_probe(
                            workload="stateful_followup",
                            mode=mode,
                            base_url=args.base_url,
                            api_key=args.api_key,
                            endpoint=args.stateful_endpoint,
                            url=stateful_url,
                            timeout_seconds=max(args.browser_timeout_seconds, 1),
                            browser_region=sticky_stateful_region,
                            fingerprint_profile_id=fingerprint_profile_id,
                            poll_interval_seconds=max(args.task_poll_interval_seconds, 1),
                            poll_max_seconds=max(args.task_poll_max_seconds, 1),
                        )
                        events.append(followup_event)
                        print(
                            f"[proxy-real-longrun] stateful followup {followup_index + 1}/{stateful_followup_count} "
                            f"status={followup_event['status']} "
                            f"proxy={followup_event.get('proxy_id') or 'none'} "
                            f"region={followup_event.get('requested_region') or 'none'} "
                            f"hot_regions={','.join(followup_event.get('hot_regions_during_request') or []) or 'none'} "
                            f"identity={followup_event.get('identity_session_status') or 'none'} "
                            f"cookies={followup_event.get('cookie_restore_count') or 0}/{followup_event.get('cookie_persist_count') or 0} "
                            f"local={followup_event.get('local_storage_restore_count') or 0}/{followup_event.get('local_storage_persist_count') or 0} "
                            f"session={followup_event.get('session_storage_restore_count') or 0}/{followup_event.get('session_storage_persist_count') or 0}",
                        )
            next_browser_at = now + max(args.browser_interval_seconds, 1)
            did_work = True

        if now >= next_status_at:
            try:
                status = fetch_status(args.base_url, args.api_key)
                snapshot = {"captured_at": int(time.time()), "status": status}
                status_snapshots.append(snapshot)
                pool = dict(status.get("proxy_pool_status") or {})
                sessions = dict(status.get("identity_session_metrics") or {})
                sites = dict(status.get("proxy_site_metrics") or {})
                print(
                    "[proxy-real-longrun] sample "
                    f"active={int(pool.get('active') or 0)}/{int(pool.get('total') or 0)} "
                    f"candidate={int(pool.get('candidate') or 0)} "
                    f"reused_sessions={int(sessions.get('reused_sessions') or 0)} "
                    f"site_records={int(sites.get('site_records') or 0)}",
                )
            except Exception as exc:  # noqa: BLE001
                errors.append(str(exc))
                print(f"[proxy-real-longrun] sample failed: {exc}", file=sys.stderr)
            next_status_at = now + max(args.status_interval_seconds, 1)
            did_work = True

        if did_work:
            continue

        next_wakeup = min(next_harvest_at, next_status_at, next_browser_at, deadline)
        if pool_hygiene_enabled:
            next_wakeup = min(next_wakeup, next_hygiene_at)
        if geo_enrich_enabled:
            next_wakeup = min(next_wakeup, next_geo_enrich_at)
        time.sleep(max(0.2, min(1.0, next_wakeup - time.time())))

    try:
        status_snapshots.append(
            {
                "captured_at": int(time.time()),
                "status": fetch_status(args.base_url, args.api_key),
            }
        )
    except Exception as exc:  # noqa: BLE001
        errors.append(str(exc))

    raw_output = resolve_path(args.raw_output)
    txt_output = resolve_path(args.txt_output)
    json_output = resolve_path(args.json_output)
    raw_payload = {
        "base_url": args.base_url,
        "mode": mode,
        "preset": preset_name,
        "generated_at": int(time.time()),
        "config_path": config_path.as_posix(),
        "duration_seconds": max(args.duration_seconds, 1),
        "browser_endpoint": args.browser_endpoint,
        "stateful_url": stateful_url or None,
        "stateful_endpoint": args.stateful_endpoint,
        "stateful_followup_count": stateful_followup_count,
        "sticky_stateful_region": sticky_stateful_region or None,
        "fingerprint_profile_id": fingerprint_profile_id or None,
        "pool_hygiene_enabled": pool_hygiene_enabled,
        "pool_hygiene_interval_seconds": max(args.pool_hygiene_interval_seconds, 1),
        "pool_hygiene_script": pool_hygiene_script.as_posix() if pool_hygiene_enabled else None,
        "pool_hygiene_extra_args": hygiene_extra_args,
        "geo_enrich_enabled": geo_enrich_enabled,
        "geo_enrich_interval_seconds": max(args.geo_enrich_interval_seconds, 1),
        "geo_enrich_limit": max(args.geo_enrich_limit, 0),
        "geo_enrich_script": geo_enrich_script.as_posix() if geo_enrich_enabled else None,
        "warm_urls": warm_urls,
        "browser_regions": browser_regions,
        "auto_browser_regions_from_db": auto_browser_regions_from_db,
        "max_browser_regions": max(args.max_browser_regions, 1),
        "events": events,
        "errors": errors,
        "status_snapshots": status_snapshots,
    }
    write_json(raw_output, raw_payload)

    report_cmd = [
        sys.executable,
        str(ROOT_DIR / "scripts" / "proxy_longrun_report.py"),
        "--base-url",
        args.base_url,
        "--input-json",
        raw_output.as_posix(),
        "--txt-output",
        txt_output.as_posix(),
        "--json-output",
        json_output.as_posix(),
    ]
    report = subprocess.run(
        report_cmd,
        cwd=ROOT_DIR,
        check=False,
        capture_output=True,
        text=True,
    )
    if report.stdout:
        print(report.stdout, end="")
    if report.stderr:
        print(report.stderr, end="", file=sys.stderr)
    if report.returncode != 0:
        return report.returncode
    print(
        f"[proxy-real-longrun] outputs raw={raw_output} txt={txt_output} json={json_output}",
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
