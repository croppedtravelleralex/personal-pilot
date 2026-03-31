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
4. `score DESC`
5. `last_used_at ASC`
6. `created_at ASC`

Meaning:
- verified and geo-matching proxies are preferred over merely high-score proxies
- smoke upstream success is a useful secondary hint
- score is still relevant but is no longer the only strong signal

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
