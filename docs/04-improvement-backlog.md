# 04 Improvement Backlog
Updated: 2026-04-17 (Asia/Shanghai)

## Current Truth Reminder

- mainline delivery baseline: `95% / 7% / green`
- overall end-state baseline: `30% / 70% / yellow`

## Stage Mapping Rule

Use `docs/19-phase-plan-and-scorecard.md` as the canonical phase and scorecard document.
This backlog remains the task ledger beneath that stage board.

## Stage Execution Ledger Rule

- `docs/19-phase-plan-and-scorecard.md` is canonical for stage execution stack, task packages, task volume, and default agent plan
- every active stage in `A1-A4 / B1-B6` must have at least one ledger entry in this backlog
- when stage packages, task volume, or agent plan changes in `docs/19`, sync the summary table below in the same round

## Stage Volume Ledger

| Stage | Ledger item | Task packages | Task volume | Default active agents |
| --- | --- | ---: | --- | ---: |
| `A1` | `B-004` | `4` | `11-13 worker-days / 4 slices / 3 modules` | `3-4` |
| `A2` | `B-005` | `4` | `10-12 worker-days / 4 slices / 3 modules` | `3` |
| `A3` | `B-011` | `4` | `10-12 worker-days / 4 slices / 3 modules` | `3-4` |
| `A4` | `B-008 + B-009` | `4` | `7-9 worker-days / 4 slices / 2-3 modules` | `1-3` |
| `B1` | `B-015` | `5` | `14-18 worker-days / 5 slices / 4 modules` | `4-6` |
| `B2` | `B-016 + B-020` | `5` | `16-20 worker-days / 5 slices / 5 modules` | `4-5` |
| `B3` | `B-017` | `5` | `14-17 worker-days / 5 slices / 5 modules` | `4-5` |
| `B4` | `B-018` | `5` | `18-22 worker-days / 5 slices / 5 modules` | `5-6` |
| `B5` | `B-021` | `4` | `15-19 worker-days / 5 slices / 5 modules` | `4-6` |
| `B6` | `B-019` | `4` | `7-9 worker-days / 4 slices / 3 modules` | `4-6` |

## Completed In This Round

- `B-009`: `scripts/windows_local_verify.ps1` re-passed as the primary Win11 acceptance entry
- `B-010`: route-level lazy loading closed the previous Vite chunk warning
- `B-012`: `Tasks` route / surface unification is complete
- `B-014`: full Rust gate recovery is complete, including `cargo test --quiet`

## P0 Backlog

### B-004 Provider-Grade Proxy IP Rotation

- Status: in progress
- Stage: `A1`
- Dependencies: keep the existing Win11 shell and typed desktop boundary unchanged
- Acceptance evidence: provider-side write, rollback, residency semantics
- Report dimension: `proxy/IP`
- Current result: `changeProxyIp` now executes provider refresh in the desktop path and returns accepted-vs-failed write semantics
- Next: add happy-path proof, decide whether refresh stays synchronous or moves to background execution, and stop overloading `proxy_harvest_sources.config_json` as the long-term carrier
- Target: harden the now-real provider write path into a stable, test-backed, operator-honest rotation contract

### B-005 Synchronizer Deep Native Commands

- Status: in progress
- Stage: `A2`
- Dependencies: existing live snapshot + native focus path
- Acceptance evidence: native `set main / layout / broadcast`
- Report dimension: `automation/RPA`
- Current result: live snapshot, native focus, native `setMain` / `layout` state writes, and capability-gated native broadcast intent write are landed
- Next: deliver physical layout/broadcast execution depth and eliminate the remaining “prepared vs executed” wording drift

### B-011 Recorder / Templates De-Fallback

- Status: in progress
- Stage: `A3`
- Dependencies: stable native command surface for recorder / template flow
- Acceptance evidence: native-first capture / compile / replay closure
- Report dimension: `automation/RPA`
- Target: turn remaining release-default fallback paths into native-first closure

## P1 Backlog: Overall End-State Expansion

### B-015 Validation Board

- Status: pending
- Stage: `B1`
- Dependencies: must not block Axis A release gate
- Acceptance evidence: detector / leak / transport / coherence evidence board
- Report dimension: `fingerprint realism`
- Target: add detector, leak, DNS, WebRTC, canvas, audio, worker, and transport validation views with repeatable acceptance evidence

### B-016 Fingerprint Observation Expansion

- Status: pending
- Stage: `B2`
- Dependencies: validation foundation and first-family canonical model
- Acceptance evidence: broader applied / observed coverage beyond the current `12` runtime fields
- Report dimension: `fingerprint controls`
- Target: expand from the current `80` declared controls and `12` projected runtime fields toward broader applied / observed signal coverage

### B-017 Session Bundle And Profile Portability

- Status: pending
- Stage: `B3`
- Dependencies: stable fingerprint persona contract and continuity baseline
- Acceptance evidence: portable session bundle + import / export + sticky metadata
- Report dimension: `session continuity`
- Target: formalize profile groups, import/export, sticky session metadata, and restart continuity assets into a stable bundle contract

### B-018 `450+` Event Taxonomy

- Status: pending
- Stage: `B4`
- Dependencies: validation reporting and stable session / persona contract
- Acceptance evidence: replayable event grammar with audit and recovery semantics
- Report dimension: `event taxonomy`
- Target: grow the current `13` shipped primitives into a replayable `450+` event taxonomy with audit and recovery semantics

### B-019 AdsPower Boundary Refresh

- Status: pending
- Stage: `B6`
- Dependencies: first-pass landing of B1-B5
- Acceptance evidence: evidence-based benchmark refresh
- Report dimension: `AdsPower benchmark`
- Target: re-score realism, proxy ecosystem, automation depth, and runtime evidence only after the new validation and runtime-depth work lands

### B-021 Runtime Adapter And External Integration

- Status: pending
- Stage: `B5`
- Dependencies: `B1-B4` first-pass baselines plus the external integration plan
- Acceptance evidence: `RuntimeAdapter` boundary, adapter-aligned explain / observation reports, external asset intake proof, cross-adapter compare report
- Report dimension: `runtime adapter`
- Target: absorb high-ROI external browser strengths into stable adapter contracts and shared persona/session/report layers without pulling browser forks into the main repo

## P2 Backlog: Runtime And Realism

### B-008 Rust Warning Cleanup

- Status: pending if warnings reappear on fresh release verify
- Stage: `A4`
- Dependencies: close Axis A features first
- Acceptance evidence: fresh release verify without warning regressions
- Report dimension: `verification / acceptance`

### B-009 Local Win11 Release Acceptance

- Status: active and passing
- Stage: `A4`
- Dependencies: A1-A3 closed
- Acceptance evidence: `windows_local_verify.ps1` + mainline gates stay green
- Report dimension: `verification / acceptance`
- Rule: keep `windows_local_verify.ps1` as the default acceptance gate

### B-020 Runtime Materialization Depth

- Status: pending
- Stage: `B2`
- Dependencies: validation board and canonical fingerprint model
- Acceptance evidence: runtime materialization deeper than the current `12` projected fields
- Report dimension: `runtime-projected signals`
- Target: deepen runtime materialization beyond the current `12` projected fields while preserving real family coherence and explainability
