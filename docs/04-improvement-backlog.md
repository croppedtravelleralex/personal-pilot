# 04 Improvement Backlog
Updated: 2026-04-17 (Asia/Shanghai)

## Current Truth Reminder

- mainline delivery baseline: `100% / 0% / green`
- overall end-state baseline: `35% / 65% / yellow`

## Stage Mapping Rule

Use `docs/19-phase-plan-and-scorecard.md` as the canonical phase and scorecard document.
This backlog remains the task ledger beneath that stage board.
`docs/20-wave-2a-execution-plan.md` is now a completed historical execution pack.

## Historical Mainline Ledger

### B-004 Provider-Grade Proxy IP Rotation

- Status: completed for current mainline scope on `2026-04-17`
- Stage: `A1`
- Shipped result: `changeProxyIp` now follows the backend provider-refresh contract and surfaces typed accepted-vs-failed write semantics with provider metadata
- Residual risk: residency / lease / rollback / health evidence still belong to the overall track, not the closed mainline

### B-005 Synchronizer Deep Native Commands

- Status: completed for current mainline scope on `2026-04-17`
- Stage: `A2`
- Shipped result: live snapshot, native focus, native `setMain` / `layout` state writes, and truthful `broadcast` intent-state reporting are aligned on the operator surface
- Residual risk: physical multi-window broadcast execution and fully typed dispatch results still belong to the overall track

### B-011 Recorder / Templates De-Fallback

- Status: completed for current mainline scope on `2026-04-17`
- Stage: `A3`
- Shipped result: recorder / templates / automation now stay native-first and only fall back on `desktop_command_not_ready`
- Residual risk: deeper replay / debug / audit depth still belongs to the overall track

### B-008 + B-009 + B-010 + B-012 + B-014 Mainline Acceptance And Polish

- Status: completed on `2026-04-17`
- Stage: `A4`
- Shipped result: full Rust gate is green, route-level lazy loading cleared the old Vite chunk warning, `Tasks` surface unification is complete, Win11 local verify and desktop release both passed

## Active Overall-Track Ledger

### B-015 Validation Board

- Status: pending and next-wave candidate
- Stage: `B1`
- Dependencies: must not break the closed Win11 mainline
- Acceptance evidence: detector / leak / transport / coherence evidence board
- Report dimension: `fingerprint realism`
- Target: add detector, leak, DNS, WebRTC, canvas, audio, worker, and transport validation views with repeatable acceptance evidence

### B-016 Fingerprint Observation Expansion

- Status: pending and next-wave candidate
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
- Target: formalize profile groups, import / export, sticky session metadata, and restart continuity assets into a stable bundle contract

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
- Dependencies: first-pass landing of `B1-B5`
- Acceptance evidence: evidence-based benchmark refresh
- Report dimension: `AdsPower benchmark`
- Target: re-score realism, proxy ecosystem, automation depth, and runtime evidence only after the new validation and runtime-depth work lands

### B-020 Runtime Materialization Depth

- Status: pending and next-wave candidate
- Stage: `B2`
- Dependencies: validation board and canonical fingerprint model
- Acceptance evidence: runtime materialization deeper than the current `12` projected fields
- Report dimension: `runtime-projected signals`
- Target: deepen runtime materialization beyond the current `12` projected fields while preserving real family coherence and explainability

### B-021 Runtime Adapter And External Integration

- Status: pending
- Stage: `B5`
- Dependencies: `B1-B4` first-pass baselines plus the external integration plan
- Acceptance evidence: `RuntimeAdapter` boundary, adapter-aligned explain / observation reports, external asset intake proof, cross-adapter compare report
- Report dimension: `runtime adapter`
- Target: absorb high-ROI external browser strengths into stable adapter contracts and shared persona / session / report layers without pulling browser forks into the main repo
