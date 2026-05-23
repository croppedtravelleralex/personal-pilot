# Improvement Backlog

## Mainline release evidence

| 领域 | 待改进项 | 当前状态 | 退出条件 |
| --- | --- | --- | --- |
| Proxy / IP | provider-side proxy rotation write | 已闭环 | 真实 provider action、rollback、cooldown、retry 均走 typed native chain |
| Synchronizer | native broadcast write closure；set-main / work-area-aware layout 已落地 | 已闭环 | staged-only 不再是主 operator route，broadcast write 走 typed native chain |
| Recorder / Templates | native-first de-fallback closure | 已闭环 | release-default path 不依赖 fallback |
| Release | Win11 final packaging / operator acceptance polish | 自动化门禁已通过；P13 已新增外部分发 readiness 文档；人工 smoke 待执行 | Type check、Vite build、Win11 baseline、Tauri release build、Windows local verify 同时通过并记录；外部分发前按 `docs/24-external-distribution-readiness.md` 完成人工 smoke |

## Overall remaining `60%`

| 领域 | 待改进项 | 当前状态 | 退出条件 |
| --- | --- | --- | --- |
| Validation | detector / leak / DNS / WebRTC / canvas / audio / worker / transport board | 前端 MVP、DNS/transport observed report、desktop WebView scoped WebRTC/canvas/audio/storage probes、profile browser runtime `validation_probe` action、WebRTC/leak native warning contract、history list、profile-level export、`validation_lightpanda_smoke` 和 P6 explicit evidence metadata 已接入；WSL2 Lightpanda nightly 真实 CDP smoke 已双次复跑 `status=passed`；P14 新增 smoke/report 脚本入口 | 继续扩大 observed coverage，并继续保留 scope/adapter/target/failure metadata |
| Fingerprint | 从 `12` runtime projected fields 扩展 runtime depth | 已扩到 `26` projected fields（`25` control-supported + derived `platform`）；P7 已新增 WebRTC/canvas/audio/leak observed audit；P14 已新增 `450` fingerprint signal taxonomy seed | declared / applied / observed 可解释且可验证；taxonomy seed 不得计入 observed proof |
| Session | 完整 `SessionBundle` 和 profile portability | profile-scoped export、import preflight、dry-run、confirmed local restore write path 已落地；P14 新增 portability smoke contract 脚本 | 跨机器 profile portability、restart continuity 和 restored payload 真实环境验收一起成立 |
| Behavior | 从 `13` primitives 扩展 replayable `450+` taxonomy | P10 behavior audit contract 已落地；P14 新增 `450` behavior event taxonomy seed 和 audit path | 事件数量来自真实 taxonomy；taxonomy seed 仍需 replay runtime 证据才能算交付 |
| Provider closure | CAPTCHA/SMS/Email production manager wiring/config | production readiness contract、acceptance checklist 和 Settings operator surface 已落地；P14 新增 provider acceptance preflight 脚本 | 真实 provider 验收、CDP detect/fill、operator UI 和失败处理闭环 |
| Runtime | headed realism、kernel strategy、adapter boundary | adapter/release smoke contract、P11 measurement pending visibility 和 P21 runtime adapter evidence gate 已落地；P14 新增 release performance smoke 脚本并测得 warning：`9301ms / 411MB / 14 processes`，超过 `2000ms / 220MB / 4 processes` 默认预算；headed runtime 未实现 | 有稳定 adapter contract、真实 runtime 验证证据，并给出性能超预算 mitigation |
| Runtime | release performance mitigation | 已新增 `docs/release-performance-mitigation-plan.md` | 复测 release artifact，至少一项指标实质改善，剩余超预算原因写入 current-state |
| Benchmark | AdsPower boundary refresh | P12 已落地 guard，P21 已新增 machine-readable deferred gate report；评分刷新暂缓 | B1-B5 有新证据后再刷新评分；没有证据时只保留 deferred 结论 |

## 固定风险

- 把 `80` declared controls 误报成 `80` runtime-applied fields。
- 把 `450+` fingerprint / event target 误报成当前已交付。
- 把 mock / fallback / staged 默认路径算作 delivery closure。
- 用历史 `77% / 23%` 或 `82% / 18%` 覆盖当前 live truth。
- 在 Mainline closeout 阶段引入 Overall 级大范围重构。
- P5/P6 已通过后仍把 WebRTC/audio warning 或 canvas failure 写成全部成功，或丢失 `failureReason`。

## 维护规则

新增 backlog 时必须标清属于 Mainline release evidence 还是 Overall `60%`。不能确定归属时先放 Overall，等有代码证据后再提升到 Mainline release gate。
