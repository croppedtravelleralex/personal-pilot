#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="${1:-no-token}"
BASE_URL="${GATEWAY_VERIFY_BASE_URL:-http://127.0.0.1:8787}"
DOWNSTREAM_TOKEN="${GATEWAY_VERIFY_DOWNSTREAM_TOKEN:-alex-local-test-token}"
ADMIN_TOKEN="${GATEWAY_VERIFY_ADMIN_TOKEN:-alex-gateway-admin-local}"
CHAT_MODEL="${GATEWAY_VERIFY_MODEL:-date-now-gpt-5.4}"
CHAT_PROMPT="${GATEWAY_VERIFY_PROMPT:-Reply with the single word ok.}"
GATEWAY_BIN="${GATEWAY_VERIFY_BIN:-$ROOT/target/release/AutoOpenBrowser}"
ENV_FILE="${GATEWAY_VERIFY_ENV_FILE:-$ROOT/.env.gateway}"
GATEWAY_LOG="${GATEWAY_VERIFY_LOG:-$ROOT/gateway.verify.out}"

RESPONSE_STATUS=""
RESPONSE_BODY=""

usage() {
  cat <<'EOF'
gateway_verify.sh

Usage:
  bash scripts/gateway_verify.sh no-token
  bash scripts/gateway_verify.sh real-upstream
  bash scripts/gateway_verify.sh all

Environment:
  GATEWAY_REAL_UPSTREAM_TOKEN   optional; if empty, script tries /root/.openclaw/cliproxy-local/config.yaml
  GATEWAY_VERIFY_BASE_URL       default http://127.0.0.1:8787
  GATEWAY_VERIFY_DOWNSTREAM_TOKEN default alex-local-test-token
  GATEWAY_VERIFY_ADMIN_TOKEN    default alex-gateway-admin-local
EOF
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "[gateway-verify] missing command: $1" >&2
    exit 1
  }
}

http_json() {
  local method="$1"
  local path="$2"
  local auth_header="${3:-}"
  local body="${4:-}"
  local tmp
  tmp="$(mktemp)"

  local -a args=(-sS -o "$tmp" -w "%{http_code}" -X "$method" "$BASE_URL$path")
  if [[ -n "$auth_header" ]]; then
    args+=(-H "$auth_header")
  fi
  args+=(-H 'Content-Type: application/json')
  if [[ -n "$body" ]]; then
    args+=(-d "$body")
  fi

  if RESPONSE_STATUS="$(curl "${args[@]}")"; then
    RESPONSE_BODY="$(cat "$tmp")"
  else
    RESPONSE_STATUS="000"
    RESPONSE_BODY="$(cat "$tmp" 2>/dev/null || true)"
  fi
  rm -f "$tmp"
}

assert_status() {
  local label="$1"
  local expected="$2"
  if [[ "$RESPONSE_STATUS" != "$expected" ]]; then
    echo "[gateway-verify] $label expected status=$expected actual=$RESPONSE_STATUS" >&2
    echo "[gateway-verify] body=$RESPONSE_BODY" >&2
    exit 1
  fi
}

assert_body_contains() {
  local label="$1"
  local needle="$2"
  if [[ "$RESPONSE_BODY" != *"$needle"* ]]; then
    echo "[gateway-verify] $label body missing '$needle'" >&2
    echo "[gateway-verify] body=$RESPONSE_BODY" >&2
    exit 1
  fi
}

health_upstream_configured() {
  python3 - "$RESPONSE_BODY" <<'PY'
import json
import sys
payload = json.loads(sys.argv[1])
print(str(bool(payload.get("upstream_configured"))).lower())
PY
}

list_gateway_pids() {
  pgrep -f "AutoOpenBrowser gateway" || true
}

stop_gateway() {
  local pids
  pids="$(list_gateway_pids)"
  if [[ -z "$pids" ]]; then
    return 0
  fi
  while read -r pid; do
    [[ -z "$pid" ]] && continue
    kill "$pid" >/dev/null 2>&1 || true
  done <<< "$pids"

  for _ in $(seq 1 30); do
    if [[ -z "$(list_gateway_pids)" ]]; then
      return 0
    fi
    sleep 0.5
  done

  pids="$(list_gateway_pids)"
  while read -r pid; do
    [[ -z "$pid" ]] && continue
    kill -9 "$pid" >/dev/null 2>&1 || true
  done <<< "$pids"
}

start_gateway_with_token() {
  local upstream_token="$1"
  if [[ ! -f "$ENV_FILE" ]]; then
    echo "[gateway-verify] missing env file: $ENV_FILE" >&2
    exit 1
  fi
  if [[ ! -x "$GATEWAY_BIN" ]]; then
    echo "[gateway-verify] missing gateway binary: $GATEWAY_BIN" >&2
    exit 1
  fi

  set -a
  source "$ENV_FILE"
  set +a
  export UPSTREAM_BEARER_TOKEN="$upstream_token"

  nohup "$GATEWAY_BIN" gateway > "$GATEWAY_LOG" 2>&1 &
}

