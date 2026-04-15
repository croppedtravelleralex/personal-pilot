#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROFILE="public-smoke"
WITH_UPSTREAM=0
SKIP_PREFLIGHT=0
SKIP_CARGO_TEST=1
SKIP_BROWSER=0
SKIP_ANOMALIES=0
SKIP_GATEWAY=0

PP_BASE_URL="${PERSONA_PILOT_BASE_URL:-http://127.0.0.1:3000}"
PP_SERVER_BIN="${PP_SERVER_BIN:-$ROOT/target/debug/PersonaPilot}"
REAL_LIGHTPANDA_BIN="${REAL_LIGHTPANDA_BIN:-/usr/local/bin/lightpanda}"
TEMP_STUB_DIR="${RELEASE_VERIFY_TEMP_DIR:-/tmp}"
TEMP_TIMEOUT_STUB="$TEMP_STUB_DIR/lightpanda_timeout_stub.sh"
TEMP_NON_ZERO_STUB="$TEMP_STUB_DIR/lightpanda_non_zero_stub.sh"
TEMP_MISSING_BIN="$TEMP_STUB_DIR/lightpanda_missing_binary_stub.sh"
CARGO_BIN="${CARGO_BIN:-/root/.cargo/bin/cargo}"
LONGRUN_REPORT_JSON="${RELEASE_VERIFY_LONGRUN_REPORT_JSON:-$ROOT/reports/proxy_real_longrun_latest.json}"
PROD_LIVE_MIN_EFFECTIVE_RATIO_PERCENT="${RELEASE_VERIFY_PROD_LIVE_MIN_EFFECTIVE_RATIO_PERCENT:-35}"
PROD_LIVE_MIN_PROMOTION_RATE_PERCENT="${RELEASE_VERIFY_PROD_LIVE_MIN_PROMOTION_RATE_PERCENT:-75}"
PROD_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT="${RELEASE_VERIFY_PROD_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT:-98}"
PROD_LIVE_MIN_RECENT_HOT_REGIONS="${RELEASE_VERIFY_PROD_LIVE_MIN_RECENT_HOT_REGIONS:-3}"
PROD_LIVE_MAX_SOURCE_TOP1_PERCENT="${RELEASE_VERIFY_PROD_LIVE_MAX_SOURCE_TOP1_PERCENT:-75}"
PROD_LIVE_MIN_GEO_COVERAGE_PERCENT="${RELEASE_VERIFY_PROD_LIVE_MIN_GEO_COVERAGE_PERCENT:-0}"

MANAGED_API=0
FINAL_STATUS="FAIL"
FINAL_REASON_CODE="unknown"
FINAL_FAILURE_SCOPE=""
FINAL_DETAIL=""
declare -a SUMMARY_LINES=()

usage() {
  cat <<'EOF'
release_baseline_verify.sh

Usage:
  bash scripts/release_baseline_verify.sh --profile public-smoke
  bash scripts/release_baseline_verify.sh --profile prod-live
  bash scripts/release_baseline_verify.sh --profile gateway-upstream

Options:
  --profile <name>      one of public-smoke / prod-live / gateway-upstream
  --with-upstream       legacy alias: also run real-upstream gateway verification
  --skip-preflight      skip scripts/preflight_release_env.sh
  --with-cargo-test     run cargo test -q before profile checks
  --skip-cargo-test     keep cargo test disabled
  --skip-browser        skip browser endpoint verification
  --skip-anomalies      skip missing-binary / timeout / non-zero anomaly verification
  --skip-gateway        skip gateway verification
  --help                print this help

Default behavior:
  - cargo test is skipped by default because this repo currently has unrelated compile blockers
  - reports are written to reports/release_<profile>_latest.txt
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --with-upstream)
      WITH_UPSTREAM=1
      shift
      ;;
    --skip-preflight)
      SKIP_PREFLIGHT=1
      shift
      ;;
    --with-cargo-test)
      SKIP_CARGO_TEST=0
      shift
      ;;
    --skip-cargo-test)
      SKIP_CARGO_TEST=1
      shift
      ;;
    --skip-browser)
      SKIP_BROWSER=1
      shift
      ;;
    --skip-anomalies)
      SKIP_ANOMALIES=1
      shift
      ;;
    --skip-gateway)
      SKIP_GATEWAY=1
      shift
      ;;
    -h|--help|help)
      usage
      exit 0
      ;;
    *)
      echo "[release-verify] unknown arg: $1" >&2
      usage
      exit 1
      ;;
  esac
