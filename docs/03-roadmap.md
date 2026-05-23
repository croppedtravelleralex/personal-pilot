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
   - 2026-05-23 已通过 `scripts/windows_local_verify.ps1 -SkipContinuityTest`
   - 覆盖 typecheck、Vite build、Win11 baseline、Rust lib/full tests、Tauri release build
   - 生成 Win11 NSIS installer：`src-tauri/target/release/bundle/nsis/PersonaPilot_0.1.0_x64-setup.exe`
   - 后续发布前可追加人工 operator smoke 和 continuity integration test

## Now：Overall remaining `70%`

目标：从 closeout-ready desktop app 走向完整平台能力。此轨道不得冒充 Mainline 已交付。

1. Validation foundation
   - Validation Board 前端 MVP 已落地。
   - 形成 detector / leak / DNS / WebRTC / canvas / audio / worker / transport evidence。
   - 区分 declared / applied / observed。
   - DNS/transport 首批 observed native collector、本地 JSON report、report history、profile-level evidence export 已接入。
   - desktop WebView scoped WebRTC/canvas/audio/storage probes 已接入，用于记录当前桌面壳浏览器 API 能力；不等同于 profile browser runtime probe。
   - profile browser runtime `validation_probe` action 已接入 runner；Lightpanda/CDP 可用时可生成 WebRTC/canvas/audio/leak runtime scoped signals，FakeRunner 只生成 warning stub。
   - P5 已新增 `validation_lightpanda_smoke` 可复跑入口，并通过 WSL2 Lightpanda nightly 真实 CDP 复跑，report `status=passed`。
   - P6 已统一 desktop WebView、profile browser、FakeRunner/native validation signals 的 explicit evidence metadata：`collectorScope`、`runtimeAdapter`、`targetProfileBrowser`、`failureReason`；旧报告读取时从 legacy `detail` 补齐字段。

2. Fingerprint runtime depth
   - 已从 `80` declared controls 和 `12` runtime projected fields 推进到 `26` runtime projected fields（`25` control-supported + derived `platform`）。
   - P5 真实 Lightpanda/CDP smoke 已通过，P6 evidence schema 已收敛，P7 fingerprint observation audit 已接入 Validation Board；下一步继续加深真实采集，不把 projected/applied fields 报成 observed signals。
   - 保持 control / derived / observation layers 分离。

3. Session / proxy orchestration
   - 已将 restart continuity 推进到 profile-scoped `SessionBundle` export、import preflight、dry-run、confirmed local restore write path。
   - 下一步补齐跨机器 profile portability 验收、lease / cooldown / health / rollback。

4. Behavior and automation depth
   - 已新增 P10 behavior audit contract：`13` shipped primitives、`8` page archetypes、workflow graph、debug trace、manual gate、recovery semantics、`450+` target-only 边界可在 Automation surface 查看。
   - 下一步才是把 audit contract 扩展为真实 replayable `450+` event taxonomy。
   - CAPTCHA/SMS/Email 已有 production readiness contract、acceptance checklist 和 Settings operator surface；下一步是真实 manager wiring、CDP detect/fill 和 provider acceptance。

5. Runtime adapter and external integration
   - Runtime adapter / release smoke contract 已接入，覆盖 Fake、Lightpanda、headed_external 边界和 release artifact 检查。
   - P11 已新增 release measurement target vs measured pending 字段，并在 Overview 暴露 adapter boundary；真实 cold start/RSS/process count smoke 仍需 operator 执行。
   - 下一步只吸收高 ROI 外部浏览器思路。
   - 不把主仓库变成 Chromium / Firefox fork host。
   - AdsPower benchmark refresh 等 B1-B5 有证据后再做。

## Later：成熟度与评分刷新

- 只有新 shipped evidence 出现时才提高 capability score。
- AdsPower comparison 只用官方公开边界和本仓库可验证证据刷新。
- `50+`、`450+`、AdsPower catch-up、external integration 继续归入 Overall track，不能写成当前 runtime depth。
