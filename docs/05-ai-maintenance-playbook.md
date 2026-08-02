# AI Maintenance Playbook

## 接手顺序

1. 读 `docs/README.md`。
2. 读 `docs/02-current-state.md`。
3. 读 `docs/final-goal-progress-breakdown.md`。
4. 读 `docs/19-phase-plan-and-scorecard.md`。
5. 读 `docs/03-roadmap.md` 和 `docs/04-improvement-backlog.md`。
6. 如任务涉及 M4-M20 执行、harness、commit 切片或验收边界，读 `docs/40-m4-m20-execution-board.md`。
7. **Stealth / 平台 live（XHS 等）**：读 `docs/45-stealth-platform-handoff.md` → `docs/44-dual-track-99plus-spec.md`。
8. 如任务明确重新开启外部分发、发布说明或 release performance，再读 `docs/24-external-distribution-readiness.md` 和 `docs/release-performance-mitigation-plan.md`；默认本机自用范围下这两份只作历史上下文。
9. 如任务明确涉及 AdsPower / BitBrowser / PersonaPilot 横评、指纹浏览器商业竞品评分或三方 benchmark，读 `docs/47-personal-pilot-adspower-bitbrowser-benchmark.md` 和 `docs/48-three-browser-benchmark-matrix-plan.md`；该轨道独立于本机自用完成度。
10. **指纹 / 反检测优化（49–56）**：先读 **`PLAN.md`**，再按波次读 `docs/49`–`docs/56` 对应 Task；改代码后跑 `PLAN.md` §3 门禁。
11. 需要执行任务时，再按范围读相关代码和测试。

## 默认事实

- Mainline：`100% / 0% / green`
- Local self-use：`100% / 0% / green`
- 唯一未验：CAPTCHA / SMS / Email 真实账号凭证 smoke
- Fingerprint：`80` declared controls / `26` runtime projected fields / `450` taxonomy seed / strict observed coverage `450 / 450`，`passed_full_observed_fingerprint_coverage`
- Behavior：`35` declared primitives；Go `30` 个 `ExecutePrimitive` shipped 分支；Rust `13` 个 active-runner-backed primitives / `8` page archetypes / `450` taxonomy seed / local deterministic replay `461 / 450`，最新状态 `passed_full_local_replay_runtime`，`contractOnly=0`。
- Session：cookie / localStorage / sessionStorage restart continuity 已落地；profile-scoped `SessionBundle` export、import preflight、dry-run、confirmed local restore write path 已 verified；跨机器/第二机 profile portability 已取消
- Benchmark：2026-07-07 用户已明确重开 AdsPower / BitBrowser / PersonaPilot 横评。AdsPower 8.6.3 已安装到 `D:\SelfMadeTool\ads\AdsPowerGlobal`，但用户截图确认 Free 账号 API & MCP 仅限付费套餐；本轮 AdsPower 只作 API-paywalled/manual-only 观察。BitBrowser 7.1.3 位于 `D:\SelfMadeTool\bitbrowser` 且 Local API `54345` 可访问。2026-07-08 代理预检已完成：Clash 机场 US/JP 可用但 DE 缺，成功节点 ip-api `proxy=true`；UDEAL 经 panda/本机桥为 LA 单出口，ip-api `proxy=false`、`hosting=false`。`scripts/three_browser_benchmark_readiness.mjs --dry-run --products=personal-pilot,bitbrowser` 已跑通双产品 UDEAL-LA 创建/打开/CDP/tab/关闭，报告 `data/reports/three-browser-benchmark/readiness/readiness-1783475270066.json`。`scripts/two_browser_benchmark_matrix.mjs` 已跑基础 launch-loop：Clash `matrix-1783480553485` 为 `40/40 ok`，UDEAL-LA `matrix-1783480223173` 为 `19/20 ok`（BitBrowser 一次内存保护失败）。`scripts/two_browser_benchmark_deep_matrix.mjs` 已跑当前可执行深度矩阵：`deep-1783482661915` 为 `6/6 ok`，国家匹配 `6/6`、TLS/H2 `6/6`、行为 `5/6`、detector 主跑 PersonaPilot `18/18`、BitBrowser `12/12`；BitBrowser Clash JP detector 瞬时缺口已用 `deep-1783490711021` 补跑 `6/6`。低配额 missing probe 已跑：PersonaPilot `missing-1783494756348`、BitBrowser `missing-1783495149879` 均 `3/3 ok`，双方 WebRTC candidate `0/3`、canvas in-session `3/3`、UA/core match `0/3`、CreepJS trust/lies parser `0/3`；当前评分 PersonaPilot `73/100`、BitBrowser `71/100`、AdsPower `N/A`。剩余缺口是 DE 同类节点、AdsPower paid/trial API、受控 DNS-token proof、CreepJS structured trust/lies 和 10-run cross-session drift；BitBrowser 免费额度刷新前不要跑高消耗矩阵。

