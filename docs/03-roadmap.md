# 03 Roadmap
Updated: 2026-04-17 (Asia/Shanghai)

## Unified Progress Truth

- mainline delivery: `100% / 0% / green`
- overall end-state: `35% / 65% / yellow`

## Phase A: Mainline Closeout Completed

This phase is complete for the current Win11 desktop shipping scope.
It is not the same thing as the broader complete-app target.

### What Closed

1. `Proxy / IP`: the shipped path now uses the real provider-refresh contract and surfaces truthful accepted-vs-failed write semantics.
2. `Synchronizer`: the operator surface now matches the native intent/state-write contract instead of overstating physical execution.
3. `Recorder / Templates`: release-default flow is now native-first, with fallback limited to command-not-ready cases.
4. `Release gate`: typecheck, build, Rust tests, Win11 baseline enforcement, local verify, and desktop release all passed together.

### Completion Guardrail

- keep the current Win11 desktop shell stable
- preserve `src/services/desktop.ts` as the only invoke boundary
- if future work reopens proxy / synchronizer / recorder / release-gate regressions, reopen Axis A explicitly instead of silently spending Axis B budget

### Historical Execution Pack

- `docs/20-wave-2a-execution-plan.md` is now the completed historical pack for the six-worker round that moved mainline from `95% / 7%` to `100% / 0%`

## Phase B: Overall End-State Expansion

This is now the active phase and is the reason the product is still only `35% / 65%` against the full target.

### Capability Tracks

1. `Validation board`: detector, leak, transport, and coherence evidence across fingerprint / proxy / runtime layers.
2. `Fingerprint runtime depth`: move beyond the current `12` projected fields and establish applied vs observed coverage.
3. `Session bundle`: stabilize profile groups, import / export, cookie-storage continuity metadata, and long-session portability.
4. `450+ fingerprint signals`: grow total observation coverage without turning the product into `450` random knobs.
5. `450+ event taxonomy`: grow from the current `13` shipped primitives into a composable replayable event grammar.
6. `AdsPower boundary refresh`: re-evaluate realism, headed runtime depth, proxy ecosystem, and automation breadth after deeper evidence lands.
7. `External integration plan`: land high-ROI assets from the external research set without breaking the Win11 / Tauri baseline.

### Detailed Stage Board

Use `docs/19-phase-plan-and-scorecard.md` as the canonical detailed execution board for:

1. `B1 Validation foundation`
2. `B2 Fingerprint model and runtime depth`
3. `B3 Session / Proxy orchestration`
4. `B4 Event grammar and automation expansion`
5. `B5 Runtime adapter and external integration`
6. `B6 AdsPower boundary refresh`

## Execution Waves And Staffing

Use the following wave order by default:

| Wave | Scope | Default active agents | Status / exit gate |
| --- | --- | ---: | --- |
| `Wave 1` | `A1 + A2` | `3-4` | completed |
| `Wave 2` | `A3 + A4` | `2-4` | completed |
| `Wave 3` | `B1 + B2` | `4-6` | next active wave; validation evidence is repeatable and fingerprint runtime depth is measurable beyond `12` projected fields |
| `Wave 4` | `B3 + B4` | `4-6` | session portability and event grammar both move out of concept stage |
| `Wave 5` | `B5 + B6` | `3-6` | runtime adapter boundary is stable and the AdsPower refresh is evidence-based |

Execution notes:

- `docs/19-phase-plan-and-scorecard.md` remains the canonical source for stage execution stack, task packages, task volume, and suggested agent split
- `docs/04-improvement-backlog.md` must carry a ledger item for every active stage in this roadmap
- do not start `Wave 4` before `B1` evidence collection and `B2` runtime-depth baselines exist
- do not refresh AdsPower parity before `Wave 5`

## Re-Verify Rule

Before claiming another upward move in live progress, re-pass the relevant gate for the changed scope and keep the Win11 baseline intact.
For any mainline regression claim, the default closure gate remains:

1. `cargo test --quiet`
2. `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1`
3. `pnpm desktop:release`
