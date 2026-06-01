# AI Maintenance Playbook

## 接手顺序

1. 读 `docs/README.md`。
2. 读 `docs/02-current-state.md`。
3. 读 `docs/final-goal-progress-breakdown.md`。
4. 读 `docs/19-phase-plan-and-scorecard.md`。
5. 读 `docs/03-roadmap.md` 和 `docs/04-improvement-backlog.md`。
6. 如任务涉及 M4-M20 执行、harness、commit 切片或验收边界，读 `docs/40-m4-m20-execution-board.md`。
7. 如任务涉及发布、外部分发或发布说明，读 `docs/24-external-distribution-readiness.md`。
8. 需要执行任务时，再按范围读相关代码和测试。

## 默认事实

- Mainline：`100% / 0% / green`
- Overall：`40% / 60% / yellow`
- Fingerprint：`80` declared controls / `26` runtime projected fields (`25` control-supported + derived `platform`) / `450` taxonomy seed / full observed coverage pending
- Behavior：`13` shipped primitives / `8` page archetypes / P10 audit contract / `450` taxonomy seed / full replay runtime pending
- Session：cookie / localStorage / sessionStorage restart continuity 已落地；profile-scoped `SessionBundle` export、import preflight、dry-run、confirmed local restore write path 已落地，跨机器 profile portability 验收未落地

## 汇报规则

- 先说明当前结论，再给证据。
- Mainline remaining `0%` 与 Overall remaining `60%` 必须分开写。
- 不把 AdsPower catch-up、`50+`、`450+` 写成当前 shipped runtime depth。
- 不复活 `77% / 23%` 或 `82% / 18%` 作为 live truth。
- 外部分发前要说明哪些限制已保留，哪些人工 smoke 尚未执行。
- 报告尽量短，优先列 landed result、当前阻塞、下一步。

## 修改规则

- 当前状态变化：更新 `02-current-state.md`。
- 路线变化：更新 `03-roadmap.md`。
- 风险、债务、后续项：更新 `04-improvement-backlog.md`。
- 接手顺序、验证纪律变化：更新 `05-ai-maintenance-playbook.md`。
- 外部分发、发布说明或人工 smoke 边界变化：更新 `24-external-distribution-readiness.md`。
- 根目录入口保持薄；除非用户要求，不改根目录文档。

## 验收规则

 meaningful change 完成前至少确认：

- 文档没有把历史进度当成 live truth。
- 文档没有把 Overall 目标当成 Mainline 已交付。
- 涉及代码改动时，按项目规则跑 type check、release build、Win11/Tauri enforcement。
- 文档-only 改动也要检查 canonical 入口是否存在且互相指向有效文件。
- 涉及 M4 provider readiness 时，`scripts/m4_acceptance_gate.ps1` 会刷新 provider preflight；`provider_dry_run_contract` passed 只代表本地 dry-run/failure taxonomy 报告完整，真实 provider smoke 仍必须单独保留 blocked/accepted 证据。
- 涉及 M4 SessionBundle operator loop 时，`session_bundle_operator_contract` passed 只代表 Settings UI、desktop wrapper、TS 类型和 Rust 本机 export/preflight/dry-run/confirmed restore chain 已接通；跨机器 portability 仍必须保留 blocked，直到第二环境 report 存在。
- 涉及 M4 runtime adapter operator loop 时，`runtime_adapter_operator_contract` passed 只代表 Dashboard 已读取 release smoke contract，并按证据强度展示 adapter、runner、profile/fingerprint evidence 和 blocker；完整 headed realism、B1-B5、远程代理/TLS、provider、portability 和 AdsPower refresh 仍按各自 report 判定。
- 涉及 M4 typed facade shrink 时，`typed_facade_shrink_contract` passed 只代表 synchronizer high-traffic DTO 和 browser Wails binding source contract 已类型化；不得写成 `tauriWailsBridge` 已移除、所有 browser payload schema 已规范化或所有 core bridge API 已统一。
- 涉及 M4 safety/logging 时，`safety_logging_contract` passed 只代表 logger 默认敏感字段、写入路径和 Text/JSON formatter 脱敏 source/test contract 已存在；不得写成真实 provider 凭证 smoke 已通过、历史报告已清洗或外部分发日志审计已完成。
- 涉及 M4 total gate 时，`passed_with_expected_external_blockers` 代表本地 M4 合同可用且没有 failed gate；仍不得把 provider、远程代理/TLS、跨机器 SessionBundle、profile-browser comparison 或 AdsPower refresh 写成已完成。
- 涉及 profile-browser comparison 时，先用 Dashboard 的 `采集 WebView` 在真实桌面 WebView 中生成 desktop report，再在同一时间窗口刷新 profile-browser report；`profile_browser_comparison_gate` v3 只有双边同窗且 category 可比才允许 passed。
