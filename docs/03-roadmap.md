# 03 Roadmap
Updated: 2026-04-17 (Asia/Shanghai)

## Unified Progress Truth

- mainline delivery: `95% / 7% / green`
- overall end-state: `30% / 70% / yellow`

## Phase A: Mainline Closeout

This phase is the current shipping track. It is not the same thing as the long-term “complete app” target.

### Current blockers

1. `Proxy / IP`: harden the real provider refresh path with success-path proof, config-carrier cleanup, and an explicit sync-vs-background execution choice.
2. `Synchronizer`: move from typed native state/intention writes to physical `layout / broadcast` execution and fully honest operator wording.
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

## Execution Waves And Staffing

Use the following wave order by default:

| Wave | Scope | Default active agents | Exit gate |
| --- | --- | ---: | --- |
| `Wave 1` | `A1 + A2` | `3-4` | proxy provider write path is honest and test-backed, and synchronizer typed writes plus operator wording are aligned with the remaining physical-execution gaps explicit |
| `Wave 2` | `A3 + A4` | `2-4` | recorder/templates are native-first and the mainline release gate is green |
| `Wave 3` | `B1 + B2` | `4-6` | validation evidence is repeatable and fingerprint runtime depth is measurable beyond `12` projected fields |
| `Wave 4` | `B3 + B4` | `4-6` | session portability and event grammar both move out of concept stage |
| `Wave 5` | `B5 + B6` | `3-6` | runtime adapter boundary is stable and the AdsPower refresh is evidence-based |

Execution notes:

- `docs/19-phase-plan-and-scorecard.md` remains the canonical source for stage execution stack, task packages, task volume, and suggested agent split
- `docs/04-improvement-backlog.md` must carry a ledger item for every active stage in this roadmap
- do not start `Wave 4` before `B1` evidence collection and `B2` runtime-depth baselines exist
- do not refresh AdsPower parity before `Wave 5`

## Re-Verify Rule

Before claiming further mainline progress again, re-pass:

1. `cargo test --quiet`
2. `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`
3. `pnpm desktop:release`
