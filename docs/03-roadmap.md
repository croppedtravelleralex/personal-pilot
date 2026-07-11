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
   - 2026-06-22 范围重置后，外部分发 readiness、人工 operator smoke 和 continuity integration test 已取消；只保留本机自用验收。

## Now：Local Self-Use Closed

目标已改为只服务本机自用。当前本机自用口径为 `100% / 0% / green`：observed fingerprint、behavior replay、SessionBundle 本机 restore、M10 stability、M15 pool/process、settings/core/proxy facade、runtime adapter local self-use 和本机 TLS/transport 均已有本机证据。release performance、外部分发、第二机/跨机器、AdsPower 刷分和远程代理账号都已取消为当前目标。

唯一未验：CAPTCHA / SMS / Email 服务商真实账号凭证 smoke。没有真实账号、余额、API key 和测试目标时，不得把 provider acceptance 写成 accepted。

2026-07-10 本地工程更新：指纹/反检测 W0（49-A1/A3/C1/D1）已完成实现与本地测试/Chrome smoke；代理订阅和录制 API 已修正为诚实边界，CI 已补 Go tests + frontend build。外部凭证、节点、DNS-token、额度和付费 API 继续暂缓，不参与当前执行顺序。 同日已补 `ExecuteMutatedActionWithResult` 类型化 payload 与 unknown-type error。 同批已完成 durable proxy subscription store（migration 16、事务刷新、自动刷新、Clash 静态导入、token/credential 脱敏），订阅 API 不再返回临时 `501`。

0. Single UI / exe convergence
   - 用户已确认唯一主线 UI 是截图所示 `personal-pilot` 1.1.0 / Wails v2 + React UI。
   - 唯一用户入口为 `D:\SelfMadeTool\personal-pilot\personal-pilot-tauri.exe`。
   - 2026-05-27 已把 Tauri release 产物名、产品名、根目录验收入口和启动 UI 壳收敛到该 exe；2026-06-22 复核 `tauriWailsBridge` / core bridge 可读取当前真实 `4/69/4` 数据。
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
   - P14 已新增 provider acceptance、本机 SessionBundle contract、taxonomy audit 等脚本入口；external distribution、release performance 和 cross-machine portability 入口只保留历史诊断价值。

2. Fingerprint runtime depth
   - 已从 `80` declared controls 和 `12` runtime projected fields 推进到 `26` runtime projected fields（`25` control-supported + derived `platform`）。
   - P5 真实 Lightpanda/CDP smoke 已通过，P6 evidence schema 已收敛，P7 fingerprint observation audit 已接入 Validation Board；P14 已新增 `docs/taxonomy/fingerprint-signal-taxonomy.json`；2026-05-28 已新增 Go 端环境注入编译器和 `Page.addScriptToEvaluateOnNewDocument` CDP 入口，覆盖 browser API/canvas/timezone/WebGL/media devices 基础 hook；`taxonomy_coverage_materialize.ps1` 已物化 `450 / 450` 条 fingerprint signal contract；2026-06-22 full observed probe + strict coverage gate 已跑通为 `passed_full_observed_fingerprint_coverage`，`450 / 450`。
   - 保持 control / derived / observation layers 分离。

3. Session / proxy orchestration
   - 已将 restart continuity 推进到 profile-scoped `SessionBundle` export、import preflight、dry-run、confirmed local restore write path。
   - P14 已新增 `scripts/session_bundle_portability_smoke.ps1` 记录本机 contract；跨机器 manual steps 已取消。
   - 2026-06-22 M8 已重置为本机 restore gate：`scripts/m8_session_handoff_gate.ps1` 会刷新 `scripts/session_bundle_portability_smoke.ps1`，并生成 `m8-session-handoff` report；最新状态 `passed_local_restore_verified` 代表本机 export/preflight/dry-run/confirmed restore 和 persisted restart-continuity artifact 已有本机证据，第二 Win11 target report 不再要求。
   - 下一步聚焦本机 restore failure reason、lease / cooldown / health / rollback。