wait_gateway_health() {
  local expect_upstream="$1"
  for _ in $(seq 1 60); do
    http_json GET "/health"
    if [[ "$RESPONSE_STATUS" == "200" ]]; then
      local actual
      actual="$(health_upstream_configured)"
      if [[ "$actual" == "$expect_upstream" ]]; then
        return 0
      fi
    fi
    sleep 1
  done
  echo "[gateway-verify] health check timeout (expect upstream_configured=$expect_upstream)" >&2
  exit 1
}

build_chat_body() {
  python3 - "$CHAT_MODEL" "$CHAT_PROMPT" <<'PY'
import json
import sys
print(json.dumps({
    "model": sys.argv[1],
    "messages": [{"role": "user", "content": sys.argv[2]}],
    "max_tokens": 16,
}, ensure_ascii=False))
PY
}

resolve_real_upstream_token() {
  if [[ -n "${GATEWAY_REAL_UPSTREAM_TOKEN:-}" ]]; then
    printf '%s' "$GATEWAY_REAL_UPSTREAM_TOKEN"
    return 0
  fi
  python3 - <<'PY'
from pathlib import Path
cfg = Path('/root/.openclaw/cliproxy-local/config.yaml')
if not cfg.exists():
    raise SystemExit(2)
for raw in cfg.read_text(encoding='utf-8').splitlines():
    line = raw.strip()
    if line.startswith('- "cpa-') and line.endswith('"'):
        print(line.split('"')[1])
        raise SystemExit(0)
raise SystemExit(2)
PY
}

verify_no_token() {
  echo "[gateway-verify] verify no-token branch"

  # Force repo-owned gateway into no-token mode for deterministic verification.
  stop_gateway
  start_gateway_with_token ""
  wait_gateway_health "false"

  http_json GET "/health"
  assert_status "GET /health" "200"
  local upstream
  upstream="$(health_upstream_configured)"
  if [[ "$upstream" != "false" ]]; then
    echo "[gateway-verify] expected upstream_configured=false, got $upstream" >&2
    exit 1
  fi

  http_json GET "/v1/models" "Authorization: Bearer $DOWNSTREAM_TOKEN"
  assert_status "GET /v1/models" "200"
  assert_body_contains "GET /v1/models" "agent-proxy"

  http_json GET "/admin/stats" "Authorization: Bearer $ADMIN_TOKEN"
  assert_status "GET /admin/stats" "200"

  http_json GET "/admin/dashboard" "Authorization: Bearer $ADMIN_TOKEN"
  assert_status "GET /admin/dashboard" "307"

  http_json GET "/dashboard/"
  assert_status "GET /dashboard/" "200"

  local chat_body
  chat_body="$(build_chat_body)"
  http_json POST "/v1/chat/completions" "Authorization: Bearer $DOWNSTREAM_TOKEN" "$chat_body"
  assert_status "POST /v1/chat/completions (no-token)" "502"
  assert_body_contains "POST /v1/chat/completions (no-token)" "upstream_unavailable"

  echo "[gateway-verify] no-token branch PASS"
}

verify_real_upstream() {
  echo "[gateway-verify] verify real-upstream branch"
  local token
  token="$(resolve_real_upstream_token)"
  if [[ -z "$token" ]]; then
    echo "[gateway-verify] real-upstream token is empty" >&2
    exit 1
  fi

  stop_gateway
  start_gateway_with_token "$token"
  wait_gateway_health "true"

  local chat_body
  chat_body="$(build_chat_body)"
  http_json POST "/v1/chat/completions" "Authorization: Bearer $DOWNSTREAM_TOKEN" "$chat_body"
  assert_status "POST /v1/chat/completions (real-upstream)" "200"
  assert_body_contains "POST /v1/chat/completions (real-upstream)" "chat.completion"

  http_json GET "/v1/models" "Authorization: Bearer $DOWNSTREAM_TOKEN"
  assert_status "GET /v1/models (real-upstream)" "200"

  http_json GET "/admin/stats" "Authorization: Bearer $ADMIN_TOKEN"
  assert_status "GET /admin/stats (real-upstream)" "200"

  echo "[gateway-verify] real-upstream branch PASS"

  stop_gateway
  start_gateway_with_token ""
  wait_gateway_health "false"

  local restore_body
  restore_body="$(build_chat_body)"
  http_json POST "/v1/chat/completions" "Authorization: Bearer $DOWNSTREAM_TOKEN" "$restore_body"
  assert_status "POST /v1/chat/completions (restore no-token)" "502"
  assert_body_contains "POST /v1/chat/completions (restore no-token)" "upstream_unavailable"

  echo "[gateway-verify] restored no-token baseline PASS"
}

main() {
  need_cmd curl
  need_cmd python3

  case "$MODE" in
    no-token)
      verify_no_token
      ;;
    real-upstream)
      verify_real_upstream
      ;;
    all)
      verify_no_token
      verify_real_upstream
      ;;
    help|-h|--help)
      usage
      ;;
    *)
      echo "[gateway-verify] unknown mode: $MODE" >&2
      usage
      exit 1
      ;;
  esac

  echo "[gateway-verify] mode=$MODE ok"
}

main "$@"
