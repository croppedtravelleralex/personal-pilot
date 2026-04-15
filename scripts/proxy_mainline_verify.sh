#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT_DIR="${ROOT_DIR}/reports"
MAIN_REPORT="${REPORT_DIR}/proxy_mainline_latest.txt"
LONGRUN_REPORT="${REPORT_DIR}/proxy_longrun_latest.txt"
LONGRUN_JSON="${REPORT_DIR}/proxy_longrun_latest.json"
REAL_LONGRUN_REPORT="${REPORT_DIR}/proxy_real_longrun_latest.txt"
REAL_LONGRUN_JSON="${REPORT_DIR}/proxy_real_longrun_latest.json"
REAL_LONGRUN_RAW="${REPORT_DIR}/proxy_real_longrun_driver_latest.json"
SOURCE_QUALITY_REPORT="${REPORT_DIR}/source_quality_summary_latest.txt"
SOURCE_QUALITY_JSON="${REPORT_DIR}/source_quality_summary_latest.json"
SESSION_CONTINUITY_REPORT="${REPORT_DIR}/session_continuity_summary_latest.txt"
SESSION_CONTINUITY_JSON="${REPORT_DIR}/session_continuity_summary_latest.json"
REAL_LIVE_EVIDENCE_REPORT="${REPORT_DIR}/real_live_evidence_summary_latest.txt"
REAL_LIVE_EVIDENCE_JSON="${REPORT_DIR}/real_live_evidence_summary_latest.json"
PATH="/home/ubuntu/.cargo/bin:${PATH}"

