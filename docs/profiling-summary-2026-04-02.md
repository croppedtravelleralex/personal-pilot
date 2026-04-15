# Profiling Summary（2026-04-02）

## 结论先说

截至当前，项目已经完成一轮可行动的 profiling 收口，结论非常明确：

1. **当前主热点更偏写侧范围刷新，而不是读侧接口。**
2. **`provider_scope_flip` 是当前最主要的范围刷新来源。**
3. **`provider_region_scope_flip` 已在 batch verify 真执行回写链中真实发生。**
4. **当前测试规模下，`/status` 与 `/proxies/:id/explain` 读侧成本明显低于 snapshot / trust refresh 写侧路径。**

---

## 已完成的 profiling 基础设施

### 已有最小观测埋点
通过 `PP_PERF_PROBE=1` 已覆盖：

#### 写侧
- `refresh_provider_risk_snapshots`
- `refresh_provider_risk_snapshot`
- `refresh_provider_region_risk_snapshot`
- `refresh_cached_trust_scores`（all / proxy / provider / provider_region）
- `refresh_proxy_trust_views_for_scope` 分支命中

#### 读侧
- `api_status`
- `api_proxy_explain`
- `selection_decision_summary_artifact`

---

## 写侧样本结论

### 首批 scoped refresh 分支统计
基于当前样本：
- `provider_scope_flip`: **3**
- `provider_region_scope_flip`: **1**
- `proxy_only_no_flip`: **2**
- `proxy_only_providerless`: **1**

总计：**7 次** `refresh_proxy_trust_views_for_scope` 分支命中。

### 当前分布
- **范围刷新分支（provider / provider_region）**：`4/7` ≈ **57.1%**
- **proxy-only 分支**：`3/7` ≈ **42.9%**

### 当前判断
- 范围刷新已经不是边缘情况
- `provider_scope_flip` 是当前主导项
- 后续优化若继续推进，应优先盯 provider 级 refresh 范围

---

## 真实任务流样本结论

### verify_proxy / open_page 自动代理选择
已经观察到：
- `provider_scope_flip`
- provider 范围 cached trust refresh

### batch verify 真执行回写链
已经观察到：
- `provider_scope_flip`
- `provider_region_scope_flip`

### 当前判断
- batch verify 真执行链更容易触发范围刷新级联
- `provider_region_scope_flip` 不再是理论分支，而是实际路径

---

## 读侧样本结论

### `/status`
样本：
- `api_status elapsed_ms=1 latest_task_count=0 latest_summary_count=0`

### `/proxies/:id/explain`
样本：
- `api_proxy_explain proxy_id=proxy-explain-endpoint elapsed_ms=3 candidate_count=1`

### 当前判断
在当前测试规模下：
- `/status`：**约 1ms**
- `/proxies/:id/explain`：**约 3ms**

读侧接口当前明显轻于：
- provider/provider×region snapshot refresh
- provider 范围 cached trust refresh

---

## 总体判断

> **当前 profiling 已足够支持下一步优化排序：优先关注写侧范围刷新（尤其是 `provider_scope_flip`），而不是优先优化 `/status` 或 `/proxies/:id/explain` 读侧。**

---

## 当前最值得继续做的事

1. 继续扩大真实任务流样本，验证 `57.1%` 的分布是否稳定
2. 观察更高候选规模下 `/proxies/:id/explain` 的增长曲线
3. 若 provider 级范围刷新持续主导，再考虑是否继续收窄 provider 级 refresh 范围


## 读侧补充样本：更高候选规模 explain

新增采样路径：
- `proxy_explain_endpoint_with_higher_candidate_count_still_returns_preview`

样本：
- `api_proxy_explain proxy_id=proxy-explain-bulk-0 elapsed_ms=6 candidate_count=3`

### 补充判断
- 当前候选规模从 1 提升到 3 时，`/proxies/:id/explain` 仍处于低毫秒级
- 在当前测试规模下，读侧 explain 仍明显轻于写侧范围刷新
- 现阶段还没有足够证据支持“优先优化 explain 读侧”


## 补充样本（第二批）

新增采样结果：
- `verify_proxy_task_kind_executes_and_persists_result`
  - `provider_scope_flip`
  - `refresh_cached_trust_scores scope=provider elapsed_ms=6`
- `status_latest_execution_summaries_include_selection_decision_artifact`
  - `provider_scope_flip`
  - `api_status elapsed_ms=4 latest_task_count=1 latest_summary_count=2`
- `proxy_explain_endpoint_with_higher_candidate_count_still_returns_preview`
  - `api_proxy_explain elapsed_ms=8 candidate_count=3`
- `verify_batch_executes_verify_tasks_and_persists_proxy_results`
  - `provider_scope_flip`
  - `provider_region_scope_flip`

### 补充判断
- `provider_scope_flip` 在追加样本里继续命中，主导地位没有反转
- `provider_region_scope_flip` 仍主要出现在 batch verify 真执行回写链
- `/status` 与 `/proxies/:id/explain` 即使补到更贴近真实的样本，仍明显轻于写侧范围刷新
- 当前没有足够证据支持优先优化读侧；下一步更值得进入 provider 级 refresh 范围收窄方案设计


## V1 收益验证补样（provider risk version / seen）

样本汇总：
- `provider_scope_flip` 命中：**3 次**，且全部表现为 `lazy_current_proxy`
- `provider_region_scope_flip` 命中：**1 次**
- proxy 级 refresh 耗时样本：**[5, 4, 5] ms**
- provider 级 refresh 耗时样本：**[]**
- provider_region 级 refresh 耗时样本：**[5] ms**
- `/status` 读侧耗时样本：**[4] ms**

### 当前判断
- `provider_scope_flip` 这一层已经稳定切换到 **当前 proxy 懒更新**，不再看到 provider 级 cached trust refresh 命中
- `provider_region_scope_flip` 仍保留 provider_region 范围刷新语义，因此当前第二阶段不宜贸然一起改
- 在现有样本下，v1 已经足以支持一个保守结论：**先继续延后 providerRegion，优先巩固 providerScope 这一层的收益判断**
