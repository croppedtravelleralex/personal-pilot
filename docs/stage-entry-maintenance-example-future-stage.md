# Stage Entry Maintenance Example For A Future Stage

## Example Scenario

A future stage reopens or closes a line because the runtime facts actually changed.

## Correct Maintenance Order

1. update `/docs/02-current-state.md` to reflect the new runtime truth
2. update `/docs/03-roadmap.md` and `/docs/04-improvement-backlog.md` if the active path changed
3. update `/TODO.md` so only still-live next actions remain
4. run:

```bash
python3 scripts/check_stage_entry_consistency.py
```

5. update root entrypoints only if the mapping or reporting rule changed
6. rerun the script before commit

## What Not To Do

- do not update root entrypoints first
- do not reopen a frozen line just because it feels like the next logical thing
- do not let `/TODO.md` keep already-landed items after the stage switches

## One-Sentence Rule

> Canonical doc maintenance is a controlled sequence, not a free-form root edit.
