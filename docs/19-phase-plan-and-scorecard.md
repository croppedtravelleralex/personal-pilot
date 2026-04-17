# 19 Phase Plan And Scorecard
Updated: 2026-04-17 (Asia/Shanghai)

## Canonical Role

This is the canonical detailed report for:

1. full-app implemented vs not implemented
2. detailed phase planning
3. internal capability scorecard
4. AdsPower benchmark comparison

Keep progress truth and capability score separate:

- mainline delivery: `100% / 0% / green`
- overall end-state: `35% / 65% / yellow`
- current capability score: `39 / 100`
- AdsPower public-boundary reference score: `83 / 100`

The progress split answers "how much of our declared scope is closed".
The capability score answers "how mature the product is relative to the final target and to AdsPower".

## Verified Reality Baseline

The current detailed report must stay anchored to these verified facts:

- first-family control taxonomy already declares `80` core control fields
- current `Lightpanda` runtime only projects `12` env-backed fingerprint fields including derived `platform`
- current behavior runtime only ships `13` real primitives
- cookie / localStorage / sessionStorage continuity is already persisted and restored across app restarts
- current real runner set is still `Fake + Lightpanda`; headed Chromium / Firefox deep runtime is not landed

These facts mean:

- the project is not without UI, not without program entry, and not blocked at the shell stage
- the project is also not yet at AdsPower-grade runtime realism, validation depth, or automation breadth

## Mainline Closed Scope

### Axis A Completion Summary

`A1 + A2 + A3 + A4` are complete for the declared current Win11 desktop mainline.
This closure is based on landed code plus a green full gate, not on softer wording.

### What Landed In The Closed Mainline

- `A1 Proxy / IP`: provider refresh-backed `changeProxyIp` contract now returns typed accepted-vs-failed business results with surfaced provider metadata on both success and failure
- `A2 Synchronizer`: UI wording now matches the real native intent/state-write contract instead of overstating physical multi-window execution
- `A3 Recorder / Templates`: native-first remains the default path; fallback is limited to `desktop_command_not_ready`
- `A4 Release gate`: `pnpm typecheck`, `pnpm build`, `cargo test --quiet`, Win11 baseline enforcement, `scripts/windows_local_verify.ps1`, and `pnpm desktop:release` all passed together

### Residual Risks That Stay Outside The Closed 0%

1. input-validation failures such as `proxy_id is required` and `proxy not found` still throw instead of returning typed business-failed payloads
2. synchronizer physical multi-window dispatch is still not a fully typed dispatch-result engine
3. automation / recorder / templates were not manually walked through end-to-end in the UI in this round

## Implemented And Shipped

### Product Surface

- Win11 desktop shell is landed on `Tauri 2 + Vite + React + TypeScript`
- `src/services/desktop.ts` remains the only native / invoke boundary
- Dashboard / Profiles / Proxies / Automation / Synchronizer / Logs / Settings are on the real operator surface
- `Tasks -> Automation` surface unification is landed

### Fingerprint And Runtime

- first-family `80` core control fields are declared and grouped into `8` sections
- canonical fingerprint runtime explainability and partial-consumption reporting are landed
- current `Lightpanda` runtime can consume `12` env-backed fingerprint fields
- the project already distinguishes declared controls vs runtime-projected fields

### Behavior And Automation

- behavior plan compile path is landed
- current shipped behavior layer has `13` primitives
- recorder desktop step-write is landed
- runtime explain / trace summary / seed-driven plan generation are landed

### Proxy / IP / Session

- provider refresh-backed `changeProxyIp` contract is landed with accepted-vs-failed write semantics and surfaced provider metadata
- proxy session continuity schema is landed through `proxy_session_bindings`
- cookie / localStorage / sessionStorage restore on restart is landed

### Synchronizer

- live desktop snapshot is landed
- native Win32 window focus is landed
- native `setMain` / `layout` internal-state writes are landed
- capability-gated native `broadcast` intent/state write is landed
- physical layout rearrangement and physical multi-window broadcast execution are still future work

### Research And Integration Planning

- external browser research is landed under `research/external/`
- the external integration plan is already documented and bounded to the overall track

## Not Implemented Yet

### Overall End-State Remaining `65%`

