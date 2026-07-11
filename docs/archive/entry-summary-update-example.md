# Entry Summary Update Example

## Scenario

A future change updates the active mainline or progress framing.
Before touching any root summary, check:

1. `/docs/02-current-state.md` already reflects the new reality
2. `/docs/03-roadmap.md` and `/docs/04-improvement-backlog.md` already reflect the new direction
3. `/TODO.md` only contains still-live next actions
4. `/docs/root-entrypoint-map.md` still describes the correct routing

## Example Update Flow

### Step 1: update canonical surfaces first
- update `/docs/02-current-state.md`
- update `/docs/03-roadmap.md` and/or `/docs/04-improvement-backlog.md`
- update `/TODO.md`

### Step 2: run the consistency script
- `python3 scripts/check_stage_entry_consistency.py`

### Step 3: refresh root entrypoints only if needed
Only after the canonical docs are aligned.

## Rule In One Sentence

> Root summaries are compatibility surfaces, not the primary record.
