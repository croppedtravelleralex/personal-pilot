# Improvement Backlog

## Mainline release evidence

| 领域 | 待改进项 | 当前状态 | 退出条件 |
| --- | --- | --- | --- |
| Proxy / IP | provider-side proxy rotation write | 已闭环 | 真实 provider action、rollback、cooldown、retry 均走 typed native chain |
| Synchronizer | native broadcast write closure；set-main / work-area-aware layout 已落地 | 已闭环 | staged-only 不再是主 operator route，broadcast write 走 typed native chain |
| Recorder / Templates | native-first de-fallback closure | 已闭环 | release-default path 不依赖 fallback |
| Release | Win11 final packaging / operator acceptance polish | 自动化门禁已通过；P13 已新增外部分发 readiness 文档；人工 smoke 待执行 | Type check、Vite build、Win11 baseline、Tauri release build、Windows local verify 同时通过并记录；外部分发前按 `docs/24-external-distribution-readiness.md` 完成人工 smoke |
| Release | 单一 exe / 单一 UI 收敛 | 2026-05-27 已重新构建并覆盖根目录 `personal-pilot-tauri.exe`；已核验 core bridge / DB 当前真实数据为 `4/93/3`；已删除第二套 UI、旁路 exe、target GUI exe 产物和仓库内 `gateway-ui/` | 构建、脚本、文档、smoke 和外部分发只认 `personal-pilot-tauri.exe`；旁路 exe/UI 不再参与构建、验证或分发 |

## Overall remaining `60%`