- validation board is not landed
- runtime materialization depth is still narrow at `12` projected fields
- `450+` fingerprint signal observation / audit coverage is not landed
- `450+` event taxonomy is not landed
- full session bundle / portability / import-export contract is not landed
- headed runtime realism and deeper kernel strategy are not landed
- AdsPower-grade realism catch-up is not landed
- external integration assets are planned but not yet runtime-landed

## Final Target

The final target is broader than the current closeout-ready desktop app:

1. keep the current Win11 desktop shell stable
2. keep typed service boundaries and single-instance discipline
3. maintain at least `80` real control fields as the first-family baseline
4. expand from `12` runtime-projected fields to materially deeper applied / observed runtime coverage
5. grow to `450+` total fingerprint signals across control / derived / observation layers
6. grow from `13` shipped primitives to a `450+` event taxonomy
7. turn restart continuity into a full `SessionBundle` contract
8. build a validation board with detector / leak / transport / coherence evidence
9. absorb high-ROI external browser strengths without pulling a browser fork into the main repo
10. reach or surpass AdsPower on realism, proxy coherence, automation depth, and operator tooling

## Active Stages: Axis B Remaining `65%`

### B1 Validation Foundation

Goal:

- create the evidence system that later score updates and AdsPower comparison must depend on

Detailed tasks:

1. land `validation board`
2. define `ValidationProfile`
3. define `ObservationReport`
4. add detector / leak / transport / coherence evidence collection

Acceptance:

- detector and leak checks are repeatable
- observation reports differentiate declared / applied / observed signals
- future benchmark refreshes can cite evidence rather than only design intent

### B2 Fingerprint Model And Runtime Depth

Goal:

- convert the current `80` control fields into deeper runtime depth and explainable maturity

Detailed tasks:

1. stabilize `Profile Spec`
2. deepen `Consistency Graph`
3. deepen `Runtime Policy`
4. expand current `12` projected fields into richer applied / observed coverage
5. keep control / derived / observation layers clearly separated

Acceptance:

- runtime coverage is no longer summarized only by `12` env fields
- first family emits coherence score + risk reasons + observation deltas
- declared control breadth and runtime depth are no longer conflated

### B3 Session / Proxy Orchestration

Goal:

- turn current restart continuity into a real session portability contract

Detailed tasks:

1. define `SessionBundle`
2. add profile groups / import / export contract
3. add sticky residency + geo / locale / timezone linkage
4. add proxy lease / cooldown / health / rollback semantics

Acceptance:

- restart continuity stays valid
- profile portability is no longer only local persistence
- proxy orchestration is coherent with session and fingerprint identity

### B4 Event Grammar And Automation Expansion

Goal:

- grow from `13` primitives to a `450+` event taxonomy without fake inflation

Detailed tasks:

1. define event grammar as `primitive x scene x phase x result`
2. add workflow graph
3. add replay / debug / audit semantics
4. add recovery and manual-gate semantics

Acceptance:

- event counts can be reported as real shipped taxonomy, not aspiration
- workflows are replayable and debuggable
- failure and manual takeover points are explainable

### B5 Runtime Adapter And External Integration

Goal:

- absorb high-ROI external assets without turning the repo into a browser fork host

Detailed tasks:

1. define `RuntimeAdapter`
2. integrate validation, schema, proxy, and session ideas from external research
3. keep the main repo free from direct Chromium / Firefox fork coupling
4. make the same persona contract reusable across runtime adapters

Acceptance:

- external integration lands as maintainable main-repo assets
- runtime adapter boundary is stable
- main Win11 desktop baseline remains intact

### B6 AdsPower Boundary Refresh

Goal:

- refresh the benchmark only after real evidence and deeper runtime maturity exist

Detailed tasks:

1. rescore all dimensions after `B1-B5` first-pass landing
2. compare current vs target vs AdsPower on evidence
3. update benchmark gaps and next priorities

Acceptance:

- AdsPower comparison is evidence-based
- scores are updated only when capability evidence changes
- benchmark language stops drifting with each ad hoc report

## Stage Execution Stack

