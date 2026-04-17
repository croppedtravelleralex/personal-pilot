# 02 Current State
Updated: 2026-04-17 (Asia/Shanghai)

## Current Report

- mainline delivery: `100% / 0% / green`
- overall end-state: `35% / 65% / yellow`
- note: `100% / 0%` means the declared mainline closeout scope `A1 + A2 + A3 + A4` is complete; it does not mean the complete-product target is finished

## Verified Evidence

## Runtime Alive

- a live runtime, started shell, or green snapshot read does not by itself mean delivery closure
- native-live, staged, and fallback paths must stay distinguished in reporting
- current synchronizer broadcast truth is now aligned to native intent/state-write semantics; physical multi-window execution is still not a shipped dispatch engine

## Build Status

Full gate is green on `2026-04-17`:

- `pnpm typecheck`
- `pnpm build`
- `cargo test --quiet`
- `cargo test --lib -- --test-threads=1`
- `cargo test --manifest-path src-tauri/Cargo.toml commands::tests -- --nocapture`
- `cargo test change_proxy_ip -- --nocapture`
- `cargo test --test integration_continuity_control_plane -- --nocapture`
- `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1`
- `powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot D:\SelfMadeTool\persona-pilot`
- `pnpm desktop:release`

## Landed Mainline Closeout

- `Tasks -> Automation` surface unification
- provider refresh-backed `changeProxyIp` desktop contract with typed accepted-vs-failed write semantics plus surfaced provider metadata on both success and failure
- recorder / templates / automation path alignment to native-first semantics; fallback is now reserved for `desktop_command_not_ready`
- synchronizer live desktop snapshot + native focus + native `setMain` / `layout` state writes + truthful `broadcast` intent-state semantics on the operator surface
- canonical `lightpanda` fingerprint runtime explain contract retained and surfaced through the real desktop chain
- deterministic full-suite stability for proxy-mode override tests and `humanize` retry tests
- route-level lazy loading that clears the old Vite chunk warning

## Reality Boundaries For Fingerprint / Events / Continuity

- the earlier `50+` fingerprint goal should be read as a minimum schema threshold; the current first-family canonical control plane already declares `80` core control fields
- current runtime materialization is still narrow: `Lightpanda` only projects `12` env-backed fingerprint fields including derived `platform`
- `450+` fingerprint signals and `450+` event types are strategic target layers, not current shipped runtime coverage
- current behavior runtime ships `13` real primitives, not a `450+` event system
- cookie / localStorage / sessionStorage continuity is already persisted and restored across app restarts via `proxy_session_bindings`
- headed Chromium / Firefox deep runtime and AdsPower-grade realism are not landed in the current product

## Mainline Completion Boundary

The current Win11 desktop mainline is now closed for the declared closeout scope.
Do not reopen `A1-A4` unless new evidence shows a regression or the scope is expanded on purpose.

Residual notes that still matter, but do not reopen the closed `0%` slice by themselves:

1. input-validation hard errors such as `proxy_id is required` and `proxy not found` still throw instead of returning typed business-failed payloads
2. synchronizer physical multi-window dispatch is still not a fully typed dispatch-result contract
3. automation / recorder / templates passed gate verification in this round, but were not manually walked through end-to-end in the UI

## What The Overall 65% Still Covers

The overall end-state track includes the broader target that users keep asking about:

1. richer fingerprint runtime materialization beyond the current `12` projected fields
2. `450+` fingerprint total-signal observation and audit coverage
3. `450+` event taxonomy instead of the current `13` behavior primitives
4. stronger headed runtime / kernel realism and validation evidence
5. a real validation board for detector, leak, and transport checks
6. a more honest AdsPower-boundary catch-up in realism, proxy ecosystem, and automation depth
7. a reusable session bundle / portability contract on top of the already-landed restart continuity assets

## Reporting Rule From Now On

Use one unified dual-axis rule everywhere:

1. `mainline delivery`: `100% / 0% / green`
2. `overall end-state`: `35% / 65% / yellow`

When the user asks about:

- current shipped app / current closeout / native mainline
  - lead with `100% / 0%`
- complete app / final target / AdsPower catch-up / `50+` / `450+`
  - lead with `35% / 65%`

Do not reuse `95% / 7%`, `30% / 70%`, `77% / 23%`, `82% / 18%`, `90%`, or `99%` as the current truth source.

## Read Order

1. `03-roadmap.md`
2. `04-improvement-backlog.md`
3. `19-phase-plan-and-scorecard.md`
4. `final-goal-progress-breakdown.md`
5. `17-full-app-audit-progress-reset.md`
6. `src/services/desktop.ts`
7. `src-tauri/src/commands.rs`