| 领域 | 待改进项 | 当前状态 | 退出条件 |
| --- | --- | --- | --- |
| Validation | detector / leak / DNS / WebRTC / canvas / audio / worker / transport board | 前端 MVP、DNS/transport observed report、desktop WebView scoped WebRTC/canvas/audio/storage probes、profile browser runtime `validation_probe` action、WebRTC/leak native warning contract、history list、profile-level export、`validation_lightpanda_smoke` 和 P6 explicit evidence metadata 已接入；WSL2 Lightpanda nightly 真实 CDP smoke 已双次复跑 `status=passed`；P14 新增 smoke/report 脚本入口 | 继续扩大 observed coverage，并继续保留 scope/adapter/target/failure metadata |
| Fingerprint | 从 `12` runtime projected fields 扩展 runtime depth | 已扩到 `26` projected fields（`25` control-supported + derived `platform`）；P7 已新增 WebRTC/canvas/audio/leak observed audit；P14 已新增 `450` fingerprint signal taxonomy seed；2026-05-28 已新增环境注入编译器和单次 CDP 新文档注入入口，覆盖 browser API/canvas/timezone/WebGL/media devices 基础 hook；`roadmap_evidence_smoke` 已证明 1.2/1.3/1.4 本机实现覆盖，`profile_browser_environment_probe.mjs` 已用真实 Chromium profile-browser 观测 1.2/1.3/1.4 passed；profile-browser comparison 最新为 `partial_comparison_only`，缺同份 desktop-webview 对照信号 | declared / applied / observed 可解释且可验证；taxonomy seed 不得计入 full observed coverage；下一步补同份 desktop/profile comparison 与更完整 detector matrix |
| Session | 完整 `SessionBundle` 和 profile portability | profile-scoped export、import preflight、dry-run、confirmed local restore write path 已落地；P14 新增 portability smoke contract 脚本；本轮补 AES-GCM bundle encrypt/decrypt 与 runtime mapping contract；最新 portability smoke 为 `local_contract_passed` | 跨机器 profile portability、restart continuity、encrypted payload restore 和 restored payload 真实环境验收一起成立 |
| Behavior | 从 `13` primitives 扩展 replayable `450+` taxonomy | P10 behavior audit contract 已落地；P14 新增 `450` behavior event taxonomy seed 和 audit path；2026-05-28 已落地 Phase 2 P0 的 Fitts Law 轨迹、粉噪、四段式点击、双击、拖拽和右键菜单计划模型；本轮继续补 typing P1 与 scroll P1 plan 模型 | 事件数量来自真实 taxonomy；taxonomy seed 仍需 replay runtime 证据才能算交付；下一步接入执行器证据 |
| Provider closure | CAPTCHA/SMS/Email production manager wiring/config | production readiness contract、acceptance checklist 和 Settings operator surface 已落地；P14 新增 provider acceptance preflight 脚本；本轮补 workflow provider adapters | 真实 provider 验收、CDP detect/fill、operator UI 和失败处理闭环 |
| Runtime | headed realism、kernel strategy、adapter boundary | adapter/release smoke contract、P11 measurement pending visibility 和 P21 runtime adapter evidence gate 已落地；P14 新增 release performance smoke 脚本；本轮补 pool prewarm plan 和 transport Xray/Sing-Box config contract；最新 runtime adapter gate 为 `blocked_evidence_required`，headed runtime 仍未实现 | 有稳定 adapter contract、真实 runtime 验证证据，并给出性能超预算 mitigation |
| Runtime | Camoufox 单任务生产级 Runner | 2026-05-27 已在 `docs/18-external-browser-integration-plan.md` 固化并深化 `90` 分方案；2026-05-28 已把 Camoufox 作为主线内核类型接入 `browser_cores.kind`、Go core DAO/解析/校验、实例启动参数分发、sidecar RPC `BrowserCoreValidateForKind` 和主线 UI 选择；Rust runner skeleton 仍显式 `runner_not_implemented`、`browser_launch_attempted=false`，`scripts/camoufox_smoke.ps1 -AllowBlocked` 仍为 `contract_ready_runtime_smoke_required` | Camoufox 可配置、可检测、可执行、可取消、可清理、可回显、可诊断；artifact 落盘；基础 profile/proxy 映射；Lightpanda 不回归；默认不常驻、不预热、并发默认 1；真实 Camoufox 页面打开、artifact、取消/超时清理、release idle smoke 与 Camoufox task-run 性能报告均通过或记录明确例外；Win11/Tauri enforcement 通过 |
| Runtime | release performance mitigation | 已新增 `docs/release-performance-mitigation-plan.md` | 复测 release artifact，至少一项指标实质改善，剩余超预算原因写入 current-state |
| UI consolidation | 第二套 UI / 旁路 exe 清理与主线 API 收口 | 根目录旁路 exe、旧 release exe、第二套 Tauri/Vite console UI 和仓库内 `gateway-ui/` 已清理；Gateway dashboard UI 如需使用必须通过 `GATEWAY_UI_DIR` 指向外部目录，仓库默认不再指向内置 UI；2026-05-27 已把 `dashboard/profile/monitor` 收口到 `services/desktop.ts` typed wrapper 或模块 facade，并为事件监控 history 补上 `300ms` debounce 与 stale-result 保护；剩余风险主要在 `synchronizer/api.ts` 和 browser 模块动态 facade | 后续深度测试确认主线 UI 操作、Wails 兼容 API、report/history 和 Settings/runtime 细节无回归；继续把 `src/modules/synchronizer/api.ts` 与 browser 模块动态 RPC 逐步显式化为 typed facade |
| Benchmark | AdsPower boundary refresh | P12 已落地 guard，P21 已新增 machine-readable deferred gate report；评分刷新暂缓 | B1-B5 有新证据后再刷新评分；没有证据时只保留 deferred 结论 |

## 固定风险

- 把 `80` declared controls 误报成 `80` runtime-applied fields。
- 把 `450+` fingerprint / event target 误报成当前已交付。
- 把 mock / fallback / staged 默认路径算作 delivery closure。
- 把历史比例 `77% / 23%` 或 `82% / 18%` (historical-only) 覆盖成当前 live truth。
- 在 Mainline closeout 阶段引入 Overall 级大范围重构。
- P5/P6 已通过后仍把 WebRTC/audio warning 或 canvas failure 写成全部成功，或丢失 `failureReason`。
- 把第二套 Tauri UI 的功能当成 `personal-pilot-tauri.exe` 主线已交付。
- 删除旧 UI 前没有确认功能是否已迁回截图主线 UI。

## 维护规则

新增 backlog 时必须标清属于 Mainline release evidence 还是 Overall `60%`。不能确定归属时先放 Overall，等有代码证据后再提升到 Mainline release gate。
