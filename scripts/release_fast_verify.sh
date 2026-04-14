#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROFILE="public-smoke"
EXTRA_ARGS=()

usage() {
  cat <<'EOF'
release_fast_verify.sh

Usage:
  bash scripts/release_fast_verify.sh
  bash scripts/release_fast_verify.sh --profile public-smoke
  bash scripts/release_fast_verify.sh --profile prod-live
  bash scripts/release_fast_verify.sh --profile gateway-upstream
  bash scripts/release_fast_verify.sh --with-upstream

Behavior:
  Fast release verification wrapper.
  It preserves profile-aware preflight and core endpoint checks, while skipping:
  - cargo test
  - browser anomaly contract cases
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --profile)
      PROFILE="${2:-}"
      shift 2
      ;;
    --with-upstream)
      EXTRA_ARGS+=("--with-upstream")
      shift
      ;;
    -h|--help|help)
      usage
      exit 0
      ;;
    *)
      echo "[release-fast] unknown arg: $1" >&2
      usage
      exit 1
      ;;
  esac
done

bash scripts/release_baseline_verify.sh \
  --profile "$PROFILE" \
  --skip-cargo-test \
  --skip-anomalies \
  "${EXTRA_ARGS[@]}"
