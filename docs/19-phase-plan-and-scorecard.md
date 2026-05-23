# 19 Phase Plan And Scorecard
Updated: 2026-05-23 (Asia/Shanghai)

## 规范角色

这是以下内容的规范详细报告：

1. 全应用已实现 vs 未实现
2. 详细双轴阶段规划
3. 内部能力评分卡
4. AdsPower 基准比较

保持进度真相和能力评分分离：

- mainline delivery: `100% / 0% / green`
- overall end-state: `30% / 70% / yellow`
- internal capability score: `34 / 100`
- AdsPower public-boundary reference score: `83 / 100`

The progress split answers “how much of our declared scope is closed”.
The capability score answers “how mature the product is relative to the final target and to AdsPower”.

## 已验证事实基线

当前详细报告必须锚定这些已验证事实：

- 第一族控制分类已声明 `80` 个核心控制字段
- 当前 `Lightpanda` 运行时投影 `26` 个字段（`25` 个 control-supported + derived `platform`），但完整 observed proof 尚未落地
- 当前行为运行时仅交付 `13` 个真实原语
- cookie/localStorage/sessionStorage 连续性已在应用重启间持久化和恢复
- profile-scoped `SessionBundle` export、import preflight、non-destructive restore contract 已落地；真实 restore write path 和 profile portability smoke 尚未落地
- Validation Board 已接入 native DNS/transport reports、desktop WebView scoped probes、profile runtime probe contract、report history 和 profile evidence export；真实 Lightpanda/CDP operator smoke 尚未完成
- 当前真实运行器集仍为 `Fake + Lightpanda`；有头 Chromium/Firefox 深度运行时未落地

这意味着：

- 项目不再"无 UI"或"无程序入口"
- 项目也尚未达到 AdsPower 级别的运行时真实感、验证深度或自动化广度

## 今日已实现

### 产品表面

- Win11 desktop shell 基于 `Tauri 2 + Vite + React + TypeScript` 已落地
- `src/services/desktop.ts` 仍然是唯一的原生/调用边界
- Dashboard/Profiles/Proxies/Automation/Synchronizer/Logs/Settings 已在真实 operator 表面
- `Tasks -> Automation` 表面合并已完成

### 指纹和运行时

- 第一族 `80` 个核心控制字段已声明并分组为 `8` 个部分
- 规范指纹运行时可解释性和部分消费报告已落地
- 当前 `Lightpanda` 运行时投影 `26` 个字段（`25` 个 control-supported + derived `platform`）
- 项目已区分已声明控制 vs 运行时投影字段

### 行为和自动化

- 行为计划编译路径已落地
- 当前交付的行为层有 `13` 个原语
- recorder desktop 步骤写入已落地
- 运行时解释/追踪摘要/种子驱动计划生成已落地

### 代理/IP/会话

- provider-aware/sticky-aware `changeProxyIp` 本地合约已落地
- 代理会话连续性 schema 通过 `proxy_session_bindings` 已落地
- cookie/localStorage/sessionStorage 重启恢复已落地

### Synchronizer

- 实时桌面快照已落地
- 原生窗口焦点已落地
- 原生 set-main 和工作区感知物理布局已落地
- 广播写入现已通过 `SetWindowPos` 实现原生物理窗口排布，确定性排序

### 研究和集成规划

- 外部浏览器研究已落地于 `research/external/`
- 外部集成计划已记录并限定在整体轨道内

## Not Implemented Yet

### Mainline — 已全部闭环 (原 `7%`)

原剩余 `7%` 的 3 个 P0 项已通过 multi-agent worktree 并行执行完成：

