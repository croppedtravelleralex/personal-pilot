# Provider 级 Refresh 范围收窄方案（草案）

## 当前背景

profiling 样本已经显示：
- 范围刷新分支占比不低
- `provider_scope_flip` 是当前主导项
- `provider_region_scope_flip` 已真实存在，但暂时不是主导项

因此，若后续要继续优化写侧，最值得优先评估的不是读侧接口，而是：

> **provider 级 cached trust refresh 的范围是否可以继续收窄。**

---

## 当前问题

当前逻辑一旦 `provider_scope_flip` 发生，就会：
- 刷新整个 provider 下所有 proxy 的 cached trust score

优点：
- 逻辑简单
- 正确性强

代价：
- 当 provider 规模变大时，单次 flip 会放大写侧刷新范围
- 如果 provider risk hit 由少量代理触发，整 provider 刷新可能过粗

---

## 当前保守判断

**现在可以进入方案设计，但不建议立刻改实现。**

原因：
- 当前样本仍偏测试环境
- 还没有 provider 规模更大的真实数据
- 需要先明确“收窄后是否会带来正确性回归风险”

---

## 推荐的收窄方向

### 方向 A：先按 provider + 最近受影响 proxy 集合收窄
当 provider risk flip 发生时：
- 优先刷新当前 proxy
- 再刷新最近一小批同 provider 代理
- 不立刻全 provider 刷新

### 方向 B：引入 provider risk version / dirty 标记
当 provider risk flip 发生时：
- 不同步刷新整 provider cached trust
- 只更新 provider risk snapshot/version
- 让 proxy 在后续被选择/解释/verify 时懒更新 cached trust

### 当前更推荐
**先研究方向 B。**

原因：
- 它更像结构性解法
- 比“缩小一部分 provider refresh 范围”更稳定
- 更适合和未来的 risk snapshot / cache version 体系整合

---

## 当前结论

> **如果后续样本继续显示 `provider_scope_flip` 主导，下一步最值得推进的是“provider risk version / dirty 标记 + 懒刷新”方向，而不是简单地把 provider refresh 改成一个拍脑袋的小范围批量刷新。**

## 阶段性决定（2026-04-02）

- **providerScope：继续验证并巩固收益判断**
- **providerRegion：继续延后，不进入下一阶段实现**
