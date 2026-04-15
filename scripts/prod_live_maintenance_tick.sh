#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

PRESET="${PROXY_VERIFY_REAL_PRESET:-legacy}"
MODE="${PERSONA_PILOT_PROXY_MODE:-prod_live}"
DB_PATH="${PROXY_VERIFY_REAL_DB:-${ROOT_DIR}/data/persona_pilot.db}"
CONFIG_PATH="${PROXY_VERIFY_REAL_CONFIG:-${PERSONA_PILOT_PROXY_HARVEST_CONFIG:-}}"
GEO_ENRICH_LIMIT="${PROXY_VERIFY_REAL_GEO_ENRICH_LIMIT:-200}"

usage() {
  cat <<'EOF'
prod_live_maintenance_tick.sh

Usage:
  bash scripts/prod_live_maintenance_tick.sh --preset stable_v1

Options:
  --preset <name>   prod_live preset: legacy | stable_v1
  --mode <name>     runtime mode, defaults to prod_live
  --db <path>       sqlite db path
  --config <path>   proxy source config path
  --help            print this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --preset)
      PRESET="${2:-}"
      shift 2
      ;;
    --mode)
      MODE="${2:-}"
      shift 2
      ;;
    --db)
      DB_PATH="${2:-}"
      shift 2
      ;;
    --config)
      CONFIG_PATH="${2:-}"
      shift 2
      ;;
    -h|--help|help)
      usage
      exit 0
      ;;
    *)
      echo "[prod-live-maintenance] unknown arg: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -z "${CONFIG_PATH}" ]]; then
  echo "[prod-live-maintenance] missing config path: set --config or PERSONA_PILOT_PROXY_HARVEST_CONFIG" >&2
  exit 1
fi

eval "$(
  python3 scripts/prod_live_presets.py print-shell --preset "${PRESET}"
)"

STATEFUL_FOLLOWUP_COUNT="${PROXY_VERIFY_REAL_STATEFUL_FOLLOWUP_COUNT:-${PROD_LIVE_PRESET_STATEFUL_FOLLOWUP_COUNT}}"
AUTO_BROWSER_REGIONS_FROM_DB="${PROXY_VERIFY_REAL_AUTO_BROWSER_REGIONS_FROM_DB:-${PROD_LIVE_PRESET_AUTO_BROWSER_REGIONS_FROM_DB}}"
POOL_HYGIENE_ENABLED="${PROXY_VERIFY_REAL_POOL_HYGIENE:-${PROD_LIVE_PRESET_POOL_HYGIENE}}"
GEO_ENRICH_ENABLED="${PROXY_VERIFY_REAL_GEO_ENRICH:-${PROD_LIVE_PRESET_GEO_ENRICH}}"
POOL_HYGIENE_EXTRA_ARGS="${PROXY_VERIFY_REAL_HYGIENE_EXTRA_ARGS:-${PROD_LIVE_PRESET_POOL_HYGIENE_EXTRA_ARGS}}"

echo "[prod-live-maintenance] preset=${PRESET} mode=${MODE} db=${DB_PATH}"
echo "[prod-live-maintenance] config=${CONFIG_PATH}"
echo "[prod-live-maintenance] stateful_followup_count=${STATEFUL_FOLLOWUP_COUNT} auto_browser_regions_from_db=${AUTO_BROWSER_REGIONS_FROM_DB}"
echo "[prod-live-maintenance] pool_hygiene=${POOL_HYGIENE_ENABLED} geo_enrich=${GEO_ENRICH_ENABLED}"
echo "[prod-live-maintenance] pool_hygiene_extra_args=${POOL_HYGIENE_EXTRA_ARGS:-<none>}"

if [[ "${GEO_ENRICH_ENABLED}" == "1" ]]; then
  python3 scripts/prod_proxy_geo_enrich.py \
    --db "${DB_PATH}" \
    --mode "${MODE}" \
    --config "${CONFIG_PATH}" \
    --only-status active \
    --limit "${GEO_ENRICH_LIMIT}" \
    --apply
else
  echo "[prod-live-maintenance] skip geo_enrich (disabled)"
fi

if [[ "${POOL_HYGIENE_ENABLED}" == "1" ]]; then
  local_args=()
  if [[ -n "${POOL_HYGIENE_EXTRA_ARGS}" ]]; then
    # shellcheck disable=SC2206
    local_args=(${POOL_HYGIENE_EXTRA_ARGS})
  fi
  python3 scripts/prod_proxy_pool_hygiene.py \
    --db "${DB_PATH}" \
    --mode "${MODE}" \
    --config "${CONFIG_PATH}" \
    --apply \
    "${local_args[@]}"
else
  echo "[prod-live-maintenance] skip pool_hygiene (disabled)"
fi
