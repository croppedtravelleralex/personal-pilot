

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## Workflow Action Dispatch

- 读取目标文档并重新排序下一阶段事项 [doc_sync]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：先对齐 VISION/CURRENT_DIRECTION/TODO，避免跑偏
- 生成 3–5 个下一阶段建议 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：为执行前两个动作提供稳定输入

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## 2026-04-01 explainability traceability pass

- 为 `summary_artifacts` 补充 `run_id / attempt / timestamp` 溯源字段，并在 task/run/status 聚合返回中自动补全上下文。
- 为 `tasks` API 返回补充 `started_at / finished_at`，统一状态与解释链的时间锚点。
- 为 `runs` 表新增并兼容迁移 `result_json`，修复 `get_task_runs` 错误复用 `task.result_json` 的问题，改为读取 run 自身结果。
- 为 `/proxies/:id/explain` 补充 `trust_score_cached_at / explain_generated_at / explain_source`，统一 explainability 溯源口径。
- 验证结果：`cargo test` 全绿（31 unit + 71 integration）。


## Workflow Action Dispatch

- 读取目标文档并重新排序下一阶段事项 [doc_sync]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：先对齐 VISION/CURRENT_DIRECTION/TODO，避免跑偏
- 生成 3–5 个下一阶段建议 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：为执行前两个动作提供稳定输入

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## Workflow Action Dispatch

- 读取目标文档并重新排序下一阶段事项 [doc_sync]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：先对齐 VISION/CURRENT_DIRECTION/TODO，避免跑偏
- 生成 3–5 个下一阶段建议 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：为执行前两个动作提供稳定输入

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## 2026-04-01 explainability schema normalization pass

- 统一 `summary_artifacts` 的 source 命名：`proxy_selection` → `selection.proxy`，runner 来源统一为 `runner.*` 前缀。
- runner 执行类摘要统一归类到 `execution` category，并将 fake/lightpanda 的 key 统一到 `<task_kind>.execution` 口径。
- 增加 artifact 归一化逻辑：category/source/severity 在 API 层统一标准化，避免历史/异源数据口径漂移。
- 补充 run-level trace 与 explain trace 的集成测试，锁定 `run_id / attempt / timestamp` 与 `explain_generated_at / explain_source / trust_score_cached_at`。
- 验证结果：`cargo test` 全绿（31 unit + 73 integration）。


## Workflow Action Dispatch

- 读取目标文档并重新排序下一阶段事项 [doc_sync]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：先对齐 VISION/CURRENT_DIRECTION/TODO，避免跑偏
- 生成 3–5 个下一阶段建议 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：为执行前两个动作提供稳定输入

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## 2026-04-01 candidate preview typing pass

- 将 `candidate_rank_preview` 从 `Vec<serde_json::Value>` 收口为强类型 `CandidateRankPreviewItem`。
- `compute_candidate_preview_with_reasons` 改为直接返回 typed DTO，去掉内部 JSON 硬拼装。
- `/proxies/:id/explain` 与相关解析逻辑统一复用 typed preview；`winner_vs_runner_up_diff` 直接从 typed preview 首项提取。
- 补充 typed preview 集成测试，锁定 `id/provider/region/score/trust_score_total/summary/winner_vs_runner_up_diff` 形状。
- 验证结果：`cargo test` 全绿（31 unit + 74 integration）。


## Workflow Action Dispatch

- 读取目标文档并重新排序下一阶段事项 [doc_sync]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：先对齐 VISION/CURRENT_DIRECTION/TODO，避免跑偏
- 生成 3–5 个下一阶段建议 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：为执行前两个动作提供稳定输入

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## 2026-04-01 explainability assembler extraction pass

- 新增 `src/api/explainability.rs`，将 fingerprint/proxy/summary/explainability 解析与归一化逻辑从 `handlers.rs` 中抽离。
- 新增 `TaskExplainability` 与 `build_task_explainability()`，统一 task/status 详情响应的 explainability 组装口径。
- `handlers.rs` 现在只保留接口流程与查询编排，减少重复解析与多处散落的字段拼装。
- `get_task_runs` 继续复用抽出的 summary enrichment 能力，保持 run 级 traceability 口径不回退。
- 验证结果：`cargo test` 全绿（31 unit + 74 integration）。

