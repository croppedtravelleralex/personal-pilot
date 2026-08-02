# Provider Risk Version / Dirty 标记最小实现方案（2026-04-02）

## 目标

在不一次性重构整个 trust cache 体系的前提下，先为 **provider 级 risk flip** 引入更可收敛的缓存失效语义，减少“provider_scope_flip → 整 provider 立即刷新 cached trust”这条路径的写放大。

---

## 当前结论

**第一步只建议落 provider risk，不建议 provider/provider_region 一起上。**

原因：
- profiling 样本显示 **`provider_scope_flip` 是当前主导项**
- `provider_region_scope_flip` 已真实存在，但暂时不是主导项
- 若两层一起改，数据结构、迁移、测试与回归面会明显扩大

因此当前最小返工路径是：

> **先只给 provider risk snapshot 增加 version / dirty 语义。**

---

## 当前实现痛点

现在一旦 `provider_scope_flip` 命中：
- 立即执行 `refresh_cached_trust_scores_for_provider(provider)`
- 导致整个 provider 下 proxy 全量重算 cached trust score

优点：
- 简单
- 正确性直接

缺点：
- provider 越大，单次写放大越明显
- risk flip 频繁时，provider 级范围刷新会持续成为热点

---

## 最小实现思路

### 新增字段（推荐）

#### `provider_risk_snapshots`
- `version INTEGER NOT NULL DEFAULT 1`

#### `proxies`
- `provider_risk_version_seen INTEGER`

---

## 新语义

### provider risk snapshot
每次 provider risk 快照发生 `risk_hit` 变化时：
- 不立刻刷新整 provider 的 cached trust score
- 而是：
  - 更新 `provider_risk_snapshots.risk_hit`
  - **递增 `provider_risk_snapshots.version`**

### proxy cached trust
每个 proxy 记录自己上次消费的是哪个 provider risk version：
- `provider_risk_version_seen`

当以下任一条件满足时，proxy 视为 provider risk 维度已过期：
- `provider_risk_version_seen IS NULL`
- `provider_risk_version_seen != provider_risk_snapshots.version`

---

## 刷新策略变化

### 旧策略
`provider_scope_flip` → 立刻刷新整 provider

### 新策略（第一阶段）
`provider_scope_flip` → **只更新 provider risk snapshot.version，不立刻刷新整 provider cached trust**

然后在这些读取/写入点懒更新当前 proxy：
- verify 回写链命中的当前 proxy
- explain proxy 当前查询的 proxy
- auto selection 候选被扫描时（后续可评估）

---

## 第一阶段范围控制

### 本阶段建议只做
1. provider risk snapshot 增加 `version`
2. proxies 增加 `provider_risk_version_seen`
3. provider risk flip 时不再整 provider refresh
4. 当前 proxy refresh 时顺手同步 `provider_risk_version_seen`

### 本阶段不做
- provider_region risk version
- provider_region dirty 标记
- selection 候选批量懒刷新
- explain/status 全链路自动补齐 version 对齐

---

## 为什么现在先不带 provider_region

1. 当前主导热点仍是 `provider_scope_flip`
2. provider_region 级链路已存在，但样本占比还不足以证明它应优先于 provider 级问题
3. 先分层落地，能减少 schema / 刷新策略 / 回归测试的复杂度

---

## 风险与回归点

### 风险 1
如果只做 provider risk version，而读取侧没有及时懒更新，可能出现：
- `cached_trust_score` 与当前 provider risk snapshot 短时间不一致

### 风险 2
如果 selection 仍大量直接消费旧 cached trust，而不校验 version，就可能降低即时一致性。

---

## 当前判断

这套方案的关键，不是“完全不允许短时旧值”，而是：
- **明确哪些路径允许懒更新**
- **明确哪些路径仍需要即时一致性**

在当前阶段，这比继续整 provider 全量刷新更有工程价值。

---

## 下一步最小落地建议

1. 先补 schema 字段：
   - `provider_risk_snapshots.version`
   - `proxies.provider_risk_version_seen`
2. 把 provider risk snapshot scoped refresh 改成：
   - 仅更新 snapshot + version
   - 不再自动 `refresh_cached_trust_scores_for_provider`
3. 给当前 proxy refresh 链加 version 对齐
4. 用测试验证：
   - provider risk flip 后，非当前 proxy 不会被立刻刷新
   - 当前 proxy refresh 后 version_seen 被更新

---

## 当前一句话结论

> **最小返工路径是：先只落 provider risk version / dirty 语义，不带 provider_region；先把“provider_scope_flip → 整 provider 立即刷新”收敛成“provider snapshot 变更 + 当前 proxy 懒更新”。**
