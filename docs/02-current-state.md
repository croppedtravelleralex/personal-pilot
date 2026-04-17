# 02 Current State
Updated: 2026-04-17 (Asia/Shanghai)

## Current Report

- mainline delivery: `95% / 7% / green`
- overall end-state: `30% / 70% / yellow`
- note: the `7%` remaining slice is `risk-weighted closeout scope`, not a strict arithmetic complement

## Verified Evidence

## Runtime Alive

- a live runtime, started shell, or green snapshot read does not by itself mean delivery closure
- native-live, staged, and fallback paths must stay distinguished in reporting
- current synchronizer broadcast can record native intent, but physical multi-window dispatch is still not landed

## Build Status

Full-gate baseline retained from `2026-04-16`:

- `cargo test --quiet`
- `pnpm typecheck`
- `pnpm build`
- Win11 baseline enforcement
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`
- `pnpm desktop:release`

Targeted `2026-04-17` re-verify for A1/A2:

- `pnpm typecheck`
- `cargo check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml commands::tests -- --nocapture`
- `cargo test change_proxy_ip_succeeds_with_provider_refresh_config_and_records_success_task -- --nocapture`
- `cargo test change_proxy_ip_fails_when_provider_refresh_success_check_does_not_match -- --nocapture`

## Landed Mainline Closure

- `Tasks -> Automation` surface unification
- provider refresh-backed `changeProxyIp` desktop contract with accepted-vs-failed write semantics
- recorder desktop step-write
- synchronizer live desktop snapshot + native focus + native `setMain` / `layout` state writes + capability-gated `broadcast` intent write
- canonical `lightpanda` fingerprint runtime explain contract
- Win11 `lightpanda` timeout / non-zero-exit stubs and startup cancel race coverage
- deterministic full-suite stability for proxy-mode override tests and `humanize` retry tests
- route-level lazy loading that clears the old Vite chunk warning

## Reality Boundaries For Fingerprint / Events / Continuity

- the earlier `50+` fingerprint goal should now be read as a minimum schema threshold; the current first-family canonical control plane already declares `80` core control fields
- current runtime materialization is still narrow: `Lightpanda` only projects `12` env-backed fingerprint fields including derived `platform`
- `450+` fingerprint signals and `450+` event types are strategic target layers, not current shipped runtime coverage
- current behavior runtime ships `13` real primitives, not a 450-type event system
- cookie / localStorage / sessionStorage continuity is already persisted and restored across app restarts via `proxy_session_bindings`
- headed Chromium / Firefox deep runtime and AdsPower-grade realism are not landed in the current mainline

## Remaining 7%

The unfinished closeout slice is now concentrated in three real product areas:

1. `Proxy / IP`: lock the provider refresh path with success-path proof, carrier cleanup, and a clear sync-vs-background execution decision.
2. `Synchronizer`: move from typed state/intention writes into physical layout/broadcast execution and remove the remaining prepared-vs-execute wording drift.
3. `Recorder / Templates`: finish deeper native closure and reduce remaining fallback dependence.

## What The Overall 70% Still Covers

The overall end-state track includes the broader target that users keep asking about:

1. richer fingerprint runtime materialization beyond the current `12` projected fields
2. `450+` fingerprint total-signal observation and audit coverage
3. `450+` event taxonomy instead of the current `13` behavior primitives
4. stronger headed runtime / kernel realism and validation evidence
5. a real validation board for detector, leak, and transport checks
6. a more honest AdsPower-boundary catch-up in realism, proxy ecosystem, and automation depth

## Reporting Rule From Now On

Use one unified dual-axis rule everywhere:

1. `mainline delivery`: `95% / 7% / green`
2. `overall end-state`: `30% / 70% / yellow`

When the user asks about:

- current shipped app / current closeout / native mainline
  - lead with `95% / 7%`
- complete app / final target / AdsPower catch-up / 50+ / 450+
  - lead with `30% / 70%`

Do not reuse `77% / 23%`, `82% / 18%`, `90%`, or `99%` as the current truth source.

## Read Order

1. `03-roadmap.md`
2. `04-improvement-backlog.md`
3. `19-phase-plan-and-scorecard.md`
4. `final-goal-progress-breakdown.md`
5. `17-full-app-audit-progress-reset.md`
6. `src/services/desktop.ts`
7. `src-tauri/src/commands.rs`
