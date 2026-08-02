# SCHEMA_SCOPE.md

第一阶段数据库 schema 范围定义。

## 目标

在进入 Rust 工程初始化前，先锁定第一批必须建模的数据对象，避免代码骨架和数据层脱节。

---

## 第一阶段 schema 范围（P0）

### 1. tasks
用途：任务主表

建议字段：
- id
- task_type
- status
- payload_json
- network_policy_id
- created_at
- updated_at
- scheduled_at
- started_at
- finished_at
- retry_count
- error_summary

### 2. task_runs
用途：任务执行记录

建议字段：
- id
- task_id
- runner_type
- status
- started_at
- finished_at
- duration_ms
- error_type
- error_message
- summary

### 3. fingerprint_profiles
用途：指纹配置主表

建议字段：
- id
- name
- browser_family
- locale
- timezone
- ua
- viewport_json
- platform
- perf_budget_tag
- enabled

### 4. proxy_endpoints
用途：代理实体表

建议字段：
- id
- provider
- protocol
- host
- port
- auth_ref
- region_country
- region_area
- region_city
- status
- success_rate
- fail_count
- last_check_at
- enabled

### 5. proxy_validation_results
用途：代理验证结果表

建议字段：
- id
- proxy_id
- validator
- target_kind
- success
- latency_ms
- error_type
- started_at
- finished_at

### 6. proxy_allocations
用途：任务与代理的分配记录

建议字段：
- id
- task_id
- run_id
- proxy_id
- region_match_score
- selection_reason
- allocated_at
- released_at
- outcome

### 7. task_network_policies
用途：任务网络策略

建议字段：
- id
- require_proxy
- proxy_strategy
- target_region_country
- target_region_area
- target_region_city
- proxy_region_match_mode
- fingerprint_profile_id
- fingerprint_strategy_id
- max_proxy_retries

---

## 暂缓到第二阶段（P1）

- fingerprint_strategies
- proxy_pool_policies
- harvested_proxies
- proxy_sources
- harvest_runs
- artifacts
- execution_logs

---

## 结论

第一阶段先保证：

> 任务、执行、指纹、代理、验证、分配、网络策略 这 7 个核心对象能闭环。
