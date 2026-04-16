# 04 Improvement Backlog
Updated: 2026-04-16 (Asia/Shanghai)

## Current Truth Reminder

- mainline delivery baseline: `95% / 7% / green`
- overall end-state baseline: `30% / 70% / yellow`

## Stage Mapping Rule

Use `docs/19-phase-plan-and-scorecard.md` as the canonical phase and scorecard document.
This backlog remains the task ledger beneath that stage board.

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
- Target: push the stable local `changeProxyIp` contract down to true provider-side rotation and residency semantics

### B-005 Synchronizer Deep Native Commands

- Status: in progress
- Stage: `A2`
- Dependencies: existing live snapshot + native focus path
- Acceptance evidence: native `set main / layout / broadcast`
- Report dimension: `automation/RPA`
- Current result: live snapshot and native focus are landed
- Next: push `set main / layout / broadcast` deeper into native batch / broadcast commands

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
