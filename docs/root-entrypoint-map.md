# Root Entrypoint Map

Updated: 2026-07-09

## Purpose

The canonical maintenance narrative now lives under `/docs`.
Root markdown entrypoints stay thin and should only route readers to the canonical surfaces.

## Canonical Docs

- `/docs/README.md`
  - project-level reading order and dual-axis reporting frame
- `/docs/02-current-state.md`
  - runtime, build, verification reality, and the current `100 / 0 / green` local self-use truth
- `/docs/45-stealth-platform-handoff.md`
  - **Stealth / XHS live** handoff: commands, instance, pass/fail rules (2026-06-29)
- `/docs/44-dual-track-99plus-spec.md`
  - PGS + XBS dual-track 99+ specification
- `/docs/43-capability-scenario-test-suite.md`
  - capability radar design spec (~380 scenarios)
- `/docs/17-full-app-audit-progress-reset.md`
  - historical `77 / 23` audit-reset context; not the live source
- `/docs/03-roadmap.md`
  - current mainline closeout plus overall end-state expansion tracks
- `/docs/04-improvement-backlog.md`
  - open blockers, risks, and deferred follow-up across both axes
- `/docs/05-ai-maintenance-playbook.md`
  - handoff workflow and reporting guardrails
- `/docs/final-goal-progress-breakdown.md`
  - canonical split for `80 declared / 26 runtime / 13 shipped / 450 taxonomy seed` and overall end-state framing
- `/docs/19-phase-plan-and-scorecard.md`
  - canonical detailed phase plan, scorecard, and AdsPower benchmark report
- `/docs/12-final-18-percent-delivery-plan.md`
  - current closure board kept under the historical path
- `/docs/13-adspower-deep-comparison.md`
  - AdsPower boundary comparison on the overall end-state track
- `/docs/18-external-browser-integration-plan.md`
  - external browser research and overall-track integration plan
- `/docs/24-external-distribution-readiness.md`
  - historical-only external distribution readiness; current local-only scope has cancelled manual operator smoke and clean Win11/second-machine gates
- `/docs/release-performance-mitigation-plan.md`
  - historical/diagnostic release artifact cold start/RSS/process warning evidence; no longer a budget target or blocker
- `/docs/25-overall-remaining-work-register.md`
  - historical register for overall end-state work; local self-use scope reset 2026-06-22
- `/docs/29-proxy-supply-chain.md`
  - proxy supply chain; §5.3 Chromium launch constraints (socks5h, host-resolver-rules)
- `/docs/49-fingerprint-proxy-optimization-plan.md` … `/docs/56-boundary-closure-and-commercial-parity.md`
  - 指纹/反检测优化轨道（战术→战略→广度→相干→行为→并发→度量→边界）；统一入口 `/PLAN.md`
- `/docs/archive/README.md`
  - archived process docs and root historical stubs index
- `/docs/agent-alexstudio-gateway-runbook.md`
  - gateway-specific runtime and acceptance guidance (external; not personal-pilot mainline)

## Root Compatibility Entrypoints

- `/README.md` -> `/docs/README.md`
- `/STATUS.md` -> `/docs/02-current-state.md`
- `/AI.md` -> `/docs/05-ai-maintenance-playbook.md`
- `/PLAN.md`
  - **指纹/反检测执行枢纽**（49–56 波次、Task 速查、统一门禁）；Mainline 路线仍见 `docs/03-roadmap.md`
- `/ROADMAP.md` -> `/docs/03-roadmap.md`
- `/PROGRESS.md` -> `/docs/02-current-state.md` + `/docs/final-goal-progress-breakdown.md`
- `/TODO.md` -> short live execution queue
- `/CURRENT_TASK.md` -> `/docs/02-current-state.md` §当前下一步
- `/EXECUTION_LOG.md` -> historical rounds; **monthly canonical log**: `/docs/logs/2026/`

## Reporting Route

Use one dual-axis rule everywhere:

1. current shipped app / closeout / native mainline -> `100% / 0% / green`
2. local self-use -> `100% / 0% / green` (CAPTCHA/SMS/Email credentials smoke excepted)

Historical Overall `40% / 60% / yellow` is context only after 2026-06-22 local-only reset.

Do not use `/docs/24-external-distribution-readiness.md`, `/docs/release-performance-mitigation-plan.md`, or `/docs/sessionbundle-cross-machine-portability-runbook.md` as current gates unless the user explicitly reopens external distribution, release performance, or cross-machine scope.

## Executable And UI Rule

Only `D:\SelfMadeTool\personal-pilot\personal-pilot-tauri.exe` is a user-openable GUI entry. `src-tauri/target/` and root `target/` are temporary build outputs and may be deleted. Do not reintroduce `PersonaPilot.exe`, `portable.exe`, `persona-pilot-desktop.exe`, installer exe as a persistent entry, `gateway-ui/`, or any second UI shell. Runtime dependencies under `bin/` and browser engines under `chrome/` are not user entries.

## Secondary Root Files

Other root `*.md` files are **compatibility stubs** routing to `/docs` unless listed above.
**Archived (2026-07-09):** `LONG_TERM_ROADMAP.md`、`DESIGN_NETWORK_IDENTITY.md`、`FINGERPRINT_BOUNDARY.md` 等 → 正文在 `docs/archive/root/`，根目录仅 stub。详见 `docs/archive/README.md`。
**Removed (2026-06-29):** `summaries/cycle-*.md` → `docs/archive/README.md`。

## Maintenance Rule

1. Update canonical `/docs` files first.
2. Update `/TODO.md` when live priorities change.
3. Keep root entrypoints thin; only touch them when the mapping or reporting rule changes.
4. Stealth/platform live changes: update `docs/45-stealth-platform-handoff.md` and `docs/02-current-state.md`.
5. Run `python3 scripts/check_stage_entry_consistency.py` before commit.

## Windows Rule

Only one root progress entrypoint is allowed: `/PROGRESS.md`.
Do not reintroduce a tracked `/progress.md` path, because it collides on Win11 case-insensitive clones.