done

case "$PROFILE" in
  public-smoke|prod-live|gateway-upstream) ;;
  *)
    echo "[release-verify] unsupported profile: $PROFILE" >&2
    usage
    exit 1
    ;;
esac

if [[ -z "${RELEASE_VERIFY_REPORT_FILE:-}" ]]; then
  REPORT_FILE="$ROOT/reports/release_${PROFILE//-/_}_latest.txt"
else
  REPORT_FILE="$RELEASE_VERIFY_REPORT_FILE"
fi

step() {
  local name="$1"
  echo
  echo "[release-verify] ===== $name ====="
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "[release-verify] missing command: $1" >&2
    exit 1
  }
}

record_step() {
  local status="$1"
  local seconds="$2"
  local reason_code="$3"
  local name="$4"
  local detail="${5:-}"
  SUMMARY_LINES+=("${status}\t${seconds}\t${reason_code}\t${name}\t${detail}")
}

set_failure_reason() {
  local reason_code="$1"
  local failure_scope="$2"
  local detail="$3"
  if [[ "$FINAL_REASON_CODE" == "unknown" || "$FINAL_REASON_CODE" == "ok" ]]; then
    FINAL_REASON_CODE="$reason_code"
    FINAL_FAILURE_SCOPE="$failure_scope"
    FINAL_DETAIL="$detail"
  fi
  echo "[release-verify] reason_code=$reason_code failure_scope=$failure_scope detail=$detail" >&2
}

run_step() {
  local name="$1"
  local reason_code="$2"
  local failure_scope="$3"
  shift 3
  local started ended elapsed
  started="$(date +%s)"
  step "$name"
  if "$@"; then
    ended="$(date +%s)"
    elapsed="$((ended - started))"
    record_step "PASS" "$elapsed" "ok" "$name"
    return 0
  fi
  ended="$(date +%s)"
  elapsed="$((ended - started))"
  set_failure_reason "$reason_code" "$failure_scope" "$name failed"
  record_step "FAIL" "$elapsed" "$reason_code" "$name" "$failure_scope"
  return 1
}

record_skip() {
  local name="$1"
  echo "[release-verify] skip $name"
  record_step "SKIP" "0" "skipped" "$name"
}

list_api_pids() {
  pgrep -f 'PersonaPilot$' || true
}

stop_api_server() {
  local pids
  pids="$(list_api_pids)"
  if [[ -z "$pids" ]]; then
    return 0
  fi

  while read -r pid; do
    [[ -z "$pid" ]] && continue
    kill "$pid" >/dev/null 2>&1 || true
  done <<< "$pids"

  for _ in $(seq 1 30); do
    if [[ -z "$(list_api_pids)" ]]; then
      return 0
    fi
    sleep 0.5
  done

  pids="$(list_api_pids)"
  while read -r pid; do
    [[ -z "$pid" ]] && continue
    kill -9 "$pid" >/dev/null 2>&1 || true
  done <<< "$pids"
}

start_api_with_runner_bin() {
  local runner_bin="$1"
  if [[ ! -x "$PP_SERVER_BIN" ]]; then
    echo "[release-verify] missing api binary: $PP_SERVER_BIN" >&2
    return 1
  fi
  PERSONA_PILOT_RUNNER=lightpanda \
  LIGHTPANDA_BIN="$runner_bin" \
  nohup "$PP_SERVER_BIN" > "$ROOT/pp.release-verify.out" 2>&1 &
}

wait_api_health() {
  local url="$PP_BASE_URL/health"
  for _ in $(seq 1 60); do
    if curl -sS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "[release-verify] api health timeout: $url" >&2
  return 1
}

make_timeout_stub() {
  cat > "$TEMP_TIMEOUT_STUB" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
sleep 30
EOF
  chmod +x "$TEMP_TIMEOUT_STUB"
}