4. Behavior and automation depth
   - 已新增 P10 behavior audit contract：`13` shipped primitives、`8` page archetypes、workflow graph、debug trace、manual gate、recovery semantics、`450+` target-only 边界可在 Automation surface 查看。
   - P14 已新增 `docs/taxonomy/behavior-event-taxonomy.json` 和 Automation taxonomy seed 可见性；2026-05-28 `taxonomy_coverage_materialize.ps1` 已物化 `461 / 450` 条 behavior replay contract。
   - 2026-05-28 已落地 Phase 2 P0 的 Go humanize 基础模型：Fitts Law 轨迹、粉噪、四段式点击、双击、拖拽和右键菜单计划，并以 Go 单测覆盖。
   - 2026-05-28 已把 shipped primitives 的 workflow graph/debug trace 状态前推到 runtime evidence backed。
   - 2026-06-22 已新增 `scripts/live_replay_runtime_gate.ps1` 本地 deterministic replay runtime，最新状态为 `passed_full_local_replay_runtime`，`461 / 450` events replayed，`461` product-runtime-backed、`0` contract-only。
   - 下一步是把 contract-only families 和本地 replay harness 继续接入 product/browser/provider runtime；不能把它写成 target-site production replay。
   - CAPTCHA/SMS/Email 已有 production readiness contract、acceptance checklist、Settings operator surface 和 M4.4 dry-run/failure taxonomy 可见性；下一步是真实 manager wiring、CDP detect/fill 和 provider acceptance。

