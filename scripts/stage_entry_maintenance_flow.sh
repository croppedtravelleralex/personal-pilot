#!/usr/bin/env bash
set -euo pipefail

python3 scripts/check_stage_entry_consistency.py

echo
echo "Next maintenance order:"
echo "1. Update docs/02-current-state.md first"
echo "2. Update docs/03-roadmap.md and docs/04-improvement-backlog.md as needed"
echo "3. Update TODO.md if live priorities changed"
echo "4. Re-run python3 scripts/check_stage_entry_consistency.py"
echo "5. Refresh root entrypoints only if the mapping or reporting rule changed"
echo "6. Re-run python3 scripts/check_stage_entry_consistency.py before commit"
