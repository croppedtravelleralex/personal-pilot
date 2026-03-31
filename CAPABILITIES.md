# CAPABILITIES

## 当前已落地能力

### 1. 任务调度与执行控制面
- DB-first task queue
- 原子 claim next task
- runner heartbeat
- stale running reclaim
- retry / cancel / rerun 基础链路
- worker idle backoff / jitter / error cap

### 2. Fingerprint profile 基础能力
- fingerprint profile 创建 / 查询
- task 创建时引用 profile id + version
- 执行前解析 active + version 匹配的 profile
- 执行结果与状态接口暴露 fingerprint resolution 状态

### 3. Proxy pool V1
- proxy 创建 / 列表 / 查询
- provider / region / min_score / cooldown 过滤
- 基于 score + last_used_at 的基础选择策略
- sticky session 正式绑定表 `proxy_session_bindings`
- sticky reuse 命中与有效性校验

### 4. Proxy verification / smoke test
- `POST /proxies/:id/smoke`
- TCP reachability 检测
- HTTP CONNECT 协议响应检测
- 上游 IP 回显信号
- 匿名性分级（transparent / anonymous / elite）
- smoke 结果写回 proxy 健康模型：
  - `last_smoke_status`
  - `last_smoke_protocol_ok`
  - `last_smoke_upstream_ok`
  - `last_exit_ip`
  - `last_anonymity_level`
  - `last_smoke_at`

### 5. 状态观测
- `/status` 聚合 counts
- fingerprint metrics summary
- proxy metrics summary
- worker backoff 参数状态暴露
- task detail / status 暴露 proxy identity 与 resolution 状态

## 当前仍属于 V1 / 临时方案的部分
- proxy 选择仍是轻量查库排序，不是正式调度器
- smoke test 仍偏“最小验证链”，不是完整真实匿名性评估
- 未完成真实外部探针的国家/地区校验
- reclaim 后半段 runs/logs 更新仍可继续收口

## 下一阶段最值得推进的方向
1. 更真实的匿名性 / 地区校验链
2. proxy 选择索引与策略正式化
3. 更完整的能力总览与 API 文档对齐

## 已明确但尚未落地的验证增强方向
- 外部 probe endpoint 验证
- 出口 country / region 回显
- geo match 判定
- 独立 `verify` 慢路径
- verification score delta 回写