5. Runtime adapter and external integration
   - Runtime adapter contract 已接入，覆盖 Fake、Lightpanda、headed_external 边界；`headed_external` 泛化 source contract 已落地，且 2026-05-30 已用仓库内 fingerprint Chromium 跑通真实 `get_title https://example.com` 单任务，report `data/reports/headed-external-smoke/headed-external-smoke-1780109927517.json` 为 `passed_real_binary_task`；同日继续跑通真实 `validation_probe`，report `data/reports/headed-external-smoke/headed-external-smoke-1780121855313.json` 为 `passed_real_binary_validation_probe`，包含 9 个 profile-browser validation signals。2026-05-31 已修复 headed_external evidence ranked selection 和 smoke stdout 污染；2026-06-02 headed repeatability report `data/reports/headed-external-smoke/headed-external-smoke-1780375482756.json` 为 `passed_real_binary_repeatability`，M10 repeatability gate `data/reports/m10-headed-repeatability/m10-headed-repeatability-gate-1780377051356.json` 为 `passed_repeatability_partial_coherence`；2026-06-22 M10 stability gate `data/reports/m10-headed-stability/m10-headed-stability-gate-1782114360122.json` 为 `passed_long_task_stability_coherence`，3/3 attempts passed，coherence/stability score 均为 1。
   - P11/P14 的 release measurement 字段和 `scripts/release_performance_smoke.ps1` 仅保留历史诊断价值；不再作为路线目标或 blocker。
   - 下一步只吸收高 ROI 外部浏览器思路。
   - 不把主仓库变成 Chromium / Firefox fork host。
   - P12 AdsPower benchmark refresh guard 已转为历史；当前本机自用范围下不重算 AdsPower 评分。
   - Camoufox 下一步按 `docs/18-external-browser-integration-plan.md` 的 `90` 分方案推进：2026-05-28 已先把它作为主线 `browser_cores.kind=camoufox` 内核类型接入 Go core manager、SQLite、sidecar RPC、实例启动参数分发和主线 UI 选择；本轮已把 Rust runner 从 skeleton 推进为最小 CDP runner，支持 open/html/text/title/final-url/validation-probe 和 stdout/stderr/content preview；真实 Camoufox binary 已通过 Firefox-compatible headless screenshot 打开 `https://example.com` 并产出持久 PNG。remote server、browser pool 和深度指纹拟真不进入第一主链；Chromium `/json/version` CDP attach 不宣称通过。
   - Phase 6 传输一致性已从死骨架推进到 Xray/SingBox 安全 ALPN 合并和 runtime family explain metadata；真实 Xray/SingBox 本地二进制配置验证已通过：Xray `run -test` 返回 `Configuration OK`，SingBox `check -c` exit code 为 `0`。这仍只是 direct outbound 配置接受性，不是远程代理出站或 TLS/HTTP2 指纹观测。
   - 2026-06-22 runtime adapter gate 已按本机自用改为 `passed_local_self_use`；profile-browser comparison、SessionBundle 本机 restore、M10 stability、M15 process/pool、observed coverage、replay runtime 和本机 TLS/transport 都已通过。唯一未验是 provider credential smoke。
   - 2026-06-11 historical update：M4 Monitor facade 已把 `EventMonitorPage` 实时事件订阅与 event-log history query/count/export/prune 迁到 `src/services/desktop.ts` typed wrappers，新增 `scripts/m4_monitor_facade_gate.ps1`、`m4-monitor-facade` evidence kind 和 Dashboard `M4 Monitor` row；M4 gate v16 已纳入 `monitor_facade_contract`，当时 report 为 `passed_with_expected_external_blockers`、`passed=15`、`expectedBlocked=4`、`failed=0`。剩余 facade shrink 主要在 browser runtime/workbench/core bridge 与过渡 `tauriWailsBridge`。
   - 2026-06-12 historical update：M4 Browser runtime facade 已把 Browser List/Detail 的 `browser:instance:*` runtime subscriptions 迁到 `src/modules/browser/api.ts` 的 `onBrowserInstanceRuntimeEvents`，API 内部统一通过 `desktopRuntimeListen` 订阅并复用 payload normalizer；新增 `scripts/m4_browser_runtime_facade_gate.ps1`、`m4-browser-runtime-facade` evidence kind 和 Dashboard `M4 Browser Runtime` row；M4 gate v17 已纳入 `browser_runtime_facade_contract`，当时复跑为 `passed_with_expected_external_blockers`、`passed=16`、`expectedBlocked=4`、`failed=0`。剩余 facade shrink 主要在 settings/core/proxy/workbench bridge 与过渡 `tauriWailsBridge`。
   - 2026-06-12 historical update：M4 Workbench DTO shrink 已把 `src/services/desktop.ts` 的 Workbench detection results / detector sites / UI state / detector run API 从 `unknown[]` 或裸 `unknown` 收窄到 `src/types/desktop.ts` 共享 DTO，并更新 `src/modules/synchronizer/api.ts` 去掉对应 `Array.isArray(results)` 退化；M4 gate v18 已把 Workbench DTO 纳入 `typed_facade_shrink_contract`，当时复跑为 `passed_with_expected_external_blockers`、`passed=16`、`expectedBlocked=4`、`failed=0`。剩余 facade shrink 主要在 settings/core/proxy bridge 与过渡 `tauriWailsBridge`。
   - 2026-06-12 historical update：M4 Runtime facade 已把 `SettingsPage` backup export/import progress、`CoreManagementPage` download progress / external URL、`ProxyPickerModal` / `ProxyPoolPage` proxy result events、`LaunchApiDocsPage` / `UsageTutorialPage` external URL calls 迁到 `src/services/desktop.ts` 的 `desktopRuntimeListen` / `desktopOpenExternalUrl`；新增 `scripts/m4_runtime_facade_gate.ps1`、`m4-runtime-facade` evidence kind 和 Dashboard `M4 Runtime` row；M4 gate v19 已纳入 `runtime_facade_contract`，当时复跑为 `passed_with_expected_external_blockers`、`passed=17`、`expectedBlocked=4`、`failed=0`。当时剩余 facade shrink 主要在更深的 settings/core/proxy bridge API 与过渡 `tauriWailsBridge` 兼容层；2026-06-22 已补 settings/core/proxy browser facade gate。
   - 2026-06-12 historical update：M4 Workbench/report DTO shrink 继续推进，`BrowserInstanceStatus`、`WorkbenchFingerprintHealthProfile`、`WorkbenchFingerprintProfile`、`IdentityReportProfile` 的 desktop service 返回值已从裸 `unknown` 收窄到共享 DTO，`src/modules/synchronizer/api.ts` 移除对应 status/report cast；M4 gate v20 扩展 `typed_facade_shrink_contract` 后当时仍为 `passed_with_expected_external_blockers`、`passed=17`、`expectedBlocked=4`、`failed=0`。当时剩余 facade shrink 主要在更深的 settings/core/proxy bridge API 和 `tauriWailsBridge` 兼容层；2026-06-22 已继续收窄。
   - 2026-06-12 historical update：M4 UI error boundary shrink 已把 Dashboard、Dashboard API、Settings、Automation、NaturalLanguageTask、TagManagement、RecordingDetailModal 的选定错误处理从 `catch (...: any)` 收窄到 `unknown` guard，并把 Dashboard WebView audio fallback / Launch API markdown code block 中的 `any` cast 改为 typed guard；M4 gate v21 新增 `ui_error_boundary_contract`，当时复跑为 `passed_with_expected_external_blockers`、`passed=18`、`expectedBlocked=4`、`failed=0`。该切片不代表全仓 legacy UI any-catch 已清零，剩余继续按小批次推进。
   - 2026-06-12 historical update：M4 Browser instance UI error boundary shrink 已把 BrowserList/Detail/Edit、BrowserSettingsModal、QuickLaunchModal 的实例操作错误路径从 `catch (...: any)` 收窄到 `unknown` guard，保留 action feedback 语义，并修正 BrowserList 启动成功/失败 mojibake 文案；M4 gate v22 扩展同一 `ui_error_boundary_contract` 后当时仍为 `passed_with_expected_external_blockers`、`passed=18`、`expectedBlocked=4`、`failed=0`。该切片不代表 Proxy/Core 大页面或全仓 legacy UI any-catch 已清零。
   - 2026-06-12 historical update：M4 Core management UI error boundary shrink 已把 CoreManagementPage 的打开目录、扫描、保存/删除、设默认、下载启动和全局设置保存错误路径从 `catch (...: any)` 收窄到 `unknown` guard；M4 gate v23 扩展同一 `ui_error_boundary_contract` 后当时仍为 `passed_with_expected_external_blockers`、`passed=18`、`expectedBlocked=4`、`failed=0`。该切片不代表 ProxyPoolPage 或全仓 legacy UI any-catch 已清零。
   - 2026-06-12 historical update：M4 Proxy pool UI error boundary shrink 已把 ProxyPoolPage 的订阅刷新、删除、保存、URL 获取 fallback、订阅导入、解析、导入确认和名称修复错误路径从 `catch (...: any)` 收窄到 `unknown` guard，并把 Clash proxy index signature 从 `any` 收窄到 `unknown`；M4 gate v24 扩展同一 `ui_error_boundary_contract` 后当时仍为 `passed_with_expected_external_blockers`、`passed=18`、`expectedBlocked=4`、`failed=0`。该切片不代表全仓 generic `Record<string, any>` 已清零。
   - 2026-06-12 historical update：M4 generic any residue shrink 已把 shared Table 泛型约束、ProxyIPHealthResult rawData 和 ProxyPoolPage/types ClashProxy index signature 的选定 generic/index `any` 收窄到 `object` / `unknown`；M4 gate v25 新增 `generic_any_residue_contract` 后当时仍为 `passed_with_expected_external_blockers`、`passed=19`、`expectedBlocked=4`、`failed=0`。该切片不代表仓库每一个显式 `any` 已移除。
   - 2026-06-12 historical update：M4 RecordingPanel UI error boundary shrink 已把行为录制面板的录制、回放、导入导出、复制、重命名、接管和清理错误路径从未显式类型的 `catch (e)` 收窄到 `catch (error: unknown)`，错误文案走 `messageFromUnknownError`，并修正回放失败 mojibake 文案；M4 gate v26 扩展同一 `ui_error_boundary_contract` 后当时仍为 `passed_with_expected_external_blockers`、`passed=19`、`expectedBlocked=4`、`failed=0`。该切片不代表全仓 legacy catch 已清零。
   - 2026-06-12 historical update：M4 bridge compatibility type shrink 已把 `desktopRpc` 参数通道和 `tauriWailsBridge` 兼容层 runtime/app proxy 类型收敛到命名边界，新增 `bridge_compat_type_contract`，阻止 `desktop.ts` / `tauriWailsBridge.ts` 回退到裸 `unknown[]` / `Promise<unknown>`；M4 gate v27 当时仍为 `passed_with_expected_external_blockers`、`passed=20`、`expectedBlocked=4`、`failed=0`。该切片不代表 `tauriWailsBridge` 已移除。
   - 2026-06-22 current update：M4 browser window.go fallback shrink 已进入 v28；M4 browser settings/core/proxy facade shrink 已进入 v29，相关 API 改走 `src/services/desktop.ts` typed wrappers，`tauriWailsBridge` App RPC surface 改为显式 allowlist；profile/browser comparison 已 passed；strict observed coverage、full local replay runtime、M10 stability、M15 pool/process 和 runtime adapter local self-use 已 passed；release smoke、external distribution smoke、第二机/跨机器 portability、AdsPower 刷分和远程代理账号已取消为目标，只保留历史 report。