| Stage | UI / TS layer | Desktop service layer | Rust / native / data layer | Validation / evidence layer |
| --- | --- | --- | --- | --- |
| `B1` Validation foundation | future `src/features/validation/*`, validation dashboards and evidence panels | typed validation commands / report reads in `src/services/desktop.ts` | detector / leak probe orchestration, report persistence, observation schema | detector, leak, DNS, WebRTC, transport, coherence evidence packs |
| `B2` Fingerprint model and runtime depth | profile editors, explain panels, runtime diff views | typed fingerprint explain / report / projection APIs | `src/network_identity/*`, `src/runner/lightpanda.rs`, `src/runner/engine.rs`, report persistence | declared vs applied vs observed delta reports |
| `B3` Session / Proxy orchestration | profile groups, import / export, portability and session-bundle screens | typed session-bundle and proxy orchestration APIs | `src/runner/engine.rs`, `src/db/schema.rs`, proxy / session lifecycle tables and serializers | portability proof, sticky residency proof, restart continuity proof |
| `B4` Event grammar and automation expansion | automation graph, replay debugger, audit timeline, manual-gate UI | typed replay / debug / audit / event graph contracts | `src/behavior/*`, `src/workflow/*`, `src/runner/lightpanda.rs`, automation data model | replay determinism, auditability, recovery-path proof |
| `B5` Runtime adapter and external integration | adapter selection UI only if needed, usually thin surface | stable adapter contracts in `src/services/desktop.ts` | `src/runner/*`, `src/network_identity/*`, `src/desktop/mod.rs`, imported external patterns | cross-adapter compare reports and integration proof |
| `B6` AdsPower boundary refresh | benchmark panels and reporting outputs | no heavy new API, mostly report aggregation | score aggregation, benchmark snapshots, doc generation | official public-source refresh + evidence-backed re-score |

## Workload Unit

Use one consistent workload unit everywhere:

- `1 worker-day` = one experienced engineer or coding worker's net implementation day
- `1 implementation slice` = one bounded worker-sized write scope that can normally be owned by one coding agent without crossing too many modules
- `1 module` = one implementation face with a stable boundary, such as `features/proxies`, `runner/lightpanda`, `src-tauri/src/commands.rs`, or a dedicated validation / report subsystem
- `1 task package` = `2-4` implementation slices under one stage outcome

Planning baseline for the active overall track:

- `Axis B` recommended budget: about `84-101 worker-days / 29 implementation slices / 27 modules`

## Recommended Execution Waves

| Wave | Scope | Recommended active agents | Status / exit gate |
| --- | --- | ---: | --- |
| `Wave 1` | `A1 + A2` | `3-4` | completed |
| `Wave 2` | `A3 + A4` | `2-4` | completed |
| `Wave 3` | `B1 + B2` | `4-6` | next active wave |
| `Wave 4` | `B3 + B4` | `4-6` | session portability and event grammar both move out of concept stage |
| `Wave 5` | `B5 + B6` | `3-6` | runtime adapter boundary is stable and AdsPower refresh is evidence-based |

Execution rule:

1. do not move to `Wave 4` without `B1` evidence collection and `B2` runtime-depth baselines
2. do not refresh AdsPower parity before `Wave 5`
3. do not silently borrow Axis B budget to hide a reopened mainline regression

## Stage Packages, Volume, And Agent Plan

### B1 Validation Foundation

Task packages:

1. `validation domain model`
2. `detector / leak probe runners`
3. `evidence persistence and report schema`
4. `validation board UI`
5. `acceptance profile presets`

Task volume:

- `5` task packages
- about `14-18 worker-days / 5 implementation slices / 4 modules`
- recommended completion shape: `3-4` workers + `1-2` explorers

### B2 Fingerprint Model And Runtime Depth

Task packages:

1. `Profile Spec and canonical grouped schema`
2. `Consistency Graph and explainability`
3. `runtime projection deepening`
4. `applied vs observed report layer`
5. `first-family thickening for Win11 business laptop`

Task volume:

- `5` task packages
- about `16-20 worker-days / 5 implementation slices / 5 modules`
- recommended completion shape: `3-4` workers + `1` explorer

### B3 Session / Proxy Orchestration

Task packages:

1. `SessionBundle contract`
2. `profile group and portability flows`
3. `sticky residency / lease / cooldown engine`
4. `geo-locale-timezone coherence enforcement`
5. `import / export / restore operator flows`

