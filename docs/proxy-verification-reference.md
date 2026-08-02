# Proxy Verification / Selection / Batch Verify Reference

## Verification paths

### Smoke
Route: `POST /proxies/:id/smoke`

Purpose:
- fast, cheap liveness and protocol sanity check
- suitable for immediate preflight or quick health signal refresh

Signals:
- `reachable`
- `protocol_ok`
- `upstream_ok`
- `exit_ip`
- `anonymity_level`

Writes back:
- `last_smoke_status`
- `last_smoke_protocol_ok`
- `last_smoke_upstream_ok`
- `last_exit_ip`
- `last_anonymity_level`
- `last_smoke_at`

### Verify
Route: `POST /proxies/:id/verify`

Purpose:
- slower path with geo validation signals
- suitable for promotion / ranking / confidence refresh

Signals:
- `reachable`
- `protocol_ok`
- `upstream_ok`
- `exit_ip`
- `exit_country`
- `exit_region`
- `geo_match_ok`
- `anonymity_level`

Writes back:
- `last_verify_status`
- `last_verify_geo_match_ok`
- `last_exit_ip`
- `last_exit_country`
- `last_exit_region`
- `last_anonymity_level`
- `last_verify_at`

## Selection priority
Current proxy selection is no longer score-only.

Current priority order:
1. `last_verify_status = ok`
2. `last_verify_geo_match_ok = true`
3. `last_smoke_upstream_ok = true`
4. `last_used_at ASC`
5. `created_at ASC`

Meaning:
- verified and geo-matching proxies are preferred over merely high-score proxies
- smoke upstream success is a useful secondary hint
- trust score already absorbs raw score; recency now only acts as the tail balancing signal

## Sticky session behavior
If `sticky_session` is provided:
- try `proxy_session_bindings` first
- validate active / expiry / cooldown / provider / region / score constraints
- if still valid, reuse sticky proxy
- after execution, upsert binding again

## Batch verify direction
Planned route:
- `POST /proxies/verify-batch`

Recommended model:
- batch endpoint only schedules verify tasks
- execution still runs via queue / runner flow
- keep status, retry, logs, observability consistent with existing task system

## Ops guidance
- use `smoke` for quick health refresh
- use `verify` before trusting region-sensitive or higher-value traffic
- treat `geo_match_ok=true` as a strong ranking signal
- treat `transparent` anonymity as lower trust than `anonymous` / `elite`


## 巡检 V1（当前已落地）

当前系统中的 batch verify 已具备以下能力：

- 支持通过 `POST /proxies/verify-batch` 按条件批量投递 `verify_proxy` 任务
- 支持 `stale_after_seconds`、`task_timeout_seconds`、`recently_used_within_seconds`、`failed_only`、`max_per_provider`
- 支持返回 `batch_id`、`created_at`
- 支持返回 `provider_summary`
- 支持把巡检批次落库到 `verify_batches`
- 支持通过 `GET /proxies/verify-batch`、`GET /proxies/verify-batch/:id` 回看批次
- 支持在批次详情中查看 `queued_count / running_count / succeeded_count / failed_count`

当前定位：
- 已经具备 **巡检批次创建、批次查询、批次进度回看、基础策略调参** 能力
- 尚未落地真正的定时调度器与多轮历史报表


## 选择闭环（巡检结果反哺 selection）

当前 proxy selection 已开始明确吃巡检结果，而不再只是简单按 `score` 排序。

当前优先级大致为：

1. **硬条件先过滤**
   - proxy 必须为 `active`
   - 不能处于 `cooldown`
   - 若任务要求 `provider / region / min_score`，必须满足

2. **强优先信号前排**
   - `last_verify_status = ok`
   - `last_verify_geo_match_ok = true`
   - `last_smoke_upstream_ok = true`

3. **风险惩罚后排**
   - `last_verify_status = failed` 的代理后排
   - `last_verify_at` 缺失的代理后排
   - `last_verify_at` 过旧（当前实现按 3600 秒口径）的代理后排

4. **最后才看资源均衡**
   - `last_used_at ASC`
   - `created_at ASC`

这意味着：
- trust score 已吸收原始分数，尾部不再单独看 `score`
- **更新鲜、已验证、地理更匹配的代理** 会更容易被选中
- **最近验证失败或验证过旧的代理** 即使分数高，也不再天然占优


### 当前已验证的关键回归

当前 selection 闭环已覆盖以下关键口径：

- fresh verified 优先于 stale high score
- verify ok 优先于 recent verify failed
- geo match verified 优先于 smoke-only
- fresh verified 优先于 missing verify
