# 03 Roadmap
Updated: 2026-04-16 (Asia/Shanghai)

## Unified Progress Truth

- mainline delivery: `95% / 7% / green`
- overall end-state: `30% / 70% / yellow`

## Phase A: Mainline Closeout

This phase is the current shipping track. It is not the same thing as the long-term “complete app” target.

### Current blockers

1. `Proxy / IP`: move from provider-aware local closure to true provider API rotation.
2. `Synchronizer`: move from live read/focus to native `set main / layout / broadcast`.
3. `Recorder / Templates`: move from desktop step-write to deeper native capture / template closure.

### Exit condition

- keep the current Win11 desktop shell stable
- finish the remaining native-closeout slice without reopening architecture scope
- preserve `src/services/desktop.ts` as the only invoke boundary

### Detailed stage board

Use `docs/19-phase-plan-and-scorecard.md` as the canonical detailed execution board for:

1. `A1 Proxy / IP closeout`
2. `A2 Synchronizer native closure`
3. `A3 Recorder / Templates native closure`
4. `A4 Mainline release gate`

## Phase B: Overall End-State Expansion

This phase covers the broader target the user keeps asking about and is the reason the overall end-state is still `30% / 70%`.

### Capability tracks

1. `Validation board`: detector, leak, transport, and coherence evidence across fingerprint / proxy / runtime layers.
2. `Fingerprint runtime depth`: move beyond the current `12` projected fields and establish applied vs observed coverage.
3. `Session bundle`: stabilize profile groups, import/export, cookie-storage continuity metadata, and long-session portability.
4. `450+ fingerprint signals`: grow total observation coverage without turning the product into `450` random knobs.
5. `450+ event taxonomy`: grow from the current `13` shipped primitives into a composable replayable event grammar.
6. `AdsPower boundary refresh`: re-evaluate realism, headed runtime depth, proxy ecosystem, and automation breadth after each major expansion.
7. `External integration plan`: land high-ROI assets from the external research set without breaking the Win11 / Tauri baseline.

### Detailed stage board

Use `docs/19-phase-plan-and-scorecard.md` as the canonical detailed execution board for:

1. `B1 Validation foundation`
2. `B2 Fingerprint model and runtime depth`
3. `B3 Session / Proxy orchestration`
4. `B4 Event grammar and automation expansion`
5. `B5 Runtime adapter and external integration`
6. `B6 AdsPower boundary refresh`

## Re-Verify Rule

Before claiming further mainline progress again, re-pass:

1. `cargo test --quiet`
2. `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`
3. `pnpm desktop:release`
