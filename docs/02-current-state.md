# Current State

Updated: 2026-05-23 (Asia/Shanghai)

## 当前 live truth

- Mainline delivery：`100% / 0% / green` (3 P0 items closed)
- Overall end-state：`30% / 70% / yellow`
- 第一族控制 schema 已声明 `80` 个 core control fields。
- 当前 runtime projection 是 `26` 个 env-backed fingerprint fields，其中 `25` 个来自 first-family control fields，另含 derived `platform`。
- 当前 behavior runtime 已交付 `13` 个 primitives。
- cookie / localStorage / sessionStorage 跨 app restart 持久化与恢复已落地。

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
- CAPTCHA 后端 handler/route 与 solver 代码边界已部分落地：`internal/captcha`、2Captcha、Capsolver、`/api/captcha/*` 核心端点；生产 manager wiring/config 尚未接入。
- SMS 后端 handler/route 与 provider 代码边界已部分落地：`internal/sms`、5sim、SMSPool、`/api/sms/*` 核心端点；生产 manager wiring/config 尚未接入。
- Email 已从内部模块推进到 `EmailService` + inbox / wait-code REST API。
- 备份单体已拆成 core/import/merge/utils；browser launch args 与 process monitor 已迁入 `internal/browser`。
- Validation Board 已落地到桌面导航：8 类 evidence（detector / leak / DNS / WebRTC / canvas / audio / worker / transport）按 declared / applied / observed 分层展示；DNS/transport observed 首批 native collector、desktop WebView scoped WebRTC/canvas/audio/storage probes、profile browser runtime validation probe action、WebRTC/leak native contract warning、JSON report、report history 和 profile-level evidence export 已接入。真实 profile browser-scoped evidence 依赖 `PERSONA_PILOT_RUNNER=lightpanda` 与 Lightpanda/CDP 可用；FakeRunner 只记录 warning stub。

## 未完成边界

### Mainline remaining

- provider-side proxy rotation write 已闭环 (multi-agent worktree closeout)。
- Synchronizer native broadcast write path 已闭环 (physical SetWindowPos + deterministic ordering)。
- Recorder / Templates native-first de-fallback closure 已闭环 (desktop session guard + empty-state fix)。
- 最终 Win11 packaging / operator acceptance polish 仍需在不重开 scope 的前提下完成（本轮已收敛 Mainline P0）。

### Overall remaining `70%`

- Validation Board 已有前端 MVP，并已接入 DNS/transport 首批 observed native collector、desktop WebView scoped WebRTC/canvas/audio/storage browser probes、profile browser runtime `validation_probe` action、本地 JSON report、history list 和 profile-level evidence export；后续需在真实 Lightpanda/CDP 环境下做 operator smoke，把 FakeRunner warning stub 替换为真实 profile runtime evidence。
- CAPTCHA/SMS/Email 仍是 handler/route 与服务代码边界部分落地，尚未完成 production manager wiring、CDP 自动化检测、填入、真实 provider 验收和 operator UI 闭环。
- runtime materialization depth 已从 `12` 扩到 `26` projected fields；仍不能把 `80` declared controls 或 `450+` target signals 报成已 observed。
- `450+` fingerprint signal observation / audit coverage 未落地。
- `450+` event taxonomy 未落地。
- 完整 `SessionBundle`、profile portability、import/export contract 未落地。
- headed runtime realism、kernel strategy、AdsPower-grade catch-up 仍是整体目标轨道。
- external browser integration 已有计划，但不是已交付 runtime depth。

## 当前下一步

Mainline P0、自动化 release gate、Validation Board 前端 MVP 已闭环。下一步方向：
1. 在真实 Lightpanda/CDP 环境执行 Validation profile runtime smoke，确认 WebRTC/leak/canvas/audio report 可重复
2. 扩展 fingerprint runtime depth，并保持 declared / applied / observed 三层分离
3. CAPTCHA/SMS/Email production manager wiring/config 只作为后续集成切片，不宣称自动化闭环
