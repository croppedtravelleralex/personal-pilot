#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PID_FILE="${ROOT_DIR}/control-plane.pid"
OUT_FILE="${ROOT_DIR}/control-plane.out"
ERR_FILE="${ROOT_DIR}/control-plane.err"
BIN_PATH="${ROOT_DIR}/target/release/AutoOpenBrowser"
HEALTH_URL="${AUTO_OPEN_BROWSER_HEALTH_URL:-http://127.0.0.1:3000/health}"
PATH="/home/ubuntu/.cargo/bin:${PATH}"

export AUTO_OPEN_BROWSER_RUNNER="${AUTO_OPEN_BROWSER_RUNNER:-lightpanda}"
export AUTO_OPEN_BROWSER_PROXY_MODE="${AUTO_OPEN_BROWSER_PROXY_MODE:-prod_live}"
export LIGHTPANDA_BIN="${LIGHTPANDA_BIN:-/usr/local/bin/lightpanda}"

list_control_plane_pids() {
  python3 - "$ROOT_DIR" "$PID_FILE" <<'PY'
import os
import sys
from pathlib import Path

root_dir = Path(sys.argv[1]).resolve()
pid_file = Path(sys.argv[2])
seen = set()

def emit(pid: int) -> None:
    if pid > 1 and pid not in seen:
        seen.add(pid)
        print(pid)

if pid_file.exists():
    try:
        emit(int(pid_file.read_text(encoding="utf-8").strip()))
    except Exception:
        pass

for entry in os.listdir("/proc"):
    if not entry.isdigit():
        continue
    pid = int(entry)
    try:
        cwd = Path(os.readlink(f"/proc/{pid}/cwd")).resolve()
        cmdline = Path(f"/proc/{pid}/cmdline").read_bytes().replace(b"\x00", b" ").decode("utf-8", errors="ignore")
    except Exception:
        continue
    if cwd != root_dir:
        continue
    if "AutoOpenBrowser" not in cmdline or " gateway" in cmdline:
        continue
    if "rustc --crate-name AutoOpenBrowser" in cmdline or "cargo build" in cmdline:
        continue
    emit(pid)
PY
}

is_pid_running() {
  local pid="$1"
  [[ -n "${pid}" ]] && kill -0 "${pid}" >/dev/null 2>&1
}

wait_for_health() {
  local max_attempts="${1:-30}"
  for _ in $(seq 1 "${max_attempts}"); do
    if curl -fsS "${HEALTH_URL}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

build_release_if_needed() {
  if [[ "${AUTO_OPEN_BROWSER_SKIP_BUILD:-0}" == "1" ]]; then
    return 0
  fi
  echo "[run_control_plane] building release binary"
  (
    cd "${ROOT_DIR}"
    cargo build --release
  )
}

status_cmd() {
  mapfile -t pids < <(list_control_plane_pids)
  if [[ "${#pids[@]}" -eq 0 ]]; then
    echo "[run_control_plane] status=stopped"
    return 1
  fi
  echo "[run_control_plane] status=running pids=${pids[*]}"
  for pid in "${pids[@]}"; do
    if is_pid_running "${pid}"; then
      echo "[run_control_plane] pid=${pid} cmdline=$(tr '\0' ' ' </proc/${pid}/cmdline)"
    fi
  done
  if curl -fsS "${HEALTH_URL}" >/dev/null 2>&1; then
    echo "[run_control_plane] health=ok url=${HEALTH_URL}"
  else
    echo "[run_control_plane] health=unreachable url=${HEALTH_URL}"
    return 1
  fi
}

stop_cmd() {
  mapfile -t pids < <(list_control_plane_pids)
  if [[ "${#pids[@]}" -eq 0 ]]; then
    rm -f "${PID_FILE}"
    echo "[run_control_plane] nothing to stop"
    return 0
  fi
  echo "[run_control_plane] stopping pids=${pids[*]}"
  for pid in "${pids[@]}"; do
    kill "${pid}" >/dev/null 2>&1 || true
  done
  for _ in $(seq 1 20); do
    local_alive=0
    for pid in "${pids[@]}"; do
      if is_pid_running "${pid}"; then
        local_alive=1
        break
      fi
    done
    if [[ "${local_alive}" == "0" ]]; then
      rm -f "${PID_FILE}"
      echo "[run_control_plane] stopped"
      return 0
    fi
    sleep 1
  done
  echo "[run_control_plane] forcing kill for pids=${pids[*]}"
  for pid in "${pids[@]}"; do
    kill -9 "${pid}" >/dev/null 2>&1 || true
  done
  rm -f "${PID_FILE}"
}

start_cmd() {
  mapfile -t pids < <(list_control_plane_pids)
  if [[ "${#pids[@]}" -gt 0 ]]; then
    echo "[run_control_plane] already running pids=${pids[*]}"
    status_cmd
    return 0
  fi
  build_release_if_needed
  if [[ ! -x "${BIN_PATH}" ]]; then
    echo "[run_control_plane] missing binary ${BIN_PATH}" >&2
    return 1
  fi
  (
    cd "${ROOT_DIR}"
    nohup "${BIN_PATH}" >"${OUT_FILE}" 2>"${ERR_FILE}" </dev/null &
    echo $! >"${PID_FILE}"
  )
  local pid
  pid="$(cat "${PID_FILE}")"
  echo "[run_control_plane] started pid=${pid}"
  if ! wait_for_health 45; then
    echo "[run_control_plane] health check failed after start" >&2
    status_cmd || true
    return 1
  fi
  status_cmd
}

restart_cmd() {
  stop_cmd
  start_cmd
}

usage() {
  cat <<'EOF'
usage: bash scripts/run_control_plane.sh [start|stop|restart|status]
EOF
}

case "${1:-status}" in
  start)
    start_cmd
    ;;
  stop)
    stop_cmd
    ;;
  restart)
    restart_cmd
    ;;
  status)
    status_cmd
    ;;
  *)
    usage >&2
    exit 1
    ;;
esac
