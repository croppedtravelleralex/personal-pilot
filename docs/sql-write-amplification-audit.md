# SQL / Write Amplification Audit

## 目标

识别当前 `PersonaPilot` 中，最容易在规模上来后变慢、变重、变抖的 SQL 热点与写放大来源。

## 结论先说

当前最值得优先关注的热点有 **4 类**：

1. **proxy selection 查询**
2. **verify batch / batch detail 聚合**
3. **status / metrics 聚合查询**
4. **代理状态高频回写**

## 热点 1：proxy selection 查询

当前 selection 已走 trust score 主排序。

关键特点：
- 基础筛选走 `status / provider / region / score`
- 排序中会用到 trust score SQL
- trust score SQL 内部包含：
  - provider 聚合子查询
  - provider × region 聚合子查询
  - success / failure 历史判断
  - verify 时间窗口判断

### 风险
- 候选代理量大后，单次 selection 查询成本会上升
- provider / provider×region 子查询会反复参与排序
- 当前索引 `idx_proxies_selection(status, provider, region, score DESC, last_used_at, created_at)`（历史/兼容）仅保留兼容含义；当前 trust score 主排序已不再依赖 `score DESC` 二次兜底，因此它只能覆盖一部分过滤与尾部排序，**无法真正覆盖 trust score 中的聚合和时间衰减逻辑**

### 建议
- 评估把 provider / provider×region 风险做成预聚合表或周期性物化结果
- 评估增加 `last_verify_status / last_verify_at / cooldown_until` 相关索引
- 评估将部分 trust score 组成项缓存为持久字段，而不是每次临时计算

## 热点 2：verify batch / batch detail 聚合

当前 `verify_batch` 已经落库，并支持详情回看。

### 风险
- batch 列表查 `verify_batches` 本身不重
- 但 batch detail 映射过程中还会再按 `verify_batch_id` 到 `tasks` 聚合统计 queued / running / succeeded / failed
- 当 batch 数量与 task 数量一起增长时，这会变成典型“列表轻、详情重”问题

### 建议
- 评估为 tasks 中的 `verify_batch_id` 建显式列，而不是长期依赖 `json_extract(input_json, '$.verify_batch_id')`
- 评估为 verify batch 做增量 summary 回写，减少每次详情现算
- 评估给 `verify_batches.status` / `created_at` 相关查询补索引

## 热点 3：status / metrics 聚合

当前状态页已经做成单条聚合 SQL，比早期散查更好。

### 风险
- 当前 totals / counts / verify metrics 都是聚合型查询
- 在任务量、代理量、verify 批次量进一步增长后，状态页会逐渐变成“每看一次都扫很多数据”的入口

### 建议
- 区分“实时强一致 status”与“弱实时 metrics summary”
- 对 verify metrics / provider summary 评估做缓存或周期性快照
- 为高频聚合字段补索引或做轻物化层

## 热点 4：代理状态高频回写

当前以下链路都会 `UPDATE proxies`：
- smoke
- verify
- execution success/failure
- cooldown 更新
- last_used / last_checked 更新

### 风险
- 同一代理在短时间内可能被多次写
- verify / execution / smoke 混在同一张 `proxies` 表回写，容易形成热点行
- 后续如果并发度更高，会放大写锁竞争与 WAL 压力

### 建议
- 评估把“事件流”与“当前状态”分离
- 保留 `proxies` 为最新状态表，同时增加 append-only verification / execution signal 表
- 定期或事件触发地把 signals 聚合回 `proxies`

## 当前 schema 观察

当前 schema 已有：
- `idx_proxies_selection(status, provider, region, score DESC, last_used_at, created_at)`（历史/兼容）
- `idx_proxy_session_bindings_lookup(proxy_id, provider, region, expires_at, last_used_at)`

### 明显缺口
- `verify_batches` 尚未看到独立索引
- `tasks.kind + status` / `tasks.kind + verify_batch_id` 没有专门索引
- `proxies.last_verify_status / last_verify_at / cooldown_until` 没有专门索引
- 聚合子查询依赖的 provider / region 维度缺少更贴合 trust score 的辅助结构

## 推荐优先级

### P0
1. 审核 selection 慢查询风险
2. 审核 `verify_batch` 详情聚合路径
3. 审核 `UPDATE proxies` 热点回写频率

### P1
4. 设计 verify / execution signal append-only 表
5. 设计 provider / provider×region 风险预聚合结构
6. 设计 status / metrics 快照层

## 一句话结论

> **当前最大的性能风险不是单条执行慢，而是 selection 聚合 + batch 聚合 + 高频状态回写叠加后的写放大与统计成本。**
