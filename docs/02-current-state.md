# Current State

Updated: 2026-05-23 (Asia/Shanghai)

## 当前 live truth

- Mainline delivery：`100% / 0% / green` (3 P0 items closed)
- Overall end-state：`30% / 70% / yellow`
- 第一族控制 schema 已声明 `80` 个 core control fields。
- 当前 runtime projection 是 `26` 个 env-backed fingerprint fields，其中 `25` 个来自 first-family control fields，另含 derived `platform`。
- 当前 behavior runtime 已交付 `13` 个 primitives。
- cookie / localStorage / sessionStorage 跨 app restart 持久化与恢复已落地。
- profile-scoped `SessionBundle` export/import preflight/dry-run/confirmed local restore write path 已落地：可导出 profile refs、fingerprint/network/continuity/behavior references、session binding metadata、cookie/localStorage/sessionStorage 存在性与计数；默认脱敏，显式本地开关才包含敏感 payload；import preflight 可检查 schema、引用缺失和 profile 冲突；dry-run 不写 DB；confirmed restore 可 upsert target profile 和 `proxy_session_bindings`，敏感 payload 只在 bundle 包含时恢复。

历史 `77% / 23%` 或 `82% / 18%` 不再作为当前进度口径。

## Runtime Alive

- Tauri desktop release build 可生成 Win11 NSIS installer。
- Runtime alive 只代表应用和本地能力可启动/调用，不等同于 Overall 终态闭环。
- Gateway real-upstream acceptance 仍需按 `upstream_configured=false` guardrail 单独验收。

## Build Status

- 2026-05-23 已通过 `scripts/windows_local_verify.ps1 -SkipContinuityTest`：type check、Vite production build、Win11 Tauri baseline、`cargo test --lib -- --test-threads=1`、Tauri release build、`cargo test --quiet`。
- Tauri release installer 已生成：`src-tauri/target/release/bundle/nsis/PersonaPilot_0.1.0_x64-setup.exe`。
- 后续 meaningful code change 仍需重新跑对应门禁；operator manual smoke 和 continuity integration test 可按发布需要追加。

## Reporting Rule From Now On

- current shipped app / closeout / native mainline：`100% / 0% / green`。
- complete app / AdsPower catch-up / `50+` control / `450+` fingerprint or event target：`30% / 70% / yellow`。
- 汇报时必须分开 Mainline 和 Overall，不把目标深度写成已交付 runtime depth。

## 已确认落地

- Win11 desktop shell 基于 Tauri 2 + Vite + React + TypeScript 已落地。
- `src/services/desktop.ts` 是 native / invoke 的统一边界。
- Dashboard / Profiles / Proxies / Automation / Synchronizer / Logs / Settings 已在真实 operator surface 上。
- `Tasks -> Automation` surface unification 已完成。
- provider-aware / sticky-aware `changeProxyIp` local desktop contract 已落地。
- Recorder desktop step-write 已落地。
- Synchronizer live desktop snapshot、native focus、native set-main、work-area-aware native physical layout 已落地。
- full Rust / integration gate 已恢复 green。
- route-level code splitting 已清掉旧 Vite chunk warning。
- CAPTCHA 后端 handler/route 与 solver 代码边界已部分落地：`internal/captcha`、2Captcha、Capsolver、`/api/captcha/*` 核心端点；生产 readiness contract 已接入桌面 API，但 manager wiring/CDP 填入/operator UI 闭环仍 blocked。
- SMS 后端 handler/route 与 provider 代码边界已部分落地：`internal/sms`、5sim、SMSPool、`/api/sms/*` 核心端点；生产 readiness contract 已接入桌面 API，但 manager wiring/CDP 填号填码/operator UI 闭环仍 blocked。
- Email 已从内部模块推进到 `EmailService` + inbox / wait-code REST API；生产 readiness contract 已接入桌面 API，但自动化流程和 operator UI 闭环仍 blocked。
- 备份单体已拆成 core/import/merge/utils；browser launch args 与 process monitor 已迁入 `internal/browser`。
- Validation Board 已落地到桌面导航：8 类 evidence（detector / leak / DNS / WebRTC / canvas / audio / worker / transport）按 declared / applied / observed 分层展示；DNS/transport observed 首批 native collector、desktop WebView scoped WebRTC/canvas/audio/storage probes、profile browser runtime validation probe action、WebRTC/leak native contract warning、JSON report、report history 和 profile-level evidence export 已接入。真实 profile browser-scoped evidence 依赖 `PERSONA_PILOT_RUNNER=lightpanda` 与 Lightpanda/CDP 可用；FakeRunner 只记录 warning stub。P5 已通过 WSL2 Lightpanda nightly 真实 CDP smoke：`validation_lightpanda_smoke --use-wsl-lightpanda` 双次复跑 `status=passed`。P6 已收敛 validation evidence schema：signals 显式携带 `collectorScope`、`runtimeAdapter`、`targetProfileBrowser`、`failureReason`，旧报告读取时从 legacy `detail` 自动补齐 metadata。P7 已新增 fingerprint observation audit：只从 WebRTC/canvas/audio/leak observed signals 统计真实观察覆盖，不把 `80` declared controls、`26` runtime projected fields 或 `450+` target-only signals 计入 observed proof。
- Runtime adapter / release smoke contract 已接入桌面 API：记录 Fake、Lightpanda、headed_external adapter 边界、release installer 路径、Win11 baseline 和性能预算目标；真实 Lightpanda/CDP operator smoke 与冷启动/RSS/进程数测量仍需单独执行。

