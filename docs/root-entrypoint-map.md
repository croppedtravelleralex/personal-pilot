# Root Entrypoint Map

Updated: 2026-04-17

## Purpose

The canonical maintenance narrative now lives under `/docs`.
Root markdown entrypoints stay thin and should only route readers to the canonical surfaces.

## Canonical Docs

- `/docs/README.md`
  - project-level reading order and dual-axis reporting frame
- `/docs/02-current-state.md`
  - runtime, build, verification reality, and the current `100 / 0 / green` vs `35 / 65 / yellow` truth
- `/docs/17-full-app-audit-progress-reset.md`
  - historical `77 / 23` audit-reset context; not the live source
- `/docs/03-roadmap.md`
  - active overall end-state expansion track after mainline closure
- `/docs/04-improvement-backlog.md`
  - active blockers, risks, and deferred follow-up on the overall track, plus the historical mainline ledger
- `/docs/05-ai-maintenance-playbook.md`
  - handoff workflow and reporting guardrails
- `/docs/final-goal-progress-breakdown.md`
  - canonical split for `80 / 12 / 13 / persistence / 450+` and overall end-state framing
- `/docs/19-phase-plan-and-scorecard.md`
  - canonical detailed phase plan, scorecard, and AdsPower benchmark report
- `/docs/20-wave-2a-execution-plan.md`
  - completed historical execution pack for the round that closed the current mainline
- `/docs/12-final-18-percent-delivery-plan.md`
  - historical closure board kept for legacy path/context only
- `/docs/13-adspower-deep-comparison.md`
  - AdsPower boundary comparison on the overall end-state track
- `/docs/18-external-browser-integration-plan.md`
  - external browser research and overall-track integration plan
- `/docs/agent-alexstudio-gateway-runbook.md`
  - gateway-specific runtime and acceptance guidance

## Root Compatibility Entrypoints

- `/README.md` -> `/docs/README.md`
- `/CURRENT_TASK.md` -> current root snapshot aligned to `/docs/02-current-state.md` + `/docs/19-phase-plan-and-scorecard.md`
- `/STATUS.md` -> `/docs/02-current-state.md`
- `/AI.md` -> `/docs/05-ai-maintenance-playbook.md`
- `/PLAN.md` -> `/docs/03-roadmap.md` + `/docs/04-improvement-backlog.md`
- `/ROADMAP.md` -> `/docs/03-roadmap.md`
- `/PROGRESS.md` -> `/docs/02-current-state.md` + `/docs/final-goal-progress-breakdown.md`
- `/TODO.md` -> short live execution queue

## Reporting Route

Use one dual-axis rule everywhere:

1. current shipped app / closeout / native mainline -> `100% / 0% / green`
2. complete app / AdsPower catch-up / `50+` control / `450+` fingerprint or event target -> `35% / 65% / yellow`

Use `/docs/13-adspower-deep-comparison.md` and `/docs/18-external-browser-integration-plan.md` only for the second route.
Use `/docs/19-phase-plan-and-scorecard.md` when the user asks for detailed phase planning, scoring, or a full benchmark summary.

## Secondary Root Files

Other root `*.md` files are working notes, design slices, or historical context.
They are not authoritative for current status unless a canonical `/docs` file links to them directly.

## Maintenance Rule

1. Update canonical `/docs` files first.
2. Update `/TODO.md` when live priorities change.
3. Keep root entrypoints thin; only touch them when the mapping or reporting rule changes.
4. Run `python scripts/check_stage_entry_consistency.py` before commit.

## Windows Rule

Only one root progress entrypoint is allowed: `/PROGRESS.md`.
Do not reintroduce a tracked `/progress.md` path, because it collides on Win11 case-insensitive clones.
