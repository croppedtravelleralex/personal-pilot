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

- current shipped app / closeout / native mainline -> `95% / 7% / green`
- complete app / AdsPower catch-up / `50+` control / `450+` fingerprint or event target -> `30% / 70% / yellow`
- current reality anchors stay fixed at `80` declared controls, `12` runtime projection fields, `13` behavior primitives, and restart continuity landed
- `450+` fingerprint signals, `450+` event taxonomy, and richer AdsPower-grade realism remain future overall-track work
- detailed phase plan, scorecard, and benchmark summary live in `/docs/19-phase-plan-and-scorecard.md`

## 2026-04-17 Mainline Delta

- landed `Tasks -> Automation` surface unification
- landed provider refresh-backed `changeProxyIp` with accepted-vs-failed write semantics
- landed recorder desktop step-write and synchronizer live read / focus / internal-state writes / broadcast intent write
- re-passed targeted `2026-04-17` A1/A2 verification: `pnpm typecheck`, `cargo check`, `cargo check --manifest-path src-tauri/Cargo.toml`, synchronizer command tests, and both `changeProxyIp` success / success-check-mismatch tests
- restored the full Rust gate to green, including `integration_api`, `integration_lightpanda_runner`, and deterministic `humanize` retry coverage
- re-verified `pnpm desktop:release` and `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`
- cleared the previous Vite chunk warning with route-level lazy loading
