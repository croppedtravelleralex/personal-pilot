# Roadmap

## Done：Mainline 已闭环 (原 `7%`)

目标达成。从 `95% / 7% / green` 推到 `100% / 0% / green`。

1. Proxy / IP closeout — **已完成**
   - provider-side proxy rotation write 已交付：真实 HTTP POST/PUT/PATCH 引擎
   - sticky/residency 语义绑定到真实 provider action
   - 失败 rollback、cooldown、retry 已类型化

2. Synchronizer native closure — **已完成**
   - native broadcast write path 已落地：物理 `SetWindowPos` 窗口排布
   - staged-only 默认路径已移出主 operator route
   - live read/focus/set-main/layout 已落地未倒退

3. Recorder / Templates native closure — **已完成**
   - fallback dependence 已收敛
   - recorder capture、template compile/replay 已 native-first
   - fallback 只作为异常恢复，不作为 release-default route

4. Mainline release gate — **已完成自动化门禁**
   - 2026-05-23 曾通过 `scripts/windows_local_verify.ps1 -SkipContinuityTest`
   - 该 gate 覆盖 typecheck、Vite build、Win11 baseline、Rust lib/full tests、Tauri release build
   - 注意：这属于历史 Tauri/PersonaPilot 0.1.0 路线证据；2026-05-25 后主线 gate 必须改为验证 `personal-pilot-tauri.exe` 1.1.0
   - P13 已新增 `docs/24-external-distribution-readiness.md`，后续发布前按新的单入口清单追加人工 operator smoke 和 continuity integration test

## Now：Overall remaining `60%`

目标：从 closeout-ready desktop app 走向完整平台能力。此轨道已从 `30% / 70% / yellow` 推进到 `40% / 60% / yellow`，原因是 P14 新增了 release/provider/session/taxonomy 的可重复 evidence 入口和 machine-readable taxonomy seed；这些仍不得冒充完整生产闭环。

0. Single UI / exe convergence
   - 用户已确认唯一主线 UI 是截图所示 `personal-pilot` 1.1.0 / Wails v2 + React UI。
   - 唯一用户入口为 `D:\SelfMadeTool\personal-pilot\personal-pilot-tauri.exe`。
   - 2026-05-27 已把 Tauri release 产物名、产品名、根目录验收入口和启动 UI 壳收敛到该 exe，并已核验 `tauriWailsBridge` / core bridge 可读取当前真实 `4/93/3` 数据。
   - 第二套 Tauri/Vite 控制台 UI 源码已删除；后续如需吸收其功能，必须重新实现在 `src/modules/**` 主线 UI 内。
   - `PersonaPilot.exe`、`portable.exe`、`persona-pilot-desktop.exe`、`src-tauri/target/release/*.exe`、安装包 exe、根目录工具 exe、`gateway-ui/` 和未接主线的 UI 代码已清理，不得再接收新功能。
   - 构建后只允许持久保留根目录 `personal-pilot-tauri.exe`；`bin/` sidecar、代理工具和 `chrome/` 浏览器引擎是运行依赖，不是用户入口。

1. Validation foundation
   - Validation Board 前端 MVP 已落地。
   - 形成 detector / leak / DNS / WebRTC / canvas / audio / worker / transport evidence。
   - 区分 declared / applied / observed。
   - DNS/transport 首批 observed native collector、本地 JSON report、report history、profile-level evidence export 已接入。
   - desktop WebView scoped WebRTC/canvas/audio/storage probes 已接入，用于记录当前桌面壳浏览器 API 能力；不等同于 profile browser runtime probe。
   - profile browser runtime `validation_probe` action 已接入 runner；Lightpanda/CDP 可用时可生成 WebRTC/canvas/audio/leak runtime scoped signals，FakeRunner 只生成 warning stub。
   - P5 已新增 `validation_lightpanda_smoke` 可复跑入口，并通过 WSL2 Lightpanda nightly 真实 CDP 复跑，report `status=passed`。
   - P6 已统一 desktop WebView、profile browser、FakeRunner/native validation signals 的 explicit evidence metadata：`collectorScope`、`runtimeAdapter`、`targetProfileBrowser`、`failureReason`；旧报告读取时从 legacy `detail` 补齐字段。
   - P14 已新增 external distribution、release performance、provider acceptance、SessionBundle portability 和 taxonomy audit 脚本入口。