6. Retire duplicate UI paths
   - 2026-05-27 已删除第二套 Tauri/Vite 控制台 UI 源码、根目录旁路 exe、`src-tauri/target/release` 持久 GUI exe 和仓库内 `gateway-ui` 静态 UI。
   - Gateway dashboard 静态 UI 不再作为仓库内置资产；如需临时使用，必须通过 `GATEWAY_UI_DIR` 显式指向外部目录。
   - 第二套 UI 不再作为发布入口，也不保留源码目录；后续只允许从历史提交中取设计参考。
- 下一步不再做旧 UI 复活式迁移；`tauriWailsBridge` 作为过渡兼容层继续显式 allowlist。当前只等待 CAPTCHA/SMS/Email 真实账号凭证 smoke。

## Later：仅在用户重开范围时

- 只有用户明确重开外部分发、AdsPower 对比、release performance budget、第二机/跨机器或远程代理账号目标时，才重新建 gate。
- `docs/24-external-distribution-readiness.md`、`docs/release-performance-mitigation-plan.md` 和 `docs/sessionbundle-cross-machine-portability-runbook.md` 只作历史上下文。

## Reopened：AdsPower / BitBrowser / PersonaPilot 横评

2026-07-07 用户明确重开 AdsPower 对比范围，并把 BitBrowser 纳入同一矩阵。该轨道只评估“商业闭源指纹浏览器横向 benchmark 与 PersonaPilot 补短板”，不改变 Mainline / Local self-use 已闭环口径。

