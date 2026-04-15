#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${AUTO_OPEN_BROWSER_BASE_URL:-http://127.0.0.1:3000}"
API_KEY="${AUTO_OPEN_BROWSER_API_KEY:-}"
POLL_INTERVAL="${VERIFY_POLL_INTERVAL:-1}"
POLL_MAX="${VERIFY_POLL_MAX:-30}"
DEFAULT_URL="${VERIFY_URL:-https://example.com}"
DEFAULT_TIMEOUT_SECONDS="${VERIFY_TIMEOUT_SECONDS:-8}"
MODE="${1:-open}"

TMP_JSON_DIR="${TMPDIR:-/tmp}/lightpanda-verify"
mkdir -p "$TMP_JSON_DIR"
LAST_CREATE_JSON_PATH=''
LAST_DETAIL_JSON_PATH=''
LAST_RUNS_JSON_PATH=''
LAST_STATUS_JSON_PATH=''
LAST_STATUS_ENTRY_JSON_PATH=''

write_json_tmp() {
  local prefix="$1"
  local path
  path="$(mktemp "$TMP_JSON_DIR/${prefix}.XXXXXX.json")"
  cat > "$path"
  printf '%s' "$path"
}

json_get() {
  local key="$1"
  local payload_path="$2"
  python3 - "$key" "$payload_path" <<'PY'
import json
import sys

key = sys.argv[1]
payload_path = sys.argv[2]
raw = payload_path.lstrip()
if raw.startswith('{') or raw.startswith('['):
    value = json.loads(payload_path)
else:
    with open(payload_path, 'r', encoding='utf-8') as f:
        value = json.load(f)
for part in key.split('.'):
    if isinstance(value, list):
        value = value[int(part)]
    else:
        value = value[part]
if isinstance(value, (dict, list)):
    print(json.dumps(value, ensure_ascii=False))
else:
    print(value)
PY
}

find_status_task() {
  local task_id="$1"
  local payload_path="$2"
  python3 - "$task_id" "$payload_path" <<'PY'
import json
import sys

task_id = sys.argv[1]
payload_path = sys.argv[2]
with open(payload_path, 'r', encoding='utf-8') as f:
    payload = json.load(f)
for key in ('latest_tasks', 'latest_browser_tasks'):
    for item in payload.get(key) or []:
        if item.get('id') == task_id:
            print(json.dumps(item, ensure_ascii=False))
            raise SystemExit(0)
raise SystemExit(2)
PY
}

pretty_print() {
  local payload_path="$1"
  python3 - "$payload_path" <<'PY'
import json
import sys
with open(sys.argv[1], 'r', encoding='utf-8') as f:
    print(json.dumps(json.load(f), ensure_ascii=False, indent=2))
PY
}

assert_summary_contains() {
  local label="$1"
  local needle="$2"
  local payload_path="$3"
  python3 - "$label" "$needle" "$payload_path" <<'PY'
import json
import sys

label, needle, payload_path = sys.argv[1], sys.argv[2], sys.argv[3]
with open(payload_path, 'r', encoding='utf-8') as f:
    payload = json.load(f)
items = payload if isinstance(payload, list) else payload.get('summary_artifacts') or []
texts = []
for item in items:
    if not isinstance(item, dict):
        continue
    if 'summary_artifacts' in item and isinstance(item['summary_artifacts'], list):
        for nested in item['summary_artifacts']:
            if isinstance(nested, dict):
                texts.extend(str(nested.get(k, '')) for k in ('title', 'summary', 'key', 'source'))
    texts.extend(str(item.get(k, '')) for k in ('title', 'summary', 'key', 'source'))
blob = '\n'.join(texts)
if needle not in blob:
    print(f"[verify] assert failed: {label} missing needle={needle}", file=sys.stderr)
    raise SystemExit(1)
PY
}

assert_non_empty_json_field() {
  local label="$1"
  local key="$2"
  local payload_path="$3"
  local value
  value="$(json_get "$key" "$payload_path")"
  if [[ -z "$value" || "$value" == "null" ]]; then
    echo "[verify] assert failed: $label missing or empty (key=$key)" >&2
    exit 1
  fi
}

assert_eq() {
  local label="$1"
  local actual="$2"
  local expected="$3"
  if [[ "$actual" != "$expected" ]]; then
    echo "[verify] assert failed: $label expected=$expected actual=$actual" >&2
    exit 1
  fi
}