## 汇报规则

- 先说明当前结论，再给证据。
- Mainline 和 Local self-use 都按 `100% / 0% / green` 写。
- 不把历史 Overall `40% / 60% / yellow`、AdsPower catch-up、外部分发、release performance 或第二机目标写成当前待办。
- 用户明确重开 AdsPower/BitBrowser 横评时，必须把它写成独立 benchmark track；不得反向污染 Mainline / Local self-use `100% / 0% / green`。
- 涉及 benchmark 代理时，先查 `data/reports/three-browser-benchmark/proxy-preflight/proxy-preflight-summary-*.json`；Clash 机场和 UDEAL-LA 必须拆成子矩阵，不得追问已可本机确认的 Clash controller 或 panda SSH 可达性。
- 涉及当前 benchmark 执行时，先确认是否真的有新增条件：DE 同类节点、AdsPower paid/trial API、受控 DNS-token 域名、BitBrowser 免费打开额度是否刷新，或需要重跑的 detector 变更。基础 launch-loop 用 `scripts/two_browser_benchmark_matrix.mjs`，深度矩阵用 `scripts/two_browser_benchmark_deep_matrix.mjs`，低配额缺口补测用 `scripts/two_browser_missing_probe_matrix.mjs`；Clash 和 UDEAL 保持分子矩阵。没有 AdsPower 付费/试用 API 时，不再反复要求用户找 key，直接标记 AdsPower 为 API-paywalled/manual-only。若未来启用 AdsPower API，必须直连 `127.0.0.1:50325`，当前 `local.adspower.com` 会被 Clash fake-ip 解析到 `198.18.*`。
- 不复活 `77% / 23%` 或 `82% / 18%` 作为 live truth。
- 本机自用范围下，不再把外部分发 smoke、release performance budget、干净 Win11/第二机或跨机器 SessionBundle 写成阻塞项。
- 报告尽量短，优先列 landed result、当前阻塞、下一步。

## 修改规则

- 当前状态变化：更新 `02-current-state.md`。
- 路线变化：更新 `03-roadmap.md`。
- 风险、债务、后续项：更新 `04-improvement-backlog.md`。
- 接手顺序、验证纪律变化：更新 `05-ai-maintenance-playbook.md`。
- AdsPower / BitBrowser / PersonaPilot 三方横评变化：更新 `02-current-state.md`、`03-roadmap.md`、`04-improvement-backlog.md`、`47` 横评报告和 `48` 矩阵计划；实测前不得刷新最终评分。
- 只有用户明确重启外部分发、发布说明、人工 smoke 或 release performance 目标时，才更新 `24-external-distribution-readiness.md` / `release-performance-mitigation-plan.md`；默认只更新当前本机 docs。
- 根目录入口保持薄；除非用户要求，不改根目录文档。

## 验收规则

 meaningful change 完成前至少确认：