2. Fingerprint runtime depth
   - 已从 `80` declared controls 和 `12` runtime projected fields 推进到 `26` runtime projected fields（`25` control-supported + derived `platform`）。
   - P5 真实 Lightpanda/CDP smoke 已通过，P6 evidence schema 已收敛，P7 fingerprint observation audit 已接入 Validation Board；P14 已新增 `docs/taxonomy/fingerprint-signal-taxonomy.json`；2026-05-28 已新增 Go 端环境注入编译器和 `Page.addScriptToEvaluateOnNewDocument` CDP 入口，覆盖 browser API/canvas/timezone/WebGL/media devices 基础 hook；`roadmap_evidence_smoke` 已证明本机实现覆盖，`profile_browser_environment_probe.mjs` 已用真实 Chromium profile-browser 观测 1.2/1.3/1.4 通过。本轮继续把环境注入接入实例启动流程，并把 environment audit 扩到 8 个注入族；`taxonomy_coverage_materialize.ps1` 已物化 `450 / 450` 条 fingerprint signal contract；下一步继续加深真实采集，不把 taxonomy seed、materialized contract 或 projected/applied fields 报成 full observed coverage。
   - 保持 control / derived / observation layers 分离。

3. Session / proxy orchestration
   - 已将 restart continuity 推进到 profile-scoped `SessionBundle` export、import preflight、dry-run、confirmed local restore write path。
   - P14 已新增 `scripts/session_bundle_portability_smoke.ps1` 记录本机 contract 和跨机器 manual steps。
   - 2026-06-02 M8 已新增 `scripts/m8_session_handoff_gate.ps1`，生成 `m8-session-handoff` report，并在 Dashboard 显示 `M8 Handoff`；当前状态 `passed_handoff_package_ready` 只代表 runbook、Settings operator loop、desktop typed contract 和本地 portability report 可交接，跨机器通过仍必须等第二 Win11 target report。
   - 下一步补齐跨机器 profile portability 验收、lease / cooldown / health / rollback。

4. Behavior and automation depth
   - 已新增 P10 behavior audit contract：`13` shipped primitives、`8` page archetypes、workflow graph、debug trace、manual gate、recovery semantics、`450+` target-only 边界可在 Automation surface 查看。
   - P14 已新增 `docs/taxonomy/behavior-event-taxonomy.json` 和 Automation taxonomy seed 可见性；2026-05-28 `taxonomy_coverage_materialize.ps1` 已物化 `461 / 450` 条 behavior replay contract。
   - 2026-05-28 已落地 Phase 2 P0 的 Go humanize 基础模型：Fitts Law 轨迹、粉噪、四段式点击、双击、拖拽和右键菜单计划，并以 Go 单测覆盖。
   - 2026-05-28 已把 shipped primitives 的 workflow graph/debug trace 状态前推到 runtime evidence backed。
   - 下一步才是把 materialized contract 扩展为真实 replayable `450+` live event runtime。
   - CAPTCHA/SMS/Email 已有 production readiness contract、acceptance checklist、Settings operator surface 和 M4.4 dry-run/failure taxonomy 可见性；下一步是真实 manager wiring、CDP detect/fill 和 provider acceptance。