MODE="${1:-mainline}"
ROUNDS="${PROXY_VERIFY_LONGRUN_ROUNDS:-3}"
BASE_URL="${PERSONA_PILOT_BASE_URL:-http://127.0.0.1:3000}"
LONGRUN_SAMPLES="${PROXY_VERIFY_LONGRUN_SAMPLES:-5}"
LONGRUN_INTERVAL_SECONDS="${PROXY_VERIFY_LONGRUN_INTERVAL_SECONDS:-60}"
REAL_LONGRUN_DURATION_SECONDS="${PROXY_VERIFY_REAL_DURATION_SECONDS:-1800}"
REAL_LONGRUN_HARVEST_INTERVAL_SECONDS="${PROXY_VERIFY_REAL_HARVEST_INTERVAL_SECONDS:-120}"
REAL_LONGRUN_STATUS_INTERVAL_SECONDS="${PROXY_VERIFY_REAL_STATUS_INTERVAL_SECONDS:-60}"
REAL_LONGRUN_BROWSER_INTERVAL_SECONDS="${PROXY_VERIFY_REAL_BROWSER_INTERVAL_SECONDS:-45}"
REAL_LONGRUN_BROWSER_TIMEOUT_SECONDS="${PROXY_VERIFY_REAL_BROWSER_TIMEOUT_SECONDS:-15}"
REAL_LONGRUN_BROWSER_ENDPOINT="${PROXY_VERIFY_REAL_BROWSER_ENDPOINT:-/browser/title}"
REAL_LONGRUN_STATEFUL_ENDPOINT="${PROXY_VERIFY_REAL_STATEFUL_ENDPOINT:-/browser/text}"
REAL_LONGRUN_WARM_URLS="${PROXY_VERIFY_REAL_WARM_URLS:-https://example.com}"
REAL_LONGRUN_BROWSER_REGION="${PROXY_VERIFY_REAL_BROWSER_REGION:-}"
REAL_LONGRUN_PRESET="${PROXY_VERIFY_REAL_PRESET:-legacy}"
REAL_LONGRUN_MAX_BROWSER_REGIONS="${PROXY_VERIFY_REAL_MAX_BROWSER_REGIONS:-3}"
REAL_LONGRUN_FINGERPRINT_PROFILE_ID="${PROXY_VERIFY_REAL_FINGERPRINT_PROFILE_ID:-}"
REAL_LONGRUN_GEO_ENRICH_INTERVAL_SECONDS="${PROXY_VERIFY_REAL_GEO_ENRICH_INTERVAL_SECONDS:-120}"
REAL_LONGRUN_GEO_ENRICH_LIMIT="${PROXY_VERIFY_REAL_GEO_ENRICH_LIMIT:-200}"
REAL_LONGRUN_DISABLE_GEO_ENRICH="${PROXY_VERIFY_REAL_DISABLE_GEO_ENRICH:-0}"
REAL_LONGRUN_MODE="${PROXY_VERIFY_REAL_MODE:-${PERSONA_PILOT_PROXY_MODE:-demo_public}}"
REAL_LONGRUN_CONFIG="${PROXY_VERIFY_REAL_CONFIG:-${PERSONA_PILOT_PROXY_HARVEST_CONFIG:-}}"
PROD_LIVE_MIN_SAMPLE_COUNT="${PROXY_VERIFY_PROD_LIVE_MIN_SAMPLE_COUNT:-6}"
PROD_LIVE_MIN_EFFECTIVE_RATIO_PERCENT="${PROXY_VERIFY_PROD_LIVE_MIN_EFFECTIVE_RATIO_PERCENT:-35}"
PROD_LIVE_MIN_PROMOTION_RATE_PERCENT="${PROXY_VERIFY_PROD_LIVE_MIN_PROMOTION_RATE_PERCENT:-75}"
PROD_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT="${PROXY_VERIFY_PROD_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT:-98}"
PROD_LIVE_MIN_RECENT_HOT_REGIONS="${PROXY_VERIFY_PROD_LIVE_MIN_RECENT_HOT_REGIONS:-3}"
PROD_LIVE_MAX_SOURCE_TOP1_PERCENT="${PROXY_VERIFY_PROD_LIVE_MAX_SOURCE_TOP1_PERCENT:-75}"
PROD_LIVE_MIN_GEO_COVERAGE_PERCENT="${PROXY_VERIFY_PROD_LIVE_MIN_GEO_COVERAGE_PERCENT:-0}"
REAL_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT="${PROXY_VERIFY_REAL_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT:-95}"
REAL_LIVE_MIN_BROWSER_TOTAL="${PROXY_VERIFY_REAL_LIVE_MIN_BROWSER_TOTAL:-6}"
REAL_LIVE_MIN_STATEFUL_PRIMARY_TOTAL="${PROXY_VERIFY_REAL_LIVE_MIN_STATEFUL_PRIMARY_TOTAL:-2}"
REAL_LIVE_MIN_STATEFUL_FOLLOWUP_TOTAL="${PROXY_VERIFY_REAL_LIVE_MIN_STATEFUL_FOLLOWUP_TOTAL:-1}"
REAL_LIVE_REQUIRE_ALL_STORAGE_POSITIVE="${PROXY_VERIFY_REAL_LIVE_REQUIRE_ALL_STORAGE_POSITIVE:-0}"
STATEFUL_SERVER_HOST="${PROXY_VERIFY_STATEFUL_HOST:-127.0.0.1}"
STATEFUL_SERVER_PORT="${PROXY_VERIFY_STATEFUL_PORT:-8766}"
REAL_LONGRUN_STATEFUL_URL="${PROXY_VERIFY_REAL_STATEFUL_URL:-http://${STATEFUL_SERVER_HOST}:${STATEFUL_SERVER_PORT}/stateful?slot=main}"
STATEFUL_SERVER_PID=""

eval "$(
  python3 scripts/prod_live_presets.py print-shell --preset "${REAL_LONGRUN_PRESET}"
)"

