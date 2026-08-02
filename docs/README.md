# Docs 维护入口

本目录保存 PersonaPilot 的内部维护真相源。根目录文档只做兼容入口；接手、汇报、规划时先读这里。

## 下一位 AI 先读

1. `docs/README.md`：阅读顺序和维护纪律。
2. `docs/02-current-state.md`：当前 live truth。
3. `docs/final-goal-progress-breakdown.md`：双轴进度口径和关键数量。
4. `docs/19-phase-plan-and-scorecard.md`：阶段计划、工作量、评分和 AdsPower 边界。
5. `docs/03-roadmap.md`：Now / Next / Later 路线。
6. `docs/04-improvement-backlog.md`：待办池和风险。
7. `docs/05-ai-maintenance-playbook.md`：AI 接手、更新和验收规则。
8. `docs/24-external-distribution-readiness.md`：历史外部分发方案；当前本机自用范围下已取消，不作为接手必读阻塞项。
9. `docs/25-overall-remaining-work-register.md`：`40% / 60%` 后的剩余工作全集和执行状态。
10. `docs/40-m4-m20-execution-board.md`：M4-M20 执行板、harness 规则、验收边界和分批 commit 纪律。
11. **Stealth / 平台自动化任务**：`docs/45-stealth-platform-handoff.md`（交接入口）→ `docs/44-dual-track-99plus-spec.md` → `docs/43-capability-scenario-test-suite.md` → `docs/29-proxy-supply-chain.md` §5.3。
12. **AdsPower / BitBrowser / PersonaPilot 横评任务**：先读 `docs/47-personal-pilot-adspower-bitbrowser-benchmark.md`，再读 `docs/48-three-browser-benchmark-matrix-plan.md`；该轨道是用户明确重开的独立 benchmark，不覆盖本机自用 `100% / 0% / green` 口径。
13. **指纹 / 代理 / 反检测优化轨道（2026-07-11，49–56，统一入口 `PLAN.md`）**：按层次读
    - `PLAN.md` — **执行枢纽**：波次、Task 速查、统一门禁、文档生命周期
    - `docs/49-fingerprint-proxy-optimization-plan.md` — 战术：修已有 hook 真实性（toString 原生化、UA/内核一致、噪声、DNS、鼠标）
    - `docs/50-asymmetric-dominance-architecture.md` — 战略：让风控「不值得拦」的五支柱（出站身份统一/同一真实用户/连接隐匿/信任优先/成本不对称）
    - `docs/51-fingerprint-surface-coverage-guide.md` — 广度：补齐未 hook 指纹面（window.chrome/permissions/WebGPU/UA-CH/WebGL 深参/字体/ClientRects，S1–S14）
    - `docs/52-device-persona-coherence-engine.md` — 相干性：真实机型人格库 + 发射前硬门禁 + geo 活体 + 受控老化
    - `docs/53-behavior-biometrics-l5-execution-bridge.md` — 行为 L5：Fitts/四段点击/惯性滚动接线、打字 dwell/flight、OS/CDP 同源
    - `docs/54-concurrency-process-hygiene.md` — 规模：多开端口预留、资源预算/背压、孤儿 reconcile、崩溃自愈
    - `docs/55-account-health-observability.md` — 度量：按日/账号过检率·封号率·成功率、warmup 引擎接线、成本不对称闭环
    - `docs/56-boundary-closure-and-commercial-parity.md` — 边界：Provider 闭环、信任双栈统一、Graph 出站、CreepJS parser、RPA 商业补齐

    八份 + `PLAN.md` 均含证据基线、Task、AC、门禁；规划轨道不改本机自用 live truth。`docs/49` 是地基。
    **2026-07-11 复核**：W0（A1/A3/C1 + D1 主体）已落地实现；D1 OS fallback 与横评刷分未关。历史 missing probe 的 UA/core `0/3` 是**改前** artifact，不得写成 W0 后仍失败。下一批本地 W1：Worker / 相干硬门禁 / L5 / 信任双栈 / CreepJS parser。

## 当前报告口径

