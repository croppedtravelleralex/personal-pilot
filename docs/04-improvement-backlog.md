# Improvement Backlog

## Mainline release evidence

> 2026-06-22 本机自用范围修正：release performance 预算达标、外部分发 smoke、干净 Win11/第二机验收、跨机器 SessionBundle portability 已取消。下表保留相关历史 report/脚本作为诊断材料，但不再把它们列为待办、阻塞项或退出条件。

| 领域 | 待改进项 | 当前状态 | 退出条件 |
| --- | --- | --- | --- |
| Proxy / IP | provider-side proxy rotation write | 已闭环 | 真实 provider action、rollback、cooldown、retry 均走 typed native chain |
| Synchronizer | native broadcast write closure；set-main / work-area-aware layout 已落地 | 已闭环 | staged-only 不再是主 operator route，broadcast write 走 typed native chain |
| Recorder / Templates | native-first de-fallback closure | 已闭环 | release-default path 不依赖 fallback |
| Release | Win11 local packaging / operator acceptance polish | 自动化门禁已通过；P13 外部分发 readiness 已改为 historical-only；人工外发 smoke 已取消 | Type check、Vite build、Win11 baseline、Tauri release build、Windows local verify 按本机需要通过并记录；不再要求外部分发人工 smoke |
| Release | 单一 exe / 单一 UI 收敛 | 2026-05-27 已重新构建并覆盖根目录 `personal-pilot-tauri.exe`；2026-06-22 已核验 core bridge / DB 当前真实数据为 `4/69/4`，另有书签 `7`、Camoufox 内核 `1`；已删除第二套 UI、旁路 exe、target GUI exe 产物和仓库内 `gateway-ui/` | 构建、脚本、文档和本机 smoke 只认 `personal-pilot-tauri.exe`；旁路 exe/UI 不再参与构建或验证 |

## Current Remaining

