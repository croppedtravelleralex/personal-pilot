# Perf Probe 样本记录（2026-04-02 第一批）

## 样本范围

本轮使用 `PP_PERF_PROBE=1` 对以下测试路径采样：

1. `scoped_trust_refresh_helper_limits_cache_refresh_when_risk_flags_do_not_change`
2. `scoped_trust_refresh_helper_updates_provider_group_and_falls_back_for_providerless_proxy`
3. `provider_risk_snapshot_hits_on_exit_ip_not_public_cluster`
4. `provider_region_risk_snapshot_hits_on_region_mismatch_cluster`

---

## 观察到的典型事件

### scoped refresh：无风险翻转
样本中出现：
- `refresh_provider_risk_snapshot scope=provider`
- `refresh_provider_region_risk_snapshot scope=provider_region`
- `refresh_proxy_trust_views_for_scope branch=proxy_only_no_flip`
- `refresh_cached_trust_scores scope=proxy`

### providerless fallback
样本中出现：
- `refresh_proxy_trust_views_for_scope branch=proxy_only_providerless`
- `refresh_cached_trust_scores scope=proxy`

### 全量 snapshot refresh
样本中多次出现：
- `refresh_provider_risk_snapshots scope=all elapsed_ms=9~29`
- `refresh_cached_trust_scores scope=all elapsed_ms=0~4`

---

## 第一批样本结论

### 1. scoped refresh 分支逻辑符合预期
当前已确认至少两个核心分支真实命中：
- `proxy_only_no_flip`
- `proxy_only_providerless`

这说明：
- 当 risk flag 没翻转时，系统确实只刷当前 proxy cached trust
- 当前 scoped refresh 设计在样本里按预期工作，没有无脑扩大刷新范围

### 2. provider / provider×region 局部 snapshot refresh 开销目前可接受
本轮样本里：
- `refresh_provider_risk_snapshot` 大约在 **8~9ms**
- `refresh_provider_region_risk_snapshot` 大约在 **9~10ms**

以当前测试规模看，这个成本是健康的。

### 3. 全量 snapshot refresh 仍是更重的路径
本轮样本里：
- `refresh_provider_risk_snapshots scope=all` 大约在 **9~29ms**
- `refresh_cached_trust_scores scope=all` 大约在 **0~4ms**

说明当前更重的部分主要还是：
- snapshot 聚合刷新
而不是 cached trust SQL 本身。

### 4. 第一批样本还不够说明生产热点
当前样本仍然偏测试环境：
- 数据量小
- 并发低
- 候选规模有限

所以目前能确认的是：
- 机制方向对
- 分支命中对
- 局部路径成本可接受

但还不能据此断言真实流量下的热点比例。

---

## 当前判断

> **第一批 perf probe 样本表明：当前 scoped refresh 设计按预期工作，局部 provider/provider×region refresh 成本可接受；下一步最值得继续观察的是更真实任务流下“全量 snapshot refresh 与局部 refresh 的命中比例”。**

---

## 下一步建议

1. 在更接近真实任务流的路径下继续采样：
   - verify_proxy
   - open_page 自动代理选择
   - batch verify
2. 补 `/status` 与 `/proxies/:id/explain` 的读取侧最小观测
3. 统计 branch 命中比例，而不只看单次耗时