make_non_zero_stub() {
  cat > "$TEMP_NON_ZERO_STUB" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "intentional non-zero from release verify stub" >&2
exit 7
EOF
  chmod +x "$TEMP_NON_ZERO_STUB"
}

ensure_real_api_online() {
  stop_api_server
  start_api_with_runner_bin "$REAL_LIGHTPANDA_BIN"
  wait_api_health
}

run_lightpanda_case() {
  local mode="$1"
  PERSONA_PILOT_BASE_URL="$PP_BASE_URL" bash scripts/lightpanda_verify.sh "$mode"
}

run_anomaly_case_with_bin() {
  local mode="$1"
  local runner_bin="$2"
  MANAGED_API=1
  stop_api_server
  start_api_with_runner_bin "$runner_bin"
  wait_api_health
  PERSONA_PILOT_BASE_URL="$PP_BASE_URL" bash scripts/lightpanda_verify.sh "$mode"
}

parse_preflight_result() {
  local log_file="$1"
  python3 - "$log_file" <<'PY'
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8", errors="replace")
matches = re.findall(
    r"reason_code=([^\s]+)\s+failure_scope=([^\s]+)\s+detail=(.*)",
    text,
)
if matches:
    code, scope, detail = matches[-1]
    print(f"{code}\t{scope}\t{detail}")
else:
    print("preflight_failed\tpreflight\tpreflight result unavailable")
PY
}

run_preflight_profile() {
  if [[ "$SKIP_PREFLIGHT" == "1" ]]; then
    record_skip "preflight release environment ($PROFILE)"
    return 0
  fi

  local started ended elapsed log_file parsed reason_code failure_scope detail
  started="$(date +%s)"
  log_file="$(mktemp)"
  step "preflight release environment ($PROFILE)"
  if bash scripts/preflight_release_env.sh --profile "$PROFILE" 2>&1 | tee "$log_file"; then
    ended="$(date +%s)"
    elapsed="$((ended - started))"
    record_step "PASS" "$elapsed" "ok" "preflight release environment ($PROFILE)"
    rm -f "$log_file"
    return 0
  fi

  ended="$(date +%s)"
  elapsed="$((ended - started))"
  parsed="$(parse_preflight_result "$log_file")"
  rm -f "$log_file"
  reason_code="$(printf '%s' "$parsed" | cut -f1)"
  failure_scope="$(printf '%s' "$parsed" | cut -f2)"
  detail="$(printf '%s' "$parsed" | cut -f3-)"
  set_failure_reason "$reason_code" "$failure_scope" "$detail"
  record_step "FAIL" "$elapsed" "$reason_code" "preflight release environment ($PROFILE)" "$detail"
  return 1
}

