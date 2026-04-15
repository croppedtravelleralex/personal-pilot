# Stage Entry Consistency Script Usage

## Command

```bash
python3 scripts/check_stage_entry_consistency.py
```

## What It Checks

- required canonical docs exist under `/docs`
- root entrypoints still point at the canonical docs
- root compatibility files do not drift back to stale stage-closeout wording
- only one tracked root progress entrypoint exists: `/PROGRESS.md`

## When To Run It

- after changing `/docs/02-current-state.md`
- after changing `/docs/03-roadmap.md` or `/docs/04-improvement-backlog.md`
- after changing root compatibility entrypoints
- before committing doc-structure maintenance

## Expected Result

If the current control surface is aligned, the script ends with:

```text
Stage entry consistency: PASS
```

## Maintenance Flow

1. update canonical `/docs` surfaces first
2. update `/TODO.md` if live priorities changed
3. run the consistency script
4. refresh root entrypoints only if needed
5. rerun the script before commit

## Rule In One Sentence

> Canonical doc maintenance is validated by script, not by memory.
