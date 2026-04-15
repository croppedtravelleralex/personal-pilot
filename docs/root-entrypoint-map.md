# Root Entrypoint Map

Updated: 2026-04-15

## Purpose

The canonical maintenance narrative now lives under `/docs`.
Root markdown entrypoints stay thin and should only route readers to the canonical surfaces.

## Canonical Docs

- `/docs/README.md`
  - project-level reading order and current reporting frame
- `/docs/02-current-state.md`
  - runtime, build, and verification reality
- `/docs/03-roadmap.md`
  - active roadmap and current mainline direction
- `/docs/04-improvement-backlog.md`
  - open blockers, risks, and deferred follow-up
- `/docs/05-ai-maintenance-playbook.md`
  - handoff and maintenance workflow
- `/docs/final-goal-progress-breakdown.md`
  - final-goal progress framing and reporting rule
- `/docs/agent-alexstudio-gateway-runbook.md`
  - gateway-specific runtime and acceptance guidance

## Root Compatibility Entrypoints

- `/README.md` -> `/docs/README.md`
- `/STATUS.md` -> `/docs/02-current-state.md`
- `/AI.md` -> `/docs/05-ai-maintenance-playbook.md`
- `/PLAN.md` -> `/docs/03-roadmap.md` + `/docs/04-improvement-backlog.md`
- `/ROADMAP.md` -> `/docs/03-roadmap.md`
- `/PROGRESS.md` -> `/docs/02-current-state.md` + `/docs/final-goal-progress-breakdown.md`
- `/TODO.md` -> short live execution queue

## Secondary Root Files

Other root `*.md` files are working notes, design slices, or historical context.
They are not authoritative for current status unless a canonical `/docs` file links to them directly.

## Maintenance Rule

1. Update canonical `/docs` files first.
2. Update `/TODO.md` when live priorities change.
3. Keep root entrypoints thin; only touch them when the mapping or reporting rule changes.
4. Run `python3 scripts/check_stage_entry_consistency.py` before commit.

## Windows Rule

Only one root progress entrypoint is allowed: `/PROGRESS.md`.
Do not reintroduce a tracked `/progress.md` path, because it collides on Win11 case-insensitive clones.
