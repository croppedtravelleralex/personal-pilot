# Verify Risk → Trust Score 收口方案

## 目标

把 verify 慢路径已经产出的 `risk_level / verification_class / recommended_action / risk_reasons` 从“主要给人看”的诊断语义，继续收口为：

- 哪些应该进入 trust score
- 哪些不应该直接进入 trust score
- 哪些应该先进入 provider/provider×region 风险汇总，而不是直接影响单代理排序

---

## 当前已存在但尚未正式并入 trust score 的信号

verify 慢路径当前已经产出：

- `risk_level`
- `risk_reasons`
- `failure_stage`
- `failure_stage_detail`
- `verification_class`
- `recommended_action`

这些信号当前主要用于：
- verify API 结果输出
- 人工判断
- explain / 运维层消费

还没有系统进入：
- proxy trust score 主表达
- provider risk snapshot 汇总策略
- selection explain 主语言

---

## 设计原则

### 1. 不要把“结果标签”直接当成 score 组件

以下字段属于**分类标签 / 决策摘要**，不建议直接作为 trust score 组件：

- `verification_class`
- `recommended_action`

原因：
- 它们是上层汇总结果，不是底层原始信号
- 如果直接拿来打分，容易和底层信号重复计分
- 会让 score 解释链变得循环依赖（因为 classification 本身就是由底层信号推导出来的）

结论：
- **`verification_class` / `recommended_action` 不直接进 trust score**
- 但可以进入 explain summary / status artifact / provider risk 聚合策略

---

### 2. `risk_level` 不能裸进 score，只能作为受控映射

`risk_level` 是多信号汇总结果，比 `verification_class` 更接近中间层，但依然不是底层原始测量。

推荐：
- 不直接把 `risk_level` 当 score 字段长期保存并参与排序
- 如要进入 score，只能走**受控映射**：
  - `low` → 0
  - `medium` → 轻 penalty
  - `high` → 重 penalty

但要注意：
- 不能和 anonymity / geo mismatch / exit ip / probe error category 这些原始信号重复计分

结论：
- **优先让底层原因进 score，而不是让 `risk_level` 本身进 score**
- `risk_level` 更适合作为 explain / summary / routing 判断字段

---

### 3. 最值得进 trust score 的，是 `risk_reasons` 里少数稳定、非重复的底层原因

当前最适合并入 score 的，不是整个 `risk_reasons`，而是其中稳定、语义清楚、且尚未被现有组件覆盖的一小部分。

### 推荐优先级

#### P0：适合直接进入单代理 trust score
1. **`transparent_proxy`**
   - 已部分通过 anonymity 进入 score
   - 但应明确成为 explain / score 统一语义的一部分
2. **`anonymous_proxy`**
   - 已部分进入 anonymity 逻辑
3. **`exit_ip_not_public`**
   - 很值，强风险信号
   - 当前值得补成明确 penalty
4. **`probe_error_category` latency/transport quality 派生项**
   - 例如协议失败 / 上游身份缺失 / 连接异常
   - 适合做轻重分层 penalty

#### P1：更适合先进入 provider/provider×region 风险聚合
5. **重复出现的 `geo_mismatch` / `region_mismatch`**
   - 单代理一次 mismatch 可以进单体信号
   - 但更有价值的是做 provider/provider×region 聚合风险
6. **重复出现的 verify failed class**
   - 更适合汇总成 provider/provider×region 风险，而不只是单代理瞬时扣分

#### P2：更适合作为 explain / action，而不是 score
7. `verification_class`
8. `recommended_action`

---

## 当前推荐推进顺序

### 第一步（最小闭环）
先不碰 `verification_class` / `recommended_action`。

先补这两个最值原始信号：
1. **`exit_ip_not_public` penalty**
2. **`probe_error_category` penalty 映射**

原因：
- 它们是 verify 慢路径里非常底层、非常稳定的真实质量信号
- 进入 score 后不容易和现有信号循环依赖
- 价值比继续堆 summary 字段更高
- 这一步优先做成“能解释、能回归、能调参”的最小闭环

### 第二步
再评估是否需要把：
- `geo mismatch severity`
- `region mismatch severity`

做成更明确的 score 组件，而不是仅依靠 `verify_geo_match_bonus` 的正向语义。

### 第三步
最后才考虑：
- provider/provider×region 汇总如何消费 `risk_reasons`
- `recommended_action` 是否参与调度层动作，而非 score

---

## 当前结论

> **下一步最值得继续进入 trust score 的，不是 `verification_class` 或 `recommended_action`，而是 `exit_ip_not_public` 与 `probe_error_category` 这类底层 verify 风险原因。**