REAL_LONGRUN_STATEFUL_FOLLOWUP_COUNT="${PROXY_VERIFY_REAL_STATEFUL_FOLLOWUP_COUNT:-${PROD_LIVE_PRESET_STATEFUL_FOLLOWUP_COUNT}}"
REAL_LONGRUN_AUTO_BROWSER_REGIONS_FROM_DB="${PROXY_VERIFY_REAL_AUTO_BROWSER_REGIONS_FROM_DB:-${PROD_LIVE_PRESET_AUTO_BROWSER_REGIONS_FROM_DB}}"
REAL_LONGRUN_POOL_HYGIENE="${PROXY_VERIFY_REAL_POOL_HYGIENE:-${PROD_LIVE_PRESET_POOL_HYGIENE}}"
REAL_LONGRUN_GEO_ENRICH="${PROXY_VERIFY_REAL_GEO_ENRICH:-${PROD_LIVE_PRESET_GEO_ENRICH}}"
REAL_LONGRUN_HYGIENE_EXTRA_ARGS="${PROXY_VERIFY_REAL_HYGIENE_EXTRA_ARGS:-${PROD_LIVE_PRESET_POOL_HYGIENE_EXTRA_ARGS}}"

mkdir -p "${REPORT_DIR}"

cleanup_stateful_server() {
  if [[ -n "${STATEFUL_SERVER_PID}" ]]; then
    kill "${STATEFUL_SERVER_PID}" >/dev/null 2>&1 || true
    wait "${STATEFUL_SERVER_PID}" >/dev/null 2>&1 || true
    STATEFUL_SERVER_PID=""
  fi
}

start_stateful_server() {
  if [[ -n "${PROXY_VERIFY_REAL_STATEFUL_URL:-}" ]]; then
    return 0
  fi
  cleanup_stateful_server
  python3 scripts/identity_stateful_test_server.py \
    --host "${STATEFUL_SERVER_HOST}" \
    --port "${STATEFUL_SERVER_PORT}" \
    > "${REPORT_DIR}/identity_stateful_test_server.log" 2>&1 &
  STATEFUL_SERVER_PID="$!"
  for _ in $(seq 1 20); do
    if curl -fsS "http://${STATEFUL_SERVER_HOST}:${STATEFUL_SERVER_PORT}/healthz" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  echo "[proxy-mainline-verify] failed to start identity stateful server" >&2
  cleanup_stateful_server
  return 1
}

trap cleanup_stateful_server EXIT

run_exact_test() {
  local suite="$1"
  local test_name="$2"
  echo "== ${suite} :: ${test_name} =="
  cargo test -q --test "${suite}" "${test_name}" -- --exact --nocapture
  echo
}

run_mainline_once() {
  echo "[proxy-mainline-verify] repo=${ROOT_DIR}"
  echo "[proxy-mainline-verify] phase=no-proxy"
  run_exact_test "integration_api" "browser_task_without_active_proxy_fails_with_consistent_no_proxy_contract"

  echo "[proxy-mainline-verify] phase=candidate-promotion"
  run_exact_test "integration_api" "candidate_proxy_verify_success_promotes_to_active"
  run_exact_test "integration_api" "candidate_proxy_verify_failure_marks_candidate_rejected_and_sets_cooldown"

  echo "[proxy-mainline-verify] phase=region-routing"
  run_exact_test "integration_api" "browser_task_region_shortage_falls_back_to_other_active_region"
  run_exact_test "integration_api" "browser_task_strict_region_shortage_fails_without_fallback"
  run_exact_test "integration_api" "replenish_tick_prioritizes_hot_region_candidates"
  run_exact_test "integration_api" "replenish_tick_suppresses_duplicate_recent_region_batch"

  echo "[proxy-mainline-verify] phase=identity-session"
  run_exact_test "integration_api" "browser_task_auto_identity_session_reuses_bound_proxy"
  run_exact_test "integration_lightpanda_runner" "lightpanda_runner_auto_session_restores_and_persists_cookies"

  echo "[proxy-mainline-verify] all sections passed"
}