- provider 侧代理轮换写入 — **已完成**：`changeProxyIp` 从本地桩升级为真实 HTTP POST/PUT/PATCH 引擎，含重试/冷却/回滚/7 种错误分类
- synchronizer 原生广播写入路径 — **已完成**：物理 `SetWindowPos` 窗口排布，确定性排序，CWAC 感知布局
- recorder/templates native-first 降级闭环 — **已完成**：desktop session 守卫防止草稿覆盖，空状态模板选择修复，源消息精确化
- 主线 release gate：Win11 本地验收和 release build 已通过；发布给外部前可再做人工 operator smoke

### 整体终态剩余 `70%`

- Validation Board 已有 MVP、native report、history、profile export 和 runtime probe contract；真实 Lightpanda/CDP operator smoke 未落地
- 运行时实现深度仍窄，仅 `26` 个投影字段，且不能等同于 observed proof
- `450+` 指纹信号观察/审计覆盖未落地
- `450+` 事件分类未落地
- `SessionBundle` export、import preflight、restore contract 已落地；真实 restore write path 和 profile portability smoke 未落地
- 有头运行时真实感和更深的内核策略未落地
- AdsPower 级真实感追赶未落地
- 外部集成资产已规划但尚未运行时落地

## 最终目标

最终目标比当前可交付桌面应用更广泛：

1. 保持当前 Win11 desktop shell 稳定
2. 保持类型化服务边界和单实例纪律
3. 维护至少 `80` 个真实控制字段作为第一族基线
4. 从 `26` 个运行时投影字段扩展到实质性的应用/观察运行时覆盖
5. 增长到 `450+` 总指纹信号（控制/派生/观察层）
6. 从 `13` 个已交付原语增长到 `450+` 事件分类
7. 将 `SessionBundle` 合约转变为真实可迁移、可导入、可恢复的 profile portability 闭环
8. 将 Validation Board 已有 evidence contract 推进到真实 Lightpanda/CDP 可重复 operator smoke
9. 吸收高 ROI 外部浏览器优势，不将浏览器 fork 拉入主仓库
10. 在真实感、代理一致性、自动化深度和运营工具方面达到或超越 AdsPower

## Axis A: Mainline — 已全部闭环

### A1 代理/IP 闭环 — 已完成

Provider 级代理轮换写入已落地：

- `changeProxyIp` 引擎 (`src/runner/engine.rs:395-738`) 执行真实 HTTP POST/PUT/PATCH 请求到 provider 端点
- 7 种错误分类：missing_proxy_id、proxy_not_found、db_error、unsupported_provider_config、unsupported_provider_method、missing_provider_credentials、provider_error
- 重试/冷却/回滚通过 `retryable` 标志和 `error_kind` 完整类型化
- sticky/residency 语义绑定到真实 provider 操作

关联维度：`proxy/IP`、`session continuity`、`mainline closeout` — 全部闭环。

### A2 Synchronizer 原生写入闭环 — 已完成

原生广播写入路径已落地：

- `broadcast_native_placement` (`src-tauri/src/commands.rs:1025-1100`) 为所有计划类型生成 CWAC 感知物理布局（nav-mirror、layout-regroup、scroll-checkpoint、input-burst）
- `apply_broadcast_native_layout` 通过 `SetWindowPos` 执行确定性排序窗口排布
- staged-only 默认路径已从主 operator 路线移除

关联维度：`operator surface`、`automation/RPA`、`mainline closeout` — 全部闭环。

### A3 Recorder/Templates 原生闭环 — 已完成

Native-first 降级闭环已完成：

- `startDraftSession` 添加守卫防止覆盖活跃 desktop session (`src/features/recorder/store.ts:93-114`)
- `ensureSelectedTemplateId` 回退链修复 — 种子数据空状态不再返回 `null` (`src/features/templates/store.ts:119-137`)
- 源消息精确化："contract is not registered" 替代误导性的 "did not answer as ready"
- hooks 链正确回退到 `filteredItems[0] ?? null`

关联维度：`automation/RPA`、`operator surface`、`mainline closeout` — 全部闭环。

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

