

## Workflow Action Dispatch

- 执行建议第 1 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：默认推进当前最优先事项
- 执行建议第 2 项 [feature]: 已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：保持双任务推进节奏

## 2026-04-01 narrower scoped trust refresh pass

- 继续收窄 `refresh_proxy_trust_views_for_scope()`：不再默认 provider-scope 刷整组 cache，而是先比较 provider risk / provider-region risk 的前后布尔状态。
- 新策略：
  - provider 缺失 → 只刷当前 proxy cache
  - provider risk 标志发生变化 → 刷整 provider cache
  - 仅 provider-region risk 标志发生变化 → 只刷 provider+region cache
  - 两类 risk 标志都未变化 → 只刷当前 proxy cache
- 新增单测 `scoped_trust_refresh_helper_limits_cache_refresh_when_risk_flags_do_not_change`，锁定“risk 标志未翻转时不应牵连同 provider 其他代理 cache 时间戳”的行为。
- 验证结果：关键测试通过，随后全量 `cargo test` 全绿（41 unit + 75 integration）。