run_longrun() {
  echo "[proxy-mainline-verify] longrun base_url=${BASE_URL}"
  echo "[proxy-mainline-verify] longrun samples=${LONGRUN_SAMPLES} interval_seconds=${LONGRUN_INTERVAL_SECONDS}"
  python3 scripts/proxy_longrun_report.py \
    --base-url "${BASE_URL}" \
    --samples "${LONGRUN_SAMPLES}" \
    --interval-seconds "${LONGRUN_INTERVAL_SECONDS}" \
    --txt-output "${LONGRUN_REPORT}" \
    --json-output "${LONGRUN_JSON}"
  local gate_output=""
  local gate_exit=0
  if gate_output="$(python3 scripts/release_prod_live_gate.py "${LONGRUN_JSON}" \
      --min-sample-count "${PROD_LIVE_MIN_SAMPLE_COUNT}" \
      --min-effective-ratio-percent "${PROD_LIVE_MIN_EFFECTIVE_RATIO_PERCENT}" \
      --min-promotion-rate-percent "${PROD_LIVE_MIN_PROMOTION_RATE_PERCENT}" \
      --min-browser-success-rate-percent "${PROD_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT}" \
      --min-recent-hot-regions "${PROD_LIVE_MIN_RECENT_HOT_REGIONS}" \
      --max-source-top1-percent "${PROD_LIVE_MAX_SOURCE_TOP1_PERCENT}" \
      --min-geo-coverage-percent "${PROD_LIVE_MIN_GEO_COVERAGE_PERCENT}")"; then
    gate_exit=0
  else
    gate_exit=$?
  fi
  echo "[proxy-mainline-verify] longrun report written txt=${LONGRUN_REPORT} json=${LONGRUN_JSON}"
  echo "[proxy-mainline-verify] longrun source/session artifacts source_txt=${SOURCE_QUALITY_REPORT} source_json=${SOURCE_QUALITY_JSON} session_txt=${SESSION_CONTINUITY_REPORT} session_json=${SESSION_CONTINUITY_JSON}"
  echo "[proxy-mainline-verify] longrun gate ${gate_output}"
  return "${gate_exit}"
}