1. keep the landed `validation board` evidence split stable
2. keep `ValidationProfile` and report history/export contracts stable
3. run real Lightpanda/CDP operator smoke outside FakeRunner
4. add deeper detector / leak / transport / coherence evidence collection

Acceptance:

- detector and leak checks are repeatable in real profile-browser runtime
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
4. expand current `26` projected fields into richer applied / observed coverage
5. keep control / derived / observation layers clearly separated

Acceptance:

- runtime coverage is no longer summarized only by `26` projected fields
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

1. deepen the landed `SessionBundle` export/import preflight/restore contract
2. add real restore write path and profile portability smoke
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

## Stage Execution Stack

| Stage | UI / TS layer | Desktop service layer | Rust / native / data layer | Validation / evidence layer |
| --- | --- | --- | --- | --- |
| `A1` Proxy / IP closeout | `src/pages/ProxiesPage.tsx`, `src/components/proxies/*`, `src/features/proxies/*` | `src/services/desktop.ts`, `src/types/desktop.ts` for typed proxy contracts | `src-tauri/src/commands.rs`, `src/desktop/mod.rs`, `src/runner/engine.rs`, proxy/session tables in `src/db/schema.rs` | provider smoke checks, continuity regression, Win11 local verify |
| `A2` Synchronizer native closure | `src/pages/SynchronizerPage.tsx`, `src/components/synchronizer/*`, `src/features/synchronizer/*` | typed synchronizer read/write contracts in `src/services/desktop.ts` | `src-tauri/src/commands.rs`, `src/desktop/mod.rs`, native window command wiring | release smoke + multi-window behavior proof |
| `A3` Recorder / Templates native closure | `src/pages/AutomationPage.tsx`, `src/components/automation/*`, `src/features/recorder/*`, `src/features/templates/*` | typed compile / launch / recorder contracts in `src/services/desktop.ts` | `src-tauri/src/commands.rs`, `src/desktop/mod.rs`, recorder/template persistence paths | end-to-end template compile / replay verification |
| `A4` Mainline release gate | thin UI touch only if regressions appear | no new API surface unless acceptance exposes a gap | whole repo build/test/release pipeline, Win11 enforcement scripts | `cargo test --quiet`, `windows_local_verify.ps1`, `pnpm desktop:release` |
| `B1` Validation foundation | future `src/features/validation/*`, validation dashboards and evidence panels | typed validation commands / report reads in `src/services/desktop.ts` | detector/leak probe orchestration, report persistence, observation schema | detector, leak, DNS, WebRTC, transport, coherence evidence packs |
| `B2` Fingerprint model and runtime depth | profile editors, explain panels, runtime diff views | typed fingerprint explain / report / projection APIs | `src/network_identity/*`, `src/runner/lightpanda.rs`, `src/runner/engine.rs`, report persistence | declared vs applied vs observed delta reports |
| `B3` Session / Proxy orchestration | profile groups, import/export, portability and session-bundle screens | typed session-bundle and proxy orchestration APIs | `src/runner/engine.rs`, `src/db/schema.rs`, proxy/session lifecycle tables and serializers | portability proof, sticky residency proof, restart continuity proof |
| `B4` Event grammar and automation expansion | automation graph, replay debugger, audit timeline, manual-gate UI | typed replay / debug / audit / event graph contracts | `src/behavior/*`, `src/workflow/*`, `src/runner/lightpanda.rs`, automation data model | replay determinism, auditability, recovery-path proof |
| `B5` Runtime adapter and external integration | adapter selection UI only if needed, usually thin surface | stable adapter contracts in `src/services/desktop.ts` | `src/runner/*`, `src/network_identity/*`, `src/desktop/mod.rs`, imported external patterns | cross-adapter compare reports and integration proof |
| `B6` AdsPower boundary refresh | benchmark panels and reporting outputs | no heavy new API, mostly report aggregation | score aggregation, benchmark snapshots, doc generation | official public-source refresh + evidence-backed re-score |

