#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROFILE="public-smoke"

PP_BASE_URL="${PERSONA_PILOT_BASE_URL:-http://127.0.0.1:3000}"
GATEWAY_BASE_URL="${GATEWAY_VERIFY_BASE_URL:-http://127.0.0.1:8787}"
PP_SERVER_BIN="${PP_SERVER_BIN:-$ROOT/target/debug/PersonaPilot}"
GATEWAY_BIN="${GATEWAY_VERIFY_BIN:-$ROOT/target/release/PersonaPilot}"
REAL_LIGHTPANDA_BIN="${REAL_LIGHTPANDA_BIN:-/usr/local/bin/lightpanda}"
ENV_FILE="${GATEWAY_VERIFY_ENV_FILE:-$ROOT/.env.gateway}"
PROXY_CONFIG_PATH="${PERSONA_PILOT_PROXY_HARVEST_CONFIG:-$ROOT/data/proxy_sources.json}"
EXPECTED_MODE="${PERSONA_PILOT_PROXY_MODE:-demo_public}"
CONTINUITY_PROFILE_ID="${PROXY_VERIFY_REAL_FINGERPRINT_PROFILE_ID:-${PROXY_REAL_LONGRUN_FINGERPRINT_PROFILE_ID:-}}"

usage() {
  cat <<'EOF'
preflight_release_env.sh

Usage:
  bash scripts/preflight_release_env.sh --profile public-smoke
  bash scripts/preflight_release_env.sh --profile prod-live
  bash scripts/preflight_release_env.sh --profile gateway-upstream

Profiles:
  public-smoke      demo/public smoke gate for browser 5 endpoints + gateway no-token
  prod-live         production-live gate for real proxy sources + continuity validation
  gateway-upstream  gateway no-token + real-upstream gate
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    -h|--help|help)
      usage
      exit 0
      ;;
    *)
      echo "[preflight] unknown arg: $1" >&2
      usage
      exit 1
      ;;
  esac
done

case "$PROFILE" in
  public-smoke|prod-live|gateway-upstream) ;;
  *)
    echo "[preflight] unsupported profile: $PROFILE" >&2
    usage
    exit 1
    ;;
esac

ok() {
  echo "[preflight] OK   $1"
}

warn() {
  echo "[preflight] WARN $1"
}

emit_result() {
  local reason_code="$1"
  local failure_scope="$2"
  local detail="$3"
  echo "[preflight] RESULT profile=$PROFILE reason_code=$reason_code failure_scope=$failure_scope detail=$detail"
}

normalize_mode() {
  local raw="${1:-}"
  if [[ -z "$raw" ]]; then
    printf '%s' ''
    return 0
  fi
  raw="${raw//-/_}"
  raw="${raw,,}"
  case "$raw" in
    prod_live) printf '%s' 'prod_live' ;;
    *) printf '%s' 'demo_public' ;;
  esac
}

fail_with() {
  local reason_code="$1"
  local detail="$2"
  local failure_scope="${3:-preflight}"
  echo "[preflight] FAIL $detail" >&2
  emit_result "$reason_code" "$failure_scope" "$detail" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail_with "preflight_failed" "missing command: $1" "tooling"
}

require_exec() {
  local path="$1"
  local label="$2"
  [[ -x "$path" ]] || fail_with "binary_missing" "$label not executable: $path" "binary_guard"
  ok "$label: $path"
}

require_file() {
  local path="$1"
  local label="$2"
  [[ -f "$path" ]] || fail_with "preflight_failed" "$label missing: $path" "file_guard"
  ok "$label: $path"
}

port_pid() {
  local port="$1"
  local line
  line="$(ss -ltnp "( sport = :$port )" 2>/dev/null | tail -n +2 | head -n 1 || true)"
  if [[ -z "$line" ]]; then
    return 0
  fi
  printf '%s' "$line" | sed -n 's/.*pid=\([0-9]\+\).*/\1/p' | head -n 1
}

pid_cmdline() {
  local pid="$1"
  if [[ -r "/proc/$pid/cmdline" ]]; then
    tr '\0' ' ' < "/proc/$pid/cmdline"
  fi
}

pid_env_var() {
  local pid="$1"
  local key="$2"
  if [[ -r "/proc/$pid/environ" ]]; then
    tr '\0' '\n' < "/proc/$pid/environ" | sed -n "s/^${key}=//p" | head -n 1
  fi
}