run_real_longrun() {
  echo "[proxy-mainline-verify] real-live base_url=${BASE_URL}"
  echo "[proxy-mainline-verify] real-live mode=${REAL_LONGRUN_MODE}"
  echo "[proxy-mainline-verify] real-live preset=${REAL_LONGRUN_PRESET}"
  echo "[proxy-mainline-verify] real-live duration_seconds=${REAL_LONGRUN_DURATION_SECONDS}"
  echo "[proxy-mainline-verify] real-live config=${REAL_LONGRUN_CONFIG:-<missing>}"
  echo "[proxy-mainline-verify] real-live stateful_url=${REAL_LONGRUN_STATEFUL_URL}"

  if [[ "${REAL_LONGRUN_MODE}" == "prod_live" && "${PROXY_VERIFY_REAL_ALLOW_DEMO:-0}" == "1" ]]; then
    echo "[proxy-mainline-verify] refusing prod_live + PROXY_VERIFY_REAL_ALLOW_DEMO=1" >&2
    return 1
  fi

  start_stateful_server

  local -a cmd=(
    python3 scripts/proxy_real_longrun_driver.py
    --base-url "${BASE_URL}"
    --mode "${REAL_LONGRUN_MODE}"
    --preset "${REAL_LONGRUN_PRESET}"
    --duration-seconds "${REAL_LONGRUN_DURATION_SECONDS}"
    --harvest-interval-seconds "${REAL_LONGRUN_HARVEST_INTERVAL_SECONDS}"
    --status-interval-seconds "${REAL_LONGRUN_STATUS_INTERVAL_SECONDS}"
    --browser-interval-seconds "${REAL_LONGRUN_BROWSER_INTERVAL_SECONDS}"
    --browser-timeout-seconds "${REAL_LONGRUN_BROWSER_TIMEOUT_SECONDS}"
    --browser-endpoint "${REAL_LONGRUN_BROWSER_ENDPOINT}"
    --stateful-url "${REAL_LONGRUN_STATEFUL_URL}"
    --stateful-endpoint "${REAL_LONGRUN_STATEFUL_ENDPOINT}"
    --stateful-followup-count "${REAL_LONGRUN_STATEFUL_FOLLOWUP_COUNT}"
    --geo-enrich-interval-seconds "${REAL_LONGRUN_GEO_ENRICH_INTERVAL_SECONDS}"
    --geo-enrich-limit "${REAL_LONGRUN_GEO_ENRICH_LIMIT}"
    --max-browser-regions "${REAL_LONGRUN_MAX_BROWSER_REGIONS}"
    --raw-output "${REAL_LONGRUN_RAW}"
    --txt-output "${REAL_LONGRUN_REPORT}"
    --json-output "${REAL_LONGRUN_JSON}"
  )

  if [[ -n "${PERSONA_PILOT_API_KEY:-}" ]]; then
    cmd+=(--api-key "${PERSONA_PILOT_API_KEY}")
  fi
  if [[ -n "${REAL_LONGRUN_CONFIG}" ]]; then
    cmd+=(--config "${REAL_LONGRUN_CONFIG}")
  fi
  if [[ -n "${REAL_LONGRUN_BROWSER_REGION}" ]]; then
    cmd+=(--browser-region "${REAL_LONGRUN_BROWSER_REGION}")
  fi
  if [[ "${REAL_LONGRUN_AUTO_BROWSER_REGIONS_FROM_DB}" == "1" ]]; then
    cmd+=(--auto-browser-regions-from-db)
  fi
  if [[ -n "${REAL_LONGRUN_FINGERPRINT_PROFILE_ID}" ]]; then
    cmd+=(--fingerprint-profile-id "${REAL_LONGRUN_FINGERPRINT_PROFILE_ID}")
  fi
  if [[ "${REAL_LONGRUN_POOL_HYGIENE}" != "1" ]]; then
    cmd+=(--disable-pool-hygiene)
  fi
  if [[ "${REAL_LONGRUN_DISABLE_GEO_ENRICH}" == "1" || "${REAL_LONGRUN_GEO_ENRICH}" != "1" ]]; then
    cmd+=(--disable-geo-enrich)
  fi
  if [[ "${PROXY_VERIFY_REAL_ALLOW_DEMO:-0}" == "1" ]]; then
    cmd+=(--allow-demo-config)
  fi

  IFS=',' read -r -a warm_urls <<< "${REAL_LONGRUN_WARM_URLS}"
  for raw_url in "${warm_urls[@]}"; do
    local url
    url="$(printf '%s' "${raw_url}" | xargs)"
    if [[ -n "${url}" ]]; then
      cmd+=(--warm-url "${url}")
    fi
  done

  PROXY_REAL_LONGRUN_HYGIENE_EXTRA_ARGS="${REAL_LONGRUN_HYGIENE_EXTRA_ARGS}" "${cmd[@]}"
  local gate_output=""
  local gate_exit=0
  if gate_output="$(python3 scripts/release_real_live_gate.py "${REAL_LONGRUN_JSON}" \
      --min-browser-success-rate-percent "${REAL_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT}" \
      --min-browser-total "${REAL_LIVE_MIN_BROWSER_TOTAL}" \
      --min-stateful-primary-total "${REAL_LIVE_MIN_STATEFUL_PRIMARY_TOTAL}" \
      --min-stateful-followup-total "${REAL_LIVE_MIN_STATEFUL_FOLLOWUP_TOTAL}" \
      $([[ "${REAL_LIVE_REQUIRE_ALL_STORAGE_POSITIVE}" == "1" ]] && printf '%s' '--require-all-storage-positive'))"; then
    gate_exit=0
  else
    gate_exit=$?
  fi
  echo "[proxy-mainline-verify] real-live report written txt=${REAL_LONGRUN_REPORT} json=${REAL_LONGRUN_JSON} raw=${REAL_LONGRUN_RAW}"
  echo "[proxy-mainline-verify] real-live source/session artifacts txt=${SOURCE_QUALITY_REPORT},${SESSION_CONTINUITY_REPORT} json=${SOURCE_QUALITY_JSON},${SESSION_CONTINUITY_JSON}"
  echo "[proxy-mainline-verify] real-live evidence artifacts txt=${REAL_LIVE_EVIDENCE_REPORT} json=${REAL_LIVE_EVIDENCE_JSON}"
  echo "[proxy-mainline-verify] real-live gate ${gate_output}"
  return "${gate_exit}"
}

