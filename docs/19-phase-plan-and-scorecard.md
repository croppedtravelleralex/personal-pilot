# 19 Phase Plan And Scorecard
Updated: 2026-04-16 (Asia/Shanghai)

## Canonical Role

This is the canonical detailed report for:

1. full-app implemented vs not implemented
2. detailed dual-axis phase planning
3. internal capability scorecard
4. AdsPower benchmark comparison

Keep progress truth and capability score separate:

- mainline delivery: `95% / 7% / green`
- overall end-state: `30% / 70% / yellow`
- internal capability score: `34 / 100`
- AdsPower public-boundary reference score: `83 / 100`

The progress split answers “how much of our declared scope is closed”.
The capability score answers “how mature the product is relative to the final target and to AdsPower”.

## Verified Reality Baseline

The current detailed report must stay anchored to these verified facts:

- first-family control taxonomy already declares `80` core control fields
- current `Lightpanda` runtime only projects `12` env-backed fingerprint fields including derived `platform`
- current behavior runtime only ships `13` real primitives
- cookie / localStorage / sessionStorage continuity is already persisted and restored across app restarts
- current real runner set is still `Fake + Lightpanda`; headed Chromium / Firefox deep runtime is not landed

These facts mean:

- the project is no longer “without UI” or “without program entry”
- the project is also not yet at AdsPower-grade runtime realism, validation depth, or automation breadth

## Implemented Today

### Product Surface

- Win11 desktop shell is landed on `Tauri 2 + Vite + React + TypeScript`
- `src/services/desktop.ts` remains the only native / invoke boundary
- Dashboard / Profiles / Proxies / Automation / Synchronizer / Logs / Settings are on the real operator surface
- `Tasks -> Automation` surface unification is already landed

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

- provider-aware / sticky-aware `changeProxyIp` local contract is landed
- proxy session continuity schema is landed through `proxy_session_bindings`
- cookie / localStorage / sessionStorage restore on restart is landed

### Synchronizer

- live desktop snapshot is landed
- native window focus is landed
- unsupported writes are honestly kept as staged / not-yet-closed paths

### Research And Integration Planning

- external browser research is landed under `research/external/`
- the external integration plan is already documented and bounded to the overall track

## Not Implemented Yet

### Mainline Remaining `7%`

- provider-side proxy rotation write is not fully closed
- synchronizer native batch / layout / broadcast write path is not fully closed
- recorder / templates deeper native closure is not fully closed
- final mainline release gate still depends on the three items above

### Overall End-State Remaining `70%`

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

## Axis A: Mainline Remaining `7%`

### A1 Proxy / IP Closeout

Goal:

- close the gap between current local contract and provider-grade proxy rotation write

Detailed tasks:

1. finish provider API write behind the stable `changeProxyIp` contract
2. bind residency / sticky semantics to real provider-side actions
3. close failure rollback, cooldown, and retry paths
4. keep proxy selection and session continuity data aligned

Acceptance:

- provider-side rotation works through the native chain
- failure rollback path is explicit and typed
- sticky / residency behavior is not only local-state decoration

Primary report dimensions:

- `proxy/IP`
- `session continuity`
- `mainline closeout`

### A2 Synchronizer Native Closure

Goal:

- move from live read / focus into real native write closure

Detailed tasks:

1. land native `set main` write path
2. land native `layout` write path
3. land native `broadcast` write path
4. remove staged-only default paths from the main operator route

Acceptance:

- main window control is native
- layout control is native
- broadcast control is native
- staged fallback is no longer the default control path

Primary report dimensions:

- `operator surface`
- `automation/RPA`
- `mainline closeout`

### A3 Recorder / Templates Native Closure

Goal:

- turn the remaining recorder / template flow into native-first closure

Detailed tasks:

1. remove remaining release-default fallback dependence
2. deepen recorder capture and template compile / replay closure
3. make template execution depend on native-first paths rather than UI fallback recovery

Acceptance:

- recorder main path is native-first
- template compile / replay path is native-first
- fallback remains only as exception handling, not the default route

Primary report dimensions:

- `automation/RPA`
- `operator surface`
- `mainline closeout`

### A4 Mainline Release Gate

Goal:

- close the remaining mainline slice without reopening architecture scope

Detailed tasks:

1. rerun `cargo test --quiet`
2. rerun `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`
3. rerun `pnpm desktop:release`
4. confirm the closeout does not introduce new boundary drift

Acceptance:

- the three gates above pass together
- Win11 / Tauri / single-boundary rules remain intact
- mainline can be reported as fully closed without borrowing from the overall track

Primary report dimensions:

- `mainline progress`
- `verification / acceptance`

## Axis B: Overall End-State Remaining `70%`

### B1 Validation Foundation

Goal:

- create the evidence system that later scores and AdsPower comparison must depend on

Detailed tasks:

1. land `validation board`
2. define `ValidationProfile`
3. define `ObservationReport`
4. add detector / leak / transport / coherence evidence collection

Acceptance:

- detector and leak checks are repeatable
- observation reports differentiate declared / applied / observed signals
- future benchmark refreshes can cite evidence rather than only design intent

Primary report dimensions:

- `fingerprint realism`
- `AdsPower benchmark`

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

Primary report dimensions:

- `fingerprint controls`
- `runtime-projected signals`
- `fingerprint realism`

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

Primary report dimensions:

- `proxy/IP`
- `session continuity`

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

Primary report dimensions:

- `event taxonomy`
- `automation/RPA`

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

Primary report dimensions:

- `fingerprint realism`
- `proxy/IP`
- `automation/RPA`
- `AdsPower benchmark`

### B6 AdsPower Boundary Refresh

Goal:

- refresh the benchmark only after real evidence and deeper runtime maturity exist

Detailed tasks:

1. rescore all dimensions after B1-B5 first-pass landing
2. compare current vs target vs AdsPower on evidence
3. update benchmark gaps and next priorities

Acceptance:

- AdsPower comparison is evidence-based
- scores are updated only when capability evidence changes
- benchmark language stops drifting with each ad hoc report

Primary report dimensions:

- `AdsPower benchmark`
- `overall end-state progress`

## Cross-Axis Rules

1. Axis A closes the current product delivery path and must not absorb Axis B scope.
2. Axis B can move in parallel, but it must not block Axis A release gate.
3. AdsPower benchmark refresh happens at `B6`, not every time a document changes.
4. Counts and scores must report current landed evidence separately from target numbers.
5. The phrases `50+`, `450+`, `AdsPower catch-up`, and `external integration` must never be reported as current shipped runtime depth.

## Score Method

### Why Scores Are Separate From Progress

Use progress for scope closure:

- `95% / 7% / green`
- `30% / 70% / yellow`

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

`Current capability score = 34 / 100`

| Dimension | Weight | Current evidence | Current score | Weighted score | Final target | AdsPower public boundary |
| --- | ---: | --- | ---: | ---: | --- | --- |
| Fingerprint quantity | 15 | `80` declared controls / `12` runtime-projected fields | `4/10` | `6.0` | `450+` total signals with control / derived / observation split | `50+` customizable parameters and `20+` options, public score `8/10` |
| Fingerprint realism | 20 | first-family consistency start exists, but runtime depth and validation are shallow | `2/10` | `4.0` | headed realism + validation board + observation evidence | public score `8/10` |
| Event taxonomy | 15 | `13` shipped primitives | `2/10` | `3.0` | `450+` replayable event taxonomy | public count undisclosed, public breadth score `8/10` |
| Proxy / IP | 15 | sticky-aware contract + session bindings are landed, provider-side write not fully closed | `5/10` | `7.5` | provider-grade rotation + lease / cooldown / rollback + coherence evidence | public score `7/10` |
| Session continuity | 10 | restart continuity for cookies / localStorage / sessionStorage is landed | `6/10` | `6.0` | full `SessionBundle` + portability + import/export | public score `8/10` |
| Product surface | 10 | real desktop entry + multi-workbench surface are landed | `5/10` | `5.0` | richer operator tooling, groups, portability, team-grade workflows | public score `9/10` |
| AdsPower parity | 15 | current product has a base surface and some real contracts, but deep parity is far away | `2/10` | `3.0` | reach or surpass AdsPower on the benchmark board | AdsPower baseline `10/10` |

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
- we already have proxy/session binding foundations

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
