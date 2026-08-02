# Dual-Entry Snapshot Maintenance Example

## Scenario

A future change updates the real runtime or acceptance story.
The canonical docs under `/docs` must move first, while root entrypoints remain routing surfaces.

## Correct Order

1. update `/docs/02-current-state.md`
2. update `/docs/03-roadmap.md` and `/docs/04-improvement-backlog.md` as needed
3. update `/TODO.md` if live priorities changed
4. run:

```bash
python3 scripts/check_stage_entry_consistency.py
```

5. update root entrypoints only if the doc map or reporting rule changed
6. rerun:

```bash
python3 scripts/check_stage_entry_consistency.py
```

7. commit only after the script passes again

## Rule

> Root entrypoints follow canonical docs; they do not lead them.