run_prod_live_acceptance() {
  local acceptance_mode="prod_live"
  echo "[proxy-mainline-verify] prod-live acceptance base_url=${BASE_URL}"
  echo "[proxy-mainline-verify] prod-live acceptance preset=${REAL_LONGRUN_PRESET}"
  echo "[proxy-mainline-verify] prod-live acceptance duration_seconds=${REAL_LONGRUN_DURATION_SECONDS}"
  echo "[proxy-mainline-verify] prod-live acceptance config=${REAL_LONGRUN_CONFIG:-<missing>}"
  echo "[proxy-mainline-verify] prod-live acceptance stateful_url=${REAL_LONGRUN_STATEFUL_URL}"
  if [[ "${REAL_LONGRUN_MODE}" != "${acceptance_mode}" ]]; then
    echo "[proxy-mainline-verify] prod-live acceptance forcing mode=${acceptance_mode} (requested=${REAL_LONGRUN_MODE})"
  fi

  if [[ "${PROXY_VERIFY_REAL_ALLOW_DEMO:-0}" == "1" ]]; then
    echo "[proxy-mainline-verify] refusing prod_live + PROXY_VERIFY_REAL_ALLOW_DEMO=1" >&2
    return 1
  fi

  start_stateful_server

  local -a cmd=(
    python3 scripts/proxy_real_longrun_driver.py
    --base-url "${BASE_URL}"
    --mode "${acceptance_mode}"
    --preset "${REAL_LONGRUN_PRESET}"
    --duration-seconds "${REAL_LONGRUN_DURATION_SECONDS}"
    --harvest-interval-seconds "${REAL_LONGRUN_HARVEST_INTERVAL_SECONDS}"
    --status-interval-seconds "${REAL_LONGRUN_STATUS_INTERVAL_SECONDS}"
    --browser-interval-seconds "${REAL_LONGRUN_BROWSER_INTERVAL_SECONDS}"
    --browser-timeout-seconds "${REAL_LONGRUN_BROWSER_TIMEOUT_SECONDS}"
    --browser-endpoint "${REAL_LONGRUN_BROWSER_ENDPOINT}"
    --stateful-url "${REAL_LONGRUN_STATEFUL_URL}"
    --stateful-endpoint "${REAL_LONGRUN_STATEFUL_ENDPOINT}"
    --stateful-followup-count "${REAL_LONGRUN_STATEFUL_FOLLOWUP_COUNT}"
    --geo-enrich-interval-seconds "${REAL_LONGRUN_GEO_ENRICH_INTERVAL_SECONDS}"
    --geo-enrich-limit "${REAL_LONGRUN_GEO_ENRICH_LIMIT}"
    --max-browser-regions "${REAL_LONGRUN_MAX_BROWSER_REGIONS}"
    --raw-output "${REAL_LONGRUN_RAW}"
    --txt-output "${REAL_LONGRUN_REPORT}"
    --json-output "${REAL_LONGRUN_JSON}"
  )

  if [[ -n "${PERSONA_PILOT_API_KEY:-}" ]]; then
    cmd+=(--api-key "${PERSONA_PILOT_API_KEY}")
  fi
  if [[ -n "${REAL_LONGRUN_CONFIG}" ]]; then
    cmd+=(--config "${REAL_LONGRUN_CONFIG}")
  fi
  if [[ -n "${REAL_LONGRUN_BROWSER_REGION}" ]]; then
    cmd+=(--browser-region "${REAL_LONGRUN_BROWSER_REGION}")
  fi
  if [[ "${REAL_LONGRUN_AUTO_BROWSER_REGIONS_FROM_DB}" == "1" ]]; then
    cmd+=(--auto-browser-regions-from-db)
  fi
  if [[ -n "${REAL_LONGRUN_FINGERPRINT_PROFILE_ID}" ]]; then
    cmd+=(--fingerprint-profile-id "${REAL_LONGRUN_FINGERPRINT_PROFILE_ID}")
  fi
  if [[ "${REAL_LONGRUN_POOL_HYGIENE}" != "1" ]]; then
    cmd+=(--disable-pool-hygiene)
  fi
  if [[ "${REAL_LONGRUN_DISABLE_GEO_ENRICH}" == "1" || "${REAL_LONGRUN_GEO_ENRICH}" != "1" ]]; then
    cmd+=(--disable-geo-enrich)
  fi

  IFS=',' read -r -a warm_urls <<< "${REAL_LONGRUN_WARM_URLS}"
  for raw_url in "${warm_urls[@]}"; do
    local url
    url="$(printf '%s' "${raw_url}" | xargs)"
    if [[ -n "${url}" ]]; then
      cmd+=(--warm-url "${url}")
    fi
  done

  PROXY_REAL_LONGRUN_HYGIENE_EXTRA_ARGS="${REAL_LONGRUN_HYGIENE_EXTRA_ARGS}" "${cmd[@]}"

  local gate_output=""
  local gate_exit=0
  if gate_output="$(python3 scripts/release_prod_live_gate.py "${REAL_LONGRUN_JSON}" \
      --min-sample-count "${PROD_LIVE_MIN_SAMPLE_COUNT}" \
      --min-effective-ratio-percent "${PROD_LIVE_MIN_EFFECTIVE_RATIO_PERCENT}" \
      --min-promotion-rate-percent "${PROD_LIVE_MIN_PROMOTION_RATE_PERCENT}" \
      --min-browser-success-rate-percent "${PROD_LIVE_MIN_BROWSER_SUCCESS_RATE_PERCENT}" \
      --min-recent-hot-regions "${PROD_LIVE_MIN_RECENT_HOT_REGIONS}" \
      --max-source-top1-percent "${PROD_LIVE_MAX_SOURCE_TOP1_PERCENT}" \
      --min-geo-coverage-percent "${PROD_LIVE_MIN_GEO_COVERAGE_PERCENT}")"; then
    gate_exit=0
  else
    gate_exit=$?
  fi
  echo "[proxy-mainline-verify] prod-live acceptance report written txt=${REAL_LONGRUN_REPORT} json=${REAL_LONGRUN_JSON} raw=${REAL_LONGRUN_RAW}"
  echo "[proxy-mainline-verify] prod-live acceptance source/session artifacts txt=${SOURCE_QUALITY_REPORT},${SESSION_CONTINUITY_REPORT} json=${SOURCE_QUALITY_JSON},${SESSION_CONTINUITY_JSON}"
  echo "[proxy-mainline-verify] prod-live acceptance evidence artifacts txt=${REAL_LIVE_EVIDENCE_REPORT} json=${REAL_LIVE_EVIDENCE_JSON}"
  echo "[proxy-mainline-verify] prod-live acceptance gate ${gate_output}"
  return "${gate_exit}"
}

cd "${ROOT_DIR}"

case "${MODE}" in
  mainline)
    run_mainline_once | tee "${MAIN_REPORT}"
    ;;
  longrun)
    run_longrun | tee "${LONGRUN_REPORT}"
    ;;
  real-live)
    run_real_longrun | tee "${REAL_LONGRUN_REPORT}"
    ;;
  prod-live)
    run_prod_live_acceptance | tee "${REAL_LONGRUN_REPORT}"
    ;;
  all)
    run_mainline_once | tee "${MAIN_REPORT}"
    run_longrun | tee "${LONGRUN_REPORT}"
    ;;
  *)
    echo "usage: bash scripts/proxy_mainline_verify.sh [mainline|longrun|real-live|prod-live|all]" >&2
    exit 1
    ;;
esac
