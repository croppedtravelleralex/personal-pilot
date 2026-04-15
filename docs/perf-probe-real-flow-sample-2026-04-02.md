# Perf Probe 真实任务流样本记录（2026-04-02）

## 样本范围

本轮使用 `PP_PERF_PROBE=1` 对更接近真实任务流的三条路径采样：

1. `verify_proxy_task_kind_executes_and_persists_result`
2. `auto_selection_result_exposes_trust_score_components_and_candidate_preview`
3. `verify_batch_is_persisted_and_queryable`

---

## 观测结果

### 1. verify_proxy 任务路径
观测到：
- `refresh_provider_risk_snapshot scope=provider provider=smoke elapsed_ms=6`
- `refresh_provider_region_risk_snapshot scope=provider_region provider=smoke region=us-west elapsed_ms=6`
- `refresh_proxy_trust_views_for_scope branch=provider_scope_flip proxy_id=proxy-task-verify provider=smoke`
- `refresh_cached_trust_scores scope=provider provider=smoke elapsed_ms=7`

### 2. open_page 自动代理选择路径
观测到：
- `refresh_provider_risk_snapshot scope=provider provider=pool-x elapsed_ms=6`
- `refresh_provider_region_risk_snapshot scope=provider_region provider=pool-x region=us-east elapsed_ms=4`
- `refresh_proxy_trust_views_for_scope branch=provider_scope_flip proxy_id=proxy-explain-best provider=pool-x`
- `refresh_cached_trust_scores scope=provider provider=pool-x elapsed_ms=5`

### 3. verify_batch 持久化查询路径
观测到：
- `refresh_provider_risk_snapshots scope=all elapsed_ms=13`
- `refresh_cached_trust_scores scope=all elapsed_ms=0`

当前这条测试主要覆盖 batch 持久化与查询，不代表 verify 执行回写链本身。

---

## 当前结论

### 1. 真实任务流里已经出现 `provider_scope_flip`
这很重要。

说明：
- 在真实任务流下，risk snapshot flip 并不是理论分支
- 它已经真实触发 provider 范围 cached trust refresh

### 2. 当前更值得重点盯的是 `provider_scope_flip` 命中率
本轮两条更贴近真实执行的路径里：
- `verify_proxy`
- `open_page` 自动代理选择

都出现了 `provider_scope_flip`。

这说明下一步 profiling 最该回答的问题不是：
- “这个分支会不会发生？”

而是：
- **“它在真实流量里发生得有多频繁？”**

### 3. provider 范围 cached trust refresh 当前单次成本可接受
当前样本里：
- `refresh_cached_trust_scores scope=provider` 大约 **5~7ms**

以当前规模看仍是健康的。

### 4. verify_batch 的当前测试样本还不够代表真实 verify 执行链
这次选的 `verify_batch_is_persisted_and_queryable` 更偏 batch 元数据能力验证，
还没有覆盖“批量 verify 真正执行并持续回写”这类更重路径。

后续如果继续采样 batch verify，应该优先选择：
- 会触发 verify task 执行
- 会触发 proxy 回写
- 会触发 trust refresh
的那类测试路径。

---

## 当前判断

> **第二批更真实任务流样本表明：`provider_scope_flip` 已经在 verify_proxy 与 open_page 自动代理选择路径中真实发生，下一步 profiling 的核心问题应转向“provider 范围 refresh 的命中比例”，而不再只是单次耗时。**

---

## 下一步建议

1. 继续采样会真正触发 verify 批量执行回写的 batch verify 路径
2. 给 `/status` 与 `/proxies/:id/explain` 增加最小读取侧观测
3. 统计 `provider_scope_flip / provider_region_scope_flip / proxy_only_no_flip` 的命中比例

---

## 补充样本：batch verify 真执行回写链

新增采样路径：
- `verify_batch_executes_verify_tasks_and_persists_proxy_results`

观测到：
- `refresh_provider_risk_snapshot scope=provider provider=pool-batch-run elapsed_ms=6`
- `refresh_provider_region_risk_snapshot scope=provider_region provider=pool-batch-run region=us-east elapsed_ms=5`
- `refresh_proxy_trust_views_for_scope branch=provider_scope_flip proxy_id=proxy-batch-run-1 provider=pool-batch-run`
- `refresh_cached_trust_scores scope=provider provider=pool-batch-run elapsed_ms=6`
- `refresh_provider_risk_snapshot scope=provider provider=pool-batch-run elapsed_ms=11`
- `refresh_provider_region_risk_snapshot scope=provider_region provider=pool-batch-run region=us-east elapsed_ms=14`
- `refresh_proxy_trust_views_for_scope branch=provider_region_scope_flip proxy_id=proxy-batch-run-2 provider=pool-batch-run region=us-east`
- `refresh_cached_trust_scores scope=provider_region provider=pool-batch-run region=us-east elapsed_ms=8`

### 补充结论

1. **batch verify 真执行链比单次 verify 更容易触发范围刷新级联。**
   在同一批次内，已经同时观察到：
   - `provider_scope_flip`
   - `provider_region_scope_flip`

2. **provider_region_scope_flip 现在已经不是理论分支。**
   它已在 batch verify 真执行回写链中真实命中。

3. **下一步 profiling 的最关键指标已经更明确：**
   不只是看 provider scope flip，
   还要看：
   - `provider_scope_flip`
   - `provider_region_scope_flip`
   - `proxy_only_no_flip`
   三者在真实任务流里的命中比例。
