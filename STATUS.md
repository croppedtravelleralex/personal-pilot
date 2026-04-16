# STATUS.md

This root `STATUS.md` is a compatibility entrypoint.
Canonical status now lives in `/docs/02-current-state.md`.

## Current Truth Markers

- mainline reporting baseline is `95% / 7% / green`
- overall end-state baseline is `30% / 70% / yellow`
- `80` declared controls do not mean `80` runtime-applied fields; current runtime projection is still `12`
- current behavior runtime is still `13` primitives
- cookie / localStorage / sessionStorage restart continuity is already landed
- AdsPower catch-up, `50+`, and `450+` belong to the overall track
- detailed phase plan and scorecard live in `/docs/19-phase-plan-and-scorecard.md`
- `runtime alive` is not the same thing as delivery closure
- `mock / fallback / staged` default paths do not count as delivery closure

## Follow These Docs

1. `/docs/02-current-state.md`
2. `/docs/final-goal-progress-breakdown.md`
3. `/docs/03-roadmap.md`
4. `/docs/04-improvement-backlog.md`
5. `/docs/17-full-app-audit-progress-reset.md`
6. `/docs/13-adspower-deep-comparison.md`
7. `/docs/18-external-browser-integration-plan.md`

## 2026-04-16 Mainline Delta

- `Tasks` surface unification is complete
- `changeProxyIp` is provider-aware / sticky-aware at the local desktop contract layer
- synchronizer now has live desktop read + native focus, with unsupported writes explicitly downgraded to staged
- recorder now has desktop step-write
- the Rust gate is fully green again, including `integration_api` / `integration_lightpanda_runner`
- route-level code splitting has cleared the old Vite chunk warning