| 领域 | 待改进项 | 当前状态 | 退出条件 |
| --- | --- | --- | --- |
| Validation/Fingerprint | `450` observed coverage | 已完成：`scripts/full_observed_fingerprint_probe.mjs` 真实本机 Chrome/Edge + CDP 采集并展开 `450` 条 observed signal；strict gate 最新状态 `passed_full_observed_fingerprint_coverage` | 变更 collector/taxonomy 后复跑 observed gate |
| Session | 本机 `SessionBundle` restore 和 profile session continuity | profile-scoped export、import preflight、dry-run、confirmed local restore write path 已落地；Settings 已新增本机 operator loop，可触发 export/preflight/dry-run/confirmed restore 并显示 restore plan、blockers、writePerformed 和 restored binding count；P14 portability smoke 已重置为本机 restore smoke；M8 gate 已重置为本机 restore verified；本轮补 AES-GCM bundle encrypt/decrypt 与 runtime mapping contract；最新 portability smoke 为 `local_restore_verified`，最新 M8 为 `passed_local_restore_verified`，M4 gate 的 `session_bundle_operator_contract` 已 passed | 保留本机 restore report；变更 export/preflight/restore/proxy_session_bindings/continuity persistence 后复跑本机 smoke；第二 Win11 target / cross-machine report 已取消 |
| Behavior | `450+` local replay runtime | 已完成：`scripts/live_replay_runtime_gate.ps1` 最新状态 `passed_full_local_replay_runtime`，`461 / 450`，`contractOnly=0` | 变更 taxonomy/runtime backing 后复跑 replay gate |
| Workflow task center | M4 本地任务创建/运行/状态检查闭环 | 2026-06-01 已把 scheduler runtime state 持久化到 SQLite：`status`、`last_run_at`、`last_error`、`retry_count`；Automation 任务页运行后刷新并展示步骤数、上次运行、错误和重试信息；迁移版本 13 可修复旧库缺列；同日 M4.3 已补 typed CDP runner actions：`select`、`dialog`、`download`、`upload`、`iframe`、`tab`，并有 mock CDP 单测覆盖命令与参数形状 | 继续补 workflow execution detail、task run history、cancel/pause operator controls、provider dry-run action、真实目标页 primitive smoke 和 UI/API typed facade；不能把本地 runner primitive 写成真实 provider 或完整 `450+` replay runtime |
| Provider closure | CAPTCHA/SMS/Email 真实账号凭证 smoke | 本地 readiness、dry-run、failure taxonomy、Settings operator surface、report history 已落地；真实账号、余额和 API key 未提供 | 用户提供真实凭证后跑 credential-backed smoke；通过前保持 `blocked_missing_credentials` |
| Runtime | 本机 runtime adapter / M10 / M15 / TLS | 已完成：M10 stability passed；M15 process proof passed；M15 pool/process integration passed；本机 direct TLS/transport observed；runtime adapter local self-use passed | 变更 runtime/pool/transport 后复跑对应 gates |
| Runtime | Camoufox 单任务生产级 Runner | 2026-05-27 已在 `docs/18-external-browser-integration-plan.md` 固化并深化 `90` 分方案；2026-05-28 已把 Camoufox 作为主线内核类型接入 `browser_cores.kind`、Go core DAO/解析/校验、实例启动参数分发、sidecar RPC `BrowserCoreValidateForKind` 和主线 UI 选择；本轮 Rust runner 已从 skeleton 改为最小 CDP runner，支持 open/html/title/final-url/text/validation-probe，输出 stdout/stderr/content preview，任务结束/超时后清理子进程；`scripts/camoufox_smoke.ps1` 已更新为源码级 runtime smoke；2026-05-31 source smoke `data/reports/camoufox-smoke/camoufox-smoke-1780198459643.json` 为 `passed`，真实 Camoufox binary page-open `data/reports/camoufox-binary-task/camoufox-binary-task-smoke-1780198493050.json` 为 `passed` | Camoufox 可配置、可检测、可执行、可取消、可清理、可回显、可诊断；artifact 落盘；基础 profile/proxy 映射；Lightpanda 不回归；默认不常驻、不预热、并发默认 1；Chromium CDP task-run、取消/超时清理、release idle smoke 与 Camoufox task-run 性能报告仍需单独证据或明确例外；Win11/Tauri enforcement 通过 |
| Runtime | release performance mitigation | 已取消为目标；M5 `scripts/release_performance_smoke.ps1` v2、`scripts/m5_release_health_gate.ps1` 和最新 `3899ms` / `433MB` / `9 processes` report 只作为本机启动诊断材料 | 无退出条件；不得写成 performance green，也不得把预算 overrun 当成当前阻塞 |
| Runtime | M15 browser pool/process lifecycle | 已完成：pool slot usage/budget/release cleanup、process attachment、proxy/session binding cleanup proof、真实 browser process CDP/RSS/cleanup report 已合并到 `passed_real_pool_process_integration` | 变更 pool 或 process cleanup 后复跑 M15 gates |
| Fingerprint evidence | M6 same-run desktop/profile comparison | Dashboard 已有 `采集 WebView` action，`profile_browser_comparison_gate.ps1` v3 已有 same-run window；2026-06-20 最新真实 report `data/reports/profile-browser-comparison/profile-browser-comparison-1781941791434.json` 为 `passed`，包含 desktop/profile signal counts、5 个可比 category 和 within-window 判定 | 保留 passed report；下一步扩大可比 category、detector/coherence matrix 和 repeatability sampling，不能把本地 comparison passed 写成 full observed coverage |
| Evidence ops | M4/M10/M13/M15 acceptance harness 与 Dashboard report console | `scripts/m4_acceptance_gate.ps1` 已聚合 live truth、runtime adapter、provider、SessionBundle 和 profile-browser comparison；M4 facade/error/bridge contracts 已进入 evidence history；M5 release health gate 只保留本机启动诊断价值；M8 本机 restore、M10 stability、M15 process/pool、observed coverage、replay runtime 和 runtime adapter local self-use 均已接入 Dashboard/report history | 本机 evidence console 已够当前使用；只剩 provider credential smoke 需要真实账号报告 |
| Safety/logging | 凭证脱敏与配置/加载错误日志边界 | 2026-06-01 已新增 logger 默认敏感字段、free-form 文本脱敏、写入路径脱敏和 Text/JSON formatter 脱敏；Go 单测覆盖 password/token/api_key/authorization/credential/secret/cookie、Bearer/Basic 和 URL userinfo 密码 | 后续新增日志字段、报告 schema 或 provider 响应体时必须复用 logger 脱敏规则；真实凭证 smoke 和历史报告清洗仍需单独证据 |
| UI consolidation | 第二套 UI / 旁路 exe 清理与主线 API 收口 | 根目录旁路 exe、旧 release exe、第二套 Tauri/Vite console UI 和仓库内 `gateway-ui/` 已清理；Gateway dashboard UI 如需使用必须通过 `GATEWAY_UI_DIR` 指向外部目录，仓库默认不再指向内置 UI；2026-05-27 已把 `dashboard/profile/monitor` 收口到 `services/desktop.ts` typed wrapper 或模块 facade，并为事件监控 history 补上 `300ms` debounce 与 stale-result 保护；M4.8 已把 synchronizer high-traffic bridge DTO 移到 `src/types/desktop.ts`，`services/desktop.ts` 不再对 list/arrange/log/tasks 暴露 `unknown[]`，`synchronizer/api.ts` 移除对应 Promise cast；同日 browser API 新增 `BrowserNativeBindings` / `getWindowGoApp()`，移除 browser Wails binding 的 `any` 入口；2026-06-02 已把 browser runtime event payload handler 改为 `BrowserRuntimeEventPayload` + `normalizeBrowserRuntimeEventPayload(payload: unknown)`，并把 `normalizeLaunchServerInfo` / `normalizeRecordingDetail` 输入从 `any` 收窄到 `unknown`；同日 Dashboard API stats/license/config/CD key 调用已改走 `src/services/desktop.ts` typed wrappers，不再动态 import Wails `App` 或使用 `const bindings: any`；Settings backup initialize/export/import 与 Browser logs read/clear 也已改走 `src/services/desktop.ts` typed wrappers，同时保留 destructive preflight、native dialog 和 Wails-compatible proxy；Profile remote author loading 已改走 `fetchRemoteAuthorProfileFromDesktop` typed wrapper，并保留 browser fetch preview fallback；FingerprintPanel behavior preset loading 已改走 `fetchBehaviorPresets` browser module facade，不再直接 import Wails `BehaviorPresetList`；AutomationPage scheduler/rule calls 已改走 browser module facade，不再直接 import Wails App bindings 或 `backend` model constructors；App shell close confirmation、notification subscriptions、environment lookup、tray/minimize 和 quit actions 已改走 `src/services/desktop.ts` typed wrappers，不再在 `src/App.tsx` 直接 import Wails App/runtime；Browser List/Detail runtime subscriptions 已改走 browser module facade；Workbench detection/UI/report DTO 与 `BrowserInstanceStatus` DTO 已进入 shared desktop types；Settings/Core/Proxy/Docs/Tutorial 页面级 runtime event / external URL 调用已改走 `desktopRuntimeListen` / `desktopOpenExternalUrl`；2026-06-22 browser Settings/Core/Proxy API 已改走 `src/services/desktop.ts` typed wrappers，`tauriWailsBridge` 改为显式 App RPC allowlist | 后续深度测试确认主线 UI 操作、Wails 兼容 API、report/history 和 Settings/runtime 细节无回归；继续把剩余 browser/profile/session 兼容路径和 `tauriWailsBridge` 逐步显式化/缩小，但不得写成 bridge 已移除 |
| Benchmark | AdsPower boundary refresh | 当前本机自用范围已取消 | 除非用户明确重开，否则不作为待办或阻塞 |

