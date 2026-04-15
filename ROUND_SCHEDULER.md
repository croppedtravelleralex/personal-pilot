# ROUND_SCHEDULER.md

`PersonaPilot` 的轮次调度器设计。

## 目标

让项目执行从“单轮手动推进”升级为“按 5 分钟节拍自动推进到下一轮”的受控系统。

---

## 1. 职责定义

轮次调度器（Round Scheduler）负责：

1. 判断当前应执行的轮次类型
2. 推进 mini-cycle 状态流转
3. 触发下一轮执行
4. 检查上一轮是否满足完成条件
5. 失败时进入补偿逻辑
6. 在第 4 轮后触发阶段汇总

它不直接承担业务执行逻辑，而是负责：

> **决定下一轮该做什么，以及这一轮是否算真正完成。**

---

## 2. 核心输入

调度器每次运行时至少读取：

- `RUN_STATE.json`
- 最新 `round-results/*.json`
- `EXECUTION_STATE_MACHINE.md`
- `EXECUTION_CHECKLIST.md`
- 当前项目核心文档（必要时）

---

## 3. 核心输出

每次调度后至少更新：

- `RUN_STATE.json`
- 本轮 `round-results/round-<n>.json`
- 如有必要更新 `EXECUTION_LOG.md`

---

## 4. 轮次推进规则

### 4.1 标准顺序

一个 mini-cycle 固定顺序：

1. `plan`
2. `build`
3. `verify`
4. `summarize`

然后进入下一个 cycle：

5. `plan`
6. `build`
7. `verify`
8. `summarize`

### 4.2 调度规则

```text
if pendingRecovery == true:
    next = recovery(plan)
elif roundType == null or roundStatus == idle:
    next = plan
elif roundType == plan and roundStatus == completed:
    next = build
elif roundType == build and roundStatus == completed:
    next = verify
elif roundType == verify and roundStatus == completed:
    next = summarize
elif roundType == summarize and roundStatus == completed:
    next = plan (cycleId + 1)
else:
    next = recovery or blocked
```

---

## 5. 调度器需要维护的状态

建议 `RUN_STATE.json` 最终至少包含：

```text
- currentRound
- cycleId
- roundType
- roundStatus
- pendingRecovery
- currentObjective
- lastExecutionAt
- lastOutputFiles
- lastVerificationResult
- lastSummaryRound
- failureCount
- lastSchedulerDecision
- nextRoundType
- nextPlannedAt
- schedulerStatus
```

---

## 6. 完成判定逻辑

调度器不应只看“有没有说完成”，而要检查结构化结果。

### 6.1 最低判定条件

#### plan
- 有 `currentObjective`
- 有 roadmap 更新
- 有 run state 更新

#### build
- `changedFiles.length >= 1`
- 有实际产出文件

#### verify
- `verificationActions.length >= 1`
- 有验证结论

#### summarize
- 有 summary
- 有最近 4 轮汇总

若不满足，则：
- `roundStatus = failed`
- `pendingRecovery = true`

---

## 7. 补偿机制

### 7.1 进入条件
- 本轮 failed
- 本轮 blocked
- 本轮缺少 round-result 文件
- 本轮缺少关键字段

### 7.2 补偿策略
- 优先补齐缺失日志
- 补齐 run state
- 若 build 没有产出，则退回 plan 重新选目标
- 若 verify 没验证，则重新执行 verify
- 若 summarize 缺汇总，则重新生成汇总

### 7.3 恢复规则
补偿完成后：
- `pendingRecovery = false`
- `roundStatus = completed`
- 再进入下一标准轮次

---

## 8. 调度器运行模式

### 模式 A：手动触发
由人或主 agent 手动触发下一轮。

### 模式 B：cron 触发
每 5 分钟触发一次调度器：
- 读取状态
- 判定下一轮
- 执行轮次
- 回写结果

### 模式 C：受控自动推进
前 1 个 mini-cycle 先人工观察，确认后再转长时间自动运行。

---

## 9. 建议实现形态

第一版建议做成一个简单的：

- Rust 可执行子命令，或
- Python / shell 调度脚本（过渡），或
- OpenClaw 内部 cron + 状态文件驱动

建议第一版优先目标：

> 先能可靠判断“下一轮是什么”和“上一轮算不算完成”。

而不是一开始就做复杂调度框架。

---

## 10. 第一版最小功能清单

- [ ] 能读 `RUN_STATE.json`
- [ ] 能判断下一轮类型
- [ ] 能写 `nextRoundType`
- [ ] 能检查 round result 是否存在
- [ ] 能判断本轮 completed / failed
- [ ] 能设置 `pendingRecovery`
- [ ] 能在 summarize 后切到下一 cycle

---

## 11. 当前建议

在恢复“每 5 分钟跑 8 小时”之前，先做：

1. 轮次调度器设计落地
2. `RUN_STATE.json` 增加调度字段
3. 做一个 4 轮 mini-cycle 调度试运行
4. 验证 state machine + scheduler 是否闭环
