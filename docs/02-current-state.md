# 02 Current State
Updated: 2026-04-16 (Asia/Shanghai)

## Current Report

- mainline delivery progress: `95%`
- mainline remaining slice: `7%` (`risk-weighted closeout slice`, not a strict arithmetic complement)
- mainline verification / acceptance: `green`
- overall end-state progress: `30%`
- overall strategic gap: `70%`
- overall end-state color: `yellow`

## Verified Evidence

Passed in this round:

- `cargo test --quiet`
- `pnpm typecheck`
- `pnpm build`
- Win11 baseline enforcement
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`
- `pnpm desktop:release`

## Landed Mainline Closure

- `Tasks -> Automation` surface unification
- provider-aware / sticky-aware `changeProxyIp` desktop contract
- recorder desktop step-write
- synchronizer live desktop snapshot + native focus
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

1. `Proxy / IP`: finish provider-side API write and residency-aware rotation closure.
2. `Synchronizer`: finish native batch / broadcast writes and shrink staged default paths.
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
