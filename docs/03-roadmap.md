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
   - 下一步补齐跨机器 profile portability 验收、lease / cooldown / health / rollback。

4. Behavior and automation depth
   - 已新增 P10 behavior audit contract：`13` shipped primitives、`8` page archetypes、workflow graph、debug trace、manual gate、recovery semantics、`450+` target-only 边界可在 Automation surface 查看。
   - P14 已新增 `docs/taxonomy/behavior-event-taxonomy.json` 和 Automation taxonomy seed 可见性；2026-05-28 `taxonomy_coverage_materialize.ps1` 已物化 `461 / 450` 条 behavior replay contract。
   - 2026-05-28 已落地 Phase 2 P0 的 Go humanize 基础模型：Fitts Law 轨迹、粉噪、四段式点击、双击、拖拽和右键菜单计划，并以 Go 单测覆盖。
   - 2026-05-28 已把 shipped primitives 的 workflow graph/debug trace 状态前推到 runtime evidence backed。
   - 下一步才是把 materialized contract 扩展为真实 replayable `450+` live event runtime。
   - CAPTCHA/SMS/Email 已有 production readiness contract、acceptance checklist、Settings operator surface 和 M4.4 dry-run/failure taxonomy 可见性；下一步是真实 manager wiring、CDP detect/fill 和 provider acceptance。

5. Runtime adapter and external integration
   - Runtime adapter / release smoke contract 已接入，覆盖 Fake、Lightpanda、headed_external 边界和 release artifact 检查；`headed_external` 泛化 source contract 已落地，且 2026-05-30 已用仓库内 fingerprint Chromium 跑通真实 `get_title https://example.com` 单任务，report `data/reports/headed-external-smoke/headed-external-smoke-1780109927517.json` 为 `passed_real_binary_task`；同日继续跑通真实 `validation_probe`，report `data/reports/headed-external-smoke/headed-external-smoke-1780121855313.json` 为 `passed_real_binary_validation_probe`，包含 9 个 profile-browser validation signals。2026-05-31 已修复 headed_external evidence ranked selection 和 smoke stdout 污染，最新 headed report `data/reports/headed-external-smoke/headed-external-smoke-1780198191425.json` 仍为 `passed_real_binary_validation_probe`。
   - P11 已新增 release measurement target vs measured pending 字段，并在 Overview 暴露 adapter boundary；P14 已新增 `scripts/release_performance_smoke.ps1` 用 release exe 生成 cold start/RSS/process count report。
   - 下一步只吸收高 ROI 外部浏览器思路。
   - 不把主仓库变成 Chromium / Firefox fork host。
   - P12 已把 AdsPower benchmark refresh 固定为 boundary guard；P21 新增 runtime adapter evidence gate report：等 B1-B5 有新证据后再重算评分。
   - Camoufox 下一步按 `docs/18-external-browser-integration-plan.md` 的 `90` 分方案推进：2026-05-28 已先把它作为主线 `browser_cores.kind=camoufox` 内核类型接入 Go core manager、SQLite、sidecar RPC、实例启动参数分发和主线 UI 选择；本轮已把 Rust runner 从 skeleton 推进为最小 CDP runner，支持 open/html/text/title/final-url/validation-probe 和 stdout/stderr/content preview；真实 Camoufox binary 已通过 Firefox-compatible headless screenshot 打开 `https://example.com` 并产出持久 PNG。remote server、browser pool 和深度指纹拟真不进入第一主链；Chromium `/json/version` CDP attach 不宣称通过。
   - Phase 6 传输一致性已从死骨架推进到 Xray/SingBox 安全 ALPN 合并和 runtime family explain metadata；真实 Xray/SingBox 本地二进制配置验证已通过：Xray `run -test` 返回 `Configuration OK`，SingBox `check -c` exit code 为 `0`。这仍只是 direct outbound 配置接受性，不是远程代理出站或 TLS/HTTP2 指纹观测。
   - 2026-05-31 已刷新 runtime adapter gate：Camoufox source smoke 与 binary page-open passed，headed_external real binary validation probe 被 ranked selection 正确选中；最新 gate `data/reports/runtime-adapter/runtime-adapter-evidence-gate-1780198524266.json` 仍 blocked，但 `runtimeAdapterEvidence=partial_real_binary_validation_probe_recorded`、`fingerprintRuntimeDepth=partial_headed_profile_browser_observed`、`signalCount=9`；profile-browser comparison 仍因 desktop WebView report 未实际采集而 blocked，provider 为 `blocked_missing_credentials`，远程代理/TLS 为 `blocked_remote_proxy_required`，SessionBundle 为 `local_contract_passed` 但跨机器 evidence pending；完整 runtime adapter / B1-B5 evidence、external distribution、AdsPower refresh 仍 blocked/deferred。
   - 2026-05-31 已新增 M4-M20 execution board 和 M4 acceptance harness：`scripts/m4_acceptance_gate.ps1` 会把外部缺口分类为 `expected_blocked`，最新 M4 report 为 `expected_blocked` 且 `failed=0`；Dashboard 已展示 M4/性能/adapter/对比/provider/session/taxonomy/external reports。2026-06-01 已推进 M4 workflow task center：scheduler task status/last run/error/retry 进入 SQLite 持久化，Automation 页立即执行后刷新并展示运行状态；同日 M4.3 typed CDP primitives v1 已补 `select/dialog/download/upload/iframe/tab` runner actions 和 mock CDP 测试；M4.4 provider dry-run/operator closure 已补 v3 preflight、failure taxonomy、Settings 可见性和 M4 gate 本地检查；M4.5/M6 已补 Dashboard `采集 WebView` 入口和 profile-browser comparison v3 同窗校验；M4.6 已补 Settings SessionBundle 本机 export/preflight/dry-run/confirmed restore operator loop；M4.7 已补 Dashboard Runtime Adapter 卡，从 release smoke contract 读取并按证据强度排序展示 adapter、runner、profile/fingerprint evidence 和 blocker；M4.8 已把 synchronizer high-traffic bridge 从 `unknown[]`/module cast 收窄到共享 DTO，并把 browser API 的 Wails binding 动态入口收敛为 `BrowserNativeBindings` / `getWindowGoApp()` 类型合同；M4.9 已新增 logger 脱敏器、写入/formatter 脱敏路径和 source/test gate；M4.10 已把 M4 总闸升级为 `passed_with_expected_external_blockers` operator 状态，并在 Dashboard evidence history 中保留 expected external blocker 语义。M5 已补 release performance/health v2：release smoke report 输出 per-metric budget、10% drift reason、healthSummary 和 mitigationHints，`scripts/m5_release_health_gate.ps1` 最新为 `passed_with_budget_overrun`，Dashboard evidence history 可见 M5 Health。M13 已补 evidence trend v1，为同 kind report 显示 latest-vs-previous status/failureReason 趋势但不改变 gate 口径。性能仍 over budget，不能写成 green；实际 desktop/profile 同窗采集、browser payload schema normalize、真实 headed coherence/repeatability evidence 继续按 M6/M10 推进。