Historical correction 2026-06-11:

- Evidence ops: M4 Monitor facade 已新增 `scripts/m4_monitor_facade_gate.ps1`、`m4-monitor-facade` report kind、Dashboard `M4 Monitor` row 和 M4 gate v16 `monitor_facade_contract`；最新 M4 gate 仍为 `passed_with_expected_external_blockers`，`passed=15`、`expectedBlocked=4`、`failed=0`。
- UI consolidation: 上表关于 2026-05-27 `monitor` 已完全收口与 `300ms` debounce/stale-result 的表述不再作为 live truth；当前代码事实是 2026-06-11 才把 `EventMonitorPage` 实时订阅和 event-log history CRUD 改走 `src/services/desktop.ts` typed wrappers，并修正 paused 状态闭包。2026-06-12 已继续补 Browser runtime facade、Workbench DTO facade 和页面级 Runtime facade；2026-06-22 已补 browser settings/core/proxy facade gate。剩余 facade shrink 主要在剩余 browser/profile/session 兼容路径和过渡 `tauriWailsBridge` 兼容层。

Historical correction 2026-06-12:

- Evidence ops: M4 Browser runtime facade 已新增 `scripts/m4_browser_runtime_facade_gate.ps1`、`m4-browser-runtime-facade` report kind、Dashboard `M4 Browser Runtime` row 和 M4 gate v17 `browser_runtime_facade_contract`；最新 M4 gate 仍为 `passed_with_expected_external_blockers`，`passed=16`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 typed facade shrink 已扩展到 Workbench DTO：`DesktopCoreWorkbenchDetectionResult`、`DesktopCoreWorkbenchUiState`、`DesktopCoreWorkbenchDetectorSite` 进入共享 desktop types，M4 gate v18 仍为 `passed_with_expected_external_blockers`，`passed=16`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 runtime facade 已新增 `scripts/m4_runtime_facade_gate.ps1`、`m4-runtime-facade` report kind、Dashboard `M4 Runtime` row 和 M4 gate v19 `runtime_facade_contract`；最新 M4 gate 为 `passed_with_expected_external_blockers`，`passed=17`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 UI error boundary contract 已纳入 M4 gate v21，选定 Dashboard/Settings/Automation/Tag/Recording/Docs surfaces 使用 `messageFromUnknownError(error: unknown)` 和 typed markdown/audio guards；最新 M4 gate 为 `passed_with_expected_external_blockers`，`passed=18`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 UI error boundary contract 已扩展到 v22，BrowserList/Detail/Edit、BrowserSettingsModal、QuickLaunchModal 的实例操作错误路径也纳入 selected surface 检查；gate 现在检查 Browser instance action-specific fallback marker，并阻止 BrowserList 启动 mojibake 文案回归；最新 M4 gate 仍为 `passed_with_expected_external_blockers`，`passed=18`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 UI error boundary contract 已扩展到 v23，CoreManagementPage 的内核操作和设置保存错误路径纳入 selected surface 检查；最新 M4 gate 仍为 `passed_with_expected_external_blockers`，`passed=18`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 UI error boundary contract 已扩展到 v24，ProxyPoolPage 的代理订阅/导入/保存/删除/修复错误路径纳入 selected surface 检查，并阻止 selected files 回退到 `[key: string]: any`；最新 M4 gate 仍为 `passed_with_expected_external_blockers`，`passed=18`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 generic any residue contract 已新增到 v25，shared Table 泛型约束、ProxyIPHealthResult rawData 和 ProxyPoolPage/types ClashProxy index signature 的选定 generic/index `any` 已收窄；最新 M4 gate 为 `passed_with_expected_external_blockers`，`passed=19`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 UI error boundary contract 已扩展到 v26，RecordingPanel 的录制/回放/导入导出/复制/重命名/接管/清理错误路径纳入 selected surface 检查，回放失败 mojibake 文案已修正；最新 M4 gate 仍为 `passed_with_expected_external_blockers`，`passed=19`、`expectedBlocked=4`、`failed=0`。
- Evidence ops: M4 bridge compatibility type contract 已新增到 v27，`desktopRpc` 参数通道与 `tauriWailsBridge` runtime/app proxy 兼容类型使用命名边界，并阻止 selected bridge files 回退到裸 `unknown[]` / `Promise<unknown>`；最新 M4 gate 仍为 `passed_with_expected_external_blockers`，`passed=20`、`expectedBlocked=4`、`failed=0`。
- UI consolidation: `BrowserListPage` / `BrowserDetailPage` 的 browser instance runtime 事件订阅已改走 `src/modules/browser/api.ts` 的 `onBrowserInstanceRuntimeEvents`，不再直接 import/call Wails `EventsOn`；browser runtime payload normalizer 仍在 API facade 内复用。`src/services/desktop.ts` 的 Workbench detection results / detector sites / UI state / detector run / fingerprint health / fingerprint snapshot / identity report / browser instance status API 也不再暴露 `unknown[]` 或裸 `unknown`。`SettingsPage` / `CoreManagementPage` / `ProxyPickerModal` / `ProxyPoolPage` / `LaunchApiDocsPage` / `UsageTutorialPage` 不再直接 import `wailsjs/runtime/runtime`。2026-06-22 browser Settings/Core/Proxy API 已改走 typed desktop wrappers；剩余 facade shrink 主要在剩余 browser/profile/session 兼容路径和过渡 `tauriWailsBridge` 兼容层。
- UI consolidation: Dashboard、Dashboard API、Settings、Automation、NaturalLanguageTask、TagManagement、RecordingDetailModal、LaunchApiDocsPage 的本轮错误边界已从弱 `any`/cast 收窄；仓库内其他 legacy 页面仍有 `catch (...: any)`，后续继续按小批切片处理，不能把本轮写成全仓清零。
- UI consolidation: BrowserListPage、BrowserDetailPage、BrowserEditPage、BrowserSettingsModal、QuickLaunchModal、CoreManagementPage、ProxyPoolPage 的本轮错误边界已从弱 `any` 收窄，BrowserList 启动成功/失败乱码已修正；本轮还收窄了选定 generic/index `any`，但仍不代表全仓显式 `any` 清零。
- UI consolidation: RecordingPanel 的本轮错误边界已从未显式类型的 catch 收窄到 `unknown`，并复用共享错误文案 helper；仍不代表 Synchronizer、desktop service 或 `tauriWailsBridge` 兼容层已完成。
- UI consolidation: `desktop.ts` / `tauriWailsBridge.ts` 的桥接参数和兼容 App proxy 类型已显式化为命名边界；2026-06-20/22 已继续处理 browser `window.go` fallback 和 settings/core/proxy facade；仍不代表 `tauriWailsBridge` 已移除，下一步继续处理剩余兼容路径。

