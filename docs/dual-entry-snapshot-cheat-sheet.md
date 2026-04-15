# Dual-Entry Snapshot Cheat Sheet

## Current Rule

The old "dual-entry snapshot" workflow is retired.
Root entrypoints are now thin compatibility files and `/docs` is the source of truth.

## Minimal Sequence

1. Update `/docs/02-current-state.md`.
2. Update `/docs/03-roadmap.md` and/or `/docs/04-improvement-backlog.md` if direction changed.
3. Update `/TODO.md` if the live execution queue changed.
4. Run:
   ```bash
   python3 scripts/check_stage_entry_consistency.py
   ```
5. Touch root entrypoints only if the mapping or reporting contract changed.
6. Run the consistency script again before commit.

## One-Line Rule

> Canonical docs first, root entrypoints stay thin.