## Workload Unit

Use one consistent workload unit everywhere:

- `1 worker-day` = one experienced engineer or coding worker's net implementation day
- `1 implementation slice` = one bounded worker-sized write scope that can normally be owned by one coding agent without crossing too many modules
- `1 module` = one implementation face with a stable boundary, such as `features/proxies`, `runner/lightpanda`, `src-tauri/src/commands.rs`, or a dedicated validation/report subsystem
- `1 task package` = `2-4` implementation slices under one stage outcome

Default parallel rule:

- `tiny`: stay local
- `mainline closeout stages`: `2-4` active agents
- `overall end-state stages`: `3-6` active agents
- `read-heavy benchmark or audit stages`: up to `6-8` explorers when write conflict is near zero

Planning baseline:

- `Axis A` recommended budget: about `38-46 worker-days / 16 implementation slices / 11-12 modules`
- `Axis B` recommended budget: about `84-101 worker-days / 29 implementation slices / 27 modules`

## Recommended Execution Waves

| Wave | Scope | Recommended active agents | Exit gate |
| --- | --- | ---: | --- |
| `Wave 1` | `A1 + A2` | `3-4` | provider-side proxy write path and synchronizer native write path are both materially closed |
| `Wave 2` | `A3 + A4` | `2-4` | recorder/templates are native-first and the full mainline release gate is green |
| `Wave 3` | `B1 + B2` | `4-6` | validation board exists and fingerprint maturity is measurable beyond `26` runtime-projected fields |
| `Wave 4` | `B3 + B4` | `4-6` | session portability and event grammar both move out of concept stage |
| `Wave 5` | `B5 + B6` | `3-6` | runtime adapter boundary is stable and AdsPower refresh is evidence-based |

Execution rule:

1. do not move to `Wave 2` without credible closure evidence for `A1` and `A2`
2. do not move to `Wave 4` without `B1` evidence collection and `B2` runtime-depth baselines
3. do not refresh AdsPower parity before `Wave 5`

## Stage Packages, Volume, And Agent Plan

### A1 Proxy / IP Closeout

Task packages:

1. `provider write adapters`
   Scope: provider-side rotate / refresh / residency write path, typed request/response normalization
2. `session residency state machine`
   Scope: sticky session lifecycle, cooldown, rollback, requested-region/provider alignment
3. `operator feedback and regression closure`
   Scope: proxy UI feedback, health/status propagation, continuity-safe failure surfaces
4. `acceptance pack`
   Scope: provider smoke path, continuity regression, local release verification proof

Task volume:

- `4` task packages
- about `11-13 worker-days / 4 implementation slices / 3 modules`
- recommended completion shape: `2-3` worker agents + `1` explorer for provider/API evidence

Suggested agent plan:

- `1` explorer for provider/API contract tracing
- `2` workers for disjoint write scopes:
  - worker A: `src/features/proxies/*`, `src/components/proxies/*`, `src/services/desktop.ts`, `src/types/desktop.ts`
  - worker B: `src-tauri/src/commands.rs`, `src/desktop/mod.rs`, `src/runner/engine.rs`, `src/db/schema.rs`
- optional `1` extra worker for acceptance automation if provider surface is broad

### A2 Synchronizer Native Closure

Task packages:

1. `native main-window write path`
2. `layout write path`
3. `broadcast write path`
4. `staged-path shrink and operator UX hardening`

Task volume:

- `4` task packages
- about `10-12 worker-days / 4 implementation slices / 3 modules`
- recommended completion shape: `2` workers + `1` explorer

Suggested agent plan:

- worker A: `src/pages/SynchronizerPage.tsx`, `src/components/synchronizer/*`, `src/features/synchronizer/*`
- worker B: `src-tauri/src/commands.rs`, `src/desktop/mod.rs`
- optional explorer: native window command tracing and acceptance checklist

