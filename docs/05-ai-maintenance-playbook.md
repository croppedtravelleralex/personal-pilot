# 05 AI Maintenance Playbook
Updated: 2026-04-16 (Asia/Shanghai)

## Default Truth

- mainline delivery truth: `95% / 7% / green`
- overall end-state truth: `30% / 70% / yellow`
- primary acceptance entry: `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`

## Reporting Default

- ask about current shipped app / current closeout / native mainline: report `95% / 7% / green` first
- ask about complete app / final target / AdsPower catch-up / `50+` fingerprint control / `450+` fingerprint or event coverage: report `30% / 70% / yellow` first
- always keep these facts aligned:
  - `80` declared core control fields do not mean `80` runtime-applied fields
  - current runtime projection is still `12` env-backed fields
  - current behavior runtime is still `13` primitives
  - cookie / localStorage / sessionStorage continuity across app restart is already landed
- never describe `450+` event types or AdsPower-grade realism as already shipped

## Detailed Report Canonical Entry

When the user asks for a detailed phase plan, a full-app benchmark report, or a score-based compare, use:

- `docs/19-phase-plan-and-scorecard.md`

That canonical report must cover:

1. implemented
2. not implemented
3. final target
4. mainline progress
5. overall end-state progress
6. fingerprint quantity
7. runtime-projected signals
8. event taxonomy
9. session continuity
10. proxy / IP
11. automation / RPA
12. AdsPower comparison

## Stage Execution Canonical Rule

When the user asks for current -> complete planning, execution slicing, or stage-by-stage delivery staffing, use:

- `docs/19-phase-plan-and-scorecard.md`

That document is the only canonical source for:

1. stage execution stack
2. workload unit
3. recommended execution waves
4. per-stage task packages
5. per-stage task volume
6. default agent mix and active-agent range

`docs/03-roadmap.md`, `docs/04-improvement-backlog.md`, and root entrypoints may summarize or route to it, but must not carry a competing stage plan.

`docs/20-wave-2a-execution-plan.md` may freeze one bounded execution round, but it must not replace the canonical wave board or the canonical mainline release gate.

## Score Source Rule

Keep progress and capability score separate:

- progress truth comes from `docs/02-current-state.md` + `docs/final-goal-progress-breakdown.md`
- capability score comes from landed code paths and the canonical scorecard in `docs/19-phase-plan-and-scorecard.md`
- AdsPower comparison must use official public sources only
- if a competitor public count is not disclosed, mark it as `undisclosed` instead of inventing a number
- counts must default to:
  - fingerprint: `declared / runtime / target`
  - event taxonomy: `shipped / target`
  - continuity: `restart continuity landed / portability not yet landed`

## Verification Order

1. `cargo test --quiet`
2. `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`
3. `pnpm desktop:release`

## Handoff Read Order

1. `01-project-charter.md`
2. `02-current-state.md`
3. `final-goal-progress-breakdown.md`
4. `03-roadmap.md`
5. `04-improvement-backlog.md`
6. `17-full-app-audit-progress-reset.md`
7. `13-adspower-deep-comparison.md` when the question is about complete app or AdsPower boundary
8. `18-external-browser-integration-plan.md` when the question is about realism, kernel depth, or external integration
9. `19-phase-plan-and-scorecard.md` when the question is about detailed phase planning, scoring, or full benchmark reporting

## Documentation Write-Back Rule

When the reporting truth changes, update in the same round:

- `docs/README.md`
- `docs/02-current-state.md`
- `docs/final-goal-progress-breakdown.md`
- `docs/03-roadmap.md`
- `docs/04-improvement-backlog.md`
- `docs/05-ai-maintenance-playbook.md`
- root entrypoints: `README.md`, `CURRENT_TASK.md`, `STATUS.md`, `PROGRESS.md`, `TODO.md`

When the stage plan changes in `docs/19-phase-plan-and-scorecard.md`, sync in the same round:

- `docs/03-roadmap.md`
- `docs/04-improvement-backlog.md`
- `docs/05-ai-maintenance-playbook.md`
- root entrypoints that route users to the detailed stage board

At minimum keep these aligned:

- stage ids: `A1-A4 / B1-B6`
- verification order for `A4`
- backlog ledger coverage for every active stage
- default agent-count guidance and stage-wave order

## Do Not Reuse

- `77% / 23%`
- `82% / 18%`
- `90%`
- `99%`
