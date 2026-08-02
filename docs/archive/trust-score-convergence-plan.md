# Trust Score Convergence Plan

## 目标

把当前 proxy selection 从“trust score 已接主排序，但仍保留较多分散语义和兜底排序”的状态，继续推进到：

> **大部分真实选择语义都由统一 trust score / risk score 表达承担。**

## 当前状态

当前排序口径已经是：

```sql
(trust_score) DESC,
COALESCE(last_used_at, '0') ASC,
created_at ASC
```

这说明：
- **trust score 已接主链**
- 原始分数已不再作为单独的二次兜底
- 资源均衡仍在尾部用 `last_used_at / created_at` 承接

## 当前已纳入 trust score 的信号

### 正向信号
- `last_verify_status = ok`
- `last_verify_geo_match_ok = true`
- `last_smoke_upstream_ok = true`
- `raw score`

### 负向信号
- recent verify failed（重 / 轻窗口）
- base verify failed penalty
- missing verify
- stale verify
- 代理个体长期失败偏多
- provider 长期失败偏多
- provider × region 近期失败聚簇

## 仍未完全收口的地方

### 1. raw score 已不再作为排序兜底
当前 `raw score` 已经进入 trust score 主表达，且主排序中的原始分数二次兜底已经移除。

这意味着：
- trust score 接管语义更彻底了
- 外部理解排序逻辑时更容易看出 trust / 资源均衡的职责边界

### 2. 资源均衡仍是纯排序尾部逻辑
当前 `last_used_at ASC`、`created_at ASC` 更像调度层资源均衡，而不是明确的策略分值。

这会导致：
- explainability 不好做
- 很难回答“为什么这次它排在前面”

### 3. verify 信号还偏离散
当前 `verify ok / geo match / upstream ok / missing / stale / failed` 已入 trust score，但匿名性等级、重复验证稳定性、probe latency、probe error 还没纳入。

### 4. 执行后真实结果与 trust score 仍然耦合偏浅
当前 execution 结束后主要回写 success / failure / cooldown / last_used 等信息，但“真实执行成功率对 trust score 的长期反馈”还可以更系统化。

## 推荐收口顺序

### P0：先把排序主语义彻底收紧
1. 明确原始分数二次兜底是不是还需要保留
2. 若保留，文档明确其职责仅是 **trust score 相同情况下的次级平局裁决**
3. 若不保留，改为把 raw score 完全内化到 trust score 中，只保留 `last_used_at / created_at` 作为资源均衡尾部

### P1：把 verify 质量信号继续并进 score
优先考虑新增：
- `anonymity_level`
- repeated verify stability
- probe latency bucket
- probe error category
- exit country / region mismatch severity

### P2：把资源均衡从“尾部排序”变成“可解释策略信号”
可以增加：
- reuse penalty / freshness bonus
- sticky reuse confidence
- newly added proxy warm-up bonus / penalty

### P3：把 execution feedback 更正式并入长期信号
建议补：
- execution success rate window
- timeout-heavy penalty
- target/site dimension feedback（后续）

## 建议新增的 trust score 信号

### verify 维度
- `elite / anonymous / transparent` 匿名性等级
- `probe_latency_ms` 延迟分层
- `probe_error_category`
- repeated probe pass streak
- geo mismatch severity（国家错 / 地区错）

### execution 维度
- recent execution success streak
- recent timeout cluster
- recent cancel / abnormal termination signal（谨慎使用）

### balancing 维度
- recently overused penalty
- sticky binding freshness bonus
- provider diversity balancing（避免单 provider 吃满）

## explainability 需要同步输出的字段

如果继续推进 trust score 核心化，建议 future status / selection explain 接口输出：
- `trust_score_total`
- `trust_score_components`
- `positive_signals`
- `negative_signals`
- `tie_breakers`

## 最小落地建议

下一轮最小可行推进建议是：

1. 保留现有 trust score SQL 主体
2. 新增 `anonymity_level` 与 `probe latency` 两类信号设计
3. 明确原始分数二次兜底的职责是“平局兜底”还是“待移除残留”
4. 补一份 explainability 输出结构，避免 trust score 继续黑盒化

## 一句话结论

当前最值得做的不是继续堆新规则，而是：

> **把已经存在的选择信号继续往 trust score 主表达里收口，并同步补 explainability。**
