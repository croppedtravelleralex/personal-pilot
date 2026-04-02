# Provider Risk / Provider×Region Risk 吸收 Verify 信号方案

## 目标

当前单代理 trust score 已经吸收了多类 verify 慢路径信号：
- anonymity
- probe latency
- exit_ip_not_public
- probe_error_category
- geo mismatch severity
- region mismatch severity

但 provider / provider×region 风险快照仍停留在较早期口径：
- provider: success_count / failure_count 汇总
- provider×region: recent failed cluster 汇总

这会导致：
- 单代理排序层进化更快
- 聚合风险层仍只看旧失败信号
- 后续 provider/provider×region 风险惩罚可能落后于真实代理质量变化

---

## 当前判断

**应该吸收，但不能一口气把所有 verify 信号直接塞进 snapshot。**

原因：
- provider/provider×region risk snapshot 是聚合层，不应简单重复单代理 score 组件
- 如果把所有 verify 风险原因都直接累加到 snapshot，会放大重复计分风险
- 更合理的做法是：
  - 只吸收**最稳定、最跨代理可复现、最具群体模式意义**的 verify 风险信号

---

## 适合进入聚合风险层的 verify 信号

### P0：优先吸收（最值）

1. **`exit_ip_not_public` 出现率**
   - 这是很强的 provider 质量信号
   - 若同 provider 下多代理都出现非公网出口，极可能代表上游质量或出口层问题
   - 适合进入 provider risk snapshot

2. **`geo_mismatch` / `region_mismatch` 的群体出现率**
   - 单代理层已经有 penalty
   - 但若某 provider/provider×region 组合反复出现错配，则更适合作为聚合风险层信号
   - `geo_mismatch` 更适合 provider 级
   - `region_mismatch` 更适合 provider×region 级

3. **`probe_error_category` 中可稳定反映上游质量的类别**
   - `protocol_invalid`
   - `upstream_missing`
   - `connect_failed`
   这些适合做 provider/provider×region 级计数与阈值判断

### P1：保留在单代理层为主

4. **anonymity / transparency**
   - 目前更适合在单代理层消费
   - 是否进入 provider 聚合层要谨慎，因为不同 provider 可能混合多种代理质量等级

5. **probe latency**
   - 当前更适合单代理层排序
   - 若未来要进 provider 聚合层，也应做 percentile / window 聚合，而不是简单均值

---

## 推荐的聚合原则

### 原则 1：聚合层只吸收“群体现象”

只有当某个 verify 风险原因在 provider 或 provider×region 维度达到一定计数 / 比例阈值时，才应转化为聚合风险命中。

不要因为单个代理一次 verify 异常，就把整个 provider 立即打成高风险。

### 原则 2：provider 与 provider×region 分层处理

- **provider risk snapshot** 更适合吸收：
  - exit_ip_not_public
  - geo_mismatch
  - protocol_invalid / upstream_missing / connect_failed 的 provider 级频发

- **provider×region risk snapshot** 更适合吸收：
  - region_mismatch
  - connect_failed cluster
  - provider×region 维度的 verify failed cluster

### 原则 3：只增加有限数量的新 hit 条件

当前不要把 snapshot 做成完整风险打分器。

推荐先只补：
1. provider: `exit_ip_not_public` 群体命中
2. provider×region: `region_mismatch` 群体命中

这样最稳、最不容易和现有 score 主链重复。

---

## 最小落地方案（当前推荐）

### 第一步
先对 snapshot 规则做两项最小增强：

#### Provider risk snapshot
若同 provider 下满足以下任一条件，则 `risk_hit = 1`：
- 原有 success/failure 规则命中
- **最近 verify 结果中 `last_probe_error_category = exit_ip_not_public` 的代理数达到阈值**
- **最近 verify 结果中 `last_exit_country != country` 的代理数达到阈值**

#### Provider×Region risk snapshot
若同 provider + region 下满足以下任一条件，则 `risk_hit = 1`：
- 原有 recent failed cluster 规则命中
- **最近 verify 结果中 `last_exit_region != region` 的代理数达到阈值**

### 第二步
再考虑是否补：
- provider/provider×region 维度的 `probe_error_category` 聚类
- provider 级 geo mismatch ratio

### 当前保守决定
- **`geo_mismatch` 暂不直接进入 provider 级 snapshot。**
- 原因：provider 级粒度过粗，容易把局部国家错配放大成整个 provider 风险。
- 当前先保留：
  - 单代理层 penalty
  - provider×region 层的 `region_mismatch` 群体命中
- 只有在后续 profiling / 真实数据证明 provider 级 `geo_mismatch` 呈现稳定群体模式时，才考虑上升到 provider 聚合层。

---

## 当前结论

> **provider/provider×region 风险汇总应该开始吸收 verify 慢路径信号，但第一步只建议补“群体性的 exit_ip_not_public 与 region/geo mismatch 命中”，不要把所有 verify 风险原因一口气搬进 snapshot。**
