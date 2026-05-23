# AI Maintenance Playbook

## 接手顺序

1. 读 `docs/README.md`。
2. 读 `docs/02-current-state.md`。
3. 读 `docs/final-goal-progress-breakdown.md`。
4. 读 `docs/19-phase-plan-and-scorecard.md`。
5. 读 `docs/03-roadmap.md` 和 `docs/04-improvement-backlog.md`。
6. 如任务涉及发布、外部分发或发布说明，读 `docs/24-external-distribution-readiness.md`。
7. 需要执行任务时，再按范围读相关代码和测试。

## 默认事实

- Mainline：`100% / 0% / green`
- Overall：`30% / 70% / yellow`
- Fingerprint：`80` declared controls / `26` runtime projected fields (`25` control-supported + derived `platform`) / `450+` target-only
- Behavior：`13` shipped primitives / `8` page archetypes / P10 audit contract / `450+` target-only
- Session：cookie / localStorage / sessionStorage restart continuity 已落地；profile-scoped `SessionBundle` export、import preflight、dry-run、confirmed local restore write path 已落地，跨机器 profile portability 验收未落地

## 汇报规则

- 先说明当前结论，再给证据。
- Mainline remaining `0%` 与 Overall remaining `70%` 必须分开写。
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
