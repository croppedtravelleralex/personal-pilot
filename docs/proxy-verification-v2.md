# Proxy Verification V2 Design

## 目标

把当前已经存在的 `smoke + verify + verify_batch` 能力，从“轻量探测链”推进到“更真实的慢路径验证链”。

V2 想回答的问题是：
- 这个代理是否真的能连到更真实的外部探测目标？
- 它暴露出来的出口 IP / 国家 / 地区是什么？
- 它的匿名性等级到底怎么样？
- 它的延迟、报错、稳定性是否足够进入更高优先级选择？

## 当前 V1 已具备的东西

- `POST /proxies/:id/smoke`
- `POST /proxies/:id/verify`
- `POST /proxies/verify-batch`
- `exit_ip / exit_country / exit_region`
- `geo_match_ok`
- `anonymity_level`
- `last_verify_status / last_verify_at`

## V1 的核心不足

1. **verify 仍偏轻量。**
   当前更多是在本地 / 简化 HTTP CONNECT 探测路径上判断，不等于真实外部探针链。

2. **缺少更丰富的 verdict。**
   现在能拿到 ip / geo / anonymity，但还缺少 latency、error category、stability 等更细质量信号。

3. **与 trust score 的耦合还不够深。**
   当前 verify 能回写状态，但“验证结果如何形成持续评分变化”还不够系统。

## V2 设计目标

### Stage A：保留 fast path
继续保留：
- `smoke` 作为快速基础探测
- `verify` 作为较慢验证入口
- `verify_batch` 作为批量任务投递入口

### Stage B：引入 external probe
新增可配置外部探针：
- `AUTO_OPEN_BROWSER_PROXY_PROBE_URL`
- `AUTO_OPEN_BROWSER_PROXY_PROBE_TIMEOUT_MS`
- `AUTO_OPEN_BROWSER_PROXY_PROBE_EXPECT_COUNTRY`
- `AUTO_OPEN_BROWSER_PROXY_PROBE_EXPECT_REGION`（可选）

外部 probe 返回建议 JSON：

```json
{
  "ip": "203.0.113.8",
  "country": "US",
  "region": "Virginia",
  "via": "...",
  "forwarded": "...",
  "x_forwarded_for": "...",
  "user_agent": "...",
  "latency_ms": 482,
  "error": null
}
```

### Stage C：形成 richer verdict
建议 verify V2 输出并持久化：
- `probe_ok`
- `probe_latency_ms`
- `probe_error`
- `probe_error_category`
- `exit_ip`
- `exit_country`
- `exit_region`
- `geo_match_ok`
- `anonymity_level`
- `verification_score_delta`
- `verification_confidence`

## 建议持久化扩展

建议在 `proxies` 中补：
- `last_probe_latency_ms`
- `last_probe_error`
- `last_probe_error_category`
- `last_verify_confidence`
- `last_verify_score_delta`
- `last_verify_source`（smoke / local_verify / external_probe）

## 建议 verdict 规则 V1

### 正向
- `probe_ok = true`
- `geo_match_ok = true`
- `anonymity_level = elite`
- latency 落在健康区间
- repeated successful verify

### 负向
- `probe_ok = false`
- upstream 不通
- `transparent`
- 国家不匹配
- 地区不匹配
- latency 过高
- repeated probe failure

## 与 trust score 的耦合建议

建议把以下信号继续并入 trust score：
- `anonymity_level`
- `probe_latency_ms` bucket
- `probe_error_category`
- `exit_ip_not_public`
- repeated verify pass/fail streak
- `verification_score_delta`

说明：
- `probe_error_category` 已经在主链里有 penalty 映射
- `exit_ip_not_public` 适合继续作为高优先级风险原因并入 trust score
- `verification_score_delta` 更适合作为解释与回写辅助，而不是单独主导排序

## verify_batch 的 V2 方向

当前 `verify_batch` 更像任务扇出器。

V2 建议它继续承担：
- 按 stale / failed-only / provider-cap 等规则筛选对象
- 统一投递慢路径 verify 任务
- 汇总 richer verdict distribution
- 暴露 provider / region / error bucket 聚合

## 推荐实现顺序

1. 增加 external probe env 与 DTO
2. 增加 latency / probe_error / source 持久化字段
3. 把 richer verdict 接到 `verify` 返回结构
4. 把 verify delta 接进 trust score / quality score
5. 让 `verify_batch` 汇总 richer verdict 分布

## 一句话结论

> **V2 的重点不是再加一个接口，而是让 verify 从“能探一眼”升级为“能给 selection 提供更可信慢路径质量信号”。**