5. Runtime adapter and external integration
   - Runtime adapter / release smoke contract 已接入，覆盖 Fake、Lightpanda、headed_external 边界和 release artifact 检查；`headed_external` 泛化 source contract 已落地，且 2026-05-30 已用仓库内 fingerprint Chromium 跑通真实 `get_title https://example.com` 单任务，report `data/reports/headed-external-smoke/headed-external-smoke-1780109927517.json` 为 `passed_real_binary_task`；同日继续跑通真实 `validation_probe`，report `data/reports/headed-external-smoke/headed-external-smoke-1780121855313.json` 为 `passed_real_binary_validation_probe`，包含 9 个 profile-browser validation signals。2026-05-31 已修复 headed_external evidence ranked selection 和 smoke stdout 污染；2026-06-02 最新 headed repeatability report `data/reports/headed-external-smoke/headed-external-smoke-1780375482756.json` 为 `passed_real_binary_repeatability`，M10 gate `data/reports/m10-headed-repeatability/m10-headed-repeatability-gate-1780377051356.json` 为 `passed_repeatability_partial_coherence`。
   - P11 已新增 release measurement target vs measured pending 字段，并在 Overview 暴露 adapter boundary；P14 已新增 `scripts/release_performance_smoke.ps1` 用 release exe 生成 cold start/RSS/process count report。
   - 下一步只吸收高 ROI 外部浏览器思路。
   - 不把主仓库变成 Chromium / Firefox fork host。
   - P12 已把 AdsPower benchmark refresh 固定为 boundary guard；P21 新增 runtime adapter evidence gate report：等 B1-B5 有新证据后再重算评分。
   - Camoufox 下一步按 `docs/18-external-browser-integration-plan.md` 的 `90` 分方案推进：2026-05-28 已先把它作为主线 `browser_cores.kind=camoufox` 内核类型接入 Go core manager、SQLite、sidecar RPC、实例启动参数分发和主线 UI 选择；本轮已把 Rust runner 从 skeleton 推进为最小 CDP runner，支持 open/html/text/title/final-url/validation-probe 和 stdout/stderr/content preview；真实 Camoufox binary 已通过 Firefox-compatible headless screenshot 打开 `https://example.com` 并产出持久 PNG。remote server、browser pool 和深度指纹拟真不进入第一主链；Chromium `/json/version` CDP attach 不宣称通过。
   - Phase 6 传输一致性已从死骨架推进到 Xray/SingBox 安全 ALPN 合并和 runtime family explain metadata；真实 Xray/SingBox 本地二进制配置验证已通过：Xray `run -test` 返回 `Configuration OK`，SingBox `check -c` exit code 为 `0`。这仍只是 direct outbound 配置接受性，不是远程代理出站或 TLS/HTTP2 指纹观测。
   - 2026-06-02 已刷新 runtime adapter gate：Camoufox source smoke 与 binary page-open passed，headed_external real binary validation probe / repeatability 被 ranked selection 正确选中；最新 gate `data/reports/runtime-adapter/runtime-adapter-evidence-gate-1780375565693.json` 仍 blocked，但 `runtimeAdapterEvidence=partial_real_binary_repeatability_recorded`、`fingerprintRuntimeDepth=partial_headed_profile_browser_repeatability_observed`、`signalCount=9`；profile-browser comparison 仍因 desktop WebView report 未实际采集而 blocked，provider 为 `blocked_missing_credentials`，远程代理/TLS 为 `blocked_remote_proxy_required`，SessionBundle 为 `local_contract_passed` 但跨机器 evidence pending；完整 runtime adapter / B1-B5 evidence、external distribution、AdsPower refresh 仍 blocked/deferred。
   - 2026-05-31 已新增 M4-M20 execution board 和 M4 acceptance harness：`scripts/m4_acceptance_gate.ps1` 会把外部缺口分类为 `expected_blocked`，最新 M4 report 为 `expected_blocked` 且 `failed=0`；Dashboard 已展示 M4/性能/adapter/对比/provider/session/taxonomy/external reports。2026-06-01 已推进 M4 workflow task center：scheduler task status/last run/error/retry 进入 SQLite 持久化，Automation 页立即执行后刷新并展示运行状态；同日 M4.3 typed CDP primitives v1 已补 `select/dialog/download/upload/iframe/tab` runner actions 和 mock CDP 测试；M4.4 provider dry-run/operator closure 已补 v3 preflight、failure taxonomy、Settings 可见性和 M4 gate 本地检查；M4.5/M6 已补 Dashboard `采集 WebView` 入口和 profile-browser comparison v3 同窗校验；M4.6 已补 Settings SessionBundle 本机 export/preflight/dry-run/confirmed local restore operator loop；M4.7 已补 Dashboard Runtime Adapter 卡，从 release smoke contract 读取并按证据强度排序展示 adapter、runner、profile/fingerprint evidence 和 blocker；M4.8 已把 synchronizer high-traffic bridge 从 `unknown[]`/module cast 收窄到共享 DTO，并把 browser API 的 Wails binding 动态入口收敛为 `BrowserNativeBindings` / `getWindowGoApp()` 类型合同；2026-06-02 又新增 browser runtime event payload schema normalizer、`scripts/m4_browser_payload_schema_gate.ps1` 和 Dashboard `M4 Payload` row；同日继续把 Dashboard API stats/license/config/CD key 调用迁到 `src/services/desktop.ts` typed wrappers，新增 `scripts/m4_dashboard_facade_gate.ps1` 和 Dashboard `M4 Dashboard` row；随后把 Settings backup initialize/export/import 和 Browser logs read/clear 迁到 `src/services/desktop.ts` typed wrappers，新增 `scripts/m4_settings_logs_facade_gate.ps1` 和 Dashboard `M4 Settings/Logs` row；之后把 Profile remote author loading 迁到 `fetchRemoteAuthorProfileFromDesktop` typed wrapper，新增 `scripts/m4_profile_facade_gate.ps1` 和 Dashboard `M4 Profile` row；接着把 FingerprintPanel behavior preset loading 改走 `fetchBehaviorPresets` browser module facade，新增 `scripts/m4_behavior_preset_facade_gate.ps1` 和 Dashboard `M4 Behavior` row；本轮又把 AutomationPage scheduler/rule 任务列表/创建/删除/立即运行与响应规则列表/创建/删除/开关/测试触发改走 browser module facade，新增 `scripts/m4_automation_facade_gate.ps1` 和 Dashboard `M4 Automation` row；2026-06-11 继续把 `src/App.tsx` app shell 关闭确认、通知订阅、环境读取、托盘隐藏/最小化和退出动作迁到 `src/services/desktop.ts` typed wrappers，新增 `scripts/m4_app_shell_facade_gate.ps1` 和 Dashboard `M4 App Shell` row；M4 gate v15 已纳入 `browser_payload_schema_contract`、`dashboard_facade_contract`、`settings_logs_facade_contract`、`profile_facade_contract`、`behavior_preset_facade_contract`、`automation_facade_contract` 与 `app_shell_facade_contract`，最新 report 仍应保持 `passed_with_expected_external_blockers`、`failed=0`；M4.9 已新增 logger 脱敏器、写入/formatter 脱敏路径和 source/test gate；M4.10 已把 M4 总闸升级为 `passed_with_expected_external_blockers` operator 状态，并在 Dashboard evidence history 中保留 expected external blocker 语义。M5 已补 release performance/health v2：release smoke report 输出 per-metric budget、10% drift reason、healthSummary 和 mitigationHints，`scripts/m5_release_health_gate.ps1` 最新为 `passed_with_budget_overrun`，Dashboard evidence history 可见 M5 Health。M13 已补 evidence trend/risk/failure taxonomy/richer diff，为同 kind report 显示 latest-vs-previous status/failureReason/category/risk diff 和 `failureReasonCategory`，但不改变 gate 口径。M6 已补 profile/browser comparison report summary/next action，把 same-run window、信号数和缺失类别展示到 evidence history；M8 已补 `scripts/m8_session_handoff_gate.ps1` 和 Dashboard M8 Handoff report row；M10 已补 `scripts/m10_headed_repeatability_gate.ps1` 和 Dashboard M10 Repeatability report row；M15 已补本地 browser pool lifecycle harness、`scripts/m15_browser_pool_gate.ps1` 和 Dashboard M15 Pool report row。实际 desktop/profile 同窗采集、M8 第二机运行、长任务稳定性、真实 browser process prewarm/RSS cleanup、远程代理/TLS 和完整 headed realism 仍未完成。性能仍 over budget，不能写成 green；M8 target run、真实 headed coherence matrix、M15 real process pool proof 和剩余 monitor/browser runtime/workbench/core bridge shrink 继续按 M6/M8/M10/M15/M4 推进。
   - 2026-06-11 current update：M4 Monitor facade 已把 `EventMonitorPage` 实时事件订阅与 event-log history query/count/export/prune 迁到 `src/services/desktop.ts` typed wrappers，新增 `scripts/m4_monitor_facade_gate.ps1`、`m4-monitor-facade` evidence kind 和 Dashboard `M4 Monitor` row；M4 gate v16 已纳入 `monitor_facade_contract`，最新 report 为 `passed_with_expected_external_blockers`、`passed=15`、`expectedBlocked=4`、`failed=0`。剩余 facade shrink 主要在 browser runtime/workbench/core bridge 与过渡 `tauriWailsBridge`。
   - 2026-06-12 current update：M4 Browser runtime facade 已把 Browser List/Detail 的 `browser:instance:*` runtime subscriptions 迁到 `src/modules/browser/api.ts` 的 `onBrowserInstanceRuntimeEvents`，API 内部统一通过 `desktopRuntimeListen` 订阅并复用 payload normalizer；新增 `scripts/m4_browser_runtime_facade_gate.ps1`、`m4-browser-runtime-facade` evidence kind 和 Dashboard `M4 Browser Runtime` row；M4 gate v17 已纳入 `browser_runtime_facade_contract`，最新复跑为 `passed_with_expected_external_blockers`、`passed=16`、`expectedBlocked=4`、`failed=0`。剩余 facade shrink 主要在 settings/core/proxy/workbench bridge 与过渡 `tauriWailsBridge`。
   - 2026-06-12 current update：M4 Workbench DTO shrink 已把 `src/services/desktop.ts` 的 Workbench detection results / detector sites / UI state / detector run API 从 `unknown[]` 或裸 `unknown` 收窄到 `src/types/desktop.ts` 共享 DTO，并更新 `src/modules/synchronizer/api.ts` 去掉对应 `Array.isArray(results)` 退化；M4 gate v18 已把 Workbench DTO 纳入 `typed_facade_shrink_contract`，最新复跑为 `passed_with_expected_external_blockers`、`passed=16`、`expectedBlocked=4`、`failed=0`。剩余 facade shrink 主要在 settings/core/proxy bridge 与过渡 `tauriWailsBridge`。
   - 2026-06-12 current update：M4 Runtime facade 已把 `SettingsPage` backup export/import progress、`CoreManagementPage` download progress / external URL、`ProxyPickerModal` / `ProxyPoolPage` proxy result events、`LaunchApiDocsPage` / `UsageTutorialPage` external URL calls 迁到 `src/services/desktop.ts` 的 `desktopRuntimeListen` / `desktopOpenExternalUrl`；新增 `scripts/m4_runtime_facade_gate.ps1`、`m4-runtime-facade` evidence kind 和 Dashboard `M4 Runtime` row；M4 gate v19 已纳入 `runtime_facade_contract`，最新复跑为 `passed_with_expected_external_blockers`、`passed=17`、`expectedBlocked=4`、`failed=0`。剩余 facade shrink 主要在更深的 settings/core/proxy bridge API 与过渡 `tauriWailsBridge` 兼容层。
   - 2026-06-12 current update：M4 Workbench/report DTO shrink 继续推进，`BrowserInstanceStatus`、`WorkbenchFingerprintHealthProfile`、`WorkbenchFingerprintProfile`、`IdentityReportProfile` 的 desktop service 返回值已从裸 `unknown` 收窄到共享 DTO，`src/modules/synchronizer/api.ts` 移除对应 status/report cast；M4 gate v20 扩展 `typed_facade_shrink_contract` 后仍为 `passed_with_expected_external_blockers`、`passed=17`、`expectedBlocked=4`、`failed=0`。剩余 facade shrink 仍主要在更深的 settings/core/proxy bridge API 和 `tauriWailsBridge` 兼容层。
   - 2026-06-12 current update：M4 UI error boundary shrink 已把 Dashboard、Dashboard API、Settings、Automation、NaturalLanguageTask、TagManagement、RecordingDetailModal 的选定错误处理从 `catch (...: any)` 收窄到 `unknown` guard，并把 Dashboard WebView audio fallback / Launch API markdown code block 中的 `any` cast 改为 typed guard；M4 gate v21 新增 `ui_error_boundary_contract`，最新复跑为 `passed_with_expected_external_blockers`、`passed=18`、`expectedBlocked=4`、`failed=0`。该切片不代表全仓 legacy UI any-catch 已清零，剩余继续按小批次推进。
   - 2026-06-12 current update：M4 Browser instance UI error boundary shrink 已把 BrowserList/Detail/Edit、BrowserSettingsModal、QuickLaunchModal 的实例操作错误路径从 `catch (...: any)` 收窄到 `unknown` guard，保留 action feedback 语义，并修正 BrowserList 启动成功/失败 mojibake 文案；M4 gate v22 扩展同一 `ui_error_boundary_contract` 后仍为 `passed_with_expected_external_blockers`、`passed=18`、`expectedBlocked=4`、`failed=0`。该切片不代表 Proxy/Core 大页面或全仓 legacy UI any-catch 已清零。
   - 2026-06-12 current update：M4 Core management UI error boundary shrink 已把 CoreManagementPage 的打开目录、扫描、保存/删除、设默认、下载启动和全局设置保存错误路径从 `catch (...: any)` 收窄到 `unknown` guard；M4 gate v23 扩展同一 `ui_error_boundary_contract` 后仍为 `passed_with_expected_external_blockers`、`passed=18`、`expectedBlocked=4`、`failed=0`。该切片不代表 ProxyPoolPage 或全仓 legacy UI any-catch 已清零。
   - 2026-06-12 current update：M4 Proxy pool UI error boundary shrink 已把 ProxyPoolPage 的订阅刷新、删除、保存、URL 获取 fallback、订阅导入、解析、导入确认和名称修复错误路径从 `catch (...: any)` 收窄到 `unknown` guard，并把 Clash proxy index signature 从 `any` 收窄到 `unknown`；M4 gate v24 扩展同一 `ui_error_boundary_contract` 后仍为 `passed_with_expected_external_blockers`、`passed=18`、`expectedBlocked=4`、`failed=0`。该切片不代表全仓 generic `Record<string, any>` 已清零。
   - 2026-06-12 current update：M4 generic any residue shrink 已把 shared Table 泛型约束、ProxyIPHealthResult rawData 和 ProxyPoolPage/types ClashProxy index signature 的选定 generic/index `any` 收窄到 `object` / `unknown`；M4 gate v25 新增 `generic_any_residue_contract` 后仍为 `passed_with_expected_external_blockers`、`passed=19`、`expectedBlocked=4`、`failed=0`。该切片不代表仓库每一个显式 `any` 已移除。

