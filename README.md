# PersonaPilot

This root `README.md` is a compatibility entrypoint.
Canonical maintenance docs live under `/docs`.

## Dual-Axis Reporting Rule

Use the `2026-04-16` dual-axis split everywhere:

- mainline delivery: `95% / 7% / green`
- overall end-state: `30% / 70% / yellow`
- mainline acceptance was verified by: `cargo test --quiet`, `pnpm typecheck`, `pnpm build`, Win11 baseline, `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`, `pnpm desktop:release`
- AdsPower catch-up, `50+` control, `450+` fingerprint signals, and `450+` event taxonomy belong to the overall track, not the current `7%` closeout

Do not treat legacy `77% / 23%`, `82% / 18%`, `90%`, or `99%` as the current truth source.

## Current Reality Anchors

- first-family already declares `80` core control fields
- current `Lightpanda` runtime only projects `12` env-backed fingerprint fields including derived `platform`
- current behavior runtime only ships `13` real primitives
- cookie / localStorage / sessionStorage continuity across app restarts is already landed
- external browser integration is currently a plan under `/docs/18-external-browser-integration-plan.md`, not shipped runtime depth

## Read Order

1. `/docs/README.md`
2. `/docs/02-current-state.md`
3. `/docs/final-goal-progress-breakdown.md`
4. `/docs/19-phase-plan-and-scorecard.md`
5. `/docs/03-roadmap.md`
6. `/docs/04-improvement-backlog.md`
7. `/docs/05-ai-maintenance-playbook.md`
8. `/docs/17-full-app-audit-progress-reset.md`
9. `/docs/13-adspower-deep-comparison.md`
10. `/docs/root-entrypoint-map.md`

## Mainline Landed Delta

- `Tasks` is no longer an orphan route; queue control now lives inside `Automation`
- `changeProxyIp` now carries provider / region / session / sticky residency semantics through the desktop contract
- recorder now appends steps through the desktop contract, and synchronizer now reads live desktop windows plus real native focus
- `lightpanda` now emits the canonical fingerprint runtime explain contract
- Win11 `lightpanda` timeout / non-zero-exit test stubs are native-testable, and startup cancel race coverage is in place
- the full Rust gate is green again, including `integration_api` and `integration_lightpanda_runner`
- route-level lazy loading is in place and the previous Vite chunk warning has been cleared