Task volume:

- `5` task packages
- about `14-17 worker-days / 5 implementation slices / 5 modules`
- recommended completion shape: `3-4` workers + `1` explorer

### B4 Event Grammar And Automation Expansion

Task packages:

1. `event grammar core`
2. `workflow graph model`
3. `replay / debug / audit pipeline`
4. `recovery / manual-gate semantics`
5. `automation UI and timeline upgrades`

Task volume:

- `5` task packages
- about `18-22 worker-days / 5 implementation slices / 5 modules`
- recommended completion shape: `4` workers + `1-2` explorers

### B5 Runtime Adapter And External Integration

Task packages:

1. `RuntimeAdapter abstraction`
2. `adapter-aligned explain / observation contract`
3. `external asset intake for validation / schema / session / proxy`
4. `cross-adapter parity reporting`

Task volume:

- `4` task packages
- about `15-19 worker-days / 5 implementation slices / 5 modules`
- recommended completion shape: `3` workers + `2-3` explorers

### B6 AdsPower Boundary Refresh

Task packages:

1. `benchmark evidence refresh`
2. `score recalculation`
3. `current / target / AdsPower comparison tables`
4. `next-gap prioritization`

Task volume:

- `4` task packages
- about `7-9 worker-days / 4 implementation slices / 3 modules`
- recommended completion shape: `1` worker + `3-5` explorers

## Recommended Parallelism By Stage

| Stage | Default active agents | Preferred mix | Why |
| --- | ---: | --- | --- |
| `B1` | `4-6` | `1-2 explorers + 3-4 workers` | validation has good slice parallelism |
| `B2` | `4-5` | `1 explorer + 3-4 workers` | schema / runtime / report split is clean |
| `B3` | `4-5` | `1 explorer + 3-4 workers` | portability / proxy / engine split is clean |
| `B4` | `5-6` | `1-2 explorers + 4 workers` | grammar / workflow / UI / native slices are separable |
| `B5` | `4-6` | `2-3 explorers + 3 workers` | integration is read-heavy before code-heavy |
| `B6` | `4-6` | `3-5 explorers + 1 worker` | benchmark refresh is mostly evidence synthesis |

## Cross-Axis Rules

1. Axis A is closed and must not be quietly reopened.
2. Axis B is now the active expansion track.
3. AdsPower benchmark refresh happens at `B6`, not every time a document changes.
4. Counts and scores must report current landed evidence separately from target numbers.
5. The phrases `50+`, `450+`, `AdsPower catch-up`, and `external integration` must never be reported as current shipped runtime depth.

## Score Method

### Why Scores Are Separate From Progress

Use progress for scope closure:

- `100% / 0% / green`
- `35% / 65% / yellow`

Use score only for capability maturity and benchmark distance.

### Score Anchors

- `0`: document or concept only
- `2`: schema / partial code only, not a stable runtime path
- `5`: shipped path exists, but coverage is narrow and validation is weak
- `8`: runtime closure + validation matrix + operator surface
- `10`: mature or competitor-grade execution

### Score Dimensions And Weights

| Dimension | Weight | What it measures |
| --- | ---: | --- |
| Fingerprint quantity | 15 | declared controls vs runtime-projected breadth vs target signal system |
| Fingerprint realism | 20 | coherence + runtime realism + validation evidence |
| Event taxonomy | 15 | real shipped event / primitive breadth and replayability |
| Proxy / IP | 15 | provider write, residency, health, coherence, rollback |
| Session continuity | 10 | restart continuity, portability, and bundle maturity |
| Product surface | 10 | real operator surface and workflow completeness |
| AdsPower parity | 15 | relative maturity gap vs AdsPower benchmark boundary |

### Current Scorecard

`Current capability score = 39 / 100`

