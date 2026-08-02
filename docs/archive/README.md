# 文档归档说明

本目录存放**已退出维护**的过程性文档，仅供历史查阅。当前真相源始终在 `docs/README.md` 所列 canonical 文件。

## 2026-06-29 清理

| 原路径 | 处置 |
|--------|------|
| `/summaries/cycle-*.md` | 已删除（2026-03 agent 循环摘要，与当前 live truth 无关） |
| `/task_plan.md` | 根目录保留 stub，指向外部 gateway 任务或作废 |
| `/findings.md` | 根目录保留 stub，指向 `/docs/archive/` |
| `/EXECUTION_LOG.md` | 保留 Round 历史；新月志写入 `docs/logs/YYYY/` |

## 2026-07-08 清理（2026-04 过程笔记）

以下为 2026-04 阶段的一次性性能剖析 / 选择链 / provider 地域样本 / stage-entry 过程笔记，已被 `docs/02-current-state.md`、`docs/03-roadmap.md`、`docs/04-improvement-backlog.md` 完全取代，已 `git mv` 到本目录：

| 归档文件 | 类别 |
|----------|------|
| `selection-verify-profiling-2026-04-02.md` | 2026-04 性能剖析 |
| `profiling-summary-2026-04-02.md` | 2026-04 性能剖析 |
| `perf-probe-sample-2026-04-02.md` | 2026-04 性能探针样本 |
| `perf-probe-real-flow-sample-2026-04-02.md` | 2026-04 性能探针样本 |
| `perf-probe-branch-summary-2026-04-02.md` | 2026-04 性能探针样本 |
| `provider-region-entry-validation-sample.md` | provider 地域样本 |
| `provider-region-entry-validation-sample-round-2.md` | provider 地域样本 |
| `provider-region-entry-validation-sample-round-3.md` | provider 地域样本 |
| `selection-to-trustscore-gap.md` | 选择链过程笔记 |
| `explainability-wording-next-step.md` | 选择链过程笔记 |
| `stage-2-boundary-plan.md` | 早期 stage 计划 |
| `reopen-conditions-next-stage.md` | 早期 stage 计划 |
| `next-stage-mainline-after-provider-scope.md` | 早期 stage 计划 |
| `next-mainline-after-refresh-scope-closure.md` | 早期 stage 计划 |
| `post-explain-phase-next-mainline.md` | 早期 stage 计划 |
| `entry-summary-update-example.md` | stage-entry 维护示例 |
| `stage-entry-maintenance-example-future-stage.md` | stage-entry 维护示例 |

## 2026-07-09 清理（被 49–56 取代的专题设计 + 根目录历史）

### docs/ 专题（现行见右侧 canonical）

| 归档文件 | 取代文档 |
|----------|----------|
| `39-adversarial-trust-inheritance.md` | `docs/50-asymmetric-dominance-architecture.md` |
| `28-network-identity-layer.md` | `docs/50` §A + `docs/49` 工作流 C |
| `34-environment-signature-sampling.md` | `docs/52-device-persona-coherence-engine.md` |
| `30-behavioral-fidelity-layer.md` | `docs/53-behavior-biometrics-l5-execution-bridge.md` |
| `identity-fingerprint-network-session-plan.md` | `docs/49`–`docs/56` |
| `verify-risk-to-trustscore-plan.md` | `docs/55-account-health-observability.md` |
| `trust-score-convergence-plan.md` | `docs/55` |
| `provider-region-entry-conditions.md` | `docs/29-proxy-supply-chain.md` |
| `next-stage-local-windows-execution-board.md` | `docs/03-roadmap.md` |
| `entry-summary-update-checklist.md` | `docs/05-ai-maintenance-playbook.md` |

### 根目录历史（`docs/archive/root/`）

| 归档文件 | 兼容 stub（根目录） |
|----------|---------------------|
| `LONG_TERM_ROADMAP.md` | `LONG_TERM_ROADMAP.md` |
| `DESIGN_NETWORK_IDENTITY.md` | `DESIGN_NETWORK_IDENTITY.md` |
| `FINGERPRINT_BOUNDARY.md` | `FINGERPRINT_BOUNDARY.md` |
| `LIGHTPANDA_V1_PLAN.md` | `LIGHTPANDA_V1_PLAN.md` |
| `VISION.md` | `VISION.md` |
| `GOLDEN_FEATURES.md` | `GOLDEN_FEATURES.md` |
| `MODULE_SCOPE.md` | `MODULE_SCOPE.md` |
| `EXECUTION_CHECKLIST.md` | `EXECUTION_CHECKLIST.md` |
| `EXECUTION_ENGINE_ARTIFACT_STRATEGY.md` | `EXECUTION_ENGINE_ARTIFACT_STRATEGY.md` |
| `EXECUTION_STATE_MACHINE.md` | `EXECUTION_STATE_MACHINE.md` |
| `ROUND_SCHEDULER.md` | `ROUND_SCHEDULER.md` |
| `STAGE_SUMMARY_2026-03-31.md` | `STAGE_SUMMARY_2026-03-31.md` |

## Stealth / 指纹优化 canonical 轨道（勿归档）

`docs/49`（战术）· `docs/50`（战略）· `docs/51`（广度）· `docs/52`（相干）· `docs/53`（行为 L5）· `docs/54`（并发）· `docs/55`（可观测）· `docs/56`（边界收口）· 统一入口 `PLAN.md`

`docs/42-asymmetric-stealth-architecture.md` 保留为 2026-06 接入摘要；详细执行以 `docs/50` 为准。

## 何时归档

- 阶段性计划已合并进 `docs/02-current-state.md` 或 `docs/04-improvement-backlog.md`
- 文档与代码/验收口径明显不一致且不再更新
- 多份文档描述同一「当前状态」造成接手混淆
- 被 `docs/49`–`docs/56` 完整取代的专题设计

## 勿归档

- `docs/02-current-state.md`、`docs/05-ai-maintenance-playbook.md`、`docs/45-stealth-platform-handoff.md`
- 仍被脚本或 gate 引用的 runbook（如 `docs/43-capability-scenario-test-suite.md`）
- `PLAN.md`、`docs/41-user-requirements-execution-plan.md`