6. Retire duplicate UI paths
   - 2026-05-27 已删除第二套 Tauri/Vite 控制台 UI 源码、根目录旁路 exe、`src-tauri/target/release` 持久 GUI exe 和仓库内 `gateway-ui` 静态 UI。
   - Gateway dashboard 静态 UI 不再作为仓库内置资产；如需临时使用，必须通过 `GATEWAY_UI_DIR` 显式指向外部目录。
   - 第二套 UI 不再作为发布入口，也不保留源码目录；后续只允许从历史提交中取设计参考。
   - 下一步不再做旧 UI 复活式迁移；改为在截图主线 UI 内继续缩小动态 facade 面积，保持 `tauriWailsBridge` 作为过渡兼容层，优先把 synchronizer/browser 剩余 API 迁到统一 typed facade。

## Later：成熟度与评分刷新

- 只有新 shipped evidence 出现时才提高 capability score。
- AdsPower comparison 只用官方公开边界和本仓库可验证证据刷新；当前 P12 结论是 deferred，不上调 score。
- `50+`、`450+`、AdsPower catch-up、external integration 继续归入 Overall track，不能写成当前 runtime depth。
- 外部分发前执行 `docs/24-external-distribution-readiness.md` 的 known limitations 和 manual smoke checklist；未执行时只能说自动化 release gate 已通过，不能说外部发布已验收。