http_status() {
  local url="$1"
  curl -sS -o /dev/null -w "%{http_code}" "$url" 2>/dev/null || echo "000"
}

json_read_field_from_file() {
  local file_path="$1"
  local key_path="$2"
  python3 - "$file_path" "$key_path" <<'PY'
import json
import sys

file_path, key_path = sys.argv[1], sys.argv[2]
with open(file_path, "r", encoding="utf-8") as fh:
    value = json.load(fh)
for part in key_path.split("."):
    if not part:
        continue
    if isinstance(value, list):
        try:
            value = value[int(part)]
        except Exception:
            value = None
            break
    elif isinstance(value, dict):
        value = value.get(part)
    else:
        value = None
        break
if value is None:
    print("")
elif isinstance(value, (dict, list)):
    print(json.dumps(value, ensure_ascii=False))
else:
    print(value)
PY
}

load_status_snapshot() {
  local tmp
  tmp="$(mktemp)"
  if ! curl -fsS "$PP_BASE_URL/status" > "$tmp"; then
    rm -f "$tmp"
    fail_with "cdp_unhealthy" "control plane status endpoint is unavailable: $PP_BASE_URL/status" "control_plane"
  fi
  printf '%s' "$tmp"
}

proxy_config_metric() {
  local metric="$1"
  python3 - "$PROXY_CONFIG_PATH" "$metric" <<'PY'
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
metric = sys.argv[2]
if not path.exists():
    print("0")
    raise SystemExit(0)

payload = json.loads(path.read_text(encoding="utf-8"))
items = payload if isinstance(payload, list) else payload.get("items") or []

summary = {
    "source_count": 0,
    "demo_count": 0,
    "prod_count": 0,
    "public_tier_count": 0,
}
for item in items:
    if not isinstance(item, dict):
        continue
    summary["source_count"] += 1
    if bool(item.get("for_demo", True)):
        summary["demo_count"] += 1
    if bool(item.get("for_prod", False)):
        summary["prod_count"] += 1
    if str(item.get("source_tier") or "").strip().lower() == "public":
        summary["public_tier_count"] += 1

print(summary.get(metric, 0))
PY
}

check_control_plane_port() {
  local pid
  pid="$(port_pid 3000 || true)"
  [[ -n "$pid" ]] || fail_with "cdp_unhealthy" "port 3000 is not listening" "control_plane"
  local cmd
  cmd="$(pid_cmdline "$pid")"
  [[ "$cmd" == *"PersonaPilot"* ]] || fail_with "port_conflict" "port 3000 owner is not PersonaPilot (pid=$pid cmd='$cmd')" "port_guard"
  ok "port 3000 owner: pid=$pid"
}

check_gateway_port() {
  local pid
  pid="$(port_pid 8787 || true)"
  if [[ -z "$pid" ]]; then
    warn "port 8787 currently not listening (gateway verify may start gateway)"
    return 0
  fi
  local cmd
  cmd="$(pid_cmdline "$pid")"
  [[ "$cmd" == *"PersonaPilot"* ]] || fail_with "port_conflict" "port 8787 occupied by non repo-owned process (pid=$pid cmd='$cmd')" "port_guard"
  ok "port 8787 owner: pid=$pid"
}

