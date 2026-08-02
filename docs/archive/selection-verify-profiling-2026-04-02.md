# Selection / Trust Cache / Verify 回写 Profiling 记录（2026-04-02）

## 结论先说

当前最值得关注的性能风险不是单条 SQL 慢，而是：

1. **risk snapshot flip 触发的范围型 cached trust refresh**
2. **cached trust score 仍依赖大型内联 SQL 表达式**
3. **selection explain / status 聚合读取复杂度持续增加**

当前判断：
- 系统还没到必须重构的程度
- 但 profiling 已经不是“以后再说”，而是当前主线的必要前置动作

---

## 观察范围

本轮主要查看：
- `refresh_proxy_trust_views_for_scope()`
- `refresh_provider_risk_snapshots()` / `refresh_provider_region_risk_snapshot_for_pair()`
- `cached_trust_score_update_sql()`
- `selection_decision_summary_artifact()` / explainability 聚合路径

---

## 观察 1：scoped refresh 策略是对的，但 flip 成本会继续上升

当前刷新策略：
- provider risk flag 不变 → 仅刷新当前 proxy cached trust
- provider risk flip → 刷新整 provider cached trust
- provider×region risk flip → 刷新整 provider+region cached trust

### 当前判断
这是正确方向，已经明显优于“每次都全量刷新”。

### 当前风险
随着 provider/provider×region snapshot 开始吸收更多 verify 慢路径信号：
- risk_hit flip 频率可能上升
- 范围刷新次数可能增加
- 写放大会逐步从“可接受”转向“需要量化”

### 当前建议
下一步 profiling 应优先记录：
- 单 proxy 刷新比例
- provider 级范围刷新比例
- provider×region 级范围刷新比例

---

## 观察 2：cached trust score 仍是大 SQL 表达式驱动

`cached_trust_score_update_sql()` 当前仍通过一个较大的 SQL CASE/EXISTS 表达式完成：
- verify status / freshness
- history risk
- provider risk snapshot
- provider×region risk snapshot
- base score

### 当前判断
优点：
- 现在实现集中，行为一致
- 还没有明显失控

风险：
- 后续如果继续往 cached trust SQL 里叠太多新 penalty
- SQL 可读性、维护成本、调试复杂度都会上升

### 当前建议
下一阶段不要继续无节制往 cached trust SQL 里堆逻辑；
优先做：
- explain-level profiling
- scope refresh 命中率统计
- 必要时再决定是否拆分 cached trust 计算层

---

## 观察 3：explainability 已进入“质量收益高，但读取开销增长”的阶段

selection explain 当前已经输出：
- trust score components
- candidate rank preview
- winner vs runner-up diff
- selection decision summary artifact
- structured selection_explain

### 当前判断
这条链的产品价值很高，不能轻易砍。

### 当前风险
随着更多 verify 风险组件并入：
- factor labels 增多
- diff 计算和 summary 解释复杂度上升
- status/latest summary 聚合链路负担会继续增加

### 当前建议
下一步 profiling 应记录：
- `/status` 最新 summary 聚合成本
- `/proxies/:id/explain` 响应成本
- candidate preview 在候选数增加时的增长情况

---

## 当前决策

### 优先级 1
继续保留当前 scoped refresh 设计，不做激进重构。

### 优先级 2
把 profiling 结论写进项目文档并作为当前 P0 持续跟踪项。

### 优先级 3
在没有 profiling 数据前，不建议继续大幅扩张 cached trust SQL 表达式。

---

## 当前一句话总结

> **当前最大的性能风险不是某一条 SQL 明显太慢，而是 selection / snapshot / trust refresh / explain 聚合叠加后的范围刷新与写放大成本。**