- 文档没有把历史进度当成 live truth。
- 文档没有把 Overall 目标当成 Mainline 已交付。
- 涉及代码改动时，按项目规则跑 type check、release build、Win11/Tauri enforcement。
- 文档-only 改动也要检查 canonical 入口是否存在且互相指向有效文件。
- 涉及 M4 provider readiness 时，`scripts/m4_acceptance_gate.ps1` 会刷新 provider preflight；`provider_dry_run_contract` passed 只代表本地 dry-run/failure taxonomy 报告完整，真实 provider smoke 仍必须单独保留 blocked/accepted 证据。
- 涉及 M4 SessionBundle operator loop 时，`session_bundle_operator_contract` passed 只代表 Settings UI、desktop wrapper、TS 类型和 Rust 本机 export/preflight/dry-run/confirmed restore chain 已接通；跨机器 portability 已取消，不再保留为 blocked。
- 涉及 M4 runtime adapter operator loop 时，`runtime_adapter_operator_contract` passed 只代表 Dashboard 可读取 runtime adapter 证据；当前本机自用通过以 `runtime_adapter_evidence_gate.ps1` 的 `passed_local_self_use` 为准。
- 涉及 M8 SessionBundle 本机恢复时，`scripts/m8_session_handoff_gate.ps1` 的 `passed_local_restore_verified` 代表本机 export/preflight/dry-run/confirmed restore 和 persisted restart-continuity artifact 已有本机证据；M8 第二机目标已取消，不得要求 `-CrossMachine` report。
- 涉及 M4 typed facade shrink 时，`typed_facade_shrink_contract` passed 只代表 synchronizer high-traffic DTO、Workbench detection/UI/report DTO、BrowserInstanceStatus DTO 和 browser Wails binding source contract 已类型化；不得写成 `tauriWailsBridge` 已移除、所有 browser payload schema 已规范化或所有 bridge API 已统一。
- 涉及 M4 browser payload schema 时，`scripts/m4_browser_payload_schema_gate.ps1` 的 `passed_browser_payload_schema_contract` 和 M4 gate v9 的 `browser_payload_schema_contract` 只代表 browser runtime event payload 已通过共享 `BrowserRuntimeEventPayload` / `normalizeBrowserRuntimeEventPayload` 归一化，且选定 browser API normalizer 输入已从 `any` 收窄；不得写成 `tauriWailsBridge` 已移除、低频 workbench/core bridge 已统一、真实 headed runtime 已通过或外部证据已完成。
- 涉及 M4 Dashboard facade 时，`scripts/m4_dashboard_facade_gate.ps1` 的 `passed_dashboard_facade_contract` 和 M4 gate v10 的 `dashboard_facade_contract` 只代表 Dashboard API 的 stats/license/config/CD key 调用已改走 `src/services/desktop.ts` typed wrappers，且 Dashboard evidence rows 保留；不得写成 `tauriWailsBridge` 已移除、profile/settings/logs 动态 binding 已统一或外部证据已完成。
- 涉及 M4 Settings/Logs facade 时，`scripts/m4_settings_logs_facade_gate.ps1` 的 `passed_settings_logs_facade_contract` 和 M4 gate v11 的 `settings_logs_facade_contract` 只代表 Settings backup initialize/export/import 与 Browser logs read/clear 已改走 `src/services/desktop.ts` typed wrappers，且 destructive preflight / native dialog / Wails-compatible proxy 仍保留；不得写成 `tauriWailsBridge` 已移除、profile/workbench/core bridge API 已统一或外部证据已完成。
- 涉及 M4 Profile facade 时，`scripts/m4_profile_facade_gate.ps1` 的 `passed_profile_facade_contract` 和 M4 gate v12 的 `profile_facade_contract` 只代表 Profile remote author loading 已改走 `src/services/desktop.ts` typed wrapper，并保留 browser fetch preview fallback；不得写成 `tauriWailsBridge` 已移除、workbench/core bridge API 已统一或外部证据已完成。
- 涉及 M4 Behavior preset facade 时，`scripts/m4_behavior_preset_facade_gate.ps1` 的 `passed_behavior_preset_facade_contract` 和 M4 gate v13 的 `behavior_preset_facade_contract` 只代表 FingerprintPanel behavior preset loading 已改走 `src/modules/browser/api.ts` 的 `fetchBehaviorPresets` module facade；不得写成 `tauriWailsBridge` 已移除、browser/workbench/core bridge API 已统一或外部证据已完成。
- 涉及 M4 Automation facade 时，`scripts/m4_automation_facade_gate.ps1` 的 `passed_automation_facade_contract` 和 M4 gate v14 的 `automation_facade_contract` 只代表 AutomationPage scheduler/rule calls 已改走 `src/modules/browser/api.ts` module facade，并移除页面级 Wails App/model direct imports；不得写成 `tauriWailsBridge` 已移除、完整 browser/app shell/monitor/workbench/core bridge API 已统一或外部证据已完成。
- 涉及 M4 App shell facade 时，`scripts/m4_app_shell_facade_gate.ps1` 的 `passed_app_shell_facade_contract` 和 M4 gate v15 的 `app_shell_facade_contract` 只代表 `src/App.tsx` 的关闭确认、通知订阅、环境读取、托盘/最小化和退出动作已改走 `src/services/desktop.ts` typed wrappers，并移除 App shell 直接 Wails App/runtime imports；不得写成 `tauriWailsBridge` 已移除、monitor/browser/workbench/core bridge API 已统一或外部证据已完成。
- 涉及 M4 Monitor facade 时，`scripts/m4_monitor_facade_gate.ps1` 的 `passed_monitor_facade_contract` 和 M4 gate v16 的 `monitor_facade_contract` 只代表 `EventMonitorPage` 的实时事件订阅和 event-log history query/count/export/prune 已改走 `src/services/desktop.ts` typed wrappers，并移除页面级 Wails App/model imports 与 `(window as any).runtime` 访问；不得写成 `tauriWailsBridge` 已移除、browser/workbench/core bridge API 已统一或外部证据已完成。
- 涉及 M4 Browser runtime facade 时，`scripts/m4_browser_runtime_facade_gate.ps1` 的 `passed_browser_runtime_facade_contract` 和 M4 gate v17 的 `browser_runtime_facade_contract` 只代表 Browser List/Detail 的 `browser:instance:*` runtime subscriptions 已改走 `src/modules/browser/api.ts` 的 `onBrowserInstanceRuntimeEvents`，并由 API facade 通过 `desktopRuntimeListen` 和 payload normalizer 统一处理；不得写成 `tauriWailsBridge` 已移除、settings/core/proxy/workbench bridge API 已统一或外部证据已完成。
- 涉及 M4 Runtime facade 时，`scripts/m4_runtime_facade_gate.ps1` 的 `passed_runtime_facade_contract` 和 M4 gate v19 的 `runtime_facade_contract` 只代表 Settings/Core/Proxy/Docs/Tutorial 选定页面级 runtime event 与 external URL 调用已改走 `desktopRuntimeListen` / `desktopOpenExternalUrl`；不得写成 `tauriWailsBridge` 已移除、所有 core/proxy/settings API 已统一、真实 headed runtime 已通过或外部证据已完成。
- 涉及 M4 Browser settings/core/proxy facade 时，`scripts/m4_browser_settings_core_proxy_facade_gate.ps1` 的 `passed_browser_settings_core_proxy_facade_contract` 和 M4 gate v29 的 `browser_settings_core_proxy_facade_contract` 只代表 browser Settings/Core/Proxy API 已改走 `src/services/desktop.ts` typed wrappers、`BrowserNativeBindings` 不再 advertise retired settings/core/proxy Wails methods、`tauriWailsBridge` App RPC surface 已显式 allowlist；不得写成 `tauriWailsBridge` 已移除、所有 browser/profile/session bridge API 已统一或外部证据已完成。
- 涉及 M4 UI error boundary 时，M4 gate v26 的 `ui_error_boundary_contract` 只代表 Dashboard、Dashboard API、Settings、Automation、NaturalLanguageTask、TagManagement、RecordingDetailModal、RecordingPanel、LaunchApiDocsPage、BrowserList/Detail/Edit、BrowserSettingsModal、QuickLaunchModal、CoreManagementPage，以及 ProxyPoolPage 这些选定 surfaces 的错误处理已改用 `unknown` guard / typed cast guard；不得写成全仓 generic `any` 已清零、所有 UI 错误边界已统一或 `tauriWailsBridge` 已移除。
- 涉及 M4 bridge compatibility type 时，M4 gate v27 的 `bridge_compat_type_contract` 只代表 `src/services/desktop.ts` 的 `desktopRpc` 参数和 `src/services/tauriWailsBridge.ts` 的 runtime/app proxy 兼容层类型使用命名边界，并阻止 selected bridge files 回退到裸 `unknown[]` / `Promise<unknown>`；不得写成 `tauriWailsBridge` 已移除、所有 browser/settings/core/proxy API 已统一或外部证据已完成。
- 涉及 M4 generic any residue 时，M4 gate v25 的 `generic_any_residue_contract` 只代表 shared Table 泛型约束、ProxyIPHealthResult rawData 和 ProxyPoolPage/types ClashProxy index signature 这组选定 generic/index `any` 已收窄；不得写成仓库每一个显式 `any` 已移除。
- 涉及 M4 safety/logging 时，`safety_logging_contract` passed 只代表 logger 默认敏感字段、写入路径和 Text/JSON formatter 脱敏 source/test contract 已存在；不得写成真实 provider 凭证 smoke 已通过或历史报告已清洗。
- 涉及 M4 total gate 时，当前本机 gate 应只剩 CAPTCHA/SMS/Email credential-backed provider smoke 作为 expected blocker；不得把缺少真实服务商账号伪装成 accepted。跨机器 SessionBundle、外部分发 smoke、release performance budget、AdsPower refresh 和远程代理账号已取消，不再当作 expected blocker。
- 涉及 M5 release health 时，`scripts/release_performance_smoke.ps1` v2 和 `scripts/m5_release_health_gate.ps1` 只保留为本机诊断 report；`passed_with_budget_overrun` 仍禁止写成 release performance green，也不再要求优化到预算 green。
- 涉及 profile-browser comparison 时，先用 Dashboard 的 `采集 WebView` 在真实桌面 WebView 中生成 desktop report，再在同一时间窗口刷新 profile-browser report；`profile_browser_comparison_gate` v3 只有双边同窗且 category 可比才允许 passed。
- 涉及 M15 browser pool 时，`scripts/m15_browser_pool_gate.ps1` 的 `passed_real_pool_process_integration` 代表本机 pool lifecycle、process attach/release cleanup、proxy/session binding cleanup 和最新真实 browser process proof 已组合通过。
- 涉及 observed fingerprint coverage 时，`scripts/observed_fingerprint_coverage_gate.ps1` 只统计 `layer=observed` 且 metadata 完整的 signals；当前 full proof 是 `passed_full_observed_fingerprint_coverage`，`450 / 450`，来源是本机真实浏览器/CDP 采集。
- 涉及 live replay runtime 时，`scripts/live_replay_runtime_gate.ps1` 当前应为 `passed_full_local_replay_runtime`，`461 / 450`，`contractOnly=0`。
- 涉及 **Stealth / 平台 live** 时：`scripts/platform_99_gate.ps1` PGS 离线通过 ≠ XHS 页面可达；`scripts/capability_scenario_suite.ps1` 雷达通过 ≠ 目标站 live；workbench navigate 必须校验 `pageUrl` 非 `chrome-error`；`ERR_NO_SUPPORTED_PROXIES` 表示 Chrome 代理参数错误（常见 `socks5h://`）；UDEAL 等代理 live 常需 `?pp_via_ssh=panda`；详见 `docs/45-stealth-platform-handoff.md`。
- 涉及 M10 headed stability 时，`scripts/m10_headed_stability_gate.ps1` 的 `passed_long_task_stability_coherence` 代表本机 headed_external validation_probe 的 3-run stability/coherence matrix 通过。
- 涉及 M15 browser process 时，`scripts/m15_browser_process_gate.ps1` 的 `passed_real_browser_process_prewarm_cleanup` 代表脚本启动自有本机 browser process、CDP ready、记录 process/RSS snapshot 并清理自有 PID tree。
