# QUEUE_CLAIM_PLAN.md

`queue claim / durable queue` 下一步方案。

目标：解决当前“SQLite 任务状态”和“内存队列状态”分离带来的漂移问题，让任务调度从“能跑”推进到“并发下一致”。

---

## 1. 当前问题

当前实现的主链路是：

- `tasks` 持久化在 SQLite
- 运行时有一个内存队列
- worker 从内存队列 pop 任务
- 然后再到数据库里检查/推进状态

这套方案在单机原型阶段够用，但已经暴露出几个问题：

1. **DB 与内存队列不是同一个原子系统**
2. `retry` / `cancel` / `running finish` 之间容易出现状态漂移
3. 多 worker 下仍然缺少真正的 `claim` 语义
4. 进程重启后，内存队列内容会丢失，只能依赖重建逻辑

---

## 2. 下一阶段目标

下一阶段不追求“一步到位的分布式队列”，只追求：

1. **单机内强一致优先**
2. **DB 成为任务可执行状态的唯一真相源**
3. **内存队列降级为性能优化层，而不是状态真相源**
4. **worker 通过原子 claim 获取任务，而不是靠 pop 后再补检查**

---

## 3. 推荐演进方向

### Phase A：DB-first claim（下一步最优先）

核心思路：

- worker 不再“先 pop 再查 DB”
- worker 改为直接向 SQLite 发起 **claim**
- 只有 claim 成功的 worker 才拥有执行权

推荐 SQL 语义：

1. 扫描一个可执行任务：
   - `status = queued`
   - `scheduled_at <= now`（如果后续启用）
2. 用条件更新原子抢占：
   - `queued -> running`
   - 同时写入 `runner_id / started_at`
3. 根据 `rows_affected == 1` 判断是否 claim 成功

这样可以把“谁拿到任务”这个问题收敛到 DB 层。

### Phase B：内存队列降级为 hint queue

在 Phase A 落地后：

- 内存队列不再是执行真相源
- 它只作为“有任务可扫”的提示层/唤醒层
- 即便队列丢了，也可以从 DB 扫描重建

### Phase C：durable requeue / recovery

补齐以下恢复逻辑：

- 进程重启时重扫 `queued`
- 对长时间停在 `running` 的任务做 lease/超时恢复
- 对崩溃 worker 留下的任务做 reclaim

---

## 4. 推荐最小数据增强

建议先给 `tasks` 表补以下字段（若已有同类字段可复用）：

- `runner_id`：当前持有执行权的 worker
- `claim_token`：可选，标记一次运行租约
- `heartbeat_at`：可选，为后续 lease 机制准备
- `scheduled_at`：统一延迟执行/退避重试入口

不一定一步全上，但 `runner_id + scheduled_at` 已经足够支撑下一步 claim 化。

---

## 5. 推荐状态机约束

下一阶段建议收紧为：

- `queued -> running` 只能通过 **claim** 发生
- `running -> terminal` 只能由持有 claim 的 worker 收尾
- `retry` 不直接依赖内存队列是否成功 push 来决定状态正确性
- `cancel` 需要同时尊重当前 claim 状态

---

## 6. 推荐最小实现顺序

1. **新增 `claim_next_task()` DB 函数**
2. **worker loop 改成 DB claim 驱动**
3. **保留内存队列，但降级成唤醒提示**
4. **启动时补一轮 queued 任务重建扫描**
5. **补 claim 相关测试（多 worker 竞争 / retry / cancel）**

---

## 7. 当前不建议立刻做的事

先不要一上来就做：

- 分布式消息队列
- Redis / Kafka
- 多节点调度
- 复杂 lease 协议
- artifact/log 全量事务化

项目现在最缺的不是“大而全”，而是**单机内一致性主干**。

---

## 8. 一句话结论

> 下一步应该把任务执行权收回到 SQLite 的原子 claim 上，让内存队列从“真相源”降级为“性能优化层”。
