#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${AUTO_OPEN_BROWSER_BASE_URL:-http://127.0.0.1:3000}"
API_KEY="${AUTO_OPEN_BROWSER_API_KEY:-}"
POLL_INTERVAL="${VERIFY_POLL_INTERVAL:-1}"
POLL_MAX="${VERIFY_POLL_MAX:-20}"

if ! command -v curl >/dev/null 2>&1; then
  echo "[verify] curl is required" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "[verify] python3 is required" >&2
  exit 1
fi

json_get() {
  local key="$1"
  python3 - "$key" <<'PY'
import json, sys
key = sys.argv[1]
data = json.load(sys.stdin)
value = data
for part in key.split('.'):
    if isinstance(value, dict):
        value = value.get(part)
    else:
        value = None
        break
if value is None:
    sys.exit(2)
if isinstance(value, (dict, list)):
    print(json.dumps(value, ensure_ascii=False))
else:
    print(value)
PY
}

curl_json() {
  local method="$1"
  local url="$2"
  local body="${3:-}"
  local -a args=(-sS -X "$method" "$url" -H 'Content-Type: application/json')
  if [[ -n "$API_KEY" ]]; then
    args+=(-H "x-api-key: $API_KEY")
  fi
  if [[ -n "$body" ]]; then
    args+=(-d "$body")
  fi
  curl "${args[@]}"
}

poll_task() {
  local task_id="$1"
  local status=""
  for i in $(seq 1 "$POLL_MAX"); do
    local task_json
    task_json="$(curl_json GET "$BASE_URL/tasks/$task_id")"
    status="$(printf '%s' "$task_json" | json_get status)"
    if [[ "$status" != "queued" && "$status" != "running" ]]; then
      printf '%s' "$status"
      return 0
    fi
    sleep "$POLL_INTERVAL"
  done
  return 1
}

create_case_task() {
  local url="$1"
  local timeout_seconds="$2"
  python3 - <<PY
import json
print(json.dumps({
  "kind": "open_page",
  "url": ${url@Q},
  "timeout_seconds": int(${timeout_seconds})
}))
PY
}

run_case() {
  local name="$1"
  local url="$2"
  local expect_status="$3"
  local timeout_seconds="${4:-5}"

  echo "[verify] case=$name"
  local payload create_json task_id final_status runs_json logs_json
  payload="$(create_case_task "$url" "$timeout_seconds")"
  create_json="$(curl_json POST "$BASE_URL/tasks" "$payload")"
  echo "$create_json"
  task_id="$(printf '%s' "$create_json" | json_get id)"
  final_status="$(poll_task "$task_id")" || {
    echo "[verify] polling timed out for task_id=$task_id" >&2
    return 1
  }
  echo "[verify] task_id=$task_id final_status=$final_status"
  if [[ "$final_status" != "$expect_status" ]]; then
    echo "[verify] expected status=$expect_status got=$final_status" >&2
    return 1
  fi
  runs_json="$(curl_json GET "$BASE_URL/tasks/$task_id/runs?limit=3&offset=0")"
  logs_json="$(curl_json GET "$BASE_URL/tasks/$task_id/logs?limit=5&offset=0")"
  echo "$runs_json"
  echo "$logs_json" >/dev/null
  echo "[verify] case=$name ok"
}

echo "[verify] base_url=$BASE_URL"
echo "[verify] use with AUTO_OPEN_BROWSER_RUNNER=lightpanda"
echo "[verify] typical scenarios to run manually:"
echo "  - invalid URL -> run_case invalid-url example.com failed"
echo "  - missing binary -> set LIGHTPANDA_BIN to missing path, then run_case missing-bin https://example.com failed"
echo "  - timeout -> point LIGHTPANDA_BIN to a long-running wrapper, then run_case timeout-case https://example.com timeout 1"
echo "  - non-zero exit -> point LIGHTPANDA_BIN to a wrapper exiting non-zero, then run_case non-zero https://example.com failed"
