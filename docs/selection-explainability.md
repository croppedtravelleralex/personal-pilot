# Selection Explainability Design

## 目标

让系统能够回答：

> **为什么这次选中了这个代理，而不是另一个？**

当前 selection 已经进入 trust score 主排序阶段，如果没有 explainability，后续 tuning 会越来越黑盒。

## 当前问题

现在系统虽然能选出代理，但外部很难直接知道：
- trust score 总分是多少
- 正向加分来自哪里
- 负向扣分来自哪里
- 为什么一个 raw score 更高的代理反而输了
- 最终是 trust score 赢的，还是 sticky / 兜底排序在起作用

## explainability 输出目标

建议最终输出：
- `trust_score_total`
- `trust_score_components`
- `positive_signals`
- `negative_signals`
- `tie_breakers`
- `selection_reason_summary`

## 建议输出结构

```json
{
  "proxy_id": "proxy-123",
  "selection_reason_summary": "verify ok + geo match + upstream ok, no recent failure, trust score wins",
  "trust_score_total": 54,
  "trust_score_components": {
    "verify_ok_bonus": 30,
    "verify_geo_match_bonus": 20,
    "smoke_upstream_ok_bonus": 10,
    "missing_verify_penalty": 0,
    "stale_verify_penalty": 0,
    "recent_failure_penalty": 0,
    "provider_risk_penalty": -10,
    "provider_region_cluster_penalty": 0,
    "raw_score_component": 4
  },
  "tie_breakers": {
    "raw_score": 0.4,
    "last_used_at": "1711880000",
    "created_at": "1711800000"
  }
}
```

## 解释层级建议

### Level 1：human summary
给人看的简短说明：
- "verify ok，且地区匹配，近期无失败，因此优先"
- "虽然 raw score 更高，但缺少 verify，最终被 trust score 压下"

### Level 2：structured components
给 API / dashboard / 调参工具看的组件分值：
- verify bonus
- geo bonus
- upstream bonus
- stale penalty
- missing penalty
- provider penalty
- raw score component

### Level 3：decision context
给排障看的附加信息：
- sticky 是否命中
- 候选池大小
- 本次是 direct resolve 还是 sticky resolve
- 被淘汰候选的前几名及其主要原因

## 推荐落地点

### API
建议未来可加：
- task detail 中返回 `selection_explain`
- status 中返回最近一次 selection explain 样本
- debug / admin endpoint 返回候选对比结果

### 数据结构
建议在任务结果或 debug 字段中临时保存：
- `selection_explain_json`
- `candidate_rank_preview`

## 最小可行版本

下一步最小实现不一定要先做复杂接口，可以先：

1. 输出选中代理的 `trust_score_total`
2. 输出主要加分项 / 扣分项
3. 输出一句人类可读 summary
4. 在 debug 场景下附带前 3 个候选简表

## 与 tuning 的关系

explainability 不只是为了“看起来高级”，它直接决定 tuning 是否可维护：
- 没 explain，就很难判断参数改动是否真的生效
- 有 explain，才能知道是 verify 在赢，还是 raw score 在偷赢

## 一句话结论

> **trust score 要继续往主链推进，selection explainability 就必须同步补上，否则后续调参会越来越像黑盒碰运气。**
