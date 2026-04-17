# PersonaPilot progress

This root progress file is now a compatibility entrypoint.
Canonical progress and reality tracking live in:

- `/docs/02-current-state.md`
- `/docs/final-goal-progress-breakdown.md`
- `/docs/19-phase-plan-and-scorecard.md`
- `/docs/03-roadmap.md`
- `/docs/17-full-app-audit-progress-reset.md`

## Current Reporting Rule

Progress must be reported with the dual-axis rule:

- current shipped app / closeout / native mainline -> `100% / 0% / green`
- complete app / AdsPower catch-up / `50+` control / `450+` fingerprint or event target -> `35% / 65% / yellow`
- current reality anchors stay fixed at `80` declared controls, `12` runtime projection fields, `13` behavior primitives, and restart continuity landed
- `450+` fingerprint signals, `450+` event taxonomy, and richer AdsPower-grade realism remain future overall-track work
- detailed phase plan, scorecard, and benchmark summary live in `/docs/19-phase-plan-and-scorecard.md`

## 2026-04-17 Mainline Closure Delta

- landed truthful provider-refresh feedback on `changeProxyIp`
- landed native-first automation / recorder / templates alignment
- landed synchronizer contract wording that matches real native intent / state semantics
- re-passed the full closure gate: `pnpm typecheck`, `pnpm build`, `cargo test --quiet`, Win11 baseline enforcement, `scripts/windows_local_verify.ps1`, and `pnpm desktop:release`
- switched the active execution narrative from mainline closeout to overall-track expansion
