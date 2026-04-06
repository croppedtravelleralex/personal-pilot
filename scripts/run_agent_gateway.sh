#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
set -a
source "$ROOT/.env.gateway"
set +a
exec "$ROOT/target/release/AutoOpenBrowser" gateway
