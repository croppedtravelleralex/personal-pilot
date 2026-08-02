# Selection → Trust Score 差异清单

## 目的

本清单用于明确：当前 proxy selection 主链中，哪些语义已经统一进入 `trust score`，哪些仍留在选择分支 / SQL 过滤 / fallback 逻辑中，后续应如何收口。

---

## 已经统一进 trust score / explainability 的语义

这些部分已经具备较强的 score 表达或 explainability 输出能力：

1. **raw score component**
   - `score * raw_score_weight_tenths`
   - 已进入 `TrustScoreComponents.raw_score_component`

2. **verify / smoke 正向信号**
   - `verify_ok_bonus`
   - `verify_geo_match_bonus`
   - `smoke_upstream_ok_bonus`

3. **verify / freshness / failure 惩罚**
   - `missing_verify_penalty`
   - `stale_verify_penalty`
   - `verify_failed_heavy_penalty`
   - `verify_failed_light_penalty`
   - `verify_failed_base_penalty`

4. **历史成功失败惩罚**
   - `individual_history_penalty`

5. **provider 风险语义**
   - `provider_risk_penalty`
   - `provider_region_cluster_penalty`

6. **候选可解释性输出**
   - `candidate_rank_preview`
   - `winner_vs_runner_up_diff`
   - `selection_reason_summary`
   - `trust_score_total`
   - `trust_score_components`

---

## 仍未完全统一进 trust score 的语义

这些部分仍主要停留在 filter / branch / fallback 层，而不是 score 主语言：

### 1. explicit 选择

代码位置：`resolve_network_policy_for_task()`

当前行为：
- 若指定 `proxy_id`，直接走 `selection_mode = "explicit"`
- 只检查：
  - proxy 存在
  - `status = active`
  - `cooldown_until` 未过期
- 不参与 auto score 排序

问题：
- `explicit` 是强覆盖语义，不进入 trust score 比较
- 当前 explain 只能说“explicit selected active proxy directly”，但无法表达“它虽然被显式选中，但其 trust score 实际偏低/偏高”

建议：
- 保留 `explicit` 作为 **硬覆盖模式**
- 但在 explainability 中增加：
  - `explicit_override: true`
  - `trust_score_total`
  - `would_rank_position_if_auto`

---

### 2. sticky session 复用

当前行为：
- 若命中 `proxy_session_bindings`，直接复用
- 只检查：
  - binding 未过期
  - proxy active
  - cooldown 未过期
  - provider/region/min_score 仍满足

问题：
- sticky 当前是强优先复用，而不是“sticky bonus”进入评分
- 这让 sticky 与 auto 排序是两套语义

建议：
- 短期：继续保留 sticky 为硬复用模式
- 中期：明确 sticky 的边界：
  - **复用阶段是硬规则**
  - **重新选新代理阶段应全部回到 trust score**
- explain 中补：
  - `sticky_reused: true`
  - `sticky_binding_age`
  - `sticky_reuse_reason`

---

### 3. min_score

当前行为：
- 仍在 SQL where/filter 层做硬过滤

问题：
- `min_score` 不是 score 主链，而是前置门槛
- 这会让“略低于门槛但高 trust score”的候选完全消失
- 如果直接把现有 `min_score` 改成软惩罚，会破坏当前 API 语义、测试假设和 no-match 解释口径

正式方案（当前推荐）：
- **保留现有 `min_score` = hard gate**
  - `score < min_score`：直接不进入候选集
- **后续新增 `soft_min_score` = optional soft ranking threshold**
  - `min_score <= score < soft_min_score`：允许进入候选集，但追加 `soft_min_score_penalty`
  - `score >= soft_min_score`：不吃该 penalty

设计原则：
1. **兼容优先**：不破坏现有 `min_score` 语义
2. **分层明确**：hard gate 与 ranking score 不混用
3. **explainability 友好**：后续可在 `trust_score_components` 中加入 `soft_min_score_penalty`
4. **渐进落地**：先文档定口径，再上 API / SQL / score 计算 / 测试

建议落地顺序：
- 第一步：文档中明确 `min_score = hard gate`
- 第二步：设计 `soft_min_score` 请求字段与 `soft_min_score_penalty` 组件
- 第三步：实现 explainability 与测试覆盖

---

### 4. cooldown

当前行为：
- 在 SQL 里硬过滤：`cooldown_until <= now`

问题：
- cooldown 完全不进入 score，而是生死开关
- 这很合理，但需要在体系上被明确定义为 **eligibility gate**，不是 ranking rule

建议：
- 不建议把 cooldown 直接降为普通 penalty
- 应明确写入设计：
  - cooldown 属于 **资格门槛**，不是排序项

---

### 5. no-match fallback / 空结果语义

当前行为：
- 若 explicit/sticky 没命中，回退到 auto
- 若 auto 也没命中，写 `no eligible active proxy matched...`

问题：
- fallback 逻辑目前主要藏在控制流里
- explain 虽然有 summary，但没有结构化表达“为什么没命中”

建议：
- 增加结构化 no-match reason：
  - `no_active_proxy`
  - `all_in_cooldown`
  - `region_filtered_out`
  - `provider_filtered_out`
  - `score_filtered_out`

---

### 6. filter 语义 vs ranking 语义边界未正式文档化

当前代码里已经自然形成了两类规则：

1. **Eligibility gate（资格门槛）**
   - active
   - cooldown
   - provider/region filter
   - min_score

2. **Ranking score（排序分）**
   - trust score components
   - provider/provider×region 风险
   - verify/smoke/history 信号

问题：
- 这条边界在代码里存在，但在正式设计里还没有被明确写死

建议：
- 下一步应先把这条边界明确进设计文档与代码注释
- 否则后续很容易把 gate 和 score 搅混

---

## 当前最值得优先收口的 4 件事

1. **先文档化 gate vs score 边界**
2. **给 explicit / sticky 增加结构化 explain 字段**
3. **给 no-match/fallback 增加结构化 reason code**
4. **按 `min_score = hard gate / soft_min_score = soft ranking` 方案推进实现**

---

## 当前判断

当前系统已经完成了：
- auto 主链 trust score 化
- explainability 主链强类型化

但还未完成：
- selection 全语义统一
- gate 与 score 正式边界固化
- explicit / sticky / no-match 的结构化 explain 完整化

因此，下一阶段最核心的动作不是再加更多零散规则，而是：

> **把 selection 里的剩余控制流语义，尽可能收进结构化 explain 与统一设计边界。**

其中 `min_score` 的正式口径已经明确为：

> **`min_score` 保持 hard gate；后续通过 `soft_min_score` + `soft_min_score_penalty` 扩展 soft ranking。**