当前阶段：基础 launch-loop、当前可执行深度矩阵和低配额 missing probe 已完成；自动化实测范围收敛为 PersonaPilot + BitBrowser，AdsPower 因 Free 账号 API & MCP 付费墙只作安装/官方资料/手工观察项。当前 100 分评分为 PersonaPilot `73/100`、BitBrowser `71/100`、AdsPower `N/A`。

已完成：

1. AdsPower 官方 8.6.3 Windows x64 安装器已下载、校验签名和 SHA256，并静默安装到 `D:\SelfMadeTool\ads\AdsPowerGlobal`。
2. BitBrowser 目录确认为 `D:\SelfMadeTool\bitbrowser`，版本 `7.1.3`，本地 API `54345` 可访问；UDEAL-LA dry-run profile 已创建并通过 open/CDP/close。
3. 已新增 `docs/48-three-browser-benchmark-matrix-plan.md`，覆盖 PersonaPilot + BitBrowser 自动化 profiles、10 连启、CreepJS/BrowserLeaks/BrowserScan/Pixelscan、IP/DNS/WebRTC/TLS/H2、行为/RPA/API、raw artifact 和脱敏规则；AdsPower 只保留 manual-only 观察边界。
4. 已完成代理预检并生成脱敏汇总：Clash 机场当前 US `3/3` ok、JP `9/15` ok、DE `0`，成功节点 ip-api `proxy=true`；UDEAL 经 panda/本机桥为 LA 单出口，`18082`/`18090` 可用，ip-api `proxy=false`、`hosting=false`。两者必须拆成 proxy submatrix。
5. 已新增 `scripts/three_browser_benchmark_readiness.mjs`；`data/reports/three-browser-benchmark/readiness/readiness-1783473069728.json` 证明 PersonaPilot 与 BitBrowser 的 UDEAL-LA 1 profile / 1 launch dry-run 可创建、打开 CDP、关闭，且报告脱敏扫描通过。当前脚本已补 AdsPower v2 create/start/stop gated dry-run 分支，但用户截图确认 Free 账号 API & MCP 仅限付费套餐，所以 AdsPower 不进入本轮自动化矩阵。
6. 已改为双产品自动化 smoke：`node scripts\three_browser_benchmark_readiness.mjs --dry-run --products=personal-pilot,bitbrowser` 生成 `data/reports/three-browser-benchmark/readiness/readiness-1783475270066.json`，PersonaPilot 与 BitBrowser 均打开 `https://browserleaks.com/ip`、CDP 可达、关闭成功，AdsPower 被显式 skipped。
7. 已新增 `scripts/two_browser_benchmark_matrix.mjs`，输出 `config.redacted.json`、`profiles.redacted.json`、`raw/*.json`、`summary.json`、`scorecard.md`，并把 `ip-api` 页面探针替换为 HTTPS `ipwho.is`，原始 IP 只保留 SHA256 hash。
8. 已跑当前可用基础 launch-loop 子矩阵：Clash 机场 US/JP `matrix-1783480553485` 为 `40/40 ok`，PersonaPilot `20/20`、BitBrowser `20/20`，国家匹配 `40/40`；UDEAL-LA `matrix-1783480223173` 为 `19/20 ok`，PersonaPilot `10/10`、BitBrowser `9/10`，唯一失败是 BitBrowser 内存保护阈值。敏感扫描只命中脚本自身脱敏正则。
9. 已新增 `scripts/two_browser_benchmark_deep_matrix.mjs`，输出 `screenshots/`、`har-lite/`、`cdp-trace/`、`detector-reports/`、`transport/`、`behavior/`、raw 和 scorecard；当前完整可执行主跑 `deep-1783482661915` 为 `6/6 ok`，国家匹配 `6/6`、TLS/H2 `6/6`、行为 `5/6`、detector 主跑 PersonaPilot `18/18`、BitBrowser `12/12`。BitBrowser Clash JP detector 瞬时缺口已用 `deep-1783490711021` 补跑为 `6/6`。Clash 已恢复 `rule` 和原 GLOBAL hash。
10. 已新增 `scripts/two_browser_missing_probe_matrix.mjs` 并按 BitBrowser 免费额度只跑 targeted probe：PersonaPilot `missing-1783494756348`、BitBrowser `missing-1783495149879` 均 `3/3 ok`；双方 country/timezone/TLS `3/3`、WebRTC candidate `0/3`、canvas in-session `3/3`、BrowserLeaks DNS observed `3/3`、UA/core major match `0/3`、CreepJS trust/lies parser `0/3`。

