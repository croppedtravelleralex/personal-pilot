#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${PERSONA_PILOT_BASE_URL:-http://127.0.0.1:3000}"
API_KEY="${PERSONA_PILOT_API_KEY:-}"
RUNNER_KIND="${PERSONA_PILOT_RUNNER:-fake}"
POLL_INTERVAL="${SMOKE_POLL_INTERVAL:-1}"
POLL_MAX="${SMOKE_POLL_MAX:-20}"
TASK_KIND="${SMOKE_TASK_KIND:-open_page}"
TEST_URL="${SMOKE_TEST_URL:-https://example.com}"
TIMEOUT_SECONDS="${SMOKE_TIMEOUT_SECONDS:-5}"

if ! command -v curl >/dev/null 2>&1; then
  echo "[smoke] curl is required" >&2
  exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
  echo "[smoke] python3 is required" >&2
  exit 1
fi

json_get() {
  local key=""
  local input
  input=""
  python3 - "" "" <<PY
import json, sys
key = sys.argv[1]
data = json.loads(sys.argv[2])
value = data
for part in key.split("."):
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

echo "[smoke] base_url=$BASE_URL runner=$RUNNER_KIND"

echo "[smoke] health"
health_json="$(curl_json GET "$BASE_URL/health")"
echo "$health_json"

echo "[smoke] create task"
create_payload="$(python3 - <<PY
import json
print(json.dumps({
  'kind': '${TASK_KIND}',
  'url': '${TEST_URL}',
  'timeout_seconds': int('${TIMEOUT_SECONDS}'),
}))
PY
)"
create_json="$(curl_json POST "$BASE_URL/tasks" "$create_payload")"
echo "$create_json"
task_id="$(printf '%s' "$create_json" | json_get id)"

echo "[smoke] task_id=$task_id"

final_status=""
for i in $(seq 1 "$POLL_MAX"); do
  task_json="$(curl_json GET "$BASE_URL/tasks/$task_id")"
  status="$(printf '%s' "$task_json" | json_get status)"
  echo "[smoke] poll=$i status=$status"
  if [[ "$status" != "queued" && "$status" != "running" ]]; then
    final_status="$status"
    break
  fi
  sleep "$POLL_INTERVAL"
done

if [[ -z "$final_status" ]]; then
  echo "[smoke] task did not finish within polling window" >&2
  exit 2
fi

echo "[smoke] final_status=$final_status"

echo "[smoke] runs"
runs_json="$(curl_json GET "$BASE_URL/tasks/$task_id/runs?limit=5")"
echo "$runs_json"

echo "[smoke] logs"
logs_json="$(curl_json GET "$BASE_URL/tasks/$task_id/logs?limit=10")"
echo "$logs_json"

echo "[smoke] status summary"
status_json="$(curl_json GET "$BASE_URL/status?limit=5")"
echo "$status_json"

echo "[smoke] done"