## 未完成边界

### Mainline remaining

- provider-side proxy rotation write 已闭环 (multi-agent worktree closeout)。
- Synchronizer native broadcast write path 已闭环 (physical SetWindowPos + deterministic ordering)。
- Recorder / Templates native-first de-fallback closure 已闭环 (desktop session guard + empty-state fix)。
- 最终 Win11 packaging / operator acceptance polish 仍需在不重开 scope 的前提下完成（本轮已收敛 Mainline P0）。

### Overall remaining `70%`

- Validation Board 已有前端 MVP，并已接入 DNS/transport 首批 observed native collector、desktop WebView scoped WebRTC/canvas/audio/storage browser probes、profile browser runtime `validation_probe` action、本地 JSON report、history list、profile-level evidence export、P5 smoke 脚本、P6 explicit evidence metadata 和 P7 fingerprint observation audit。WebRTC/audio capability warning、canvas `toDataURL` missing 等真实失败/警告原因必须保留在 report 中。
- CAPTCHA/SMS/Email 仍是 handler/route 与服务代码边界部分落地，并已有 production readiness contract 可报告凭证、manager wiring、CDP 自动化、operator UI blocker；尚未完成真实 provider 验收和自动化闭环。
- runtime materialization depth 已从 `12` 扩到 `26` projected fields；P7 已有 observed audit summary，但仍不能把 `80` declared controls 或 `450+` target signals 报成已 observed。
- `450+` fingerprint signal taxonomy / full observation coverage 未落地；当前 P7 只是 WebRTC/canvas/audio/leak observed audit，不是 450+ 全量采集。
- `450+` event taxonomy 未落地。
- `SessionBundle` profile-level export、import preflight、dry-run 和 confirmed local restore write path 已落地；跨机器 profile portability 仍需真实环境验收。
- headed runtime realism、kernel strategy、AdsPower-grade catch-up 仍是整体目标轨道；当前仅有 adapter/release smoke contract，不是 headed runtime 实现。
- external browser integration 已有计划和 contract 边界，但不是已交付 runtime depth；主仓库仍不托管 Chromium/Firefox fork。

## 当前下一步

Mainline P0、自动化 release gate、Validation Board 前端 MVP 已闭环。下一步方向：
1. P9 推进 CAPTCHA/SMS/Email production manager wiring/config，但不宣称真实 provider 闭环，除非凭证和验收可用
2. P10 扩展 behavior taxonomy/workflow audit，并继续避免把 `450+` 目标写成已交付
3. P11 推进 runtime adapter/headed realism/performance measurement，不托管 Chromium/Firefox fork
