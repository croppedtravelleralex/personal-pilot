# EXECUTION_STATE_MACHINE.md

`AutoOpenBrowser` 自动执行状态机定义。

## 目标

把周期执行从“口头协议”升级成“可约束、可检查、可恢复”的执行系统。

---

## 1. 顶层状态

### idle
- 未开始执行
- 等待下一轮触发

### planning
- 正在读取上下文
- 正在更新 roadmap
- 正在选择本轮唯一主目标

### building
- 正在执行本轮最小推进动作
- 允许修改文档、schema、代码、目录结构

### verifying
- 正在做 bug 检查 / 功能验证 / 一致性检查

### summarizing
- 正在汇总最近一个 mini-cycle（4 轮）
- 正在生成阶段总结

### blocked
- 由于关键决策缺失、依赖缺失、环境问题等无法继续

### failed
- 本轮没有满足完成条件
- 需要进入补偿/重试流程

### completed
- 本轮已满足完成条件并写入轮次结果

---

## 2. 轮次类型

### plan
- 对应 planning
- 读文件、更新路线图、确定唯一目标

### build
- 对应 building
- 产生产出

### verify
- 对应 verifying
- 做检查、验证、记录问题

### summarize
- 对应 summarizing
- 汇总 4 轮结果并输出总结

---

## 3. 状态流转规则

### 标准 mini-cycle
1. idle -> planning
2. planning -> completed
3. idle -> building
4. building -> completed
5. idle -> verifying
6. verifying -> completed
7. idle -> summarizing
8. summarizing -> completed
9. completed -> idle

### 异常流转
- 任意执行态 -> blocked
- 任意执行态 -> failed
- failed -> planning（补偿）
- blocked -> planning（问题消除后）

---

## 4. 本轮唯一主目标规则

每轮只能定义一个 `currentObjective`。

如果本轮出现多个目标，则视为违规，应：
- 记录 warning
- 降级为 failed 或 no-op

---

## 5. 完成条件

### plan 轮完成条件
- 已读取必需文件
- 已更新 roadmap
- 已写入唯一主目标
- 已更新 run state

### build 轮完成条件
- 至少有 1 个真实产出
- 至少改动 1 个项目文件
- 已写入 execution log

### verify 轮完成条件
- 至少完成 1 项验证动作
- 已记录验证结果
- 已记录 bug / 风险 / 结论之一

### summarize 轮完成条件
- 已汇总最近 4 轮
- 已更新 execution log
- 已生成阶段总结文本

---

## 6. 失败判定

以下任一情况视为 failed：

- 口头说开始，但没有文件产出
- 没有更新 `RUN_STATE.json`
- 没有更新 `EXECUTION_LOG.md`
- 没有定义唯一主目标
- 验证轮没有任何验证结果
- 汇总轮没有形成总结

---

## 7. 补偿机制

当一轮失败时：
- `roundStatus = failed`
- `pendingRecovery = true`
- 下一轮优先执行补偿
- 不允许静默跳过失败轮

补偿轮应优先完成：
1. 补齐缺失日志
2. 补齐缺失状态
3. 完成原轮次最低产出条件

---

## 8. 当前建议

当前不要直接跑长周期自动执行。
先用本状态机完成：
- 1 个 mini-cycle（4 轮）试运行
- 验证状态流转是否真实闭环
