# Perf Probe 分支命中统计（2026-04-02）

## 样本来源

统计来源于以下采样日志：
- `/tmp/aob-perf-sample-1.log`
- `/tmp/aob-perf-sample-2.log`
- `/tmp/aob-perf-sample-3.log`
- `/tmp/aob-perf-sample-4.log`
- `/tmp/aob-perf-real-1.log`
- `/tmp/aob-perf-real-2.log`
- `/tmp/aob-perf-real-3.log`
- `/tmp/aob-perf-batch-exec.log`

统计脚本：
- `scripts/summarize_perf_probe.py`

---

## 当前统计结果

### scoped refresh 分支命中数
- `provider_scope_flip`: **3**
- `provider_region_scope_flip`: **1**
- `proxy_only_no_flip`: **2**
- `proxy_only_providerless`: **1**

总计：**7 次** `refresh_proxy_trust_views_for_scope` 分支命中。

### 汇总判断
- **范围刷新分支（provider/provider_region）**：`4/7` ≈ **57.1%**
- **proxy-only 分支（no_flip/providerless）**：`3/7` ≈ **42.9%**

---

## 当前意义

### 1. 范围刷新不是边角情况
在当前样本里，范围刷新分支命中占比已经高于 proxy-only 分支。

这说明：
- `provider_scope_flip`
- `provider_region_scope_flip`

都不是“偶发理论分支”，而是当前架构里需要认真盯住的真实路径。

### 2. `provider_scope_flip` 当前是最值得优先关注的分支
在现有样本中：
- `provider_scope_flip = 3`
- `provider_region_scope_flip = 1`

说明当前范围刷新压力更主要来自 provider 级翻转，而不是 provider×region 级翻转。

### 3. 目前样本仍然偏测试驱动，不代表线上比例
当前结果只能说明：
- 范围刷新已经真实常见
- 在现有样本里占比不低

但还不能直接外推到生产比例。

---

## 当前结论

> **基于 2026-04-02 的第一批 perf probe 样本，`refresh_proxy_trust_views_for_scope` 的范围刷新分支命中占比约为 57.1%，已经高于 proxy-only 分支；其中 `provider_scope_flip` 是当前最主要的范围刷新来源。**

---

## 下一步建议

1. 继续扩大真实任务流样本，验证这个比例是否稳定
2. 优先给 `/status` 与 `/proxies/:id/explain` 增加读取侧观测
3. 若后续样本仍显示 `provider_scope_flip` 占主导，再考虑是否需要进一步收窄 provider 级 refresh 范围
