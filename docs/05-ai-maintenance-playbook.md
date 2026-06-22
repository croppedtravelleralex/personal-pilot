# AI Maintenance Playbook

## 接手顺序

1. 读 `docs/README.md`。
2. 读 `docs/02-current-state.md`。
3. 读 `docs/final-goal-progress-breakdown.md`。
4. 读 `docs/19-phase-plan-and-scorecard.md`。
5. 读 `docs/03-roadmap.md` 和 `docs/04-improvement-backlog.md`。
6. 如任务涉及 M4-M20 执行、harness、commit 切片或验收边界，读 `docs/40-m4-m20-execution-board.md`。
7. 如任务明确重新开启外部分发、发布说明或 release performance，再读 `docs/24-external-distribution-readiness.md` 和 `docs/release-performance-mitigation-plan.md`；默认本机自用范围下这两份只作历史上下文。
8. 需要执行任务时，再按范围读相关代码和测试。

## 默认事实

- Mainline：`100% / 0% / green`
- Overall：`40% / 60% / yellow`
- Fingerprint：`80` declared controls / `26` runtime projected fields (`25` control-supported + derived `platform`) / `450` taxonomy seed / strict observed coverage `20 / 450` partial
- Behavior：`13` shipped primitives / `8` page archetypes / P10 audit contract / `450` taxonomy seed / local deterministic replay `461 / 450` passed (`326` product-runtime-backed、`135` contract-only); target-site/browser/provider replay pending
- Session：cookie / localStorage / sessionStorage restart continuity 已落地；profile-scoped `SessionBundle` export、import preflight、dry-run、confirmed local restore write path 已落地；跨机器/第二机 profile portability 已按本机自用范围取消

## 汇报规则

- 先说明当前结论，再给证据。
- Mainline remaining `0%` 与 Overall remaining `60%` 必须分开写。
- 不把 AdsPower catch-up、`50+`、`450+` 写成当前 shipped runtime depth。
- 不复活 `77% / 23%` 或 `82% / 18%` 作为 live truth。
- 本机自用范围下，不再把外部分发 smoke、release performance budget、干净 Win11/第二机或跨机器 SessionBundle 写成阻塞项。
- 报告尽量短，优先列 landed result、当前阻塞、下一步。

## 修改规则

- 当前状态变化：更新 `02-current-state.md`。
- 路线变化：更新 `03-roadmap.md`。
- 风险、债务、后续项：更新 `04-improvement-backlog.md`。
- 接手顺序、验证纪律变化：更新 `05-ai-maintenance-playbook.md`。
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
- 涉及 M4 runtime adapter operator loop 时，`runtime_adapter_operator_contract` passed 只代表 Dashboard 可读取历史 release smoke contract，并按证据强度展示 adapter、runner、profile/fingerprint evidence 和 blocker；完整 headed realism、B1-B5、远程代理/TLS、provider、本机 restore 深度和 AdsPower refresh 仍按各自 report 判定。
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
- 涉及 M4 total gate 时，`passed_with_expected_external_blockers` 代表本地 M4 合同可用且没有 failed gate；v29 最新口径是 `passed=24`、`expectedBlocked=2`、`failed=0`。仍不得把 provider、远程代理/TLS、AdsPower refresh、full observed coverage 或 target-site/browser/provider replay 写成已完成。跨机器 SessionBundle、外部分发 smoke 和 release performance budget 已取消，不再当作 expected blocker。
- 涉及 M5 release health 时，`scripts/release_performance_smoke.ps1` v2 和 `scripts/m5_release_health_gate.ps1` 只保留为本机诊断 report；`passed_with_budget_overrun` 仍禁止写成 release performance green，也不再要求优化到预算 green。
- 涉及 profile-browser comparison 时，先用 Dashboard 的 `采集 WebView` 在真实桌面 WebView 中生成 desktop report，再在同一时间窗口刷新 profile-browser report；`profile_browser_comparison_gate` v3 只有双边同窗且 category 可比才允许 passed。
- 涉及 M15 browser pool 时，`scripts/m15_browser_pool_gate.ps1` 的 `passed_pool_lifecycle_harness` 只代表本地 in-memory acquire/release、resource budget、cleanup proof 和 prewarm budget step 合同可验收；不得写成真实 browser process prewarm、CDP ready、RSS/process cleanup proof、proxy/TLS、provider、AdsPower refresh 或完整 `450` coverage 已完成。
- 涉及 observed fingerprint coverage 时，`scripts/observed_fingerprint_coverage_gate.ps1` 只统计 `layer=observed` 且 `collectorScope`、`runtimeAdapter`、`targetProfileBrowser`、`failureReason` metadata 完整的 signals；当前 `partial_observed_fingerprint_coverage` 是 `20 / 450`，taxonomy/materialized contracts 排除在外，不得写成 full observed。
- 涉及 live replay runtime 时，`scripts/live_replay_runtime_gate.ps1` 的 `passed_local_replay_runtime` 只代表本地 deterministic replay harness 已跑过 `461 / 450` taxonomy events；其中 `135` contract-only event 仍需接入 product/browser/provider runtime，不得写成真实目标站 replay。
- 涉及 M10 headed stability 时，`scripts/m10_headed_stability_gate.ps1` 的 `passed_long_task_stability_coherence` 只代表本机 headed_external validation_probe 的 3-run stability/coherence matrix 通过；不得写成完整 headed realism、remote proxy/TLS、provider 或 AdsPower refresh。
- 涉及 M15 browser process 时，`scripts/m15_browser_process_gate.ps1` 的 `passed_real_browser_process_prewarm_cleanup` 只代表脚本启动自有本机 browser process、CDP ready、记录 process/RSS snapshot 并清理自有 PID tree；不得写成完整 browser pool acquire/release、proxy/session cleanup、provider 或 AdsPower refresh。