6. Retire duplicate UI paths
   - 2026-05-27 已删除第二套 Tauri/Vite 控制台 UI 源码、根目录旁路 exe、`src-tauri/target/release` 持久 GUI exe 和仓库内 `gateway-ui` 静态 UI。
   - Gateway dashboard 静态 UI 不再作为仓库内置资产；如需临时使用，必须通过 `GATEWAY_UI_DIR` 显式指向外部目录。
   - 第二套 UI 不再作为发布入口，也不保留源码目录；后续只允许从历史提交中取设计参考。
- 下一步不再做旧 UI 复活式迁移；改为在截图主线 UI 内继续缩小动态 facade 面积，保持 `tauriWailsBridge` 作为过渡兼容层，优先把剩余 settings/core/proxy bridge API 和兼容层边界迁到统一 typed facade。

## Later：成熟度与评分刷新

- 只有新 shipped evidence 出现时才提高 capability score。
- AdsPower comparison 只用官方公开边界和本仓库可验证证据刷新；当前 P12 结论是 deferred，不上调 score。
- `50+`、`450+`、AdsPower catch-up、external integration 继续归入 Overall track，不能写成当前 runtime depth。
- 外部分发前执行 `docs/24-external-distribution-readiness.md` 的 known limitations 和 manual smoke checklist；未执行时只能说自动化 release gate 已通过，不能说外部发布已验收。
