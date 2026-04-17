# 20 Wave 2A Bounded Execution Pack
Updated: 2026-04-17 (Asia/Shanghai)

## Purpose

This document is the immediate bounded execution pack after the `2026-04-17` cleanup push.
It is not the canonical wave board and it does not replace the canonical mainline release gate in `docs/19-phase-plan-and-scorecard.md`.

Current truth stays unchanged until this wave lands:

- mainline delivery: `95% / 7% / green`
- overall end-state: `30% / 70% / yellow`

Wave 2A exit target:

- mainline target after landing and re-verify: `97% / 3% / green`
- overall target after landing and re-verify: `35% / 65% / yellow`

These are wave targets, not current truth.

## Status Refresh

- `W2A-P3` physical layout execution is already landed; the remaining `A2` priority slice is typed broadcast-contract alignment plus truthful operator wording.
- `W2A-P5/P6` and the `A3` bridge/surface packages remain open.
- This pack still freezes ownership and volume for the round, but live stage authority stays in `docs/19-phase-plan-and-scorecard.md`.

## Scope Freeze

This wave only works inside the already-approved desktop boundary:

1. keep `Win11 + Tauri 2 + Vite + React + TypeScript`
2. keep `src/services/desktop.ts` as the only invoke boundary
3. do not reopen headed-browser fork integration or new runtime adapters
4. do not inflate progress by counting planned `450+` fingerprint or event layers as shipped

## Decision Freeze

1. `A1` stays on the real synchronous provider refresh path for this wave.
   The goal is to make the current path more typed and operator-honest, not to redesign it into a background executor in the same round.
2. `A2` prioritizes physical `layout` execution first.
   Physical `layout` is already landed in this round; the remaining synchronizer slice is typed `broadcast` contract alignment and truthful native-intent reporting.
3. `A3` prioritizes automation bridge correctness first.
   The next step is to make the existing compile/launch/detail/manual-gate chain truthful and typed before expanding recorder depth again.

## Work Packages

| Package | Track | Goal | Write scope | Task volume | Default agents | Exit gate |
| --- | --- | --- | --- | --- | ---: | --- |
| `W2A-P1` | `A3` | repair automation bridge request/response correctness | `src/features/automation/hooks.ts` | `3-4 worker-days / 1 slice / 1 module` | `1` | launch/detail/manual-gate requests use real desktop payload shapes |
| `W2A-P2` | `A3` | align automation store + page truth with the shipped native chain | `src/features/automation/store.ts`, `src/pages/AutomationPage.tsx`, `src/components/automation/*` | `4-5 worker-days / 2 slices / 3 modules` | `1` | operator surface no longer says launch/detail are missing when commands are present |
| `W2A-P3` | `A2` | add physical layout planning + Win32 apply path | `src-tauri/src/commands.rs` | `5-7 worker-days / 2 slices / 1 module` | `1` | `grid / overlap / uniform_size` produce real window movement or explicit partial-failure reporting |
| `W2A-P4` | `A2` | consume physical-layout truth on the synchronizer operator surface | `src/features/synchronizer/store.ts`, `src/pages/SynchronizerPage.tsx`, `src/components/synchronizer/*` | `3-4 worker-days / 2 slices / 3 modules` | `1` | UI reflects physical layout applied vs prepared-only paths without wording drift |
| `W2A-P5` | `A1` | expose typed provider-refresh execution feedback | `src/desktop/mod.rs`, `src/types/desktop.ts` | `4-5 worker-days / 2 slices / 2 modules` | `1` | proxy result returns structured execution / rollback / provider-refresh metadata |
| `W2A-P6` | `A1` | surface proxy provider-write truth in the operator UI | `src/features/proxies/*`, `src/components/proxies/*` | `4-5 worker-days / 2 slices / 4 modules` | `1` | operator can distinguish accepted write, rollback signal, provider source, and request id |

## Agent Split

Use `6` active workers for this wave because it contains real write work across disjoint scopes:

1. Worker `A3-bridge`: `W2A-P1`
2. Worker `A3-surface`: `W2A-P2`
3. Worker `A2-native-layout`: `W2A-P3`
4. Worker `A2-surface`: `W2A-P4`
5. Worker `A1-backend-feedback`: `W2A-P5`
6. Worker `A1-surface-feedback`: `W2A-P6`

## Integration Order

1. merge `A3` bridge correctness first so Automation Center stops carrying false contract gaps
2. merge `A2` native layout backend before the synchronizer surface adjustments
3. merge `A1` backend feedback before proxy UI wording/detail work
4. re-run typecheck, Rust check/test, web build, and Win11 baseline after all six packages integrate

## Acceptance Gate For This Wave

This wave gate is local to the pack above. It does not replace the canonical mainline re-verify rule.

1. `pnpm typecheck`
2. `pnpm build`
3. `cargo check`
4. `cargo check --manifest-path src-tauri/Cargo.toml`
5. targeted Rust tests for any new `A1/A2` logic
6. `powershell -ExecutionPolicy Bypass -File C:\Users\Lenovo\.codex\templates\win11-tauri-vite-react-ts\scripts\enforce-win11-tauri.ps1 -ProjectRoot D:\SelfMadeTool\persona-pilot`

## Progress Rule

Do not move the live reported truth from `95% / 7%` and `30% / 70%` until:

1. at least `W2A-P1/P2/P3/P5` land together
2. the acceptance gate above passes
3. docs are updated to reflect the new boundary honestly
