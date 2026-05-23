# PersonaPilot progress

This root progress file is now a compatibility entrypoint.
Canonical progress and reality tracking live in:

- `/docs/02-current-state.md`
- `/docs/final-goal-progress-breakdown.md`
- `/docs/19-phase-plan-and-scorecard.md`
- `/docs/03-roadmap.md`
- `/docs/17-full-app-audit-progress-reset.md`

## Current Reporting Rule

Progress must be reported with the dual-axis rule:

- current shipped app / closeout / native mainline -> `100% / 0% / green`
- complete app / AdsPower catch-up / `50+` control / `450+` fingerprint or event target -> `30% / 70% / yellow`
- current reality anchors stay fixed at `80` declared controls, `12` runtime projection fields, `13` behavior primitives, and restart continuity landed
- `450+` fingerprint signals, `450+` event taxonomy, and richer AdsPower-grade realism remain future overall-track work
- detailed phase plan, scorecard, and benchmark summary live in `/docs/19-phase-plan-and-scorecard.md`

## 2026-05-23 Release gate evidence

- `scripts/windows_local_verify.ps1 -SkipContinuityTest` 已通过。
- 覆盖：`pnpm typecheck`、`pnpm build`、Win11 Tauri baseline enforcement、`cargo test --lib -- --test-threads=1`、`pnpm desktop:release`、`cargo test --quiet`。
- Win11 NSIS installer 已生成：`src-tauri/target/release/bundle/nsis/PersonaPilot_0.1.0_x64-setup.exe`。
- 下一阶段主线切到 Overall `70%` 的 Validation Board MVP，不提高 capability score，直到有 observed evidence。

## 2026-05-23 Validation WebRTC/leak contract signals

- Validation report now includes WebRTC and leak observed-category warning signals.
- These signals are honest contract warnings: they record the missing browser-scoped probe boundary instead of pretending leak/WebRTC probing is complete.
- Next implementation slice is browser-scoped WebRTC/leak probing plus canvas/audio collectors.
## 2026-05-23 Validation desktop WebView probes

- Validation evidence collection now runs desktop WebView scoped browser API probes before writing the Tauri JSON report.
- Added observed signals for WebRTC ICE candidate capability, canvas render sample, AudioContext sample rate, and storage-scope availability.
- Scope is explicitly recorded as `desktop-webview; target-profile-browser=false`, so this does not close profile browser runtime / CDP scoped probing or raise the Overall `30% / 70%` status.
## 2026-05-23 Validation report history/export

- 新增 validation report history list native command 与 desktop wrapper。
- 新增 profile-level evidence export，导出本地 validation reports 为 JSON evidence 包。
- Validation 页面新增 history 加载与 export evidence 控制。
- WebRTC/canvas/audio/leak collector 仍是下一批，不提高 Overall 终态口径。
## 2026-05-23 Validation observed collectors v1

- 新增 `collect_validation_report` Tauri command，统一从 `src/services/desktop.ts` 暴露给前端。
- Validation 页面可触发首批 observed 采集，并展示最近 report。
- 首批 report 覆盖 DNS `example.com` 解析与 HTTPS transport HEAD probe，并写入本地 `reports/validation/*.json`。
- 这只是 DNS/transport 最小闭环，不代表 WebRTC/canvas/audio/leak 或完整 detector 闭环。
## 2026-05-23 Validation Board MVP

- 新增 `Validation` 桌面页面与导航入口。
- 已覆盖 8 类 evidence：detector、leak、DNS、WebRTC、canvas、audio、worker、transport。
- Board 明确拆分 `declared / applied / observed`，避免把 declared controls 误报成 runtime-applied fields。
- 当前 observed 层仍是前端 board-level 状态，下一步接真实采集命令、持久化 report、profile-level export。

## 2026-05-22 Integration/API follow-up

- CAPTCHA 后端 handler/route 与 solver 代码边界已部分落地：`internal/captcha`、2Captcha、Capsolver、`/api/captcha/solve`、`/api/captcha/solve-token`、config/balance；生产 manager wiring/config 尚未接入，当前端点可能返回 service unavailable。
- SMS 后端 handler/route 与 provider 代码边界已部分落地：`internal/sms`、5sim、SMSPool、buy/status/cancel/balance；生产 manager wiring/config 尚未接入，当前端点可能返回 service unavailable。
- Email 已推进到 `EmailService` + inbox / wait-code REST API，`email_sessions` 表初始化为 best-effort；`CreateInbox` 当前保存失败会记录日志并继续返回。
- Backup 单体已拆为 core/import/merge/utils；browser launch args 和 process monitor 已迁入 `internal/browser`。
- 仍不能宣称完整 CAPTCHA/SMS/Email 自动化闭环；CDP 检测、截图/sitekey 抽取、填入、真实 provider 验收、operator UI 仍属 Overall 70% 工作。

## 2026-05-21 Mainline 闭环 (Multi-Agent)

- provider 级代理 IP 轮换引擎落地 — 真实 HTTP POST/PUT/PATCH 到 provider 端点，替换 `unsupported_provider_config` 桩代码
- synchronizer 原生批量/广播写入落地 — 物理 `SetWindowPos` 窗口排布，确定性排序
- recorder/templates native-first 降级闭环 — desktop session 守卫，空状态模板选择修复
- engineering hygiene 修复：SQLite 路径 env var 降级、删除 `package-lock.json`、添加 CI workflow (cargo test + clippy + pnpm typecheck)、`.env` 到 `.gitignore`
- 所有代码经 4 个独立 subagent 审查；2 CRITICAL + 1 HIGH + 4 MEDIUM 问题已发现并修复
- 4 个并行 workstream 零冲突合并

主线剩余：`93% → 100%` (3 个 P0 项闭环)

## 2026-04-16 Mainline Delta

- landed `Tasks -> Automation` surface unification
- landed provider-aware / sticky-aware `changeProxyIp` semantics
- landed recorder desktop step-write and synchronizer live read/focus
- restored the full Rust gate to green, including `integration_api`, `integration_lightpanda_runner`, and deterministic `humanize` retry coverage
- re-verified `pnpm desktop:release` and `powershell -ExecutionPolicy Bypass -File scripts/windows_local_verify.ps1 -SkipContinuityTest`
- cleared the previous Vite chunk warning with route-level lazy loading