assert_json_field_eq() {
  local label="$1"
  local key="$2"
  local expected="$3"
  local payload_path="$4"
  local value
  value="$(json_get "$key" "$payload_path")"
  assert_eq "$label" "$value" "$expected"
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

build_payload() {
  local url="$1"
  local timeout_seconds="$2"
  python3 - "$url" "$timeout_seconds" <<'PY'
import json
import sys

print(json.dumps({
    "url": sys.argv[1],
    "timeout_seconds": int(sys.argv[2]),
}, ensure_ascii=False))
PY
}

poll_task_detail() {
  local task_id="$1"
  for _ in $(seq 1 "$POLL_MAX"); do
    local task_json
    task_json="$(curl_json GET "$BASE_URL/tasks/$task_id")"
    local status
    status="$(json_get status "$task_json")"
    if [[ "$status" != "queued" && "$status" != "running" ]]; then
      printf '%s' "$task_json"
      return 0
    fi
    sleep "$POLL_INTERVAL"
  done
  echo "[verify] polling timed out for task_id=$task_id" >&2
  exit 2
}

run_case() {
  local name="$1"
  local endpoint="$2"
  local url="$3"
  local timeout_seconds="$4"
  local expect_status="$5"
  local expect_failure_scope="${6:-}"

  echo "[verify] base_url=$BASE_URL"
  echo "[verify] endpoint=$endpoint"
  echo "[verify] runner_mainline=serve+cdp"
  echo "[verify] case=$name"

  local payload task_id detail_status run_status status_entry_status
  payload="$(build_payload "$url" "$timeout_seconds")"
  LAST_CREATE_JSON_PATH="$(curl_json POST "$BASE_URL$endpoint" "$payload" | write_json_tmp create)"
  task_id="$(json_get id "$LAST_CREATE_JSON_PATH")"
  LAST_DETAIL_JSON_PATH="$(poll_task_detail "$task_id" | write_json_tmp detail)"
  detail_status="$(json_get status "$LAST_DETAIL_JSON_PATH")"
  LAST_RUNS_JSON_PATH="$(curl_json GET "$BASE_URL/tasks/$task_id/runs?limit=5&offset=0" | write_json_tmp runs)"
  run_status="$(json_get 0.status "$LAST_RUNS_JSON_PATH")"
  LAST_STATUS_JSON_PATH="$(curl_json GET "$BASE_URL/status?limit=30&offset=0" | write_json_tmp status)"
  LAST_STATUS_ENTRY_JSON_PATH="$(find_status_task "$task_id" "$LAST_STATUS_JSON_PATH" | write_json_tmp status-entry)"
  status_entry_status="$(json_get status "$LAST_STATUS_ENTRY_JSON_PATH")"

  assert_eq 'task detail status' "$detail_status" "$expect_status"
  assert_eq 'latest run status' "$run_status" "$expect_status"
  assert_eq 'status snapshot status' "$status_entry_status" "$expect_status"

  if [[ -n "$expect_failure_scope" ]]; then
    assert_json_field_eq 'task detail failure_scope' 'failure_scope' "$expect_failure_scope" "$LAST_DETAIL_JSON_PATH"
    assert_json_field_eq 'latest run failure_scope' '0.failure_scope' "$expect_failure_scope" "$LAST_RUNS_JSON_PATH"
    assert_json_field_eq 'status snapshot failure_scope' 'failure_scope' "$expect_failure_scope" "$LAST_STATUS_ENTRY_JSON_PATH"
  fi

  echo '[verify] create response'
  pretty_print "$LAST_CREATE_JSON_PATH"
  echo '[verify] terminal task detail'
  pretty_print "$LAST_DETAIL_JSON_PATH"
  echo '[verify] latest runs'
  pretty_print "$LAST_RUNS_JSON_PATH"
  echo '[verify] matching status entry'
  pretty_print "$LAST_STATUS_ENTRY_JSON_PATH"
}

verify_open_fields() {
  assert_non_empty_json_field 'detail.title' 'title' "$LAST_DETAIL_JSON_PATH"
  assert_non_empty_json_field 'detail.final_url' 'final_url' "$LAST_DETAIL_JSON_PATH"
  assert_non_empty_json_field 'run.title' '0.title' "$LAST_RUNS_JSON_PATH"
  assert_non_empty_json_field 'run.final_url' '0.final_url' "$LAST_RUNS_JSON_PATH"
}

verify_html_fields() {
  verify_open_fields
  assert_json_field_eq 'detail.content_kind' 'content_kind' 'text/html' "$LAST_DETAIL_JSON_PATH"
  assert_json_field_eq 'detail.content_source_action' 'content_source_action' 'get_html' "$LAST_DETAIL_JSON_PATH"
  assert_json_field_eq 'detail.content_ready' 'content_ready' 'True' "$LAST_DETAIL_JSON_PATH"
  assert_non_empty_json_field 'detail.content_preview' 'content_preview' "$LAST_DETAIL_JSON_PATH"
  assert_json_field_eq 'run.content_kind' '0.content_kind' 'text/html' "$LAST_RUNS_JSON_PATH"
  assert_json_field_eq 'run.content_source_action' '0.content_source_action' 'get_html' "$LAST_RUNS_JSON_PATH"
}

verify_title_fields() {
  verify_open_fields
}

verify_final_url_fields() {
  assert_non_empty_json_field 'detail.final_url' 'final_url' "$LAST_DETAIL_JSON_PATH"
  assert_non_empty_json_field 'run.final_url' '0.final_url' "$LAST_RUNS_JSON_PATH"
}

verify_text_fields() {
  verify_open_fields
  assert_json_field_eq 'detail.content_kind' 'content_kind' 'text/plain' "$LAST_DETAIL_JSON_PATH"
  assert_json_field_eq 'detail.content_source_action' 'content_source_action' 'extract_text' "$LAST_DETAIL_JSON_PATH"
  assert_json_field_eq 'detail.content_ready' 'content_ready' 'True' "$LAST_DETAIL_JSON_PATH"
  assert_non_empty_json_field 'detail.content_preview' 'content_preview' "$LAST_DETAIL_JSON_PATH"
  assert_json_field_eq 'run.content_kind' '0.content_kind' 'text/plain' "$LAST_RUNS_JSON_PATH"
  assert_json_field_eq 'run.content_source_action' '0.content_source_action' 'extract_text' "$LAST_RUNS_JSON_PATH"
}

case "$MODE" in
  help|-h|--help)
    usage
    ;;
  open)
    run_case 'open' '/browser/open' "$DEFAULT_URL" "$DEFAULT_TIMEOUT_SECONDS" 'succeeded'
    verify_open_fields
    ;;
  html)
    run_case 'html' '/browser/html' "$DEFAULT_URL" "$DEFAULT_TIMEOUT_SECONDS" 'succeeded'
    verify_html_fields
    ;;
  title)
    run_case 'title' '/browser/title' "$DEFAULT_URL" "$DEFAULT_TIMEOUT_SECONDS" 'succeeded'
    verify_title_fields
    ;;
  final-url)
    run_case 'final-url' '/browser/final-url' "$DEFAULT_URL" "$DEFAULT_TIMEOUT_SECONDS" 'succeeded'
    verify_final_url_fields
    ;;
  text)
    run_case 'text' '/browser/text' "$DEFAULT_URL" "$DEFAULT_TIMEOUT_SECONDS" 'succeeded'
    verify_text_fields
    ;;
  missing-binary)
    run_case 'missing-binary' '/browser/open' "$DEFAULT_URL" "$DEFAULT_TIMEOUT_SECONDS" 'failed' 'runner_invocation'
    assert_summary_contains 'detail binary_not_found' 'error_kind=binary_not_found' "$LAST_DETAIL_JSON_PATH"
    assert_summary_contains 'runs binary_not_found' 'error_kind=binary_not_found' "$LAST_RUNS_JSON_PATH"
    ;;
  timeout)
    POLL_MAX="${VERIFY_TIMEOUT_POLL_MAX:-90}"
    run_case 'timeout' '/browser/open' "$DEFAULT_URL" "${VERIFY_TIMEOUT_SECONDS:-1}" 'timed_out' 'runner_timeout'
    assert_summary_contains 'detail timeout' 'error_kind=timeout' "$LAST_DETAIL_JSON_PATH"
    assert_summary_contains 'runs timeout' 'error_kind=timeout' "$LAST_RUNS_JSON_PATH"
    ;;
  non-zero)
    run_case 'non-zero' '/browser/open' "$DEFAULT_URL" "$DEFAULT_TIMEOUT_SECONDS" 'failed' 'runner_process_exit'
    assert_summary_contains 'detail runner_non_zero_exit' 'error_kind=runner_non_zero_exit' "$LAST_DETAIL_JSON_PATH"
    assert_summary_contains 'runs runner_non_zero_exit' 'error_kind=runner_non_zero_exit' "$LAST_RUNS_JSON_PATH"
    ;;
  custom)
    : "${VERIFY_CREATE_ENDPOINT:?VERIFY_CREATE_ENDPOINT is required}"
    : "${VERIFY_EXPECT_STATUS:?VERIFY_EXPECT_STATUS is required}"
    run_case \
      "${VERIFY_CASE_NAME:-custom}" \
      "$VERIFY_CREATE_ENDPOINT" \
      "${VERIFY_CASE_URL:-$DEFAULT_URL}" \
      "${VERIFY_CASE_TIMEOUT_SECONDS:-$DEFAULT_TIMEOUT_SECONDS}" \
      "$VERIFY_EXPECT_STATUS" \
      "${VERIFY_EXPECT_FAILURE_SCOPE:-}"
    ;;
  *)
    echo "[verify] unknown mode: $MODE" >&2
    usage
    exit 1
    ;;
esac

echo "[verify] case=$MODE ok"