- Mainline delivery：`100% / 0% / green`
- Local self-use：`100% / 0% / green`
- 唯一未验：CAPTCHA / SMS / Email 服务商真实账号凭证 smoke；没有真实账号和密钥，不能伪造 accepted。
- Fingerprint：`80` declared controls / `26` runtime projected fields / `450` taxonomy seed / strict observed coverage `450 / 450`，最新状态 `passed_full_observed_fingerprint_coverage`。
- Behavior：`35` declared primitives；Go `30` 个 `ExecutePrimitive` shipped 分支；Rust `13` 个 active-runner-backed primitives / `8` page archetypes / `450` taxonomy seed / local deterministic replay `461 / 450`，最新状态 `passed_full_local_replay_runtime`，`contractOnly=0`。
- Session：cookie / localStorage / sessionStorage restart continuity 已落地；profile-scoped `SessionBundle` 本机 export、preflight、dry-run、confirmed restore 已 verified；跨机器/第二机 portability 已取消。
- Runtime：M10 stability/coherence `3/3` passed；M15 real browser process prewarm/CDP/RSS/cleanup proof passed；M15 pool/process integration passed；本机 direct TLS/transport observed；runtime adapter gate 状态 `passed_local_self_use`。
- Benchmark：2026-07-07 用户明确重开 AdsPower / BitBrowser / PersonaPilot 横评；AdsPower 已安装到 `D:\SelfMadeTool\ads\AdsPowerGlobal`，但 Free 账号 API & MCP 为付费墙，当前只作安装/官方资料/手工观察项。2026-07-08 已完成代理预检：Clash 机场只覆盖 US/JP（DE 缺），UDEAL 经 panda/本机桥是 LA 单出口；两者必须拆成 proxy submatrix，不混一个 IP 质量分。当前自动化实测收敛为 PersonaPilot + BitBrowser；基础 launch-loop 已跑，Clash US/JP `40/40 ok`，UDEAL-LA `19/20 ok`（BitBrowser 1 次内存保护失败）。深度矩阵 `deep-1783482661915` 已跑当前可执行 Clash US/JP + UDEAL-LA：`6/6` launch ok、国家匹配 `6/6`、TLS/H2 `6/6`、行为 `5/6`（PersonaPilot Clash US 一次 CDP click timeout）、detector 主跑 PersonaPilot `18/18`、BitBrowser `12/12`；BitBrowser Clash JP detector 瞬时缺口已用 `deep-1783490711021` 补跑为 `6/6`。低配额 missing probe `missing-1783494756348` / `missing-1783495149879` 均 `3/3 ok`，双方 UA/core match `0/3`、WebRTC candidate `0/3`、canvas in-session `3/3`；当前评分 PersonaPilot `73/100`、BitBrowser `71/100`、AdsPower `N/A`。**注意**：2026-07-10 已落地 UA/core 单一真相源与 toString nativeization，但**尚未用新 raw artifact 刷分**；上述 `0/3` 是改前基线。DE、AdsPower API、权威 DNS-token、CreepJS structured trust/lies 和 10-run drift 仍是缺口。

历史 `77% / 23%` 或 `82% / 18%` 只能作为历史上下文，不作为 live truth。

## 真相来源优先级

1. 当前代码、配置、测试、构建和验证结果。
2. `docs/02-current-state.md`、`docs/final-goal-progress-breakdown.md`、`docs/19-phase-plan-and-scorecard.md`。
3. `TODO.md`、`STATUS.md`、`PROGRESS.md`、`docs/root-entrypoint-map.md`。
4. 其他 `docs/` 历史设计文档。
5. 根目录说明文档和聊天上下文。

## 更新纪律

- 当前事实变更先更新 `02-current-state.md`。
- 路线变化更新 `03-roadmap.md`。
- 未做、风险、技术债更新 `04-improvement-backlog.md`。
- 接手规则变化更新 `05-ai-maintenance-playbook.md`。
- 不把历史 Overall `40% / 60% / yellow` 当成当前本机自用 live truth。
- 不把 staged / fallback / mock 默认路径算作闭环交付。
- 本机自用范围下，不再把外部分发 smoke、release performance 预算、干净 Win11/第二机验证、跨机器 SessionBundle portability、AdsPower 刷分或远程代理账号当作未完成项；相关旧文档只作历史上下文。