下一阶段按“本地可落地优先、外部阻塞暂缓”推进：

1. 基于 deep/missing raw artifact 继续本地 P1：workflow 历史/控制、workbench target binding、CreepJS structured parser；不把单次 detector 可达性误写成长期过检率。
2. 指纹轨道从已完成 W0 转入 W1 本地项：Worker 注入、人格库/相干性硬门禁、行为 L5 接线、信任双栈统一。
3. Germany 节点、AdsPower paid/trial API、受控 DNS-token 和 BitBrowser 额度属于外部条件；条件未变化前不重复执行、不刷新评分。

## Reopened：指纹 / 反检测优化轨道（49–56）

2026-07-08 起用户明确推进「追平 BitBrowser + 不对称支配」独立轨道。与 Mainline / 横评并列；执行以 **`PLAN.md`** 为准。

| 波次 | 范围 | 关键产出 |
| --- | --- | --- |
| W0 | 49 A1/A3/D1/C1 | **已完成（2026-07-10）**：toString 原生化、UA/内核单一真相源、鼠标重试/往返压缩、DNS 假阳性修正 |
| W1 | 50 B1/C1 + 52 DP1/2 + 53 L5 + 56 T1 | Worker 注入、相干性硬门禁、信任双栈 |
| W2 | 50 A + 51 S + 54 CP + 56 E/F | uTLS、指纹面、并发预留、Graph 出站 |
| W3 | 51/52/53/54/55 收敛 | 老化、纵向指标、成本不对称闭环 |
| W4 | 56 P/T/B/R | Provider smoke、trust.yaml、CreepJS parser、RPA MVP |

文档归档：`docs/archive/README.md`（2026-07-09 已移入 27+ 过程/专题文档 + 12 根目录历史文件）。
