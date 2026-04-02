# Dual-Entry Snapshot Cheat Sheet

## Future stage switch: minimal sequence

1. Update `STATUS.md`
2. Update `TODO.md`
3. Update `PROGRESS.md`
4. Run:
   ```bash
   python3 scripts/check_stage_entry_consistency.py
   ```
5. Update `README.md` Current Stage Snapshot
6. Update `AI.md` Current Stage Snapshot
7. Run again:
   ```bash
   python3 scripts/check_stage_entry_consistency.py
   ```
8. Commit only after pass

## One-line rule

> Source-of-truth first, dual-entry snapshot last.
