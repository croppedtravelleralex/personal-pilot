# Docs 维护入口

本目录保存 PersonaPilot 的内部维护真相源。根目录文档只做兼容入口；接手、汇报、规划时先读这里。

## 下一位 AI 先读

1. `docs/README.md`：阅读顺序和维护纪律。
2. `docs/02-current-state.md`：当前 live truth。
3. `docs/final-goal-progress-breakdown.md`：双轴进度口径和关键数量。
4. `docs/19-phase-plan-and-scorecard.md`：阶段计划、工作量、评分和 AdsPower 边界。
5. `docs/03-roadmap.md`：Now / Next / Later 路线。
6. `docs/04-improvement-backlog.md`：待办池和风险。
7. `docs/05-ai-maintenance-playbook.md`：AI 接手、更新和验收规则。

## 当前报告口径

- Mainline delivery：`95% / 7% / green`
- Overall end-state：`30% / 70% / yellow`
- Fingerprint：`80` declared controls / `12` runtime projected fields / `450+` target-only
- Behavior：`13` shipped primitives / `450+` target-only
- Session：cookie / localStorage / sessionStorage restart continuity 已落地；完整 `SessionBundle` 未落地

历史 `77% / 23%` 或 `82% / 18%` 只能作为历史上下文，不作为 live truth。

## 真相来源优先级

1. 当前代码、配置、测试、构建和验证结果。
2. `docs/02-current-state.md`、`docs/final-goal-progress-breakdown.md`、`docs/19-phase-plan-and-scorecard.md`。
3. `TODO.md`、`STATUS.md`、`PROGRESS.md`、`docs/root-entrypoint-map.md`。
4. 其他 `docs/` 历史设计文档。
5. 根目录说明文档和聊天上下文。

## 更新纪律

- 当前事实变更先更新 `02-current-state.md`。
- 路线变化更新 `03-roadmap.md`。
- 未做、风险、技术债更新 `04-improvement-backlog.md`。
- 接手规则变化更新 `05-ai-maintenance-playbook.md`。
- 不把 Overall 目标数字伪装成 Mainline 已交付。
- 不把 staged / fallback / mock 默认路径算作闭环交付。