json_query_from_file() {
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

latest_browser_context() {
  local tmp
  tmp="$(mktemp)"
  if ! curl -fsS "$PP_BASE_URL/status?limit=20&offset=0" > "$tmp"; then
    rm -f "$tmp"
    return 1
  fi
  python3 - "$tmp" <<'PY'
import json
import re
import sys
from pathlib import Path

payload = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
items = payload.get("latest_browser_tasks") or []
if not items:
    items = [
        item
        for item in (payload.get("latest_tasks") or [])
        if item.get("kind") in {"open_page", "get_html", "get_title", "get_final_url", "extract_text"}
    ]
item = items[0] if items else {}
summary_text = "\n".join(
    f"{artifact.get('title', '')}\n{artifact.get('summary', '')}\n{artifact.get('key', '')}"
    for artifact in (item.get("summary_artifacts") or [])
    if isinstance(artifact, dict)
)
match = re.search(r"execution_stage=([A-Za-z0-9_.:-]+)", summary_text)
execution_stage = match.group(1) if match else ""
print(json.dumps({
    "task_id": item.get("id"),
    "failure_scope": item.get("failure_scope"),
    "browser_failure_signal": item.get("browser_failure_signal"),
    "execution_stage": execution_stage,
    "error_message": item.get("error_message"),
    "proxy_id": item.get("proxy_id"),
    "proxy_resolution_status": item.get("proxy_resolution_status"),
    "selection_reason_summary": item.get("selection_reason_summary"),
}, ensure_ascii=False))
PY
  rm -f "$tmp"
}

capture_browser_failure_reason() {
  local default_reason="$1"
  local default_scope="$2"
  local tmp reason_code failure_scope browser_failure_signal execution_stage proxy_id selection_reason error_message detail
  tmp="$(mktemp)"
  if latest_browser_context > "$tmp"; then
    failure_scope="$(json_query_from_file "$tmp" "failure_scope")"
    browser_failure_signal="$(json_query_from_file "$tmp" "browser_failure_signal")"
    execution_stage="$(json_query_from_file "$tmp" "execution_stage")"
    error_message="$(json_query_from_file "$tmp" "error_message")"
    proxy_id="$(json_query_from_file "$tmp" "proxy_id")"
    selection_reason="$(json_query_from_file "$tmp" "selection_reason_summary")"
    reason_code="$default_reason"
    if [[ "${error_message,,}" == *"proxy not found"* ]]; then
      reason_code="proxy_claim_lost"
    elif [[ -z "$proxy_id" && "$selection_reason" == *"no eligible active proxy"* ]]; then
      reason_code="no_active_proxy"
    fi
    detail="task_id=$(json_query_from_file "$tmp" "task_id") browser_failure_signal=${browser_failure_signal:-none} execution_stage=${execution_stage:-unknown} selection_reason=${selection_reason:-none} error_message=${error_message:-none}"
    set_failure_reason "$reason_code" "${failure_scope:-$default_scope}" "$detail"
  else
    set_failure_reason "$default_reason" "$default_scope" "browser verification failed and status context was unavailable"
  fi
  rm -f "$tmp"
}

run_browser_case() {
  local case_name="$1"
  local reason_code="$2"
  local started ended elapsed
  started="$(date +%s)"
  step "lightpanda_verify $case_name"
  if run_lightpanda_case "$case_name"; then
    ended="$(date +%s)"
    elapsed="$((ended - started))"
    record_step "PASS" "$elapsed" "ok" "lightpanda_verify $case_name"
    return 0
  fi
  ended="$(date +%s)"
  elapsed="$((ended - started))"
  capture_browser_failure_reason "$reason_code" "browser_verify"
  record_step "FAIL" "$elapsed" "$FINAL_REASON_CODE" "lightpanda_verify $case_name" "$FINAL_DETAIL"
  return 1
}

run_gateway_case() {
  local mode="$1"
  local reason_code="$2"
  local started ended elapsed
  started="$(date +%s)"
  step "gateway_verify $mode"
  if bash scripts/gateway_verify.sh "$mode"; then
    ended="$(date +%s)"
    elapsed="$((ended - started))"
    record_step "PASS" "$elapsed" "ok" "gateway_verify $mode"
    return 0
  fi
  ended="$(date +%s)"
  elapsed="$((ended - started))"
  set_failure_reason "$reason_code" "gateway" "gateway_verify $mode failed"
  record_step "FAIL" "$elapsed" "$reason_code" "gateway_verify $mode" "gateway"
  return 1
}

prod_live_report_verdict() {
  local report_file="$1"
  python3 scripts/release_prod_live_gate.py \
    "$report_file" \
    --min-effective-ratio-percent "$PROD_LIVE_MIN_EFFECTIVE_RATIO_PERCENT" \
    --min-promotion-rate-percent "$PROD_LIVE_MIN_PROMOTION_RATE_PERCENT" \
    --min-browser-success-rate-percent "$PROD_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT" \
    --min-recent-hot-regions "$PROD_LIVE_MIN_RECENT_HOT_REGIONS" \
    --max-source-top1-percent "$PROD_LIVE_MAX_SOURCE_TOP1_PERCENT" \
    --min-geo-coverage-percent "$PROD_LIVE_MIN_GEO_COVERAGE_PERCENT"
}

run_prod_live_longrun() {
  local started ended elapsed verdict reason_code failure_scope detail
  started="$(date +%s)"
  step "prod-live longrun driver"
  if ! bash scripts/proxy_mainline_verify.sh real-live; then
    ended="$(date +%s)"
    elapsed="$((ended - started))"
    if verdict="$(prod_live_report_verdict "$LONGRUN_REPORT_JSON" 2>/dev/null)"; then
      :
    else
      verdict="$(prod_live_report_verdict "$LONGRUN_REPORT_JSON" 2>/dev/null || printf '%s' 'preflight_failed	prod_live_driver	prod-live driver failed before report validation')"
    fi
    reason_code="$(printf '%s' "$verdict" | cut -f1)"
    failure_scope="$(printf '%s' "$verdict" | cut -f2)"
    detail="$(printf '%s' "$verdict" | cut -f3-)"
    set_failure_reason "$reason_code" "$failure_scope" "$detail"
    record_step "FAIL" "$elapsed" "$reason_code" "prod-live longrun driver" "$detail"
    return 1
  fi
  ended="$(date +%s)"
  elapsed="$((ended - started))"

  verdict="$(prod_live_report_verdict "$LONGRUN_REPORT_JSON")" || {
    reason_code="$(printf '%s' "$verdict" | cut -f1)"
    failure_scope="$(printf '%s' "$verdict" | cut -f2)"
    detail="$(printf '%s' "$verdict" | cut -f3-)"
    set_failure_reason "$reason_code" "$failure_scope" "$detail"
    record_step "FAIL" "$elapsed" "$reason_code" "prod-live acceptance gate" "$detail"
    return 1
  }
  record_step "PASS" "$elapsed" "ok" "prod-live longrun driver"
  return 0
}

append_prod_live_report_metrics() {
  if [[ ! -f "$LONGRUN_REPORT_JSON" ]]; then
    return 0
  fi
  python3 - "$LONGRUN_REPORT_JSON" <<'PY'
import json
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
payload = json.loads(report_path.read_text(encoding="utf-8"))
summary = payload.get("summary") or {}
latest = summary.get("latest") or {}
event_summary = summary.get("event_summary") or {}
source_quality_summary = payload.get("source_quality_summary") or {}
effective_geo_quality_summary = source_quality_summary.get("effective_geo_quality_summary") or {}

def emit(key: str, value) -> None:
    if isinstance(value, (dict, list)):
        print(f"{key}={json.dumps(value, ensure_ascii=False)}")
    else:
        print(f"{key}={value}")

emit("prod_live_effective_active_ratio_percent_median", ((summary.get("effective_active_ratio_percent") or {}).get("median")) or 0.0)
emit("prod_live_promotion_rate_median", ((summary.get("promotion_rate") or {}).get("median")) or 0.0)
emit("prod_live_browser_success_rate_percent", summary.get("browser_success_rate_percent") or 0.0)
emit("prod_live_recent_hot_regions", latest.get("recent_hot_regions") or summary.get("recent_hot_regions_union") or [])
emit("prod_live_source_concentration_top1_percent", latest.get("source_concentration_top1_percent") or ((summary.get("source_concentration_top1_percent") or {}).get("median")) or 0.0)
emit("prod_live_browser_proxy_not_found_failures", event_summary.get("browser_proxy_not_found_failures") or 0)
emit("prod_live_effective_geo_quality_summary", effective_geo_quality_summary)
PY
}

write_summary_report() {
  mkdir -p "$(dirname "$REPORT_FILE")"
  {
    echo "timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "profile=$PROFILE"
    echo "result=$FINAL_STATUS"
    echo "reason_code=$FINAL_REASON_CODE"
    echo "failure_scope=$FINAL_FAILURE_SCOPE"
    echo "detail=$FINAL_DETAIL"
    echo "with_upstream=$WITH_UPSTREAM"
    echo "skip_preflight=$SKIP_PREFLIGHT"
    echo "skip_cargo_test=$SKIP_CARGO_TEST"
    echo "skip_browser=$SKIP_BROWSER"
    echo "skip_anomalies=$SKIP_ANOMALIES"
    echo "skip_gateway=$SKIP_GATEWAY"
    echo "persona_pilot_base_url=$PP_BASE_URL"
    echo "real_lightpanda_bin=$REAL_LIGHTPANDA_BIN"
    if [[ "$PROFILE" == "prod-live" || -f "$LONGRUN_REPORT_JSON" ]]; then
      append_prod_live_report_metrics
    fi
    echo "---"
    printf "status\tseconds\treason_code\tstep\tdetail\n"
    for line in "${SUMMARY_LINES[@]}"; do
      printf "%b\n" "$line"
    done
  } > "$REPORT_FILE"
  echo "[release-verify] report: $REPORT_FILE"
}

cleanup() {
  rm -f "$TEMP_TIMEOUT_STUB" "$TEMP_NON_ZERO_STUB" >/dev/null 2>&1 || true
  if [[ "$MANAGED_API" == "1" ]]; then
    ensure_real_api_online || true
  fi
}

on_exit() {
  local code="$1"
  if [[ "$code" == "0" ]]; then
    FINAL_STATUS="PASS"
    if [[ "$FINAL_REASON_CODE" == "unknown" ]]; then
      FINAL_REASON_CODE="ok"
      FINAL_FAILURE_SCOPE="none"
      FINAL_DETAIL="profile=$PROFILE"
    fi
  else
    FINAL_STATUS="FAIL"
  fi
  cleanup
  write_summary_report
}

run_public_smoke_profile() {
  run_step "stage entry consistency" "preflight_failed" "stage_entry" python3 scripts/check_stage_entry_consistency.py
  run_step "control plane health" "cdp_unhealthy" "control_plane" curl -fsS "$PP_BASE_URL/health"

  if [[ "$SKIP_BROWSER" == "0" ]]; then
    run_browser_case open "browser_open_failed"
    run_browser_case html "browser_contract_failed"
    run_browser_case title "browser_contract_failed"
    run_browser_case final-url "browser_contract_failed"
    run_browser_case text "browser_contract_failed"
  else
    record_skip "lightpanda browser verification"
  fi

  if [[ "$SKIP_ANOMALIES" == "0" ]]; then
    make_timeout_stub
    make_non_zero_stub
    run_step "lightpanda_verify missing-binary (managed api runner)" "browser_contract_failed" "anomaly_contract" run_anomaly_case_with_bin missing-binary "$TEMP_MISSING_BIN"
    run_step "lightpanda_verify timeout (managed api runner)" "browser_contract_failed" "anomaly_contract" run_anomaly_case_with_bin timeout "$TEMP_TIMEOUT_STUB"
    run_step "lightpanda_verify non-zero (managed api runner)" "browser_contract_failed" "anomaly_contract" run_anomaly_case_with_bin non-zero "$TEMP_NON_ZERO_STUB"
    run_step "restore api with real lightpanda bin" "cdp_unhealthy" "control_plane" ensure_real_api_online
  else
    record_skip "lightpanda anomaly verification"
  fi

  if [[ "$SKIP_GATEWAY" == "0" ]]; then
    run_gateway_case no-token "gateway_no_token_failed"
    if [[ "$WITH_UPSTREAM" == "1" ]]; then
      run_gateway_case real-upstream "gateway_upstream_failed"
    fi
  else
    record_skip "gateway verification"
  fi
}

run_prod_live_profile() {
  run_step "control plane health" "cdp_unhealthy" "control_plane" curl -fsS "$PP_BASE_URL/health"

  if [[ "$SKIP_BROWSER" == "0" ]]; then
    run_browser_case open "browser_open_failed"
  else
    record_skip "prod-live browser open"
  fi

  run_prod_live_longrun
}

run_gateway_upstream_profile() {
  if [[ "$SKIP_GATEWAY" == "1" ]]; then
    record_skip "gateway verification"
    return 0
  fi
  run_gateway_case no-token "gateway_no_token_failed"
  run_gateway_case real-upstream "gateway_upstream_failed"
}

run_main() {
  need_cmd bash
  need_cmd python3
  need_cmd curl

  if [[ "$SKIP_CARGO_TEST" == "0" ]]; then
    if [[ ! -x "$CARGO_BIN" ]]; then
      CARGO_BIN="$(command -v cargo)"
    fi
    run_step "cargo test -q" "preflight_failed" "cargo_test" "$CARGO_BIN" test -q
  else
    record_skip "cargo test -q"
  fi

  run_preflight_profile

  case "$PROFILE" in
    public-smoke)
      run_public_smoke_profile
      ;;
    prod-live)
      run_prod_live_profile
      ;;
    gateway-upstream)
      run_gateway_upstream_profile
      ;;
  esac

  echo
  echo "[release-verify] PASS profile=$PROFILE"
}

main() {
  trap 'on_exit $?' EXIT
  run_main
}

main "$@"