Current correction 2026-06-22:

- Evidence ops: 2026-06-22 早期 M4 gate v28 report `data/reports/m4-acceptance/m4-acceptance-gate-1782111648992.json` 为 `passed_with_expected_external_blockers`，summary `passed=23`、`expectedBlocked=2`、`failed=0`；profile/browser comparison report `data/reports/profile-browser-comparison/profile-browser-comparison-1781941791434.json` 已 `passed`，SessionBundle local restore report `data/reports/session-portability/session-bundle-portability-smoke-1782111637335.json` 已 `local_restore_verified`，二者不再作为 M4 expected blocker。最新 M4 口径见下方 v29 correction。
- Runtime: 最新 release smoke `data/reports/release-smoke/release-performance-smoke-1781933571814.json` 仍为 `warning` / `over_budget`，实测 `3899ms` cold start、`433MB` idle RSS、`9` processes；该报告只作历史诊断，继续禁止写成 performance green，也不再作为当前优化目标。
- Release data: 当前 `data/app.db` 为 profiles `4`、proxies `69`、cores `4`、bookmarks `7`、Camoufox cores `1`；历史 `93` 或 `95` 代理数只作为旧快照。
- Scope: 外部分发 smoke、干净 Win11/第二机验收和跨机器 portability 不再进入 backlog 退出条件；后续只维护本机 operator 能力。