| Dimension | Weight | Current evidence | Current score | Weighted score | Final target | AdsPower public boundary |
| --- | ---: | --- | ---: | ---: | --- | --- |
| Fingerprint quantity | 15 | `80` declared controls / `12` runtime-projected fields | `4/10` | `6.0` | `450+` total signals with control / derived / observation split | `50+` customizable parameters and `20+` options, public score `8/10` |
| Fingerprint realism | 20 | first-family consistency start exists, but runtime depth and validation are shallow | `2/10` | `4.0` | headed realism + validation board + observation evidence | public score `8/10` |
| Event taxonomy | 15 | `13` shipped primitives | `2/10` | `3.0` | `450+` replayable event taxonomy | public count undisclosed, public breadth score `8/10` |
| Proxy / IP | 15 | typed provider-refresh feedback is landed, but residency / lease / rollback / health evidence is not closed | `6/10` | `9.0` | provider-grade rotation + lease / cooldown / rollback + coherence evidence | public score `7/10` |
| Session continuity | 10 | restart continuity for cookies / localStorage / sessionStorage is landed | `6/10` | `6.0` | full `SessionBundle` + portability + import / export | public score `8/10` |
| Product surface | 10 | real desktop entry + multi-workbench surface are landed with more truthful operator status | `6/10` | `6.0` | richer operator tooling, groups, portability, team-grade workflows | public score `9/10` |
| AdsPower parity | 15 | current product has an honest mainline surface and some real contracts, but deep parity is still far away | `3/10` | `4.5` | reach or surpass AdsPower on the benchmark board | AdsPower baseline `10/10` |

`AdsPower public-boundary reference score = 83 / 100`

This is an inferred public-boundary score, not a source-code audit of AdsPower.

## Count Reporting Rules

Always report fingerprint and event quantities as multi-part numbers:

- fingerprint: `declared / runtime / target`
- event taxonomy: `shipped / target`
- session continuity: `restart continuity landed / session bundle not yet landed`

Default wording:

- fingerprint quantity: `80 declared / 12 runtime / 450+ target-only`
- event quantity: `13 shipped / 450+ target-only`
- continuity: `restart continuity landed / portability not yet landed`

## AdsPower Benchmark Summary

### Confirmed Public Boundary As Of 2026-04-16

Official public materials currently indicate that AdsPower provides:

1. `50+` customizable fingerprint parameters and `20+` options
2. dual stealth browsers: Chromium-based `SunBrowser` and Firefox-based `FlowerBrowser`
3. built-in `RPA`, `Local API`, and `Synchronizer`
4. headless launch with API key
5. profile sharing and transfer paths that preserve cookies / fingerprints / IP-related data
6. batch profile operations, permissions, and a more mature operator surface

### What PersonaPilot Already Has Against That Boundary

- our control-plane schema breadth is already large on paper: `80` declared controls
- we already have a real desktop product shell and workflow surface
- we already have restart continuity for cookies and storage
- we already have proxy / session binding foundations

### What AdsPower Still Clearly Leads In

- runtime materialization depth
- fingerprint realism evidence and validation tooling
- mature automation breadth
- richer proxy / profile / portability ecosystem
- Chrome / Firefox real runtime depth and wider productized operator tooling

## Public Source Set For AdsPower Comparison

The comparison above was checked against official public AdsPower materials on `2026-04-16`:

1. [AdsPower homepage](https://www.adspower.com/)
2. [AdsPower browser fingerprint guide](https://help.adspower.com/docs/browser_fingerprint)
3. [AdsPower Synchronizer guide](https://help.adspower.com/docs/synchronizer)
4. [AdsPower Local API guide](https://help.adspower.com/docs/api)
5. [AdsPower RPA guide](https://help.adspower.com/docs/rpa)
6. [Create a profile](https://help.adspower.com/docs/creating_browser_profiles)
7. [Profile sharing](https://help.adspower.com/docs/Profile_Sharing)
8. [Transfer profiles from another antidetect](https://help.adspower.com/docs/transfer_profiles_to_adspower_from_another_antidetect)
9. [AdsPower Local API product page](https://www.adspower.com/local-api)
10. [AdsPower pricing / feature matrix](https://www.adspower.com/pricing)

## Update Rule

When this report is refreshed:

1. keep progress truth synced with `docs/02-current-state.md` and `docs/final-goal-progress-breakdown.md`
2. only raise scores when new shipped evidence exists
3. if a competitor public count is not disclosed, mark it as `undisclosed` instead of inventing a number
4. refresh the AdsPower public boundary using official public sources only