### A3 Recorder / Templates Native Closure

Task packages:

1. `recorder capture closure`
2. `template compile and persistence closure`
3. `template replay / launch closure`
4. `fallback removal and operator polish`

Task volume:

- `4` task packages
- about `10-12 worker-days / 4 implementation slices / 3 modules`
- recommended completion shape: `2-3` workers + `1` explorer

Suggested agent plan:

- worker A: `src/components/automation/*`, `src/features/recorder/*`, `src/features/templates/*`
- worker B: `src/services/desktop.ts`, `src/types/desktop.ts`, `src-tauri/src/commands.rs`
- optional worker C: replay/debug polish and verification scripts

### A4 Mainline Release Gate

Task packages:

1. `build and type gate`
2. `Rust integration and continuity gate`
3. `Win11 local verify gate`
4. `release artifact and regression summary`

Task volume:

- `4` task packages
- about `7-9 worker-days / 4 implementation slices / 2-3 modules` because this stage is verification-heavy, not feature-heavy
- recommended completion shape: `1` local integrator + `1-2` explorers for failure isolation

Suggested agent plan:

- no more than `2` explorers in parallel for failing gate diagnosis
- keep actual fixes centralized to avoid acceptance drift during closeout

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

Suggested agent plan:

- worker A: `src/features/validation/*`, UI and store
- worker B: `src/services/desktop.ts`, `src/types/desktop.ts`, command contracts
- worker C: `src-tauri/src/commands.rs`, `src/desktop/mod.rs`, probe execution
- worker D: persistence/report schema if needed
- explorer(s): official detector/leak target mapping and acceptance matrix

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

Suggested agent plan:

- worker A: `src/network_identity/first_family.rs`, validators, consistency graph
- worker B: `src/network_identity/fingerprint_consumption.rs`, explainability, reporting
- worker C: `src/runner/lightpanda.rs`, `src/runner/engine.rs` runtime integration
- optional worker D: UI/editor/explain panels
- explorer: signal mapping and observation-gap audit

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

Suggested agent plan:

- worker A: `src/features/profiles/*`, portability UI
- worker B: `src/features/proxies/*`, proxy orchestration UI/state
- worker C: `src/runner/engine.rs`, `src/db/schema.rs`
- worker D: service/typed contract layer
- explorer: portability and continuity edge-case audit

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

Suggested agent plan:

- worker A: `src/behavior/*`
- worker B: `src/workflow/*`, execution model
- worker C: `src/components/automation/*`, `src/features/automation/*`
- worker D: service/native contract surfaces
- explorer(s): event taxonomy design audit and replay edge cases

### B5 Runtime Adapter And External Integration

Task packages:

1. `RuntimeAdapter abstraction`
2. `adapter-aligned explain / observation contract`
3. `external asset intake for validation/schema/session/proxy`
4. `cross-adapter parity reporting`

Task volume:

- `4` task packages
- about `15-19 worker-days / 5 implementation slices / 5 modules`
- recommended completion shape: `3` workers + `2-3` explorers

Suggested agent plan:

- worker A: `src/runner/*` adapter boundaries
- worker B: `src/network_identity/*` and report contracts
- worker C: `src/services/desktop.ts` / command layer if UI-exposed
- explorers: external project mapping and integration-risk review

### B6 AdsPower Boundary Refresh

Current P12 status: `deferred by evidence gate`. B1-B5 do not yet have enough new shipped evidence to rescore AdsPower parity, so this refresh stage preserves the current scorecard and records the guard instead of raising the benchmark result.

Task packages:

1. `benchmark evidence refresh`
2. `score recalculation`
3. `current / target / AdsPower comparison tables`
4. `next-gap prioritization`

Task volume:

- `4` task packages
- about `7-9 worker-days / 4 implementation slices / 3 modules`
- recommended completion shape: `1` worker + `3-5` explorers

Suggested agent plan:

- explorer-heavy stage; use up to `6` explorers if comparison is read-heavy and conflict-free
- keep a single worker or main integrator responsible for the canonical benchmark docs and score update

## Recommended Parallelism By Stage

| Stage | Default active agents | Preferred mix | Why |
| --- | ---: | --- | --- |
| `A1` | `3-4` | `1 explorer + 2-3 workers` | provider/API + session engine split cleanly |
| `A2` | `3` | `1 explorer + 2 workers` | UI/native split is clean |
| `A3` | `3-4` | `1 explorer + 2-3 workers` | recorder/template/service split is clean |
| `A4` | `1-3` | `1 local integrator + 0-2 explorers` | acceptance work is integration-heavy |
| `B1` | `4-6` | `1-2 explorers + 3-4 workers` | validation has good slice parallelism |
| `B2` | `4-5` | `1 explorer + 3-4 workers` | schema/runtime/report split is clean |
| `B3` | `4-5` | `1 explorer + 3-4 workers` | portability/proxy/engine split is clean |
| `B4` | `5-6` | `1-2 explorers + 4 workers` | grammar/workflow/UI/native slices are separable |
| `B5` | `4-6` | `2-3 explorers + 3 workers` | integration is read-heavy before code-heavy |
| `B6` | `4-6` | `3-5 explorers + 1 worker` | benchmark refresh is mostly evidence synthesis |

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

P12 guard result: score refresh is deferred until B1-B5 evidence exists. Do not raise this score from docs-only updates.

| Dimension | Weight | Current evidence | Current score | Weighted score | Final target | AdsPower public boundary |
| --- | ---: | --- | ---: | ---: | --- | --- |
| Fingerprint quantity | 15 | `80` declared controls / `26` runtime-projected fields | `4/10` | `6.0` | `450+` total signals with control / derived / observation split | `50+` customizable parameters and `20+` options, public score `8/10` |
| Fingerprint realism | 20 | first-family consistency start exists, but runtime depth and validation are shallow | `2/10` | `4.0` | headed realism + validation board + observation evidence | public score `8/10` |
| Event taxonomy | 15 | `13` shipped primitives | `2/10` | `3.0` | `450+` replayable event taxonomy | public count undisclosed, public breadth score `8/10` |
| Proxy / IP | 15 | sticky-aware contract + session bindings and provider-aware rotation are landed; broader provider ecosystem evidence remains incomplete | `5/10` | `7.5` | provider-grade rotation + lease / cooldown / rollback + coherence evidence | public score `7/10` |
| Session continuity | 10 | restart continuity and `SessionBundle` export/import preflight/restore contract are landed; real portability smoke is open | `6/10` | `6.0` | full `SessionBundle` + portability + import/export | public score `8/10` |
| Product surface | 10 | real desktop entry + multi-workbench surface are landed | `5/10` | `5.0` | richer operator tooling, groups, portability, team-grade workflows | public score `9/10` |
| AdsPower parity | 15 | current product has a base surface and some real contracts, but deep parity is far away | `2/10` | `3.0` | reach or surpass AdsPower on the benchmark board | AdsPower baseline `10/10` |

`AdsPower public-boundary reference score = 83 / 100`

This is an inferred public-boundary score, not a source-code audit of AdsPower.

## Count Reporting Rules

Always report fingerprint and event quantities as multi-part numbers:

- fingerprint: `declared / runtime / target`
- event taxonomy: `shipped / target`
- session continuity: `restart continuity landed / SessionBundle contract landed / portability smoke not yet landed`

Default wording:

- fingerprint quantity: `80 declared / 26 runtime-projected / 450+ target-only`
- event quantity: `13 shipped / 450+ target-only`
- continuity: `restart continuity landed / SessionBundle contract landed / portability not yet landed`

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
5. if B1-B5 evidence is incomplete, record `deferred` and keep the current score unchanged