Superseded correction 2026-06-22 M4-M20 local evidence:

- 这段保留 2026-06-22 早期状态。下方 `local self-use closure` 是当前事实。

Current correction 2026-06-22 local self-use closure:

- Fingerprint: strict observed coverage 已由本机 Chrome/Edge + CDP full probe 推进到 `passed_full_observed_fingerprint_coverage`，`450 / 450`。
- Behavior: local replay runtime 已推进到 `passed_full_local_replay_runtime`，`461 / 450`，`contractOnly=0`。
- Runtime: M10 stability、M15 real process proof、M15 pool/process integration、本机 direct TLS/transport 和 runtime adapter local self-use 已通过。
- Scope: remote proxy account proof、AdsPower refresh、release performance budget、external distribution、second-machine/cross-machine 全部不是当前待办。
- Remaining: 只剩 CAPTCHA/SMS/Email 真实账号凭证 smoke。

## 固定风险

- 把 `80` declared controls 误报成 `80` runtime-applied fields。
- 把 `450+` fingerprint / event target、materialized contract 误报成 observed/live runtime 已交付。
- 把 mock / fallback / staged 默认路径算作 delivery closure。
- 把历史比例 `77% / 23%` 或 `82% / 18%` (historical-only) 覆盖成当前 live truth。
- 在 Mainline closeout 阶段引入 Overall 级大范围重构。
- P5/P6 已通过后仍把 WebRTC/audio warning 或 canvas failure 写成全部成功，或丢失 `failureReason`。
- 把第二套 Tauri UI 的功能当成 `personal-pilot-tauri.exe` 主线已交付。
- 删除旧 UI 前没有确认功能是否已迁回截图主线 UI。
- 把已取消的外部分发、release performance budget、第二机/cross-machine 项重新写成当前阻塞。

## 维护规则

新增 backlog 时必须标清属于 Mainline release evidence 还是 Overall `60%`。不能确定归属时先放 Overall，等有代码证据后再提升到 Mainline release gate。