check_control_plane_health() {
  local normalized_expected_mode
  normalized_expected_mode="$(normalize_mode "$EXPECTED_MODE")"
  local status_3000
  status_3000="$(http_status "$PP_BASE_URL/health")"
  [[ "$status_3000" == "200" ]] || fail_with "cdp_unhealthy" "control plane health not ready: $PP_BASE_URL/health status=$status_3000" "control_plane"
  ok "control plane health: $PP_BASE_URL/health status=200"

  local status_file
  status_file="$(load_status_snapshot)"
  local runtime_mode
  runtime_mode="$(json_read_field_from_file "$status_file" "mode")"
  if [[ -z "$runtime_mode" ]]; then
    runtime_mode="$(json_read_field_from_file "$status_file" "proxy_pool_status.mode")"
  fi
  if [[ -z "$runtime_mode" ]]; then
    local control_plane_pid
    control_plane_pid="$(port_pid 3000 || true)"
    if [[ -n "$control_plane_pid" ]]; then
      runtime_mode="$(pid_env_var "$control_plane_pid" "PERSONA_PILOT_PROXY_MODE")"
    fi
  fi
  runtime_mode="$(normalize_mode "$runtime_mode")"
  rm -f "$status_file"

  case "$PROFILE" in
    public-smoke)
      [[ "$normalized_expected_mode" == "demo_public" ]] || fail_with "preflight_failed" "public-smoke requires PERSONA_PILOT_PROXY_MODE=demo_public, got $EXPECTED_MODE" "mode_guard"
      [[ "$runtime_mode" == "demo_public" ]] || fail_with "preflight_failed" "public-smoke requires /status mode=demo_public, got ${runtime_mode:-<empty>}" "mode_guard"
      ;;
    prod-live)
      [[ "$normalized_expected_mode" == "prod_live" ]] || fail_with "preflight_failed" "prod-live requires PERSONA_PILOT_PROXY_MODE=prod_live, got $EXPECTED_MODE" "mode_guard"
      [[ "$runtime_mode" == "prod_live" ]] || fail_with "preflight_failed" "prod-live requires /status mode=prod_live, got ${runtime_mode:-<empty>}" "mode_guard"
      ;;
  esac
  ok "runtime mode: ${runtime_mode:-unknown}"
}

check_public_smoke_config() {
  if [[ ! -f "$PROXY_CONFIG_PATH" ]]; then
    warn "proxy config missing for public-smoke: $PROXY_CONFIG_PATH"
    return 0
  fi
  local demo_count source_count
  demo_count="$(proxy_config_metric "demo_count")"
  source_count="$(proxy_config_metric "source_count")"
  if [[ "$source_count" == "0" ]]; then
    warn "proxy config exists but contains no sources: $PROXY_CONFIG_PATH"
    return 0
  fi
  if [[ "$demo_count" == "0" ]]; then
    warn "public-smoke config has no for_demo sources: $PROXY_CONFIG_PATH"
  else
    ok "public-smoke config includes demo/public sources: $PROXY_CONFIG_PATH"
  fi
}

check_prod_live_contract() {
  [[ -n "$CONTINUITY_PROFILE_ID" ]] || fail_with "preflight_failed" "prod-live requires continuity fingerprint profile id via PROXY_VERIFY_REAL_FINGERPRINT_PROFILE_ID or PROXY_REAL_LONGRUN_FINGERPRINT_PROFILE_ID" "continuity_profile_guard"
  require_file "$PROXY_CONFIG_PATH" "proxy harvest config"

  local source_count prod_count
  source_count="$(proxy_config_metric "source_count")"
  prod_count="$(proxy_config_metric "prod_count")"
  [[ "$source_count" != "0" ]] || fail_with "preflight_failed" "prod-live config contains no sources: $PROXY_CONFIG_PATH" "source_mode_guard"
  [[ "$prod_count" != "0" ]] || fail_with "preflight_failed" "prod-live config has no for_prod sources and must not use demo/public-only config: $PROXY_CONFIG_PATH" "source_mode_guard"

  ok "prod-live fingerprint profile: $CONTINUITY_PROFILE_ID"
  ok "prod-live config includes for_prod sources: $PROXY_CONFIG_PATH"
}

main() {
  need_cmd bash
  need_cmd curl
  need_cmd python3
  need_cmd ss

  case "$PROFILE" in
    public-smoke)
      require_exec "$PP_SERVER_BIN" "PersonaPilot debug binary"
      require_exec "$REAL_LIGHTPANDA_BIN" "Lightpanda binary"
      require_exec "$GATEWAY_BIN" "PersonaPilot release binary"
      require_file "$ENV_FILE" "gateway env file"
      check_control_plane_port
      check_gateway_port
      check_control_plane_health
      check_public_smoke_config
      ;;
    prod-live)
      require_exec "$PP_SERVER_BIN" "PersonaPilot debug binary"
      require_exec "$REAL_LIGHTPANDA_BIN" "Lightpanda binary"
      check_control_plane_port
      check_control_plane_health
      check_prod_live_contract
      ;;
    gateway-upstream)
      require_exec "$GATEWAY_BIN" "PersonaPilot release binary"
      require_file "$ENV_FILE" "gateway env file"
      check_gateway_port
      ;;
  esac

  ok "profile=$PROFILE"
  emit_result "ok" "none" "profile=$PROFILE"
  echo "[preflight] PASS"
}

main "$@"
